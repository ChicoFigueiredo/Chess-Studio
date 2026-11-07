use crate::PgnGame;
use chess_core::FenError;
use chess_game::{Game, GameSetup, PolicyKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    InvalidFen(FenError),
    IllegalMove { ply: usize, san: String },
    Rejected { ply: usize, san: String },
}

impl PgnGame {
    pub fn to_assisted_game(&self) -> Result<Game, ConvertError> {
        Ok(Game::new(GameSetup::standard(PolicyKind::Assisted)))
    }
}
