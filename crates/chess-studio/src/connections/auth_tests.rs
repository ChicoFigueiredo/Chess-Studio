//! A máquina de estados da autorização, vista pela interface (`ConnectionsUi::apply` e
//! `drive`, como a tela faz): navegador, chave pessoal, recusa, prazo, cancelar, Cofre sem
//! senha-mestra, desbloqueio, renovação, desconexão e o ciclo de vida do segredo. Servidor
//! falso, relógio falso e **cofre falso**: nada toca o Lichess nem o cofre real do usuário, e o
//! navegador é um gravador (nenhum teste abre navegador).

use super::*;
use crate::connections::{ConnectionsUi, Op};
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::secrets::{VolatileKeyring, net_id, volatile_vault_with};
use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{
    AuthorizationSource, ConnectionId as NetId, FakeClock, Net, NetConfig, RetryPolicy,
};
use chess_store::{ConnectionKind, NewConnection};
use chess_vault::KdfParams;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const TOKEN: &str = "lio_TOKEN_DE_TESTE_9f8e7d6c";
const KEY: &str = "lip_CHAVE_PESSOAL_DE_TESTE_1a2b3c";
const CODE: &str = "codigo_de_autorizacao_123";
const PASSWORD: &str = "uma senha-mestra de teste";
const FAST: KdfParams = KdfParams {
    memory_kib: 8,
    iterations: 1,
    parallelism: 1,
};
const LICHESS: ProviderId = ProviderId::Lichess;

struct Env {
    clock: Arc<FakeClock>,
    server: FakeServer,
    service: NetService,
    ui: ConnectionsUi,
    dialog: ConsentDialog,
    keyring: Arc<VolatileKeyring>,
    opened: Arc<Mutex<Vec<String>>>,
    answers: Vec<ProviderConsentAnswered>,
}

fn env() -> Env {
    env_with(VolatileKeyring::new(), true)
}

fn env_with(keyring: VolatileKeyring, lichess_on: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let keyring = Arc::new(keyring);
    let vault = volatile_vault_with(keyring.clone(), "teste-autorizacao", Some(FAST));
    let mut config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .retry(RetryPolicy {
            max_retries: 0,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
        })
        .vault(Arc::new(vault.vault.clone()));
    for host in ["lichess.org", "explorer.lichess.org", "api.chess.com"] {
        config = config.route(host, &server.base_url());
    }
    let net = Net::open(config).expect("abre a rede");
    let service = NetService::with_vault(net, clock.clone(), false, vault);
    if lichess_on {
        service.set_enabled(LICHESS, true).unwrap();
    }
    let opened = Arc::new(Mutex::new(Vec::new()));
    let ui = ConnectionsUi {
        opener: Opener::recording(opened.clone(), true),
        ..ConnectionsUi::default()
    };
    Env {
        clock,
        server,
        service,
        ui,
        dialog: ConsentDialog::default(),
        keyring,
        opened,
        answers: Vec::new(),
    }
}

fn token_response(token: &str) -> FakeResponse {
    FakeResponse::json(&format!(
        "{{\"token_type\":\"Bearer\",\"access_token\":\"{token}\",\"expires_in\":31536000}}"
    ))
}

fn account_response(scopes: &str) -> FakeResponse {
    FakeResponse::json("{\"id\":\"georges\",\"username\":\"Georges\"}")
        .header("x-oauth-scopes", scopes)
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).expect("hex"));
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8(out).expect("utf-8")
}

fn query_param(url: &str, key: &str) -> String {
    let query = url.split_once('?').expect("consulta").1;
    query
        .split('&')
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| decode(v))
        })
        .unwrap_or_else(|| panic!("sem {key}"))
}

