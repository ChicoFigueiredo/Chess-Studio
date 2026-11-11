//! O Cofre com o cofre do sistema (falso, em memória): o segredo vai para lá e nunca para o
//! disco; a autorização fica presa ao Provedor e ao Perfil local; conferência antes de
//! guardar; recusa do Provedor marcada; nada de segredo em `Debug` ou erro; e o Cofre como
//! fonte de autorização da camada de rede.

mod common;

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{
    AuthorizationSource, ConnectionId, LocalProfileId, Net, NetConfig, NetError, ProviderId,
    Request,
};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{Rejection, SecretVault, VaultError, VaultStatus};
use common::*;
use std::fs;
use std::sync::Arc;

fn system_vault(name: &str) -> (std::path::PathBuf, Arc<MemoryKeyring>, SecretVault) {
    let dir = temp_dir(name);
    let keyring = Arc::new(MemoryKeyring::new());
    let vault = open(&dir, &keyring, &clock());
    (dir, keyring, vault)
}

#[test]
fn with_a_system_vault_the_secret_goes_there_and_never_to_disk() {
    let (dir, keyring, vault) = system_vault("sistema");
    assert_eq!(vault.status(), VaultStatus::System);

    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");

    assert_eq!(keyring.accounts(), vec!["lichess/1/7".to_owned()]);
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
    let refs = vault.references();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].connection, CONNECTION);
    assert_eq!(refs[0].provider, LICHESS);
    assert_eq!(refs[0].profile, PROFILE);
    assert_eq!(refs[0].stored_at, T0);
    assert_eq!(refs[0].rejected, None);
    assert!(dir.join("index.json").exists());
    assert!(
        files_containing(&dir, TOKEN).is_empty(),
        "token em claro no disco"
    );

    // Reaberto, a referência continua e o segredo vem do cofre do sistema.
    let reopened = open(&dir, &keyring, &clock());
    assert_eq!(
        reopened.read(CONNECTION).expect("lê").expose_secret(),
        TOKEN
    );
    assert_eq!(
        vault.store(PROFILE, LICHESS, CONNECTION, secret("")),
        Err(VaultError::EmptySecret)
    );
}

#[test]
fn preloading_reads_the_system_vault_once_so_requests_never_wait_for_it() {
    let (dir, keyring, vault) = system_vault("preload");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    drop(vault);

    let vault = open(&dir, &keyring, &clock());
    assert_eq!(vault.preload(), Ok(1));
    keyring.set_failing(true);

    assert!(
        vault.authorization(PROFILE, LICHESS, CONNECTION).is_some(),
        "o gate não volta ao cofre do sistema"
    );
    let fresh = open(&dir, &keyring, &clock());
    assert!(matches!(fresh.preload(), Err(VaultError::System { .. })));
}

#[test]
fn replacing_and_deleting() {
    let (_dir, keyring, vault) = system_vault("substitui");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");

    vault
        .store(PROFILE, LICHESS, CONNECTION, secret("lip_NOVO_TOKEN_99"))
        .expect("substitui");
    assert_eq!(
        vault.read(CONNECTION).expect("lê").expose_secret(),
        "lip_NOVO_TOKEN_99"
    );
    assert_eq!(keyring.accounts().len(), 1);

    vault.delete(CONNECTION).expect("apaga");
    assert!(keyring.accounts().is_empty());
    assert!(vault.references().is_empty());
    assert_eq!(vault.read(CONNECTION).unwrap_err(), VaultError::NotFound);
    vault.delete(CONNECTION).expect("apagar de novo não é erro");
}

#[test]
fn a_failing_system_vault_is_reported_and_the_reference_stays() {
    let (_dir, keyring, vault) = system_vault("falha");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    keyring.set_failing(true);

    let err = vault.delete(CONNECTION).unwrap_err();
    assert!(matches!(err, VaultError::System { .. }), "{err:?}");
    assert!(
        vault.reference(CONNECTION).is_some(),
        "o app não finge que apagou"
    );
    let err = vault
        .store(PROFILE, LICHESS, ConnectionId(8), secret(TOKEN))
        .unwrap_err();
    assert!(matches!(err, VaultError::System { .. }), "{err:?}");
    assert!(vault.reference(ConnectionId(8)).is_none());
    assert!(!err.to_string().contains(TOKEN));
}

