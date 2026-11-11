//! O que o Painel Explorador lembra entre sessões, por Perfil local (#231): a aba, a fonte, o
//! jogador consultado, a cor, os ritmos e as faixas de rating. Vai para as preferências do
//! Perfil local (`ui_preferences`, chave [`KEY`]) como um JSON pequeno; nunca guarda Posição,
//! resultado ou autorização.

use super::driver::Explorer;
use super::model::{Source, Tab};
use chess_core::Color;
use chess_store::Pace;
use serde_json::{Value, json};

/// Chave da preferência no Perfil local.
pub const KEY: &str = "explorer.panel";

/// Pisos de rating que o Explorador do Lichess aceita.
pub const RATING_BANDS: [u16; 9] = [0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500];

impl Tab {
    fn key(self) -> &'static str {
        match self {
            Tab::Openings => "openings",
            Tab::Endgames => "endgames",
        }
    }

    fn from_key(text: &str) -> Option<Self> {
        [Tab::Openings, Tab::Endgames]
            .into_iter()
            .find(|tab| tab.key() == text)
    }
}

impl Source {
    fn from_key(text: &str) -> Option<Self> {
        Source::ALL.into_iter().find(|source| source.key() == text)
    }
}

impl Explorer {
    /// As escolhas a lembrar, como JSON.
    pub fn prefs(&self) -> Value {
        json!({
            "tab": self.tab.key(),
            "source": self.source.key(),
            "player": self.player.trim(),
            "color": match self.player_color {
                Color::White => "white",
                Color::Black => "black",
            },
            "paces": self.paces.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "ratings": self.ratings,
        })
    }

    /// Aplica o que foi lembrado. Qualquer campo desconhecido, ausente ou fora do esperado é
    /// ignorado: o Painel nunca falha por causa de uma preferência velha ou editada.
    pub fn apply_prefs(&mut self, saved: &Value) {
        if let Some(tab) = saved
            .get("tab")
            .and_then(Value::as_str)
            .and_then(Tab::from_key)
        {
            self.tab = tab;
        }
        if let Some(source) = saved
            .get("source")
            .and_then(Value::as_str)
            .and_then(Source::from_key)
        {
            self.source = source;
        }
        if let Some(player) = saved.get("player").and_then(Value::as_str) {
            self.player = player.chars().take(40).collect();
        }
        match saved.get("color").and_then(Value::as_str) {
            Some("white") => self.player_color = Color::White,
            Some("black") => self.player_color = Color::Black,
            _ => {}
        }
        if let Some(paces) = saved.get("paces").and_then(Value::as_array) {
            self.paces = paces
                .iter()
                .filter_map(Value::as_str)
                .filter_map(Pace::parse)
                .collect();
        }
        if let Some(ratings) = saved.get("ratings").and_then(Value::as_array) {
            self.ratings = ratings
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|r| u16::try_from(r).ok())
                .filter(|r| RATING_BANDS.contains(r))
                .collect();
        }
    }

    /// Liga ou desliga um ritmo no filtro.
    pub fn toggle_pace(&mut self, pace: Pace) {
        match self.paces.iter().position(|p| *p == pace) {
            Some(index) => {
                self.paces.remove(index);
            }
            None => {
                self.paces.push(pace);
                self.paces
                    .sort_by_key(|p| Pace::ALL.iter().position(|q| q == p));
            }
        }
    }

    /// Liga ou desliga uma faixa de rating no filtro (guardadas em ordem crescente).
    pub fn toggle_rating(&mut self, floor: u16) {
        match self.ratings.iter().position(|r| *r == floor) {
            Some(index) => {
                self.ratings.remove(index);
            }
            None if RATING_BANDS.contains(&floor) => {
                self.ratings.push(floor);
                self.ratings.sort_unstable();
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn the_choices_round_trip() {
        let mut explorer = Explorer::default();
        explorer.tab = Tab::Endgames;
        explorer.source = Source::Player;
        explorer.player = "alguem_exemplo".into();
        explorer.player_color = Color::Black;
        explorer.paces = vec![Pace::Blitz, Pace::Classical];
        explorer.ratings = vec![1800, 2200];
        let saved = explorer.prefs();
        let mut restored = Explorer::default();
        restored.apply_prefs(&saved);
        assert_eq!(restored.prefs(), saved);
        assert_eq!(restored.tab, Tab::Endgames);
        assert_eq!(restored.source, Source::Player);
        assert_eq!(restored.player_color, Color::Black);
        assert_eq!(restored.paces, [Pace::Blitz, Pace::Classical]);
        assert_eq!(restored.ratings, [1800, 2200]);
        explorer.toggle_pace(Pace::Blitz);
        assert_ne!(explorer.prefs(), saved);
    }

    #[test]
    fn a_damaged_or_foreign_preference_never_breaks_the_panel() {
        let mut explorer = Explorer::default();
        explorer.apply_prefs(&json!("lixo"));
        explorer.apply_prefs(&json!({
            "tab": 3, "source": "fonte-que-nao-existe", "color": "verde",
            "paces": ["blitz", "ultra-bullet", 7], "ratings": [1800, 1234, -5, "x", 99999]
        }));
        assert_eq!(explorer.tab, Tab::Openings);
        assert_eq!(explorer.source, Source::Library, "o padrão offline");
        assert_eq!(explorer.player_color, Color::White);
        assert_eq!(explorer.paces, [Pace::Blitz]);
        assert_eq!(explorer.ratings, [1800], "só faixas que o Provedor aceita");
    }

    #[test]
    fn toggling_keeps_the_filters_ordered_and_without_duplicates() {
        let mut explorer = Explorer::default();
        explorer.toggle_pace(Pace::Rapid);
        explorer.toggle_pace(Pace::Bullet);
        assert_eq!(explorer.paces, [Pace::Bullet, Pace::Rapid]);
        explorer.toggle_pace(Pace::Rapid);
        assert_eq!(explorer.paces, [Pace::Bullet]);
        explorer.toggle_rating(2000);
        explorer.toggle_rating(1000);
        explorer.toggle_rating(1234);
        assert_eq!(explorer.ratings, [1000, 2000]);
        explorer.toggle_rating(1000);
        assert_eq!(explorer.ratings, [2000]);
    }
}
