//! Instalação fixada (ADR-0003) contra um servidor HTTP local e a Engine falsa: hash certo
//! instala e valida; hash errado, erro HTTP, rede fora e arquivo corrompido são rejeitados
//! sem deixar nada no disco. O teste com a rede real fica `#[ignore]`.
#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::thread;

use chess_uci::{ArchiveKind, InstallError, InstallSpec, Progress, install};
use sha2::{Digest, Sha256};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");
const EXE_IN_ARCHIVE: &str = "stockfish/stockfish-test-universal";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("install-{name}"));
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

fn fake_bytes() -> Vec<u8> {
    fs::read(FAKE).expect("ler Engine falsa")
}

/// tar.gz com o layout do Stockfish: `stockfish/<exe>`, `stockfish/Copying.txt` e um
/// arquivo de código-fonte que não deve ser extraído.
fn tar_gz(exe: &[u8]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(gz);
    for (path, data) in [
        (EXE_IN_ARCHIVE, exe),
        ("stockfish/Copying.txt", b"GPL" as &[u8]),
        ("stockfish/src/main.cpp", b"int main(){}"),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, data).expect("tar");
    }
    builder.into_inner().expect("tar").finish().expect("gz")
}

fn zip_file(exe: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(EXE_IN_ARCHIVE, options).expect("zip");
    zip.write_all(exe).expect("zip");
    zip.start_file("stockfish/AUTHORS", options).expect("zip");
    zip.write_all(b"autores").expect("zip");
    zip.finish().expect("zip").into_inner()
}

/// Servidor HTTP mínimo: responde `status` e `body` a cada conexão (o suficiente para `ureq`).
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

fn spec(url: String, sha256: String, kind: ArchiveKind) -> InstallSpec {
    InstallSpec {
        url,
        sha256,
        kind,
        executable: EXE_IN_ARCHIVE.to_owned(),
        keep: vec!["Copying.txt".to_owned(), "AUTHORS".to_owned()],
    }
}

fn run(spec: &InstallSpec, root: &Path) -> Result<chess_uci::Installed, InstallError> {
    install(
        spec,
        root,
        "stockfish-test",
        &AtomicBool::new(false),
        &mut |_| {},
    )
}

/// Nomes na pasta, sem o marcador de procedência que o app grava.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != chess_uci::INSTALL_MARKER)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn correct_hash_installs_validates_and_keeps_only_the_needed_files() {
    let root = scratch("ok");
    let archive = tar_gz(&fake_bytes());
    let url = serve(200, archive.clone());
    let spec = spec(url, sha256(&archive), ArchiveKind::TarGz);

    let mut steps = Vec::new();
    let done = install(
        &spec,
        &root,
        "stockfish-test",
        &AtomicBool::new(false),
        &mut |p| steps.push(p),
    )
    .expect("instalação");

    assert_eq!(done.dir, root.join("stockfish-test"));
    assert_eq!(done.engine.path, done.dir.join("stockfish-test-universal"));
    assert_eq!(done.engine.name.as_deref(), Some("FakeUci 1.0"));
    assert_eq!(done.sha256, sha256(&archive));
    assert_eq!(
        names(&done.dir),
        vec!["Copying.txt", "stockfish-test-universal"]
    );
    // Sem restos: nenhum diretório temporário ao lado.
    assert_eq!(names(&root), vec!["stockfish-test"]);
    // O processo passou por todas as etapas, em ordem.
    assert!(matches!(steps.first(), Some(Progress::Downloading { .. })));
    let tail: Vec<_> = steps
        .iter()
        .filter(|s| !matches!(s, Progress::Downloading { .. }))
        .collect();
    assert_eq!(
        tail,
        [
            &Progress::Verifying,
            &Progress::Extracting,
            &Progress::Validating
        ]
    );
}

#[test]
fn zip_archives_install_too() {
    let root = scratch("zip");
    let archive = zip_file(&fake_bytes());
    let url = serve(200, archive.clone());
    let done = run(&spec(url, sha256(&archive), ArchiveKind::Zip), &root).expect("instalação");
    assert_eq!(
        names(&done.dir),
        vec!["AUTHORS", "stockfish-test-universal"]
    );
}

