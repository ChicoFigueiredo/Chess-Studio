//! Autorização do Lichess: Conexão autorizada pelo navegador (OAuth2 Authorization Code com
//! PKCE S256, cliente público, sem segredo de cliente) ou por token pessoal colado, conferida
//! em `/api/account` e guardada no Cofre; revogação com `DELETE /api/token`.
//!
//! O app nunca vê a senha do Lichess. Abrir o navegador é tarefa da interface: aqui ficam a
//! URL ([`AuthFlow::url`]) e a máquina de estados ([`AuthState`]: aguardando, autorizado,
//! recusado, expirado, falhou). O retorno chega ao servidor de loopback do `chess-net`
//! ([`chess_net::LoopbackCallback`]): `127.0.0.1`, porta efêmera, um único retorno, com
//! prazo.
//!
//! Escopo mínimo por papel ([`ConnectionRole`]): jogador, `board:play`; Engine online,
//! `bot:play`. Um token com os dois, ou sem o do papel, é recusado e não é guardado. O
//! token só é guardado depois de conferido: até lá fica em conferência no Cofre
//! ([`chess_vault::SecretVault::stage`]), só em memória.
//!
//! Fatos do serviço (documentação oficial, conferida em 2026-11-11): autorização em
//! `https://lichess.org/oauth`, troca em `POST https://lichess.org/api/token`
//! (form-urlencoded, sem `client_secret`), `GET /api/account` responde os escopos do token no
//! cabeçalho `X-OAuth-Scopes`, revogação em `DELETE /api/token` com o token como `Bearer`.

use crate::family::ProviderProfile;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chess_net::{
    CallbackError, CallbackParams, Clock, ConnectionId, Fetched, LocalProfileId, LoopbackCallback,
    Net, NetError, Request, Secret, Timestamp,
};
use chess_vault::{SecretVault, VaultError, VaultStatus};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// Identificador do app no Lichess (cliente público: qualquer texto que identifique o app).
pub const CLIENT_ID: &str = "chess-studio";
/// Caminho do retorno no servidor de loopback.
const CALLBACK_PATH: &str = "/chess-studio/retorno";
/// Prazo padrão para concluir a autorização no navegador.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(10 * 60);
/// Tamanho máximo aceito para o código de autorização.
const MAX_CODE_LEN: usize = chess_net::MAX_TOKEN_LEN;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Para que serve a Conexão do Lichess. A do jogador e a de Engine nunca se misturam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionRole {
    /// Jogador (Board API): escopo `board:play`.
    Player,
    /// Engine online (Bot API, conta BOT separada): escopo `bot:play`.
    Engine,
}

impl ConnectionRole {
    /// O escopo mínimo que a autorização pede para o papel.
    pub const fn scope(self) -> &'static str {
        match self {
            Self::Player => "board:play",
            Self::Engine => "bot:play",
        }
    }

    /// O escopo do papel no Provedor do perfil.
    pub fn scope_in(self, profile: &ProviderProfile) -> &'static str {
        match self {
            Self::Player => profile.player_scope,
            Self::Engine => profile.engine_scope,
        }
    }

    const fn other(self) -> Self {
        match self {
            Self::Player => Self::Engine,
            Self::Engine => Self::Player,
        }
    }
}

/// A conta confirmada pelo Lichess em `/api/account`, com os escopos reais do token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LichessAccount {
    pub id: String,
    pub username: String,
    /// Escopos do token, como o Lichess os informa (em ordem).
    pub scopes: Vec<String>,
    /// Conta BOT (título `BOT`).
    pub is_bot: bool,
}

