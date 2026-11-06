//! Tempo injetado (ADR-0002): o domínio nunca lê o relógio do sistema.

use serde::{Deserialize, Serialize};

/// Instante de um Comando ou Evento.
///
/// `mono_ms` é monotônico, em milissegundos desde o início da Partida, e é o
/// único que as regras usam. `wall_ms` (relógio de parede, ms desde a época Unix)
/// é só metadado para o histórico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    pub mono_ms: u64,
    #[serde(default)]
    pub wall_ms: Option<i64>,
}

impl Timestamp {
    /// Instante só com a parte monotônica (testes, relógio falso).
    pub fn mono(mono_ms: u64) -> Self {
        Self {
            mono_ms,
            wall_ms: None,
        }
    }

    /// Instante com relógio de parede para metadados.
    pub fn with_wall(mono_ms: u64, wall_ms: i64) -> Self {
        Self {
            mono_ms,
            wall_ms: Some(wall_ms),
        }
    }
}

/// Controle de tempo (base + incremento, em ms). A Política Assistido o ignora;
/// o relógio de xadrez é do B6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeControl {
    pub base_ms: u64,
    pub increment_ms: u64,
}