/// O "navegador" volta ao app: `GET <redirect_uri>?<query>`.
fn come_back(redirect_uri: &str, query: &str) -> std::io::Result<String> {
    let rest = redirect_uri
        .strip_prefix("http://127.0.0.1:")
        .expect("retorno em 127.0.0.1");
    let (port, path) = rest.split_at(rest.find('/').expect("caminho"));
    let mut stream = TcpStream::connect(("127.0.0.1", port.parse::<u16>().expect("porta")))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "GET {path}?{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

impl Env {
    fn op(&mut self, op: Op) {
        self.ui
            .apply(vec![op], &self.service, &mut self.dialog, None);
    }

    fn frame(&mut self) {
        let answers = std::mem::take(&mut self.answers);
        self.ui
            .drive(&self.service, &mut self.dialog, None, &answers);
    }

    /// Quadros (sem bloquear) até `done`, com prazo de 10 s de relógio real.
    fn until(&mut self, what: &str, done: impl Fn(&ConnectionsUi) -> bool) {
        let start = Instant::now();
        loop {
            self.frame();
            if done(&self.ui) {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "não aconteceu: {what}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn authorize_player(&mut self) {
        self.op(Op::Authorize(Slot::player(LICHESS)));
        self.frame();
    }

    /// O endereço que o app mandou abrir no navegador (o último).
    fn browser_url(&self) -> String {
        self.opened
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("o navegador foi aberto")
    }

    /// O navegador volta com um código válido para o `state` da autorização.
    fn approve(&self) {
        let url = self.browser_url();
        let state = query_param(&url, "state");
        let redirect = query_param(&url, "redirect_uri");
        come_back(&redirect, &format!("code={CODE}&state={state}")).expect("retorno");
    }

    fn ending(&self, slot: Slot) -> Option<&Ending> {
        self.ui.auth.endings.get(&slot)
    }

    fn lichess_list(&self) -> Vec<&Connection> {
        self.ui
            .list
            .iter()
            .filter(|c| c.provider == "lichess")
            .collect()
    }

    fn authorized_id(&self, role: Role) -> Option<chess_net::ConnectionId> {
        self.service.authorized_connection(LICHESS, role)
    }

    /// Faz uma autorização completa pelo navegador (para os testes que partem dela).
    fn authorized(&mut self) -> ConnectionId {
        self.server.route("/api/token", token_response(TOKEN));
        self.server
            .route("/api/account", account_response("board:play"));
        self.authorize_player();
        self.approve();
        let slot = Slot::player(LICHESS);
        self.until("autorizado", |ui| {
            matches!(ui.auth.endings.get(&slot), Some(Ending::Authorized { .. }))
        });
        self.lichess_list()
            .iter()
            .find(|c| c.is_authorized())
            .expect("Conexão autorizada")
            .id
    }
}

fn player() -> Slot {
    Slot::player(LICHESS)
}

#[test]
fn authorizing_in_the_browser_end_to_end_stores_the_secret_only_in_the_vault() {
    let mut env = env();
    env.server.route("/api/token", token_response(TOKEN));
    env.server
        .route("/api/account", account_response("board:play"));
    env.authorize_player();

    // Espera: o navegador abriu na página esperada e a tela vê o estado de espera.
    let url = env.browser_url();
    assert!(crate::connections::browser::is_lichess_authorization(&url));
    let attempt = env.ui.auth.attempt.as_ref().expect("esperando");
    assert!(attempt.is_browser() && !attempt.open_failed);
    assert_eq!(attempt.minutes_left(NetService::now()), 10);
    assert!(env.ui.auth.is_busy(LICHESS));
    assert_eq!(
        env.authorized_id(Role::Player),
        None,
        "ainda não autorizado"
    );

    env.approve();
    env.until("autorizado", |ui| {
        matches!(
            ui.auth.endings.get(&player()),
            Some(Ending::Authorized { .. })
        )
    });

    let Some(Ending::Authorized { name, scopes, kind }) = env.ending(player()).cloned() else {
        panic!("autorizado");
    };
    assert_eq!(name, "Georges");
    assert_eq!(scopes, ["board:play"]);
    assert_eq!(kind, ConnectionKind::AuthorizedBrowser);
    assert!(env.ui.auth.attempt.is_none());

    // O estado "Autorizado" é de verdade: banco + Cofre.
    let list = env.lichess_list();
    assert_eq!(list.len(), 1);
    let connection = list[0];
    assert!(connection.is_authorized() && connection.is_default);
    assert_eq!(connection.identifier, "Georges");
    assert_eq!(connection.scopes, ["board:play"]);
    assert!(connection.authorized_at_ms.is_some());
    assert_eq!(
        env.ui.auth.standings.get(&connection.id),
        Some(&Standing::Active)
    );
    assert!(env.ui.authorized.contains(&LICHESS));
    assert_eq!(
        env.authorized_id(Role::Player),
        Some(net_id(connection.id)),
        "o Explorador passa a ver a Conexão"
    );
    assert_eq!(env.authorized_id(Role::Engine), None);
    assert!(
        env.service
            .vault()
            .reference(net_id(connection.id))
            .is_some(),
        "o segredo está no Cofre"
    );
    assert_eq!(env.keyring.accounts().len(), 1);
    // A conferência em /api/account foi feita com o token em conferência.
    let account = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/account")
        .expect("conferiu a conta");
    assert_eq!(
        account.header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
}

#[test]
fn refusing_in_the_browser_stores_nothing_and_offers_to_try_again() {
    let mut env = env();
    env.authorize_player();
    let url = env.browser_url();
    let state = query_param(&url, "state");
    let redirect = query_param(&url, "redirect_uri");
    come_back(&redirect, &format!("error=access_denied&state={state}")).expect("retorno");
    env.until("recusado", |ui| {
        ui.auth.endings.get(&player()) == Some(&Ending::Refused)
    });
    assert!(env.ui.auth.attempt.is_none());
    assert!(env.lichess_list().is_empty(), "a reserva foi descartada");
    assert!(env.keyring.accounts().is_empty());
    assert_eq!(env.authorized_id(Role::Player), None);
    assert!(
        !env.server.requests().iter().any(|r| r.path == "/api/token"),
        "sem código, nada foi trocado"
    );
    // "Tentar de novo" abre o navegador outra vez.
    env.op(Op::Retry(player()));
    assert_eq!(env.opened.lock().unwrap().len(), 2);
    assert!(env.ui.auth.attempt.is_some());
}

#[test]
fn a_forged_return_is_refused_and_nothing_is_stored() {
    let mut env = env();
    env.authorize_player();
    let redirect = query_param(&env.browser_url(), "redirect_uri");
    come_back(&redirect, &format!("code={CODE}&state=outro-state")).expect("retorno");
    env.until("falhou", |ui| {
        matches!(ui.auth.endings.get(&player()), Some(Ending::Problem(_)))
    });
    let Some(Ending::Problem(note)) = env.ending(player()) else {
        panic!("problema");
    };
    assert_eq!(note.key, "auth-problem-callback");
    assert!(env.lichess_list().is_empty());
    assert!(env.keyring.accounts().is_empty());
}

#[test]
fn the_deadline_ends_the_wait_and_discards_the_reservation() {
    let mut env = env();
    env.authorize_player();
    assert_eq!(env.lichess_list().len(), 1, "reservada enquanto espera");
    assert!(env.lichess_list()[0].is_pending());
    // Antes de usar a Conexão reservada, ela nunca aparece para a pessoa nem vale como
    // autorizada.
    assert_eq!(env.authorized_id(Role::Player), None);
    env.clock.advance(Duration::from_secs(11 * 60));
    env.until("prazo", |ui| {
        ui.auth.endings.get(&player()) == Some(&Ending::TimedOut)
    });
    assert!(env.lichess_list().is_empty());
    assert!(env.ui.auth.attempt.is_none());
    assert!(env.keyring.accounts().is_empty());
}

#[test]
fn cancelling_closes_the_local_door_and_stores_nothing() {
    let mut env = env();
    env.authorize_player();
    let redirect = query_param(&env.browser_url(), "redirect_uri");
    env.op(Op::CancelAuthorization);
    assert!(env.ui.auth.attempt.is_none());
    assert!(env.ui.auth.endings.is_empty(), "cancelar não é erro");
    assert!(env.lichess_list().is_empty());
    // A porta local fecha: o retorno que chegasse tarde não é aceito por ninguém.
    let start = Instant::now();
    while come_back(&redirect, "code=x&state=y").is_ok() && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(come_back(&redirect, "code=x&state=y").is_err());
    assert!(env.keyring.accounts().is_empty());
}

#[test]
fn a_valid_personal_key_is_checked_then_stored_and_never_shown_again() {
    let mut env = env();
    env.server
        .route("/api/account", account_response("board:play"));
    env.op(Op::OpenKeyForm {
        slot: player(),
        renew: None,
    });
    env.ui.auth.key_forms.get_mut(&player()).unwrap().secret = Zeroizing::new(KEY.to_owned());
    // O formulário nunca vaza em Debug.
    assert!(!format!("{:?}", env.ui).contains(KEY));
    env.op(Op::SubmitKey(player()));
    assert!(
        env.ui.auth.key_forms.is_empty(),
        "a chave saiu do formulário"
    );
    let attempt = env.ui.auth.attempt.as_ref().expect("conferindo");
    assert!(!attempt.is_browser());
    env.until("autorizado", |ui| {
        matches!(
            ui.auth.endings.get(&player()),
            Some(Ending::Authorized { .. })
        )
    });
    let connection = env.lichess_list()[0].clone();
    assert_eq!(connection.kind, ConnectionKind::AuthorizedKey);
    assert_eq!(connection.identifier, "Georges");
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(connection.id)));
    assert!(env.opened.lock().unwrap().is_empty(), "sem navegador");
    assert!(!format!("{:?}", env.ui).contains(KEY));
    let account = &env.server.requests()[0];
    assert_eq!(
        account.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
}

#[test]
fn a_key_of_the_wrong_role_is_refused_with_an_explanation_in_both_directions() {
    // Chave de Engine em Conexão de jogador.
    let mut env = env();
    env.server
        .route("/api/account", account_response("bot:play"));
    env.op(Op::OpenKeyForm {
        slot: player(),
        renew: None,
    });
    env.ui.auth.key_forms.get_mut(&player()).unwrap().secret = Zeroizing::new(KEY.to_owned());
    env.op(Op::SubmitKey(player()));
    env.until("recusada", |ui| ui.auth.endings.contains_key(&player()));
    let Some(Ending::Problem(note)) = env.ending(player()) else {
        panic!("problema");
    };
    assert_eq!(note.key, "auth-problem-wrong-player");
    assert!(env.lichess_list().is_empty(), "nada foi guardado");
    assert!(env.keyring.accounts().is_empty());
    assert_eq!(env.authorized_id(Role::Player), None);

    // Chave de jogador em Conexão de Engine online.
    let mut env = self::env();
    env.server
        .route("/api/account", account_response("board:play"));
    let engine = Slot::engine(LICHESS);
    env.op(Op::OpenKeyForm {
        slot: engine,
        renew: None,
    });
    env.ui.auth.key_forms.get_mut(&engine).unwrap().secret = Zeroizing::new(KEY.to_owned());
    env.op(Op::SubmitKey(engine));
    env.until("recusada", |ui| ui.auth.endings.contains_key(&engine));
    let Some(Ending::Problem(note)) = env.ending(engine) else {
        panic!("problema");
    };
    assert_eq!(note.key, "auth-problem-wrong-engine");
    assert!(env.keyring.accounts().is_empty());
    assert_eq!(env.authorized_id(Role::Engine), None);
}

#[test]
fn a_key_with_both_permissions_is_also_refused() {
    let mut env = env();
    env.server
        .route("/api/account", account_response("board:play, bot:play"));
    env.op(Op::OpenKeyForm {
        slot: player(),
        renew: None,
    });
    env.ui.auth.key_forms.get_mut(&player()).unwrap().secret = Zeroizing::new(KEY.to_owned());
    env.op(Op::SubmitKey(player()));
    env.until("recusada", |ui| ui.auth.endings.contains_key(&player()));
    assert!(matches!(
        env.ending(player()),
        Some(Ending::Problem(note)) if note.key == "auth-problem-wrong-player"
    ));
    assert!(env.keyring.accounts().is_empty());
}

#[test]
fn malformed_or_empty_keys_never_leave_the_computer() {
    let mut env = env();
    env.op(Op::OpenKeyForm {
        slot: player(),
        renew: None,
    });
    for (typed, expected) in [
        ("   ", "auth-key-error-empty"),
        ("duas palavras", "auth-key-error-format"),
        (&"x".repeat(5000), "auth-key-error-format"),
    ] {
        env.ui.auth.key_forms.get_mut(&player()).unwrap().secret = Zeroizing::new(typed.to_owned());
        env.op(Op::SubmitKey(player()));
        assert_eq!(
            env.ui.auth.key_forms[&player()].error,
            Some(expected),
            "{typed:?}"
        );
        assert!(env.ui.auth.attempt.is_none());
    }
    assert!(env.server.requests().is_empty(), "nada saiu para a rede");
    assert!(env.lichess_list().is_empty());
}

#[test]
fn a_key_the_provider_rejects_is_not_stored() {
    let mut env = env();
    env.server.route("/api/account", FakeResponse::status(401));
    env.op(Op::OpenKeyForm {
        slot: player(),
        renew: None,
    });
    env.ui.auth.key_forms.get_mut(&player()).unwrap().secret = Zeroizing::new(KEY.to_owned());
    env.op(Op::SubmitKey(player()));
    env.until("recusada", |ui| ui.auth.endings.contains_key(&player()));
    assert!(matches!(
        env.ending(player()),
        Some(Ending::Problem(note)) if note.key == "auth-problem-key-rejected"
    ));
    assert!(env.keyring.accounts().is_empty());
    assert!(env.lichess_list().is_empty());
}

#[test]
fn without_a_system_vault_nothing_is_stored_until_a_master_password_is_created() {
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.server.route("/api/token", token_response(TOKEN));
    env.server
        .route("/api/account", account_response("board:play"));
    assert_eq!(env.service.vault().status(), VaultStatus::Unavailable);

    // Autorizar sem cofre: a Central explica (abre a criação da senha-mestra) e nada sai.
    env.op(Op::Authorize(player()));
    let dialog = env.ui.auth.vault_dialog.as_ref().expect("diálogo de criar");
    assert_eq!(dialog.mode, VaultMode::Create);
    assert!(env.ui.auth.attempt.is_none());
    assert!(env.server.requests().is_empty());
    assert!(env.opened.lock().unwrap().is_empty());
    assert!(env.lichess_list().is_empty(), "nenhuma reserva sem cofre");
    assert!(env.keyring.accounts().is_empty());

    // Validações do diálogo, uma a uma; nenhuma chega ao Cofre.
    let fill = |env: &mut Env, password: &str, confirm: &str, understood: bool| {
        let dialog = env.ui.auth.vault_dialog.as_mut().unwrap();
        dialog.password = Zeroizing::new(password.to_owned());
        dialog.confirm = Zeroizing::new(confirm.to_owned());
        dialog.understood = understood;
        env.op(Op::SubmitVaultDialog);
        env.ui.auth.vault_dialog.as_ref().unwrap().error
    };
    assert_eq!(
        fill(&mut env, "curta", "curta", true),
        Some("vault-error-weak")
    );
    assert_eq!(
        fill(&mut env, PASSWORD, "outra senha qualquer", true),
        Some("vault-error-mismatch")
    );
    assert_eq!(
        fill(&mut env, PASSWORD, PASSWORD, false),
        Some("vault-error-understand")
    );
    assert_eq!(env.service.vault().status(), VaultStatus::Unavailable);

    // Senha válida e entendimento marcado: o Argon2id roda em fundo (a tela segue viva) e, ao
    // terminar, a autorização continua sozinha.
    assert_eq!(fill(&mut env, PASSWORD, PASSWORD, true), None);
    assert!(
        env.ui.auth.vault_dialog.as_ref().unwrap().is_busy(),
        "trabalhando em fundo"
    );
    assert!(
        env.ui
            .auth
            .vault_dialog
            .as_ref()
            .unwrap()
            .password
            .is_empty(),
        "a senha saiu dos campos"
    );
    env.until("cofre criado e autorização iniciada", |ui| {
        ui.auth.attempt.is_some()
    });
    assert!(env.ui.auth.vault_dialog.is_none());
    assert_eq!(env.service.vault().status(), VaultStatus::OwnUnlocked);
    assert_eq!(env.opened.lock().unwrap().len(), 1, "o navegador abriu");
    env.approve();
    env.until("autorizado", |ui| {
        matches!(
            ui.auth.endings.get(&player()),
            Some(Ending::Authorized { .. })
        )
    });
    // Nada ficou no cofre do sistema (não há), e o token só existe cifrado no cofre próprio.
    assert!(env.keyring.accounts().is_empty());
    assert_eq!(
        env.authorized_id(Role::Player).map(|id| id.0 > 0),
        Some(true)
    );
}

#[test]
fn a_locked_own_vault_asks_for_the_password_and_the_wrong_one_is_refused() {
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.service
        .vault()
        .create_own(&Secret::new(PASSWORD))
        .expect("cria");
    env.service.vault().lock();
    assert_eq!(env.service.vault().status(), VaultStatus::OwnLocked);
    env.server.route("/api/token", token_response(TOKEN));
    env.server
        .route("/api/account", account_response("board:play"));

    env.op(Op::Authorize(player()));
    assert_eq!(
        env.ui.auth.vault_dialog.as_ref().map(|d| d.mode),
        Some(VaultMode::Unlock)
    );
    assert!(env.ui.auth.attempt.is_none());

    let try_password = |env: &mut Env, password: &str| {
        env.ui.auth.vault_dialog.as_mut().unwrap().password = Zeroizing::new(password.to_owned());
        env.op(Op::SubmitVaultDialog);
        env.until("resposta do cofre", |ui| {
            ui.auth.vault_dialog.as_ref().is_none_or(|d| !d.is_busy())
        });
    };
    try_password(&mut env, "senha errada, com certeza");
    assert_eq!(
        env.ui.auth.vault_dialog.as_ref().unwrap().error,
        Some("vault-error-wrong")
    );
    assert_eq!(env.service.vault().status(), VaultStatus::OwnLocked);
    assert!(env.ui.auth.attempt.is_none());

    try_password(&mut env, PASSWORD);
    assert!(env.ui.auth.vault_dialog.is_none());
    assert!(env.ui.auth.attempt.is_some(), "a autorização continuou");
}

#[test]
fn closing_the_vault_dialog_abandons_the_request_that_waited_for_it() {
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.op(Op::Authorize(player()));
    assert!(env.ui.auth.vault_dialog.is_some() && env.ui.auth.waiting.is_some());
    env.op(Op::CloseVaultDialog);
    assert!(env.ui.auth.vault_dialog.is_none());
    assert!(env.ui.auth.waiting.is_none() && env.ui.auth.attempt.is_none());
    assert!(env.server.requests().is_empty());
}

#[test]
fn locking_the_vault_suspends_the_authorization_and_unlocking_brings_it_back() {
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.service
        .vault()
        .create_own(&Secret::new(PASSWORD))
        .expect("cria");
    let id = env.authorized();
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(id)));
    assert!(env.ui.authorized.contains(&LICHESS));

    env.op(Op::LockVault);
    assert_eq!(env.service.vault().status(), VaultStatus::OwnLocked);
    assert_eq!(env.authorized_id(Role::Player), None, "o Explorador para");
    assert_eq!(env.ui.auth.standings.get(&id), Some(&Standing::Locked));
    assert!(!env.ui.authorized.contains(&LICHESS));
    assert_eq!(env.ui.provider_standing(LICHESS), Some(Standing::Locked));

    env.op(Op::OpenVaultDialog(VaultMode::Unlock));
    env.ui.auth.vault_dialog.as_mut().unwrap().password = Zeroizing::new(PASSWORD.to_owned());
    env.op(Op::SubmitVaultDialog);
    env.until("desbloqueado", |ui| ui.auth.vault_dialog.is_none());
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(id)));
    assert_eq!(env.ui.provider_standing(LICHESS), Some(Standing::Active));
}

