//! Teste com Stockfish real (ADR-0003: o binário nunca entra no repositório).
//!
//! Ignorado por padrão. Para rodar:
//!
//! ```text
//! CHESS_STUDIO_STOCKFISH=/caminho/para/stockfish \
//!   cargo test -p chess-uci --test stockfish -- --ignored
//! ```
//!
//! Sem a variável, procura `stockfish` no `PATH`; se não achar, o teste avisa e passa.

use std::path::PathBuf;
use std::time::Duration;

use chess_core::{CanonicalPosition, Color};
use chess_uci::{BestMove, Engine, EngineConfig, EngineMove, GoParams};

fn find_stockfish() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CHESS_STUDIO_STOCKFISH") {
        return Some(PathBuf::from(path));
    }
    let names: &[&str] = if cfg!(windows) {
        &["stockfish.exe"]
    } else {
        &["stockfish"]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

#[test]
#[ignore = "exige Stockfish instalado (CHESS_STUDIO_STOCKFISH ou PATH)"]
fn real_stockfish_handshake_search_stop_and_self_play() {
    let Some(path) = find_stockfish() else {
        eprintln!("Stockfish não encontrado; defina CHESS_STUDIO_STOCKFISH. Teste ignorado.");
        return;
    };
    let mut engine = Engine::spawn(EngineConfig::new(path)).expect("handshake com o Stockfish");
    assert!(
        engine
            .info()
            .name
            .as_deref()
            .is_some_and(|n| n.starts_with("Stockfish")),
        "{:?}",
        engine.info()
    );
    assert!(
        engine
            .info()
            .options
            .iter()
            .any(|o| o.name == "Skill Level")
    );
    engine.set_option("Skill Level", Some("3")).unwrap();
    engine.new_game().unwrap();

    // Busca com stop.
    engine
        .set_position(&chess_uci::PositionSpec::startpos())
        .unwrap();
    engine.go(&GoParams::infinite()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    engine.stop().unwrap();
    let best = engine.wait_bestmove(Duration::from_secs(30)).unwrap();
    assert!(matches!(best, BestMove::Move { .. }), "{best:?}");

    // Autojogo validado pelo núcleo.
    let go = GoParams::movetime(Duration::from_millis(50));
    let mut position = CanonicalPosition::start();
    for _ in 0..16 {
        match engine.search_legal_move(&position, &go).unwrap() {
            EngineMove::Played { next, .. } => position = next,
            EngineMove::NoMove => break,
        }
    }
    assert_eq!(position.side_to_move(), Color::White);
    assert!(engine.is_alive());
    engine.quit();
}
