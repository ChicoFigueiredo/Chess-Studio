//! Autorizar uma Conexão do Lichess pela Central (CX-11, #231): a máquina de estados que a tela
//! vê, sem egui.
//!
//! Dois caminhos levam a uma Conexão **autorizada** (segredo no Cofre, nunca no banco):
//!
//! - **pelo navegador** ([`Intent::Browser`]): o app abre a página de autorização do Lichess no
//!   navegador do sistema ([`browser::open_authorization`]), mostra a espera com "Cancelar" e lê
//!   [`AuthFlow::state`] a cada quadro: autorizado, recusado, prazo vencido ou falha;
//! - **por chave pessoal** ([`Intent::Key`], fluxo avançado): a chave colada num campo mascarado
//!   vai direto ao Cofre em conferência e só é guardada depois que o Lichess a confirma.
//!
//! Antes de rodar, o pedido passa por duas portas, sem nunca bloquear a interface: o Provedor
//! ligado (senão abre o diálogo "Ligar Lichess?") e o Cofre pronto (senão abre o diálogo da
//! senha-mestra: criar, desbloquear). Quando a porta abre, o pedido continua sozinho.
//!
//! O `id` da Conexão é a chave do segredo no Cofre. Por isso a linha é **reservada** antes
//! ([`chess_store::Store::begin_authorization`]) e descartada, junto com o segredo, se a
//! autorização não termina. A Conexão de jogador e a de Engine online ([`Slot`]) nunca se
//! misturam: cada uma tem a sua autorização, a sua permissão mínima e a sua entrada na tela.

use super::{ConnectionsUi, Note, browser};
use crate::net_service::{NetService, Preload};
use crate::persistence::PersistenceHandle;
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::secrets::{self, ForgetError, Job, Standing, net_id};
use crate::training_ui::with_store;
use chess_net::{ProviderId, Secret, Timestamp};
use chess_providers::{
    AuthError, AuthFlow, AuthState, AuthTask, ConnectionRole as AuthRole, DEFAULT_DEADLINE,
    LichessAccount, Revocation,
};
use chess_store::{Connection, ConnectionId, ConnectionKind, ConnectionRole as Role, StoreError};
use chess_vault::{VaultError, VaultStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use zeroize::{Zeroize, Zeroizing};

/// Uma das duas entradas de autorização de um Provedor: jogador ou Engine online.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Slot {
    pub provider: ProviderId,
    pub role: Role,
}

impl Slot {
    pub fn player(provider: ProviderId) -> Self {
        Self {
            provider,
            role: Role::Player,
        }
    }

    pub fn engine(provider: ProviderId) -> Self {
        Self {
            provider,
            role: Role::Engine,
        }
    }

    pub fn is_engine(self) -> bool {
        self.role == Role::Engine
    }

    fn auth_role(self) -> AuthRole {
        match self.role {
            Role::Player => AuthRole::Player,
            Role::Engine => AuthRole::Engine,
        }
    }
}

/// O que a pessoa pediu, guardado enquanto uma porta (Provedor desligado, Cofre trancado) pede
/// uma decisão dela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Intent {
    /// Autorizar pelo navegador. `renew`: a Conexão a renovar (a expirada), senão uma nova.
    Browser {
        slot: Slot,
        renew: Option<ConnectionId>,
    },
    /// Guardar a chave pessoal que está no formulário de `slot`.
    Key {
        slot: Slot,
        renew: Option<ConnectionId>,
    },
}

impl Intent {
    pub fn slot(self) -> Slot {
        match self {
            Self::Browser { slot, .. } | Self::Key { slot, .. } => slot,
        }
    }

    fn renew(self) -> Option<ConnectionId> {
        match self {
            Self::Browser { renew, .. } | Self::Key { renew, .. } => renew,
        }
    }

    fn kind(self) -> ConnectionKind {
        match self {
            Self::Browser { .. } => ConnectionKind::AuthorizedBrowser,
            Self::Key { .. } => ConnectionKind::AuthorizedKey,
        }
    }
}

/// A porta que o pedido espera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// Resposta ao diálogo "Ligar <Provedor>?".
    Consent,
    /// Senha-mestra (criar ou desbloquear).
    Vault,
}

/// A autorização que está acontecendo agora (uma por vez).
pub(crate) struct Attempt {
    pub slot: Slot,
    pub connection: ConnectionId,
    pub profile: chess_store::ProfileId,
    pub kind: ConnectionKind,
    /// Renovando uma Conexão que já existia: se falhar, ela continua como estava.
    pub renewing: bool,
    pub started: Timestamp,
    /// O navegador não abriu: a tela oferece copiar o endereço.
    pub open_failed: bool,
    work: Work,
}

enum Work {
    Browser(AuthFlow),
    /// `AuthTask` não é `Sync`: o `Mutex` deixa o estado viver num recurso do Bevy.
    Key(Mutex<AuthTask<Result<LichessAccount, AuthError>>>),
}

