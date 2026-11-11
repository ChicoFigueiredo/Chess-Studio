//! PLAT-02 / ADR-0006: nenhum código de rede roda sem ação do usuário. A rede só sai por dois
//! lugares:
//!
//! - o download fixado de Engine (`chess-uci::install`, ADR-0003), chamado só pelo assistente
//!   (depois do botão de consentimento) e pelo gerenciador de Engines (ação explícita na UI);
//! - a camada de rede por Provedor (`chess-net`), em que todo pedido passa pelo gate
//!   (consentimento por Provedor, catálogo, Trava de fair play, limites) antes do transporte.
//!   O gate tem dois transportes: HTTP (`transport.rs`, `ureq`) e ICS (`ics_transport.rs`,
//!   TCP de texto do FICS, #237), cada um chamado só pelo seu gate (`net.rs` e `net/ics.rs`).
//!
//! Nenhum outro crate declara cliente HTTP nem abre socket. Teste estático: varre as fontes e
//! os manifestos e falha se surgir um terceiro caminho de rede.
//!
//! **Única escuta de rede permitida:** o servidor de loopback do retorno da autorização pelo
//! navegador (OAuth2 PKCE do Lichess, #224), em `chess-net/src/loopback.rs`. É uma exceção
//! legítima e só ela: o RFC 8252 (OAuth2 em app nativo) exige receber o código num endereço
//! de loopback, e o arquivo escuta só em `127.0.0.1`, porta efêmera, para um único retorno,
//! com prazo, e não fala com Provedor nenhum (a troca do código passa pelo gate). Qualquer
//! outro arquivo com socket continua reprovado.
//!
//! **Única conexão TCP crua de produção:** o transporte ICS do FICS, em
//! `chess-net/src/ics_transport.rs` (#237). Só abre conexão (nunca escuta), num único ponto
//! (`TcpStream::connect_timeout`), e só o gate do ICS o chama, depois do consentimento, do
//! catálogo dos endereços ICS e da Trava de fair play.

use std::fs;
use std::path::{Path, PathBuf};

/// Os únicos arquivos de produção que falam `ureq`.
const HTTP_CLIENTS: [&str; 2] = ["chess-net/src/transport.rs", "chess-uci/src/install.rs"];
/// Os únicos crates que declaram `ureq`.
const HTTP_CRATES: [&str; 2] = ["chess-net", "chess-uci"];
/// Sockets crus só no servidor falso dos testes (feature `testing` do chess-net)...
const SOCKET_DIR: &str = "chess-net/src/testing/";
/// ...e no servidor de loopback do retorno da autorização (a única escuta de produção).
const LOOPBACK_FILE: &str = "chess-net/src/loopback.rs";
/// ...e o transporte ICS do FICS (a única conexão TCP crua de produção, #237).
const ICS_TRANSPORT_FILE: &str = "chess-net/src/ics_transport.rs";
/// O gate do ICS: o único arquivo que chega ao transporte ICS.
const ICS_GATE_FILE: &str = "chess-net/src/net/ics.rs";
/// ...e o "navegador" falso dos testes da Central, que só conecta ao retorno em `127.0.0.1`
/// da autorização (nunca a um endereço de fora). Arquivo só de testes.
const BROWSER_STUB_FILE: &str = "chess-studio/src/connections/auth_tests.rs";

fn workspace_crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("lê diretório") {
        let path = entry.expect("entrada").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Fontes de produção (`crates/*/src`), sem testes de integração.
fn production_sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    for krate in fs::read_dir(workspace_crates()).expect("crates") {
        let src = krate.expect("crate").path().join("src");
        if src.is_dir() {
            rust_sources(&src, &mut files);
        }
    }
    let mut out: Vec<(String, String)> = files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(workspace_crates())
                .expect("dentro de crates")
                .to_string_lossy()
                .replace('\\', "/");
            (rel, fs::read_to_string(&p).expect("lê fonte"))
        })
        .collect();
    out.sort();
    out
}

