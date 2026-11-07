//! Súmula manual (V2-SUM-01): o registro que o jogador escreve, Lance a Lance, como
//! numa competição física, **sem** ver a notação automática.
//!
//! É um registro **separado** da Partida (ADR-0001/0002): este módulo não conhece
//! [`chess_game::Game`], o log de eventos nem o PGN, então não há caminho de código por
//! onde a Súmula possa alterá-los. Guarda o texto **cru**, como foi escrito (`"e4"`,
//! `"Cf3"`, `"O-O"`): interpretar, normalizar e comparar com os Lances reais é a
//! conferência (V2-SUM-02, ticket seguinte), que lê [`ManualScoresheet::entries`].
//!
//! O tempo entra por parâmetro (sem `Instant::now()`): quem entrega a Súmula informa o
//! instante. Depois de entregue ela é imutável, para a conferência comparar sempre o
//! mesmo texto.
//!
//! ```
//! use chess_training::ManualScoresheet;
//!
//! let mut sheet = ManualScoresheet::new();
//! sheet.write(1, " e4 ").unwrap();
//! sheet.write(2, "e5").unwrap();
//! assert_eq!(sheet.text(1), Some("e4"));
//! sheet.deliver(1_000).unwrap();
//! assert!(sheet.write(3, "Cf3").is_err());
//! ```

use std::collections::BTreeMap;
use std::fmt;

/// Por que uma escrita na Súmula foi recusada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoresheetError {
    /// A Súmula já foi entregue: não aceita mais mudanças.
    AlreadyDelivered,
    /// Meio-lance fora de `1..=MAX_PLY`.
    PlyOutOfRange,
    /// Texto maior que [`MAX_ENTRY_CHARS`] caracteres.
    EntryTooLong,
    /// Texto com quebra de linha ou outro caractere de controle.
    ControlCharacter,
}

impl fmt::Display for ScoresheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyDelivered => "a Súmula já foi entregue",
            Self::PlyOutOfRange => "meio-lance fora do intervalo da Súmula",
            Self::EntryTooLong => "anotação longa demais para um Lance",
            Self::ControlCharacter => "anotação com caractere de controle",
        })
    }
}

impl std::error::Error for ScoresheetError {}

/// Maior número de meios-lances que uma Súmula comporta.
pub const MAX_PLY: u16 = 1000;

/// Maior anotação por Lance, em caracteres (uma SAN longa tem ~8; sobra folga para
/// escrita manual com marcas como `!?` ou `+`).
pub const MAX_ENTRY_CHARS: usize = 24;

/// Súmula manual de uma Partida: o texto escrito para cada meio-lance.
///
/// Meio-lance (`ply`) a partir de 1, como em `HistoryEntry::ply`: ímpar é das brancas,
/// par é das pretas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManualScoresheet {
    entries: BTreeMap<u16, String>,
    delivered_at_ms: Option<i64>,
}

impl ManualScoresheet {
    /// Súmula em branco, em anotação.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstrói uma Súmula gravada. Valida cada anotação como [`Self::write`] faz.
    pub fn from_parts(
        entries: impl IntoIterator<Item = (u16, String)>,
        delivered_at_ms: Option<i64>,
    ) -> Result<Self, ScoresheetError> {
        let mut sheet = Self::new();
        for (ply, text) in entries {
            sheet.write(ply, &text)?;
        }
        sheet.delivered_at_ms = delivered_at_ms;
        Ok(sheet)
    }

    /// Escreve (ou, com texto em branco, apaga) a anotação do meio-lance. Espaços nas
    /// pontas são descartados; o resto fica exatamente como o jogador escreveu.
    pub fn write(&mut self, ply: u16, text: &str) -> Result<(), ScoresheetError> {
        if self.is_delivered() {
            return Err(ScoresheetError::AlreadyDelivered);
        }
        if !(1..=MAX_PLY).contains(&ply) {
            return Err(ScoresheetError::PlyOutOfRange);
        }
        let text = text.trim();
        if text.chars().any(char::is_control) {
            return Err(ScoresheetError::ControlCharacter);
        }
        if text.chars().count() > MAX_ENTRY_CHARS {
            return Err(ScoresheetError::EntryTooLong);
        }
        if text.is_empty() {
            self.entries.remove(&ply);
        } else {
            self.entries.insert(ply, text.to_owned());
        }
        Ok(())
    }