impl std::fmt::Debug for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attempt")
            .field("slot", &self.slot)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Attempt {
    /// Minutos que faltam para o prazo do navegador vencer (no mínimo 1 enquanto espera).
    pub fn minutes_left(&self, now: Timestamp) -> u64 {
        let end = self.started.plus(DEFAULT_DEADLINE);
        super::model::minutes_left(now, end)
    }

    pub fn is_browser(&self) -> bool {
        matches!(self.work, Work::Browser(_))
    }

    /// O endereço de autorização, para "Abrir de novo" e "Copiar endereço".
    pub fn url(&self) -> Option<&str> {
        match &self.work {
            Work::Browser(flow) => Some(flow.url()),
            Work::Key(_) => None,
        }
    }

    fn cancel(&self) {
        if let Work::Browser(flow) = &self.work {
            flow.cancel();
        }
    }
}

/// Como terminou a última autorização de uma entrada, para o cartão mostrar e oferecer a
/// próxima ação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    /// O Lichess confirmou: o nome e o que a autorização permite.
    Authorized {
        name: String,
        scopes: Vec<String>,
        kind: ConnectionKind,
    },
    /// A pessoa recusou na página do Lichess.
    Refused,
    /// O prazo para autorizar no navegador acabou.
    TimedOut,
    /// A pessoa não quis ligar o Provedor.
    Declined,
    /// Não deu certo: id Fluent da mensagem (e um argumento).
    Problem(Note),
}

/// Formulário da chave pessoal de uma entrada. O texto é zerado ao descartar e nunca sai da
/// memória do app (nem em log, nem para a área de transferência).
pub(crate) struct KeyForm {
    pub secret: Zeroizing<String>,
    pub error: Option<&'static str>,
    /// A Conexão a renovar com a nova chave (a expirada); `None`: uma nova.
    pub renew: Option<ConnectionId>,
}

impl KeyForm {
    fn new(renew: Option<ConnectionId>) -> Self {
        Self {
            secret: Zeroizing::new(String::new()),
            error: None,
            renew,
        }
    }
}

/// O que o diálogo da senha-mestra faz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VaultMode {
    Create,
    Unlock,
    Change,
}

/// O diálogo da senha-mestra (cofre próprio). Os campos são zerados ao descartar.
pub(crate) struct VaultDialog {
    pub mode: VaultMode,
    pub current: Zeroizing<String>,
    pub password: Zeroizing<String>,
    pub confirm: Zeroizing<String>,
    /// "Entendi que não há como recuperar a senha" (só ao criar).
    pub understood: bool,
    pub error: Option<&'static str>,
    busy: Option<Job<Result<(), VaultError>>>,
}

impl VaultDialog {
    pub fn new(mode: VaultMode) -> Self {
        Self {
            mode,
            current: Zeroizing::new(String::new()),
            password: Zeroizing::new(String::new()),
            confirm: Zeroizing::new(String::new()),
            understood: false,
            error: None,
            busy: None,
        }
    }

    /// O Argon2id está rodando em fundo: os campos ficam travados.
    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    /// Preenche o diálogo e o deixa "trabalhando" para sempre (cena de captura).
    pub fn set_busy_for_demo(&mut self) {
        self.password = Zeroizing::new("senha-de-demonstracao".to_owned());
        self.confirm = Zeroizing::new("senha-de-demonstracao".to_owned());
        self.understood = true;
        self.busy = Some(Job::spawn("chess-studio-demo", || {
            std::thread::sleep(std::time::Duration::from_secs(3600));
            Ok(())
        }));
    }
}

/// A conferência ("Testar") de uma Conexão autorizada.
pub(crate) enum Check {
    Running(Mutex<AuthTask<Result<LichessAccount, AuthError>>>),
    Done {
        result: Result<String, &'static str>,
        at: Timestamp,
    },
}

impl std::fmt::Debug for Check {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running(_) => f.write_str("Running"),
            Self::Done { result, .. } => write!(f, "Done({result:?})"),
        }
    }
}

