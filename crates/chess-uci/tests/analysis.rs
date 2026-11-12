//! Análise contínua (C8): `go infinite` num slot próprio, com `MultiPV`, `info` em SAN do
//! ponto de vista das brancas, atualização limitada, troca de posição e carga limitada.
//! Observada pela interface pública contra a Engine falsa.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chess_core::Move;
use chess_uci::{
    AnalysisConfig, AnalysisController, AnalysisEvent, AnalysisUpdate, EngineConfig, FailureKind,
    Limits, PositionSpec, Score, Stage, Timeouts,
};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Folgas largas, como em `load.rs` (#236): só o teste do `stop` que a Engine não lê
/// exercita um estouro, e ele aperta o próprio `stop_grace`. Com a máquina carregada um
/// prazo curto mata a Engine falsa, que nem chega a ler o `stop` (#296).
fn timeouts() -> Timeouts {
    Timeouts {
        handshake: ms(10_000),
        ready: ms(10_000),
        search_slack: ms(2_000),
        search_default: ms(5_000),
        stop_grace: ms(5_000),
        quit: ms(2_000),
    }
}

fn trace_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "chess-studio-analysis-{}-{name}.trace",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn config(extra: &[&str], trace: Option<&PathBuf>) -> AnalysisConfig {
    let mut engine = EngineConfig::new(FAKE)
        .arg("--info")
        .arg("--options")
        .timeouts(timeouts());
    for arg in extra {
        engine = engine.arg(*arg);
    }
    if let Some(trace) = trace {
        engine = engine.arg("--trace").arg(trace.as_os_str());
    }
    let mut config = AnalysisConfig::new(engine);
    config.interval = ms(40);
    config
}

fn started(config: AnalysisConfig) -> AnalysisController {
    let mut ctrl = AnalysisController::start(config);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Ready { .. }));
    ctrl
}

fn mv(text: &str) -> Move {
    text.parse().unwrap()
}

/// Consome eventos até `pred` aceitar um (ou 30 s): devolve o aceito. Uma falha que `pred`
/// não aceita encerra o teste na hora, com o motivo, em vez de esperar o prazo à toa.
fn wait_for(
    ctrl: &mut AnalysisController,
    mut pred: impl FnMut(&AnalysisEvent) -> bool,
) -> AnalysisEvent {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            if pred(&event) {
                return event;
            }
            assert!(
                !matches!(event, AnalysisEvent::Failed(_)),
                "a análise falhou: {event:?}"
            );
        }
        std::thread::sleep(ms(5));
    }
    panic!("evento esperado não chegou");
}

fn update_at_depth(ctrl: &mut AnalysisController, depth: u32) -> AnalysisUpdate {
    match wait_for(ctrl, |e| match e {
        AnalysisEvent::Update(u) => u.lines.iter().any(|l| l.depth >= depth),
        _ => false,
    }) {
        AnalysisEvent::Update(u) => u,
        other => panic!("{other:?}"),
    }
}

fn trace_lines(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Espera o rastro mostrar que a Engine **leu** o `stop`. O rastro só guarda o que a Engine
/// leu: se ela não lê o `stop` dentro do `stop_grace`, o worker a encerra e avisa com
/// `Failed`, e o `stop` nunca aparece (#296). Por isso a espera também olha os eventos e,
/// se falhar, mostra o rastro inteiro e o estado do controlador.
fn wait_engine_read_stop(ctrl: &mut AnalysisController, trace: &PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut events = Vec::new();
    while !trace_lines(trace).iter().any(|l| l == "stop") {
        while let Some(event) = ctrl.poll() {
            assert!(
                !matches!(event, AnalysisEvent::Failed(_)),
                "a análise falhou antes de a Engine ler o stop: {event:?}; rastro: {:?}",
                trace_lines(trace)
            );
            events.push(event);
        }
        assert!(
            Instant::now() < deadline,
            "stop não chegou à Engine; rastro: {:?}; is_analyzing={}, is_dead={}; eventos: \
             {events:?}",
            trace_lines(trace),
            ctrl.is_analyzing(),
            ctrl.is_dead()
        );
        std::thread::sleep(ms(5));
    }
}

#[test]
fn reports_depth_score_and_principal_lines_in_san_with_multipv() {
    let mut ctrl = started(config(&[], None));
    let id = ctrl.analyze(PositionSpec::startpos(), 2);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.id, id);
    assert_eq!(update.lines.len(), 2, "MultiPV 2");
    let first = &update.lines[0];
    assert_eq!(first.multipv, 1);
    assert_eq!(first.depth, 5);
    assert_eq!(first.seldepth, Some(7));
    assert_eq!(first.score, Score::Cp(35));
    assert_eq!(first.nodes, Some(5000));
    assert_eq!(first.san[0], "a3", "PV em SAN, validada pelo chess-core");
    assert_eq!(first.pv.len(), first.san.len());
    assert_eq!(update.lines[1].multipv, 2);
    assert_eq!(update.lines[1].san[0], "a4");
    assert_eq!(update.lines[1].score, Score::Cp(25));
}

