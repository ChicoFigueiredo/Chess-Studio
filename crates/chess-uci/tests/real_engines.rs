//! Todas as Engines reais instaladas no computador, pelos mesmos caminhos do app (ADR-0003: os
//! binários nunca entram no repositório).
//!
//! Ignorado por padrão. Para rodar:
//!
//! ```text
//! cargo test -p chess-uci --test real_engines -- --ignored --nocapture
//! ```
//!
//! O teste usa `detect_host` (o "Procurar no computador" do gerenciador), a mesma montagem de
//! opções do app (`launch_options`, `advertised_backends`/`recommended_backend` para o Lc0) e,
//! para cada Engine achada: handshake, autojogo validado pelo núcleo, análise com várias linhas
//! e mate em 1, e a Engine adversária (`EnginePlayer`) com relógio e Elo. Ao fim imprime uma
//! tabela e só passa se nenhuma etapa falhou. Para limitar a poucas Engines, defina
//! `CHESS_STUDIO_REAL_ENGINES=berserk,lc0` (trechos do nome do arquivo).
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::time::{Duration, Instant};

use chess_core::{CanonicalPosition, Color};
use chess_game::{Command, EngineSeat, Game, GameSetup, PolicyKind, TimeControl, Timestamp};
use chess_uci::{
    AnalysisConfig, AnalysisController, AnalysisEvent, DetectedEngine, Engine, EngineConfig,
    EngineMove, EnginePlayer, GoParams, PlayerStatus, PositionSpec, QuitOutcome, Score, Timeouts,
    advertised_backends, detect_hardware, detect_host, engines_dir, installed_networks, probe_full,
    recommended_backend, weights_option,
};

/// Uma etapa do teste: devolve a nota do que viu ou o motivo da falha.
type Step = fn(&DetectedEngine) -> Result<String, String>;

fn generous() -> Timeouts {
    Timeouts {
        handshake: Duration::from_secs(20),
        ready: Duration::from_secs(60),
        search_slack: Duration::from_secs(5),
        search_default: Duration::from_secs(60),
        stop_grace: Duration::from_secs(10),
        quit: Duration::from_secs(5),
    }
}

fn file_name(found: &DetectedEngine) -> String {
    found
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// A configuração com que o app sobe a Engine: o Lc0 leva a rede instalada em
/// `<engines>/networks/` e o backend recomendado entre os que ele anuncia.
fn config_for(found: &DetectedEngine) -> EngineConfig {
    let mut config = EngineConfig::new(&found.path).timeouts(generous());
    if file_name(found).starts_with("lc0") {
        let dir = engines_dir().expect("pasta de Engines do app");
        let network = installed_networks(&dir)
            .into_iter()
            .find_map(|n| weights_option(&dir, &n))
            .expect("o Lc0 precisa de uma rede em <engines>/networks/");
        config = config.option(&network.0, &network.1);
        let (_, options) = probe_full(&found.path, Duration::from_secs(20)).unwrap();
        let advertised = advertised_backends(&options);
        if let Some(backend) =
            recommended_backend(std::env::consts::OS, detect_hardware(), &advertised)
        {
            config = config.option("Backend", &backend);
        }
    }
    config
}

fn self_play(found: &DetectedEngine) -> Result<String, String> {
    let mut engine = Engine::spawn(config_for(found)).map_err(|e| format!("handshake: {e:?}"))?;
    engine
        .new_game()
        .map_err(|e| format!("ucinewgame: {e:?}"))?;
    let go = GoParams::movetime(Duration::from_millis(100));
    let mut position = CanonicalPosition::start();
    let mut played = 0;
    for _ in 0..20 {
        match engine
            .search_legal_move(&position, &go)
            .map_err(|e| format!("lance {played}: {e:?}"))?
        {
            EngineMove::Played { next, .. } => {
                position = next;
                played += 1;
            }
            EngineMove::NoMove => break,
        }
    }
    if !engine.is_alive() {
        return Err("a Engine morreu durante o autojogo".into());
    }
    match engine.quit() {
        QuitOutcome::Graceful => Ok(format!("{played} meios-lances legais; quit limpo")),
        QuitOutcome::Killed => Err(format!("{played} meios-lances, mas ignorou o quit")),
    }
}

fn reach(
    ctrl: &mut AnalysisController,
    id: u64,
    depth: u32,
    lines: usize,
) -> Result<chess_uci::AnalysisUpdate, String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        while let Some(event) = ctrl.poll() {
            match event {
                AnalysisEvent::Update(u)
                    if u.id == id
                        && u.lines.len() == lines
                        && u.lines.iter().all(|l| l.depth >= depth) =>
                {
                    return Ok(u);
                }
                AnalysisEvent::Failed(f) => return Err(format!("falha: {f:?}")),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "profundidade {depth} com {lines} linha(s) não chegou em 60 s"
    ))
}

