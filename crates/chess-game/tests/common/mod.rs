#![allow(clippy::unwrap_used)]
//! Apoio dos testes: relógio falso e atalhos por SAN. Nenhum relógio do sistema.
#![allow(dead_code)]

use chess_core::{Piece, Square};
use chess_game::{Command, Event, EventKind, Game, GameSetup, PolicyKind, Rejection, Timestamp};

/// Relógio falso: cada chamada avança 1 s.
pub struct FakeClock(u64);

impl FakeClock {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn now(&mut self) -> Timestamp {
        self.0 += 1000;
        Timestamp::mono(self.0)
    }
}

pub fn sq(s: &str) -> Square {
    s.parse().expect("casa válida")
}

pub fn assisted() -> Game {
    Game::new(GameSetup::standard(PolicyKind::Assisted))
}

pub fn assisted_from(fen: &str) -> Game {
    Game::new(GameSetup::from_fen(PolicyKind::Assisted, fen).expect("FEN válido"))
}

pub fn release(game: &mut Game, clock: &mut FakeClock, from: &str, to: &str) -> Vec<Event> {
    game.handle(
        Command::Release {
            from: sq(from),
            to: sq(to),
        },
        clock.now(),
    )
    .expect("comando aceito")
}

/// Joga um Lance em SAN pelo Comando `Release` (e `ChoosePromotion` se preciso).
pub fn play_san(game: &mut Game, clock: &mut FakeClock, san: &str) -> Vec<Event> {
    let mv = game
        .snapshot()
        .position
        .parse_san(san)
        .unwrap_or_else(|e| panic!("SAN {san}: {e}"));
    let mut events = release(game, clock, &mv.from().to_string(), &mv.to().to_string());
    if let Some(piece) = mv.promotion() {
        events.extend(choose_promotion(game, clock, piece).expect("promoção aceita"));
    }
    events
}

pub fn choose_promotion(
    game: &mut Game,
    clock: &mut FakeClock,
    piece: Piece,
) -> Result<Vec<Event>, Rejection> {
    game.handle(Command::ChoosePromotion { piece }, clock.now())
}

pub fn play_all(game: &mut Game, clock: &mut FakeClock, sans: &str) {
    for san in sans.split_whitespace() {
        play_san(game, clock, san);
    }
}

pub fn kinds(events: &[Event]) -> Vec<&EventKind> {
    events.iter().map(|e| &e.kind).collect()
}

/// Ciclo de cavalos que devolve a mesma Posição (4 meio-lances).
pub const KNIGHT_CYCLE: &str = "Nf3 Nf6 Ng1 Ng8";
