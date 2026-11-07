//! Detecção de Engines UCI já instaladas (DEC-27): procura executáveis com nome de Engine
//! conhecida no `PATH`, em locais comuns por SO e no diretório de dados do app, e confirma
//! cada candidato com o handshake UCI (prazo curto). Nunca executa arquivos de nome
//! desconhecido: varrer `/usr/bin` inteiro rodando tudo seria perigoso.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{Engine, EngineConfig, EngineError, EngineOption, Timeouts};

/// Prefixos (em minúsculas) de nomes de executáveis de Engines UCI conhecidas.
const KNOWN_ENGINES: &[&str] = &[
    "stockfish",
    "fairy-stockfish",
    "lc0",
    "komodo",
    "berserk",
    "ethereal",
    "igel",
    "rubichess",
    "arasan",
    "texel",
    "koivisto",
    "obsidian",
    "alexandria",
    "caissa",
];

/// Prazo do handshake ao validar candidatas.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Engine UCI confirmada pelo handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedEngine {
    pub path: PathBuf,
    pub name: Option<String>,
    pub author: Option<String>,
}

impl DetectedEngine {
    /// Nome para mostrar: o anunciado no handshake ou o nome do arquivo.
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            self.path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    }
}

/// Ambiente consultado para montar a lista de pastas (injetável nos testes).
#[derive(Debug, Clone, Default)]
pub struct SearchEnv {
    /// `linux`, `macos` ou `windows` (valores de `std::env::consts::OS`).
    pub os: String,
    pub path_var: Option<OsString>,
    pub home: Option<PathBuf>,
    /// Windows: `%ProgramFiles%`, `%ProgramFiles(x86)%` e `%LOCALAPPDATA%`.
    pub program_files: Vec<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    /// Diretório de dados do app (`<dados>`), cujo `engines/` também é varrido.
    pub data_dir: Option<PathBuf>,
}

impl SearchEnv {
    /// O ambiente real do processo.
    pub fn from_host() -> Self {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        Self {
            os: std::env::consts::OS.to_owned(),
            path_var: std::env::var_os("PATH"),
            home: var("HOME").or_else(|| var("USERPROFILE")),
            program_files: ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(|n| var(n))
                .collect(),
            local_app_data: var("LOCALAPPDATA"),
            data_dir: app_data_dir(),
        }
    }
}

