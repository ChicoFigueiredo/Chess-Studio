//! Catálogo fixado de Engines (DEC-28, ADR-0003, PEND-08): versão, URL oficial e SHA-256 de
//! cada arquivo por SO/arquitetura, mais as redes neurais do Lc0. Nada é "a última
//! release": mudar uma versão exige nova release do app e novos hashes, calculados
//! baixando o arquivo (veja `docs/decisoes/seguranca-engines.md`).
//!
//! Desde o Stockfish 19 a release publica binários `universal`, que escolhem em execução
//! o melhor código da CPU (AVX2, BMI2, AVX-512, NEON…). Por isso a escolha do asset depende
//! só de SO e arquitetura; não há mais um build por conjunto de instruções.
//!
//! O Lc0 só publica binários oficiais para Windows (e Android); em Linux e macOS o app
//! detecta um Lc0 instalado pelo usuário, mas não o baixa.

/// Versão do Stockfish fixada nesta build.
pub const STOCKFISH_VERSION: &str = "19";
/// Licença (GPLv3) a exibir antes do download.
pub const STOCKFISH_LICENSE_NAME: &str = "GNU GPL v3";
pub const STOCKFISH_LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
/// Código-fonte da versão fixada.
pub const STOCKFISH_SOURCE_URL: &str = "https://github.com/official-stockfish/Stockfish/tree/sf_19";
/// Prefixo oficial dos downloads da release `sf_19`.
pub const RELEASE_BASE_URL: &str =
    "https://github.com/official-stockfish/Stockfish/releases/download/sf_19";

/// Versão do Lc0 fixada nesta build.
pub const LC0_VERSION: &str = "0.32.1";
const LC0_SOURCE_URL: &str = "https://github.com/LeelaChessZero/lc0/tree/v0.32.1";
const LC0_BASE_URL: &str = "https://github.com/LeelaChessZero/lc0/releases/download/v0.32.1";
const LC0_NETS_BASE_URL: &str = "https://storage.lczero.org/files/networks-contrib";

/// Hosts de onde o app aceita baixar (lista fechada; sempre `https`).
pub const OFFICIAL_HOSTS: &[&str] = &["github.com", "storage.lczero.org"];

/// A URL é `https://` de um host oficial, sem credenciais nem porta.
pub fn is_official_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    OFFICIAL_HOSTS.contains(&host)
}

/// Formato do arquivo baixado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
}

/// Onde a Engine calcula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Gpu,
}

/// Um asset oficial: o que baixar, o que esperar e onde está o executável dentro dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// Variante do build (`""` quando só há uma; ex.: `cpu-openblas`, `gpu-cuda12`).
    pub variant: &'static str,
    /// Nome para mostrar (ex.: "CPU (OpenBLAS)").
    pub label: &'static str,
    pub backend: Backend,
    pub os: &'static str,
    pub arch: &'static str,
    pub file_name: &'static str,
    /// Prefixo oficial do download.
    pub base_url: &'static str,
    /// Tamanho do download em bytes (mostrado ao usuário antes de confirmar).
    pub size: u64,
    /// SHA-256 do arquivo baixado, em hexadecimal minúsculo.
    pub sha256: &'static str,
    pub kind: ArchiveKind,
    /// Caminho do executável dentro do arquivo.
    pub executable: &'static str,
    /// Outros arquivos a extrair (nome-base, na mesma pasta do executável): licenças e
    /// bibliotecas de que o executável depende. Nada além disso sai do arquivo.
    pub keep: &'static [&'static str],
    /// Aviso a mostrar antes de instalar (dependências externas, requisitos de hardware).
    pub note: &'static str,
}

impl Asset {
    /// URL oficial do download.
    pub fn url(&self) -> String {
        format!("{}/{}", self.base_url, self.file_name)
    }
}

/// Rede neural oficial, instalada à parte e com hash próprio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    pub id: &'static str,
    pub name: &'static str,
    pub file_name: &'static str,
    pub base_url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    /// Rede indicada para começar.
    pub recommended: bool,
    pub note: &'static str,
}

impl Network {
    pub fn url(&self) -> String {
        format!("{}/{}", self.base_url, self.file_name)
    }
}

/// Uma Engine do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub license_name: &'static str,
    pub license_url: &'static str,
    pub source_url: &'static str,
    pub assets: &'static [Asset],
    pub networks: &'static [Network],
}