/// Tudo o que a Central sabe sobre autorizações (um campo do [`ConnectionsUi`]). O `Debug` é
/// de propósito vazio: aqui moram a senha-mestra e a chave pessoal em digitação.
#[derive(Default)]
pub(crate) struct Auth {
    pub attempt: Option<Attempt>,
    /// Como terminou a última tentativa de cada entrada.
    pub endings: HashMap<Slot, Ending>,
    pub key_forms: HashMap<Slot, KeyForm>,
    /// O pedido que espera uma porta, e qual.
    pub waiting: Option<(Intent, Gate)>,
    /// O último pedido de cada entrada, para "Tentar de novo".
    pub last: HashMap<Slot, Intent>,
    pub vault_dialog: Option<VaultDialog>,
    /// Desconexões em andamento (revogação + apagar).
    revoking: HashMap<ConnectionId, Mutex<AuthTask<Revocation>>>,
    pub checks: HashMap<ConnectionId, Check>,
    /// A situação de cada Conexão autorizada (derivada do Cofre e do Provedor).
    pub standings: HashMap<ConnectionId, Standing>,
    /// Quais entradas têm o detalhe da Engine online aberto.
    pub engine_open: std::collections::HashSet<ProviderId>,
    /// A Conexão que espera confirmação para ser desconectada.
    pub confirm_disconnect: Option<ConnectionId>,
    signature: Option<Signature>,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Signature {
    profile: chess_store::ProfileId,
    status: VaultStatus,
    loaded: bool,
    list: Vec<(ConnectionId, Option<i64>, bool)>,
    rejected: Vec<(i64, bool)>,
}

impl Auth {
    pub fn is_busy(&self, provider: ProviderId) -> bool {
        self.attempt
            .as_ref()
            .is_some_and(|a| a.slot.provider == provider)
    }

    pub fn is_disconnecting(&self, id: ConnectionId) -> bool {
        self.revoking.contains_key(&id)
    }
}

/// O abridor de páginas do navegador: o de verdade em produção; os testes põem um gravador
/// (nenhum teste abre navegador).
#[derive(Clone)]
pub(crate) struct Opener(Arc<dyn Fn(&str, bool) -> bool + Send + Sync>);

impl Opener {
    pub fn new(open: impl Fn(&str, bool) -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(open))
    }

    /// Abre `url`; `authorization` diz que é a página de autorização do Lichess.
    pub fn open(&self, url: &str, authorization: bool) -> bool {
        (self.0)(url, authorization)
    }

    #[cfg(test)]
    pub fn recording(log: Arc<Mutex<Vec<String>>>, succeeds: bool) -> Self {
        Self(Arc::new(move |url, _| {
            log.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(url.to_owned());
            succeeds
        }))
    }
}

impl Default for Opener {
    fn default() -> Self {
        Self(Arc::new(|url, authorization| {
            if authorization {
                browser::open_authorization(url)
            } else {
                browser::open(url)
            }
        }))
    }
}

impl std::fmt::Debug for Opener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Opener")
    }
}

// --- Mensagens ------------------------------------------------------------------------------

/// A mensagem, em linguagem de usuário, de um erro de autorização. Nunca mostra o texto técnico
/// do erro: ele é para quem desenvolve.
pub(crate) fn problem_of(error: &AuthError, slot: Slot) -> Ending {
    let plain = |key| Ending::Problem(Note::plain(key));
    match error {
        AuthError::Disabled => plain("auth-problem-off"),
        AuthError::Vault(VaultError::Locked) => plain("auth-problem-locked"),
        AuthError::Vault(VaultError::Unavailable) => plain("auth-problem-no-vault"),
        AuthError::Vault(_) => plain("auth-problem-vault"),
        AuthError::Local { .. } => plain("auth-problem-local"),
        AuthError::StateMismatch | AuthError::MalformedCallback => plain("auth-problem-callback"),
        AuthError::CodeRejected => plain("auth-problem-code"),
        AuthError::InvalidToken => plain("auth-problem-key-format"),
        AuthError::TokenRejected => plain("auth-problem-key-rejected"),
        AuthError::WrongScopes { .. } if slot.is_engine() => plain("auth-problem-wrong-engine"),
        AuthError::WrongScopes { .. } => plain("auth-problem-wrong-player"),
        AuthError::ScopesUnconfirmed => plain("auth-problem-no-permissions"),
        AuthError::Malformed { .. } => plain("auth-problem-malformed"),
        AuthError::Net(error) => Ending::Problem(Note::with(
            "auth-problem-net",
            "detail",
            error_phrase(error),
        )),
        // Cancelar não é erro: quem cancelou já sabe.
        _ => plain("auth-problem-other"),
    }
}

/// Frase curta (id Fluent) para falha de rede dentro de "auth-problem-net".
fn error_phrase(error: &chess_net::NetError) -> &'static str {
    super::model::error_key(error)
}

/// Id Fluent de uma permissão do Lichess em linguagem de usuário. Permissões que o app não
/// conhece aparecem como "Outra permissão: <nome>".
pub(crate) fn permission_key(scope: &str) -> Option<&'static str> {
    Some(match scope {
        "board:play" => "perm-board-play",
        "bot:play" => "perm-bot-play",
        "challenge:read" => "perm-challenge-read",
        "challenge:write" => "perm-challenge-write",
        "preference:read" => "perm-preference-read",
        "preference:write" => "perm-preference-write",
        "email:read" => "perm-email-read",
        "study:read" => "perm-study-read",
        "study:write" => "perm-study-write",
        "puzzle:read" => "perm-puzzle-read",
        "follow:read" => "perm-follow-read",
        "follow:write" => "perm-follow-write",
        "msg:write" => "perm-msg-write",
        "team:read" => "perm-team-read",
        "team:write" => "perm-team-write",
        "tournament:write" => "perm-tournament-write",
        "racer:write" => "perm-racer-write",
        "web:mod" => "perm-web-mod",
        _ => return None,
    })
}