#[test]
fn the_authorization_is_bound_to_the_provider_and_the_local_profile() {
    let (_dir, _keyring, vault) = system_vault("vinculo");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");

    let auth = vault
        .authorization(PROFILE, LICHESS, CONNECTION)
        .expect("autoriza");
    assert_eq!(auth.provider(), LICHESS);
    for (profile, provider, connection) in [
        (LocalProfileId(2), LICHESS, CONNECTION),
        (PROFILE, ProviderId::ChessCom, CONNECTION),
        (PROFILE, ProviderId::PlayStrategy, CONNECTION),
        (PROFILE, LICHESS, ConnectionId(8)),
    ] {
        assert!(
            vault.authorization(profile, provider, connection).is_none(),
            "{profile:?} {provider:?} {connection:?}"
        );
    }
}

#[test]
fn a_tampered_index_cannot_rebind_a_secret_to_another_profile_or_provider() {
    let (dir, keyring, vault) = system_vault("indice");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    drop(vault);
    let path = dir.join("index.json");
    let text = fs::read_to_string(&path).expect("índice");
    fs::write(&path, text.replace("\"profile\": 1", "\"profile\": 2")).expect("adultera");

    let vault = open(&dir, &keyring, &clock());
    assert!(
        vault
            .authorization(LocalProfileId(2), LICHESS, CONNECTION)
            .is_none(),
        "a conta no cofre do sistema leva o Perfil: o índice não a redireciona"
    );

    fs::write(&path, b"{ isto nao e json").expect("corrompe");
    let err = SecretVault::open(config(&dir, &keyring, &clock())).unwrap_err();
    assert_eq!(err, VaultError::Corrupted { file: "índice" });
}

#[test]
fn a_staged_secret_is_used_only_while_it_lives_and_replaces_only_on_commit() {
    let (_dir, keyring, vault) = system_vault("conferencia");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    let header = |v: &SecretVault| {
        format!(
            "{:?}",
            v.authorization(PROFILE, LICHESS, CONNECTION)
                .map(|a| a.provider())
        )
    };

    let staged = vault.stage(PROFILE, LICHESS, CONNECTION, secret("lip_EM_CONFERENCIA_1"));
    assert_eq!(
        vault.read(CONNECTION).expect("lê").expose_secret(),
        TOKEN,
        "o guardado não muda durante a conferência"
    );
    assert_eq!(header(&vault), "Some(Lichess)");
    // Outro Perfil não usa o segredo em conferência.
    assert!(
        vault
            .authorization(LocalProfileId(2), LICHESS, CONNECTION)
            .is_none()
    );
    drop(staged);
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);

    let staged = vault.stage(PROFILE, LICHESS, CONNECTION, secret("lip_EM_CONFERENCIA_2"));
    staged.commit().expect("confirma");
    assert_eq!(
        vault.read(CONNECTION).expect("lê").expose_secret(),
        "lip_EM_CONFERENCIA_2"
    );
    assert_eq!(keyring.accounts().len(), 1);
}

#[test]
fn a_rejection_by_the_provider_marks_the_reference_and_keeps_the_secret() {
    let dir = temp_dir("recusa");
    let keyring = Arc::new(MemoryKeyring::new());
    let clock = clock();
    let vault = open(&dir, &keyring, &clock);
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    clock.advance(std::time::Duration::from_secs(60));

    vault.rejected(PROFILE, LICHESS, CONNECTION, 401);
    // Recusa de outro Perfil ou Provedor não mexe nesta Conexão.
    vault.rejected(LocalProfileId(2), LICHESS, CONNECTION, 403);

    let expected = Some(Rejection {
        status: 401,
        at: T0.plus(std::time::Duration::from_secs(60)),
    });
    assert_eq!(vault.reference(CONNECTION).expect("ref").rejected, expected);
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
    let reopened = open(&dir, &keyring, &clock);
    assert_eq!(
        reopened.reference(CONNECTION).expect("ref").rejected,
        expected,
        "a marca sobrevive ao reinício"
    );

    vault
        .store(PROFILE, LICHESS, CONNECTION, secret("lip_REAUTORIZADO_1"))
        .expect("reautoriza");
    assert_eq!(vault.reference(CONNECTION).expect("ref").rejected, None);
}

