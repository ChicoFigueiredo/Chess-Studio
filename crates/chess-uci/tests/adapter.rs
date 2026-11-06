//! Contrato do adaptador, observado pela interface pública contra a Engine falsa.
//! UCI-02 (timeout/crash) e UCI-03 (Lance ilegal) no nível do adaptador.

use std::time::{Duration, Instant};

use chess_core::CanonicalPosition;
use chess_uci::{
    BestMove, Clock, Engine, EngineConfig, EngineError, EngineMove, GoParams, LogSource,
    PositionSpec, QuitOutcome, Stage, Timeouts,
};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn short_timeouts() -> Timeouts {
    Timeouts {
        handshake: ms(600),
        ready: ms(600),
        search_slack: ms(100),
        search_default: ms(300),
        stop_grace: ms(150),
        quit: ms(300),
    }
}

fn config(mode: &str) -> EngineConfig {
    EngineConfig::new(FAKE)
        .arg("--mode")
        .arg(mode)
        .timeouts(short_timeouts())
}

fn spawn(mode: &str) -> Engine {
    Engine::spawn(config(mode)).expect("Engine falsa deve completar o handshake")
}

fn start_spec() -> PositionSpec {
    PositionSpec::startpos()
}

// --- handshake ---------------------------------------------------------------

#[test]
fn handshake_reports_identity_and_announced_options() {
    let engine = spawn("normal");
    let info = engine.info();
    assert_eq!(info.name.as_deref(), Some("FakeUci 1.0"));
    assert_eq!(info.author.as_deref(), Some("Chess Studio tests"));
    let skill = info
        .options
        .iter()
        .find(|o| o.name == "Skill Level")
        .expect("opção anunciada");
    assert_eq!(skill.kind, "spin");
    assert!(skill.details.contains("max 20"));
    assert!(info.options.iter().any(|o| o.name == "UCI_LimitStrength"));
}

#[test]
fn handshake_tolerates_garbage_lines() {
    let mut engine = spawn("garbage");
    engine.is_ready().expect("readyok");
    engine.set_position(&start_spec()).unwrap();
    let best = engine.search(&GoParams::movetime(ms(50))).unwrap();
    assert!(matches!(best, BestMove::Move { .. }));
}

#[test]
fn handshake_times_out_when_uciok_never_arrives() {
    let started = Instant::now();
    let err = Engine::spawn(config("hang-handshake")).unwrap_err();
    assert!(
        matches!(
            err,
            EngineError::Timeout {
                stage: Stage::Handshake,
                killed: true,
                ..
            }
        ),
        "{err}"
    );
    assert!(started.elapsed() < ms(2000));
}

