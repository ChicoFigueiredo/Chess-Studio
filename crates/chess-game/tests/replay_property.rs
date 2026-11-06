#![allow(clippy::unwrap_used)]
//! ADR-0002 / critério do B2: `replay(setup, eventos)` == estado obtido pelos Comandos.
//! Partidas pseudoaleatórias determinísticas por semente (xorshift, sem crate novo).

mod common;
use chess_core::{Color, Piece};
use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};
use common::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_command(rng: &mut Rng, game: &Game) -> Command {
    let snap = game.snapshot();
    let moves = snap.position.legal_moves();
    match rng.below(20) {
        0 => Command::Undo,
        1 => Command::Redo,
        2 => Command::OfferDraw {
            side: if rng.below(2) == 0 {
                Color::White
            } else {
                Color::Black
            },
        },
        3 => Command::AcceptDraw,
        4 => Command::DeclineDraw,
        5 => Command::RetractMove,
        6 => Command::ChoosePromotion {
            piece: [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight][rng.below(4)],
        },
        7 => Command::Release {
            // Possivelmente ilegal.
            from: chess_core::Square::new(rng.below(8) as u8, rng.below(8) as u8).unwrap(),
            to: chess_core::Square::new(rng.below(8) as u8, rng.below(8) as u8).unwrap(),
        },
        8 if rng.below(40) == 0 => Command::Resign { side: Color::White },
        _ if !moves.is_empty() => {
            let mv = moves[rng.below(moves.len())];
            Command::Release {
                from: mv.from(),
                to: mv.to(),
            }
        }
        _ => Command::Undo,
    }
}

fn run_seed(seed: u64, setup: GameSetup) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut game = Game::new(setup.clone());
    let mut at = 0;
    for _ in 0..400 {
        at += 1 + rng.below(500) as u64;
        let _ = game.handle(random_command(&mut rng, &game), Timestamp::mono(at));
    }

    let replayed = Game::replay(setup.clone(), game.events());
    assert_eq!(replayed.snapshot(), game.snapshot(), "semente {seed}");
    assert_eq!(replayed.events(), game.events(), "semente {seed}");

    let strict = Game::try_replay(setup, game.events()).expect("log coerente");
    assert_eq!(strict.snapshot(), game.snapshot(), "semente {seed}");
}

#[test]
fn replay_reproduz_o_estado_de_partidas_aleatorias_da_posicao_inicial() {
    for seed in 1..=60 {
        run_seed(seed, GameSetup::standard(PolicyKind::Assisted));
    }
}

#[test]
fn replay_reproduz_o_estado_a_partir_de_fens_com_promocao_e_relogio_alto() {
    for (i, fen) in [
        "8/P6k/8/8/8/8/p7/K7 w - - 0 1",
        "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 90 40",
        "4k3/8/8/8/8/8/8/R3K3 w - - 140 100",
    ]
    .into_iter()
    .enumerate()
    {
        for seed in 1..=15 {
            run_seed(
                seed + 100 * i as u64,
                GameSetup::from_fen(PolicyKind::Assisted, fen).unwrap(),
            );
        }
    }
}

#[test]
fn log_truncado_reconstroi_o_prefixo_e_log_corrompido_e_detectado() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5 Nf3");
    let setup = game.setup().clone();

    let prefix = Game::replay(setup.clone(), &game.events()[..2]);
    assert_eq!(prefix.snapshot().history.len(), 2);

    // Pula o 2º evento: o 3º (Nf3 branco) não é a vez das brancas.
    let mut broken = game.events().to_vec();
    broken.remove(1);
    let err = Game::try_replay(setup.clone(), &broken).unwrap_err();
    assert_eq!(err.index, 1);
    assert_eq!(Game::replay(setup, &broken).snapshot().history.len(), 1);
}

