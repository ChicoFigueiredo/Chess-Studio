//! Tabela fixada do Stockfish (DEC-28, ADR-0003): versão, URL oficial e SHA-256 de cada
//! asset por SO/arquitetura. Nada é "a última release": mudar a versão exige nova release
//! do app e novos hashes.
//!
//! Desde o Stockfish 19 a release publica binários `universal`, que escolhem em execução
//! o melhor código da CPU (AVX2, BMI2, AVX-512, NEON…). Por isso a escolha do asset depende
//! só de SO e arquitetura; não há mais um build por conjunto de instruções.

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

/// Formato do arquivo baixado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
}

/// Um asset oficial: o que baixar, o que esperar e onde está o executável dentro dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    pub os: &'static str,
    pub arch: &'static str,
    pub file_name: &'static str,
    /// Tamanho do download em bytes (mostrado ao usuário antes de confirmar).
    pub size: u64,
    /// SHA-256 do arquivo baixado, em hexadecimal minúsculo.
    pub sha256: &'static str,
    pub kind: ArchiveKind,
    /// Caminho do executável dentro do arquivo.
    pub executable: &'static str,
}

impl Asset {
    /// URL oficial do download.
    pub fn url(&self) -> String {
        format!("{RELEASE_BASE_URL}/{}", self.file_name)
    }
}

/// Assets do Stockfish 19 (hashes conferidos baixando cada arquivo).
pub const ASSETS: &[Asset] = &[
    Asset {
        os: "linux",
        arch: "x86_64",
        file_name: "stockfish-linux-x86-64-universal.tar.gz",
        size: 81_388_977,
        sha256: "9defc0d4e55d49c65a6d042f3e571a39fcea499ade6dbe741b53b8c65e03611f",
        kind: ArchiveKind::TarGz,
        executable: "stockfish/stockfish-linux-x86-64-universal",
    },
    Asset {
        os: "linux",
        arch: "aarch64",
        file_name: "stockfish-linux-arm64-universal.tar.gz",
        size: 80_181_250,
        sha256: "fe26cfd1d9db4c8af3d21e24d9ff34cacb31c1f940085a7583da11796f2bac01",
        kind: ArchiveKind::TarGz,
        executable: "stockfish/stockfish-linux-arm64-universal",
    },
    Asset {
        os: "macos",
        arch: "x86_64",
        file_name: "stockfish-macos-universal.tar.gz",
        size: 82_323_876,
        sha256: "a1f0e3bcc5a6927a11fe6fc8e54a779754645f3c2bae2cf13420fd1957adaa77",
        kind: ArchiveKind::TarGz,
        executable: "stockfish/stockfish-macos-universal",
    },
    Asset {
        os: "macos",
        arch: "aarch64",
        file_name: "stockfish-macos-universal.tar.gz",
        size: 82_323_876,
        sha256: "a1f0e3bcc5a6927a11fe6fc8e54a779754645f3c2bae2cf13420fd1957adaa77",
        kind: ArchiveKind::TarGz,
        executable: "stockfish/stockfish-macos-universal",
    },
    Asset {
        os: "windows",
        arch: "x86_64",
        file_name: "stockfish-windows-x86-64-universal.zip",
        size: 81_431_614,
        sha256: "3c8bf1f9ea66a09350a40df4f632288285ac206d99f33ab5842c408fc30b48a7",
        kind: ArchiveKind::Zip,
        executable: "stockfish/stockfish-windows-x86-64-universal.exe",
    },
    Asset {
        os: "windows",
        arch: "aarch64",
        file_name: "stockfish-windows-arm64-universal.zip",
        size: 80_190_536,
        sha256: "8372ad3f0d7276deb2c70f801f541ec7db463219fc6d9c7592864e542aa4f401",
        kind: ArchiveKind::Zip,
        executable: "stockfish/stockfish-windows-arm64-universal.exe",
    },
];

/// Asset para um SO e uma arquitetura (nomes de `std::env::consts`).
pub fn asset_for(os: &str, arch: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|a| a.os == os && a.arch == arch)
}

/// Asset do computador em que o app está rodando; `None` se não houver build oficial.
pub fn asset_for_host() -> Option<&'static Asset> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Pasta de instalação dentro de `<dados>/engines/`.
pub fn install_dir_name() -> String {
    format!("stockfish-{STOCKFISH_VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_asset_has_a_well_formed_pinned_hash_and_official_url() {
        for asset in ASSETS {
            assert_eq!(asset.sha256.len(), 64, "{}", asset.file_name);
            assert!(
                asset
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
            assert!(asset.url().starts_with(
                "https://github.com/official-stockfish/Stockfish/releases/download/sf_19/"
            ));
            assert!(asset.size > 0);
        }
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
