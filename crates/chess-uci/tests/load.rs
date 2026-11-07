//! Carga e backends da análise (C10, UCI-06): o plano do governador é aplicado à Engine entre
//! buscas (`stop` + `setoption` + nova busca), pausa e rajadas funcionam, e a escolha do
//! backend recua para a CPU quando a GPU não sobe. Observado pela interface pública contra o
//! Lc0 falso (`fake_uci --lc0`).
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chess_uci::{
    AnalysisConfig, AnalysisController, AnalysisEvent, Duty, EngineConfig, FailureKind, GpuVendor,
    Hardware, Level, LoadPlan, PositionSpec, Timeouts,
};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn timeouts() -> Timeouts {
    Timeouts {
        handshake: ms(1500),
        ready: ms(1500),
        search_slack: ms(100),
        search_default: ms(300),
        stop_grace: ms(300),
        quit: ms(300),
    }
}

fn trace_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "chess-studio-load-{}-{name}.trace",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn config(extra: &[&str], trace: &PathBuf) -> AnalysisConfig {
    let mut engine = EngineConfig::new(FAKE)
        .arg("--info")
        .arg("--options")
        .arg("--lc0")
        .timeouts(timeouts());
    for arg in extra {
        engine = engine.arg(*arg);
    }
    engine = engine.arg("--trace").arg(trace.as_os_str());
    let mut config = AnalysisConfig::new(engine);
    config.interval = ms(40);
    config.os = "linux";
    config.hardware = Some(Hardware {
        gpu: Some(GpuVendor::Nvidia),
    });
    config
}

fn wait_for(
    ctrl: &mut AnalysisController,
    mut pred: impl FnMut(&AnalysisEvent) -> bool,
) -> AnalysisEvent {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            if pred(&event) {
                return event;
            }
        }
        std::thread::sleep(ms(5));
    }
    panic!("evento esperado não chegou");
}

fn trace_lines(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn wait_trace(path: &PathBuf, pred: impl Fn(&[String]) -> bool) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let lines = trace_lines(path);
        if pred(&lines) {
            return lines;
        }
        assert!(Instant::now() < deadline, "trace esperado não chegou: {lines:?}");
        std::thread::sleep(ms(10));
    }
}

fn started(config: AnalysisConfig) -> AnalysisController {
    let mut ctrl = AnalysisController::start(config);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Ready { .. }));
    ctrl
}

fn count(lines: &[String], text: &str) -> usize {
    lines.iter().filter(|l| l.as_str() == text).count()
}

fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|l| l == text)
}

// --- plano de carga ------------------------------------------------------------------------

#[test]
fn a_reduced_plan_caps_threads_batch_and_cache_and_restarts_the_search() {
    let trace = trace_path("reduced");
    let mut cfg = config(&[], &trace);
    cfg.threads = Some(8);
    let mut ctrl = started(cfg);
    ctrl.analyze(PositionSpec::startpos(), 1);
    wait_trace(&trace, |l| count(l, "go infinite") == 1);
    ctrl.set_load(LoadPlan::for_level(Level::Reduced, 8));
    let lines = wait_trace(&trace, |l| count(l, "go infinite") == 2);
    // O `stop` vem antes das opções, que vêm antes da nova busca.
    let stop = lines.iter().rposition(|l| l == "stop").expect("stop");
    let threads = lines
        .iter()
        .rposition(|l| l == "setoption name Threads value 4")
        .expect("Threads 4");
    let batch = lines
        .iter()
        .position(|l| l == "setoption name MinibatchSize value 64")
        .expect("MinibatchSize");
    assert!(has(&lines, "setoption name NNCacheSize value 400000"));
    let second_go = lines.iter().rposition(|l| l == "go infinite").unwrap();
    assert!(stop < threads && stop < batch && threads < second_go, "{lines:?}");
}

#[test]
fn going_back_to_full_restores_the_configured_values() {
    let trace = trace_path("restore");
    let mut cfg = config(&[], &trace);
    cfg.threads = Some(8);
    let mut ctrl = started(cfg);
    ctrl.analyze(PositionSpec::startpos(), 1);
    ctrl.set_load(LoadPlan::for_level(Level::Minimal, 8));
    wait_trace(&trace, |l| has(l, "setoption name Threads value 1"));
    ctrl.set_load(LoadPlan::FULL);
    let lines = wait_trace(&trace, |l| {
        has(l, "setoption name MinibatchSize value 0") && has(l, "setoption name NNCacheSize value 2000000")
    });
    let last_threads = lines
        .iter()
        .rposition(|l| l.starts_with("setoption name Threads value"))
        .unwrap();
    assert_eq!(lines[last_threads], "setoption name Threads value 8");
}

