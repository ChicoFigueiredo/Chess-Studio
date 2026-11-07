//! Análise com Stockfish real (ADR-0003: o binário nunca entra no repositório). Ignorado por
//! padrão; veja `tests/stockfish.rs` para a variável `CHESS_STUDIO_STOCKFISH`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chess_uci::{
    AnalysisConfig, AnalysisController, AnalysisEvent, AnalysisUpdate, EngineConfig, PositionSpec,
    Score,
};

fn find_stockfish() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CHESS_STUDIO_STOCKFISH") {
        return Some(PathBuf::from(path));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("stockfish"))
        .find(|p| p.is_file())
}

fn reached(ctrl: &mut AnalysisController, id: u64, depth: u32, lines: usize) -> AnalysisUpdate {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            match event {
                AnalysisEvent::Update(u)
                    if u.id == id
                        && u.lines.len() == lines
                        && u.lines.iter().all(|l| l.depth >= depth) =>
                {
                    return u;
                }
                AnalysisEvent::Failed(f) => panic!("{f:?}"),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("profundidade {depth} não foi alcançada");
}

#[test]
#[ignore = "exige Stockfish instalado (CHESS_STUDIO_STOCKFISH ou PATH)"]
fn real_stockfish_analyses_with_multipv_and_switches_position() {
    let Some(path) = find_stockfish() else {
        eprintln!("Stockfish não encontrado; defina CHESS_STUDIO_STOCKFISH. Teste ignorado.");
        return;
    };
    let mut config = AnalysisConfig::new(EngineConfig::new(path));
    config.threads = Some(1);
    config.hash_mb = Some(16);
    let mut ctrl = AnalysisController::start(config);
    let id = ctrl.analyze(PositionSpec::startpos(), 3);
    let update = reached(&mut ctrl, id, 10, 3);
    assert!(update.lines.iter().all(|l| !l.san.is_empty()));
    assert!(matches!(update.lines[0].score, Score::Cp(_)));
    // Mate em 1 para as brancas (Ra8#): o placar é de brancas.
    let id = ctrl.analyze(PositionSpec::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1"), 1);
    let update = reached(&mut ctrl, id, 5, 1);
    assert_eq!(update.lines[0].score, Score::Mate(1));
    assert_eq!(update.lines[0].san[0], "Ra8#");
}
