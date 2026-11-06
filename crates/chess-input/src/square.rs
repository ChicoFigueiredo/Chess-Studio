//! Casa do tabuleiro provisória.
//!
//! TODO(A2): substituir por `chess_core::Square` quando existir; este módulo é o único
//! ponto que precisa mudar (a geometria só usa `file()`, `rank()` e `new`).

use std::fmt;

/// Casa a1..h8. `file` 0 = coluna a, `rank` 0 = fila 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoardSquare {
    file: u8,
    rank: u8,
}

impl BoardSquare {
    /// `None` se `file` ou `rank` estiver fora de 0..8.
    pub fn new(file: u8, rank: u8) -> Option<Self> {
        (file < 8 && rank < 8).then_some(Self { file, rank })
    }

    /// Coluna 0..8 (a..h).
    pub fn file(self) -> u8 {
        self.file
    }

    /// Fila 0..8 (1..8).
    pub fn rank(self) -> u8 {
        self.rank
    }

    /// As 64 casas, de a1 a h8 (fila a fila).
    pub fn all() -> impl Iterator<Item = BoardSquare> {
        (0..8).flat_map(|rank| (0..8).map(move |file| BoardSquare { file, rank }))
    }
}

impl fmt::Display for BoardSquare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", char::from(b'a' + self.file), self.rank + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cover_a1_to_h8() {
        let names: Vec<String> = BoardSquare::all().map(|s| s.to_string()).collect();
        assert_eq!(names.len(), 64);
        assert_eq!(names[0], "a1");
        assert_eq!(names[7], "h1");
        assert_eq!(names[63], "h8");
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(BoardSquare::new(8, 0).is_none());
        assert!(BoardSquare::new(0, 8).is_none());
        assert_eq!(BoardSquare::new(4, 3).unwrap().to_string(), "e4");
    }
}