#[test]
fn a_plan_without_changes_does_not_touch_the_engine() {
    let trace = trace_path("noop");
    let mut ctrl = started(config(&[], &trace));
    ctrl.analyze(PositionSpec::startpos(), 1);
    wait_trace(&trace, |l| count(l, "go infinite") == 1);
    ctrl.set_load(LoadPlan::FULL);
    std::thread::sleep(ms(200));
    let lines = trace_lines(&trace);
    assert_eq!(count(&lines, "go infinite"), 1, "{lines:?}");
}

#[test]
fn the_paused_plan_stops_the_engine_and_resuming_restarts_the_search() {
    let trace = trace_path("paused");
    let mut ctrl = started(config(&[], &trace));
    ctrl.analyze(PositionSpec::startpos(), 1);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(_)));
    ctrl.set_load(LoadPlan::for_level(Level::Paused, 4));
    wait_trace(&trace, |l| has(l, "stop"));
    // Pausada: nenhuma atualização nova.
    std::thread::sleep(ms(200));
    while ctrl.poll().is_some() {}
    std::thread::sleep(ms(250));
    assert!(ctrl.poll().is_none(), "pausada não emite");
    assert_eq!(count(&trace_lines(&trace), "go infinite"), 1);
    assert!(ctrl.is_analyzing(), "o pedido segue valendo, só está em espera");
    ctrl.set_load(LoadPlan::FULL);
    wait_trace(&trace, |l| count(l, "go infinite") == 2);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(_)));
}

#[test]
fn the_intermittent_plan_alternates_bursts_and_pauses() {
    let trace = trace_path("duty");
    let mut ctrl = started(config(&[], &trace));
    ctrl.analyze(PositionSpec::startpos(), 1);
    let plan = LoadPlan {
        duty: Some(Duty {
            run: ms(60),
            pause: ms(120),
        }),
        ..LoadPlan::for_level(Level::Intermittent, 4)
    };
    ctrl.set_load(plan);
    let lines = wait_trace(&trace, |l| count(l, "go infinite") >= 4);
    assert!(count(&lines, "stop") >= 3, "uma parada por rajada: {lines:?}");
    // Rajadas devolvem análise, e uma nova posição durante a pausa é atendida.
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(_)));
    let id = ctrl.analyze(PositionSpec::startpos().with_moves(["e2e4".parse().unwrap()]), 1);
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(u) if u.id == id));
    assert!(matches!(event, AnalysisEvent::Update(_)));
}

#[test]
fn a_new_position_while_paused_waits_for_the_resume() {
    let trace = trace_path("paused-new");
    let mut ctrl = started(config(&[], &trace));
    ctrl.analyze(PositionSpec::startpos(), 1);
    ctrl.set_load(LoadPlan::for_level(Level::Paused, 4));
    wait_trace(&trace, |l| has(l, "stop"));
    let id = ctrl.analyze(PositionSpec::startpos().with_moves(["e2e4".parse().unwrap()]), 1);
    std::thread::sleep(ms(300));
    assert!(
        !trace_lines(&trace).iter().any(|l| l == "position startpos moves e2e4"),
        "pausada: não envia a posição"
    );
    ctrl.set_load(LoadPlan::FULL);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(u) if u.id == id));
}

// --- backends ------------------------------------------------------------------------------

