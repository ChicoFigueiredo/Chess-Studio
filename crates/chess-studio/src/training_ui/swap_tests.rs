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
fn frame(training: &mut TrainingUi, p: &mut Persistence, session: &mut Session) {
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
    frame(&mut training, &mut p, &mut session);
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
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(s.training.session_swapped());
    assert_eq!(rows(&mut s.p), before);
    // Repetir o Exercício troca a Sessão do próprio Treino e não é "troca de fora".
    s.training.start(
        "afogamento-001",
        &mut s.session,
        &AidSettings::physical(),
        None,
    );
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(s.training.session_swapped());
    s.training.leave_for_tests(&mut s.session);
    assert_eq!(s.session.game().events().len(), 1, "a d4 volta intacta");
}

#[test]
fn new_game_during_an_exercise_leaves_the_training_and_keeps_the_stored_game() {
    let mut s = scene();
    let before = rows(&mut s.p);
    s.session.new_game();
    frame(&mut s.training, &mut s.p, &mut s.session);
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
    frame(&mut s.training, &mut s.p, &mut s.session);
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
    frame(&mut s.training, &mut s.p, &mut s.session);
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
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert_eq!(rows(&mut s.p), opened);
}

#[test]
fn an_online_game_starting_during_an_exercise_leaves_the_training_too() {
    let mut s = scene();
    let before = rows(&mut s.p);
    let game = Game::new(online_setup());
    s.session.show_online(game, None, true);
    frame(&mut s.training, &mut s.p, &mut s.session);
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

// ---- #309: casos de borda -----------------------------------------------------------------

use crate::new_game::NewGameConfig;
use crate::persistence::PersistenceHandle;
use chess_game::PolicyKind;
use std::sync::Mutex;

/// A Partida da Engine online que o modo espectador mostra (provedor terminado em `-bot`).
fn engine_game() -> Game {
    let player = |name: &str| OnlinePlayer {
        name: name.into(),
        rating: None,
    };
    Game::new(
        GameSetup::online(
            OnlineGame {
                provider: "lichess-bot".into(),
                remote_id: "En9ineQ1".into(),
                local_color: chess_core::Color::White,
                opponent: Opponent::Human,
                white: player("Motor"),
                black: player("Salma"),
                rated: false,
                days_per_move: None,
            },
            Variant::Standard,
            None,
        )
        .unwrap(),
    )
}

fn count(rows: &[(GameId, u64)], id: GameId) -> u64 {
    rows.iter().find(|(i, _)| *i == id).unwrap().1
}

/// A Partida competitiva local do usuário (e4 jogado), com o Treino aberto sobre ela.
fn competitive_scene() -> Scene {
    let mut p = Persistence::new(Store::open_in_memory().unwrap());
    let mut session = p.restore().session;
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2", "e4");
    session.command(Command::PressClock);
    p.sync(&session);
    let kept = p.current_game().unwrap();
    assert!(session.competitive_in_progress());
    let mut training = TrainingUi::default();
    training.open(None);
    training.start(
        "afogamento-001",
        &mut session,
        &AidSettings::physical(),
        None,
    );
    frame(&mut training, &mut p, &mut session);
    Scene {
        p,
        session,
        training,
        first: kept,
        kept,
    }
}

/// A Partida online ao vivo do usuário, gravada no banco, com o Treino aberto sobre ela.
fn live_online_scene() -> Scene {
    let mut p = Persistence::new(Store::open_in_memory().unwrap());
    let mut session = p.restore().session;
    play(&mut session, "e2", "e4");
    p.sync(&session);
    let first = p.current_game().unwrap();
    let game = Game::new(online_setup());
    let id = {
        let (store, profile) = p.store_and_profile();
        store
            .create_game_for(profile, game.setup(), &chess_store::GameMetadata::default())
            .unwrap()
    };
    session.show_online(game, Some(id), true);
    p.sync(&session);
    assert_eq!(p.current_game(), Some(id));
    let mut training = TrainingUi::default();
    training.open(None);
    training.start(
        "afogamento-001",
        &mut session,
        &AidSettings::physical(),
        None,
    );
    frame(&mut training, &mut p, &mut session);
    assert!(training.session_swapped());
    Scene {
        p,
        session,
        training,
        first,
        kept: id,
    }
}

#[test]
fn watching_during_an_exercise_hands_the_users_game_back_underneath() {
    let mut s = competitive_scene();
    let before = rows(&mut s.p);
    assert!(s.session.watch(&engine_game()));
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(!s.training.session_swapped(), "ver sai do Treino");
    assert!(s.session.is_spectator());
    // Por baixo do espectador está a Partida competitiva do usuário, não a do Exercício.
    assert!(s.session.own().competitive_in_progress());
    let n = count(&before, s.kept);
    assert_eq!(s.session.own().game().events().len() as u64, n);
    assert_eq!(rows(&mut s.p), before, "nada gravado a mais");
    assert_eq!(s.p.current_game(), Some(s.kept));
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert_eq!(rows(&mut s.p), before);
    // Sair do espectador devolve a Partida do usuário; o que ela joga vai só para o log dela.
    assert!(s.session.leave_spectator());
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert_eq!(s.session.game().events().len() as u64, n);
    assert!(s.session.competitive_in_progress());
    let len = s.session.game().events().len();
    play(&mut s.session, "e7", "e5");
    assert!(s.session.game().events().len() > len, "o Lance foi aceito");
    frame(&mut s.training, &mut s.p, &mut s.session);
    let after = rows(&mut s.p);
    assert_eq!(after.len(), before.len());
    assert!(
        count(&after, s.kept) > n,
        "o Lance novo vai para o log da Partida do usuário"
    );
}

#[test]
fn the_exercise_over_the_spectator_comes_and_goes_without_touching_the_users_game() {
    let mut p = Persistence::new(Store::open_in_memory().unwrap());
    let mut session = p.restore().session;
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2", "e4");
    session.command(Command::PressClock);
    p.sync(&session);
    let kept = p.current_game().unwrap();
    let mut training = TrainingUi::default();
    let before = rows(&mut p);
    assert!(session.watch(&engine_game()));
    frame(&mut training, &mut p, &mut session);
    training.start_for_tests("afogamento-001", &mut session);
    frame(&mut training, &mut p, &mut session);
    assert!(training.session_swapped());
    assert!(training.kept().is_some_and(Session::is_spectator));
    assert_eq!(rows(&mut p), before);
    training.leave_for_tests(&mut session);
    frame(&mut training, &mut p, &mut session);
    assert!(session.is_spectator());
    assert_eq!(rows(&mut p), before);
    // Ver outra vez com o Exercício por cima: um só espectador, a Partida do usuário por baixo.
    training.start_for_tests("afogamento-001", &mut session);
    frame(&mut training, &mut p, &mut session);
    assert!(session.watch(&engine_game()));
    frame(&mut training, &mut p, &mut session);
    assert!(!training.session_swapped());
    assert!(session.is_spectator());
    assert!(
        session.own().competitive_in_progress(),
        "sem espectador aninhado"
    );
    assert!(session.leave_spectator());
    assert!(!session.is_spectator(), "um só nível de espectador");
    assert_eq!(session.game().events().len() as u64, count(&before, kept));
    frame(&mut training, &mut p, &mut session);
    assert_eq!(rows(&mut p), before);
}

#[test]
fn resuming_another_game_with_a_live_online_game_kept_loses_nothing() {
    let mut s = live_online_scene();
    let before = rows(&mut s.p);
    let game = s.p.switch_to_game(s.first).unwrap();
    s.session.open_game(game);
    s.p.track(&s.session);
    let opened = rows(&mut s.p);
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(!s.training.session_swapped());
    assert!(!s.session.is_online());
    assert_eq!(s.session.snapshot().history[0].san, "e4");
    assert_eq!(rows(&mut s.p), opened);
    assert_eq!(opened.len(), before.len());
    let online = opened.iter().find(|(id, _)| *id == s.kept).unwrap();
    assert_eq!(online.1, 0, "a Partida online guardada não ganhou Evento");
    play(&mut s.session, "e7", "e5");
    frame(&mut s.training, &mut s.p, &mut s.session);
    let last = rows(&mut s.p);
    assert_eq!(last.iter().find(|(id, _)| *id == s.first).unwrap().1, 2);
    assert_eq!(last.iter().find(|(id, _)| *id == s.kept).unwrap().1, 0);
}

#[test]
fn watching_with_a_live_online_game_kept_gives_it_back_on_leaving() {
    let mut s = live_online_scene();
    let before = rows(&mut s.p);
    assert!(s.session.watch(&engine_game()));
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(s.session.own().is_online());
    assert_eq!(s.session.own().online_store_id(), Some(s.kept));
    assert_eq!(rows(&mut s.p), before);
    assert!(s.session.leave_spectator());
    frame(&mut s.training, &mut s.p, &mut s.session);
    assert!(s.session.is_online());
    assert_eq!(s.p.current_game(), Some(s.kept));
    assert_eq!(rows(&mut s.p), before);
}

// ---- Os sistemas do Bevy, na ordem real (guarda e depois `persist_session`) ---------------

fn into_app(s: Scene) -> (App, GameId) {
    let Scene {
        p,
        session,
        training,
        kept,
        ..
    } = s;
    let mut app = App::new();
    app.insert_resource(session)
        .insert_resource(training)
        .insert_resource(PersistenceHandle(Mutex::new(p)))
        .add_systems(
            Update,
            (training_guard, crate::persistence::persist_session).chain(),
        );
    (app, kept)
}

fn app_rows(app: &App) -> Vec<(GameId, u64)> {
    let mut p = app
        .world()
        .resource::<PersistenceHandle>()
        .0
        .lock()
        .unwrap();
    rows(&mut p)
}

#[test]
fn the_systems_in_the_real_order_never_write_the_exercise_or_duplicate_a_log() {
    let (mut app, kept) = into_app(scene());
    let before = app_rows(&app);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app_rows(&app), before, "o Exercício não é gravado");
    // Nova Partida durante o Exercício: no mesmo quadro o guarda sai do Treino e só então a
    // persistência grava, na Partida nova.
    app.world_mut().resource_mut::<Session>().new_game();
    app.update();
    assert!(!app.world().resource::<TrainingUi>().session_swapped());
    let opened = app_rows(&app);
    assert_eq!(opened.len(), before.len() + 1);
    assert_eq!(&opened[..before.len()], &before[..]);
    play(&mut app.world_mut().resource_mut::<Session>(), "c2", "c4");
    app.update();
    let last = app_rows(&app);
    assert_eq!(&last[..before.len()], &before[..]);
    assert_eq!(last[before.len()].1, 1);
    assert!(last.iter().any(|(id, _)| *id == kept));
}

#[test]
fn watching_in_the_real_order_keeps_the_users_log_and_never_records_the_exercise() {
    let (mut app, kept) = into_app(competitive_scene());
    let before = app_rows(&app);
    assert!(
        app.world_mut()
            .resource_mut::<Session>()
            .watch(&engine_game())
    );
    app.update();
    app.update();
    assert_eq!(app_rows(&app), before);
    assert!(
        app.world()
            .resource::<Session>()
            .own()
            .competitive_in_progress()
    );
    assert!(app.world_mut().resource_mut::<Session>().leave_spectator());
    play(&mut app.world_mut().resource_mut::<Session>(), "e7", "e5");
    app.update();
    let after = app_rows(&app);
    assert_eq!(after.len(), before.len());
    assert!(count(&after, kept) > count(&before, kept));
}
