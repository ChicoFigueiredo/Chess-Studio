//! Conversão da linha principal numa `Game` (Política Assistido, para estudo).
//! As variações não entram na `Game` (o log de Eventos é linear, ADR-0002);
//! elas continuam na árvore `PgnGame` para a biblioteca.

use crate::PgnGame;
use chess_core::FenError;
use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConvertError {
    InvalidFen(FenError),
    /// Lance da linha principal ilegal na Posição (árvore adulterada depois de importada).
    IllegalMove {
        ply: usize,
        san: String,
    },
    /// A Partida recusou o Comando (por exemplo, Lance depois do fim da Partida).
    Rejected {
        ply: usize,
        san: String,
    },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFen(e) => write!(f, "FEN inicial inválido: {e}"),
            Self::IllegalMove { ply, san } => {
                write!(f, "meio-lance {ply}: {san:?} é ilegal na posição")
            }
            Self::Rejected { ply, san } => {
                write!(f, "meio-lance {ply}: a Partida recusou {san:?}")
            }
        }
    }
}

impl std::error::Error for ConvertError {}

impl PgnGame {
    /// Reproduz a linha principal numa Partida Assistida (sem relógio). Os instantes
    /// são sintéticos (1 ms por Comando): o PGN não traz tempos.
    pub fn to_assisted_game(&self) -> Result<Game, ConvertError> {
        let setup = GameSetup::from_fen(PolicyKind::Assisted, &self.start_fen)
            .map_err(ConvertError::InvalidFen)?;
        let mut game = Game::new(setup);
        let mut ms = 0_u64;
        for (index, node) in self.moves.iter().enumerate() {
            let ply = index + 1;
            let mv = game.snapshot().position.parse_san(&node.san).map_err(|_| {
                ConvertError::IllegalMove {
                    ply,
                    san: node.san.clone(),
                }
            })?;
            let rejected = || ConvertError::Rejected {
                ply,
                san: node.san.clone(),
            };
            let mut send = |command: Command| {
                ms += 1;
                game.handle(command, Timestamp::mono(ms))
                    .map(|_| ())
                    .map_err(|_| rejected())
            };
            send(Command::Release {
                from: mv.from(),
                to: mv.to(),
            })?;
            if let Some(piece) = mv.promotion() {
                send(Command::ChoosePromotion { piece })?;
            }
        }
        Ok(game)
    }
}
