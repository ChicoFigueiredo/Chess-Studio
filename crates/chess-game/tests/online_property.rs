#![allow(clippy::unwrap_used)]
//! Propriedade da reconciliação por retrato (#230): aplicar qualquer sequência, com
//! repetição e fora de ordem, dos retratos de uma mesma Partida online nunca regride nem
//! diverge, e terminar no retrato final dá o mesmo estado que aplicá-lo sozinho.
//! Partidas pseudoaleatórias determinísticas por semente (xorshift, sem crate novo).

use chess_core::{CanonicalPosition, Color};
use chess_game::{
    Command, EndReason, Game, GameResult, GameSetup, OnlineGame, OnlinePlayer, Opponent, Outcome,
    ProviderClock, ProviderEnd, ProviderSnapshot, TimeControl, Timestamp, Variant,
};

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

fn setup() -> GameSetup {
    let player = |name: &str| OnlinePlayer {
        name: name.into(),
        rating: None,
    };
    GameSetup::online(
        OnlineGame {
            provider: "lichess".into(),
            remote_id: "prop0001".into(),
            local_color: Color::White,
            opponent: Opponent::Human,
            white: player("w"),
            black: player("b"),
            rated: true,
            days_per_move: None,
        },
        Variant::Standard,
        None,
    )
    .unwrap()
    .with_time_control(TimeControl {
        base_ms: 600_000,
        increment_ms: 2_000,
    })
}

/// Os retratos de uma Partida aleatória: um por meio-lance (0 = antes do primeiro), com
/// relógios e propostas de empate variando; o último pode trazer o fim.
fn snapshots(rng: &mut Rng) -> Vec<ProviderSnapshot> {
    let mut position = CanonicalPosition::start();
    let mut moves: Vec<String> = Vec::new();
    let plies = 1 + rng.below(60);
    let mut out = Vec::new();
    for ply in 0..=plies {
        let clock = ProviderClock {
            white_ms: 600_000 - (ply as u64) * 1_000,
            black_ms: 600_000 - (ply as u64) * 1_500,
            running: ply >= 2,
        };
        let draw_offer = match rng.below(6) {
            0 => Some(Color::White),
            1 => Some(Color::Black),
            _ => None,
        };
        out.push(ProviderSnapshot {
            moves: moves.clone(),
            clock: Some(clock),
            draw_offer,
            end: None,
        });
        if ply == plies {
            break;
        }
        let legal = position.legal_moves();
        if legal.is_empty() {
            break;
        }
        let mv = legal[rng.below(legal.len())];
        position = position.play(mv).unwrap();
        moves.push(mv.to_string());
    }
    if rng.below(2) == 0
        && let Some(last) = out.last_mut()
    {
        last.end = Some(match rng.below(3) {
            0 => ProviderEnd::Aborted,
            1 => ProviderEnd::Decided(Outcome {
                result: GameResult::Draw,
                reason: EndReason::DrawAgreement,
            }),
            _ => ProviderEnd::Decided(Outcome {
                result: GameResult::BlackWins,
                reason: EndReason::Resignation,
            }),
        });
    }
    out
}

fn history(game: &Game) -> Vec<String> {
    game.snapshot()
        .history
        .iter()
        .map(|h| h.mv.to_string())
        .collect()
}

fn run_seed(seed: u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let all = snapshots(&mut rng);
    let last = all.len() - 1;
    let final_moves = all[last].moves.clone();

    let mut game = Game::new(setup());
    let mut at = 0;
    let mut seen_len = 0;
    let mut was_over = false;
    for _ in 0..(3 * all.len()) {
        at += 1 + rng.below(3_000) as u64;
        let i = rng.below(all.len());
        game.handle(Command::Provider(all[i].clone()), Timestamp::mono(at))
            .unwrap_or_else(|e| panic!("semente {seed}: retrato {i} recusado: {e}"));
        let now = history(&game);
        // Nunca regride, nunca diverge: sempre um prefixo do final, sem encolher.
        assert!(now.len() >= seen_len, "semente {seed}: regrediu");
        assert_eq!(
            now[..],
            final_moves[..now.len()],
            "semente {seed}: divergiu"
        );
        seen_len = now.len();
        // Fim confirmado não volta atrás.
        let over = game.snapshot().is_over();
        assert!(!was_over || over, "semente {seed}: Partida reaberta");
        was_over = over;
    }
    at += 1;
    game.handle(Command::Provider(all[last].clone()), Timestamp::mono(at))
        .unwrap();

    let mut alone = Game::new(setup());
    alone
        .handle(Command::Provider(all[last].clone()), Timestamp::mono(at))
        .unwrap();
    let (a, b) = (game.snapshot(), alone.snapshot());
    assert_eq!(a.history, b.history, "semente {seed}");
    assert_eq!(a.position, b.position, "semente {seed}");
    assert_eq!(a.outcome, b.outcome, "semente {seed}");
    assert_eq!(a.is_over(), b.is_over(), "semente {seed}");
    assert_eq!(a.pending_draw_offer, b.pending_draw_offer, "semente {seed}");
    if !a.is_over() {
        let (ca, cb) = (a.clock.unwrap(), b.clock.unwrap());
        assert_eq!((ca.white_ms, ca.black_ms), (cb.white_ms, cb.black_ms));
        assert_eq!(ca.running, cb.running, "semente {seed}");
    }
    // O log que resultou disso reabre igual (ADR-0002).
    let reopened = Game::try_replay(setup(), game.events()).unwrap();
    assert_eq!(reopened.snapshot(), game.snapshot(), "semente {seed}");
}

#[test]
fn qualquer_ordem_de_retratos_com_repeticao_converge_sem_regredir() {
    for seed in 1..=120 {
        run_seed(seed);
    }
}
