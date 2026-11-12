//! As brechas da Trava de fair play achadas na auditoria do V4 (#261), pela camada do app: a
//! Engine adversária local, a busca por Posição da Biblioteca e a Engine online. Uma Partida
//! online contra pessoa de verdade (servidor falso, relógio falso e cofre falso: nenhuma chamada
//! ao Lichess, nenhuma autorização real) ou a trava ligada pelo mesmo caminho que a sessão usa.
//! As respostas são **sintéticas**, no formato documentado da Board API e da Bot API (as mesmas
//! dos testes do `chess-providers`), com apelidos inventados.

#![allow(clippy::unwrap_used)]

use super::tests::{feed, online_game};
use super::*;
use crate::net_service::NetService;
use crate::new_game::{NewGameConfig, Opponent as NewGameOpponent};
use crate::online::service::{OnlinePlay, StoreAccess};
use chess_core::Color;
use chess_game::{EngineSeat, Opponent, PolicyKind};
use chess_net::testing::{FakeResponse, FakeServer, LiveFeed};
use chess_net::{ConnectionId, FakeClock, Net, NetConfig, ProviderId, Timestamp};
use chess_providers::Seek;
use chess_store::{ConnectionRole, LibraryQuery, ProfileId, Store};
use chess_uci::EngineConfig;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const GAME: &str = "qP8XaFtN";
const WAIT: Duration = Duration::from_secs(10);
const PLAYER: ConnectionId = ConnectionId(1);

/// Correspondência na Najdorf: 1.e4 c5 2.Nf3 d6 3.d4 cxd4 4.Nxd4 Nf6.
const NAJDORF: [&str; 8] = [
    "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6",
];

/// Executável que não existe: o `EnginePlayer` nasce (é o que a trava impede), o processo não.
fn engine_config() -> EngineConfig {
    EngineConfig::new("/nao/existe/engine-adversaria")
}

fn seat(color: Color) -> EngineSeat {
    EngineSeat { color, elo: 1500 }
}

fn position_after(moves: &[&str]) -> chess_core::CanonicalPosition {
    moves
        .iter()
        .fold(chess_core::CanonicalPosition::start(), |p, m| {
            p.play(m.parse::<chess_core::Move>().unwrap()).unwrap()
        })
}

/// Correspondência contra pessoa em andamento, fora do tabuleiro (a trava a segue pelo banco).
fn correspondence_after(moves: &[&str]) -> chess_game::Game {
    let mut game = online_game("cor1", Some(3), Opponent::Human);
    feed(&mut game, moves, None);
    game
}

// ---- A Partida online de verdade --------------------------------------------------------------

/// Cofre falso: um token de teste para a Conexão registrada (nunca o cofre real).
#[derive(Debug)]
struct FakeVault;

impl chess_net::AuthorizationSource for FakeVault {
    fn authorization(
        &self,
        _profile: chess_net::LocalProfileId,
        provider: ProviderId,
        _connection: ConnectionId,
    ) -> Option<chess_net::Authorization> {
        Some(chess_net::Authorization::bearer(
            provider,
            &chess_net::Secret::new("lip_TOKEN_DE_TESTE_DA_TRAVA"),
        ))
    }
}

struct Env {
    server: FakeServer,
    service: NetService,
    fair: FairPlay,
    play: OnlinePlay,
    session: Session,
    store: Store,
    profile: ProfileId,
    events: LiveFeed,
}

fn env() -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .vault(Arc::new(FakeVault))
        .route("lichess.org", &server.base_url());
    let net = Net::open(config).unwrap();
    net.set_enabled(ProviderId::Lichess, true).unwrap();
    let service = NetService::from_net(net, clock, false);
    service.set_authorized_connection(ProviderId::Lichess, ConnectionRole::Player, Some(PLAYER));
    let events = LiveFeed::new();
    server.route("/api/stream/event", FakeResponse::live(&events));
    let store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    Env {
        fair: service.fair_play().clone(),
        server,
        service,
        play: OnlinePlay::new(),
        session: Session::new(),
        store,
        profile,
        events,
    }
}