impl CatalogEntry {
    /// Assets disponíveis para um SO e uma arquitetura (nomes de `std::env::consts`).
    pub fn assets_for(&self, os: &str, arch: &str) -> Vec<&'static Asset> {
        self.assets
            .iter()
            .filter(|a| a.os == os && a.arch == arch)
            .collect()
    }

    /// Nome da pasta em `<dados>/engines/`: `<id>-<versão>[-<variante>]`.
    pub fn dir_name(&self, asset: &Asset) -> String {
        if asset.variant.is_empty() {
            format!("{}-{}", self.id, self.version)
        } else {
            format!("{}-{}-{}", self.id, self.version, asset.variant)
        }
    }
}

/// Todas as Engines que o app sabe instalar.
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "stockfish",
        name: "Stockfish",
        version: STOCKFISH_VERSION,
        license_name: STOCKFISH_LICENSE_NAME,
        license_url: STOCKFISH_LICENSE_URL,
        source_url: STOCKFISH_SOURCE_URL,
        assets: ASSETS,
        networks: &[],
    },
    CatalogEntry {
        id: "lc0",
        name: "Leela Chess Zero (Lc0)",
        version: LC0_VERSION,
        license_name: "GNU GPL v3",
        license_url: "https://www.gnu.org/licenses/gpl-3.0.html",
        source_url: LC0_SOURCE_URL,
        assets: LC0_ASSETS,
        networks: LC0_NETWORKS,
    },
];

/// Engine do catálogo pelo identificador.
pub fn entry(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.id == id)
}

const SF_KEEP: &[&str] = &["Copying.txt", "AUTHORS"];

const fn stockfish(
    os: &'static str,
    arch: &'static str,
    file_name: &'static str,
    size: u64,
    sha256: &'static str,
    kind: ArchiveKind,
    executable: &'static str,
) -> Asset {
    Asset {
        variant: "",
        label: "Universal",
        backend: Backend::Cpu,
        os,
        arch,
        file_name,
        base_url: RELEASE_BASE_URL,
        size,
        sha256,
        kind,
        executable,
        keep: SF_KEEP,
        note: "",
    }
}

/// Assets do Stockfish 19 (hashes conferidos baixando cada arquivo).
pub const ASSETS: &[Asset] = &[
    stockfish(
        "linux",
        "x86_64",
        "stockfish-linux-x86-64-universal.tar.gz",
        81_388_977,
        "9defc0d4e55d49c65a6d042f3e571a39fcea499ade6dbe741b53b8c65e03611f",
        ArchiveKind::TarGz,
        "stockfish/stockfish-linux-x86-64-universal",
    ),
    stockfish(
        "linux",
        "aarch64",
        "stockfish-linux-arm64-universal.tar.gz",
        80_181_250,
        "fe26cfd1d9db4c8af3d21e24d9ff34cacb31c1f940085a7583da11796f2bac01",
        ArchiveKind::TarGz,
        "stockfish/stockfish-linux-arm64-universal",
    ),
    stockfish(
        "macos",
        "x86_64",
        "stockfish-macos-universal.tar.gz",
        82_323_876,
        "a1f0e3bcc5a6927a11fe6fc8e54a779754645f3c2bae2cf13420fd1957adaa77",
        ArchiveKind::TarGz,
        "stockfish/stockfish-macos-universal",
    ),
    stockfish(
        "macos",
        "aarch64",
        "stockfish-macos-universal.tar.gz",
        82_323_876,
        "a1f0e3bcc5a6927a11fe6fc8e54a779754645f3c2bae2cf13420fd1957adaa77",
        ArchiveKind::TarGz,
        "stockfish/stockfish-macos-universal",
    ),
    stockfish(
        "windows",
        "x86_64",
        "stockfish-windows-x86-64-universal.zip",
        81_431_614,
        "3c8bf1f9ea66a09350a40df4f632288285ac206d99f33ab5842c408fc30b48a7",
        ArchiveKind::Zip,
        "stockfish/stockfish-windows-x86-64-universal.exe",
    ),
    stockfish(
        "windows",
        "aarch64",
        "stockfish-windows-arm64-universal.zip",
        80_190_536,
        "8372ad3f0d7276deb2c70f801f541ec7db463219fc6d9c7592864e542aa4f401",
        ArchiveKind::Zip,
        "stockfish/stockfish-windows-arm64-universal.exe",
    ),
];