/// Comandos aleatórios para as Políticas competitivas: toques, ajustes, relógio,
/// retratação, Pausa, Reclamações e Engine (UCI aleatório, às vezes inválido).
fn random_fide_command(rng: &mut Rng, game: &Game) -> Command {
    let snap = game.snapshot();
    let moves = snap.position.legal_moves();
    let any_square =
        |rng: &mut Rng| chess_core::Square::new(rng.below(8) as u8, rng.below(8) as u8).unwrap();
    match rng.below(24) {
        0..=2 => Command::PressClock,
        3 => Command::RetractMove,
        4 => Command::Pause,
        5 | 6 => Command::Resume,
        7 => Command::ClaimDraw,
        8 if !moves.is_empty() => Command::ClaimDrawWithMove {
            mv: moves[rng.below(moves.len())],
        },
        9 => Command::Adjust {
            square: any_square(rng),
        },
        10 => Command::EngineMove {
            uci: if rng.below(3) == 0 {
                "zz99".to_owned()
            } else if moves.is_empty() {
                "e2e4".to_owned()
            } else {
                moves[rng.below(moves.len())].to_string()
            },
        },
        11 => Command::ChoosePromotion {
            piece: [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight][rng.below(4)],
        },
        12..=14 => Command::Touch {
            square: any_square(rng),
        },
        15 => Command::Release {
            from: any_square(rng),
            to: any_square(rng),
        },
        16 if rng.below(40) == 0 => Command::Resign { side: Color::Black },
        _ if !moves.is_empty() => {
            let mv = moves[rng.below(moves.len())];
            Command::Release {
                from: mv.from(),
                to: mv.to(),
            }
        }
        _ => Command::PressClock,
    }
}

fn run_fide_seed(seed: u64, setup: GameSetup) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut game = Game::new(setup.clone());
    let mut at = 0;
    for _ in 0..500 {
        at += 1 + rng.below(400) as u64;
        let command = random_fide_command(&mut rng, &game);
        let _ = game.handle(command, Timestamp::mono(at));
    }
    let replayed = Game::replay(setup.clone(), game.events());
    assert_eq!(replayed.snapshot(), game.snapshot(), "semente {seed}");
    assert_eq!(replayed.events(), game.events(), "semente {seed}");
    let strict = Game::try_replay(setup, game.events()).expect("log coerente");
    assert_eq!(strict.snapshot(), game.snapshot(), "semente {seed}");
    // O estado reconstruído também lê o mesmo relógio em qualquer instante.
    for probe in [at, at + 1_000, at + 10_000_000] {
        assert_eq!(
            replayed.clock_at(Timestamp::mono(probe)),
            game.clock_at(Timestamp::mono(probe)),
            "semente {seed}"
        );
    }
}

#[test]
fn replay_reproduz_o_estado_da_simulacao_fide_com_e_sem_relogio() {
    use chess_game::TimeControl;
    for seed in 1..=60 {
        run_fide_seed(seed, GameSetup::standard(PolicyKind::FideSimulation));
        run_fide_seed(
            seed + 1000,
            GameSetup::standard(PolicyKind::FideSimulation).with_time_control(TimeControl {
                base_ms: 60_000,
                increment_ms: 2_000,
            }),
        );
    }
}

#[test]
fn replay_reproduz_o_estado_do_assistido_com_relogio_e_pausa() {
    use chess_game::TimeControl;
    for seed in 1..=30 {
        run_fide_seed(
            seed + 2000,
            GameSetup::standard(PolicyKind::Assisted).with_time_control(TimeControl {
                base_ms: 30_000,
                increment_ms: 500,
            }),
        );
    }
}

#[test]
fn replay_reproduz_fide_a_partir_de_fens_especiais() {
    for (i, fen) in [
        "8/P6k/8/8/8/8/6r1/K7 w - - 0 1",
        "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 90 40",
        "4k3/8/8/8/8/8/8/R3K3 w - - 98 100",
    ]
    .into_iter()
    .enumerate()
    {
        for seed in 1..=20 {
            run_fide_seed(
                seed + 3000 * (i as u64 + 1),
                GameSetup::from_fen(PolicyKind::FideSimulation, fen).unwrap(),
            );
        }
    }
}

#[test]
fn replay_reproduz_o_estado_da_eliminacao_imediata_nas_duas_etapas() {
    use chess_game::{InfractionStage, TimeControl};
    for (i, stage) in [InfractionStage::Clock, InfractionStage::Release]
        .into_iter()
        .enumerate()
    {
        for seed in 1..=40 {
            let base = GameSetup::standard(PolicyKind::SuddenElimination)
                .with_infraction_stage(stage);
            run_fide_seed(seed + 5000 * (i as u64 + 1), base.clone());
            run_fide_seed(
                seed + 5500 * (i as u64 + 1),
                base.with_time_control(TimeControl {
                    base_ms: 60_000,
                    increment_ms: 2_000,
                }),
            );
        }
    }
}