/// A autorização permite mais do que o Chess Studio precisa para o papel.
pub(crate) fn has_extra_permissions(scopes: &[String], role: Role) -> bool {
    let needed = match role {
        Role::Player => "board:play",
        Role::Engine => "bot:play",
    };
    scopes.iter().any(|s| s != needed)
}

// --- Máquina de estados ----------------------------------------------------------------------

impl ConnectionsUi {
    /// Pede para autorizar `slot` pelo navegador (`renew`: renovar a Conexão expirada).
    pub(crate) fn authorize(
        &mut self,
        slot: Slot,
        renew: Option<ConnectionId>,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        self.proceed(Intent::Browser { slot, renew }, service, dialog, handle);
    }

    /// Abre o formulário da chave pessoal de `slot` (ainda sem exigir nada: digitar é livre).
    pub(crate) fn open_key_form(&mut self, slot: Slot, renew: Option<ConnectionId>) {
        self.auth.endings.remove(&slot);
        self.auth.key_forms.insert(slot, KeyForm::new(renew));
        self.expanded.insert(slot.provider);
        if slot.is_engine() {
            self.auth.engine_open.insert(slot.provider);
        }
    }

    /// Guarda a chave que está no formulário: confere, pede o Provedor e o Cofre, e segue.
    pub(crate) fn submit_key(
        &mut self,
        slot: Slot,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(form) = self.auth.key_forms.get_mut(&slot) else {
            return;
        };
        form.error = None;
        let trimmed = form.secret.trim();
        if trimmed.is_empty() {
            form.error = Some("auth-key-error-empty");
            return;
        }
        if !Secret::new(trimmed).is_token_like() {
            form.error = Some("auth-key-error-format");
            return;
        }
        let renew = form.renew;
        self.proceed(Intent::Key { slot, renew }, service, dialog, handle);
    }

    /// As portas, em ordem: Provedor ligado, Cofre pronto. Cada uma pode parar o pedido e abrir
    /// o diálogo dela; quando a pessoa responde, [`ConnectionsUi::poll_auth`] continua daqui.
    fn proceed(
        &mut self,
        intent: Intent,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        let slot = intent.slot();
        if self.auth.attempt.is_some() {
            return;
        }
        self.auth.waiting = None;
        self.auth.endings.remove(&slot);
        self.auth.last.insert(slot, intent);
        if !service.is_enabled(slot.provider) {
            dialog.ask(slot.provider, Some(self.i18n_feature()));
            if dialog.current() == Some(slot.provider) {
                self.auth.waiting = Some((intent, Gate::Consent));
            }
            return;
        }
        if let Some(problem) = self.vault_gate(intent, service) {
            if let Some(problem) = problem {
                self.auth.endings.insert(slot, problem);
            }
            return;
        }
        self.run(intent, service, handle);
    }

    /// O nome do recurso na frase do diálogo "Ligar <Provedor>?".
    fn i18n_feature(&self) -> String {
        crate::i18n::I18n::pt_br().t("auth-feature")
    }

    /// `None`: o Cofre está pronto. `Some(None)`: parou para pedir a senha-mestra.
    /// `Some(Some(fim))`: parou com um problema.
    fn vault_gate(&mut self, intent: Intent, service: &NetService) -> Option<Option<Ending>> {
        if service.vault_error().is_some() {
            return Some(Some(Ending::Problem(Note::plain(
                "auth-problem-vault-broken",
            ))));
        }
        match service.vault().status() {
            VaultStatus::System | VaultStatus::OwnUnlocked => {
                if let Preload::Failed(_) = service.preload() {
                    return Some(Some(Ending::Problem(Note::plain(
                        "auth-problem-system-vault",
                    ))));
                }
                None
            }
            VaultStatus::OwnLocked => {
                self.auth.vault_dialog = Some(VaultDialog::new(VaultMode::Unlock));
                self.auth.waiting = Some((intent, Gate::Vault));
                Some(None)
            }
            VaultStatus::Unavailable => {
                self.auth.vault_dialog = Some(VaultDialog::new(VaultMode::Create));
                self.auth.waiting = Some((intent, Gate::Vault));
                Some(None)
            }
        }
    }