/// Por que a autorização, a conferência ou a revogação não deram certo. Nenhuma variante
/// carrega token, código ou verificador.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthError {
    /// O Lichess está desligado na Central: nada foi enviado.
    Disabled,
    /// O Cofre não pode guardar agora (sem cofre, bloqueado, falha).
    Vault(VaultError),
    /// Falha local: a porta de loopback não abriu, ou faltou o gerador aleatório ou uma
    /// thread do sistema.
    Local { detail: &'static str },
    /// O `state` do retorno não confere: retorno forjado ou de outra autorização.
    StateMismatch,
    /// O retorno não trouxe um código válido.
    MalformedCallback,
    /// O Lichess recusou o código (já usado, vencido ou de outra autorização).
    CodeRejected,
    /// Token vazio, com espaço ou caractere de controle, ou longo demais.
    InvalidToken,
    /// O Lichess não aceita o token (expirado, revogado ou inexistente).
    TokenRejected,
    /// O token não tem o escopo do papel da Conexão, ou tem o do outro papel.
    WrongScopes {
        role: ConnectionRole,
        scopes: Vec<String>,
    },
    /// O Lichess não informou os escopos do token: sem eles, nada é guardado.
    ScopesUnconfirmed,
    /// A resposta do Lichess não tem o formato documentado.
    Malformed { detail: String },
    /// Falha da camada de rede (sem conexão, prazo, HTTP inesperado…).
    Net(NetError),
    /// Cancelado por quem pediu.
    Cancelled,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str(
                "O Lichess está desligado. Ligue-o em Arquivo > Conexões; nada foi enviado.",
            ),
            Self::Vault(e) => e.fmt(f),
            Self::Local { detail } => write!(f, "Falha local na autorização: {detail}."),
            Self::StateMismatch => f.write_str(
                "O retorno do navegador não corresponde a esta autorização; nada foi aceito. \
                 Comece de novo.",
            ),
            Self::MalformedCallback => {
                f.write_str("O retorno do navegador veio incompleto; comece de novo.")
            }
            Self::CodeRejected => f.write_str(
                "O Lichess recusou o código de autorização (já usado ou vencido); comece de novo.",
            ),
            Self::InvalidToken => f.write_str(
                "O token não tem o formato de um token do Lichess (vazio, com espaços ou longo \
                 demais).",
            ),
            Self::TokenRejected => {
                f.write_str("O Lichess não aceita este token (expirado, revogado ou inexistente).")
            }
            Self::WrongScopes { role, scopes } => {
                let found = if scopes.is_empty() {
                    "nenhum".to_owned()
                } else {
                    scopes.join(", ")
                };
                match role {
                    ConnectionRole::Player => write!(
                        f,
                        "Uma Conexão de jogador precisa do escopo board:play e não pode ter o \
                         de Engine (bot:play). Escopos do token: {found}."
                    ),
                    ConnectionRole::Engine => write!(
                        f,
                        "Uma Conexão de Engine precisa do escopo bot:play e não pode ter o de \
                         jogador (board:play). Escopos do token: {found}."
                    ),
                }
            }
            Self::ScopesUnconfirmed => f.write_str(
                "O Lichess não informou os escopos do token; por segurança ele não foi guardado.",
            ),
            Self::Malformed { detail } => {
                write!(
                    f,
                    "A resposta do Lichess não tem o formato esperado ({detail})."
                )
            }
            Self::Net(e) => e.fmt(f),
            Self::Cancelled => f.write_str("Autorização cancelada."),
        }
    }
}

impl AuthError {
    /// A mensagem com o nome do Provedor do perfil no lugar de «Lichess» (a [`fmt::Display`]
    /// fala do Lichess, o padrão). Para a interface mostrar o erro de outro Provedor da
    /// família, o PlayStrategy.
    pub fn describe(&self, profile: &ProviderProfile) -> String {
        self.to_string().replace("Lichess", profile.name)
    }
}

impl std::error::Error for AuthError {}

/// Situação de uma autorização pelo navegador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// Aguardando o usuário no navegador (e, depois do retorno, a conferência do token).
    Waiting,
    /// Token conferido e guardado no Cofre.
    Authorized(LichessAccount),
    /// O usuário recusou no Lichess.
    Refused,
    /// O prazo passou sem retorno.
    Expired,
    Failed(AuthError),
}

/// Uma autorização pelo navegador em andamento. Abra [`AuthFlow::url`] no navegador do
/// sistema e consulte [`AuthFlow::state`] a cada quadro. Descartar cancela.
pub struct AuthFlow {
    url: String,
    state: Arc<Mutex<AuthState>>,
    cancel: Arc<AtomicBool>,
}

