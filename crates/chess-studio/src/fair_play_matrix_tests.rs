//! Matriz de ramos da Trava de fair play (NFR-10, #269): cada `Computation` contra cada
//! estado da trava, positivo e negativo, e a conferência de que a matriz documentada em
//! `docs/seguranca/matriz-ramos-v4.md` tem uma linha para cada `Computation`.

#![allow(clippy::unwrap_used)]

use super::tests::{feed, online_game};
use super::*;
use chess_core::Move;
use chess_game::{Command, GameSetup, Opponent, ProviderEnd, Timestamp};

const SCOTCH: [&str; 10] = [
    "e2e4", "e7e5", "g1f3", "b8c6", "d2d4", "e5d4", "f3d4", "g8f6", "d4c6", "b7c6",
];

fn fair() -> FairPlay {
    FairPlay::new(FairPlayGuard::new())
}

fn after(moves: &[&str]) -> CanonicalPosition {
    moves.iter().fold(CanonicalPosition::start(), |p, m| {
        p.play(m.parse::<Move>().unwrap()).unwrap()
    })
}

fn unrelated() -> Subject {
    Subject::position(&after(&["d2d4", "d7d5", "c2c4"]))
}

fn locked(computation: Computation, scope: LockScope) -> Result<(), FairPlayLocked> {
    Err(FairPlayLocked { computation, scope })
}

fn correspondence(id: &str) -> Game {
    let mut game = online_game(id, Some(3), Opponent::Human);
    feed(&mut game, &SCOTCH, None);
    game
}

/// Estado livre: nenhuma trava, nenhum serviço recusado (ramo positivo de todas).
#[test]
fn matrix_free_state_allows_every_computation_on_every_entry_point() {
    let fair = fair();
    let start = CanonicalPosition::start();
    let local = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    for computation in Computation::ALL {
        assert_eq!(fair.check(computation, &unrelated()), Ok(()));
        assert_eq!(fair.check_fen(computation, &start.to_fen()), Ok(()));
        assert_eq!(fair.check_fen(computation, "lixo"), Ok(()));
        assert_eq!(fair.check_game(computation, &local), Ok(()));
        assert_eq!(fair.check_anywhere(computation), Ok(()));
        assert_eq!(fair.check_start(computation, Some(&start)), Ok(()));
        assert_eq!(fair.check_start(computation, None), Ok(()));
    }
    assert!(!fair.any());
}

/// Trava da rede (Partida online contra pessoa com a sessão aberta): tudo recusado, em todos
/// os pontos de entrada; só a Engine adversária da própria Partida competitiva local escapa
/// da trava que ela mesma segura.
#[test]
fn matrix_network_guard_refuses_every_computation_on_every_entry_point() {
    let fair = fair();
    let hold = fair.guard().hold("Partida online contra pessoa");
    let start = CanonicalPosition::start();
    let local = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    for computation in Computation::ALL {
        let refused = locked(computation, LockScope::Everything);
        assert_eq!(fair.check(computation, &unrelated()), refused);
        assert_eq!(fair.check_fen(computation, &start.to_fen()), refused);
        assert_eq!(fair.check_fen(computation, "lixo"), refused);
        assert_eq!(fair.check_game(computation, &local), refused);
        assert_eq!(fair.check_anywhere(computation), refused);
        assert_eq!(fair.check_start(computation, Some(&start)), refused);
        assert_eq!(fair.check_start(computation, None), refused);
    }
    drop(hold);
    for computation in Computation::ALL {
        assert_eq!(fair.check(computation, &unrelated()), Ok(()));
    }
}

/// A trava da Partida competitiva local: só `OpponentEngine` joga com ela (FP-02); com uma
/// Partida online por cima, ela também para.
#[test]
fn matrix_competitive_hold_spares_only_the_opponent_engine() {
    let fair = fair();
    fair.follow_competitive(CompetitiveGame::OnBoard);
    for computation in Computation::ALL {
        let expected = if computation == Computation::OpponentEngine {
            Ok(())
        } else {
            locked(computation, LockScope::Everything)
        };
        assert_eq!(
            fair.check(computation, &unrelated()),
            expected,
            "{computation:?}"
        );
        assert_eq!(
            fair.check_anywhere(computation),
            expected,
            "{computation:?}"
        );
    }
    let online = fair.guard().hold("Partida online contra pessoa");
    for computation in Computation::ALL {
        assert_eq!(
            fair.check(computation, &unrelated()),
            locked(computation, LockScope::Everything),
            "{computation:?}"
        );
    }
    drop(online);
    fair.follow_competitive(CompetitiveGame::None);
    for computation in Computation::ALL {
        assert_eq!(fair.check(computation, &unrelated()), Ok(()));
    }
}

