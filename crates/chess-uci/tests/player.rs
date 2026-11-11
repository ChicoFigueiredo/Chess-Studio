//! A Engine adversária integrada à Partida (B9), observada pela interface pública contra
//! a Engine falsa: jogo normal, Lance ilegal (UCI-03), crash e travamento (UCI-02,
//! BDD-06), relógio, força por Elo, reclamação de empate e fila sem bloqueio.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chess_core::{Color, Square};
use chess_game::{
    Command, EngineSeat, EventKind, Game, GameSetup, PolicyKind, TimeControl, Timestamp,
};
use chess_uci::{EngineConfig, EnginePlayer, FailureKind, PlayerStatus, StrengthApplied, Timeouts};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

fn timeouts() -> Timeouts {
    Timeouts {
        handshake: ms(1500),
        ready: ms(1500),
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
        .timeouts(timeouts())
}

fn trace_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "chess-studio-uci-{}-{name}.trace",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn traced(mode: &str, path: &Path) -> EngineConfig {
    config(mode)
        .arg("--elo")
        .arg("--trace")
        .arg(path.as_os_str())
}

fn black_engine(elo: u32) -> EngineSeat {
    EngineSeat {
        color: Color::Black,
        elo,
    }
}

/// Relógio de teste: instantes crescentes a partir do início.
struct Clock0(Instant);

impl Clock0 {
    fn now(&self) -> Timestamp {
        Timestamp::mono(u64::try_from(self.0.elapsed().as_millis()).unwrap())
    }
}

fn human_move(game: &mut Game, t: &Clock0, from: &str, to: &str) {
    game.handle(
        Command::Release {
            from: sq(from),
            to: sq(to),
        },
        t.now(),
    )
    .unwrap();
}

/// Chama `drive` como a interface faria (a cada quadro) até a condição valer. Cada
/// chamada precisa ser rápida: a thread da UI nunca bloqueia.
fn drive_until(
    player: &mut EnginePlayer,
    game: &mut Game,
    t: &Clock0,
    cond: impl Fn(&EnginePlayer, &Game) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !cond(player, game) {
        assert!(
            Instant::now() < deadline,
            "tempo esgotado esperando a Engine"
        );
        let before = Instant::now();
        player.drive(game, t.now());
        assert!(
            before.elapsed() < ms(400),
            "drive bloqueou a thread da interface"
        );
        std::thread::sleep(ms(5));
    }
}

fn plies(game: &Game) -> usize {
    game.snapshot().history.len()
}

#[test]
fn engine_answers_a_human_move_with_position_from_the_start() {
    let trace = trace_path("normal");
    let mut game =
        Game::new(GameSetup::standard(PolicyKind::Assisted).with_engine(black_engine(1500)));
    let mut player = EnginePlayer::new(black_engine(1500), traced("normal", &trace));
    let t = Clock0(Instant::now());

    // Vez das Brancas (humano): a Engine não faz nada.
    player.drive(&mut game, t.now());
    assert_eq!(player.status(), PlayerStatus::Waiting);

    human_move(&mut game, &t, "e2", "e4");
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 2);
    assert_eq!(game.snapshot().history[1].san, "a5");
    assert_eq!(game.snapshot().side_to_move, Color::White);

    // Segundo lance: `position` vem desde o início, com todos os Lances.
    human_move(&mut game, &t, "g1", "f3");
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 4);
    let sent = std::fs::read_to_string(&trace).unwrap();
    assert!(sent.contains("position startpos moves e2e4\n"), "{sent}");
    assert!(
        sent.contains("position startpos moves e2e4 a7a5 g1f3\n"),
        "{sent}"
    );
    // Sem relógio na Partida: `movetime`.
    assert!(sent.contains("go movetime 1000"), "{sent}");
    // Força por Elo (a falsa anuncia UCI_Elo 1320..3190).
    assert!(sent.contains("setoption name UCI_LimitStrength value true"));
    assert!(sent.contains("setoption name UCI_Elo value 1500"));
    assert_eq!(
        player.strength(),
        Some(StrengthApplied::Elo {
            requested: 1500,
            used: 1500
        })
    );
    assert_eq!(player.engine_name(), Some("FakeUci 1.0"));
    assert_eq!(game.setup().engine(), Some(black_engine(1500)));
}