fn analysis(found: &DetectedEngine) -> Result<String, String> {
    let (_, options) =
        probe_full(&found.path, Duration::from_secs(20)).map_err(|e| format!("opções: {e:?}"))?;
    let multipv = options
        .iter()
        .any(|o| o.name.eq_ignore_ascii_case("multipv"));
    let lines: u8 = if multipv { 3 } else { 1 };
    let mut ctrl = AnalysisController::start(AnalysisConfig::new(config_for(found)));
    let id = ctrl.analyze(PositionSpec::startpos(), lines);
    let update = reach(&mut ctrl, id, 4, usize::from(lines))?;
    if update.lines.iter().any(|l| l.san.is_empty()) {
        return Err("linha sem SAN".into());
    }
    let first = update.lines[0].san.first().cloned().unwrap_or_default();
    let id = ctrl.analyze(
        PositionSpec::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1"),
        1,
    );
    let mate = reach(&mut ctrl, id, 1, 1)?;
    let ok = mate.lines[0].score == Score::Mate(1) && mate.lines[0].san[0] == "Ra8#";
    if !ok {
        return Err(format!(
            "mate em 1: {:?} {:?}",
            mate.lines[0].score, mate.lines[0].san
        ));
    }
    Ok(format!(
        "{lines} linha(s), 1º lance {first}; mate em 1 achado"
    ))
}

fn now(start: Instant) -> Timestamp {
    Timestamp::mono(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX))
}

fn player(found: &DetectedEngine) -> Result<String, String> {
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
    let mut player = EnginePlayer::new(seat, config_for(found));
    let start = Instant::now();
    for round in 0..4 {
        let snap = game.snapshot();
        let Some(mv) = snap.position.legal_moves().into_iter().last() else {
            break;
        };
        if mv.promotion().is_some() {
            break;
        }
        game.handle(Command::Touch { square: mv.from() }, now(start))
            .map_err(|e| format!("toque: {e:?}"))?;
        game.handle(
            Command::Release {
                from: mv.from(),
                to: mv.to(),
            },
            now(start),
        )
        .map_err(|e| format!("soltar: {e:?}"))?;
        game.handle(Command::PressClock, now(start))
            .map_err(|e| format!("relógio: {e:?}"))?;
        let want = 2 * (round + 1);
        let deadline = Instant::now() + Duration::from_secs(60);
        while game.snapshot().history.len() < want && game.snapshot().outcome.is_none() {
            if Instant::now() > deadline {
                return Err(format!(
                    "não respondeu no Lance {round}: {:?}",
                    player.status()
                ));
            }
            if let PlayerStatus::Failed(f) = player.status() {
                return Err(format!("falhou: {f:?}"));
            }
            player.drive(&mut game, now(start));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let moves: Vec<String> = game
        .snapshot()
        .history
        .iter()
        .map(|h| h.san.clone())
        .collect();
    if moves.len() < 6 {
        return Err(format!(
            "só {} meios-lances: {:?}",
            moves.len(),
            player.status()
        ));
    }
    Ok(format!(
        "força {:?}; {}",
        player.strength(),
        moves.join(" ")
    ))
}

#[test]
#[ignore = "exige Engines instaladas no computador (pasta de Engines do app, PATH...)"]
fn every_engine_found_on_this_computer_works_through_the_app_paths() {
    let only: Vec<String> = std::env::var("CHESS_STUDIO_REAL_ENGINES")
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();
    let found: Vec<DetectedEngine> = detect_host()
        .into_iter()
        .filter(|f| only.is_empty() || only.iter().any(|o| file_name(f).contains(o)))
        .collect();
    assert!(!found.is_empty(), "nenhuma Engine achada");
    let mut failures = 0;
    for engine in &found {
        let label = format!("{} [{}]", engine.display_name(), short(&engine.path));
        eprintln!("\n=== {label}\n    autor: {:?}", engine.author);
        let steps: [(&str, Step); 3] = [
            ("autojogo", self_play),
            ("análise", analysis),
            ("adversária", player),
        ];
        for (name, step) in steps {
            let started = Instant::now();
            match step(engine) {
                Ok(note) => eprintln!(
                    "    PASS {name} ({:.1}s): {note}",
                    started.elapsed().as_secs_f32()
                ),
                Err(note) => {
                    failures += 1;
                    eprintln!(
                        "    FAIL {name} ({:.1}s): {note}",
                        started.elapsed().as_secs_f32()
                    );
                }
            }
        }
    }
    assert_eq!(
        failures, 0,
        "{failures} etapa(s) falharam; veja a tabela acima"
    );
}

fn short(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default()
}