#[test]
fn a_provider_that_is_off_asks_first_and_resumes_when_turned_on() {
    let mut env = env_with(VolatileKeyring::new(), false);
    env.op(Op::Authorize(player()));
    assert_eq!(env.dialog.current(), Some(LICHESS), "abre 'Ligar Lichess?'");
    assert!(env.ui.auth.attempt.is_none());
    assert!(env.server.requests().is_empty());

    // "Agora não": nada acontece e a tela explica.
    env.answers.push(ProviderConsentAnswered {
        provider: LICHESS,
        accepted: false,
    });
    env.frame();
    assert_eq!(env.ending(player()), Some(&Ending::Declined));
    assert!(env.ui.auth.attempt.is_none());

    // Liga e continua sozinho.
    env.dialog.dismiss();
    env.op(Op::DismissEnding(player()));
    env.op(Op::Authorize(player()));
    env.service.set_enabled(LICHESS, true).unwrap();
    env.answers.push(ProviderConsentAnswered {
        provider: LICHESS,
        accepted: true,
    });
    env.frame();
    assert!(env.ui.auth.attempt.is_some(), "a autorização continuou");
    assert_eq!(env.opened.lock().unwrap().len(), 1);
}

#[test]
fn a_connection_the_provider_rejects_becomes_expired_with_a_way_to_reauthorize_keeping_its_data() {
    let mut env = env();
    let id = env.authorized();
    assert_eq!(env.ui.provider_standing(LICHESS), Some(Standing::Active));

    // O Provedor recusa o segredo: a marca fica no Cofre.
    let reference = env.service.vault().reference(net_id(id)).unwrap();
    env.service
        .vault()
        .rejected(reference.profile, LICHESS, net_id(id), 401);
    env.frame();
    assert_eq!(env.ui.auth.standings.get(&id), Some(&Standing::Expired));
    assert_eq!(env.ui.provider_standing(LICHESS), Some(Standing::Expired));
    assert_eq!(env.authorized_id(Role::Player), None, "não pede mais");
    assert!(!env.ui.authorized.contains(&LICHESS));
    // Os dados locais ficam.
    let connection = env.lichess_list()[0].clone();
    assert_eq!(connection.identifier, "Georges");
    assert_eq!(connection.scopes, ["board:play"]);

    // Reautorizar renova a mesma Conexão (mesmo id), sem duplicar.
    env.op(Op::DismissEnding(player()));
    env.op(Op::Reauthorize(id));
    assert!(env.ui.auth.attempt.as_ref().is_some_and(|a| a.renewing));
    env.approve();
    env.until("renovada", |ui| {
        matches!(
            ui.auth.endings.get(&player()),
            Some(Ending::Authorized { .. })
        )
    });
    assert_eq!(env.lichess_list().len(), 1);
    assert_eq!(env.lichess_list()[0].id, id);
    assert_eq!(env.ui.auth.standings.get(&id), Some(&Standing::Active));
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(id)));
}