#[test]
fn elo_outside_the_engine_range_is_clamped_and_skill_level_is_the_fallback() {
    let trace = trace_path("clamp");
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut low = EnginePlayer::new(black_engine(400), traced("normal", &trace));
    let t = Clock0(Instant::now());
    drive_until(&mut low, &mut game, &t, |p, _| p.strength().is_some());
    assert_eq!(
        low.strength(),
        Some(StrengthApplied::Elo {
            requested: 400,
            used: 1320
        })
    );

    // Sem `UCI_Elo` anunciado: `Skill Level` (DEC-29).
    let skill_trace = trace_path("skill");
    let cfg = config("normal").arg("--trace").arg(skill_trace.as_os_str());
    let mut skill = EnginePlayer::new(black_engine(1800), cfg);
    drive_until(&mut skill, &mut game, &t, |p, _| p.strength().is_some());
    assert_eq!(
        skill.strength(),
        Some(StrengthApplied::SkillLevel {
            requested: 1800,
            level: 10
        })
    );
    assert!(
        std::fs::read_to_string(&skill_trace)
            .unwrap()
            .contains("setoption name Skill Level value 10")
    );
}

#[test]
fn engine_receives_the_game_clock_and_presses_its_own() {
    let trace = trace_path("clock");
    let setup = GameSetup::standard(PolicyKind::FideSimulation)
        .with_time_control(TimeControl {
            base_ms: 3_600_000,
            increment_ms: 30_000,
        })
        .with_engine(EngineSeat {
            color: Color::White,
            elo: 2000,
        });
    let mut game = Game::new(setup);
    let mut player = EnginePlayer::new(
        EngineSeat {
            color: Color::White,
            elo: 2000,
        },
        traced("normal", &trace),
    );
    let t = Clock0(Instant::now());
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 1);
    let sent = std::fs::read_to_string(&trace).unwrap();
    let go = sent.lines().find(|l| l.starts_with("go ")).unwrap();
    assert!(go.contains("wtime "), "{go}");
    assert!(go.contains("btime "), "{go}");
    assert!(go.contains("winc 30000 binc 30000"), "{go}");
    assert!(!go.contains("movetime"), "{go}");
    // O relógio da Engine foi acionado pelo próprio Lance: agora corre o das Pretas.
    let snap = game.snapshot();
    assert_eq!(snap.clock.unwrap().running, Some(Color::Black));
}

#[test]
fn illegal_move_is_recoverable_and_never_changes_the_position() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut player = EnginePlayer::new(black_engine(1500), config("illegal"));
    let t = Clock0(Instant::now());
    human_move(&mut game, &t, "e2", "e4");
    let before = game.snapshot().position.to_fen();
    drive_until(&mut player, &mut game, &t, |p, _| {
        matches!(p.status(), PlayerStatus::Failed(_))
    });
    let PlayerStatus::Failed(f) = player.status() else {
        unreachable!()
    };
    assert_eq!(f.kind, FailureKind::IllegalMove);
    assert_eq!(game.snapshot().position.to_fen(), before);
    assert_eq!(plies(&game), 1);
    assert!(
        game.events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::EngineMoveRejected { .. })),
        "a recusa fica no log"
    );
    assert!(game.snapshot().outcome.is_none());

    // "Trocar Engine": a Partida segue com outra Engine.
    player.replace_engine(config("normal"));
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 2);
}

#[test]
fn crash_keeps_the_game_and_offers_retry_and_replace() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut player = EnginePlayer::new(black_engine(1500), config("crash"));
    let t = Clock0(Instant::now());
    human_move(&mut game, &t, "d2", "d4");
    drive_until(&mut player, &mut game, &t, |p, _| {
        matches!(p.status(), PlayerStatus::Failed(_))
    });
    let PlayerStatus::Failed(f) = player.status() else {
        unreachable!()
    };
    assert_eq!(f.kind, FailureKind::Crashed);
    assert!(!f.message.is_empty());
    assert_eq!(plies(&game), 1, "Partida preservada");
    // A UI segue responsiva e a Partida aceita Lances humanos normalmente.
    player.drive(&mut game, t.now());
    assert!(matches!(player.status(), PlayerStatus::Failed(_)));

    // "Tentar de novo" com a mesma Engine (que cai de novo): volta a falhar, sem pânico.
    player.retry();
    drive_until(&mut player, &mut game, &t, |p, _| {
        matches!(p.status(), PlayerStatus::Failed(_))
    });

    // "Trocar Engine": outra Engine assume de onde a Partida está.
    player.replace_engine(config("normal"));
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 2);
}

