#![allow(clippy::unwrap_used)]
//! Desempenho: 1000 partidas geradas importam em menos de 2 s (build de release).

use chess_core::CanonicalPosition;
use chess_pgn::parse_pgn;
use std::time::Instant;

/// Partida pseudoaleatória determinística (LCG) com até `plies` meios-lances legais.
fn random_game(seed: u64, plies: usize) -> String {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut pos = CanonicalPosition::start();
    let mut out = format!("[Event \"Gerada {seed}\"]\n[Result \"*\"]\n\n");
    for ply in 0..plies {
        let moves = pos.legal_moves();
        if moves.is_empty() {
            break;
        }
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let mv = moves[(state >> 33) as usize % moves.len()];
        if ply % 2 == 0 {
            out.push_str(&format!("{}. ", ply / 2 + 1));
        }
        out.push_str(&pos.san(mv).unwrap());
        out.push_str(" {c} ");
        pos = pos.play(mv).unwrap();
    }
    out.push_str("*\n\n");
    out
}

#[test]
#[cfg_attr(debug_assertions, ignore = "só mede em release: cargo test --release")]
fn mil_partidas_importam_em_menos_de_dois_segundos() {
    let text: String = (0..1000).map(|i| random_game(i, 80)).collect();
    let start = Instant::now();
    let games = parse_pgn(&text).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(games.len(), 1000);
    assert!(elapsed.as_secs_f64() < 2.0, "demorou {elapsed:?}");
}

#[test]
fn partidas_geradas_sao_importadas_corretamente() {
    let text: String = (0..20).map(|i| random_game(i, 60)).collect();
    let games = parse_pgn(&text).unwrap();
    assert_eq!(games.len(), 20);
    assert!(games.iter().all(|g| !g.moves.is_empty()));
}