#[test]
fn a_failed_renewal_leaves_the_existing_connection_as_it_was() {
    let mut env = env();
    let id = env.authorized();
    env.op(Op::DismissEnding(player()));
    env.op(Op::Reauthorize(id));
    let url = env.browser_url();
    let redirect = query_param(&url, "redirect_uri");
    let state = query_param(&url, "state");
    come_back(&redirect, &format!("error=access_denied&state={state}")).expect("retorno");
    env.until("recusado", |ui| {
        ui.auth.endings.get(&player()) == Some(&Ending::Refused)
    });
    assert_eq!(env.lichess_list().len(), 1, "a Conexão continua");
    assert_eq!(env.lichess_list()[0].id, id);
    assert_eq!(env.ui.auth.standings.get(&id), Some(&Standing::Active));
}

#[test]
fn player_and_engine_authorizations_never_share_the_secret() {
    let mut env = env();
    let player_id = env.authorized();
    // Autorizar a Engine online é outra entrada, com outra permissão.
    env.server
        .route("/api/account", account_response("bot:play"));
    let engine = Slot::engine(LICHESS);
    env.op(Op::Authorize(engine));
    let url = env.browser_url();
    assert_eq!(query_param(&url, "scope"), "bot:play");
    env.approve();
    env.until("Engine autorizada", |ui| {
        matches!(
            ui.auth.endings.get(&engine),
            Some(Ending::Authorized { .. })
        )
    });
    let engine_id = env
        .lichess_list()
        .iter()
        .find(|c| c.role == Role::Engine)
        .expect("Conexão de Engine")
        .id;
    assert_ne!(engine_id, player_id);
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(player_id)));
    assert_eq!(env.authorized_id(Role::Engine), Some(net_id(engine_id)));
    assert_eq!(env.keyring.accounts().len(), 2, "dois segredos, dois ids");
    // Desconectar uma não toca a outra.
    env.server.route("/api/token", FakeResponse::status(204));
    env.op(Op::Disconnect(engine_id));
    env.until("desconectada", |ui| {
        !ui.list.iter().any(|c| c.id == engine_id)
    });
    assert_eq!(env.keyring.accounts().len(), 1);
    assert_eq!(env.authorized_id(Role::Engine), None);
    assert_eq!(env.authorized_id(Role::Player), Some(net_id(player_id)));
}

