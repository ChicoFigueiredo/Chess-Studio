//! Filtro de Exercícios no Treino: por pacote, tema e dificuldade.

use crate::exercise::Exercise;

/// Critérios combinados com "e"; campo vazio não filtra.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExerciseFilter {
    /// `Some(None)` só os de fábrica; `Some(Some(id))` só o pacote; `None` todos.
    pub pack: Option<Option<String>>,
    /// Tema exato (sem diferenciar maiúsculas); vazio não filtra.
    pub theme: String,
    pub min_rating: Option<u32>,
    pub max_rating: Option<u32>,
}

impl ExerciseFilter {
    pub fn matches(&self, exercise: &Exercise) -> bool {
        if let Some(pack) = &self.pack
            && &exercise.pack != pack
        {
            return false;
        }
        let theme = self.theme.trim();
        if !theme.is_empty()
            && !exercise
                .themes
                .iter()
                .any(|t| t.eq_ignore_ascii_case(theme))
        {
            return false;
        }
        if self.min_rating.is_some() || self.max_rating.is_some() {
            // Exercício sem rating (de fábrica) não entra numa faixa de dificuldade.
            let Some(rating) = exercise.rating else {
                return false;
            };
            if self.min_rating.is_some_and(|min| rating < min)
                || self.max_rating.is_some_and(|max| rating > max)
            {
                return false;
            }
        }
        true
    }
}