#[test]
fn tampered_download_is_rejected_and_deleted() {
    let root = scratch("tampered");
    let genuine = tar_gz(&fake_bytes());
    let expected = sha256(&genuine);
    let mut tampered = genuine;
    let middle = tampered.len() / 2;
    tampered[middle] ^= 0xff;
    let url = serve(200, tampered.clone());

    let err = run(&spec(url, expected.clone(), ArchiveKind::TarGz), &root).expect_err("rejeitar");
    match err {
        InstallError::HashMismatch {
            expected: e,
            actual,
        } => {
            assert_eq!(e, expected);
            assert_eq!(actual, sha256(&tampered));
        }
        other => panic!("esperava HashMismatch, veio {other:?}"),
    }
    assert!(names(&root).is_empty(), "restos: {:?}", names(&root));
}

#[test]
fn a_failed_reinstall_keeps_the_previous_good_install() {
    let root = scratch("keep-old");
    let archive = tar_gz(&fake_bytes());
    let url = serve(200, archive.clone());
    run(&spec(url, sha256(&archive), ArchiveKind::TarGz), &root).expect("primeira");

    let bad_url = serve(200, b"lixo".to_vec());
    let err = run(&spec(bad_url, sha256(&archive), ArchiveKind::TarGz), &root);
    assert!(matches!(err, Err(InstallError::HashMismatch { .. })));
    assert_eq!(
        names(&root.join("stockfish-test")),
        vec!["Copying.txt", "stockfish-test-universal"]
    );
}

#[test]
fn http_error_status_is_reported_with_its_code() {
    let root = scratch("http");
    let url = serve(404, b"nada".to_vec());
    let err = run(&spec(url, "0".repeat(64), ArchiveKind::TarGz), &root).expect_err("404");
    assert!(matches!(err, InstallError::Http(404)), "{err:?}");
    assert!(names(&root).is_empty());
}

#[test]
fn unreachable_server_is_a_network_error() {
    let root = scratch("offline");
    // Porta que acabou de ser liberada: ninguém escuta.
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let url = format!("http://127.0.0.1:{port}/asset");
    let err = run(&spec(url, "0".repeat(64), ArchiveKind::TarGz), &root).expect_err("offline");
    assert!(matches!(err, InstallError::Network(_)), "{err:?}");
    assert!(names(&root).is_empty());
}

#[test]
fn archive_with_matching_hash_but_not_an_archive_is_rejected() {
    let root = scratch("not-archive");
    let body = b"isto nao e um tar.gz".to_vec();
    let url = serve(200, body.clone());
    let err = run(&spec(url, sha256(&body), ArchiveKind::TarGz), &root).expect_err("inválido");
    assert!(matches!(err, InstallError::Archive(_)), "{err:?}");
    assert!(names(&root).is_empty());
}

#[test]
fn executable_that_does_not_speak_uci_is_rejected_and_removed() {
    let root = scratch("no-uci");
    let archive = tar_gz(b"#!/bin/sh\nexit 0\n");
    let url = serve(200, archive.clone());
    let err = run(&spec(url, sha256(&archive), ArchiveKind::TarGz), &root).expect_err("sem UCI");
    assert!(matches!(err, InstallError::Handshake(_)), "{err:?}");
    assert!(names(&root).is_empty());
}

#[test]
fn cancelling_stops_and_cleans_up() {
    let root = scratch("cancel");
    let archive = tar_gz(&fake_bytes());
    let url = serve(200, archive.clone());
    let cancel = AtomicBool::new(true);
    let err = install(
        &spec(url, sha256(&archive), ArchiveKind::TarGz),
        &root,
        "stockfish-test",
        &cancel,
        &mut |_| {},
    )
    .expect_err("cancelado");
    assert!(matches!(err, InstallError::Cancelled), "{err:?}");
    assert!(names(&root).is_empty());
}

/// Baixa o asset Linux x86-64 real do Stockfish 19 (cerca de 81 MB) para um diretório
/// temporário e o valida. Só roda com `--ignored` e rede.
#[test]
#[ignore = "toca a rede real: baixa ~81 MB do site oficial do Stockfish"]
fn real_stockfish_19_linux_download() {
    let asset = chess_uci::asset_for("linux", "x86_64").expect("asset Linux");
    let root = scratch("real");
    let spec = InstallSpec::from(asset);
    let done = run(&spec, &root).expect("instalação real");
    assert_eq!(done.sha256, asset.sha256);
    let name = done.engine.name.as_deref().unwrap_or_default();
    assert!(name.starts_with("Stockfish"), "nome: {name}");
    assert!(
        done.dir.join("Copying.txt").is_file(),
        "licença GPL ao lado do binário"
    );
    eprintln!("instalado em {} ({name})", done.engine.path.display());
}
