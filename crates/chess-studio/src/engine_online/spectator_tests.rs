//! "Ver no tabuleiro" (#302): a Partida da Engine online no tabuleiro, só para ver. Pelo
//! serviço da Engine online com o servidor falso (nenhuma chamada ao Lichess), a `Session` do
//! app, a persistência num banco em memória e a Trava de fair play do `NetService`. Apelidos
//! inventados; respostas sintéticas no formato da Bot API.

#![allow(clippy::unwrap_used)]

use super::service::{Activity, Settings};
use super::spectator::{self, Banner, SpectatorUi, Watch};
use super::tests::{Env, GAME, WAIT, env, full, move_path, ok, scripted, spec, state};
use super::view::Intent;
use super::{EngineOnlineUi, apply_board_intent};
use crate::new_game::NewGameConfig;
use crate::online::service::StoreAccess;
use crate::persistence::Persistence;
use crate::session::{OnlineIntent, Session};
use chess_core::{Color, Square};
use chess_game::{
    Command, Game, GameSetup, OnlineGame, OnlinePlayer, Opponent, PolicyKind, ProviderSnapshot,
    TimeControl, Timestamp as GameTime, Variant,
};
use chess_input::{GestureEvent, Modifiers, MoveMethod, PointerButton};
use chess_net::testing::LiveFeed;
use chess_store::{GameQuery, Store};
use std::time::{Duration, Instant};

fn sq(name: &str) -> Square {
    name.parse().unwrap()
}

fn play(session: &mut Session, from: &str, to: &str) {
    session.command(Command::Release {
        from: sq(from),
        to: sq(to),
    });
}

fn tap(session: &mut Session, square: &str) {
    session.gesture(GestureEvent::PointerDown {
        button: PointerButton::Primary,
        square: Some(sq(square)),
        modifiers: Modifiers::default(),
    });
    session.gesture(GestureEvent::PointerUp {
        square: Some(sq(square)),
    });
}

fn drag(session: &mut Session, from: &str, to: &str) {
    session.gesture(GestureEvent::PointerDown {
        button: PointerButton::Primary,
        square: Some(sq(from)),
        modifiers: Modifiers::default(),
    });
    session.gesture(GestureEvent::PointerMove {
        square: Some(sq(to)),
        world: chess_input::square_center(sq(to)),
    });
    session.gesture(GestureEvent::PointerUp {
        square: Some(sq(to)),
    });
}

/// Uma Partida da Engine online em andamento: a Engine joga de brancas (a vez é dela desde o
/// início) e responde com `moves`, um por vez.
fn engine_game(moves: &[&str]) -> (Env, LiveFeed) {
    let mut env = env();
    for uci in ["e2e4", "d2d4"] {
        env.server.route(&move_path(GAME, uci), ok());
    }
    env.server
        .route(&format!("/api/bot/game/{GAME}/resign"), ok());
    let (factory, _) = scripted(moves);
    env.start(Settings::default(), factory);
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(
        &full(GAME, "Salma")
            .to_string()
            .replace("\"motor\"", "\"salma\""),
    );
    assert!(env.until(|e| e.run.game(GAME).is_some()));
    (env, feed)
}

/// Uma Partida online contra pessoa (para conferir que só a da Engine entra no modo espectador).
fn human_game(moves: &[&str]) -> Game {
    let setup = GameSetup::online(
        OnlineGame {
            provider: "lichess".into(),
            remote_id: "pEssoa01".into(),
            local_color: Color::White,
            opponent: Opponent::Human,
            white: OnlinePlayer {
                name: "Voce_Demo".into(),
                rating: None,
            },
            black: OnlinePlayer {
                name: "Coruja_Azul".into(),
                rating: None,
            },
            rated: false,
            days_per_move: Some(3),
        },
        Variant::Standard,
        None,
    )
    .unwrap();
    let mut game = Game::new(setup);
    game.handle(
        Command::Provider(ProviderSnapshot {
            moves: moves.iter().map(|m| (*m).to_owned()).collect(),
            clock: None,
            draw_offer: None,
            end: None,
        }),
        GameTime::mono(1),
    )
    .unwrap();
    game
}

