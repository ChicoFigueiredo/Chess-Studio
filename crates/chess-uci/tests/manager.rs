//! Gerenciador de Engines (UCI-05, PEND-08): atualização que preserva a versão anterior,
//! remoção que só apaga o que o app instalou, rede neural verificada e opções UCI validadas.
//! Servidor HTTP local e Engine falsa; nada toca a rede real.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use chess_uci::{
    ArchiveKind, Control, Engine, EngineConfig, EngineOption, InstallError, InstallSpec, Limits,
    NetworkSpec, OptionGroup, RemoveError, SettingError, group_of, install, install_network,
    parse_option, remove_installed, resolve_settings, update, validate_option,
};
use sha2::{Digest, Sha256};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");
const EXE: &str = "pkg/lc0-test";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("manager-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("criar diretório de teste");
    dir
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(gz);
    for (path, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        // `set_path` rejeitaria `..`; grava o nome cru para simular um arquivo malicioso.
        let name = path.as_bytes();
        header.as_old_mut().name[..name.len()].copy_from_slice(name);
        header.set_cksum();
        builder.append(&header, *data).expect("tar");
    }
    builder.into_inner().expect("tar").finish().expect("gz")
}

fn engine_archive() -> Vec<u8> {
    let exe = fs::read(FAKE).expect("ler Engine falsa");
    tar_gz(&[
        (EXE, &exe),
        ("pkg/COPYING", b"GPL"),
        ("pkg/helper.dll", b"dll-de-apoio"),
        ("pkg/../../escape.txt", b"fora"),
        ("pkg/extra.bin", b"nao pedido"),
    ])
}

fn serve(status: u16, body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}/asset")
}

