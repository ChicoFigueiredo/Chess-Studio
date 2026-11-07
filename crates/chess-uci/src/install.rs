//! Instalação fixada de uma Engine (ADR-0003): baixa, confere o SHA-256 embutido, extrai
//! só o que precisa, marca como executável e valida com o handshake UCI. Qualquer falha
//! apaga o que foi baixado: um arquivo adulterado nunca chega a ser extraído.
//!
//! Bloqueia a thread que chama; o app a executa numa thread própria. A rede só é tocada
//! aqui, e só quando o usuário confirma o download.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::EngineError;
use crate::catalog::{ArchiveKind, Asset};
use crate::detect::{DetectedEngine, probe_full};
use crate::protocol::EngineOption;

/// Marcador gravado na pasta de toda Engine instalada pelo app: só pastas com ele podem ser
/// apagadas pelo gerenciador (PEND-08, remoção segura).
pub const INSTALL_MARKER: &str = ".chess-studio-install";
/// Teto de cada arquivo extraído e do total (defesa contra "zip bomb").
const MAX_ENTRY: u64 = 1 << 30;
const MAX_EXTRACTED: u64 = 2 << 30;
/// Prazo do handshake de validação (o binário universal tem ~100 MB).
const VALIDATE_TIMEOUT: Duration = Duration::from_secs(15);
/// Teto de segurança do corpo baixado, além do tamanho esperado.
const MAX_DOWNLOAD: u64 = 1 << 30;

/// O que baixar e como conferir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSpec {
    pub url: String,
    /// SHA-256 esperado, em hexadecimal.
    pub sha256: String,
    pub kind: ArchiveKind,
    /// Caminho do executável dentro do arquivo.
    pub executable: String,
    /// Outros arquivos a extrair, por nome-base, da mesma pasta do executável (licenças e
    /// bibliotecas). Qualquer outra entrada do arquivo é ignorada.
    pub keep: Vec<String>,
}

impl From<&Asset> for InstallSpec {
    fn from(asset: &Asset) -> Self {
        Self {
            url: asset.url(),
            sha256: asset.sha256.to_owned(),
            kind: asset.kind,
            executable: asset.executable.to_owned(),
            keep: asset.keep.iter().map(|k| (*k).to_owned()).collect(),
        }
    }
}

/// Etapas do andamento, para a barra de progresso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Downloading { done: u64, total: Option<u64> },
    Verifying,
    Extracting,
    Validating,
}

/// Por que a instalação falhou.
#[derive(Debug)]
pub enum InstallError {
    /// Sem rede, DNS, conexão recusada, TLS…
    Network(String),
    /// O servidor respondeu com status de erro.
    Http(u16),
    /// O arquivo baixado não tem o SHA-256 esperado; foi apagado.
    HashMismatch { expected: String, actual: String },
    /// Falha de disco (sem espaço, sem permissão…).
    Io { path: PathBuf, source: io::Error },
    /// O arquivo não é um tar.gz/zip válido ou falta o executável nele.
    Archive(String),
    /// O executável extraído não completou o handshake UCI.
    Handshake(EngineError),
    /// O usuário cancelou.
    Cancelled,
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "falha de rede: {msg}"),
            Self::Http(code) => write!(f, "o servidor respondeu HTTP {code}"),
            Self::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "SHA-256 diferente do esperado ({expected}); obtido {actual}"
                )
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Archive(msg) => write!(f, "arquivo inválido: {msg}"),
            Self::Handshake(err) => write!(f, "a Engine não respondeu ao UCI: {err}"),
            Self::Cancelled => write!(f, "cancelado"),
        }
    }
}

impl std::error::Error for InstallError {}

/// Resultado de uma instalação bem-sucedida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub dir: PathBuf,
    pub engine: DetectedEngine,
    /// Opções anunciadas no handshake de validação (alimentam o formulário de configuração).
    pub options: Vec<EngineOption>,
    /// SHA-256 verificado do arquivo baixado.
    pub sha256: String,
}

/// Apaga o diretório temporário se a instalação não chegar ao fim.
struct PartGuard {
    path: PathBuf,
}