#[test]
fn hung_engine_times_out_without_blocking_and_keeps_the_game() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut player = EnginePlayer::new(black_engine(1500), config("hang")).with_movetime(ms(100));
    let t = Clock0(Instant::now());
    human_move(&mut game, &t, "e2", "e4");
    drive_until(&mut player, &mut game, &t, |p, _| {
        matches!(p.status(), PlayerStatus::Failed(_))
    });
    let PlayerStatus::Failed(f) = player.status() else {
        unreachable!()
    };
    assert_eq!(f.kind, FailureKind::Timeout);
    assert_eq!(plies(&game), 1);
    player.replace_engine(config("normal"));
    drive_until(&mut player, &mut game, &t, |_, g| plies(g) == 2);
}

#[test]
fn engine_that_cannot_start_fails_with_a_clear_message() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut player = EnginePlayer::new(
        black_engine(1500),
        EngineConfig::new("/nao/existe/engine").timeouts(timeouts()),
    );
    let t = Clock0(Instant::now());
    human_move(&mut game, &t, "e2", "e4");
    drive_until(&mut player, &mut game, &t, |p, _| {
        matches!(p.status(), PlayerStatus::Failed(_))
    });
    let PlayerStatus::Failed(f) = player.status() else {
        unreachable!()
    };
    assert_eq!(f.kind, FailureKind::Spawn);
    assert!(f.message.contains("/nao/existe/engine"), "{}", f.message);
}

#[test]
fn undo_while_thinking_discards_the_stale_answer() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let cfg = config("slow").arg("--delay-ms").arg("500");
    let mut player = EnginePlayer::new(black_engine(1500), cfg).with_movetime(ms(100));
    let t = Clock0(Instant::now());
    human_move(&mut game, &t, "e2", "e4");
    drive_until(&mut player, &mut game, &t, |p, _| p.is_thinking());
    game.handle(Command::Undo, t.now()).unwrap();
    assert_eq!(plies(&game), 0);
    // Passado o atraso da Engine, a resposta antiga não vira Lance.
    let until = Instant::now() + ms(900);
    while Instant::now() < until {
        player.drive(&mut game, t.now());
        std::thread::sleep(ms(10));
    }
    assert_eq!(plies(&game), 0);
    assert_eq!(game.snapshot().side_to_move, Color::White);
}

#[test]
fn engine_claims_a_valid_draw_in_competitive_policies() {
    // 100 semi-lances sem captura nem peão: a Engine (Brancas) reclama (DEC-38).
    let setup = GameSetup::from_fen(
        PolicyKind::FideSimulation,
        "4k3/8/8/8/8/8/8/R3K3 w - - 100 80",
    )
    .unwrap()
    .with_engine(EngineSeat {
        color: Color::White,
        elo: 1500,
    });
    let mut game = Game::new(setup);
    assert!(game.snapshot().claimable_draw.is_some());
    let mut player = EnginePlayer::new(
        EngineSeat {
            color: Color::White,
            elo: 1500,
        },
        config("normal"),
    );
    let t = Clock0(Instant::now());
    drive_until(&mut player, &mut game, &t, |_, g| {
        g.snapshot().outcome.is_some()
    });
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, chess_game::GameResult::Draw);
    assert_eq!(plies(&game), 0, "reclamou em vez de jogar");
}

#[test]
fn full_game_of_the_fake_engine_against_a_scripted_human_reaches_a_result_or_keeps_legal() {
    // Humano roteirizado joga o primeiro Lance legal; a Engine responde por 10 lances.
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut player = EnginePlayer::new(black_engine(1500), config("normal")).with_movetime(ms(50));
    let t = Clock0(Instant::now());
    for round in 0..10 {
        if game.snapshot().outcome.is_some() {
            break;
        }
        let snap = game.snapshot();
        let mv = snap.position.legal_moves().into_iter().last().unwrap();
        let promotion = mv.promotion().is_some();
        game.handle(
            Command::Release {
                from: mv.from(),
                to: mv.to(),
            },
            t.now(),
        )
        .unwrap();
        if promotion {
            game.handle(
                Command::ChoosePromotion {
                    piece: chess_core::Piece::Queen,
                },
                t.now(),
            )
            .unwrap();
        }
        let want = 2 * (round + 1);
        drive_until(&mut player, &mut game, &t, |_, g| {
            plies(g) >= want || g.snapshot().outcome.is_some()
        });
    }
    assert!(plies(&game) >= 10);
}
