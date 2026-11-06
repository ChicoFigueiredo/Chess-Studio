//! Preferências de interface do app: Apresentação e qualidade gráfica (B12, UX-07).
//!
//! Toda a leitura e escrita passa por [`load`] e [`save`]. Hoje gravam um JSON pequeno no
//! diretório de configuração do usuário.
//!
//! TODO(B4, ponto de ligação): quando o `chess-store` estiver ligado ao app, trocar o corpo
//! de `load`/`save` por `Store::get_pref/set_pref` (chaves `ui.presentation` e
//! `ui.quality`, perfil ativo). Nada fora deste arquivo precisa mudar.

use crate::presentation::{GraphicsQuality, Presentation};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::path::PathBuf;

/// Preferências persistidas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UiPrefs {
    pub presentation: Presentation,
    pub quality: GraphicsQuality,
}

impl UiPrefs {
    fn to_json(self) -> Value {
        json!({
            "ui.presentation": self.presentation.key(),
            "ui.quality": self.quality.key(),
        })
    }

    /// Valores ausentes ou desconhecidos caem no padrão (arquivo antigo ou corrompido).
    fn from_json(value: &Value) -> Self {
        let get = |key: &str| value.get(key).and_then(Value::as_str);
        Self {
            presentation: get("ui.presentation")
                .and_then(Presentation::from_key)
                .unwrap_or_default(),
            quality: get("ui.quality")
                .and_then(GraphicsQuality::from_key)
                .unwrap_or_default(),
        }
    }
}

fn prefs_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let home = PathBuf::from(h);
                if cfg!(target_os = "macos") {
                    home.join("Library/Application Support")
                } else {
                    home.join(".config")
                }
            })
        })?;
    Some(base.join("chess-studio").join("ui-prefs.json"))
}

/// Lê as preferências (padrão se não houver nada gravado).
pub fn load() -> UiPrefs {
    prefs_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|v| UiPrefs::from_json(&v))
        .unwrap_or_default()
}

/// Grava as preferências no ato (sem "Salvar"); falhas só geram aviso.
pub fn save(prefs: UiPrefs) {
    let Some(path) = prefs_path() else { return };
    if let Some(dir) = path.parent()
        && let Err(err) = std::fs::create_dir_all(dir)
    {
        warn!("não foi possível criar {}: {err}", dir.display());
        return;
    }
    if let Err(err) = std::fs::write(&path, prefs.to_json().to_string()) {
        warn!("não foi possível gravar {}: {err}", path.display());
    }
}

/// Carrega as preferências na partida do app e as regrava quando mudam. Desligado nos modos
/// de captura e benchmark, que não devem ler nem alterar as preferências do usuário.
pub struct PrefsPlugin {
    pub enabled: bool,
}

impl Plugin for PrefsPlugin {
    fn build(&self, app: &mut App) {
        if !self.enabled {
            return;
        }
        let prefs = load();
        // Inserido antes do `PresentationPlugin`, cujo `init_resource` não sobrescreve.
        app.insert_resource(prefs.presentation)
            .insert_resource(prefs.quality)
            .add_systems(Update, persist);
    }
}

fn persist(
    presentation: Res<Presentation>,
    quality: Res<GraphicsQuality>,
    mut last: Local<Option<UiPrefs>>,
) {
    let now = UiPrefs {
        presentation: *presentation,
        quality: *quality,
    };
    match *last {
        // Primeira leitura: é o que acabou de ser carregado.
        None => *last = Some(now),
        Some(prev) if prev != now => {
            save(now);
            *last = Some(now);
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_round_trip_through_json() {
        let prefs = UiPrefs {
            presentation: Presentation::ImmersiveTable,
            quality: GraphicsQuality::Economic,
        };
        assert_eq!(UiPrefs::from_json(&prefs.to_json()), prefs);
    }

    #[test]
    fn unknown_or_missing_values_fall_back_to_defaults() {
        let value = json!({ "ui.presentation": "hologram" });
        assert_eq!(UiPrefs::from_json(&value), UiPrefs::default());
        assert_eq!(UiPrefs::from_json(&json!(null)), UiPrefs::default());
    }
}