#[test]
fn no_secret_shows_up_in_debug_or_errors() {
    let (_dir, _keyring, vault) = system_vault("debug");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    let staged = vault.stage(PROFILE, LICHESS, ConnectionId(8), secret(TOKEN));

    let texts = [
        format!("{vault:?}"),
        format!("{:?}", vault.references()),
        format!("{staged:?}"),
        format!("{:?}", vault.read(CONNECTION)),
        format!("{:?}", vault.authorization(PROFILE, LICHESS, CONNECTION)),
        format!("{:?}", secret(TOKEN)),
        VaultError::WrongPassword.to_string(),
    ];
    for text in texts {
        assert!(!text.contains(TOKEN), "segredo vazou: {text}");
        assert!(!text.contains("SECRETO"), "segredo vazou: {text}");
    }
}

// ---------------- O Cofre como fonte de autorização da camada de rede ----------------

fn net_with(vault: &SecretVault, server: &FakeServer) -> Net {
    let net = Net::open(
        NetConfig::in_memory()
            .clock(clock())
            .route("lichess.org", &server.base_url())
            .route("explorer.lichess.org", &server.base_url())
            .route("tablebase.lichess.ovh", &server.base_url())
            .vault(Arc::new(vault.clone())),
    )
    .expect("abre a rede");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");
    net
}

#[test]
fn the_network_gets_the_header_from_the_vault_only_for_authenticated_endpoints() {
    let (_dir, _keyring, vault) = system_vault("rede");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    let server = FakeServer::start();
    server.route("/masters", FakeResponse::json("{}"));
    server.route("/standard", FakeResponse::json("{}"));
    server.route("/api/account", FakeResponse::status(401));
    let net = net_with(&vault, &server);

    net.fetch(
        Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters")
            .connection(CONNECTION),
    )
    .wait()
    .expect("Explorador autorizado");
    net.fetch(
        Request::get(
            ProviderId::Lichess,
            "https://tablebase.lichess.ovh/standard",
        )
        .query("fen", "8/8/8/8/8/8/4K3/6kQ w - - 0 1")
        .connection(CONNECTION),
    )
    .wait()
    .expect("finais");
    let err = net
        .fetch(
            Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
                .connection(CONNECTION),
        )
        .wait()
        .unwrap_err();

    let sent = server.requests();
    assert_eq!(
        sent[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(sent[1].header("authorization"), None, "finais sem token");
    assert!(matches!(
        err,
        NetError::ConnectionRejected { status: 401, .. }
    ));
    assert_eq!(
        vault
            .reference(CONNECTION)
            .expect("ref")
            .rejected
            .map(|r| r.status),
        Some(401),
        "o 401 marcou a Conexão"
    );
    assert!(
        format!("{:?}", net.recent_operations())
            .find(TOKEN)
            .is_none(),
        "token no diário de operações"
    );
}

#[test]
fn with_the_own_vault_locked_nothing_leaves_and_the_error_is_typed() {
    let dir = temp_dir("rede-bloqueado");
    let keyring = Arc::new(MemoryKeyring::unavailable());
    let vault = open(&dir, &keyring, &clock());
    vault.create_own(&secret(PASSWORD)).expect("cria");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    vault.lock();
    let server = FakeServer::start();
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_with(&vault, &server);

    let err = net
        .fetch(
            Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters")
                .connection(CONNECTION),
        )
        .wait()
        .unwrap_err();

    assert!(matches!(err, NetError::NotAuthorized { .. }), "{err:?}");
    assert_eq!(server.connections(), 0);
    vault.unlock(&secret(PASSWORD)).expect("desbloqueia");
    net.fetch(
        Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters")
            .connection(CONNECTION),
    )
    .wait()
    .expect("desbloqueado, sai com o token");
    assert!(files_containing(&dir, TOKEN).is_empty());
}
