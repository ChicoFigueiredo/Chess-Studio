//! Teste estático do Cofre: só o Cofre e a camada de rede leem o segredo em claro
//! (`Secret::expose_secret`), e o cofre do sistema falso só entra em testes.

use std::fs;
use std::path::{Path, PathBuf};

/// Os únicos arquivos de produção que leem um segredo em claro: o `Secret` e o cabeçalho
/// (`chess-net`), o cofre do sistema e o cofre próprio (`chess-vault`).
const READERS: [&str; 3] = [
    "chess-net/src/secret.rs",
    "chess-vault/src/keyring.rs",
    "chess-vault/src/vault.rs",
];

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

/// Código de produção (`crates/*/src`) sem os módulos `#[cfg(test)]` do fim dos arquivos.
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
            let text = fs::read_to_string(&p).expect("lê fonte");
            let code = text.split("#[cfg(test)]").next().unwrap_or("").to_owned();
            (rel, code)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn only_the_vault_and_the_network_header_read_a_secret_in_clear() {
    let readers: Vec<String> = production_sources()
        .into_iter()
        .filter(|(_, code)| code.contains("expose_secret("))
        .map(|(p, _)| p)
        .collect();
    let allowed: Vec<String> = READERS.iter().map(|s| (*s).to_owned()).collect();
    for reader in &readers {
        assert!(
            allowed.contains(reader),
            "{reader} lê segredo em claro fora do Cofre"
        );
    }
}

#[test]
fn the_fake_system_vault_is_enabled_only_for_tests() {
    for krate in fs::read_dir(workspace_crates()).expect("crates") {
        let dir = krate.expect("crate").path();
        let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line.to_owned();
                continue;
            }
            if line.starts_with("chess-vault") && line.contains("testing") {
                assert!(
                    section.contains("dev-dependencies"),
                    "{} liga o cofre falso fora de [dev-dependencies] ({section})",
                    dir.display()
                );
            }
        }
    }
    let lib = fs::read_to_string(workspace_crates().join("chess-vault/src/lib.rs")).expect("lib");
    assert!(
        lib.contains("#[cfg(feature = \"testing\")]\npub mod testing;"),
        "o cofre falso precisa ficar atrás da feature `testing`"
    );
}

#[test]
fn the_secret_type_keeps_debug_hidden_and_has_no_display() {
    let secret =
        fs::read_to_string(workspace_crates().join("chess-net/src/secret.rs")).expect("secret.rs");
    assert!(!secret.contains("Display for Secret"));
    assert!(secret.contains("impl fmt::Debug for Secret"));
    assert!(secret.contains("impl fmt::Debug for Authorization"));
    for name in ["pub struct Secret", "pub struct Authorization"] {
        let at = secret.find(name).expect("tipo");
        let attributes = &secret[secret[..at].rfind("\n\n").unwrap_or(0)..at];
        assert!(
            !attributes
                .lines()
                .any(|l| l.trim_start().starts_with("#[derive") && l.contains("Debug")),
            "{name} não pode derivar Debug"
        );
    }
}