impl fmt::Debug for AuthFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthFlow")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl AuthFlow {
    /// Endereço de autorização do Lichess, para o navegador do sistema. Leva o desafio PKCE
    /// e o `state`; nunca o verificador nem token.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Situação atual (barato; pode ser lido a cada quadro).
    pub fn state(&self) -> AuthState {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Desiste: a porta local fecha e nada é guardado.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Espera (tempo real) até sair de [`AuthState::Waiting`] ou até `timeout`. Para testes e
    /// threads de fundo, nunca para a interface.
    pub fn wait(&self, timeout: Duration) -> AuthState {
        let start = Instant::now();
        loop {
            let state = self.state();
            if state != AuthState::Waiting || start.elapsed() >= timeout {
                return state;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for AuthFlow {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Uma operação de Conexão a caminho (token pessoal, conferência, revogação). Consulte com
/// [`AuthTask::try_take`] a cada quadro (nunca bloqueia).
#[derive(Debug)]
pub struct AuthTask<T> {
    rx: Receiver<T>,
    done: bool,
}

impl<T: Send + 'static> AuthTask<T> {
    fn spawn(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        // Sem a thread (sistema sem recursos), o canal fecha e a tarefa nunca termina.
        let _ = thread::Builder::new().name(name.to_owned()).spawn(move || {
            let _ = tx.send(work());
        });
        Self { rx, done: false }
    }

    fn ready(value: T) -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(value);
        Self { rx, done: false }
    }

    /// O resultado, se já chegou. Entregue uma vez.
    pub fn try_take(&mut self) -> Option<T> {
        if self.done {
            return None;
        }
        match self.rx.try_recv() {
            Ok(value) => {
                self.done = true;
                Some(value)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// Espera até `timeout` (tempo real). Para testes e threads de fundo.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<T> {
        if self.done {
            return None;
        }
        match self.rx.recv_timeout(timeout) {
            Ok(value) => {
                self.done = true;
                Some(value)
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }
}

/// Resultado de uma revogação: o segredo local é apagado sempre; a revogação no Lichess é
/// informada à parte, sem afirmar sucesso se a chamada falhou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    /// `DELETE /api/token` no Lichess.
    pub remote: Result<(), AuthError>,
    /// Apagar o segredo e a referência no Cofre.
    pub local: Result<(), VaultError>,
}

/// As Conexões autorizadas do Lichess: autorizar pelo navegador, aceitar token pessoal,
/// conferir e revogar. Barato de clonar.
#[derive(Clone)]
pub struct LichessAuth {
    profile: ProviderProfile,
    net: Net,
    vault: SecretVault,
    clock: Arc<dyn Clock>,
    deadline: Duration,
}

impl fmt::Debug for LichessAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LichessAuth")
            .field("vault", &self.vault)
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

/// Texto aleatório em base64url (sem preenchimento) de `N` bytes do gerador do sistema.
fn random_text<const N: usize>() -> Result<Zeroizing<String>, AuthError> {
    let mut bytes = Zeroizing::new([0u8; N]);
    getrandom::fill(bytes.as_mut()).map_err(|_| AuthError::Local {
        detail: "gerador aleatório do sistema indisponível",
    })?;
    Ok(Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_ref())))
}

/// Desafio PKCE S256: `BASE64URL(SHA256(verificador))`.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Percent-encoding de componente (RFC 3986: só os não reservados passam).
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub(crate) fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Comparação em tempo constante (o `state` não vaza por tempo de resposta).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Código de autorização aceitável: ASCII visível, sem espaço, de 1 a [`MAX_CODE_LEN`] bytes.
fn well_formed(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_CODE_LEN && value.bytes().all(|b| b.is_ascii_graphic())
}

#[derive(Deserialize)]
struct WireToken {
    token_type: String,
    access_token: String,
}

#[derive(Deserialize)]
struct WireAccount {
    id: String,
    username: String,
    #[serde(default)]
    title: Option<String>,
}

fn parse_scopes(header: &str) -> Vec<String> {
    let mut scopes: Vec<String> = header
        .split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

fn net_error(error: NetError) -> AuthError {
    match error {
        NetError::ProviderDisabled { .. } => AuthError::Disabled,
        NetError::Cancelled => AuthError::Cancelled,
        NetError::ConnectionRejected { .. } => AuthError::TokenRejected,
        other => AuthError::Net(other),
    }
}

/// O Cofre aceita guardar agora?
fn vault_ready(vault: &SecretVault) -> Result<(), AuthError> {
    match vault.status() {
        VaultStatus::System | VaultStatus::OwnUnlocked => Ok(()),
        VaultStatus::OwnLocked => Err(AuthError::Vault(VaultError::Locked)),
        VaultStatus::Unavailable => Err(AuthError::Vault(VaultError::Unavailable)),
    }
}

/// O que a thread da autorização pelo navegador leva.
struct Pending {
    redirect_uri: String,
    verifier: Zeroizing<String>,
    expected_state: Zeroizing<String>,
    deadline: Timestamp,
    connection: ConnectionId,
    role: ConnectionRole,
    profile: LocalProfileId,
}

impl LichessAuth {
    /// `clock` é o relógio injetado (o mesmo da `Net`): nele corre o prazo do navegador.
    pub fn new(net: Net, vault: SecretVault, clock: Arc<dyn Clock>) -> Self {
        Self::with_profile(ProviderProfile::LICHESS, net, vault, clock)
    }

    /// A mesma autorização para outro Provedor da família (o PlayStrategy): endereços,
    /// escopos e Provedor vêm do perfil; o fluxo é o mesmo (OAuth2 PKCE S256).
    pub fn with_profile(
        profile: ProviderProfile,
        net: Net,
        vault: SecretVault,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            profile,
            net,
            vault,
            clock,
            deadline: DEFAULT_DEADLINE,
        }
    }

    /// O perfil do Provedor desta autorização.
    pub fn provider_profile(&self) -> &ProviderProfile {
        &self.profile
    }

    /// Prazo para o usuário concluir no navegador (padrão: [`DEFAULT_DEADLINE`]).
    pub fn deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    /// Começa a autorização pelo navegador da Conexão `connection` com o papel `role`: abre
    /// o servidor de loopback e devolve o fluxo com a URL. Falha na hora (sem rede) se o
    /// Lichess está desligado ou o Cofre não pode guardar.
    pub fn authorize(
        &self,
        connection: ConnectionId,
        role: ConnectionRole,
    ) -> Result<AuthFlow, AuthError> {
        if !self.net.is_enabled(self.profile.id) {
            return Err(AuthError::Disabled);
        }
        vault_ready(&self.vault)?;
        let callback = LoopbackCallback::bind(CALLBACK_PATH).map_err(|_| AuthError::Local {
            detail: "a porta local para o retorno do navegador não abriu",
        })?;
        let redirect_uri = callback.redirect_uri();
        // 32 bytes aleatórios: verificador de 43 caracteres (RFC 7636) e `state` imprevisível.
        let verifier = random_text::<32>()?;
        let expected_state = random_text::<32>()?;
        let url = format!(
            "{}?{}",
            self.profile.url(self.profile.authorize_path),
            form(&[
                ("response_type", "code"),
                ("client_id", CLIENT_ID),
                ("redirect_uri", &redirect_uri),
                ("code_challenge_method", "S256"),
                ("code_challenge", &pkce_challenge(&verifier)),
                ("scope", role.scope_in(&self.profile)),
                ("state", &expected_state),
            ])
        );
        let state = Arc::new(Mutex::new(AuthState::Waiting));
        let cancel = Arc::new(AtomicBool::new(false));
        let pending = Pending {
            redirect_uri,
            verifier,
            expected_state,
            deadline: self.clock.now().plus(self.deadline),
            connection,
            role,
            profile: self.net.active_profile(),
        };
        let (auth, shared, stop) = (self.clone(), state.clone(), cancel.clone());
        thread::Builder::new()
            .name("chess-providers-autorizacao".to_owned())
            .spawn(move || {
                let outcome = auth.run_flow(callback, &pending, &stop);
                *shared.lock().unwrap_or_else(PoisonError::into_inner) = outcome;
            })
            .map_err(|_| AuthError::Local {
                detail: "thread da autorização",
            })?;
        Ok(AuthFlow { url, state, cancel })
    }

    fn run_flow(
        &self,
        callback: LoopbackCallback,
        pending: &Pending,
        cancel: &AtomicBool,
    ) -> AuthState {
        let params = match callback.wait(self.clock.as_ref(), pending.deadline, cancel) {
            Ok(params) => params,
            Err(CallbackError::Expired) => return AuthState::Expired,
            Err(CallbackError::Cancelled) => return AuthState::Failed(AuthError::Cancelled),
            Err(CallbackError::Unavailable) => {
                return AuthState::Failed(AuthError::Local {
                    detail: "a porta local para o retorno do navegador fechou",
                });
            }
        };
        match self.finish_flow(&params, pending, cancel) {
            Ok(account) => AuthState::Authorized(account),
            Err(Refusal) => AuthState::Refused,
            Err(Failure(error)) => AuthState::Failed(error),
        }
    }

    /// Do retorno do navegador até o token guardado.
    fn finish_flow(
        &self,
        params: &CallbackParams,
        pending: &Pending,
        cancel: &AtomicBool,
    ) -> Result<LichessAccount, FlowEnd> {
        // O `state` primeiro: sem ele conferido, nada do retorno é levado em conta (nem a
        // recusa).
        let state_ok = params.count("state") == 1
            && params
                .get("state")
                .is_some_and(|s| same(s, &pending.expected_state));
        if !state_ok {
            return Err(Failure(AuthError::StateMismatch));
        }
        if params.get("error").is_some() {
            return if params.get("error") == Some("access_denied") {
                Err(Refusal)
            } else {
                Err(Failure(AuthError::MalformedCallback))
            };
        }
        let code = match params.get("code") {
            Some(code) if params.count("code") == 1 && well_formed(code) => code,
            _ => return Err(Failure(AuthError::MalformedCallback)),
        };
        if cancel.load(Ordering::Acquire) {
            return Err(Failure(AuthError::Cancelled));
        }
        let token = self.exchange(code, pending).map_err(Failure)?;
        self.confirm_and_store(
            pending.profile,
            pending.connection,
            pending.role,
            token,
            cancel,
        )
        .map_err(Failure)
    }

    /// Troca o código pelo token (`POST /api/token`, form-urlencoded, sem segredo de
    /// cliente).
    fn exchange(&self, code: &str, pending: &Pending) -> Result<Secret, AuthError> {
        let body = Zeroizing::new(form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", &pending.verifier),
            ("redirect_uri", &pending.redirect_uri),
            ("client_id", CLIENT_ID),
        ]));
        let request = Request::post(self.profile.id, self.profile.url(self.profile.token_path))
            .accept("application/json")
            .timeout(REQUEST_TIMEOUT)
            .body(
                "application/x-www-form-urlencoded",
                body.as_bytes().to_vec(),
            );
        let mut fetched = match self.net.fetch(request).wait() {
            Ok(fetched) => fetched,
            Err(NetError::Http { status: 400, .. }) => return Err(AuthError::CodeRejected),
            Err(error) => return Err(net_error(error)),
        };
        let token = take_token(&fetched);
        fetched.body.zeroize();
        token
    }

    /// Confere o token em `/api/account` (nome e escopos reais) com ele só em conferência no
    /// Cofre; guarda se o papel bate e ninguém desistiu.
    fn confirm_and_store(
        &self,
        profile: LocalProfileId,
        connection: ConnectionId,
        role: ConnectionRole,
        token: Secret,
        cancel: &AtomicBool,
    ) -> Result<LichessAccount, AuthError> {
        let staged = self
            .vault
            .stage(profile, self.profile.id, connection, token);
        let account = self.account(connection)?;
        let wrong = !account
            .scopes
            .iter()
            .any(|s| s == role.scope_in(&self.profile))
            || account
                .scopes
                .iter()
                .any(|s| s == role.other().scope_in(&self.profile));
        if wrong {
            return Err(AuthError::WrongScopes {
                role,
                scopes: account.scopes,
            });
        }
        if cancel.load(Ordering::Acquire) {
            return Err(AuthError::Cancelled);
        }
        staged.commit().map_err(AuthError::Vault)?;
        Ok(account)
    }

    /// `GET /api/account` pela Conexão (bloqueia: chame de uma thread de fundo).
    fn account(&self, connection: ConnectionId) -> Result<LichessAccount, AuthError> {
        let fetched = self
            .net
            .fetch(
                Request::get(self.profile.id, self.profile.url(self.profile.account_path))
                    .accept("application/json")
                    .timeout(REQUEST_TIMEOUT)
                    .connection(connection),
            )
            .wait()
            .map_err(net_error)?;
        let wire: WireAccount =
            serde_json::from_slice(&fetched.body).map_err(|e| AuthError::Malformed {
                detail: e.to_string(),
            })?;
        if wire.id.is_empty() || wire.username.is_empty() {
            return Err(AuthError::Malformed {
                detail: "conta sem identificador".to_owned(),
            });
        }
        let scopes = fetched
            .oauth_scopes
            .as_deref()
            .map(parse_scopes)
            .ok_or(AuthError::ScopesUnconfirmed)?;
        Ok(LichessAccount {
            id: wire.id,
            username: wire.username,
            scopes,
            is_bot: wire.title.as_deref() == Some("BOT"),
        })
    }

    /// Token pessoal colado (fluxo avançado): confere em `/api/account`, recusa escopo do
    /// outro papel e guarda no Cofre, substituindo o anterior só se o novo for aceito.
    pub fn personal_token(
        &self,
        connection: ConnectionId,
        role: ConnectionRole,
        token: Secret,
    ) -> AuthTask<Result<LichessAccount, AuthError>> {
        if !self.net.is_enabled(self.profile.id) {
            return AuthTask::ready(Err(AuthError::Disabled));
        }
        if let Err(e) = vault_ready(&self.vault) {
            return AuthTask::ready(Err(e));
        }
        let token = token.trimmed();
        if !token.is_token_like() {
            return AuthTask::ready(Err(AuthError::InvalidToken));
        }
        let profile = self.net.active_profile();
        let auth = self.clone();
        AuthTask::spawn("chess-providers-token", move || {
            auth.confirm_and_store(profile, connection, role, token, &AtomicBool::new(false))
        })
    }

    /// Confere a Conexão guardada: nome e escopos reais (um `401` marca a Conexão no Cofre).
    pub fn check(&self, connection: ConnectionId) -> AuthTask<Result<LichessAccount, AuthError>> {
        let auth = self.clone();
        AuthTask::spawn("chess-providers-conferir", move || auth.account(connection))
    }

    /// Revoga a Conexão: `DELETE /api/token` no Lichess com o token e, **sempre**, apaga o
    /// segredo local, mesmo se a chamada remota falhar (que é informada em
    /// [`Revocation::remote`]).
    pub fn revoke(&self, connection: ConnectionId) -> AuthTask<Revocation> {
        let auth = self.clone();
        AuthTask::spawn("chess-providers-revogar", move || {
            let remote = auth
                .net
                .fetch(
                    Request::delete(auth.profile.id, auth.profile.url(auth.profile.token_path))
                        .timeout(REQUEST_TIMEOUT)
                        .connection(connection),
                )
                .wait()
                .map(|_| ())
                .map_err(net_error);
            let local = auth.vault.delete(connection);
            Revocation { remote, local }
        })
    }
}

/// Como o fluxo pelo navegador terminou sem autorização.
enum FlowEnd {
    Refusal,
    Failure(AuthError),
}

use FlowEnd::{Failure, Refusal};

/// O token da resposta da troca, validado.
fn take_token(fetched: &Fetched) -> Result<Secret, AuthError> {
    let wire: WireToken =
        serde_json::from_slice(&fetched.body).map_err(|_| AuthError::Malformed {
            detail: "resposta da troca do código".to_owned(),
        })?;
    let WireToken {
        token_type,
        access_token,
    } = wire;
    let token = Secret::new(access_token);
    if !token_type.eq_ignore_ascii_case("bearer") {
        return Err(AuthError::Malformed {
            detail: "tipo de token diferente de Bearer".to_owned(),
        });
    }
    if !token.is_token_like() {
        return Err(AuthError::Malformed {
            detail: "token vazio, longo demais ou com caracteres inválidos".to_owned(),
        });
    }
    Ok(token)
}
