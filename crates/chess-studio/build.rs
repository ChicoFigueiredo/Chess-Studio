//! Embute o commit git (curto) no binário para o relatório do benchmark e, no Windows, o ícone
//! e o nome do app no `.exe` (atalhos do instalador e Gerenciador de Tarefas usam os do executável).

use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "desconhecido".to_owned(), |s| s.trim().to_owned());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .is_some_and(|o| !o.stdout.is_empty());
    println!(
        "cargo:rustc-env=CHESS_STUDIO_GIT_COMMIT={commit}{}",
        if dirty { "-dirty" } else { "" }
    );
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
}

/// Ícone e metadados do `.exe` (recurso Win32). Falhar aqui é falhar o build: sem ícone, os
/// atalhos do Menu Iniciar mostrariam o ícone genérico.
fn windows_resources() {
    println!("cargo:rerun-if-changed=icons/icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icons/icon.ico")
        .set("FileDescription", "Chess Studio")
        .set("ProductName", "Chess Studio");
    if let Err(e) = res.compile() {
        panic!("não foi possível embutir o ícone no .exe: {e}");
    }
}
