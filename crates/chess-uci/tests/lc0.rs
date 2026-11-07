//! Lc0 real (C10): nunca roda na suíte padrão (ADR-0003, `#[ignore]`).
//!
//! ```text
//! export CHESS_STUDIO_LC0=~/.cache/chess-studio-dev/lc0/build/lc0
//! export CHESS_STUDIO_LC0_NET=~/.cache/chess-studio-dev/lc0/networks/t1-512x15x8h-distilled-swa-3395000.pb.gz
//! cargo test -p chess-uci --test lc0 -- --ignored --nocapture
//! ```
//!
//! A rede precisa ser a do catálogo (T1) e o executável, um Lc0 0.32 com backend de GPU
//! (para o primeiro teste) ou de CPU.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use chess_uci::{
    AnalysisConfig, AnalysisController, AnalysisEvent, EngineConfig, Level, LoadPlan, PositionSpec,
    Timeouts, is_gpu_backend,
};

fn lc0() -> Option<(String, String)> {
    Some((
        std::env::var("CHESS_STUDIO_LC0").ok()?,
        std::env::var("CHESS_STUDIO_LC0_NET").ok()?,
    ))
}

fn config(exe: &str, net: &str) -> AnalysisConfig {
    let timeouts = Timeouts {
        handshake: Duration::from_secs(20),
        ready: Duration::from_secs(60),
        search_slack: Duration::from_secs(5),
        search_default: Duration::from_secs(60),
        stop_grace: Duration::from_secs(10),
        quit: Duration::from_secs(5),
    };
    let engine = EngineConfig::new(exe)
        .timeouts(timeouts)
        .option("WeightsFile", net);
    let mut config = AnalysisConfig::new(engine);
    config.interval = Duration::from_millis(100);
    config
}

fn wait_for(
    ctrl: &mut AnalysisController,
    secs: u64,
    mut pred: impl FnMut(&AnalysisEvent) -> bool,
) -> AnalysisEvent {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            println!("{event:?}");
            if pred(&event) {
                return event;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("evento esperado não chegou");
}

#[test]
#[ignore = "precisa de um Lc0 real e de uma rede (CHESS_STUDIO_LC0 e CHESS_STUDIO_LC0_NET)"]
fn lc0_picks_a_backend_analyses_and_obeys_the_load_plan() {
    let Some((exe, net)) = lc0() else {
        eprintln!("CHESS_STUDIO_LC0/CHESS_STUDIO_LC0_NET ausentes: nada a testar");
        return;
    };
    let mut ctrl = AnalysisController::start(config(&exe, &net));
    let AnalysisEvent::Ready {
        backend, backends, ..
    } = wait_for(&mut ctrl, 120, |e| matches!(e, AnalysisEvent::Ready { .. }))
    else {
        unreachable!()
    };
    println!("backends anunciados: {backends:?}; em uso: {backend:?}");
    let backend = backend.expect("o Lc0 anuncia Backend");
    if std::process::Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .is_ok_and(|o| o.status.success())
        && backends.iter().any(|b| is_gpu_backend(b))
    {
        assert!(is_gpu_backend(&backend), "GPU NVIDIA presente: {backend}");
    }

    let id = ctrl.analyze(PositionSpec::startpos(), 2);
    let deep = |e: &AnalysisEvent| match e {
        AnalysisEvent::Update(u) => {
            u.id == id && u.lines.iter().any(|l| l.nodes.unwrap_or(0) > 500)
        }
        _ => false,
    };
    wait_for(&mut ctrl, 60, deep);

    // Plano reduzido: a busca reinicia com tetos menores e continua devolvendo análise.
    ctrl.set_load(LoadPlan::for_level(Level::Minimal, 2));
    wait_for(&mut ctrl, 60, deep);

    // Pausado: sem atualizações; liberado: volta.
    ctrl.set_load(LoadPlan::for_level(Level::Paused, 2));
    std::thread::sleep(Duration::from_millis(1500));
    while ctrl.poll().is_some() {}
    std::thread::sleep(Duration::from_millis(1500));
    assert!(ctrl.poll().is_none(), "pausada não emite");
    ctrl.set_load(LoadPlan::FULL);
    wait_for(&mut ctrl, 60, deep);
}