impl Env {
    /// Um quadro do app, como `online::drive_online` faz.
    fn frame(&mut self) {
        self.play.follow_connection(&self.service);
        crate::online::route_intents(&mut self.play, &mut self.session);
        let frame = self.play.frame(Some(StoreAccess {
            store: &mut self.store,
            profile: self.profile,
        }));
        crate::online::after_frame(&mut self.play, &mut self.session, frame, None);
        self.fair.follow_games(self.play.locked_games());
    }

    fn until(&mut self, mut cond: impl FnMut(&Self) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < WAIT {
            self.frame();
            if cond(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// Busca em tempo real pareada: a Partida contra "Coruja_Azul" vai ao tabuleiro.
    fn real_time_game(&mut self) -> LiveFeed {
        let seek = LiveFeed::new();
        self.server
            .route("/api/board/seek", FakeResponse::live(&seek));
        let feed = LiveFeed::new();
        self.server.route(
            &format!("/api/board/game/stream/{GAME}"),
            FakeResponse::live(&feed),
        );
        self.frame();
        self.play
            .seek(&self.service, Seek::real_time(600, 5))
            .unwrap();
        assert!(self.events.wait_connected(WAIT));
        assert!(seek.wait_connected(WAIT));
        self.events
            .push(&json!({ "type": "gameStart", "game": info() }).to_string());
        seek.close();
        let start = Instant::now();
        while !feed.is_connected() && start.elapsed() < WAIT {
            self.frame();
            std::thread::sleep(Duration::from_millis(5));
        }
        feed.push(&full("e2e4 c7c5").to_string());
        assert!(
            self.until(|e| e.session.is_online() && e.session.snapshot().history.len() == 2),
            "a Partida vai ao tabuleiro"
        );
        feed
    }
}

fn info() -> Value {
    json!({
        "fullId": format!("{GAME}91cB"), "gameId": GAME,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": "white", "lastMove": "", "source": "lobby",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "rapid", "perf": "rapid", "rated": false, "hasMoved": false,
        "opponent": { "id": "coruja_azul", "username": "Coruja_Azul", "rating": 1662 },
        "isMyTurn": true, "secondsLeft": 600,
        "compat": { "bot": false, "board": true },
        "id": GAME,
    })
}

/// `gameFull` sintético: rápida 10+5, o jogador local de brancas contra "Coruja_Azul".
fn full(moves: &str) -> Value {
    json!({
        "type": "gameFull", "id": GAME,
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "speed": "rapid", "perf": { "name": "Rapid" }, "rated": false,
        "createdAt": 1_794_398_400_000_i64,
        "white": { "id": "voce_demo", "name": "Voce_Demo", "title": null, "rating": 1566 },
        "black": { "id": "coruja_azul", "name": "Coruja_Azul", "title": null, "rating": 1662 },
        "initialFen": "startpos",
        "clock": { "initial": 600_000, "increment": 5_000 },
        "state": {
            "type": "gameState", "moves": moves, "wtime": 600_000, "btime": 600_000,
            "winc": 5000, "binc": 5000, "status": "started",
        },
    })
}

// ---- 1. Engine adversária local --------------------------------------------------------------

/// O passo a passo da issue: Partida online em tempo real de brancas; Nova Partida contra a
/// Engine (de pretas) com a Partida online fora do tabuleiro. A Engine nem nasce, e a busca por
/// Posição da Biblioteca também não responde.
#[test]
fn the_opponent_engine_never_starts_while_a_real_time_game_is_off_the_board() {
    let mut env = env();
    let _feed = env.real_time_game();
    // O jogador troca o tabuleiro: a sessão em tempo real continua aberta (PO-17).
    env.session.new_game();
    env.frame();
    assert!(env.fair.everything_locked());
    assert!(!env.session.is_online());

    let refused = env
        .session
        .start_engine_game(seat(Color::White), engine_config(), &env.fair);
    assert_eq!(
        refused,
        Err(FairPlayLocked {
            computation: Computation::OpponentEngine,
            scope: LockScope::Everything,
        })
    );
    assert!(
        !env.session.engine_active(),
        "nenhuma Engine adversária com a Partida online em andamento"
    );
    assert!(env.session.snapshot().history.is_empty(), "nada mudou");
    assert_eq!(
        env.fair
            .check_anywhere(Computation::OpponentEngine)
            .unwrap_err()
            .scope,
        LockScope::Everything,
        "a Ação Contra a Engine fica desabilitada pelo mesmo motivo"
    );
    // Item 2 da issue em tempo real: a busca por Posição da Biblioteca também não sai.
    let fen = chess_core::CanonicalPosition::start().to_fen();
    let query = by_position(&fen, env.profile);
    assert_eq!(
        search_library(Some(&env.fair), None, &env.store, &query)
            .unwrap_err()
            .scope,
        LockScope::Everything
    );
}

#[test]
fn a_running_opponent_engine_stops_when_the_lock_engages() {
    let fair = FairPlay::new(chess_net::FairPlayGuard::new());
    let mut session = Session::new();
    session
        .start_engine_game(seat(Color::Black), engine_config(), &fair)
        .unwrap();
    assert!(session.engine_active(), "sem trava, a Engine joga");
    // Uma Partida online contra pessoa começa (a sessão dela segura a trava da rede).
    let hold = fair.guard().hold("Partida online contra pessoa");
    session.tick_engine(&fair);
    assert!(
        !session.engine_active(),
        "com a trava ligada a Engine não recebe mais a Posição"
    );
    assert_eq!(session.engine_pending(), Some(seat(Color::Black)));
    assert_eq!(
        session.engine_locked().map(|l| l.scope),
        Some(LockScope::Everything)
    );
    // Religar não passa enquanto a trava estiver ligada; com o fim confirmado, volta.
    assert!(session.resume_engine(engine_config(), &fair).is_err());
    assert!(!session.engine_active());
    drop(hold);
    session.resume_engine(engine_config(), &fair).unwrap();
    assert!(session.engine_active());
    assert_eq!(session.engine_locked(), None);
}

#[test]
fn a_reopened_engine_game_does_not_resume_under_the_lock() {
    let fair = FairPlay::new(chess_net::FairPlayGuard::new());
    let mut realtime = online_game("rt1", None, Opponent::Human);
    feed(&mut realtime, &["e2e4"], None);
    fair.follow_games([&realtime]);
    let setup =
        chess_game::GameSetup::standard(PolicyKind::Assisted).with_engine(seat(Color::Black));
    let mut session = Session::from_game(chess_game::Game::new(setup));
    assert!(session.resume_engine(engine_config(), &fair).is_err());
    assert!(!session.engine_active());
    assert_eq!(session.engine_pending(), Some(seat(Color::Black)));
}

/// A trava que a Partida competitiva local segura não vale para a adversária dela (FP-02).
#[test]
fn a_local_competitive_game_against_the_engine_keeps_its_opponent() {
    let service = NetService::in_memory();
    let fair = service.fair_play().clone();
    let mut session = Session::new();
    session
        .start_vs_engine(
            &NewGameConfig {
                opponent: NewGameOpponent::Engine,
                user_side: Color::White,
                ..NewGameConfig::for_policy(PolicyKind::FideSimulation)
            },
            engine_config(),
            &fair,
        )
        .unwrap();
    let mut engaged = false;
    service.follow_competitive(session.competitive_in_progress(), &mut engaged);
    assert!(
        engaged && fair.everything_locked(),
        "a competitiva trava o resto"
    );
    session.tick_engine(&fair);
    assert!(
        session.engine_active(),
        "a adversária da própria Partida competitiva joga"
    );
    // Uma Partida online contra pessoa por cima: aí a Engine para.
    let _online = fair.guard().hold("Partida online contra pessoa");
    session.tick_engine(&fair);
    assert!(!session.engine_active());
}

#[test]
fn correspondence_refuses_the_opponent_engine_only_at_its_positions() {
    let fair = FairPlay::new(chess_net::FairPlayGuard::new());
    let corr = correspondence_after(&NAJDORF);
    fair.follow_games([&corr]);
    // A Posição da correspondência num tabuleiro de estudo, contra a Engine.
    let fen = position_after(&NAJDORF).to_fen();
    let setup = chess_game::GameSetup::from_fen(PolicyKind::Assisted, &fen)
        .unwrap()
        .with_engine(seat(Color::White));
    let mut session = Session::from_game(chess_game::Game::new(setup));
    assert_eq!(
        session
            .resume_engine(engine_config(), &fair)
            .unwrap_err()
            .scope,
        LockScope::ThisPosition
    );
    assert!(!session.engine_active(), "a Posição da correspondência");
    // Outra abertura: livre.
    let mut other = Session::new();
    other
        .start_vs_engine(
            &NewGameConfig {
                opponent: NewGameOpponent::Engine,
                user_side: Color::White,
                ..NewGameConfig::default()
            },
            engine_config(),
            &fair,
        )
        .unwrap();
    assert!(other.engine_active(), "outra Posição continua livre");
    other.tick_engine(&fair);
    assert!(other.engine_active());
}

// ---- 2. Busca por Posição na Biblioteca -------------------------------------------------------

/// Na Najdorf da correspondência, mais adiante: 5.Nc3 a6 6.Be3 e5 (outra estrutura).
const NAJDORF_LATER: [&str; 12] = [
    "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "a7a6", "c1e3", "e7e5",
];

/// A Biblioteca com a Partida que passa pela Posição da correspondência e uma de outra abertura.
fn library_with_games() -> (Store, ProfileId) {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    store
        .import_pgn(
            "[White \"Ana\"]\n[Black \"Beto\"]\n[Result \"0-1\"]\n\n\
             1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 6. Be3 e5 7. Nb3 Be6 0-1\n\n\
             [White \"Eva\"]\n[Black \"Caio\"]\n[Result \"1/2-1/2\"]\n\n\
             1. d4 d5 2. c4 e6 3. Nc3 Nf6 1/2-1/2\n",
            None,
        )
        .unwrap();
    (store, profile)
}

/// Um tabuleiro de estudo com os Lances `moves`.
fn study_after(moves: &[&str]) -> chess_game::Game {
    let mut game = chess_game::Game::new(chess_game::GameSetup::standard(PolicyKind::Assisted));
    for (i, m) in moves.iter().enumerate() {
        let mv: chess_core::Move = m.parse().unwrap();
        game.handle(
            chess_game::Command::Release {
                from: mv.from(),
                to: mv.to(),
            },
            chess_game::Timestamp::mono(i as u64),
        )
        .unwrap();
    }
    game
}

fn by_position(fen: &str, profile: ProfileId) -> LibraryQuery {
    LibraryQuery {
        position: Some(fen.to_owned()),
        profile: Some(profile),
        ..LibraryQuery::default()
    }
}

#[test]
fn the_library_position_search_refuses_a_correspondence_position() {
    let fair = FairPlay::new(chess_net::FairPlayGuard::new());
    let corr = correspondence_after(&NAJDORF);
    fair.follow_games([&corr]);
    let (store, profile) = library_with_games();
    let total = |fen: &str, shown: Option<&Subject>| {
        search_library(Some(&fair), shown, &store, &by_position(fen, profile))
            .map(|page| page.unwrap().total)
    };
    // A Posição da correspondência: a Partida, o que se jogou depois e o resultado não saem.
    let fen = position_after(&NAJDORF).to_fen();
    assert_eq!(
        total(&fen, None),
        Err(FairPlayLocked {
            computation: Computation::LibrarySearch,
            scope: LockScope::ThisPosition,
        })
    );
    // Outra abertura: a lista sai.
    let other = position_after(&["d2d4", "d7d5", "c2c4", "e7e6"]).to_fen();
    assert_eq!(total(&other, None), Ok(1));
    // Mais adiante, pelo FEN solto: outra estrutura, livre.
    let later = position_after(&NAJDORF_LATER).to_fen();
    assert_eq!(total(&later, None), Ok(1));
    // A mesma Posição no tabuleiro de estudo que passou pela da correspondência ("Usar a
    // posição do tabuleiro"): descende dela, travada (PO-13, item 3).
    let study = study_after(&NAJDORF_LATER);
    let shown = Subject::at(&study, NAJDORF_LATER.len()).unwrap();
    assert_eq!(
        total(&later, Some(&shown)).unwrap_err().scope,
        LockScope::ThisPosition
    );
    // Sem o ponto único da Trava, a busca por Posição não sai; sem filtro de Posição, sai.
    assert!(search_library(None, None, &store, &by_position(&other, profile)).is_err());
    let all = LibraryQuery {
        profile: Some(profile),
        ..LibraryQuery::default()
    };
    let page = search_library(None, None, &store, &all).unwrap().unwrap();
    assert_eq!(page.total, 2);
}

// ---- 3. Engine online -------------------------------------------------------------------------

mod engine_online {
    use super::*;
    use crate::engine_online::service::{
        Activity, EngineFactory, EngineOnline, EngineSpec, RunState, Settings, Unavailable,
    };
    use chess_providers::MoveEngine;
    use chess_uci::{ControllerEvent, SearchRequest};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    const ENGINE_CONNECTION: ConnectionId = ConnectionId(2);
    const BOT_GAME: &str = "En9ineQ1";

    /// Engine roteirizada: guarda as buscas que recebeu (a Posição que chegou a ela).
    struct Scripted {
        searches: Arc<Mutex<Vec<SearchRequest>>>,
        script: VecDeque<String>,
        queue: VecDeque<ControllerEvent>,
        next: u64,
    }

    impl MoveEngine for Scripted {
        fn search(&mut self, request: SearchRequest) -> u64 {
            let id = self.next;
            self.next += 1;
            self.searches.lock().unwrap().push(request);
            if let Some(uci) = self.script.pop_front() {
                self.queue.push_back(ControllerEvent::Move { id, uci });
            }
            id
        }

        fn cancel(&mut self) {
            self.queue.clear();
        }

        fn poll(&mut self) -> Option<ControllerEvent> {
            self.queue.pop_front()
        }
    }

    fn scripted(moves: &[&str]) -> (EngineFactory, Arc<Mutex<Vec<SearchRequest>>>) {
        let searches = Arc::new(Mutex::new(Vec::new()));
        let handle = searches.clone();
        let script: VecDeque<String> = moves.iter().map(|m| (*m).to_owned()).collect();
        (
            Box::new(move |_| {
                Ok(Box::new(Scripted {
                    searches: handle.clone(),
                    script: script.clone(),
                    queue: VecDeque::new(),
                    next: 1,
                }))
            }),
            searches,
        )
    }

    fn spec() -> EngineSpec {
        EngineSpec {
            name: "Stockfish de teste".to_owned(),
            path: std::path::PathBuf::from("/engines/stockfish-de-teste"),
            options: Vec::new(),
        }
    }

    struct Bot {
        server: FakeServer,
        service: NetService,
        fair: FairPlay,
        run: EngineOnline,
        store: Store,
        profile: ProfileId,
        events: LiveFeed,
    }

    fn bot() -> Bot {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let config = NetConfig::in_memory()
            .clock(clock.clone())
            .jitter_seed(7)
            .vault(Arc::new(FakeVault))
            .route("lichess.org", &server.base_url());
        let net = Net::open(config).unwrap();
        net.set_enabled(ProviderId::Lichess, true).unwrap();
        let service = NetService::from_net(net, clock, false);
        service.set_authorized_connection(
            ProviderId::Lichess,
            ConnectionRole::Engine,
            Some(ENGINE_CONNECTION),
        );
        let events = LiveFeed::new();
        server.route("/api/stream/event", FakeResponse::live(&events));
        let store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        Bot {
            fair: service.fair_play().clone(),
            server,
            service,
            run: EngineOnline::default(),
            store,
            profile,
            events,
        }
    }

    impl Bot {
        fn frame(&mut self) {
            self.run.frame(Some(StoreAccess {
                store: &mut self.store,
                profile: self.profile,
            }));
        }

        fn until(&mut self, mut cond: impl FnMut(&Self) -> bool) -> bool {
            let start = Instant::now();
            while start.elapsed() < WAIT {
                self.frame();
                if cond(self) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            false
        }

        fn settle(&mut self) {
            for _ in 0..30 {
                self.frame();
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn posts_to(&self, path: &str) -> usize {
            self.server
                .requests()
                .iter()
                .filter(|r| r.method == "POST" && r.path == path)
                .count()
        }

        fn push_challenge(&self, id: &str, variant: &str) {
            self.events.push(
                &json!({ "type": "challenge", "challenge": {
                    "id": id, "status": "created",
                    "challenger": { "id": "salma", "name": "Salma", "rating": 1662 },
                    "destUser": { "id": "motor", "name": "Motor", "title": "BOT" },
                    "variant": { "key": variant, "name": variant },
                    "rated": false, "speed": "blitz",
                    "timeControl": { "type": "clock", "limit": 300, "increment": 2, "show": "5+2" },
                    "color": "random",
                } })
                .to_string(),
            );
        }

        /// O Lichess abre a Partida da Engine (de brancas) em `fen`, na variante `variant`.
        fn open_game(&self, fen: &str, variant: &str) -> LiveFeed {
            let feed = LiveFeed::new();
            self.server.route(
                &format!("/api/bot/game/stream/{BOT_GAME}"),
                FakeResponse::live(&feed),
            );
            self.events.push(
                &json!({ "type": "gameStart", "game": {
                    "fullId": format!("{BOT_GAME}91cB"), "gameId": BOT_GAME,
                    "fen": fen, "color": "white", "lastMove": "", "source": "friend",
                    "status": { "id": 20, "name": "started" },
                    "variant": { "key": variant, "name": variant },
                    "speed": "blitz", "perf": "blitz", "rated": false, "hasMoved": false,
                    "opponent": { "id": "salma", "username": "Salma", "rating": 1662 },
                    "isMyTurn": true, "secondsLeft": 300,
                    "compat": { "bot": true, "board": false },
                    "id": BOT_GAME,
                } })
                .to_string(),
            );
            feed
        }

        fn activity_has(&self, wanted: impl Fn(&Activity) -> bool) -> bool {
            self.run.activity().any(wanted)
        }
    }

    /// `gameFull` sintético: `initial` é `startpos` ou um FEN.
    fn bot_full(initial: &str, variant: &str, moves: &str) -> String {
        json!({
            "type": "gameFull", "id": BOT_GAME,
            "variant": { "key": variant, "name": variant, "short": variant },
            "speed": "blitz", "rated": false,
            "white": { "id": "motor", "name": "Motor", "title": "BOT", "rating": 2500 },
            "black": { "id": "salma", "name": "Salma", "rating": 1662 },
            "initialFen": initial,
            "clock": { "initial": 300_000, "increment": 2_000 },
            "state": bot_state(moves),
        })
        .to_string()
    }

    fn bot_state(moves: &str) -> Value {
        json!({
            "type": "gameState", "moves": moves, "wtime": 300_000, "btime": 300_000,
            "winc": 2000, "binc": 2000, "status": "started",
        })
    }

    fn resign_path() -> String {
        format!("/api/bot/game/{BOT_GAME}/resign")
    }

    fn move_path(uci: &str) -> String {
        format!("/api/bot/game/{BOT_GAME}/move/{uci}")
    }

    #[test]
    fn it_does_not_turn_on_while_a_real_time_human_game_is_in_progress() {
        let mut bot = bot();
        let mut realtime = online_game("rt1", None, Opponent::Human);
        feed(&mut realtime, &["e2e4"], None);
        bot.fair.follow_games([&realtime]);
        let (factory, _) = scripted(&[]);
        let started = bot
            .run
            .start(&bot.service, spec(), Settings::default(), factory);
        assert_eq!(
            started,
            Err(Unavailable::FairPlay(FairPlayLocked {
                computation: Computation::OnlineEngine,
                scope: LockScope::Everything,
            })),
            "a Engine online é cálculo: não liga"
        );
        bot.frame();
        assert_eq!(bot.run.state(), RunState::Stopped);
        assert!(bot.server.requests().is_empty(), "nada sai");
    }

    #[test]
    fn a_challenge_that_arrives_under_the_lock_is_declined_and_never_accepted() {
        let mut bot = bot();
        bot.server.route("/api/challenge/chal0001/decline", ok());
        bot.server.route("/api/challenge/chal0001/accept", ok());
        let (factory, _) = scripted(&[]);
        bot.run
            .start(&bot.service, spec(), Settings::default(), factory)
            .unwrap();
        assert!(bot.events.wait_connected(WAIT));
        // Uma Partida online contra pessoa começa (a sessão dela segura a trava da rede).
        let _hold = bot.fair.guard().hold("Partida online contra pessoa");
        bot.push_challenge("chal0001", "standard");
        assert!(bot.until(|b| b.posts_to("/api/challenge/chal0001/decline") == 1));
        bot.settle();
        assert_eq!(bot.posts_to("/api/challenge/chal0001/accept"), 0);
        assert!(
            bot.activity_has(
                |a| matches!(a, Activity::FairPlayDeclined { name } if name == "Salma")
            )
        );
    }

    /// Com só a correspondência travando (por Posição), o desafio comum segue e o "a partir de
    /// uma Posição" fica de fora (o Lichess só diz qual Posição quando a Partida começa).
    #[test]
    fn under_a_correspondence_lock_only_challenges_from_a_free_start_are_accepted() {
        let mut bot = bot();
        bot.server.route("/api/challenge/chal0001/accept", ok());
        bot.server.route("/api/challenge/chal0002/decline", ok());
        let (factory, _) = scripted(&[]);
        let settings = Settings {
            policy: chess_providers::ChallengePolicy {
                max_concurrent: 4,
                ..chess_providers::ChallengePolicy::default()
            },
            ..Settings::default()
        };
        let corr = correspondence_after(&NAJDORF);
        bot.fair.follow_games([&corr]);
        bot.run
            .start(&bot.service, spec(), settings, factory)
            .unwrap();
        assert!(bot.events.wait_connected(WAIT));
        bot.push_challenge("chal0001", "standard");
        bot.push_challenge("chal0002", "fromPosition");
        assert!(bot.until(|b| {
            b.posts_to("/api/challenge/chal0001/accept") == 1
                && b.posts_to("/api/challenge/chal0002/decline") == 1
        }));
        assert_eq!(bot.posts_to("/api/challenge/chal0002/accept"), 0);
    }

    /// O cenário da issue: com a correspondência em andamento, um desafio feito pelo site a
    /// partir da Posição dela. A Engine nunca recebe a Posição, nenhum Lance sai e a Partida é
    /// abandonada.
    #[test]
    fn a_game_from_the_correspondence_position_never_reaches_the_engine() {
        let mut bot = bot();
        bot.server.route(&resign_path(), ok());
        let (factory, searches) = scripted(&["b1c3"]);
        bot.run
            .start(&bot.service, spec(), Settings::default(), factory)
            .unwrap();
        assert!(bot.events.wait_connected(WAIT));
        let corr = correspondence_after(&NAJDORF);
        bot.fair.follow_games([&corr]);
        let fen = position_after(&NAJDORF).to_fen();
        let game = bot.open_game(&fen, "fromPosition");
        assert!(bot.until(|_| game.wait_connected(Duration::from_millis(1))));
        game.push(&bot_full(&fen, "fromPosition", ""));
        assert!(bot.until(|b| b.posts_to(&resign_path()) == 1), "abandonada");
        bot.settle();
        assert!(
            searches.lock().unwrap().is_empty(),
            "a Engine não recebeu a Posição da correspondência"
        );
        assert_eq!(bot.posts_to(&move_path("b1c3")), 0, "nenhum Lance saiu");
        assert_eq!(bot.posts_to(&resign_path()), 1, "um só abandono");
        assert!(bot.activity_has(|a| matches!(a, Activity::FairPlayResigned { .. })));
    }

    /// O app fechou no meio de uma Partida da Engine online; ao religar, o Lichess reenvia o
    /// `gameStart` e a Partida é retomada do banco. Se ela está na Posição da correspondência em
    /// andamento, a Partida retomada também obedece à Trava: a Engine não calcula.
    #[test]
    fn a_resumed_online_engine_game_under_the_lock_does_not_calculate() {
        let mut bot = bot();
        // A Partida da Engine no banco, depois de 1.e4 c5 2.Nf3 d6 3.d4 cxd4 4.Nxd4 Nf6.
        let setup = chess_game::GameSetup::online(
            chess_game::OnlineGame {
                provider: crate::engine_online::service::BOT_KEY.into(),
                remote_id: BOT_GAME.into(),
                local_color: Color::White,
                opponent: Opponent::Human,
                white: chess_game::OnlinePlayer {
                    name: "Motor".into(),
                    rating: Some(2500),
                },
                black: chess_game::OnlinePlayer {
                    name: "Salma".into(),
                    rating: Some(1662),
                },
                rated: false,
                days_per_move: None,
            },
            chess_game::Variant::Standard,
            None,
        )
        .unwrap();
        let mut stored = chess_game::Game::new(setup.clone());
        feed(&mut stored, &NAJDORF, None);
        let id = bot
            .store
            .create_game_for(
                bot.profile,
                &setup,
                &chess_store::GameMetadata {
                    title: None,
                    extra: json!({ "online": {
                        "provider": crate::engine_online::service::BOT_KEY,
                        "remote_id": BOT_GAME,
                    } }),
                },
            )
            .unwrap();
        bot.store.append_events(id, stored.events()).unwrap();
        // A correspondência contra pessoa está nessa mesma Posição.
        let corr = correspondence_after(&NAJDORF);
        bot.fair.follow_games([&corr]);
        bot.server.route(&resign_path(), ok());
        bot.server.route(&move_path("b1c3"), ok());
        let (factory, searches) = scripted(&["b1c3"]);
        bot.run
            .start(&bot.service, spec(), Settings::default(), factory)
            .unwrap();
        assert!(bot.events.wait_connected(WAIT));
        let fen = position_after(&NAJDORF).to_fen();
        let game = bot.open_game(&fen, "standard");
        assert!(bot.until(|_| game.wait_connected(Duration::from_millis(1))));
        game.push(&bot_full("startpos", "standard", &NAJDORF.join(" ")));
        assert!(bot.until(|b| b.posts_to(&resign_path()) == 1), "abandonada");
        bot.settle();
        assert!(
            bot.activity_has(|a| matches!(a, Activity::GameResumed { .. })),
            "a Partida foi retomada do banco: {:?}",
            bot.run
        );
        assert!(bot.activity_has(|a| matches!(a, Activity::FairPlayResigned { .. })));
        assert!(
            searches.lock().unwrap().is_empty(),
            "a Engine não recebeu a Posição da Partida retomada"
        );
        assert_eq!(bot.posts_to(&move_path("b1c3")), 0, "nenhum Lance saiu");
    }

    /// A Partida aceita antes da Trava: quando ela liga, a próxima busca não chega à Engine (o
    /// jogador não pode repetir nela os Lances da Partida contra pessoa).
    #[test]
    fn a_game_in_progress_stops_calculating_when_the_lock_engages() {
        let mut bot = bot();
        bot.server.route(&resign_path(), ok());
        bot.server.route(&move_path("e2e4"), ok());
        bot.server.route(&move_path("d2d4"), ok());
        let (factory, searches) = scripted(&["e2e4", "d2d4"]);
        bot.run
            .start(&bot.service, spec(), Settings::default(), factory)
            .unwrap();
        assert!(bot.events.wait_connected(WAIT));
        let start = chess_core::CanonicalPosition::start().to_fen();
        let game = bot.open_game(&start, "standard");
        assert!(bot.until(|_| game.wait_connected(Duration::from_millis(1))));
        game.push(&bot_full("startpos", "standard", ""));
        assert!(bot.until(|b| b.posts_to(&move_path("e2e4")) == 1));
        assert_eq!(searches.lock().unwrap().len(), 1);
        // Uma Partida online contra pessoa começa; o adversário responde.
        let _hold = bot.fair.guard().hold("Partida online contra pessoa");
        game.push(&bot_state("e2e4").to_string());
        game.push(&bot_state("e2e4 e7e5").to_string());
        assert!(bot.until(|b| b.posts_to(&resign_path()) == 1), "abandonada");
        bot.settle();
        assert_eq!(
            searches.lock().unwrap().len(),
            1,
            "a Posição depois de 1...e5 não chegou à Engine"
        );
        assert_eq!(bot.posts_to(&move_path("d2d4")), 0);
    }
}

fn ok() -> FakeResponse {
    FakeResponse::json(r#"{"ok":true}"#)
}
