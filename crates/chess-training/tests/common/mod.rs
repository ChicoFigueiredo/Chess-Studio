#![allow(dead_code, clippy::unwrap_used)]
use chess_core::Square;
use chess_game::{Command, Game, Timestamp};
use chess_training::{AidsRecord, Catalog, Exercise, ExerciseRun, Feedback};

pub fn exercise(id: &str) -> Exercise {
    Catalog::bundled().unwrap().get(id).unwrap().clone()
}

pub fn no_aids() -> AidsRecord {
    AidsRecord::new(&[], Feedback::Immediate)
}

/// Resolução nova sobre a Partida do Exercício.
pub fn start(id: &str, aids: AidsRecord) -> (ExerciseRun, Game) {
    let ex = exercise(id);
    let game = ex.new_game().unwrap();
    (ExerciseRun::start(&ex, aids, 0), game)
}

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

/// O jogador solta a peça (`e2e4`) e o juiz do Exercício reage.
pub fn release(run: &mut ExerciseRun, game: &mut Game, uci: &str, at: u64) {
    game.handle(
        Command::Release {
            from: sq(&uci[0..2]),
            to: sq(&uci[2..4]),
        },
        Timestamp::mono(at),
    )
    .unwrap();
    if let Some(p) = uci.get(4..5) {
        let piece = match p {
            "q" => chess_core::Piece::Queen,
            "n" => chess_core::Piece::Knight,
            "r" => chess_core::Piece::Rook,
            _ => chess_core::Piece::Bishop,
        };
        game.handle(Command::ChoosePromotion { piece }, Timestamp::mono(at))
            .unwrap();
    }
    run.drive(game, at);
}
