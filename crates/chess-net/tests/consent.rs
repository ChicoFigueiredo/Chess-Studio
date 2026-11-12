//! Consentimento por Provedor (CX-09, ADR-0006): nasce desligado, liga-se por Perfil local,
//! fica gravado, e um Provedor desligado nunca abre conexão.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{LocalProfileId, Net, NetConfig, NetError, ProviderId, catalog};
use common::*;
use std::time::Duration;

#[test]
fn every_provider_is_born_disabled_and_the_app_starts_offline() {
    let clock = clock();
    let server = server(&clock);
    let net = net(&server, &clock);
    for provider in catalog() {
        assert!(
            !net.is_enabled(provider.id),
            "{} ligado de fábrica",
            provider.name
        );
    }
    let status = net.status();
    assert!(status.is_offline());
    assert_eq!(status.enabled_count(), 0);
}

#[test]
fn disabled_provider_opens_no_connection_and_says_which_provider_to_enable() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net(&server, &clock);

    let err = net.fetch(masters()).wait().unwrap_err();

    assert_eq!(
        err,
        NetError::ProviderDisabled {
            provider: ProviderId::Lichess
        }
    );
    let text = err.to_string();
    assert!(text.contains("Lichess"), "{text}");
    assert!(text.contains("Arquivo > Conexões"), "{text}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(server.connections(), 0, "nenhuma conexão pode ser aberta");
}

#[test]
fn turning_a_provider_off_again_blocks_the_next_request() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    net.fetch(masters()).wait().expect("ligado: consulta");
    net.set_enabled(ProviderId::Lichess, false)
        .expect("desliga");
    let err = net.fetch(masters()).wait().unwrap_err();

    assert!(matches!(err, NetError::ProviderDisabled { .. }));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn consent_is_per_local_profile_and_survives_reopening() {
    let dir = std::env::temp_dir().join(format!("chess-net-consent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("rede.db");
    let clock = clock();

    {
        let net = Net::open(NetConfig::at_path(&path).clock(clock.clone())).expect("abre");
        assert_eq!(net.active_profile(), LocalProfileId(1));
        net.set_enabled(ProviderId::Lichess, true).expect("liga");
        net.set_enabled(ProviderId::ChessCom, true).expect("liga");
        net.set_enabled(ProviderId::ChessCom, false)
            .expect("desliga");
    }

    let net = Net::open(NetConfig::at_path(&path).clock(clock.clone())).expect("reabre");
    assert!(net.is_enabled(ProviderId::Lichess));
    assert!(!net.is_enabled(ProviderId::ChessCom));
    assert_eq!(net.status().enabled_count(), 1);

    net.set_active_profile(LocalProfileId(2));
    assert!(!net.is_enabled(ProviderId::Lichess), "outro Perfil local");
    assert!(net.status().is_offline());

    net.set_active_profile(LocalProfileId(1));
    net.forget_profile(LocalProfileId(1))
        .expect("Perfil local excluído");
    assert!(!net.is_enabled(ProviderId::Lichess));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_provider_without_network_cannot_be_turned_on() {
    let clock = clock();
    let server = server(&clock);
    let net = net(&server, &clock);
    for id in [
        ProviderId::QueenAlice,
        ProviderId::Twic,
        ProviderId::PgnMentor,
    ] {
        let err = net.set_enabled(id, true).unwrap_err();
        assert_eq!(err, NetError::NoNetworkService { provider: id });
        assert!(!net.is_enabled(id));
    }
}

#[test]
fn the_consent_notice_says_what_is_sent_and_where() {
    for provider in catalog().iter().filter(|p| p.uses_network()) {
        let notice = provider.sends;
        assert!(notice.len() > 40, "{}: aviso curto demais", provider.name);
        for endpoint in provider.endpoints {
            assert!(
                notice.contains(endpoint.host),
                "{}: o aviso não cita {}",
                provider.name,
                endpoint.host
            );
        }
    }
}

/// Ramo de segurança (NFR-10, #269): o banco do consentimento que o app não entende (versão
/// mais nova) ou que não abre é erro tipado `Storage`; a rede nunca sobe "sem consentimentos"
/// em silêncio, e nada é regravado.
#[test]
fn a_consent_database_the_app_cannot_read_is_a_typed_storage_error() {
    let dir = std::env::temp_dir().join(format!("chess-net-storage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let clock = clock();

    // Versão do esquema mais nova que a do app.
    let newer = dir.join("nova.db");
    {
        let conn = rusqlite::Connection::open(&newer).expect("cria");
        conn.pragma_update(None, "user_version", 9999)
            .expect("versão");
    }
    let err = Net::open(NetConfig::at_path(&newer).clock(clock.clone())).unwrap_err();
    assert!(matches!(err, NetError::Storage { .. }), "{err:?}");

    // Um diretório no lugar do arquivo do banco.
    let err = Net::open(NetConfig::at_path(&dir).clock(clock.clone())).unwrap_err();
    assert!(matches!(err, NetError::Storage { .. }), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