    /// A anotação do meio-lance, se houver.
    pub fn text(&self, ply: u16) -> Option<&str> {
        self.entries.get(&ply).map(String::as_str)
    }

    /// Anotações em ordem de meio-lance (a entrada da conferência).
    pub fn entries(&self) -> impl Iterator<Item = (u16, &str)> {
        self.entries.iter().map(|(p, t)| (*p, t.as_str()))
    }

    /// Quantos meios-lances têm anotação.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entrega a Súmula (como entregá-la ao árbitro): congela o texto. Entregar de novo
    /// é recusado e não muda o instante.
    pub fn deliver(&mut self, at_ms: i64) -> Result<(), ScoresheetError> {
        if self.is_delivered() {
            return Err(ScoresheetError::AlreadyDelivered);
        }
        self.delivered_at_ms = Some(at_ms);
        Ok(())
    }

    pub fn is_delivered(&self) -> bool {
        self.delivered_at_ms.is_some()
    }

    pub fn delivered_at_ms(&self) -> Option<i64> {
        self.delivered_at_ms
    }

    /// Enquanto a Súmula está em anotação, a notação automática fica oculta. Depois de
    /// entregue, a conferência pode mostrá-la.
    pub fn hides_automatic_notation(&self) -> bool {
        !self.is_delivered()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_trims_keeps_the_raw_text_and_blank_clears() {
        let mut sheet = ManualScoresheet::new();
        sheet.write(1, "  Cf3 ").unwrap();
        sheet.write(2, "o-o").unwrap();
        assert_eq!(sheet.text(1), Some("Cf3"));
        assert_eq!(sheet.text(2), Some("o-o"), "não normaliza: é a conferência");
        sheet.write(1, "   ").unwrap();
        assert_eq!(sheet.text(1), None);
        assert_eq!(sheet.len(), 1);
    }

    #[test]
    fn entries_come_in_ply_order() {
        let mut sheet = ManualScoresheet::new();
        sheet.write(3, "Cf3").unwrap();
        sheet.write(1, "e4").unwrap();
        let all: Vec<_> = sheet.entries().collect();
        assert_eq!(all, vec![(1, "e4"), (3, "Cf3")]);
    }

    #[test]
    fn invalid_writes_are_rejected_and_change_nothing() {
        let mut sheet = ManualScoresheet::new();
        assert_eq!(sheet.write(0, "e4"), Err(ScoresheetError::PlyOutOfRange));
        assert_eq!(
            sheet.write(MAX_PLY + 1, "e4"),
            Err(ScoresheetError::PlyOutOfRange)
        );
        assert_eq!(
            sheet.write(1, &"x".repeat(MAX_ENTRY_CHARS + 1)),
            Err(ScoresheetError::EntryTooLong)
        );
        assert_eq!(
            sheet.write(1, "e4\ne5"),
            Err(ScoresheetError::ControlCharacter)
        );
        assert!(sheet.is_empty());
        assert!(sheet.write(1, &"x".repeat(MAX_ENTRY_CHARS)).is_ok());
    }

    #[test]
    fn delivering_freezes_the_sheet_and_ends_the_notation_hiding() {
        let mut sheet = ManualScoresheet::new();
        sheet.write(1, "e4").unwrap();
        assert!(sheet.hides_automatic_notation());
        sheet.deliver(5_000).unwrap();
        assert!(!sheet.hides_automatic_notation());
        assert_eq!(sheet.delivered_at_ms(), Some(5_000));
        assert_eq!(sheet.write(2, "e5"), Err(ScoresheetError::AlreadyDelivered));
        assert_eq!(sheet.deliver(9_000), Err(ScoresheetError::AlreadyDelivered));
        assert_eq!(sheet.delivered_at_ms(), Some(5_000));
        assert_eq!(sheet.text(1), Some("e4"));
    }

    #[test]
    fn from_parts_round_trips_and_validates() {
        let sheet =
            ManualScoresheet::from_parts([(1, "e4".to_owned()), (2, "e5".to_owned())], Some(7))
                .unwrap();
        assert!(sheet.is_delivered());
        assert_eq!(sheet.len(), 2);
        assert!(ManualScoresheet::from_parts([(0, "e4".to_owned())], None).is_err());
    }
}