    /// Reserva a Conexão e inicia o trabalho de fundo.
    fn run(&mut self, intent: Intent, service: &NetService, handle: Option<&PersistenceHandle>) {
        let slot = intent.slot();
        let kind = intent.kind();
        let renew = intent.renew();
        let Some((id, profile)) = self.reserve(slot, kind, renew, service, handle) else {
            self.auth
                .endings
                .insert(slot, Ending::Problem(Note::plain("auth-problem-save")));
            return;
        };
        let started = NetService::now();
        let work = match intent {
            Intent::Browser { .. } => {
                match service.auth().authorize(net_id(id), slot.auth_role()) {
                    Ok(flow) => Ok(Work::Browser(flow)),
                    Err(error) => Err(error),
                }
            }
            Intent::Key { .. } => {
                // A chave sai do formulário (e da memória dele) direto para o Cofre.
                let secret = self
                    .auth
                    .key_forms
                    .remove(&slot)
                    .map(|mut form| {
                        let text = std::mem::take(&mut *form.secret);
                        Secret::new(text)
                    })
                    .unwrap_or_else(|| Secret::new(String::new()));
                Ok(Work::Key(Mutex::new(service.auth().personal_token(
                    net_id(id),
                    slot.auth_role(),
                    secret,
                ))))
            }
        };
        match work {
            Ok(work) => {
                let mut attempt = Attempt {
                    slot,
                    connection: id,
                    profile,
                    kind,
                    renewing: renew.is_some(),
                    started,
                    open_failed: false,
                    work,
                };
                if let Some(url) = attempt.url() {
                    attempt.open_failed = !self.opener.open(url, true);
                }
                self.auth.attempt = Some(attempt);
            }
            Err(error) => {
                if renew.is_none() {
                    self.discard(id, service, handle);
                }
                self.auth.endings.insert(slot, problem_of(&error, slot));
            }
        }
    }

    /// Reserva (ou reaproveita, na renovação) a linha da Conexão.
    fn reserve(
        &mut self,
        slot: Slot,
        kind: ConnectionKind,
        renew: Option<ConnectionId>,
        service: &NetService,
        handle: Option<&PersistenceHandle>,
    ) -> Option<(ConnectionId, chess_store::ProfileId)> {
        if let Some(id) = renew {
            let profile = self.loaded_for?;
            return Some((id, profile));
        }
        for attempt in 0..2 {
            let result = with_store(&mut self.local, handle, |store, profile| {
                store
                    .begin_authorization(profile, slot.provider.key(), slot.role, kind)
                    .map(|id| (id, profile))
            })?;
            match result {
                Ok(found) => return Some(found),
                // Sobrou uma reserva de um app que fechou no meio: descarta (apagando o segredo
                // dela) e tenta de novo uma vez.
                Err(StoreError::ConnectionExists { .. }) if attempt == 0 => {
                    self.refresh(handle);
                    self.discard_pending(service, handle);
                }
                Err(_) => return None,
            }
        }
        None
    }

    /// Descarta uma Conexão reservada e o segredo dela (se chegou a ser guardado).
    fn discard(
        &mut self,
        id: ConnectionId,
        service: &NetService,
        handle: Option<&PersistenceHandle>,
    ) {
        let vault = service.vault().clone();
        let _ = with_store(&mut self.local, handle, |store, profile| {
            secrets::forget_connection(store, &vault, profile, id)
        });
        self.dirty = true;
    }

    /// Descarta as reservas que ninguém está esperando (o app fechou no meio de uma
    /// autorização): apaga o segredo e a linha.
    pub(crate) fn discard_pending(
        &mut self,
        service: &NetService,
        handle: Option<&PersistenceHandle>,
    ) {
        if self.auth.attempt.is_some() {
            return;
        }
        let pending: Vec<ConnectionId> = self
            .list
            .iter()
            .filter(|c| c.is_pending())
            .map(|c| c.id)
            .collect();
        for id in pending {
            self.discard(id, service, handle);
        }
    }

    /// Cancela a autorização em andamento: a porta local fecha e nada é guardado.
    pub(crate) fn cancel_authorization(
        &mut self,
        service: &NetService,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(attempt) = self.auth.attempt.take() else {
            // Cancelar enquanto espera uma porta.
            self.auth.waiting = None;
            return;
        };
        attempt.cancel();
        if !attempt.renewing {
            self.discard(attempt.connection, service, handle);
        }
        self.auth.endings.remove(&attempt.slot);
    }

    /// Abre de novo a página de autorização (a pessoa fechou a aba).
    pub(crate) fn reopen_browser(&mut self) {
        let opener = self.opener.clone();
        if let Some(attempt) = self.auth.attempt.as_mut()
            && let Some(url) = attempt.url()
        {
            attempt.open_failed = !opener.open(url, true);
        }
    }

    /// Avança o que está em andamento (a cada quadro): respostas aos diálogos, o navegador, a
    /// chave pessoal, as conferências e as desconexões. Nunca bloqueia.
    pub(crate) fn poll_auth(
        &mut self,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
        answers: &[ProviderConsentAnswered],
    ) {
        self.poll_consent(service, dialog, handle, answers);
        self.poll_vault_dialog(service, dialog, handle);
        self.poll_attempt(service, handle);
        self.poll_checks();
        self.poll_disconnects(service, handle);
    }