/// Pastas onde procurar, sem repetição e na ordem: `PATH`, locais comuns do SO, dados do app.
pub fn search_dirs(env: &SearchEnv) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(path) = &env.path_var {
        dirs.extend(std::env::split_paths(path));
    }
    let home = |rel: &str| env.home.as_ref().map(|h| h.join(rel));
    match env.os.as_str() {
        "linux" => {
            for fixed in [
                "/usr/bin",
                "/usr/games",
                "/usr/local/bin",
                "/usr/local/games",
                "/snap/bin",
                "/var/lib/flatpak/exports/bin",
            ] {
                dirs.push(PathBuf::from(fixed));
            }
            dirs.extend(home(".local/bin"));
            dirs.extend(home(".local/share/flatpak/exports/bin"));
        }
        "macos" => {
            dirs.push(PathBuf::from("/opt/homebrew/bin"));
            dirs.push(PathBuf::from("/usr/local/bin"));
            dirs.extend(home(".local/bin"));
        }
        "windows" => {
            let mut roots: Vec<PathBuf> = env.program_files.clone();
            roots.extend(env.local_app_data.as_ref().map(|p| p.join("Programs")));
            for root in roots {
                dirs.push(root.clone());
                // Um nível abaixo, só pastas com cara de Engine (ex.: `Program Files\Stockfish`).
                if let Ok(read) = std::fs::read_dir(&root) {
                    for entry in read.flatten() {
                        let name = entry.file_name().to_string_lossy().to_lowercase();
                        if KNOWN_ENGINES.iter().any(|k| name.contains(k)) && entry.path().is_dir() {
                            dirs.push(entry.path());
                        }
                    }
                }
            }
        }
        _ => {}
    }
    if let Some(data) = &env.data_dir {
        let engines = data.join("engines");
        dirs.push(engines.clone());
        if let Ok(read) = std::fs::read_dir(&engines) {
            let mut sub: Vec<PathBuf> = read
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            sub.sort();
            dirs.extend(sub);
        }
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// O nome do arquivo parece o de uma Engine conhecida (sem extensão, ou `.exe`).
fn looks_like_engine(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    let stem = match lower.strip_suffix(".exe") {
        Some(stem) => stem,
        None if lower.contains('.') => return false,
        None => &lower,
    };
    KNOWN_ENGINES.iter().any(|known| stem.starts_with(known))
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    true
}

/// Arquivos executáveis de nome conhecido diretamente em `dir`.
pub fn engine_files_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = read
        .flatten()
        .filter(|e| looks_like_engine(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        // `metadata` segue links simbólicos: `/usr/games/stockfish` costuma ser um.
        .filter(|p| {
            std::fs::metadata(p)
                .map(|m| m.is_file() && is_executable(&m))
                .unwrap_or(false)
        })
        .collect();
    found.sort();
    found
}

/// Inicia o executável, faz o handshake UCI com `timeout` e o encerra.
pub fn probe_engine(path: &Path, timeout: Duration) -> Result<DetectedEngine, EngineError> {
    probe_full(path, timeout).map(|(engine, _)| engine)
}

/// Como [`probe_engine`], devolvendo também as opções anunciadas (para o formulário de
/// configuração do gerenciador).
pub fn probe_full(
    path: &Path,
    timeout: Duration,
) -> Result<(DetectedEngine, Vec<EngineOption>), EngineError> {
    let timeouts = Timeouts {
        handshake: timeout,
        ready: timeout,
        quit: Duration::from_millis(500),
        ..Timeouts::default()
    };
    let config = EngineConfig::new(path).timeouts(timeouts).log_capacity(0);
    probe_config_full(config)
}

/// Como [`probe_engine`], com a configuração completa (argumentos extras, nos testes).
pub fn probe_config(config: EngineConfig) -> Result<DetectedEngine, EngineError> {
    probe_config_full(config).map(|(engine, _)| engine)
}

fn probe_config_full(
    config: EngineConfig,
) -> Result<(DetectedEngine, Vec<EngineOption>), EngineError> {
    let path = config.program.clone();
    let engine = spawn_retrying_busy(&config)?;
    let info = engine.info().clone();
    engine.quit();
    Ok((
        DetectedEngine {
            path,
            name: info.name,
            author: info.author,
        },
        info.options,
    ))
}

/// `Engine::spawn` que tolera "arquivo de texto ocupado" (ETXTBSY): um executável recém-gravado
/// pode continuar aberto para escrita num `fork` concorrente por instantes; tentar de novo resolve.
fn spawn_retrying_busy(config: &EngineConfig) -> Result<Engine, EngineError> {
    let mut attempts = 0;
    loop {
        match Engine::spawn(config.clone()) {
            Err(EngineError::Spawn { source, .. })
                if source.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 20 =>
            {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
            other => return other,
        }
    }
}

/// Valida as candidatas das `dirs`; a mesma Engine alcançada por links diferentes aparece
/// uma vez. Bloqueia até `timeout` por candidata que não responde: chame de uma thread.
pub fn detect_in(dirs: &[PathBuf], timeout: Duration) -> Vec<DetectedEngine> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        for file in engine_files_in(dir) {
            let key = std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
            if !seen.insert(key) {
                continue;
            }
            if let Ok(found) = probe_engine(&file, timeout) {
                out.push(found);
            }
        }
    }
    out
}

/// Detecção sobre o computador real.
pub fn detect_host() -> Vec<DetectedEngine> {
    detect_in(&search_dirs(&SearchEnv::from_host()), PROBE_TIMEOUT)
}

/// Diretório de dados do app (`<dados>`): `XDG_DATA_HOME` ou `~/.local/share` no Linux,
/// `~/Library/Application Support` no macOS, `%APPDATA%` no Windows, sempre `/ChessStudio` (a mesma pasta do banco, `paths.rs` do app).
pub fn app_data_dir() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = match std::env::consts::OS {
        "windows" => var("APPDATA")?,
        "macos" => var("HOME")?.join("Library/Application Support"),
        _ => var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share")))?,
    };
    Some(base.join("ChessStudio"))
}

/// `<dados>/engines`, onde o download instala as Engines.
pub fn engines_dir() -> Option<PathBuf> {
    app_data_dir().map(|d| d.join("engines"))
}

const PREFERRED_FILE: &str = "preferred.txt";

/// Grava o executável escolhido em `<engines>/preferred.txt` (preferência do usuário).
pub fn save_preferred(engines_dir: &Path, engine: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(engines_dir)?;
    std::fs::write(
        engines_dir.join(PREFERRED_FILE),
        engine.to_string_lossy().as_bytes(),
    )
}

/// Executável escolhido antes, se o arquivo ainda existir.
pub fn load_preferred(engines_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(engines_dir.join(PREFERRED_FILE)).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_names_are_recognised_conservatively() {
        assert!(looks_like_engine("stockfish"));
        assert!(looks_like_engine("stockfish-linux-x86-64-universal"));
        assert!(looks_like_engine("Stockfish.exe"));
        assert!(looks_like_engine("lc0"));
        assert!(!looks_like_engine("stockfish.tar.gz"));
        assert!(!looks_like_engine("Copying.txt"));
        assert!(!looks_like_engine("bash"));
    }

    #[test]
    fn linux_dirs_cover_common_locations_without_duplicates() {
        let env = SearchEnv {
            os: "linux".into(),
            path_var: Some(OsString::from("/usr/bin:/opt/x")),
            home: Some(PathBuf::from("/home/u")),
            ..SearchEnv::default()
        };
        let dirs = search_dirs(&env);
        for want in [
            "/usr/bin",
            "/opt/x",
            "/usr/games",
            "/usr/local/bin",
            "/snap/bin",
            "/var/lib/flatpak/exports/bin",
            "/home/u/.local/bin",
        ] {
            assert!(dirs.contains(&PathBuf::from(want)), "falta {want}");
        }
        assert_eq!(
            dirs.iter().filter(|d| **d == Path::new("/usr/bin")).count(),
            1
        );
    }

    #[test]
    fn macos_dirs_include_homebrew() {
        let env = SearchEnv {
            os: "macos".into(),
            ..SearchEnv::default()
        };
        let dirs = search_dirs(&env);
        assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
    }
}
