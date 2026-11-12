//! #307: Nova Partida, Retomar, Biblioteca e Partida online durante um Exercício trocam a
//! `Session` do Treino. A regra (docs/decisoes/2026-11-12-treino-troca-de-sessao.md): trocar a
//! Partida do tabuleiro **sai** do Treino; a Partida guardada não volta por cima nem é gravada
//! de novo, e continua no banco, retomável, com o mesmo log.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::persistence::Persistence;
use chess_game::{Game, GameSetup, OnlineGame, OnlinePlayer, Opponent, Variant};
use chess_store::{GameId, GameQuery};

fn sq(name: &str) -> Square {
    name.parse().unwrap()
}

fn play(session: &mut Session, from: &str, to: &str) {
    session.command(Command::Release {
        from: sq(from),
        to: sq(to),
    });
}

/// O que os sistemas fazem em cada quadro: o guarda do Treino e depois `persist_session`
/// (que nem roda enquanto o Exercício está no tabuleiro).
fn frame(training: &mut TrainingUi, p: &mut Persistence, session: &Session) {
    training.leave_if_board_replaced(session);
    if !training.session_swapped() {
        p.sync(session);
    }
}

/// Cada Partida do banco com a contagem de Eventos.
fn rows(p: &mut Persistence) -> Vec<(GameId, u64)> {
    let (store, profile) = p.store_and_profile();
    let mut all: Vec<_> = store
        .list_games(&GameQuery {
            only_unfinished: false,
            profile: Some(profile),
            limit: None,
        })
        .unwrap()
        .into_iter()
        .map(|g| (g.id, g.event_count))
        .collect();
    all.sort_by_key(|(id, _)| id.0);
    all
}

/// A Partida `first` (e4) e a atual `kept` (d4), com o Treino aberto sobre a atual.
struct Scene {
    p: Persistence,
    session: Session,
    training: TrainingUi,
    first: GameId,
    kept: GameId,
}

fn scene() -> Scene {
    let mut p = Persistence::new(Store::open_in_memory().unwrap());
    let mut session = p.restore().session;
    play(&mut session, "e2", "e4");
    p.sync(&session);
    let first = p.current_game().unwrap();
    session.new_game();
    p.sync(&session);
    play(&mut session, "d2", "d4");
    p.sync(&session);
    let kept = p.current_game().unwrap();
    let mut training = TrainingUi::default();
    training.open(None);
    training.start(
        "afogamento-001",
        &mut session,
        &AidSettings::physical(),
        None,
    );
    frame(&mut training, &mut p, &session);
    assert!(training.session_swapped());
    Scene {
        p,
        session,
        training,
        first,
        kept,
    }
}

#[test]
fn the_guard_does_nothing_while_the_exercise_stays_on_the_board() {
    let mut s = scene();
    let before = rows(&mut s.p);
    frame(&mut s.training, &mut s.p, &s.session);
    assert!(s.training.session_swapped());
    assert_eq!(rows(&mut s.p), before);
    // Repetir o Exercício troca a Sessão do próprio Treino e não é "troca de fora".
    s.training.start(
        "afogamento-001",
        &mut s.session,
        &AidSettings::physical(),
        None,
    );
    frame(&mut s.training, &mut s.p, &s.session);
    assert!(s.training.session_swapped());
    s.training.leave_for_tests(&mut s.session);
    assert_eq!(s.session.game().events().len(), 1, "a d4 volta intacta");
}

#[test]
fn new_game_during_an_exercise_leaves_the_training_and_keeps_the_stored_game() {
    let mut s = scene();
    let before = rows(&mut s.p);
    s.session.new_game();
    frame(&mut s.training, &mut s.p, &s.session);
    assert!(!s.training.session_swapped());
    assert_eq!(s.training.view, View::Closed);
    // Sair do Treino agora não devolve a Partida guardada por cima da nova.
    s.training.leave_for_tests(&mut s.session);
    assert!(s.session.game().events().is_empty(), "a Partida nova fica");
    let after = rows(&mut s.p);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "uma Partida nova, nenhuma sumiu"
    );
    assert_eq!(&after[..before.len()], &before[..], "as guardadas, iguais");
    assert_eq!(s.p.resumable_game(), Some(s.kept), "a guardada é retomável");
    // Jogar na nova grava só nela.
    play(&mut s.session, "c2", "c4");
    frame(&mut s.training, &mut s.p, &s.session);
    let last = rows(&mut s.p);
    assert_eq!(&last[..before.len()], &before[..]);
    assert_eq!(last[before.len()].1, 1);
}

#[test]
fn resume_and_library_during_an_exercise_open_the_game_without_duplicating_the_log() {
    let mut s = scene();
    // Retomar e Biblioteca usam o mesmo caminho: `switch_to_game` + `open_game` + `track`.
    let game = s.p.switch_to_game(s.first).unwrap();
    s.session.open_game(game);
    s.p.track(&s.session);
    let opened = rows(&mut s.p);
    frame(&mut s.training, &mut s.p, &s.session);
    assert!(!s.training.session_swapped());
    s.training.leave_for_tests(&mut s.session);
    assert_eq!(s.session.snapshot().history[0].san, "e4", "a aberta fica");
    assert_eq!(s.p.current_game(), Some(s.first));
    assert_eq!(
        rows(&mut s.p),
        opened,
        "nenhum Evento duplicado, nenhuma Partida nova"
    );
    assert_eq!(opened.len(), 2);
    let kept = opened.iter().find(|(id, _)| *id == s.kept).unwrap();
    assert_eq!(kept.1, 1, "a guardada continua com o Lance d4 e nada mais");
    assert_eq!(s.p.resumable_game(), Some(s.kept));
    // Mais um quadro e a Partida aberta continua sem duplicar.
    frame(&mut s.training, &mut s.p, &s.session);
    assert_eq!(rows(&mut s.p), opened);
}

#[test]
fn an_online_game_starting_during_an_exercise_leaves_the_training_too() {
    let mut s = scene();
    let before = rows(&mut s.p);
    let game = Game::new(online_setup());
    s.session.show_online(game, None, true);
    frame(&mut s.training, &mut s.p, &s.session);
    assert!(!s.training.session_swapped());
    s.training.leave_for_tests(&mut s.session);
    assert!(s.session.is_online(), "a Partida online fica no tabuleiro");
    assert_eq!(
        rows(&mut s.p),
        before,
        "o log da guardada não muda nem duplica"
    );
    assert_eq!(s.p.resumable_game(), Some(s.first));
}

fn online_setup() -> GameSetup {
    let player = |name: &str| OnlinePlayer {
        name: name.into(),
        rating: None,
    };
    GameSetup::online(
        OnlineGame {
            provider: "lichess".into(),
            remote_id: "Tr3in0Q1".into(),
            local_color: chess_core::Color::White,
            opponent: Opponent::Human,
            white: player("Ana"),
            black: player("Salma"),
            rated: false,
            days_per_move: None,
        },
        Variant::Standard,
        None,
    )
    .unwrap()
}
