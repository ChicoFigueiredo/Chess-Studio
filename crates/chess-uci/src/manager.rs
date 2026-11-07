//! Operações do gerenciador de Engines (UCI-05, PEND-08): instalar e atualizar do catálogo,
//! remover com segurança e escolher a rede neural. Bloqueiam a thread que chama; o app as
//! executa em threads próprias. A rede só é tocada nas funções de instalação/atualização,
//! e só depois de o usuário confirmar.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::catalog::{Asset, CatalogEntry, Network, is_official_url};
use crate::install::{
    INSTALL_MARKER, InstallError, InstallSpec, Installed, InstalledNetwork, NetworkSpec, Progress,
    install, install_network,
};

/// Subpasta de `<dados>/engines/` com as redes neurais baixadas.
pub const NETWORKS_DIR: &str = "networks";

/// Por que uma remoção foi recusada.
#[derive(Debug)]
pub enum RemoveError {
    /// A pasta não é filha direta de `<dados>/engines/` (ou o diretório de engines não existe).
    OutsideEngines,
    /// A pasta é um link simbólico.
    Symlink,
    /// Falta o marcador: o app não instalou esta pasta.
    NotInstalledByApp,
    Io(io::Error),
}

impl fmt::Display for RemoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideEngines => write!(f, "a pasta está fora do diretório de Engines do app"),
            Self::Symlink => write!(f, "a pasta é um link simbólico"),
            Self::NotInstalledByApp => write!(f, "o app não instalou esta pasta"),
            Self::Io(e) => write!(f, "falha de disco: {e}"),
        }
    }
}

impl std::error::Error for RemoveError {}

/// Apaga uma Engine instalada pelo app. Recusa qualquer coisa que não seja uma pasta
/// **filha direta** de `engines_dir` (depois de resolver `..` e links), que não seja link
/// simbólico e que não tenha o marcador de instalação. Engines do usuário nunca passam.
pub fn remove_installed(engines_dir: &Path, dir: &Path) -> Result<(), RemoveError> {
    let root = fs::canonicalize(engines_dir).map_err(|_| RemoveError::OutsideEngines)?;
    // `symlink_metadata` não segue o link: um link para fora é visto como link.
    let meta = fs::symlink_metadata(dir).map_err(|_| RemoveError::OutsideEngines)?;
    if meta.file_type().is_symlink() {
        return Err(RemoveError::Symlink);
    }
    if !meta.is_dir() {
        return Err(RemoveError::OutsideEngines);
    }
    let real = fs::canonicalize(dir).map_err(RemoveError::Io)?;
    if real.parent() != Some(root.as_path()) {
        return Err(RemoveError::OutsideEngines);
    }
    let marker = real.join(INSTALL_MARKER);
    match fs::symlink_metadata(&marker) {
        Ok(m) if m.is_file() => {}
        _ => return Err(RemoveError::NotInstalledByApp),
    }
    fs::remove_dir_all(&real).map_err(RemoveError::Io)
}

/// Resultado de uma atualização bem-sucedida.
#[derive(Debug)]
pub struct Updated {
    pub installed: Installed,
    /// A versão anterior foi apagada (`false` se não havia ou se a remoção foi recusada).
    pub old_removed: bool,
}