fn spec(url: String, sha: String) -> InstallSpec {
    InstallSpec {
        url,
        sha256: sha,
        kind: ArchiveKind::TarGz,
        executable: EXE.to_owned(),
        keep: vec!["COPYING".into(), "helper.dll".into()],
    }
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

fn install_v1(root: &Path) -> chess_uci::Installed {
    let archive = engine_archive();
    let url = serve(200, archive.clone());
    install(
        &spec(url, sha256(&archive)),
        root,
        "lc0-1",
        &never(),
        &mut |_| {},
    )
    .expect("instalar v1")
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

// --- extração segura ----------------------------------------------------------------

#[test]
fn only_declared_files_are_extracted_and_traversal_entries_are_ignored() {
    let root = scratch("extract");
    let done = install_v1(&root);
    let files = names(&done.dir);
    assert!(files.contains(&"lc0-test".to_owned()), "{files:?}");
    assert!(files.contains(&"COPYING".to_owned()));
    assert!(
        files.contains(&"helper.dll".to_owned()),
        "DLL declarada em keep"
    );
    assert!(
        !files.contains(&"extra.bin".to_owned()),
        "arquivo não pedido"
    );
    assert!(!files.contains(&"escape.txt".to_owned()));
    assert!(!root.parent().expect("pai").join("escape.txt").exists());
    assert!(!root.join("escape.txt").exists());
    // O marcador prova que o app instalou esta pasta.
    assert!(done.dir.join(chess_uci::INSTALL_MARKER).is_file());
    assert!(!done.options.is_empty(), "opções do handshake");
}

// --- remoção segura -----------------------------------------------------------------

#[test]
fn removal_deletes_only_an_app_installed_directory() {
    let root = scratch("remove-ok");
    let done = install_v1(&root);
    remove_installed(&root, &done.dir).expect("remover");
    assert!(!done.dir.exists());
    assert!(root.exists(), "o diretório de engines permanece");
}

#[test]
fn removal_refuses_anything_outside_the_engines_directory() {
    let base = scratch("remove-outside");
    let root = base.join("engines");
    fs::create_dir_all(&root).expect("engines");
    let outside = base.join("minhas-coisas");
    fs::create_dir_all(&outside).expect("fora");
    fs::write(outside.join("importante.txt"), "x").expect("arquivo");
    // Até com marcador, fora de engines/ não se apaga.
    fs::write(outside.join(chess_uci::INSTALL_MARKER), "x").expect("marcador");

    for target in [
        outside.clone(),
        root.join("..").join("minhas-coisas"),
        root.clone(),
        root.join(".."),
        PathBuf::from("/"),
    ] {
        let err = remove_installed(&root, &target).expect_err("deve recusar");
        assert!(
            matches!(err, RemoveError::OutsideEngines | RemoveError::Symlink),
            "{target:?}: {err:?}"
        );
    }
    assert!(outside.join("importante.txt").exists());
}

#[test]
fn removal_refuses_a_symlink_pointing_outside() {
    let base = scratch("remove-link");
    let root = base.join("engines");
    fs::create_dir_all(&root).expect("engines");
    let outside = base.join("alvo");
    fs::create_dir_all(&outside).expect("alvo");
    fs::write(outside.join("dado.txt"), "x").expect("dado");
    fs::write(outside.join(chess_uci::INSTALL_MARKER), "x").expect("marcador");
    std::os::unix::fs::symlink(&outside, root.join("lc0-1")).expect("link");

    let err = remove_installed(&root, &root.join("lc0-1")).expect_err("link");
    assert!(matches!(err, RemoveError::Symlink), "{err:?}");
    assert!(outside.join("dado.txt").exists());
}

#[test]
fn removal_refuses_a_directory_the_app_did_not_install() {
    let root = scratch("remove-foreign");
    let theirs = root.join("stockfish-do-usuario");
    fs::create_dir_all(&theirs).expect("pasta");
    fs::write(theirs.join("stockfish"), "binario").expect("arquivo");
    let err = remove_installed(&root, &theirs).expect_err("sem marcador");
    assert!(matches!(err, RemoveError::NotInstalledByApp), "{err:?}");
    assert!(theirs.join("stockfish").exists());
}

// --- atualização ---------------------------------------------------------------------

#[test]
fn update_installs_the_new_version_then_removes_the_old_one() {
    let root = scratch("update-ok");
    let v1 = install_v1(&root);
    let archive = engine_archive();
    let url = serve(200, archive.clone());
    let done = update(
        &spec(url, sha256(&archive)),
        &root,
        "lc0-2",
        Some(&v1.dir),
        &never(),
        &mut |_| {},
    )
    .expect("atualizar");
    assert_eq!(done.installed.dir, root.join("lc0-2"));
    assert!(done.old_removed);
    assert!(!v1.dir.exists());
    assert!(done.installed.engine.path.is_file());
}

#[test]
fn a_failing_update_keeps_the_previous_version_working() {
    let root = scratch("update-fail");
    let v1 = install_v1(&root);
    let archive = engine_archive();
    // Hash errado: a atualização falha antes de tocar na versão anterior.
    let url = serve(200, archive);
    let err = update(
        &spec(url, "0".repeat(64)),
        &root,
        "lc0-2",
        Some(&v1.dir),
        &never(),
        &mut |_| {},
    )
    .expect_err("deve falhar");
    assert!(matches!(err, InstallError::HashMismatch { .. }), "{err:?}");
    assert_eq!(names(&root), vec!["lc0-1"], "nada de resto, v1 intacta");
    let again = chess_uci::probe_engine(&v1.engine.path, Duration::from_secs(3));
    assert!(again.is_ok(), "a versão anterior continua respondendo");
}

#[test]
fn an_update_whose_engine_fails_the_handshake_keeps_the_previous_version() {
    let root = scratch("update-nouci");
    let v1 = install_v1(&root);
    let archive = tar_gz(&[(EXE, b"#!/bin/sh\nexit 0\n"), ("pkg/COPYING", b"GPL")]);
    let url = serve(200, archive.clone());
    let err = update(
        &spec(url, sha256(&archive)),
        &root,
        "lc0-2",
        Some(&v1.dir),
        &never(),
        &mut |_| {},
    )
    .expect_err("sem UCI");
    assert!(matches!(err, InstallError::Handshake(_)), "{err:?}");
    assert_eq!(names(&root), vec!["lc0-1"]);
}

// --- rede neural -----------------------------------------------------------------------

#[test]
fn network_with_the_right_hash_is_installed_and_wrong_hash_is_rejected() {
    let root = scratch("net");
    let body = b"pesos-da-rede".to_vec();
    let url = serve(200, body.clone());
    let ok = NetworkSpec {
        url,
        sha256: sha256(&body),
        file_name: "rede.pb.gz".into(),
    };
    let done = install_network(&ok, &root, &never(), &mut |_| {}).expect("rede");
    assert_eq!(done.path, root.join("rede.pb.gz"));
    assert_eq!(fs::read(&done.path).expect("ler"), body);

    let bad_url = serve(200, b"adulterada".to_vec());
    let bad = NetworkSpec {
        url: bad_url,
        sha256: sha256(&body),
        file_name: "outra.pb.gz".into(),
    };
    let err = install_network(&bad, &root, &never(), &mut |_| {}).expect_err("hash");
    assert!(matches!(err, InstallError::HashMismatch { .. }), "{err:?}");
    assert_eq!(names(&root), vec!["rede.pb.gz"], "nenhum resto");
}

#[test]
fn network_file_names_cannot_escape_the_directory() {
    let root = scratch("net-name");
    for name in ["../fuga.pb.gz", "a/b.pb.gz", "..", "", ".oculto"] {
        let spec = NetworkSpec {
            url: "http://127.0.0.1:9/x".into(),
            sha256: "0".repeat(64),
            file_name: name.into(),
        };
        let err = install_network(&spec, &root, &never(), &mut |_| {}).expect_err(name);
        assert!(matches!(err, InstallError::Archive(_)), "{name}: {err:?}");
    }
    assert!(names(&root).is_empty());
}

// --- opções UCI ------------------------------------------------------------------------

fn specs(lines: &[&str]) -> Vec<chess_uci::OptionSpec> {
    lines
        .iter()
        .map(|l| {
            let opt = EngineOption::parse_line(l).expect("linha option");
            parse_option(&opt).expect("opção suportada")
        })
        .collect()
}

const LIMITS: Limits = Limits {
    max_threads: 8,
    max_hash_mb: 4096,
};

#[test]
fn announced_options_become_typed_controls() {
    let s = specs(&[
        "option name Threads type spin default 1 min 1 max 1024",
        "option name Hash type spin default 16 min 1 max 33554432",
        "option name UCI_LimitStrength type check default false",
        "option name Style type combo default Normal var Normal var Aggressive var Solid",
        "option name Nickname type string default <empty>",
        "option name Clear Hash type button",
    ]);
    assert_eq!(
        s[0].control,
        Control::Spin {
            default: 1,
            min: 1,
            max: 1024
        }
    );
    assert_eq!(s[2].control, Control::Check { default: false });
    assert_eq!(
        s[3].control,
        Control::Combo {
            default: "Normal".into(),
            vars: vec!["Normal".into(), "Aggressive".into(), "Solid".into()]
        }
    );
    assert_eq!(
        s[4].control,
        Control::Text {
            default: String::new()
        }
    );
    assert_eq!(s[5].control, Control::Button);
    assert_eq!(group_of("Threads"), OptionGroup::Resources);
    assert_eq!(group_of("Hash"), OptionGroup::Resources);
    assert_eq!(group_of("UCI_Elo"), OptionGroup::Level);
    assert_eq!(group_of("Skill Level"), OptionGroup::Level);
    assert_eq!(group_of("Style"), OptionGroup::Advanced);
}

#[test]
fn values_are_validated_against_the_announced_option() {
    let s = specs(&[
        "option name Threads type spin default 1 min 1 max 1024",
        "option name Hash type spin default 16 min 1 max 33554432",
        "option name UCI_LimitStrength type check default false",
        "option name Style type combo default Normal var Normal var Solid",
        "option name Nickname type string default <empty>",
        "option name Debug Log File type string default <empty>",
        "option name SyzygyPath type string default <empty>",
    ]);
    let ok = |n: &str, v: &str| validate_option(&s, &LIMITS, n, v);
    assert_eq!(ok("Threads", "4").expect("ok"), "4");
    assert_eq!(ok("UCI_LimitStrength", "true").expect("ok"), "true");
    assert_eq!(ok("Style", "Solid").expect("ok"), "Solid");
    assert_eq!(ok("Nickname", "Chico").expect("ok"), "Chico");

    // Limites de recurso do app valem mesmo abaixo do máximo anunciado.
    assert!(matches!(
        ok("Threads", "64"),
        Err(SettingError::OutOfRange { .. })
    ));
    assert!(matches!(
        ok("Hash", "100000"),
        Err(SettingError::OutOfRange { .. })
    ));
    assert!(matches!(
        ok("Threads", "0"),
        Err(SettingError::OutOfRange { .. })
    ));
    assert!(matches!(
        ok("Threads", "muitos"),
        Err(SettingError::Invalid { .. })
    ));
    assert!(matches!(
        ok("UCI_LimitStrength", "talvez"),
        Err(SettingError::Invalid { .. })
    ));
    assert!(matches!(
        ok("Style", "Louco"),
        Err(SettingError::Invalid { .. })
    ));
    // Opção não anunciada.
    assert!(matches!(
        ok("Inventada", "1"),
        Err(SettingError::Unknown(_))
    ));
    // Arquivos arbitrários: bloqueados.
    assert!(matches!(
        ok("Debug Log File", "/home/u/.bashrc"),
        Err(SettingError::Restricted(_))
    ));
    assert!(matches!(
        ok("SyzygyPath", "/etc"),
        Err(SettingError::Restricted(_))
    ));
}

#[test]
fn values_cannot_inject_uci_commands() {
    let s = specs(&["option name Nickname type string default <empty>"]);
    for bad in ["x\nquit", "x\rquit", "a\u{0}b", "x\u{7}"] {
        let err = validate_option(&s, &LIMITS, "Nickname", bad).expect_err("injeção");
        assert!(
            matches!(err, SettingError::Invalid { .. }),
            "{bad:?}: {err:?}"
        );
    }
    // O nome também: `Nickname\nquit` não é opção anunciada.
    assert!(validate_option(&s, &LIMITS, "Nickname\nquit", "1").is_err());
}

#[test]
fn settings_resolve_to_ordered_setoption_pairs_and_reject_the_bad_ones() {
    let s = specs(&[
        "option name Threads type spin default 1 min 1 max 1024",
        "option name Hash type spin default 16 min 1 max 33554432",
    ]);
    let mut map = BTreeMap::new();
    map.insert("Hash".to_owned(), "256".to_owned());
    map.insert("Threads".to_owned(), "4".to_owned());
    let pairs = resolve_settings(&s, &LIMITS, &map).expect("ok");
    assert_eq!(
        pairs,
        vec![
            ("Threads".to_owned(), "4".to_owned()),
            ("Hash".to_owned(), "256".to_owned())
        ],
        "Threads antes de Hash"
    );
    map.insert("Threads".to_owned(), "99".to_owned());
    assert!(resolve_settings(&s, &LIMITS, &map).is_err());
}

#[test]
fn configured_options_reach_the_engine_and_hostile_ones_never_do() {
    let dir = scratch("setoption");
    let trace = dir.join("trace.txt");
    let config = |opts: &[(&str, &str)]| {
        let mut c = EngineConfig::new(FAKE)
            .arg("--trace")
            .arg(trace.to_str().expect("utf8"));
        for (n, v) in opts {
            c = c.option(*n, *v);
        }
        c
    };
    let engine = Engine::spawn(config(&[("Skill Level", "7")])).expect("spawn");
    engine.quit();
    let text = fs::read_to_string(&trace).expect("trace");
    assert!(text.contains("setoption name Skill Level value 7"), "{text}");

    let _ = fs::remove_file(&trace);
    let err = Engine::spawn(config(&[("Skill Level", "7\nquit")]));
    assert!(
        err.is_err(),
        "valor com quebra de linha é recusado ao iniciar"
    );
    let text = fs::read_to_string(&trace).unwrap_or_default();
    assert!(!text.contains("setoption name Skill Level value 7"), "{text}");
}
