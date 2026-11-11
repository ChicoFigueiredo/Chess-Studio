//! Preferências de aparência do Perfil local: Tema, Densidade e Escala (DS-05, DS-13). Ficam
//! fora do Perfil de interface (DS-07): trocar de Perfil de interface não muda o Tema.

use super::themes::{ThemeId, all_themes};
use bevy::prelude::Resource;

/// Escolha do Tema: seguir o sistema (padrão) ou um dos 22 Temas fixo. "Seguir o sistema"
/// alterna só entre o Escuro e o Claro de fábrica; escolher um Tema de xadrez fixa esse Tema.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Fixed(ThemeId),
}

impl ThemeMode {
    /// Escuro de fábrica, fixo.
    pub const DARK: ThemeMode = ThemeMode::Fixed(ThemeId::DARK);
    /// Claro de fábrica, fixo.
    pub const LIGHT: ThemeMode = ThemeMode::Fixed(ThemeId::LIGHT);

    /// "Seguir o sistema" e depois os 22 Temas, na ordem do catálogo.
    pub fn all() -> Vec<ThemeMode> {
        std::iter::once(ThemeMode::System)
            .chain(all_themes().map(ThemeMode::Fixed))
            .collect()
    }

    /// Chave de tradução de "Seguir o sistema"; os Temas fixos usam o nome do catálogo.
    pub const SYSTEM_LABEL_KEY: &'static str = "theme-mode-system";

    /// Texto do seletor: o rótulo traduzido de "Seguir o sistema" ou o nome do Tema.
    pub fn label(self, system_label: &str) -> String {
        match self {
            ThemeMode::System => system_label.to_owned(),
            ThemeMode::Fixed(id) => id.name().to_owned(),
        }
    }

    /// Texto gravado no banco: `"System"` ou o id do Tema (`"feltro"`, `"dark"`).
    pub fn storage_key(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Fixed(id) => id.key(),
        }
    }

    /// Lê o texto gravado. Aceita os valores antigos (`"System"`, `"Dark"`, `"Light"`, que eram
    /// o `Debug` do enum de antes) e as grafias em pt-BR; valor desconhecido é `None`.
    pub fn from_storage(text: &str) -> Option<Self> {
        match text {
            "System" | "system" | "sistema" => Some(ThemeMode::System),
            "Dark" | "escuro" => Some(ThemeMode::DARK),
            "Light" | "claro" => Some(ThemeMode::LIGHT),
            id => ThemeId::from_key(id).map(ThemeMode::Fixed),
        }
    }
}

/// O Tema efetivo. Sem informação do sistema (Linux sem portal, WSL), vale o Escuro.
pub fn resolve_theme(mode: ThemeMode, system_dark: Option<bool>) -> ThemeId {
    match mode {
        ThemeMode::Fixed(id) => id,
        ThemeMode::System => {
            if system_dark.unwrap_or(true) {
                ThemeId::DARK
            } else {
                ThemeId::LIGHT
            }
        }
    }
}

/// Densidade dos controles: Confortável (32px, padrão) ou Compacta (24px).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    pub fn key(self) -> &'static str {
        match self {
            Density::Comfortable => "density-comfortable",
            Density::Compact => "density-compact",
        }
    }
}

/// Escala da interface em porcentagem, sempre entre 80% e 200% (DS-13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiScale(u16);

impl UiScale {
    pub const MIN_PERCENT: u16 = 80;
    pub const MAX_PERCENT: u16 = 200;
    /// Degraus de Ctrl+= e Ctrl+-.
    const STEPS: [u16; 9] = [80, 90, 100, 110, 125, 150, 175, 190, 200];

    /// Limita ao intervalo permitido.
    pub fn from_percent(percent: i64) -> Self {
        Self(percent.clamp(Self::MIN_PERCENT.into(), Self::MAX_PERCENT.into()) as u16)
    }

    pub fn percent(self) -> u16 {
        self.0
    }

    /// Multiplicador do egui (`zoom_factor`), de 0,8 a 2,0.
    pub fn factor(self) -> f32 {
        f32::from(self.0) / 100.0
    }

    /// Próximo degrau acima (Ctrl+=); no máximo, fica onde está.
    pub fn zoom_in(self) -> Self {
        Self::STEPS
            .into_iter()
            .find(|&s| s > self.0)
            .map_or(self, Self)
    }

    /// Próximo degrau abaixo (Ctrl+-); no mínimo, fica onde está.
    pub fn zoom_out(self) -> Self {
        Self::STEPS
            .into_iter()
            .rev()
            .find(|&s| s < self.0)
            .map_or(self, Self)
    }
}

