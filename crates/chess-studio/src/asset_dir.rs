//! Localização da pasta `assets/` em um app instalado. O Bevy só acha `assets` relativo ao
//! `CARGO_MANIFEST_DIR` quando roda via `cargo run`; fora disso o 3D ficaria sem modelos e
//! texturas. A resolução é pura (recebe ambiente e "existe?") para ser testável por layout.

use std::path::{Path, PathBuf};

use crate::paths::Platform;

/// Variável que força a pasta de assets (vence qualquer layout).
pub const ENV_OVERRIDE: &str = "CHESS_STUDIO_ASSETS";

/// Arquivo que só existe numa pasta de assets válida; usado para validar candidatos.
const MARKER: &str = "models/chess_pieces.glb";

/// Candidatos em ordem de prioridade, dado o diretório do executável.
pub fn candidates(platform: Platform, exe_dir: &Path) -> Vec<PathBuf> {
    // Tarball/zip: assets ao lado do executável.
    let mut list = vec![exe_dir.join("assets")];
    match platform {
        // Linux instalado (/usr/bin -> /usr/share/chess-studio/assets).
        Platform::Linux => list.push(exe_dir.join("../share/chess-studio/assets")),
        // Chess Studio.app/Contents/MacOS/chess-studio -> Contents/Resources/assets.
        Platform::MacOs => list.push(exe_dir.join("../Resources/assets")),
        Platform::Windows => {}
    }
    list
}

/// Escolhe a pasta de assets: override, depois layouts instalados, depois a pasta de
/// desenvolvimento; sem nenhuma, devolve a de desenvolvimento mesmo (erros do Bevy ficam claros).
pub fn resolve(
    platform: Platform,
    exe_dir: Option<&Path>,
    env: &dyn Fn(&str) -> Option<String>,
    has_marker: &dyn Fn(&Path) -> bool,
    dev_dir: &Path,
) -> PathBuf {
    if let Some(over) = env(ENV_OVERRIDE).filter(|v| !v.is_empty()) {
        return PathBuf::from(over);
    }
    if let Some(dir) = exe_dir {
        for cand in candidates(platform, dir) {
            if has_marker(&cand) {
                return cand;
            }
        }
    }
    dev_dir.to_path_buf()
}

/// Pasta de assets do processo atual, como texto para `AssetPlugin::file_path`.
pub fn current() -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let dir = resolve(
        Platform::current(),
        exe_dir.as_deref(),
        &crate::paths::process_env,
        &|p| p.join(MARKER).is_file(),
        &dev,
    );
    dir.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn override_wins_even_without_marker() {
        let env = |n: &str| (n == ENV_OVERRIDE).then(|| "/meus/assets".to_owned());
        let got = resolve(
            Platform::Linux,
            Some(Path::new("/opt/cs")),
            &env,
            &|_| true,
            Path::new("/dev/assets"),
        );
        assert_eq!(got, PathBuf::from("/meus/assets"));
    }

    #[test]
    fn next_to_executable_for_tarball_and_zip() {
        for p in [Platform::Linux, Platform::Windows, Platform::MacOs] {
            let want = PathBuf::from("/opt/cs/assets");
            let got = resolve(
                p,
                Some(Path::new("/opt/cs")),
                &no_env,
                &|c| c == want,
                Path::new("/dev/assets"),
            );
            assert_eq!(got, want);
        }
    }

    #[test]
    fn linux_installed_uses_share() {
        let want = PathBuf::from("/usr/bin/../share/chess-studio/assets");
        let got = resolve(
            Platform::Linux,
            Some(Path::new("/usr/bin")),
            &no_env,
            &|c| c == want,
            Path::new("/dev/assets"),
        );
        assert_eq!(got, want);
    }

    #[test]
    fn macos_bundle_uses_resources() {
        let want = PathBuf::from("/A/Chess Studio.app/Contents/MacOS/../Resources/assets");
        let got = resolve(
            Platform::MacOs,
            Some(Path::new("/A/Chess Studio.app/Contents/MacOS")),
            &no_env,
            &|c| c == want,
            Path::new("/dev/assets"),
        );
        assert_eq!(got, want);
    }

    #[test]
    fn share_layout_is_ignored_outside_linux() {
        let share = PathBuf::from("/usr/bin/../share/chess-studio/assets");
        let got = resolve(
            Platform::Windows,
            Some(Path::new("/usr/bin")),
            &no_env,
            &|c| c == share,
            Path::new("/dev/assets"),
        );
        assert_eq!(got, PathBuf::from("/dev/assets"));
    }

    #[test]
    fn falls_back_to_dev_dir() {
        let got = resolve(
            Platform::Linux,
            None,
            &no_env,
            &|_| false,
            Path::new("/dev/assets"),
        );
        assert_eq!(got, PathBuf::from("/dev/assets"));
    }
}