    fn poll_consent(
        &mut self,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
        answers: &[ProviderConsentAnswered],
    ) {
        let Some((intent, Gate::Consent)) = self.auth.waiting else {
            return;
        };
        let provider = intent.slot().provider;
        let Some(answer) = answers.iter().find(|a| a.provider == provider) else {
            return;
        };
        self.auth.waiting = None;
        if answer.accepted {
            self.proceed(intent, service, dialog, handle);
        } else {
            self.auth.endings.insert(intent.slot(), Ending::Declined);
        }
    }

    fn poll_attempt(&mut self, service: &NetService, handle: Option<&PersistenceHandle>) {
        let Some(attempt) = self.auth.attempt.as_ref() else {
            return;
        };
        let finished: Option<Result<LichessAccount, Ending>> = match &attempt.work {
            Work::Browser(flow) => match flow.state() {
                AuthState::Waiting => None,
                AuthState::Authorized(account) => Some(Ok(account)),
                AuthState::Refused => Some(Err(Ending::Refused)),
                AuthState::Expired => Some(Err(Ending::TimedOut)),
                AuthState::Failed(AuthError::Cancelled) => {
                    // Cancelado pela própria pessoa: sem mensagem.
                    Some(Err(Ending::Declined))
                }
                AuthState::Failed(error) => Some(Err(problem_of(&error, attempt.slot))),
            },
            Work::Key(task) => task
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .try_take()
                .map(|result| result.map_err(|error| problem_of(&error, attempt.slot))),
        };
        let Some(finished) = finished else {
            return;
        };
        let Some(attempt) = self.auth.attempt.take() else {
            return;
        };
        match finished {
            Ok(account) => self.complete(attempt, account, service, handle),
            Err(ending) => {
                if !attempt.renewing {
                    self.discard(attempt.connection, service, handle);
                }
                // Cancelar: não mostra nada (a pessoa sabe o que fez).
                if ending == Ending::Declined {
                    self.auth.endings.remove(&attempt.slot);
                } else {
                    self.auth.endings.insert(attempt.slot, ending);
                }
            }
        }
    }

    /// O Lichess confirmou: grava o nome, o que foi liberado e o instante.
    fn complete(
        &mut self,
        attempt: Attempt,
        account: LichessAccount,
        service: &NetService,
        handle: Option<&PersistenceHandle>,
    ) {
        let LichessAccount {
            username, scopes, ..
        } = account;
        let at = NetService::now().as_millis();
        let (id, kind, profile) = (attempt.connection, attempt.kind, attempt.profile);
        let recorded = with_store(&mut self.local, handle, |store, current| {
            if current != profile {
                // A pessoa trocou de Perfil local no meio: esta autorização não é do atual.
                return Err(StoreError::WrongProfile { profile });
            }
            store.record_authorization(profile, id, kind, &username, &scopes, at)
        });
        match recorded {
            Some(Ok(())) => {
                self.auth.endings.insert(
                    attempt.slot,
                    Ending::Authorized {
                        name: username,
                        scopes,
                        kind,
                    },
                );
                self.notes.remove(&attempt.slot.provider);
                self.dirty = true;
            }
            other => {
                if !attempt.renewing {
                    self.discard(id, service, handle);
                }
                let key = match other {
                    Some(Err(StoreError::ConnectionExists { .. })) => "auth-problem-duplicate",
                    _ => "auth-problem-save",
                };
                self.auth
                    .endings
                    .insert(attempt.slot, Ending::Problem(Note::plain(key)));
            }
        }
    }

    // --- Cofre -------------------------------------------------------------------------------

    /// Abre o diálogo da senha-mestra no modo pedido (a faixa do Cofre).
    pub(crate) fn open_vault_dialog(&mut self, mode: VaultMode) {
        self.auth.vault_dialog = Some(VaultDialog::new(mode));
    }

    /// Fecha o diálogo sem fazer nada; o pedido que esperava o Cofre é abandonado.
    pub(crate) fn close_vault_dialog(&mut self) {
        self.auth.vault_dialog = None;
        if matches!(self.auth.waiting, Some((_, Gate::Vault))) {
            self.auth.waiting = None;
        }
    }

    /// Bloqueia o cofre próprio: a chave e os segredos saem da memória. As autorizações deixam
    /// de valer na hora (o Explorador para de pedir).
    pub(crate) fn lock_vault(&mut self, service: &NetService, handle: Option<&PersistenceHandle>) {
        service.vault().lock();
        self.sync(service, handle);
    }