#[test]
fn disconnecting_revokes_at_the_provider_deletes_the_secret_and_removes_the_connection() {
    let mut env = env();
    let id = env.authorized();
    env.server.route("/api/token", FakeResponse::status(204));
    env.op(Op::Disconnect(id));
    assert!(env.ui.auth.is_disconnecting(id));
    env.until("desconectada", |ui| !ui.list.iter().any(|c| c.id == id));
    assert!(env.keyring.accounts().is_empty(), "o segredo foi apagado");
    assert!(env.service.vault().reference(net_id(id)).is_none());
    assert_eq!(env.authorized_id(Role::Player), None);
    assert!(!env.ui.authorized.contains(&LICHESS));
    assert_eq!(env.ui.notes[&LICHESS], Note::plain("connections-revoke-ok"));
    // O Lichess recebeu a revogação, com o token da Conexão.
    let revoke = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.method == "DELETE")
        .expect("revogou");
    assert_eq!(revoke.path, "/api/token");
    assert_eq!(
        revoke.header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
}

#[test]
fn when_the_remote_revocation_fails_the_local_secret_is_still_deleted_and_the_user_is_told() {
    let mut env = env();
    let id = env.authorized();
    env.server.route("/api/token", FakeResponse::status(500));
    env.op(Op::Disconnect(id));
    env.until("desconectada", |ui| !ui.list.iter().any(|c| c.id == id));
    assert!(env.keyring.accounts().is_empty());
    assert_eq!(
        env.ui.notes[&LICHESS],
        Note::plain("connections-revoke-remote-failed"),
        "não finge que revogou"
    );
}