/// A Partida competitiva do usuário guardada por baixo do Treino ou do modo espectador (FP-06):
/// tudo recusado em todos os pontos de entrada, inclusive a Engine adversária de outra Partida.
/// Só a adversária de uma Partida competitiva local em andamento escapa (FP-02: a que volta ao
/// tabuleiro antes de a trava ver a troca); com uma Partida online por cima, nem ela.
#[test]
fn matrix_competitive_game_aside_refuses_everything_but_a_competitive_opponent() {
    let fair = fair();
    fair.follow_competitive(CompetitiveGame::Aside);
    let start = CanonicalPosition::start();
    let assisted = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    let competitive = Game::new(GameSetup::standard(chess_game::PolicyKind::FideSimulation));
    for computation in Computation::ALL {
        let refused = locked(computation, LockScope::Everything);
        assert_eq!(
            fair.check(computation, &unrelated()),
            refused,
            "{computation:?}"
        );
        assert_eq!(fair.check_fen(computation, &start.to_fen()), refused);
        assert_eq!(fair.check_fen(computation, "lixo"), refused);
        assert_eq!(fair.check_game(computation, &assisted), refused);
        assert_eq!(fair.check_anywhere(computation), refused);
        assert_eq!(fair.check_start(computation, Some(&start)), refused);
        assert_eq!(fair.check_start(computation, None), refused);
        let own = if computation == Computation::OpponentEngine {
            Ok(())
        } else {
            refused
        };
        assert_eq!(
            fair.check_game(computation, &competitive),
            own,
            "{computation:?}"
        );
    }
    let online = fair.guard().hold("Partida online contra pessoa");
    assert!(
        fair.check_game(Computation::OpponentEngine, &competitive)
            .is_err()
    );
    drop(online);
    // De volta ao tabuleiro: a adversária do tabuleiro joga; ao acabar, tudo livre.
    fair.follow_competitive(CompetitiveGame::OnBoard);
    assert_eq!(
        fair.check_game(Computation::OpponentEngine, &assisted),
        Ok(())
    );
    fair.follow_competitive(CompetitiveGame::None);
    for computation in Computation::ALL {
        assert_eq!(fair.check(computation, &unrelated()), Ok(()));
    }
    assert!(!fair.any());
}

/// Partida em tempo real contra pessoa, sem a sessão aberta: nada calcula, em Posição nenhuma.
#[test]
fn matrix_real_time_game_refuses_every_computation_everywhere() {
    let fair = fair();
    let mut game = online_game("rt1", None, Opponent::Human);
    feed(&mut game, &["e2e4"], None);
    fair.follow_games([&game]);
    for computation in Computation::ALL {
        let refused = locked(computation, LockScope::Everything);
        assert_eq!(
            fair.check(computation, &unrelated()),
            refused,
            "{computation:?}"
        );
        assert_eq!(fair.check_anywhere(computation), refused, "{computation:?}");
        assert_eq!(
            fair.check_start(computation, None),
            refused,
            "{computation:?}"
        );
    }
    // O fim confirmado solta.
    feed(&mut game, &["e2e4"], Some(ProviderEnd::Aborted));
    fair.follow_games([&game]);
    for computation in Computation::ALL {
        assert_eq!(fair.check(computation, &unrelated()), Ok(()));
        assert_eq!(fair.check_anywhere(computation), Ok(()));
    }
}

/// Correspondência em andamento: a própria Partida (`ThisGame`), a estrutura da Posição
/// (`ThisPosition`), e liberdade para o resto. Em todos os serviços.
#[test]
fn matrix_correspondence_locks_game_and_structure_but_not_the_rest() {
    let fair = fair();
    let game = correspondence("cor1");
    fair.follow_games([&game]);
    let own = Subject::at(&game, 4).unwrap();
    let same_structure = Subject::position(&after(&SCOTCH));
    let free = unrelated();
    let start = CanonicalPosition::start();
    for computation in Computation::ALL {
        assert_eq!(
            fair.check(computation, &own),
            locked(computation, LockScope::ThisGame),
            "{computation:?}"
        );
        assert_eq!(
            fair.check(computation, &same_structure),
            locked(computation, LockScope::ThisPosition),
            "{computation:?}"
        );
        assert_eq!(fair.check(computation, &free), Ok(()), "{computation:?}");
        // A Posição inicial (a abertura comum) não trava.
        assert_eq!(
            fair.check(computation, &Subject::position(&start)),
            Ok(()),
            "{computation:?}"
        );
        // Sem Posição conhecida: espera qualquer trava.
        assert_eq!(
            fair.check_start(computation, None),
            locked(computation, LockScope::ThisPosition),
            "{computation:?}"
        );
        assert_eq!(
            fair.check_start(computation, Some(&same_structure_position())),
            locked(computation, LockScope::ThisPosition),
            "{computation:?}"
        );
        // Ligar o serviço antes de saber a Posição é permitido; a Posição é conferida depois.
        assert_eq!(fair.check_anywhere(computation), Ok(()), "{computation:?}");
        // FEN ilegível com trava ligada: recusa; FEN legível e livre: passa.
        assert_eq!(
            fair.check_fen(computation, "lixo"),
            locked(computation, LockScope::Everything),
            "{computation:?}"
        );
        assert_eq!(
            fair.check_fen(computation, &after(&["d2d4", "d7d5", "c2c4"]).to_fen()),
            Ok(()),
            "{computation:?}"
        );
    }
}

