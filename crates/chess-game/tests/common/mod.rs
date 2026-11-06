#![allow(clippy::unwrap_used)]
//! Apoio dos testes: relógio falso e atalhos por SAN. Nenhum relógio do sistema.
#![allow(dead_code)]

use chess_core::{Piece, Square};
use chess_game::{
    Command, Event, EventKind, Game, GameSetup, PolicyKind, Rejection, TimeControl, Timestamp,
};

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

// ---- Simulação FIDE -------------------------------------------------------

pub fn fide() -> Game {
    Game::new(GameSetup::standard(PolicyKind::FideSimulation))
}

pub fn fide_from(fen: &str) -> Game {
    Game::new(GameSetup::from_fen(PolicyKind::FideSimulation, fen).expect("FEN válido"))
}

pub fn timed(setup: GameSetup, base_ms: u64, increment_ms: u64) -> Game {
    Game::new(setup.with_time_control(TimeControl {
        base_ms,
        increment_ms,
    }))
}

pub fn fide_timed_from(fen: &str, base_ms: u64, increment_ms: u64) -> Game {
    timed(
        GameSetup::from_fen(PolicyKind::FideSimulation, fen).expect("FEN válido"),
        base_ms,
        increment_ms,
    )
}

pub fn at(ms: u64) -> Timestamp {
    Timestamp::mono(ms)
}

pub fn touch(game: &mut Game, ms: u64, square: &str) -> Result<Vec<Event>, Rejection> {
    game.handle(Command::Touch { square: sq(square) }, at(ms))
}

pub fn drop_piece(game: &mut Game, ms: u64, from: &str, to: &str) -> Result<Vec<Event>, Rejection> {
    game.handle(
        Command::Release {
            from: sq(from),
            to: sq(to),
        },
        at(ms),
    )
}

pub fn press(game: &mut Game, ms: u64) -> Result<Vec<Event>, Rejection> {
    game.handle(Command::PressClock, at(ms))
}

/// Jogada FIDE completa: toca, solta (escolhe a promoção) e aciona o relógio.
/// Cada passo avança 1 ms a partir de `ms`; devolve o instante seguinte.
pub fn fide_play(game: &mut Game, ms: u64, san: &str) -> u64 {
    let mv = game.snapshot().position.parse_san(san).expect("SAN válido");
    touch(game, ms, &mv.from().to_string()).expect("toque");
    drop_piece(game, ms + 1, &mv.from().to_string(), &mv.to().to_string()).expect("soltura");
    let mut t = ms + 2;
    if let Some(piece) = mv.promotion() {
        game.handle(Command::ChoosePromotion { piece }, at(t))
            .expect("promoção");
        t += 1;
    }
    if game.snapshot().outcome.is_none() {
        press(game, t).expect("relógio");
    }
    t + 1
}

pub fn fide_play_all(game: &mut Game, mut ms: u64, sans: &str) -> u64 {
    for san in sans.split_whitespace() {
        ms = fide_play(game, ms, san);
    }
    ms
}