#[test]
fn scores_are_always_from_whites_point_of_view() {
    let mut ctrl = started(config(&[], None));
    // Pretas jogam: a Engine diz +35 para quem joga, ou seja, -35 para as brancas.
    ctrl.analyze(PositionSpec::startpos().with_moves([mv("e2e4")]), 1);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.lines[0].score, Score::Cp(-35));
    assert_eq!(update.lines[0].san[0], "a5");
}

#[test]
fn mate_scores_follow_the_same_convention() {
    let mut ctrl = started(config(&["--mate"], None));
    ctrl.analyze(PositionSpec::startpos(), 1);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.lines[0].score, Score::Mate(2), "brancas dão mate");
    ctrl.analyze(PositionSpec::startpos().with_moves([mv("e2e4")]), 1);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.lines[0].score, Score::Mate(-2), "pretas dão mate");
}

#[test]
fn changing_position_sends_stop_then_a_new_position_and_drops_stale_updates() {
    let trace = trace_path("switch");
    let mut ctrl = started(config(&[], Some(&trace)));
    let first = ctrl.analyze(PositionSpec::startpos(), 1);
    update_at_depth(&mut ctrl, 5);
    let second = ctrl.analyze(PositionSpec::startpos().with_moves([mv("e2e4")]), 1);
    assert_ne!(first, second);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.id, second, "nada da posição antiga");
    std::thread::sleep(ms(150));
    while let Some(event) = ctrl.poll() {
        if let AnalysisEvent::Update(u) = event {
            assert_eq!(u.id, second);
        }
    }
    let lines = trace_lines(&trace);
    let stop = lines.iter().position(|l| l == "stop").expect("stop");
    let moved = lines
        .iter()
        .position(|l| l == "position startpos moves e2e4")
        .expect("position com moves");
    assert!(stop < moved, "stop antes da nova position: {lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some("go infinite"));
}

#[test]
fn stop_pauses_the_analysis_and_resuming_works() {
    let trace = trace_path("pause");
    let mut ctrl = started(config(&[], Some(&trace)));
    ctrl.analyze(PositionSpec::startpos(), 1);
    update_at_depth(&mut ctrl, 5);
    ctrl.stop();
    assert!(!ctrl.is_analyzing());
    // O `stop` chega à Engine de forma assíncrona: espera pelo rastro, em vez de supor o
    // tempo (#236). Atualizações emitidas antes dele podem já estar na fila; descarta-as.
    wait_engine_read_stop(&mut ctrl, &trace);
    while ctrl.poll().is_some() {}
    // Daí em diante, nenhuma atualização nova.
    std::thread::sleep(ms(200));
    while let Some(event) = ctrl.poll() {
        assert!(!matches!(event, AnalysisEvent::Update(_)), "{event:?}");
    }
    ctrl.analyze(PositionSpec::startpos(), 1);
    update_at_depth(&mut ctrl, 5);
}