impl PartGuard {
    fn file(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        // Diretório de staging ou arquivo parcial: o que sobrar some.
        let _ = fs::remove_dir_all(&self.path);
        let _ = fs::remove_file(&self.path);
    }
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> InstallError + '_ {
    move |source| InstallError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Instala em `<engines_dir>/<dir_name>/`. `cancel` é consultado a cada bloco baixado.
pub fn install(
    spec: &InstallSpec,
    engines_dir: &Path,
    dir_name: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Installed, InstallError> {
    fs::create_dir_all(engines_dir).map_err(io_err(engines_dir))?;
    let part = engines_dir.join(format!(".{dir_name}.part"));
    let _ = fs::remove_dir_all(&part);
    fs::create_dir_all(&part).map_err(io_err(&part))?;
    let _guard = PartGuard::file(part.clone());

    let archive_path = part.join("download.bin");
    let actual = download(spec, &archive_path, cancel, progress)?;

    progress(Progress::Verifying);
    if !actual.eq_ignore_ascii_case(&spec.sha256) {
        // `_guard` apaga o diretório temporário, inclusive o arquivo adulterado.
        return Err(InstallError::HashMismatch {
            expected: spec.sha256.to_lowercase(),
            actual,
        });
    }

    progress(Progress::Extracting);
    let extract_dir = part.join("out");
    fs::create_dir_all(&extract_dir).map_err(io_err(&extract_dir))?;
    let exe_name = extract(spec, &archive_path, &extract_dir)?;
    fs::remove_file(&archive_path).map_err(io_err(&archive_path))?;

    progress(Progress::Validating);
    let staged_exe = extract_dir.join(&exe_name);
    let (mut engine, options) =
        probe_full(&staged_exe, VALIDATE_TIMEOUT).map_err(InstallError::Handshake)?;
    write_marker(&extract_dir, spec, &actual)?;

    let final_dir = engines_dir.join(dir_name);
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir).map_err(io_err(&final_dir))?;
    }
    fs::rename(&extract_dir, &final_dir).map_err(io_err(&final_dir))?;
    // O resto de `part` (agora vazio) é removido pelo guard ao sair.
    engine.path = final_dir.join(&exe_name);
    Ok(Installed {
        dir: final_dir,
        engine,
        options,
        sha256: actual,
    })
}

/// Grava o marcador de procedência na pasta instalada.
fn write_marker(dir: &Path, spec: &InstallSpec, sha256: &str) -> Result<(), InstallError> {
    let path = dir.join(INSTALL_MARKER);
    let text = format!(
        "chess-studio-install 1\nurl={}\nsha256={sha256}\n",
        spec.url
    );
    fs::write(&path, text).map_err(io_err(&path))
}

/// Rede neural (ou outro arquivo único) a baixar: URL, hash esperado e nome do arquivo final.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSpec {
    pub url: String,
    pub sha256: String,
    /// Nome-base do arquivo em `networks_dir`; qualquer separador ou `..` é recusado.
    pub file_name: String,
}

/// Arquivo único instalado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNetwork {
    pub path: PathBuf,
    pub sha256: String,
}

/// Baixa um arquivo único para `<networks_dir>/<file_name>`, conferindo o SHA-256 antes de
/// o colocar no lugar. Falha deixa o diretório como estava.
pub fn install_network(
    spec: &NetworkSpec,
    networks_dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledNetwork, InstallError> {
    let name = spec.file_name.as_str();
    if name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', '\\', '\0'])
        || name.contains("..")
    {
        return Err(InstallError::Archive(format!(
            "nome de arquivo inválido: {name:?}"
        )));
    }
    fs::create_dir_all(networks_dir).map_err(io_err(networks_dir))?;
    let part = networks_dir.join(format!(".{name}.part"));
    let _guard = PartGuard::file(part.clone());
    let inner = InstallSpec {
        url: spec.url.clone(),
        sha256: spec.sha256.clone(),
        kind: ArchiveKind::TarGz,
        executable: String::new(),
        keep: Vec::new(),
    };
    let actual = download(&inner, &part, cancel, progress)?;
    progress(Progress::Verifying);
    if !actual.eq_ignore_ascii_case(&spec.sha256) {
        return Err(InstallError::HashMismatch {
            expected: spec.sha256.to_lowercase(),
            actual,
        });
    }
    let final_path = networks_dir.join(name);
    fs::rename(&part, &final_path).map_err(io_err(&final_path))?;
    Ok(InstalledNetwork {
        path: final_path,
        sha256: actual,
    })
}

/// Baixa para `dest` calculando o SHA-256 no caminho; devolve o hash em hexadecimal.
fn download(
    spec: &InstallSpec,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<String, InstallError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        // Um download que começa em HTTPS nunca é rebaixado para HTTP por redirecionamento.
        .https_only(spec.url.starts_with("https://"))
        .max_redirects(5)
        .build()
        .into();
    let mut response = agent.get(&spec.url).call().map_err(|e| match e {
        ureq::Error::StatusCode(code) => InstallError::Http(code),
        other => InstallError::Network(other.to_string()),
    })?;
    let total = response.body().content_length();
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .reader();

    let mut file = File::create(dest).map_err(io_err(dest))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_reported = 0u64;
    progress(Progress::Downloading { done, total });
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(InstallError::Cancelled);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| InstallError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(io_err(dest))?;
        done += n as u64;
        if done - last_reported >= 256 * 1024 {
            last_reported = done;
            progress(Progress::Downloading { done, total });
        }
    }
    file.flush().map_err(io_err(dest))?;
    progress(Progress::Downloading { done, total });
    Ok(hex(&hasher.finalize()))
}