    /// Valida os campos e inicia a operação do Cofre em fundo (o Argon2id é lento de propósito).
    pub(crate) fn submit_vault_dialog(&mut self, service: &NetService) {
        let Some(dialog) = self.auth.vault_dialog.as_mut() else {
            return;
        };
        if dialog.busy.is_some() {
            return;
        }
        dialog.error = None;
        let mode = dialog.mode;
        let enough = |text: &str| text.chars().count() >= chess_vault::MIN_PASSWORD_CHARS;
        match mode {
            VaultMode::Unlock if dialog.password.is_empty() => {
                dialog.error = Some("vault-error-empty");
                return;
            }
            VaultMode::Create | VaultMode::Change => {
                if mode == VaultMode::Change && dialog.current.is_empty() {
                    dialog.error = Some("vault-error-empty-current");
                    return;
                }
                if !enough(&dialog.password) {
                    dialog.error = Some("vault-error-weak");
                    return;
                }
                if *dialog.password != *dialog.confirm {
                    dialog.error = Some("vault-error-mismatch");
                    return;
                }
                if mode == VaultMode::Create && !dialog.understood {
                    dialog.error = Some("vault-error-understand");
                    return;
                }
            }
            VaultMode::Unlock => {}
        }
        // As senhas viram `Secret` (zerado ao descartar) e saem dos campos agora.
        let current = Secret::new(std::mem::take(&mut *dialog.current));
        let password = Secret::new(std::mem::take(&mut *dialog.password));
        dialog.confirm.zeroize();
        let vault = service.vault().clone();
        dialog.busy = Some(Job::spawn(
            "chess-studio-senha-mestra",
            move || match mode {
                VaultMode::Create => vault.create_own(&password),
                VaultMode::Unlock => vault.unlock(&password),
                VaultMode::Change => vault.change_password(&current, &password),
            },
        ));
    }

    fn poll_vault_dialog(
        &mut self,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(vault_dialog) = self.auth.vault_dialog.as_mut() else {
            return;
        };
        let Some(job) = vault_dialog.busy.as_mut() else {
            return;
        };
        let Some(outcome) = job.poll() else {
            // Sem resposta: a thread morreu (sem recursos do sistema)?
            return;
        };
        vault_dialog.busy = None;
        match outcome {
            Ok(()) => {
                self.auth.vault_dialog = None;
                self.sync(service, handle);
                if let Some((intent, Gate::Vault)) = self.auth.waiting {
                    self.auth.waiting = None;
                    self.proceed(intent, service, dialog, handle);
                }
            }
            Err(error) => vault_dialog.error = Some(vault_error_key(&error)),
        }
    }

    // --- Conferir e desconectar --------------------------------------------------------------

    /// "Testar" uma Conexão autorizada: pergunta ao Lichess quem ela é.
    pub(crate) fn check_authorized(&mut self, id: ConnectionId, service: &NetService) {
        if matches!(self.auth.checks.get(&id), Some(Check::Running(_))) {
            return;
        }
        let task = service.auth().check(net_id(id));
        self.auth
            .checks
            .insert(id, Check::Running(Mutex::new(task)));
    }