impl Default for UiScale {
    fn default() -> Self {
        Self(100)
    }
}

/// Aparência da interface (recurso do Bevy e preferência do Perfil local).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub theme: ThemeMode,
    pub density: Density,
    pub scale: UiScale,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_follows_the_os_and_falls_back_to_dark() {
        assert_eq!(
            resolve_theme(ThemeMode::System, Some(false)),
            ThemeId::LIGHT
        );
        assert_eq!(resolve_theme(ThemeMode::System, Some(true)), ThemeId::DARK);
        assert_eq!(resolve_theme(ThemeMode::System, None), ThemeId::DARK);
    }

    #[test]
    fn an_explicit_theme_ignores_the_os() {
        assert_eq!(resolve_theme(ThemeMode::DARK, Some(false)), ThemeId::DARK);
        assert_eq!(resolve_theme(ThemeMode::LIGHT, Some(true)), ThemeId::LIGHT);
        assert_eq!(resolve_theme(ThemeMode::LIGHT, None), ThemeId::LIGHT);
    }

    #[test]
    fn scale_is_clamped_to_80_through_200_percent() {
        assert_eq!(UiScale::from_percent(10).percent(), 80);
        assert_eq!(UiScale::from_percent(-5).percent(), 80);
        assert_eq!(UiScale::from_percent(500).percent(), 200);
        assert_eq!(UiScale::from_percent(125).percent(), 125);
        assert!((UiScale::from_percent(80).factor() - 0.8).abs() < 1e-6);
        assert!((UiScale::from_percent(200).factor() - 2.0).abs() < 1e-6);
        assert_eq!(UiScale::default().percent(), 100);
    }

    #[test]
    fn zoom_steps_walk_the_ladder_and_stop_at_the_ends() {
        let mut s = UiScale::default();
        s = s.zoom_in();
        assert_eq!(s.percent(), 110);
        s = s.zoom_out().zoom_out();
        assert_eq!(s.percent(), 90);
        for _ in 0..20 {
            s = s.zoom_in();
        }
        assert_eq!(s.percent(), 200);
        for _ in 0..20 {
            s = s.zoom_out();
        }
        assert_eq!(s.percent(), 80);
        // Um valor fora da escada (125 vira 150, 118 vira 125 e volta a 110).
        assert_eq!(UiScale::from_percent(118).zoom_in().percent(), 125);
        assert_eq!(UiScale::from_percent(118).zoom_out().percent(), 110);
    }

    /// A preferência antiga (o `Debug` do enum de antes) continua carregando.
    #[test]
    fn old_stored_values_still_load() {
        assert_eq!(ThemeMode::from_storage("System"), Some(ThemeMode::System));
        assert_eq!(ThemeMode::from_storage("Dark"), Some(ThemeMode::DARK));
        assert_eq!(ThemeMode::from_storage("Light"), Some(ThemeMode::LIGHT));
        assert_eq!(ThemeMode::from_storage("sistema"), Some(ThemeMode::System));
        assert_eq!(ThemeMode::from_storage("escuro"), Some(ThemeMode::DARK));
        assert_eq!(ThemeMode::from_storage("claro"), Some(ThemeMode::LIGHT));
        assert_eq!(ThemeMode::from_storage("lixo"), None);
    }

    #[test]
    fn every_choice_round_trips_through_its_storage_key() {
        let all = ThemeMode::all();
        assert_eq!(all.len(), 23, "Seguir o sistema + 22 Temas");
        assert_eq!(all[0], ThemeMode::System);
        for mode in all {
            assert_eq!(ThemeMode::from_storage(mode.storage_key()), Some(mode));
        }
    }

    #[test]
    fn follow_the_system_only_switches_between_the_factory_themes() {
        for dark in [Some(true), Some(false), None] {
            let id = resolve_theme(ThemeMode::System, dark);
            assert!(id == ThemeId::DARK || id == ThemeId::LIGHT);
        }
        let feltro = ThemeId::from_key("feltro").expect("feltro");
        assert_eq!(resolve_theme(ThemeMode::Fixed(feltro), Some(false)), feltro);
        assert_eq!(resolve_theme(ThemeMode::Fixed(feltro), None), feltro);
    }

    #[test]
    fn defaults_are_system_comfortable_and_100_percent() {
        let a = Appearance::default();
        assert_eq!(a.theme, ThemeMode::System);
        assert_eq!(a.density, Density::Comfortable);
        assert_eq!(a.scale.percent(), 100);
    }
}