fn same_structure_position() -> CanonicalPosition {
    after(&SCOTCH)
}

/// `check_game` (Engine adversária): a Partida local livre passa, a que descende da Posição
/// travada não, e uma Partida que não se refaz trava por dúvida.
#[test]
fn matrix_check_game_follows_the_lineage_of_the_shown_game() {
    let fair = fair();
    fair.follow_games([&correspondence("cor1")]);
    let free = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    assert_eq!(fair.check_game(Computation::OpponentEngine, &free), Ok(()));
    let mut study = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    for (i, m) in SCOTCH.iter().enumerate() {
        let mv: Move = m.parse().unwrap();
        study
            .handle(
                Command::Release {
                    from: mv.from(),
                    to: mv.to(),
                },
                Timestamp::mono(i as u64),
            )
            .unwrap();
    }
    assert_eq!(
        fair.check_game(Computation::OpponentEngine, &study),
        locked(Computation::OpponentEngine, LockScope::ThisPosition)
    );
}

/// Lock envenenado: na dúvida, trava (ramo defensivo de `check` e de `everything_locked`).
#[test]
fn matrix_a_poisoned_state_lock_refuses_everything() {
    let fair = fair();
    let shared = fair.state.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared.write().unwrap();
        panic!("envenena o lock de propósito");
    })
    .join();
    assert!(fair.state.is_poisoned());
    for computation in Computation::ALL {
        assert_eq!(
            fair.check(computation, &unrelated()),
            locked(computation, LockScope::Everything),
            "{computation:?}"
        );
    }
    assert!(fair.everything_locked());
}

/// Partida contra a Engine do Provedor ou local: não entra na trava (ramo negativo de
/// `follow_games`).
#[test]
fn matrix_provider_engine_and_local_games_are_not_followed() {
    let fair = fair();
    let mut vs_engine = online_game("ai1", None, Opponent::ProviderEngine { level: 2 });
    feed(&mut vs_engine, &["e2e4"], None);
    let local = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
    fair.follow_games([&vs_engine, &local]);
    assert_eq!(fair.followed(), 0);
    assert!(!fair.any());
}

// --- A matriz documentada cobre toda a `Computation` ---------------------------------------

const SOURCE: &str = include_str!("fair_play.rs");
const MATRIX: &str = include_str!("../../../docs/seguranca/matriz-ramos-v4.md");

/// Os nomes das variantes de `Computation`, lidos do código-fonte.
fn computation_variants() -> Vec<String> {
    let body = SOURCE
        .split("pub enum Computation {")
        .nth(1)
        .expect("enum Computation")
        .split("\n}\n")
        .next()
        .expect("fim do enum");
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.strip_suffix(','))
        .filter(|name| name.chars().all(|c| c.is_ascii_alphanumeric()) && !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_computation_variant_is_tracked_in_all() {
    let variants = computation_variants();
    assert!(!variants.is_empty());
    assert_eq!(
        variants.len(),
        Computation::ALL.len(),
        "Computation::ALL precisa listar todas as variantes: {variants:?}"
    );
    for name in &variants {
        assert!(
            Computation::ALL.iter().any(|c| format!("{c:?}") == *name),
            "{name} falta em Computation::ALL"
        );
    }
}

/// Falha se um novo valor de `Computation` não tiver linha na matriz de ramos
/// (`docs/seguranca/matriz-ramos-v4.md`).
#[test]
fn every_computation_has_a_row_in_the_branch_matrix() {
    for name in computation_variants() {
        let marker = format!("`Computation::{name}`");
        assert!(
            MATRIX
                .lines()
                .any(|l| l.starts_with('|') && l.contains(&marker)),
            "Computation::{name} não tem linha em docs/seguranca/matriz-ramos-v4.md"
        );
    }
}