/// Instala a nova versão **ao lado** da anterior; só depois de o hash conferir e o handshake
/// passar a anterior é removida. Qualquer falha deixa a anterior intacta e sem restos.
/// `new_dir_name` igual ao da anterior reinstala por cima (a troca só ocorre ao final).
pub fn update(
    spec: &InstallSpec,
    engines_dir: &Path,
    new_dir_name: &str,
    old_dir: Option<&Path>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Updated, InstallError> {
    let installed = install(spec, engines_dir, new_dir_name, cancel, progress)?;
    let old_removed = match old_dir {
        Some(old) if !same_dir(old, &installed.dir) => remove_installed(engines_dir, old).is_ok(),
        _ => false,
    };
    Ok(Updated {
        installed,
        old_removed,
    })
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// O que o usuário vê e confirma antes de uma instalação.
#[derive(Debug, Clone, Copy)]
pub struct InstallPlan {
    pub entry: &'static CatalogEntry,
    pub asset: &'static Asset,
    /// Rede neural a baixar junto (Lc0), se houver.
    pub network: Option<&'static Network>,
}

impl InstallPlan {
    /// Total a baixar, em bytes.
    pub fn total_size(&self) -> u64 {
        self.asset.size + self.network.map_or(0, |n| n.size)
    }

    /// Pasta da Engine em `<dados>/engines/`.
    pub fn dir_name(&self) -> String {
        self.entry.dir_name(self.asset)
    }

    /// Todas as URLs são `https` de hosts oficiais (verificação feita antes de baixar).
    pub fn is_official(&self) -> bool {
        is_official_url(&self.asset.url())
            && self.network.is_none_or(|n| is_official_url(&n.url()))
    }
}

/// Instala do catálogo. Recusa, sem tocar a rede, qualquer URL que não seja oficial.
pub fn install_from_catalog(
    plan: &InstallPlan,
    engines_dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Installed, InstallError> {
    ensure_official(plan)?;
    install(
        &InstallSpec::from(plan.asset),
        engines_dir,
        &plan.dir_name(),
        cancel,
        progress,
    )
}

/// Atualiza do catálogo (veja [`update`]).
pub fn update_from_catalog(
    plan: &InstallPlan,
    engines_dir: &Path,
    old_dir: Option<&Path>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Updated, InstallError> {
    ensure_official(plan)?;
    update(
        &InstallSpec::from(plan.asset),
        engines_dir,
        &plan.dir_name(),
        old_dir,
        cancel,
        progress,
    )
}

fn ensure_official(plan: &InstallPlan) -> Result<(), InstallError> {
    if plan.is_official() {
        Ok(())
    } else {
        Err(InstallError::Network(
            "URL fora da lista de hosts oficiais".to_owned(),
        ))
    }
}

/// Baixa uma rede neural do catálogo para `<engines_dir>/networks/`.
pub fn install_network_from_catalog(
    network: &Network,
    engines_dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledNetwork, InstallError> {
    if !is_official_url(&network.url()) {
        return Err(InstallError::Network(
            "URL fora da lista de hosts oficiais".to_owned(),
        ));
    }
    install_network(
        &NetworkSpec {
            url: network.url(),
            sha256: network.sha256.to_owned(),
            file_name: network.file_name.to_owned(),
        },
        &engines_dir.join(NETWORKS_DIR),
        cancel,
        progress,
    )
}

/// Opção UCI `WeightsFile` para uma rede **instalada pelo app**: o arquivo precisa estar
/// diretamente em `<engines_dir>/networks/` (sem links para fora). Qualquer outro caminho
/// é recusado: a opção de arquivo não é editável livremente (PEND-08, T8).
pub fn weights_option(engines_dir: &Path, network: &Path) -> Option<(String, String)> {
    let root = fs::canonicalize(engines_dir.join(NETWORKS_DIR)).ok()?;
    let real = fs::canonicalize(network).ok()?;
    (real.parent() == Some(root.as_path()) && real.is_file())
        .then(|| ("WeightsFile".to_owned(), real.to_string_lossy().into_owned()))
}

/// Redes neurais já baixadas (arquivos em `<engines_dir>/networks/`), em ordem de nome.
pub fn installed_networks(engines_dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(engines_dir.join(NETWORKS_DIR)) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| !n.to_string_lossy().starts_with('.'))
        })
        .collect();
    files.sort();
    files
}

/// Existe uma versão do catálogo diferente da instalada? Devolve o asset da variante
/// instalada na versão do catálogo (a atualização é sempre para a versão fixada no app).
pub fn update_available(
    catalog_id: &str,
    installed_version: &str,
    variant: &str,
    os: &str,
    arch: &str,
) -> Option<InstallPlan> {
    let entry = crate::catalog::entry(catalog_id)?;
    if entry.version == installed_version {
        return None;
    }
    let asset = entry
        .assets_for(os, arch)
        .into_iter()
        .find(|a| a.variant == variant)?;
    Some(InstallPlan {
        entry,
        asset,
        network: None,
    })
}

/// Planos de instalação do catálogo para este computador, uma Engine por vez: a(s)
/// variante(s) disponíveis; Engines sem build oficial para o SO ficam fora (a UI avisa).
pub fn plans_for(os: &str, arch: &str) -> Vec<InstallPlan> {
    let mut plans = Vec::new();
    for entry in crate::catalog::CATALOG {
        for asset in entry.assets_for(os, arch) {
            plans.push(InstallPlan {
                entry,
                asset,
                network: None,
            });
        }
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CATALOG, LC0_VERSION};

    #[test]
    fn windows_offers_stockfish_and_all_lc0_builds_but_linux_only_stockfish() {
        let win = plans_for("windows", "x86_64");
        assert!(win.iter().any(|p| p.entry.id == "stockfish"));
        assert_eq!(win.iter().filter(|p| p.entry.id == "lc0").count(), 4);
        let linux = plans_for("linux", "x86_64");
        assert!(linux.iter().all(|p| p.entry.id == "stockfish"));
        assert!(win.iter().all(InstallPlan::is_official));
    }

    #[test]
    fn plan_total_includes_the_network() {
        let lc0 = crate::catalog::entry("lc0").expect("lc0");
        let plan = InstallPlan {
            entry: lc0,
            asset: &lc0.assets[0],
            network: lc0.networks.first(),
        };
        assert_eq!(
            plan.total_size(),
            lc0.assets[0].size + lc0.networks[0].size
        );
        assert_eq!(plan.dir_name(), format!("lc0-{LC0_VERSION}-cpu-openblas"));
    }

    #[test]
    fn update_is_offered_only_for_a_different_catalog_version() {
        assert!(update_available("stockfish", "19", "", "linux", "x86_64").is_none());
        let plan = update_available("stockfish", "18", "", "linux", "x86_64").expect("update");
        assert_eq!(plan.entry.version, "19");
        assert!(update_available("lc0", "0.31.0", "gpu-nope", "windows", "x86_64").is_none());
        assert!(update_available("desconhecida", "1", "", "linux", "x86_64").is_none());
        assert!(CATALOG.len() >= 2);
    }

    #[test]
    fn weights_must_live_in_the_networks_directory() {
        let base = std::env::temp_dir().join(format!("weights-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join(NETWORKS_DIR)).expect("dir");
        let net = base.join(NETWORKS_DIR).join("rede.pb.gz");
        fs::write(&net, "x").expect("rede");
        let outside = base.join("fora.pb.gz");
        fs::write(&outside, "x").expect("fora");
        assert!(weights_option(&base, &net).is_some());
        assert!(weights_option(&base, &outside).is_none());
        assert!(weights_option(&base, &base.join(NETWORKS_DIR).join("../fora.pb.gz")).is_none());
        let _ = fs::remove_dir_all(&base);
    }
}