const LC0_OPENBLAS_KEEP: &[&str] = &[
    "COPYING",
    "libopenblas.dll",
    "mimalloc-override.dll",
    "mimalloc-redirect.dll",
    "mimalloc-LICENSE",
];
const LC0_DNNL_KEEP: &[&str] = &[
    "COPYING",
    "dnnl.dll",
    "DNNL-LICENSE",
    "mimalloc-override.dll",
    "mimalloc-redirect.dll",
    "mimalloc-LICENSE",
];
const LC0_DML_KEEP: &[&str] = &[
    "COPYING",
    "onnxruntime.dll",
    "ONNX-LICENSE",
    "README.txt",
    "mimalloc-override.dll",
    "mimalloc-redirect.dll",
    "mimalloc-LICENSE",
];
const LC0_CUDA12_KEEP: &[&str] = &[
    "COPYING",
    "cublas64_12.dll",
    "cublasLt64_12.dll",
    "cudart64_12.dll",
    "CUDA.txt",
    "README.txt",
    "mimalloc-override.dll",
    "mimalloc-redirect.dll",
    "mimalloc-LICENSE",
];

#[allow(clippy::too_many_arguments)]
const fn lc0_windows(
    variant: &'static str,
    label: &'static str,
    backend: Backend,
    file_name: &'static str,
    size: u64,
    sha256: &'static str,
    keep: &'static [&'static str],
    note: &'static str,
) -> Asset {
    Asset {
        variant,
        label,
        backend,
        os: "windows",
        arch: "x86_64",
        file_name,
        base_url: LC0_BASE_URL,
        size,
        sha256,
        kind: ArchiveKind::Zip,
        executable: "lc0.exe",
        keep,
        note,
    }
}

/// Assets do Lc0 0.32.1 (release oficial; só Windows x86-64 tem binários publicados).
/// Cada hash foi calculado baixando o arquivo e coincide com o `digest` da release.
pub const LC0_ASSETS: &[Asset] = &[
    lc0_windows(
        "cpu-openblas",
        "CPU (OpenBLAS)",
        Backend::Cpu,
        "lc0-v0.32.1-windows-cpu-openblas.zip",
        23_818_982,
        "b2caa8443f0e0cb15cf76c335c53985f2973cd6438e77d3e2366cd21d2effa38",
        LC0_OPENBLAS_KEEP,
        "",
    ),
    lc0_windows(
        "cpu-dnnl",
        "CPU (oneDNN, processadores Intel)",
        Backend::Cpu,
        "lc0-v0.32.1-windows-cpu-dnnl.zip",
        24_001_097,
        "b9cfcfbd3dabffbfd452f6e8e087c22273721bef48eba19640b6d92006c142f5",
        LC0_DNNL_KEEP,
        "",
    ),
    lc0_windows(
        "gpu-dx12",
        "GPU DirectX 12 (ONNX-DML, qualquer placa)",
        Backend::Gpu,
        "lc0-v0.32.1-windows-onnx-dml.zip",
        25_410_041,
        "86aac242b3a1c6986d02acc33b8cc64175b11a21b6941c62443c07fa18ac7539",
        LC0_DML_KEEP,
        "Precisa do DirectML.dll da Microsoft, que não acompanha o pacote por licença: veja o README.txt na pasta da Engine.",
    ),
    lc0_windows(
        "gpu-cuda12",
        "GPU NVIDIA (CUDA 12)",
        Backend::Gpu,
        "lc0-v0.32.1-windows-gpu-nvidia-cuda12.zip",
        581_970_338,
        "8d0ce17676eb15e303bea9e790742d31c94ce5d24107f6187adc58e820f6d2f7",
        LC0_CUDA12_KEEP,
        "Placa NVIDIA com driver recente. O pacote inclui as bibliotecas CUDA (download grande).",
    ),
];

/// Redes neurais oficiais recomendadas (lista "Best Networks" de lczero.org).
pub const LC0_NETWORKS: &[Network] = &[
    Network {
        id: "t1-256x10",
        name: "T1-256x10 (pequena, para CPU)",
        file_name: "t1-256x10-distilled-swa-2432500.pb.gz",
        base_url: LC0_NETS_BASE_URL,
        size: 37_118_673,
        sha256: "bc27a6cae8ad36f2b9a80a6ad9dabb0d6fda25b1e7f481a79bc359e14f563406",
        recommended: true,
        note: "Boa para CPU e placas modestas.",
    },
    Network {
        id: "t1-512x15",
        name: "T1-512x15 (média, para GPU)",
        file_name: "t1-512x15x8h-distilled-swa-3395000.pb.gz",
        base_url: LC0_NETS_BASE_URL,
        size: 149_758_071,
        sha256: "1fdb1519e5b02e03f1d9201ec8eb95f640e32cc6459a4f14c0eab6890dc097e8",
        recommended: false,
        note: "Mais forte; pede GPU (cerca de 1,8 GB de memória de vídeo).",
    },
];

