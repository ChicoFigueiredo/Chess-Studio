//! Diretórios do usuário (dados e documentos) sem dependências externas: a crate
//! `directories` puxa `option-ext` (MPL-2.0), fora da allowlist do ADR-0004. A resolução é
//! pura (recebe o ambiente) para ser testável.

use std::path::PathBuf;

/// Sistemas com convenções de caminho diferentes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    MacOs,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

const APP_DIR: &str = "ChessStudio";
const DB_FILE: &str = "chess-studio.db";

fn non_empty(env: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<PathBuf> {
    env(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Diretório de dados do aplicativo (`~/.local/share/ChessStudio`, `%APPDATA%\ChessStudio`,
/// `~/Library/Application Support/ChessStudio`).
pub fn data_dir(platform: Platform, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let base = match platform {
        Platform::Linux => non_empty(env, "XDG_DATA_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| non_empty(env, "HOME").map(|h| h.join(".local").join("share"))),
        Platform::Windows => non_empty(env, "APPDATA")
            .or_else(|| non_empty(env, "USERPROFILE").map(|h| h.join("AppData").join("Roaming"))),
        Platform::MacOs => {
            non_empty(env, "HOME").map(|h| h.join("Library").join("Application Support"))
        }
    }?;
    Some(base.join(APP_DIR))
}

/// Arquivo do banco SQLite do usuário.
pub fn database_path(platform: Platform, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    data_dir(platform, env).map(|d| d.join(DB_FILE))
}

/// Pasta de documentos (onde o PGN exportado é gravado). No Linux respeita
/// `XDG_DOCUMENTS_DIR`; senão `~/Documents`; sem `HOME`, o diretório atual.
pub fn documents_dir(platform: Platform, env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let home = match platform {
        Platform::Windows => non_empty(env, "USERPROFILE"),
        _ => non_empty(env, "HOME"),
    };
    if platform == Platform::Linux
        && let Some(dir) = non_empty(env, "XDG_DOCUMENTS_DIR").filter(|p| p.is_absolute())
    {
        return dir;
    }
    home.map_or_else(|| PathBuf::from("."), |h| h.join("Documents"))
}

pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn linux_uses_xdg_data_home_then_home() {
        let e = env(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/ana")]);
        assert_eq!(
            database_path(Platform::Linux, &e),
            Some(PathBuf::from("/data/ChessStudio/chess-studio.db"))
        );
        let e = env(&[("HOME", "/home/ana")]);
        assert_eq!(
            data_dir(Platform::Linux, &e),
            Some(PathBuf::from("/home/ana/.local/share/ChessStudio"))
        );
        // XDG relativo é inválido pela especificação.
        let e = env(&[("XDG_DATA_HOME", "rel"), ("HOME", "/home/ana")]);
        assert_eq!(
            data_dir(Platform::Linux, &e),
            Some(PathBuf::from("/home/ana/.local/share/ChessStudio"))
        );
    }

    #[test]
    fn windows_and_macos_conventions() {
        let e = env(&[("APPDATA", "C:\\Users\\ana\\AppData\\Roaming")]);
        assert_eq!(
            data_dir(Platform::Windows, &e),
            Some(PathBuf::from("C:\\Users\\ana\\AppData\\Roaming").join("ChessStudio"))
        );
        let e = env(&[("HOME", "/Users/ana")]);
        assert_eq!(
            data_dir(Platform::MacOs, &e),
            Some(PathBuf::from(
                "/Users/ana/Library/Application Support/ChessStudio"
            ))
        );
    }

    #[test]
    fn missing_environment_gives_none_or_fallback() {
        let e = env(&[]);
        assert_eq!(data_dir(Platform::Linux, &e), None);
        assert_eq!(documents_dir(Platform::Linux, &e), PathBuf::from("."));
    }

    #[test]
    fn documents_prefers_xdg_then_home_documents() {
        let e = env(&[("XDG_DOCUMENTS_DIR", "/docs"), ("HOME", "/home/ana")]);
        assert_eq!(documents_dir(Platform::Linux, &e), PathBuf::from("/docs"));
        let e = env(&[("HOME", "/home/ana")]);
        assert_eq!(
            documents_dir(Platform::Linux, &e),
            PathBuf::from("/home/ana/Documents")
        );
    }
}
