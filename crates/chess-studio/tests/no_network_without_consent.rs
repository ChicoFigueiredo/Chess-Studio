//! PLAT-02 / ADR-0006: nenhum código de rede roda sem ação do usuário. A rede só sai por dois
//! lugares:
//!
//! - o download fixado de Engine (`chess-uci::install`, ADR-0003), chamado só pelo assistente
//!   (depois do botão de consentimento) e pelo gerenciador de Engines (ação explícita na UI);
//! - a camada de rede por Provedor (`chess-net`), em que todo pedido passa pelo gate
//!   (consentimento por Provedor, catálogo, Trava de fair play, limites) antes do transporte.
//!
//! Nenhum outro crate declara cliente HTTP nem abre socket. Teste estático: varre as fontes e
//! os manifestos e falha se surgir um terceiro caminho de rede.

use std::fs;
use std::path::{Path, PathBuf};

/// Os únicos arquivos de produção que falam `ureq`.
const HTTP_CLIENTS: [&str; 2] = ["chess-net/src/transport.rs", "chess-uci/src/install.rs"];
/// Os únicos crates que declaram `ureq`.
const HTTP_CRATES: [&str; 2] = ["chess-net", "chess-uci"];
/// Sockets crus só no servidor falso dos testes (feature `testing` do chess-net).
const SOCKET_DIR: &str = "chess-net/src/testing/";

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
fn raw_sockets_exist_only_in_the_fake_server_of_the_tests() {
    for socket in ["TcpStream", "UdpSocket", "TcpListener", "std::net::"] {
        for file in files_containing(socket) {
            assert!(
                file.starts_with(SOCKET_DIR),
                "{socket} fora do servidor falso: {file}"
            );
        }
    }
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
fn inside_the_provider_layer_only_the_gate_reaches_the_transport() {
    let users: Vec<String> = production_sources()
        .into_iter()
        .filter(|(p, _)| p.starts_with("chess-net/src/") && p != "chess-net/src/transport.rs")
        .filter(|(_, s)| {
            let code = non_test_part(s);
            code.contains("transport::") || code.contains("Http::new") || code.contains(".http.")
        })
        .map(|(p, _)| p)
        .collect();
    assert_eq!(users, vec!["chess-net/src/net.rs".to_owned()]);
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