/// A ordem que o #296 viu: o worker escreve o `stop`, mas a Engine não o lê (sob carga, um
/// processo pode ficar sem CPU além do `stop_grace`). O pedido não se perde em silêncio:
/// passado o `stop_grace`, o worker encerra a Engine e avisa com `Failed` na etapa de parada.
/// Só o rastro, que guarda o que a Engine leu, fica sem o `stop`.
#[test]
fn a_stop_the_engine_never_reads_ends_the_engine_with_a_timeout() {
    let trace = trace_path("deaf");
    let mut cfg = config(&["--deaf-after-infinite"], Some(&trace));
    cfg.engine.timeouts.stop_grace = ms(300);
    let mut ctrl = started(cfg);
    ctrl.analyze(PositionSpec::startpos(), 1);
    update_at_depth(&mut ctrl, 5);
    ctrl.stop();
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Timeout, "{failure:?}");
    assert!(
        failure.message.contains(&Stage::Stop.to_string()),
        "o stop foi enviado e o prazo dele estourou: {failure:?}"
    );
    assert!(ctrl.is_dead());
    let lines = trace_lines(&trace);
    assert_eq!(
        lines.last().map(String::as_str),
        Some("go infinite"),
        "a Engine não leu nada depois do go: {lines:?}"
    );
}

#[test]
fn updates_are_throttled_not_one_per_info_line() {
    let mut cfg = config(&["--info-depths", "400"], None);
    cfg.interval = ms(200);
    let mut ctrl = started(cfg);
    ctrl.analyze(PositionSpec::startpos(), 3);
    let mut updates = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut best = 0;
    while best < 400 && Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            if let AnalysisEvent::Update(u) = event {
                updates += 1;
                best = best.max(u.lines.iter().map(|l| l.depth).max().unwrap_or(0));
            }
        }
        std::thread::sleep(ms(5));
    }
    assert_eq!(best, 400, "a última profundidade chega");
    assert!(
        updates <= 10,
        "1200 linhas de info viraram {updates} updates"
    );
}

#[test]
fn threads_and_hash_are_clamped_to_the_app_limits_before_analysis() {
    let trace = trace_path("limits");
    let mut cfg = config(&[], Some(&trace));
    cfg.threads = Some(64);
    cfg.hash_mb = Some(1_000_000);
    cfg.limits = Limits {
        max_threads: 2,
        max_hash_mb: 256,
    };
    let mut ctrl = AnalysisController::start(cfg);
    let ready = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Ready { .. }));
    let AnalysisEvent::Ready {
        threads, hash_mb, ..
    } = ready
    else {
        unreachable!()
    };
    assert_eq!((threads, hash_mb), (Some(2), Some(256)));
    ctrl.analyze(PositionSpec::startpos(), 1);
    update_at_depth(&mut ctrl, 1);
    let lines = trace_lines(&trace);
    for expected in [
        "setoption name Threads value 2",
        "setoption name Hash value 256",
        "setoption name MultiPV value 1",
    ] {
        assert!(
            lines.contains(&expected.to_owned()),
            "{expected}: {lines:?}"
        );
    }
}

#[test]
fn engine_crash_is_a_failure_event_and_the_controller_dies() {
    let mut ctrl = started(config(&["--mode", "crash"], None));
    ctrl.analyze(PositionSpec::startpos(), 1);
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Crashed);
    assert!(ctrl.is_dead());
}

#[test]
fn spawn_failure_is_reported() {
    let cfg = AnalysisConfig::new(EngineConfig::new("/nao/existe/engine"));
    let mut ctrl = AnalysisController::start(cfg);
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Spawn);
}

#[test]
fn position_with_a_fen_and_moves_is_sent_as_is() {
    let trace = trace_path("fen");
    let mut ctrl = started(config(&[], Some(&trace)));
    let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
    ctrl.analyze(PositionSpec::from_fen(fen).with_moves([mv("e2e4")]), 1);
    let update = update_at_depth(&mut ctrl, 5);
    assert_eq!(update.lines[0].score, Score::Cp(-35), "pretas jogam");
    assert!(trace_lines(&trace).contains(&format!("position fen {fen} moves e2e4")));
}