#[test]
fn removing_a_connection_deletes_its_secret_and_a_new_one_never_inherits_it() {
    let mut env = env();
    let id = env.authorized();
    assert_eq!(env.keyring.accounts().len(), 1);
    env.op(Op::Remove(id));
    env.frame();
    assert!(env.keyring.accounts().is_empty(), "o segredo saiu do Cofre");
    assert!(env.service.vault().reference(net_id(id)).is_none());
    assert!(env.lichess_list().is_empty());
    assert_eq!(env.authorized_id(Role::Player), None);

    // Uma Conexão nova (outro nome) não herda nada, nem de um id reaproveitado.
    env.ui.forms.entry(LICHESS).or_default();
    env.op(Op::Link {
        provider: LICHESS,
        kind: ConnectionKind::PublicName,
        identifier: "nova_exemplo".into(),
        nickname: String::new(),
    });
    env.frame();
    let fresh = env.lichess_list()[0].clone();
    assert!(fresh.id > id, "AUTOINCREMENT: o id não volta");
    for candidate in [net_id(id), net_id(fresh.id)] {
        assert!(env.service.vault().reference(candidate).is_none());
        assert!(env.service.vault().read(candidate).is_err());
    }
    assert!(!fresh.is_authorized());
    assert_eq!(env.ui.provider_standing(LICHESS), None);
    assert_eq!(env.authorized_id(Role::Player), None);
}

