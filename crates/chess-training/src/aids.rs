//! Registro dos Auxílios visuais de uma Resolução (TRN-03).

use serde::{Deserialize, Serialize};

/// Um Auxílio visual do Perfil de auxílios (marcação que não existiria na mesa física).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aid {
    Destinations,
    Ghosts,
    SelectedPiece,
    LastMove,
    Check,
}

impl Aid {
    pub const ALL: [Aid; 5] = [
        Aid::Destinations,
        Aid::Ghosts,
        Aid::SelectedPiece,
        Aid::LastMove,
        Aid::Check,
    ];
}

/// Quando explicar um erro (preferência do usuário).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feedback {
    #[default]
    Immediate,
    AfterAttempt,
    ReviewOnly,
}

/// Quais Auxílios foram usados e quais ficaram desligados numa Resolução.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AidsRecord {
    /// Ligados em algum momento da Resolução.
    pub used: Vec<Aid>,
    /// Desligados durante toda a Resolução.
    pub off: Vec<Aid>,
    pub feedback: Feedback,
}

impl AidsRecord {
    /// `enabled` são os Auxílios ligados; todos os demais ficam registrados como desligados.
    pub fn new(enabled: &[Aid], feedback: Feedback) -> Self {
        Self {
            used: Aid::ALL
                .into_iter()
                .filter(|a| enabled.contains(a))
                .collect(),
            off: Aid::ALL
                .into_iter()
                .filter(|a| !enabled.contains(a))
                .collect(),
            feedback,
        }
    }

    /// Exercício sem nenhum Auxílio.
    pub fn without_aids(&self) -> bool {
        self.used.is_empty()
    }

    /// Soma o que `later` mostra ligado agora (um Auxílio ligado no meio da Resolução
    /// conta como usado) e adota o feedback mais recente.
    pub(crate) fn merge(&mut self, later: &AidsRecord) {
        let enabled: Vec<Aid> = Aid::ALL
            .into_iter()
            .filter(|a| self.used.contains(a) || later.used.contains(a))
            .collect();
        *self = Self::new(&enabled, later.feedback);
    }
}
