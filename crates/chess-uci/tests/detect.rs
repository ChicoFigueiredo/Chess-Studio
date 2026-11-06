//! Detecção de Engines instaladas (DEC-27) contra a Engine falsa: só entra na lista quem
//! responde ao handshake UCI; nomes desconhecidos nem são executados.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chess_uci::{detect_in, load_preferred, probe_engine, save_preferred};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");
const SHORT: Duration = Duration::from_millis(800);

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("detect-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("criar diretório de teste");
    dir
}

fn install_fake(dir: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    fs::copy(FAKE, &target).expect("copiar Engine falsa");
    target
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let target = dir.join(name);
    fs::write(&target, format!("#!/bin/sh\n{body}\n")).expect("escrever script");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod");
    target
}

#[test]
fn handshake_returns_name_author_and_path() {
    let dir = scratch("probe");
    let engine = install_fake(&dir, "stockfish");
    let found = probe_engine(&engine, SHORT).expect("handshake");
    assert_eq!(found.path, engine);
    assert_eq!(found.name.as_deref(), Some("FakeUci 1.0"));
    assert_eq!(found.author.as_deref(), Some("Chess Studio tests"));
}

#[test]
fn only_candidates_that_speak_uci_are_listed() {
    let dir = scratch("filter");
    let good = install_fake(&dir, "stockfish");
    // Nome de Engine, mas não fala UCI: sai na hora, sem handshake.
    script(&dir, "lc0", "exit 0");
    // Nome de Engine, mas nunca responde: o prazo curto o descarta.
    script(&dir, "komodo", "sleep 5");
    // Nome desconhecido: nem é executado (este escreveria um arquivo-marca).
    let marker = dir.join("executed");
    script(&dir, "tool", &format!("touch {}", marker.display()));
    // Arquivo que não é executável.
    fs::write(dir.join("stockfish-notes"), "texto").expect("escrever");

    let found = detect_in(std::slice::from_ref(&dir), SHORT);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, good);
    assert!(
        !marker.exists(),
        "candidata de nome desconhecido foi executada"
    );
}

#[test]
fn symlinks_to_the_same_engine_appear_once() {
    let dir = scratch("links");
    let real = install_fake(&dir, "stockfish");
    let other = scratch("links-b");
    std::os::unix::fs::symlink(&real, other.join("stockfish")).expect("symlink");

    let found = detect_in(&[dir, other], SHORT);
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn missing_directories_are_ignored() {
    let found = detect_in(&[PathBuf::from("/não/existe")], SHORT);
    assert!(found.is_empty());
}

#[test]
fn preferred_engine_round_trips_and_forgets_deleted_files() {
    let dir = scratch("preferred");
    let engine = install_fake(&dir, "stockfish");
    assert_eq!(load_preferred(&dir), None);
    save_preferred(&dir, &engine).expect("gravar");
    assert_eq!(load_preferred(&dir), Some(engine.clone()));
    fs::remove_file(&engine).expect("apagar");
    assert_eq!(load_preferred(&dir), None);
}