#[test]
fn removing_a_connection_keeps_it_when_the_vault_refuses_to_delete_the_secret() {
    let mut env = env();
    let id = env.authorized();
    env.keyring.set_failing(true);
    env.op(Op::Remove(id));
    env.frame();
    assert_eq!(env.lichess_list().len(), 1, "a Conexão ficou");
    assert!(env.service.vault().reference(net_id(id)).is_some());
    assert_eq!(
        env.ui.notes[&LICHESS],
        Note::plain("connections-remove-error")
    );
}

#[test]
fn a_reservation_left_behind_by_a_closed_app_is_discarded_with_its_secret() {
    let mut env = env();
    // O app fechou depois de guardar o segredo e antes de concluir a autorização.
    let (id, profile) = with_store(&mut env.ui.local, None, |store, profile| {
        let id = store
            .begin_authorization(
                profile,
                "lichess",
                Role::Player,
                ConnectionKind::AuthorizedBrowser,
            )
            .unwrap();
        (id, profile)
    })
    .unwrap();
    env.service
        .vault()
        .store(
            secrets::net_profile(profile),
            LICHESS,
            net_id(id),
            Secret::new(TOKEN),
        )
        .unwrap();
    env.ui.dirty = true;
    env.frame();
    env.frame();
    assert!(env.lichess_list().is_empty(), "a reserva foi descartada");
    assert!(env.keyring.accounts().is_empty(), "e o segredo também");
}

#[test]
fn switching_the_local_profile_drops_what_belongs_to_the_other_one() {
    let mut env = env();
    env.authorize_player();
    assert!(env.ui.auth.attempt.is_some());
    // Outro Perfil local passa a ser o ativo: a espera não é dele.
    env.ui.loaded_for = Some(chess_store::ProfileId(99));
    env.frame();
    assert!(env.ui.auth.attempt.is_none());
}

#[test]
fn a_linked_name_is_never_authorized_and_the_authorized_state_needs_the_vault() {
    let mut env = env();
    env.ui.forms.entry(LICHESS).or_default();
    env.op(Op::Link {
        provider: LICHESS,
        kind: ConnectionKind::PublicName,
        identifier: "ana_exemplo".into(),
        nickname: String::new(),
    });
    env.frame();
    assert_eq!(env.ui.provider_standing(LICHESS), None);
    assert!(env.ui.authorized.is_empty());
    assert_eq!(env.authorized_id(Role::Player), None);

    // Uma linha "autorizada" no banco sem segredo no Cofre não vale como autorizada.
    with_store(&mut env.ui.local, None, |store, profile| {
        let id = store
            .begin_authorization(
                profile,
                "lichess",
                Role::Player,
                ConnectionKind::AuthorizedBrowser,
            )
            .unwrap();
        store
            .record_authorization(
                profile,
                id,
                ConnectionKind::AuthorizedBrowser,
                "fantasma",
                &["board:play".to_owned()],
                1,
            )
            .unwrap();
    });
    env.ui.dirty = true;
    env.frame();
    assert_eq!(env.ui.provider_standing(LICHESS), Some(Standing::Expired));
    assert_eq!(env.authorized_id(Role::Player), None);
}

#[test]
fn authorizing_a_name_already_linked_absorbs_it_without_duplicating() {
    let mut env = env();
    env.ui.forms.entry(LICHESS).or_default();
    env.op(Op::Link {
        provider: LICHESS,
        kind: ConnectionKind::PublicName,
        identifier: "georges".into(),
        nickname: "Principal".into(),
    });
    env.frame();
    env.authorized();
    let list = env.lichess_list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].nickname, "Principal");
    assert!(list[0].is_authorized());
    let _ = NewConnection::new("lichess", ConnectionKind::PublicName, "x");
}