fn ready(ctrl: &mut AnalysisController) -> (Option<String>, Vec<String>) {
    match wait_for(ctrl, |e| matches!(e, AnalysisEvent::Ready { .. })) {
        AnalysisEvent::Ready {
            backend, backends, ..
        } => (backend, backends),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_backend_is_chosen_automatically_from_what_the_engine_announces() {
    let trace = trace_path("auto");
    let mut ctrl = AnalysisController::start(config(&[], &trace));
    let (backend, backends) = ready(&mut ctrl);
    assert_eq!(backend.as_deref(), Some("cuda-auto"), "NVIDIA + Linux");
    assert_eq!(backends, ["cuda-auto", "blas"]);
    assert!(has(&trace_lines(&trace), "setoption name Backend value cuda-auto"));
}

#[test]
fn without_a_gpu_the_automatic_choice_is_the_cpu_backend() {
    let trace = trace_path("auto-cpu");
    let mut cfg = config(&[], &trace);
    cfg.hardware = Some(Hardware { gpu: None });
    let mut ctrl = AnalysisController::start(cfg);
    assert_eq!(ready(&mut ctrl).0.as_deref(), Some("blas"));
}

#[test]
fn a_manual_backend_is_honoured_and_not_replaced_by_the_automatic_one() {
    let trace = trace_path("manual");
    let mut cfg = config(&[], &trace);
    cfg.backend = Some("blas".into());
    let mut ctrl = AnalysisController::start(cfg);
    assert_eq!(ready(&mut ctrl).0.as_deref(), Some("blas"));
    // Valor de outra fonte (opções salvas do gerenciador) também vale.
    let mut cfg = config(&[], &trace);
    cfg.engine = cfg.engine.option("Backend", "blas");
    let mut ctrl = AnalysisController::start(cfg);
    assert_eq!(ready(&mut ctrl).0.as_deref(), Some("blas"));
}

#[test]
fn a_manual_backend_the_engine_does_not_have_is_refused_before_it_is_sent() {
    let trace = trace_path("manual-bad");
    let mut cfg = config(&[], &trace);
    cfg.backend = Some("onnx-dml\nquit".into());
    let mut ctrl = AnalysisController::start(cfg);
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert!(failure.message.contains("backend"), "{}", failure.message);
    assert!(!trace_lines(&trace).iter().any(|l| l.contains("onnx-dml")));
}

#[test]
fn a_gpu_backend_that_dies_on_the_first_search_falls_back_to_the_cpu() {
    let trace = trace_path("fallback");
    let mut ctrl = AnalysisController::start(config(&["--fail-backend", "cuda-auto"], &trace));
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::BackendFallback { .. }));
    let AnalysisEvent::BackendFallback { from, to, message } = event else {
        unreachable!()
    };
    assert_eq!((from.as_str(), to.as_str()), ("cuda-auto", "blas"));
    assert!(message.contains("cuda-auto") && message.contains("blas"), "{message}");
    assert!(message.to_lowercase().contains("driver"), "{message}");
    let (backend, _) = ready(&mut ctrl);
    assert_eq!(backend.as_deref(), Some("blas"));
    // E a análise funciona na CPU.
    let id = ctrl.analyze(PositionSpec::startpos(), 1);
    wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Update(u) if u.id == id));
    assert!(has(&trace_lines(&trace), "setoption name Backend value blas"));
}

#[test]
fn fallback_can_be_turned_off_and_the_error_is_then_explained() {
    let trace = trace_path("no-fallback");
    let mut cfg = config(&["--fail-backend", "cuda-auto"], &trace);
    cfg.cpu_fallback = false;
    let mut ctrl = AnalysisController::start(cfg);
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Crashed);
    assert!(failure.message.contains("cuda-auto"), "{}", failure.message);
    assert!(failure.message.to_lowercase().contains("driver"), "{}", failure.message);
}

#[test]
fn a_gpu_only_build_without_a_gpu_reports_a_clear_error() {
    let trace = trace_path("gpu-only");
    let mut ctrl = AnalysisController::start(config(
        &["--backends", "cuda-auto", "--fail-backend", "cuda-auto"],
        &trace,
    ));
    let event = wait_for(&mut ctrl, |e| matches!(e, AnalysisEvent::Failed(_)));
    let AnalysisEvent::Failed(failure) = event else {
        unreachable!()
    };
    assert!(failure.message.contains("cuda-auto"), "{}", failure.message);
    assert!(
        failure.message.contains("gerenciador"),
        "diz o que fazer: {}",
        failure.message
    );
}

#[test]
fn engines_without_a_backend_option_are_untouched() {
    let trace = trace_path("stockfish-like");
    let mut engine = EngineConfig::new(FAKE)
        .arg("--info")
        .arg("--options")
        .timeouts(timeouts());
    engine = engine.arg("--trace").arg(trace.as_os_str());
    let mut ctrl = AnalysisController::start(AnalysisConfig::new(engine));
    let (backend, backends) = ready(&mut ctrl);
    assert_eq!(backend, None);
    assert!(backends.is_empty());
    assert!(!trace_lines(&trace).iter().any(|l| l.contains("Backend")));
}