fn watch(session: &mut Session, env: &Env) {
    let mut ui = SpectatorUi::default();
    if spectator::request(&mut ui, session, &env.run, GAME) == Watch::Confirm {
        assert_eq!(
            spectator::confirm(&mut ui, session, &env.run),
            Watch::Watching
        );
    }
    assert!(session.is_spectator());
}

#[test]
fn the_board_shows_only_what_the_provider_confirmed_and_follows_each_new_move() {
    let (mut env, feed) = engine_game(&["e2e4"]);
    let mut session = Session::new();
    let mut ui = SpectatorUi::default();
    // Sem Partida própria em andamento, a vista troca sem perguntar.
    assert_eq!(
        spectator::request(&mut ui, &mut session, &env.run, GAME),
        Watch::Watching
    );
    assert!(session.is_spectator() && ui.confirm.is_none());
    assert_eq!(session.online_remote_id(), Some(GAME));
    assert!(session.snapshot().history.is_empty());

    // A Engine envia o Lance dela; o Lichess ainda não confirmou.
    assert!(env.until(|e| e.posts_to(&move_path(GAME, "e2e4")) == 1));
    assert!(
        env.run.game(GAME).unwrap().sent_move().is_some(),
        "o controlador tem a intenção enviada"
    );
    spectator::follow(&mut session, &env.run);
    assert!(
        session.snapshot().history.is_empty(),
        "só o que foi confirmado"
    );
    assert!(
        session
            .snapshot()
            .online
            .as_ref()
            .is_none_or(|o| o.sent_move.is_none()),
        "a intenção enviada da Engine não aparece"
    );
    assert!(session.snapshot().made_move.is_none());
    assert!(session.snapshot().position.piece_at(sq("e2")).is_some());

    // O Lichess confirma e o adversário responde: o tabuleiro acompanha, Lance a Lance.
    feed.push(&state("e2e4", "started").to_string());
    assert!(env.until(|e| e.run.game(GAME).unwrap().snapshot().history.len() == 1));
    spectator::follow(&mut session, &env.run);
    assert_eq!(session.snapshot().history.len(), 1);
    assert!(session.snapshot().position.piece_at(sq("e4")).is_some());
    feed.push(&state("e2e4 e7e5", "started").to_string());
    assert!(env.until(|e| e.run.game(GAME).unwrap().snapshot().history.len() == 2));
    spectator::follow(&mut session, &env.run);
    assert_eq!(session.snapshot().history.len(), 2);
    assert!(session.snapshot().position.piece_at(sq("e5")).is_some());

    // A Partida termina: a faixa conta o resultado; o tabuleiro fica no último Lance.
    feed.push(
        &serde_json::json!({
            "type": "gameState", "moves": "e2e4 e7e5", "wtime": 300_000, "btime": 290_000,
            "winc": 2000, "binc": 2000, "status": "resign", "winner": "white",
        })
        .to_string(),
    );
    assert!(env.until(|e| e.run.game(GAME).is_some_and(|g| g.snapshot().is_over())));
    spectator::follow(&mut session, &env.run);
    let banner = Banner::of(&session, Some(&env.run)).unwrap();
    assert_eq!(banner.ended, Some(Some("1-0".to_owned())));
    assert!(banner.engine_white && !banner.gone);
    feed.close();
    assert!(env.until(|e| e.run.game(GAME).is_none()));
    spectator::follow(&mut session, &env.run);
    assert!(session.is_spectator(), "o usuário decide quando sair");
    assert_eq!(session.snapshot().history.len(), 2);
}