    fn poll_checks(&mut self) {
        for check in self.auth.checks.values_mut() {
            let Check::Running(task) = check else {
                continue;
            };
            let done = task
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner)
                .try_take();
            if let Some(result) = done {
                *check = Check::Done {
                    result: result
                        .map(|account| account.username)
                        .map_err(|error| match error {
                            AuthError::TokenRejected | AuthError::Vault(_) => "auth-check-rejected",
                            AuthError::Net(ref e) => super::model::error_key(e),
                            _ => "auth-check-other",
                        }),
                    at: NetService::now(),
                };
            }
        }
    }

    /// Desconecta: pede ao Lichess para revogar, apaga o segredo daqui (sempre) e remove a
    /// Conexão. Quando a revogação no Lichess falha, a tela avisa onde revogar à mão.
    pub(crate) fn disconnect(&mut self, id: ConnectionId, service: &NetService) {
        self.auth.confirm_disconnect = None;
        if self.auth.revoking.contains_key(&id) {
            return;
        }
        let task = service.auth().revoke(net_id(id));
        self.auth.revoking.insert(id, Mutex::new(task));
    }

    fn poll_disconnects(&mut self, service: &NetService, handle: Option<&PersistenceHandle>) {
        let ids: Vec<ConnectionId> = self.auth.revoking.keys().copied().collect();
        for id in ids {
            let done = self
                .auth
                .revoking
                .get_mut(&id)
                .and_then(|task| task.get_mut().ok().and_then(AuthTask::try_take));
            let Some(revocation) = done else {
                continue;
            };
            self.auth.revoking.remove(&id);
            let provider = self
                .list
                .iter()
                .find(|c| c.id == id)
                .and_then(|c| ProviderId::from_key(&c.provider))
                .unwrap_or(ProviderId::Lichess);
            let vault = service.vault().clone();
            let removed = with_store(&mut self.local, handle, |store, profile| {
                secrets::forget_connection(store, &vault, profile, id)
            });
            let note = match (&revocation.local, removed) {
                (Err(_), _) | (_, Some(Err(ForgetError::Vault(_)))) => {
                    Note::plain("connections-revoke-local-failed")
                }
                (Ok(()), Some(Ok(()))) if revocation.remote.is_ok() => {
                    Note::plain("connections-revoke-ok")
                }
                (Ok(()), Some(Ok(()))) => Note::plain("connections-revoke-remote-failed"),
                _ => Note::plain("connections-remove-error"),
            };
            self.notes.insert(provider, note);
            self.auth.checks.remove(&id);
            self.dirty = true;
        }
    }

    // --- Estado derivado -----------------------------------------------------------------------

    /// Recarrega as Conexões, descarta reservas esquecidas e mantém em dia o que o resto do app
    /// consulta: a Conexão autorizada de cada entrada em [`NetService`]. Chame a cada quadro
    /// (é barato: só refaz o trabalho quando algo mudou).
    pub(crate) fn sync(&mut self, service: &NetService, handle: Option<&PersistenceHandle>) {
        let before = self.loaded_for;
        self.refresh(handle);
        let Some(profile) = self.loaded_for else {
            return;
        };
        if before.is_some() && before != self.loaded_for {
            self.reset_for_profile();
        }
        // Trocou de Perfil local no meio de uma autorização: ela não é do Perfil atual.
        if self
            .auth
            .attempt
            .as_ref()
            .is_some_and(|a| a.profile != profile)
            && let Some(attempt) = self.auth.attempt.take()
        {
            attempt.cancel();
            if !attempt.renewing {
                self.discard(attempt.connection, service, handle);
            }
        }
        self.discard_pending(service, handle);

        let vault = service.vault();
        let loaded = service.vault_loaded();
        let signature = Signature {
            profile,
            status: vault.status(),
            loaded,
            list: self
                .list
                .iter()
                .filter(|c| c.kind.is_authorized())
                .map(|c| (c.id, c.authorized_at_ms, c.is_default))
                .collect(),
            rejected: vault
                .references()
                .iter()
                .map(|r| (r.connection.0, r.rejected.is_some()))
                .collect(),
        };
        if self.auth.signature.as_ref() == Some(&signature) {
            return;
        }
        self.auth.signature = Some(signature);
        self.auth.standings = self
            .list
            .iter()
            .filter(|c| c.is_authorized())
            .map(|c| (c.id, secrets::standing(c, vault, loaded)))
            .collect();
        for role in [Role::Player, Role::Engine] {
            let best = self.active_authorization(ProviderId::Lichess, role);
            service.set_authorized_connection(ProviderId::Lichess, role, best.map(net_id));
        }
        self.authorized = ProviderId::ALL
            .into_iter()
            .filter(|id| self.provider_standing(*id) == Some(Standing::Active))
            .collect();
    }

    fn reset_for_profile(&mut self) {
        if let Some(attempt) = self.auth.attempt.take() {
            attempt.cancel();
        }
        self.auth.endings.clear();
        self.auth.key_forms.clear();
        self.auth.waiting = None;
        self.auth.last.clear();
        self.auth.checks.clear();
        self.auth.confirm_disconnect = None;
        self.auth.signature = None;
    }

    /// A Conexão autorizada e **em uso** (segredo no Cofre aberto, não recusada) da entrada;
    /// a padrão primeiro.
    pub(crate) fn active_authorization(
        &self,
        provider: ProviderId,
        role: Role,
    ) -> Option<ConnectionId> {
        let mut found: Vec<&Connection> = self
            .list
            .iter()
            .filter(|c| c.provider == provider.key() && c.role == role && c.is_authorized())
            .filter(|c| self.auth.standings.get(&c.id) == Some(&Standing::Active))
            .collect();
        found.sort_by_key(|c| !c.is_default);
        found.first().map(|c| c.id)
    }

    /// A melhor situação entre as Conexões autorizadas de jogador do Provedor (para o selo do
    /// cartão): em uso, depois bloqueada, depois recusada. `None`: nenhuma autorizada.
    pub(crate) fn provider_standing(&self, provider: ProviderId) -> Option<Standing> {
        self.list
            .iter()
            .filter(|c| c.provider == provider.key() && c.role == Role::Player)
            .filter(|c| c.is_authorized())
            .filter_map(|c| self.auth.standings.get(&c.id).copied())
            .min_by_key(|s| match s {
                Standing::Active => 0,
                Standing::Locked => 1,
                Standing::Expired => 2,
                Standing::Unusable => 3,
            })
    }
}

/// Id Fluent da mensagem de um erro do Cofre no diálogo da senha-mestra.
pub(crate) fn vault_error_key(error: &VaultError) -> &'static str {
    match error {
        VaultError::WrongPassword => "vault-error-wrong",
        VaultError::WeakPassword { .. } => "vault-error-weak",
        VaultError::Corrupted { .. } | VaultError::UnsupportedFormat { .. } => {
            "vault-error-corrupted"
        }
        VaultError::CannotCreateOwn | VaultError::NoOwnVault => "vault-error-state",
        _ => "vault-error-other",
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
