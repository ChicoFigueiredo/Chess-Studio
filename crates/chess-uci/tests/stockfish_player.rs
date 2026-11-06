//! A Engine adversária com Stockfish real (ADR-0003: o binário nunca é versionado).
//!
//! Ignorado por padrão. Para rodar:
//!
//! ```text
//! CHESS_STUDIO_STOCKFISH=~/.cache/chess-studio-dev/stockfish/stockfish \
//!   cargo test -p chess-uci --test stockfish_player -- --ignored --nocapture
//! ```
//!
//! Sem a variável, procura `stockfish` no `PATH`; se não achar, o teste avisa e passa.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chess_core::Color;
use chess_game::{Command, EngineSeat, Game, GameSetup, PolicyKind, TimeControl, Timestamp};
use chess_uci::{EngineConfig, EnginePlayer, PlayerStatus, StrengthApplied};

fn find_stockfish() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CHESS_STUDIO_STOCKFISH") {
        return Some(PathBuf::from(path));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join("stockfish"))
        .find(|p| p.is_file())
}

fn now(start: Instant) -> Timestamp {
    Timestamp::mono(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX))
}

#[test]
#[ignore = "exige Stockfish instalado (CHESS_STUDIO_STOCKFISH ou PATH)"]
fn real_stockfish_plays_black_with_clock_and_elo() {
    let Some(path) = find_stockfish() else {
        eprintln!("Stockfish não encontrado; defina CHESS_STUDIO_STOCKFISH. Teste ignorado.");
        return;
    };
    let seat = EngineSeat {
        color: Color::Black,
        elo: 1500,
    };
    let mut game = Game::new(
        GameSetup::standard(PolicyKind::FideSimulation)
            .with_time_control(TimeControl {
                base_ms: 60_000,
                increment_ms: 1_000,
            })
            .with_engine(seat),
    );
    let mut player = EnginePlayer::new(seat, EngineConfig::new(path));
    let start = Instant::now();
    // Humano roteirizado: o último Lance legal de cada vez (Simulação FIDE: tocar,
    // soltar e acionar o relógio).
    for round in 0..6 {
        let snap = game.snapshot();
        let mv = snap.position.legal_moves().into_iter().last().unwrap();
        if mv.promotion().is_some() {
            break;
        }
        game.handle(Command::Touch { square: mv.from() }, now(start))
            .unwrap();
        game.handle(
            Command::Release {
                from: mv.from(),
                to: mv.to(),
            },
            now(start),
        )
        .unwrap();
        game.handle(Command::PressClock, now(start)).unwrap();
        let want = 2 * (round + 1);
        let deadline = Instant::now() + Duration::from_secs(30);
        while game.snapshot().history.len() < want && game.snapshot().outcome.is_none() {
            assert!(Instant::now() < deadline, "Stockfish não respondeu");
            assert!(
                !matches!(player.status(), PlayerStatus::Failed(_)),
                "{:?}",
                player.status()
            );
            player.drive(&mut game, now(start));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let snap = game.snapshot();
    assert!(snap.history.len() >= 6, "{:?}", player.status());
    assert!(
        player
            .engine_name()
            .is_some_and(|n| n.starts_with("Stockfish")),
        "{:?}",
        player.engine_name()
    );
    assert!(matches!(
        player.strength(),
        Some(StrengthApplied::Elo { .. })
    ));
    let moves: Vec<&str> = snap.history.iter().map(|h| h.san.as_str()).collect();
    eprintln!(
        "{:?}, força {:?}: {}",
        player.engine_name(),
        player.strength(),
        moves.join(" ")
    );
}