#[test]
fn no_gesture_or_command_on_the_watched_board_makes_a_move_or_an_intent() {
    let (mut env, _feed) = engine_game(&[]);
    let mut session = Session::new();
    session.set_move_method(MoveMethod::Both);
    watch(&mut session, &env);
    let events = session.game().events().len();
    let posts = env.posts();

    // As peças das duas cores, por toque e por arrasto, e todas as Ações de jogo.
    tap(&mut session, "e2");
    tap(&mut session, "e4");
    drag(&mut session, "d2", "d4");
    drag(&mut session, "e7", "e5");
    for command in [
        Command::Resign { side: Color::Black },
        Command::OfferDraw { side: Color::Black },
        Command::AcceptDraw,
        Command::ClaimDraw,
        Command::Undo,
        Command::PressClock,
        Command::Pause,
    ] {
        session.command(command);
    }
    session.resign_side_to_move();
    session.toggle_pause();
    session.press_clock();
    session.claim_draw(None);
    session.online_request(OnlineIntent::Resign);
    session.online_request(OnlineIntent::Play(chess_core::Move::new(
        sq("e2"),
        sq("e4"),
        None,
    )));

    assert!(session.take_online_intents().is_empty(), "nenhuma intenção");
    assert!(!session.online_can_move(), "o tabuleiro nunca fica ao vivo");
    session.set_online_live(true);
    assert!(!session.online_can_move());
    assert_eq!(session.game().events().len(), events, "nenhum Lance");
    assert!(session.snapshot().history.is_empty());
    // A Partida do usuário, por baixo, também não recebeu nada.
    assert!(session.own_mut().take_online_intents().is_empty());
    assert!(session.own().snapshot().history.is_empty());
    // Nada saiu para o Lichess por causa do tabuleiro.
    for _ in 0..10 {
        env.frame();
        spectator::follow(&mut session, &env.run);
    }
    assert_eq!(env.posts(), posts);
}

#[test]
fn watching_the_online_engine_does_not_turn_the_fair_play_lock_on() {
    let (env, _feed) = engine_game(&[]);
    let mut session = Session::new();
    watch(&mut session, &env);
    let mut engaged = false;
    env.service.follow_session(&session, &mut engaged);
    assert!(!session.competitive_in_progress());
    assert_eq!(
        crate::hud_policy::allowance(&session),
        crate::hud_policy::Allowance::ALL,
        "é a Engine jogando (AD-05)"
    );
    assert!(!engaged && !env.service.fair_play().any());
    assert_eq!(env.service.fair_play().followed(), 0);
}

#[test]
fn the_users_own_competitive_game_still_locks_the_online_engine_while_watching_fp04() {
    let (mut env, feed) = engine_game(&["e2e4", "d2d4"]);
    assert!(env.until(|e| e.posts_to(&move_path(GAME, "e2e4")) == 1));
    feed.push(&state("e2e4", "started").to_string());
    // A Partida competitiva local do usuário em andamento segura a trava.
    let mut session = Session::new();
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2", "e4");
    assert!(session.competitive_in_progress());
    let mut engaged = false;
    env.service.follow_session(&session, &mut engaged);
    assert!(engaged && env.service.fair_play().any());

    // Ver a Partida da Engine não solta a trava da Partida do usuário, guardada por baixo.
    watch(&mut session, &env);
    env.service.follow_session(&session, &mut engaged);
    assert!(engaged && env.service.fair_play().any());

    // FP-04: no Lance seguinte, a Engine online não recebe a Posição e a Partida é abandonada.
    feed.push(&state("e2e4 e7e5", "started").to_string());
    assert!(env.until(|e| {
        e.run
            .activity()
            .any(|a| matches!(a, Activity::FairPlayStopped { .. }))
    }));
    assert!(env.until(|e| e.posts_to(&format!("/api/bot/game/{GAME}/resign")) == 1));
    assert_eq!(
        env.posts_to(&move_path(GAME, "d2d4")),
        0,
        "a Engine não jogou"
    );

    // Ao sair, a Partida do usuário volta e a trava segue com ela.
    assert!(session.leave_spectator());
    env.service.follow_session(&session, &mut engaged);
    assert!(engaged && session.competitive_in_progress());
}

/// Um quadro do app: o serviço da Engine grava no banco do app, o modo espectador acompanha e a
/// persistência grava o que é dela.
fn app_frame(env: &mut Env, persistence: &mut Persistence, session: &mut Session) {
    {
        let (store, profile) = persistence.store_and_profile();
        env.run.frame(Some(StoreAccess { store, profile }));
    }
    spectator::follow(session, &env.run);
    persistence.sync(session);
}