/// Extrai o executável e os arquivos de licença para `out`, sem usar os caminhos do
/// arquivo (nada de `../`): só entradas de nome exato são lidas e gravadas por nome base.
/// Devolve o nome do executável.
fn extract(spec: &InstallSpec, archive: &Path, out: &Path) -> Result<String, InstallError> {
    let exe_name = spec
        .executable
        .rsplit('/')
        .next()
        .unwrap_or(&spec.executable)
        .to_owned();
    let prefix = spec
        .executable
        .strip_suffix(exe_name.as_str())
        .unwrap_or_default()
        .to_owned();
    let wanted = |entry: &str| -> Option<String> {
        if entry == spec.executable {
            return Some(exe_name.clone());
        }
        spec.keep
            .iter()
            .find(|keep| entry == format!("{prefix}{keep}"))
            .cloned()
    };

    let mut found_exe = false;
    let mut total = 0u64;
    let mut write = |entry: &str, reader: &mut dyn Read| -> Result<(), InstallError> {
        let Some(name) = wanted(entry) else {
            return Ok(());
        };
        let target = out.join(&name);
        let mut file = File::create(&target).map_err(io_err(&target))?;
        // Teto por arquivo e no total: um arquivo que se expande demais é rejeitado.
        let copied = io::copy(&mut reader.take(MAX_ENTRY + 1), &mut file).map_err(io_err(&target))?;
        total += copied;
        if copied > MAX_ENTRY || total > MAX_EXTRACTED {
            return Err(InstallError::Archive(format!(
                "{name} excede o tamanho máximo permitido"
            )));
        }
        if name == exe_name {
            make_executable(&target)?;
            found_exe = true;
        }
        Ok(())
    };

    match spec.kind {
        ArchiveKind::TarGz => {
            let file = File::open(archive).map_err(io_err(archive))?;
            let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
            let entries = tar
                .entries()
                .map_err(|e| InstallError::Archive(e.to_string()))?;
            for entry in entries {
                let mut entry = entry.map_err(|e| InstallError::Archive(e.to_string()))?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let path = entry
                    .path()
                    .map_err(|e| InstallError::Archive(e.to_string()))?
                    .to_string_lossy()
                    .into_owned();
                write(path.trim_start_matches("./"), &mut entry)?;
            }
        }
        ArchiveKind::Zip => {
            let file = File::open(archive).map_err(io_err(archive))?;
            let mut zip =
                zip::ZipArchive::new(file).map_err(|e| InstallError::Archive(e.to_string()))?;
            for i in 0..zip.len() {
                let mut entry = zip
                    .by_index(i)
                    .map_err(|e| InstallError::Archive(e.to_string()))?;
                if !entry.is_file() {
                    continue;
                }
                let name = entry.name().to_owned();
                write(&name, &mut entry)?;
            }
        }
    }
    if found_exe {
        Ok(exe_name)
    } else {
        Err(InstallError::Archive(format!(
            "o executável {} não está no arquivo",
            spec.executable
        )))
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(io_err(path))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), InstallError> {
    Ok(())
}