#[test]
fn handshake_times_out_when_readyok_never_arrives() {
    let err = Engine::spawn(config("no-ready")).unwrap_err();
    assert!(
        matches!(
            err,
            EngineError::Timeout {
                stage: Stage::Ready,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn spawn_reports_missing_executable() {
    let err = Engine::spawn(EngineConfig::new("/nao/existe/engine")).unwrap_err();
    assert!(matches!(err, EngineError::Spawn { .. }), "{err}");
    assert!(err.to_string().contains("/nao/existe/engine"));
}

// --- busca -------------------------------------------------------------------

#[test]
fn search_returns_bestmove_for_the_given_position() {
    let mut engine = spawn("normal");
    engine.new_game().unwrap();
    engine.is_ready().unwrap();
    // Depois de 1.a3 a7 o primeiro lance legal em ordem alfabética das Brancas é a3a4.
    let spec = start_spec().with_moves(["a2a3".parse().unwrap(), "a7a6".parse().unwrap()]);
    engine.set_position(&spec).unwrap();
    let best = engine.search(&GoParams::movetime(ms(50))).unwrap();
    assert_eq!(
        best,
        BestMove::Move {
            text: "a3a4".into(),
            ponder: None
        }
    );
}

#[test]
fn commands_are_sent_in_uci_syntax() {
    let mut engine = spawn("normal");
    engine.set_option("Skill Level", Some("5")).unwrap();
    engine.set_position(&start_spec()).unwrap();
    let clock = Clock {
        wtime: ms(60_000),
        btime: ms(59_000),
        winc: ms(1_000),
        binc: ms(2_000),
    };
    engine.search(&GoParams::clock(clock)).unwrap();
    engine.search(&GoParams::depth(3)).unwrap();
    engine
        .set_position(&PositionSpec::from_fen("8/8/8/8/8/8/8/K1k5 w - - 0 1"))
        .unwrap();

    let sent: Vec<String> = engine
        .log()
        .into_iter()
        .filter(|e| e.source == LogSource::ToEngine)
        .map(|e| e.line)
        .collect();
    for expected in [
        "setoption name Skill Level value 5",
        "position startpos",
        "go wtime 60000 btime 59000 winc 1000 binc 2000",
        "go depth 3",
        "position fen 8/8/8/8/8/8/8/K1k5 w - - 0 1",
    ] {
        assert!(sent.iter().any(|l| l == expected), "faltou {expected:?}");
    }
}

#[test]
fn stop_ends_an_infinite_search_with_a_bestmove() {
    let mut engine = spawn("normal");
    engine.set_position(&start_spec()).unwrap();
    engine.go(&GoParams::infinite()).unwrap();
    let err = engine.wait_bestmove(ms(60)).unwrap_err();
    assert!(
        matches!(
            err,
            EngineError::Timeout {
                stage: Stage::Search,
                killed: false,
                ..
            }
        ),
        "{err}"
    );
    assert!(err.is_recoverable());
    engine.stop().unwrap();
    let best = engine.wait_bestmove(ms(300)).unwrap();
    assert!(matches!(best, BestMove::Move { .. }));
}

#[test]
fn bestmove_none_means_no_move() {
    let mut engine = spawn("none");
    engine.set_position(&start_spec()).unwrap();
    assert_eq!(
        engine.search(&GoParams::movetime(ms(50))).unwrap(),
        BestMove::None
    );
    let outcome = engine
        .search_legal_move(&CanonicalPosition::start(), &GoParams::movetime(ms(50)))
        .unwrap();
    assert_eq!(outcome, EngineMove::NoMove);
}

#[test]
fn legal_move_is_applied_to_a_new_position_and_the_original_is_untouched() {
    let mut engine = spawn("normal");
    let start = CanonicalPosition::start();
    let outcome = engine
        .search_legal_move(&start, &GoParams::movetime(ms(50)))
        .unwrap();
    let EngineMove::Played { mv, next } = outcome else {
        panic!("esperava um lance");
    };
    assert_eq!(mv.to_string(), "a2a3");
    assert_eq!(next, start.play(mv).unwrap());
    assert_eq!(start, CanonicalPosition::start());
}

// --- falhas ------------------------------------------------------------------

#[test]
fn illegal_move_is_a_recoverable_error_and_leaves_the_position_unchanged() {
    let mut engine = spawn("illegal");
    let position = CanonicalPosition::start();
    let before = position.to_fen();
    let err = engine
        .search_legal_move(&position, &GoParams::movetime(ms(50)))
        .unwrap_err();
    match &err {
        EngineError::IllegalMove { text, fen } => {
            assert_eq!(text, "a1a1");
            assert_eq!(fen, &before);
        }
        other => panic!("esperava IllegalMove, veio {other}"),
    }
    assert!(err.is_recoverable());
    assert_eq!(position.to_fen(), before);
    // A Engine continua utilizável depois do erro.
    assert!(engine.is_alive());
    engine.is_ready().unwrap();
}

#[test]
fn malformed_move_text_is_a_recoverable_error() {
    let mut engine = spawn("malformed");
    let err = engine
        .search_legal_move(&CanonicalPosition::start(), &GoParams::movetime(ms(50)))
        .unwrap_err();
    assert!(
        matches!(&err, EngineError::MalformedMove { text, .. } if text == "xyz"),
        "{err}"
    );
    assert!(err.is_recoverable());
    assert!(engine.is_alive());
}

#[test]
fn hung_engine_is_reported_as_timeout_without_blocking_and_is_killed() {
    let mut engine = spawn("hang");
    engine.set_position(&start_spec()).unwrap();
    let started = Instant::now();
    let err = engine.search(&GoParams::movetime(ms(50))).unwrap_err();
    // movetime 50 + folga 100 + stop_grace 150 = 300 ms de teto.
    assert!(started.elapsed() < ms(1500), "demorou {:?}", started.elapsed());
    assert!(
        matches!(
            err,
            EngineError::Timeout {
                stage: Stage::Search,
                killed: true,
                ..
            }
        ),
        "{err}"
    );
    assert!(!err.is_recoverable());
    assert!(!engine.is_alive());
    assert!(matches!(
        engine.is_ready().unwrap_err(),
        EngineError::Terminated
    ));
}

#[test]
fn crash_during_search_is_detected_and_reported() {
    let mut engine = spawn("crash");
    engine.set_position(&start_spec()).unwrap();
    let started = Instant::now();
    let err = engine.search(&GoParams::movetime(ms(2000))).unwrap_err();
    // Detecta pelo EOF, bem antes do prazo da busca.
    assert!(started.elapsed() < ms(1000), "demorou {:?}", started.elapsed());
    match &err {
        EngineError::Crashed { exit } => {
            let status = exit.as_deref().unwrap_or_default();
            assert!(status.contains('3'), "status: {status}");
        }
        other => panic!("esperava Crashed, veio {other}"),
    }
    assert!(!engine.is_alive());
    // Posição do chamador intacta e comandos seguintes falham de forma tipada.
    assert!(matches!(
        engine.set_position(&start_spec()).unwrap_err(),
        EngineError::Terminated | EngineError::Crashed { .. } | EngineError::Io(_)
    ));
}

#[test]
fn slow_engine_within_budget_is_accepted() {
    let cfg = config("slow").arg("--delay-ms").arg("80");
    let mut engine = Engine::spawn(cfg).unwrap();
    engine.set_position(&start_spec()).unwrap();
    let best = engine.search(&GoParams::movetime(ms(50))).unwrap();
    assert!(matches!(best, BestMove::Move { .. }));
}

#[test]
fn slow_engine_beyond_budget_times_out() {
    let cfg = config("slow").arg("--delay-ms").arg("3000");
    let mut engine = Engine::spawn(cfg).unwrap();
    engine.set_position(&start_spec()).unwrap();
    let started = Instant::now();
    let err = engine.search(&GoParams::movetime(ms(50))).unwrap_err();
    assert!(started.elapsed() < ms(1500));
    assert!(matches!(
        err,
        EngineError::Timeout {
            stage: Stage::Search,
            killed: true,
            ..
        }
    ));
}

#[test]
fn search_while_searching_is_rejected_without_side_effects() {
    let mut engine = spawn("normal");
    engine.set_position(&start_spec()).unwrap();
    engine.go(&GoParams::infinite()).unwrap();
    let err = engine.go(&GoParams::infinite()).unwrap_err();
    assert!(matches!(err, EngineError::SearchInProgress));
    engine.stop().unwrap();
    assert!(engine.wait_bestmove(ms(300)).is_ok());
}

// --- log e encerramento ------------------------------------------------------

#[test]
fn log_keeps_only_the_most_recent_lines() {
    let mut engine = Engine::spawn(config("normal").log_capacity(6)).unwrap();
    engine.set_position(&start_spec()).unwrap();
    for _ in 0..5 {
        engine.search(&GoParams::movetime(ms(10))).unwrap();
    }
    let log = engine.log();
    assert_eq!(log.len(), 6);
    let last = log.last().unwrap();
    assert_eq!(last.source, LogSource::FromEngine);
    assert!(last.line.starts_with("bestmove "));
    // O handshake já saiu do buffer.
    assert!(!log.iter().any(|e| e.line == "uciok"));
}

#[test]
fn quit_ends_the_process_gracefully() {
    let engine = spawn("normal");
    let started = Instant::now();
    assert_eq!(engine.quit(), QuitOutcome::Graceful);
    assert!(started.elapsed() < ms(250));
}

#[test]
fn quit_kills_an_engine_that_ignores_it_after_the_timeout() {
    let engine = spawn("ignore-quit");
    let started = Instant::now();
    assert_eq!(engine.quit(), QuitOutcome::Killed);
    assert!(started.elapsed() < ms(1500));
}