/// Código sem os módulos `#[cfg(test)]` (que ficam no fim dos arquivos neste projeto).
fn non_test_part(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

fn files_containing(needle: &str) -> Vec<String> {
    production_sources()
        .into_iter()
        .filter(|(_, s)| non_test_part(s).contains(needle))
        .map(|(p, _)| p)
        .collect()
}

/// `(crate, manifesto)` de cada crate do workspace.
fn manifests() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for krate in fs::read_dir(workspace_crates()).expect("crates") {
        let dir = krate.expect("crate").path();
        let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let name = dir
            .file_name()
            .expect("nome")
            .to_string_lossy()
            .into_owned();
        out.push((name, text));
    }
    out
}

fn declares(manifest: &str, dep: &str) -> bool {
    manifest.lines().any(|l| {
        let l = l.trim_start();
        [" ", "=", "."]
            .iter()
            .any(|sep| l.starts_with(&format!("{dep}{sep}")))
    })
}

#[test]
fn only_the_engine_installer_and_the_provider_layer_speak_http() {
    let mut users = files_containing("ureq::");
    users.sort();
    assert_eq!(users, HTTP_CLIENTS.map(str::to_owned).to_vec());
    for forbidden in [
        "reqwest",
        "hyper::",
        "tokio::net",
        "isahc",
        "surf::",
        "attohttpc",
    ] {
        let hits = files_containing(forbidden);
        assert!(hits.is_empty(), "{forbidden} em {hits:?}");
    }
}

#[test]
fn raw_sockets_exist_only_in_the_fake_server_the_oauth_loopback_and_the_ics_transport() {
    for socket in ["TcpStream", "UdpSocket", "TcpListener", "std::net::"] {
        for file in files_containing(socket) {
            let ics =
                file == ICS_TRANSPORT_FILE && socket != "TcpListener" && socket != "UdpSocket";
            assert!(
                file.starts_with(SOCKET_DIR)
                    || file == LOOPBACK_FILE
                    || file == BROWSER_STUB_FILE
                    || ics,
                "{socket} fora do servidor falso, do loopback da autorização e do transporte ICS: {file}"
            );
        }
    }
    // O transporte ICS só abre conexão, num único ponto, e não é cliente HTTP.
    let ics = fs::read_to_string(workspace_crates().join(ICS_TRANSPORT_FILE)).expect("ICS");
    let code = non_test_part(&ics);
    assert_eq!(
        code.matches("TcpStream::connect_timeout(").count(),
        1,
        "um único ponto de conexão ICS"
    );
    assert!(
        !code.contains("TcpStream::connect(") && !code.contains("bind("),
        "o transporte ICS não escuta nem conecta por outro caminho"
    );
    assert!(
        !code.contains("ureq"),
        "o transporte ICS não é cliente HTTP"
    );
    // O loopback só escuta (TcpListener) e só em 127.0.0.1: nunca abre conexão para fora.
    let loopback = fs::read_to_string(workspace_crates().join(LOOPBACK_FILE)).expect("loopback");
    let code = non_test_part(&loopback);
    assert!(
        !code.contains("TcpStream::connect"),
        "o loopback não abre conexão"
    );
    assert!(!code.contains("UdpSocket"), "o loopback não usa UDP");
    assert!(!code.contains("ureq"), "o loopback não é cliente HTTP");
    assert!(
        code.contains("Ipv4Addr::LOCALHOST") && !code.contains("UNSPECIFIED"),
        "o loopback escuta só em 127.0.0.1"
    );
    assert_eq!(
        code.matches("TcpListener::bind(").count(),
        1,
        "um único ponto de escuta"
    );
    let lib = fs::read_to_string(workspace_crates().join("chess-net/src/lib.rs")).expect("lib.rs");
    assert!(
        lib.contains("#[cfg(feature = \"testing\")]\npub mod testing;"),
        "o servidor falso precisa ficar atrás da feature `testing`"
    );
}