fn app_until(
    env: &mut Env,
    persistence: &mut Persistence,
    session: &mut Session,
    mut cond: impl FnMut(&Env, &Session) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < WAIT {
        app_frame(env, persistence, session);
        if cond(env, session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn events_of(persistence: &mut Persistence, id: chess_store::GameId) -> usize {
    let (store, _) = persistence.store_and_profile();
    store.load_game(id).unwrap().events().len()
}

fn rows(persistence: &mut Persistence) -> usize {
    let (store, _) = persistence.store_and_profile();
    store.list_games(&GameQuery::default()).unwrap().len()
}

#[test]
fn nothing_is_stored_twice_and_the_users_game_stays_the_current_one() {
    let mut env = env();
    env.server.route(&move_path(GAME, "e2e4"), ok());
    // O banco do app: a persistência grava a Partida do usuário; o serviço, a da Engine.
    let mut persistence = Persistence::new(std::mem::replace(
        &mut env.store,
        Store::open_in_memory().unwrap(),
    ));
    let mut session = Session::new();
    play(&mut session, "e2", "e4");
    play(&mut session, "e7", "e5");
    persistence.sync(&session);
    let user_game = persistence.current_game().unwrap();
    let user_events = session.game().events().len();
    assert_eq!(events_of(&mut persistence, user_game), user_events);

    let (factory, _) = scripted(&["e2e4"]);
    env.run
        .start(&env.service, spec(), Settings::default(), factory)
        .unwrap();
    assert!(env.events.wait_connected(WAIT));
    let feed = env.open_game(GAME, "Salma");
    assert!(app_until(
        &mut env,
        &mut persistence,
        &mut session,
        |_, _| { feed.wait_connected(Duration::from_millis(1)) }
    ));
    feed.push(
        &full(GAME, "Salma")
            .to_string()
            .replace("\"motor\"", "\"salma\""),
    );
    assert!(app_until(
        &mut env,
        &mut persistence,
        &mut session,
        |e, _| { e.run.game(GAME).is_some() }
    ));
    watch(&mut session, &env);

    assert!(app_until(
        &mut env,
        &mut persistence,
        &mut session,
        |e, _| { e.posts_to(&move_path(GAME, "e2e4")) == 1 }
    ));
    feed.push(&state("e2e4", "started").to_string());
    feed.push(&state("e2e4 e7e5", "started").to_string());
    assert!(app_until(
        &mut env,
        &mut persistence,
        &mut session,
        |_, s| { s.snapshot().history.len() == 2 }
    ));
    for _ in 0..5 {
        app_frame(&mut env, &mut persistence, &mut session);
    }

    // Duas Partidas no banco: a do usuário e a da Engine, cada uma uma vez, com o seu log.
    assert_eq!(rows(&mut persistence), 2);
    assert_eq!(
        persistence.current_game(),
        Some(user_game),
        "a atual é a do usuário"
    );
    assert_eq!(events_of(&mut persistence, user_game), user_events);
    let engine_game = {
        let (store, _) = persistence.store_and_profile();
        store
            .list_games(&GameQuery::default())
            .unwrap()
            .into_iter()
            .find(|g| g.metadata["online"]["remote_id"] == GAME)
            .unwrap()
            .id
    };
    assert_eq!(
        events_of(&mut persistence, engine_game),
        env.run.game(GAME).unwrap().events().len(),
        "só os Eventos do controlador, uma vez"
    );

    // Sair: a Partida do usuário volta, na mesma linha do banco.
    assert!(session.leave_spectator());
    app_frame(&mut env, &mut persistence, &mut session);
    assert_eq!(rows(&mut persistence), 2);
    assert_eq!(persistence.current_game(), Some(user_game));
    play(&mut session, "g1", "f3");
    persistence.sync(&session);
    assert_eq!(
        events_of(&mut persistence, user_game),
        session.game().events().len()
    );
    assert_eq!(rows(&mut persistence), 2, "nenhuma Partida nova");
}

#[test]
fn leaving_restores_the_session_that_was_on_the_board() {
    let (env, _feed) = engine_game(&[]);
    let mut session = Session::new();
    session.set_move_method(MoveMethod::TwoTaps);
    play(&mut session, "e2", "e4");
    play(&mut session, "c7", "c5");
    let events = session.game().events().len();
    let position = session.snapshot().position.clone();
    let epoch = session.epoch();

    // Pela tela da Engine online: "Ver no tabuleiro" (com a pergunta) e a tela sai da frente.
    let mut ui = EngineOnlineUi {
        open: true,
        ..EngineOnlineUi::default()
    };
    let mut spectator_ui = SpectatorUi::default();
    apply_board_intent(
        &Intent::Watch(GAME.to_owned()),
        &mut ui,
        &mut spectator_ui,
        &mut session,
        &env.run,
    );
    assert_eq!(
        spectator_ui.confirm.as_deref(),
        Some(GAME),
        "pergunta antes"
    );
    assert!(!session.is_spectator() && ui.open);
    assert_eq!(
        spectator::confirm(&mut spectator_ui, &mut session, &env.run),
        Watch::Watching
    );
    assert_ne!(session.epoch(), epoch);
    assert_eq!(session.own().epoch(), epoch);
    assert_eq!(session.own().game().events().len(), events);
    let watching = session.revision();

    apply_board_intent(
        &Intent::LeaveSpectator,
        &mut ui,
        &mut spectator_ui,
        &mut session,
        &env.run,
    );
    assert!(!session.is_spectator());
    assert_eq!(session.epoch(), epoch);
    assert_eq!(session.game().events().len(), events);
    assert_eq!(session.snapshot().position, position);
    assert_eq!(session.move_method(), MoveMethod::TwoTaps);
    assert_ne!(session.revision(), watching, "a cena se redesenha");
    assert!(!session.leave_spectator(), "fora do modo espectador, nada");
    // A Partida segue de onde estava.
    play(&mut session, "g1", "f3");
    assert_eq!(session.snapshot().history.len(), 3);
}

#[test]
fn the_view_asks_only_when_the_users_own_game_is_in_progress() {
    let (env, _feed) = engine_game(&[]);
    let mut ui = SpectatorUi::default();

    // Partida sem Lance: troca sem perguntar.
    let mut fresh = Session::new();
    assert_eq!(
        spectator::request(&mut ui, &mut fresh, &env.run, GAME),
        Watch::Watching
    );
    // Já no modo espectador, trocar a Partida vista também não pergunta.
    assert_eq!(
        spectator::request(&mut ui, &mut fresh, &env.run, GAME),
        Watch::Watching
    );
    assert!(fresh.leave_spectator() && !fresh.is_spectator());

    // Partida em andamento: pergunta; desistir não muda nada.
    let mut playing = Session::new();
    play(&mut playing, "e2", "e4");
    let epoch = playing.epoch();
    assert_eq!(
        spectator::request(&mut ui, &mut playing, &env.run, GAME),
        Watch::Confirm
    );
    assert!(!playing.is_spectator() && playing.epoch() == epoch);
    spectator::cancel(&mut ui);
    assert!(ui.confirm.is_none());
    assert_eq!(
        spectator::confirm(&mut ui, &mut playing, &env.run),
        Watch::Gone
    );
    assert!(!playing.is_spectator());

    // Partida terminada: troca sem perguntar.
    let mut over = Session::new();
    play(&mut over, "e2", "e4");
    over.resign_side_to_move();
    assert!(over.snapshot().is_over());
    assert_eq!(
        spectator::request(&mut ui, &mut over, &env.run, GAME),
        Watch::Watching
    );

    // Partida que o serviço não tem: nada muda.
    let mut other = Session::new();
    assert_eq!(
        spectator::request(&mut ui, &mut other, &env.run, "naoexiste"),
        Watch::Gone
    );
    assert!(!other.is_spectator() && ui.confirm.is_none());
}

#[test]
fn the_users_local_clock_is_paused_while_watching_and_the_game_is_not_ended() {
    let (env, _feed) = engine_game(&[]);
    let mut session = Session::new();
    session.start(&NewGameConfig {
        time: Some(TimeControl {
            base_ms: 300_000,
            increment_ms: 0,
        }),
        ..NewGameConfig::default()
    });
    play(&mut session, "e2", "e4");
    assert!(spectator::clock_running(&session));
    watch(&mut session, &env);
    assert!(session.own().snapshot().paused, "o relógio dela não corre");
    assert!(!session.own().snapshot().is_over(), "a Partida não acabou");
    assert!(session.leave_spectator());
    assert!(session.snapshot().paused && !session.snapshot().is_over());
    assert_eq!(session.snapshot().history.len(), 1);
}

#[test]
fn only_the_online_engines_game_goes_to_the_board_in_spectator_mode() {
    let mut session = Session::new();
    play(&mut session, "e2", "e4");
    assert!(
        !session.watch(&human_game(&["d2d4"])),
        "Partida de pessoa, nunca"
    );
    assert!(!session.is_spectator());
    assert_eq!(session.snapshot().history.len(), 1);
}

#[test]
fn the_users_online_game_keeps_being_mirrored_underneath_while_watching() {
    let (env, _feed) = engine_game(&[]);
    let mut session = Session::new();
    session.show_online(human_game(&["e2e4"]), None, true);
    assert!(spectator::needs_confirmation(&session));
    watch(&mut session, &env);
    assert_eq!(
        crate::online::board_key(session.own()).map(|k| k.remote_id),
        Some("pEssoa01".to_owned()),
        "o serviço da Partida online segue a do usuário"
    );
    // O Provedor confirma um Lance do adversário: o espelho vai para a Partida do usuário.
    session
        .own_mut()
        .mirror_online(&human_game(&["e2e4", "e7e5"]), true);
    assert!(
        session.snapshot().history.is_empty(),
        "o tabuleiro segue na da Engine"
    );
    assert!(session.leave_spectator());
    assert_eq!(session.snapshot().history.len(), 2, "volta em dia");
    assert!(session.online_can_move(), "e ao vivo");
}

#[test]
fn the_banner_sits_beside_the_board_when_there_is_room_and_above_it_otherwise() {
    use super::spectator::BannerLayout;
    use bevy_egui::egui::{Rect, pos2};
    let area = Rect::from_min_max(pos2(0.0, 160.0), pos2(1228.0, 875.0));
    // Espaço à esquerda do tabuleiro: o cartão cabe ali, sem cobrir casa nenhuma.
    let board = Rect::from_min_max(pos2(290.0, 230.0), pos2(900.0, 850.0));
    let (at, layout) = spectator::banner_place(area, Some(board));
    assert_eq!(at, pos2(16.0, 176.0));
    let BannerLayout::Card { width } = layout else {
        panic!("{layout:?}");
    };
    assert!(at.x + width <= board.left() - 16.0, "{width}");
    // Muito espaço: o cartão não passa da largura máxima.
    let narrow = Rect::from_min_max(pos2(700.0, 230.0), pos2(1100.0, 850.0));
    assert_eq!(
        spectator::banner_place(area, Some(narrow)).1,
        BannerLayout::Card { width: 320.0 }
    );
    // Sem espaço ao lado (ou sem a câmera): uma linha só, no alto da área.
    let wide = Rect::from_min_max(pos2(150.0, 230.0), pos2(1100.0, 850.0));
    assert_eq!(
        spectator::banner_place(area, Some(wide)),
        (pos2(16.0, 168.0), BannerLayout::Chip)
    );
    assert_eq!(spectator::banner_place(area, None).1, BannerLayout::Chip);
}

#[test]
fn the_capture_scenes_of_spectator_mode_show_a_legal_engine_game() {
    for name in [
        "watch-confirm",
        "spectator",
        "spectator-2d",
        "spectator-ended",
    ] {
        assert!(super::demo::Scene::named(name).is_some(), "{name}");
    }
    let i18n = crate::i18n::I18n::pt_br();
    for ended in [false, true] {
        let game = super::demo::watched_game(ended).unwrap();
        assert_eq!(game.snapshot().history.len(), 22, "todos os Lances legais");
        assert_eq!(game.snapshot().is_over(), ended);
        let mut session = Session::new();
        assert!(session.watch(&game));
        let banner = Banner::of(&session, None).unwrap();
        assert!(banner.engine_white && banner.opponent == "TorreDeMadeira");
        assert_eq!(
            banner.line(&i18n),
            "A Engine joga de Brancas contra TorreDeMadeira (1720) no Lichess."
        );
    }
}