/// Asset do Stockfish para um SO e uma arquitetura (nomes de `std::env::consts`).
pub fn asset_for(os: &str, arch: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|a| a.os == os && a.arch == arch)
}

/// Asset do Stockfish do computador em que o app está; `None` se não houver build oficial.
pub fn asset_for_host() -> Option<&'static Asset> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Pasta de instalação do Stockfish dentro de `<dados>/engines/`.
pub fn install_dir_name() -> String {
    format!("stockfish-{STOCKFISH_VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn well_formed_hash(hash: &str) -> bool {
        hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    #[test]
    fn every_asset_has_a_well_formed_pinned_hash_and_official_url() {
        for entry in CATALOG {
            for asset in entry.assets {
                assert!(well_formed_hash(asset.sha256), "{}", asset.file_name);
                assert!(is_official_url(&asset.url()), "{}", asset.url());
                assert!(asset.size > 0);
                assert!(!asset.executable.is_empty());
            }
            for net in entry.networks {
                assert!(well_formed_hash(net.sha256), "{}", net.file_name);
                assert!(is_official_url(&net.url()), "{}", net.url());
                assert!(net.size > 0);
                assert!(!net.file_name.contains('/'));
            }
        }
        assert!(
            asset_for("linux", "x86_64")
                .expect("asset")
                .url()
                .starts_with(
                    "https://github.com/official-stockfish/Stockfish/releases/download/sf_19/"
                )
        );
    }

    #[test]
    fn only_https_urls_of_official_hosts_are_accepted() {
        assert!(is_official_url("https://github.com/a/b"));
        assert!(is_official_url("https://storage.lczero.org/files/x.pb.gz"));
        assert!(!is_official_url("http://github.com/a/b"));
        assert!(!is_official_url("https://github.com.evil.example/a"));
        assert!(!is_official_url("https://evilgithub.com/a"));
        assert!(!is_official_url("https://github.com@evil.example/a"));
        assert!(!is_official_url("https://github.com:444/a"));
        assert!(!is_official_url("ftp://github.com/a"));
        assert!(!is_official_url("https://evil.example/github.com/a"));
    }

    #[test]
    fn catalog_ids_and_install_dirs_are_unique_and_safe() {
        let mut dirs = std::collections::HashSet::new();
        for entry in CATALOG {
            assert!(!entry.license_name.is_empty() && is_official_url(entry.source_url));
            for asset in entry.assets {
                let dir = entry.dir_name(asset);
                assert!(
                    dir.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b)),
                    "{dir}"
                );
                // O mesmo arquivo serve a macOS x86-64 e arm64: a pasta é por entrada/variante.
                if !(entry.id == "stockfish" && asset.os == "macos") {
                    assert!(dirs.insert((dir, asset.os, asset.arch)));
                }
                // Tudo o que se extrai tem nome-base simples.
                for keep in asset.keep {
                    assert!(!keep.contains('/') && !keep.contains(".."), "{keep}");
                }
            }
        }
        assert_eq!(install_dir_name(), "stockfish-19");
    }

    #[test]
    fn lc0_has_gpu_and_cpu_builds_for_windows_and_a_recommended_network() {
        let lc0 = entry("lc0").expect("lc0");
        let win = lc0.assets_for("windows", "x86_64");
        assert!(win.iter().any(|a| a.backend == Backend::Cpu));
        assert!(win.iter().any(|a| a.backend == Backend::Gpu));
        assert!(
            lc0.assets_for("linux", "x86_64").is_empty(),
            "sem binário oficial"
        );
        assert_eq!(lc0.networks.iter().filter(|n| n.recommended).count(), 1);
        assert_eq!(lc0.version, LC0_VERSION);
    }

    #[test]
    fn lookup_by_os_and_arch() {
        assert_eq!(
            asset_for("linux", "x86_64").map(|a| a.file_name),
            Some("stockfish-linux-x86-64-universal.tar.gz")
        );
        assert_eq!(
            asset_for("windows", "aarch64").map(|a| a.kind),
            Some(ArchiveKind::Zip)
        );
        assert_eq!(
            asset_for("macos", "aarch64").map(|a| a.file_name),
            asset_for("macos", "x86_64").map(|a| a.file_name)
        );
        assert!(asset_for("freebsd", "x86_64").is_none());
    }
}