#[test]
fn network_cargo_dependencies_exist_only_in_the_two_network_crates() {
    for (name, text) in manifests() {
        if declares(&text, "ureq") {
            assert!(
                HTTP_CRATES.contains(&name.as_str()),
                "{name} declara a dependência de rede ureq"
            );
        }
        for dep in [
            "reqwest",
            "hyper",
            "isahc",
            "surf",
            "attohttpc",
            "curl",
            "tokio",
        ] {
            assert!(
                !declares(&text, dep),
                "{name} declara a dependência de rede {dep}"
            );
        }
    }
}

#[test]
fn the_fake_server_is_enabled_only_for_tests() {
    for (name, text) in manifests() {
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line.to_owned();
                continue;
            }
            if line.starts_with("chess-net") && line.contains("testing") {
                assert!(
                    section.contains("dev-dependencies"),
                    "{name} liga o servidor falso fora de [dev-dependencies] ({section})"
                );
            }
        }
    }
}

#[test]
fn inside_the_provider_layer_only_the_gate_reaches_the_transports() {
    let net_sources: Vec<(String, String)> = production_sources()
        .into_iter()
        .filter(|(p, _)| p.starts_with("chess-net/src/"))
        .collect();
    let http_users: Vec<String> = net_sources
        .iter()
        .filter(|(p, _)| p != "chess-net/src/transport.rs" && p != ICS_TRANSPORT_FILE)
        .filter(|(_, s)| {
            let code = non_test_part(s).replace("ics_transport::", "");
            code.contains("transport::") || code.contains("Http::new") || code.contains(".http.")
        })
        .map(|(p, _)| p.clone())
        .collect();
    assert_eq!(http_users, vec!["chess-net/src/net.rs".to_owned()]);
    let ics_users: Vec<String> = net_sources
        .iter()
        .filter(|(p, _)| p != ICS_TRANSPORT_FILE)
        .filter(|(_, s)| {
            let code = non_test_part(s);
            code.contains("ics_transport::") || code.contains("IcsSocket")
        })
        .map(|(p, _)| p.clone())
        .collect();
    assert_eq!(ics_users, vec![ICS_GATE_FILE.to_owned()]);
    // O gate do ICS confere consentimento e catálogo, e de novo logo antes de conectar.
    let gate = fs::read_to_string(workspace_crates().join(ICS_GATE_FILE)).expect("gate ICS");
    let code = non_test_part(&gate);
    let admit = code.find("fn admit_ics").expect("gate do ICS");
    assert!(code[admit..].contains("is_enabled(") && code[admit..].contains("ics_endpoint("));
    let connect = code.find("IcsSocket::connect(").expect("conexão ICS");
    assert!(
        code[..connect].contains("self.admit_ics("),
        "o gate confere de novo logo antes de conectar"
    );
}

#[test]
fn download_entry_points_are_only_called_by_the_wizard_and_the_manager() {
    for entry in [
        "chess_uci::install(",
        "chess_uci::install_network(",
        "chess_uci::install_from_catalog(",
        "chess_uci::install_network_from_catalog(",
    ] {
        for file in files_containing(entry) {
            assert!(
                file == "chess-studio/src/engine_wizard.rs"
                    || file == "chess-studio/src/engine_manager.rs",
                "{entry} chamado fora do assistente/gerenciador: {file}"
            );
        }
    }
}

#[test]
fn wizard_starts_the_download_only_from_the_consent_button() {
    let wizard = production_sources()
        .into_iter()
        .find(|(p, _)| p == "chess-studio/src/engine_wizard.rs")
        .expect("engine_wizard.rs")
        .1;
    let code = non_test_part(&wizard);
    let calls: Vec<_> = code.match_indices("start_download(").collect();
    // Definição + a única chamada, dentro do ramo `wizard-download-confirm`.
    assert_eq!(calls.len(), 2, "start_download deve ter uma única chamada");
    let call_at = calls[1].0;
    let window = &code[call_at.saturating_sub(120)..call_at];
    assert!(
        window.contains("wizard-download-confirm"),
        "start_download fora do botão de consentimento"
    );
}
