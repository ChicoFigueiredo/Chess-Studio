//! PLAT-02 / ADR-0003: nenhum código de rede roda sem ação do usuário. O único cliente de rede
//! do workspace é o download de Engine (`chess-uci::install`), e ele só é chamado pelo assistente
//! (depois do botão de consentimento) e pelo gerenciador de Engines (ação explícita na UI).
//! Teste estático: varre as fontes e falha se surgir outro caminho de rede.

use std::fs;
use std::path::{Path, PathBuf};

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
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(workspace_crates())
                .expect("dentro de crates")
                .to_string_lossy()
                .replace('\\', "/");
            (rel, fs::read_to_string(&p).expect("lê fonte"))
        })
        .collect()
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

#[test]
fn only_the_engine_installer_talks_to_the_network() {
    assert_eq!(
        files_containing("ureq::"),
        vec!["chess-uci/src/install.rs".to_owned()]
    );
    for forbidden in [
        "TcpStream",
        "UdpSocket",
        "TcpListener",
        "reqwest",
        "hyper::",
        "std::net::",
        "tokio::net",
    ] {
        let hits = files_containing(forbidden);
        assert!(hits.is_empty(), "{forbidden} em {hits:?}");
    }
}

#[test]
fn network_cargo_dependencies_exist_only_in_the_engine_crate() {
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
        for dep in ["ureq", "reqwest", "hyper", "isahc", "surf", "attohttpc"] {
            let declared = text.lines().any(|l| {
                let l = l.trim_start();
                l.starts_with(&format!("{dep} ")) || l.starts_with(&format!("{dep}="))
            });
            assert!(
                !declared || (name == "chess-uci" && dep == "ureq"),
                "{name} declara a dependência de rede {dep}"
            );
        }
    }
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