#[test]
fn authorizing_while_another_authorization_runs_is_ignored() {
    let mut env = env();
    env.authorize_player();
    env.op(Op::Authorize(player()));
    assert_eq!(env.opened.lock().unwrap().len(), 1, "só um navegador");
    assert_eq!(env.lichess_list().len(), 1);
}

#[test]
fn a_failed_browser_open_is_reported_and_can_be_retried() {
    let mut env = env();
    env.ui.opener = Opener::recording(env.opened.clone(), false);
    env.authorize_player();
    assert!(env.ui.auth.attempt.as_ref().unwrap().open_failed);
    env.ui.opener = Opener::recording(env.opened.clone(), true);
    env.op(Op::ReopenBrowser);
    assert!(!env.ui.auth.attempt.as_ref().unwrap().open_failed);
    assert_eq!(env.opened.lock().unwrap().len(), 2);
}

#[test]
fn the_master_password_and_the_key_never_appear_in_debug_or_in_the_vault_dialog_text() {
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.op(Op::Authorize(player()));
    let dialog = env.ui.auth.vault_dialog.as_mut().unwrap();
    dialog.password = Zeroizing::new(PASSWORD.to_owned());
    dialog.confirm = Zeroizing::new(PASSWORD.to_owned());
    dialog.current = Zeroizing::new("senha atual de teste".to_owned());
    assert!(!format!("{:?}", env.ui).contains(PASSWORD));
    assert!(!format!("{:?}", env.ui).contains("senha atual de teste"));
    assert!(!format!("{:?}", env.ui.auth).contains(PASSWORD));
}

#[test]
fn permissions_read_in_plain_portuguese_and_extra_ones_are_flagged() {
    let i18n = crate::i18n::I18n::pt_br();
    for scope in [
        "board:play",
        "bot:play",
        "preference:read",
        "challenge:write",
    ] {
        let key = permission_key(scope).expect("conhecida");
        assert_ne!(i18n.t(key), key, "{scope}");
    }
    assert_eq!(permission_key("algo:novo"), None);
    assert!(!has_extra_permissions(
        &["board:play".to_owned()],
        Role::Player
    ));
    assert!(has_extra_permissions(
        &["board:play".to_owned(), "email:read".to_owned()],
        Role::Player
    ));
    assert!(has_extra_permissions(
        &["board:play".to_owned()],
        Role::Engine
    ));
    let _ = NetId(1);
}

#[test]
fn the_online_explorer_asks_only_with_an_authorized_connection_and_then_with_its_key() {
    use crate::explorer::{Authorization, Explorer, Input, Source};
    const FEN: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../chess-providers/tests/fixtures/lichess-explorer-masters.http");
    // Cofre próprio (sem cofre do sistema): é o que se bloqueia.
    let mut env = env_with(VolatileKeyring::missing(), true);
    env.service
        .vault()
        .create_own(&Secret::new(PASSWORD))
        .expect("cria");
    env.server.route(
        "/masters",
        FakeResponse::recorded(&fixture).expect("gravada"),
    );
    let mut explorer = Explorer::default();
    explorer.source = Source::Masters;
    let start = Instant::now();
    let drive = |env: &mut Env, explorer: &mut Explorer, from_ms: u64, frames: u64| {
        for i in 0..frames {
            env.frame();
            let input = Input {
                fen: FEN,
                now: start + Duration::from_millis(from_ms + i * 10),
                at: NetService::now(),
                lock: None,
                authorization: Authorization::lichess_now(&env.service),
                subject: None,
            };
            explorer.drive(&input, &env.service);
            std::thread::sleep(Duration::from_millis(3));
        }
    };

    // Sem Conexão autorizada, nenhum pedido sai (mesmo com o Lichess ligado e a Posição parada).
    drive(&mut env, &mut explorer, 0, 80);
    assert!(env.server.requests().is_empty());
    assert_eq!(
        Authorization::lichess_now(&env.service),
        Authorization::NotAvailable
    );

    // Com a Conexão autorizada pela Central, o Explorador pede, com a chave dela.
    env.authorized();
    assert_eq!(
        Authorization::lichess_now(&env.service),
        Authorization::Granted
    );
    let before = env.server.requests().len();
    drive(&mut env, &mut explorer, 1_000, 200);
    let masters: Vec<_> = env
        .server
        .requests()
        .into_iter()
        .skip(before)
        .filter(|r| r.path == "/masters")
        .collect();
    assert_eq!(masters.len(), 1, "um pedido ao Explorador");
    assert_eq!(
        masters[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );

    // Bloquear o Cofre (ou a recusa do Provedor) tira a Conexão: nenhum pedido novo.
    env.service.vault().lock();
    assert_eq!(env.service.vault().status(), VaultStatus::OwnLocked);
    env.frame();
    assert_eq!(env.authorized_id(Role::Player), None);
    explorer.source = Source::Lichess;
    let sent = env.server.requests().len();
    drive(&mut env, &mut explorer, 5_000, 80);
    assert_eq!(env.server.requests().len(), sent);
}
