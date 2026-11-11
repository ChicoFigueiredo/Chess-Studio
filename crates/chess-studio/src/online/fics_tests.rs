//! O FICS como visitante pela camada do app (#254): a conexão de visitante, o tabuleiro
//! (`Session`), o Painel, o banco e a Trava de fair play juntos, como a interface faz a cada
//! quadro. Servidor ICS falso em loopback e relógio falso: nenhuma conexão ao FICS real,
//! nenhum login real (CX-17). As transcrições são **sintéticas** (o `style 12` segue `help
//! style12`; as demais linhas seguem o texto que clientes de código aberto do FICS reconhecem),
//! como nos testes do `chess-providers`.

#![allow(clippy::unwrap_used)]

use super::fics::FicsInfo;
use super::fics_view::{self, FicsForm};
use super::service::{GameKey, OnlineNotice, OnlinePlay, Provider, StoreAccess, Unavailable};
use super::view::PanelView;
use super::{after_frame, route_intents};
use crate::analysis::{Analysis, Status};
use crate::fair_play::{Computation, FairPlay, LockScope, Subject};
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::session::{OnlineIntent, Session};
use chess_core::{CanonicalPosition, Color, Move, Square};
use chess_game::{EndReason, GameResult, PolicyKind};
use chess_input::{GestureEvent, Modifiers, PointerButton};
use chess_net::testing::FakeIcsServer;
use chess_net::{FakeClock, Net, NetConfig, ProviderId, Timestamp};
use chess_providers::{FicsError, FicsStatus, LinkStatus};
use chess_store::{GameQuery, LibraryQuery, ProfileId, Store};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const WAIT: Duration = Duration::from_secs(10);
const ME: &str = "GuestQWER";
const OPPONENT: &str = "GuestEFGH";
const GAME: u32 = 42;

struct Env {
    server: FakeIcsServer,
    service: NetService,
    fair: FairPlay,
    play: OnlinePlay,
    session: Session,
    store: Store,
    profile: ProfileId,
    notices: Vec<OnlineNotice>,
}

fn env_with(enabled: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeIcsServer::start();
    let net = Net::open(
        NetConfig::in_memory()
            .clock(clock.clone())
            .route("freechess.org", &server.route_url()),
    )
    .unwrap();
    if enabled {
        net.set_enabled(ProviderId::Fics, true).unwrap();
    }
    let service = NetService::from_net(net, clock, false);
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
        notices: Vec::new(),
    }
}

fn env() -> Env {
    env_with(true)
}

impl Env {
    /// Um quadro do app, como `drive_online` faz.
    fn frame(&mut self) {
        self.play.follow_connection(&self.service);
        route_intents(&mut self.play, &mut self.session);
        let frame = self.play.frame(Some(StoreAccess {
            store: &mut self.store,
            profile: self.profile,
        }));
        after_frame(&mut self.play, &mut self.session, frame, None);
        self.fair.follow_games(self.play.locked_games());
        self.notices.extend(self.play.take_notices());
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

    /// Quadros por um tempo curto, para provar que algo **não** acontece.
    fn frames_for(&mut self, time: Duration) {
        let start = Instant::now();
        while start.elapsed() < time {
            self.frame();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn saw(&self, info: &FicsInfo) -> bool {
        self.notices
            .iter()
            .any(|n| matches!(n, OnlineNotice::Fics(i) if i == info))
    }

    fn panel(&self) -> PanelView {
        PanelView::of(&self.play, &self.session, T0)
    }

    fn plies(&self) -> usize {
        self.session.snapshot().history.len()
    }

    fn sent(&self, line: &str) -> usize {
        self.server
            .received_lines()
            .iter()
            .filter(|l| *l == line)
            .count()
    }

    /// Clique na origem e no destino (dois toques), como no tabuleiro 3D ou 2D.
    fn tap(&mut self, from: &str, to: &str) {
        for at in [from, to] {
            let square: Square = at.parse().unwrap();
            self.session.gesture(GestureEvent::PointerDown {
                button: PointerButton::Primary,
                square: Some(square),
                modifiers: Modifiers::default(),
            });
            self.session.gesture(GestureEvent::PointerUp {
                square: Some(square),
            });
        }
    }

    fn stored(&self) -> Vec<chess_store::GameSummary> {
        self.store.list_games(&GameQuery::default()).unwrap()
    }

    /// "Entrar como visitante" e a entrada sintética: `login:`, nome, Enter, sessão.
    fn login(&mut self) {
        self.play.fics.connect(&self.service).unwrap();
        assert!(self.server.wait_connected(WAIT));
        self.server
            .send("\n    Free Internet Chess Server (sintético)\n\nlogin: ");
        assert!(self.until(|e| e.sent("guest") == 1));
        self.server.send(&format!(
            "\nLogging you in as \"{ME}\"; you may use this name to play unrated games.\n\n\
             Press return to enter the server as \"{ME}\": "
        ));
        assert!(self.until(|e| e.server.received_lines().len() >= 2));
        self.server.send(&format!(
            "\n**** Starting FICS session as {ME}(U) ****\n\nfics% "
        ));
        assert!(self.until(|e| e.play.fics.status() == Some(FicsStatus::Ready)));
        assert!(self.saw(&FicsInfo::LoggedIn {
            handle: ME.to_owned()
        }));
    }

    /// Busca 5+0 pela tela e a Partida `GAME` começa, com o jogador local de brancas.
    fn start_game(&mut self) {
        self.play.fics.seek(5, 0, T0).unwrap();
        assert!(self.server.wait_line("seek 5 0 unrated", WAIT));
        self.server.send(&format!(
            "Your seek has been posted with index 7.\nfics% \n\
             Creating: {ME} (++++) {OPPONENT} (++++) unrated blitz 5 0\n\
             {{Game {GAME} ({ME} vs. {OPPONENT}) Creating unrated blitz match.}}\n\n{}\nfics% ",
            board(&CanonicalPosition::start(), 300, 300)
        ));
        assert!(
            self.until(|e| e.session.is_online()),
            "a Partida vai ao tabuleiro"
        );
        assert!(self.play.fics.seek_state().is_none(), "a busca acabou");
    }

    /// O FICS manda o tabuleiro `position`.
    fn board(&self, position: &CanonicalPosition, white_s: i64, black_s: i64) {
        self.server
            .send(&format!("\n{}\nfics% ", board(position, white_s, black_s)));
    }
}

/// Linha `style 12` sintética da Posição, do ponto de vista do jogador local (brancas).
fn board(position: &CanonicalPosition, white_s: i64, black_s: i64) -> String {
    let relation = if position.side_to_move() == Color::White {
        1
    } else {
        -1
    };
    style12(position, GAME, relation, white_s, black_s)
}

fn style12(
    position: &CanonicalPosition,
    game: u32,
    relation: i8,
    white_s: i64,
    black_s: i64,
) -> String {
    let fen = position.to_fen();
    let fields: Vec<&str> = fen.split(' ').collect();
    let ranks: Vec<String> = fields[0]
        .split('/')
        .map(|rank| {
            rank.chars()
                .map(|c| match c.to_digit(10) {
                    Some(n) => "-".repeat(n as usize),
                    None => c.to_string(),
                })
                .collect()
        })
        .collect();
    let side = if fields[1] == "w" { "W" } else { "B" };
    let double_push = match fields[3] {
        "-" => "-1".to_owned(),
        ep => (ep.as_bytes()[0] - b'a').to_string(),
    };
    let flag = |c: char| if fields[2].contains(c) { "1" } else { "0" };
    format!(
        "<12> {} {side} {double_push} {} {} {} {} {} {game} {ME} {OPPONENT} {relation} 5 0 39 39 {white_s} {black_s} {} none (0:00) none 0 1 0",
        ranks.join(" "),
        flag('K'),
        flag('Q'),
        flag('k'),
        flag('q'),
        fields[4],
        fields[5],
    )
}

fn mv(text: &str) -> Move {
    text.parse().unwrap()
}

/// As Posições depois de cada Lance, a partir da inicial.
fn positions(moves: &[&str]) -> Vec<CanonicalPosition> {
    let mut out = vec![CanonicalPosition::start()];
    for m in moves {
        let next = out.last().unwrap().play(mv(m)).unwrap();
        out.push(next);
    }
    out
}

fn analysis_with_fake_engine() -> Analysis {
    let mut analysis = Analysis::default();
    analysis.set_default_engine(Some(std::path::PathBuf::from("/nao/existe/engine")));
    analysis.set_enabled(true);
    analysis
}

// ---- Partida de visitante completa pelo app --------------------------------------------------

#[test]
fn a_guest_game_goes_from_the_seek_through_the_board_and_the_panel_into_the_library() {
    let mut env = env();
    env.login();
    env.start_game();
    assert_eq!(env.session.snapshot().policy, PolicyKind::Online);
    assert_eq!(env.session.user_side(), Color::White);
    let key = GameKey::new(Provider::Fics, GAME.to_string());
    assert_eq!(super::board_key(&env.session), Some(key));
    // O mesmo Painel da Partida online, com o selo do FICS e a conexão ao vivo.
    let panel = env.panel();
    assert_eq!(panel.link, Some(LinkStatus::Live));
    assert!(!panel.desynced && !panel.dropped);
    // A Trava de fair play: a sessão a segura do começo ao fim confirmado.
    assert!(env.play.fics.holds_fair_play());
    assert!(env.service.net().fair_play_guard().is_engaged());
    assert!(env.play.realtime_in_progress());
    assert!(env.session.competitive_in_progress());

    let game = ["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"];
    let p = positions(&game);
    for (index, pair) in game.chunks(2).enumerate() {
        let mine = pair[0];
        let plies = env.plies();
        // O Lance pelos gestos de sempre: intenção enviada, um único comando.
        env.tap(&mine[..2], &mine[2..4]);
        assert!(env.until(|e| e.sent(mine) == 1), "{mine} sai uma vez");
        assert!(
            env.session.snapshot().made_move.is_some(),
            "a intenção enviada aparece no tabuleiro, fora do log"
        );
        assert_eq!(
            env.plies(),
            plies,
            "nada no histórico antes do FICS confirmar"
        );
        if pair.len() == 1 {
            break;
        }
        env.board(&p[2 * index + 1], 290, 300);
        env.board(&p[2 * index + 2], 290, 280);
        assert!(env.until(|e| e.plies() == 2 * index + 2));
        assert!(env.session.snapshot().made_move.is_none());
    }
    // 4. Qxf7#: o FICS confirma o Lance e o mate.
    assert!(env.service.net().fair_play_guard().is_engaged());
    env.server.send(&format!(
        "\n{}\n{{Game {GAME} ({ME} vs. {OPPONENT}) {OPPONENT} checkmated}} 1-0\nfics% ",
        board(&p[7], 279, 270)
    ));
    assert!(env.until(|e| e.session.snapshot().is_over()));
    let outcome = env.session.snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::WhiteWins, EndReason::Checkmate)
    );
    assert!(env.until(|e| !e.service.net().fair_play_guard().is_engaged()));
    assert!(!env.fair.any(), "a trava caiu com o fim confirmado");
    assert!(!env.play.realtime_in_progress());
    // Envio único: cada Lance saiu uma vez.
    for m in ["e2e4", "f1c4", "d1h5", "h5f7"] {
        assert_eq!(env.sent(m), 1, "{m}");
    }
    // O banco tem exatamente o log do controlador, com a Origem do FICS.
    let id = env.session.online_store_id().unwrap();
    let stored = env.store.load_game(id).unwrap();
    assert_eq!(stored.events().len(), env.session.game().events().len());
    assert_eq!(stored.snapshot(), env.session.game().snapshot());
    let summary = env.stored().into_iter().find(|g| g.id == id).unwrap();
    assert!(summary.finished);
    assert_eq!(summary.metadata["online"]["provider"], "fics");
    assert_eq!(summary.metadata["online"]["remote_id"], "42");
    assert_eq!(summary.metadata["online"]["guest"], true);
    assert_eq!(
        summary.title.as_deref(),
        Some("Partida online · FICS (visitante)")
    );
    let page = env
        .store
        .search_library(&LibraryQuery {
            profile: Some(env.profile),
            ..LibraryQuery::default()
        })
        .unwrap();
    let entry = page.entries.iter().find(|e| e.ref_id == id.0).unwrap();
    assert_eq!(entry.white.as_deref(), Some(ME));
    assert_eq!(entry.black.as_deref(), Some(OPPONENT));
    assert_eq!(entry.result, "1-0");
    assert!(
        entry
            .event
            .as_deref()
            .unwrap()
            .starts_with("Partida online · FICS (visitante)"),
        "{:?}",
        entry.event
    );
    // Agora a análise sobe.
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert!(analysis.has_engine());
}

#[test]
fn draw_resign_and_abort_reach_the_fics_once_and_nothing_changes_until_it_confirms() {
    let mut env = env();
    env.login();
    env.start_game();
    for (intent, line) in [
        (OnlineIntent::OfferDraw, "draw"),
        (OnlineIntent::Abort, "abort"),
        (OnlineIntent::Resign, "resign"),
    ] {
        env.session.online_request(intent);
        assert!(env.until(|e| e.sent(line) == 1), "{line}");
    }
    env.frames_for(Duration::from_millis(100));
    assert!(!env.session.snapshot().is_over(), "só com a confirmação");
    assert!(env.fair.everything_locked());
    // O adversário propõe empate: a proposta aparece; recusar sai uma vez.
    env.server
        .send(&format!("\n{OPPONENT} offers you a draw.\nfics% "));
    assert!(env.until(|e| e.session.snapshot().pending_draw_offer == Some(Color::Black)));
    env.session.online_request(OnlineIntent::DeclineDraw);
    assert!(env.until(|e| e.sent("decline") == 1));
    // Reclamar vitória não existe no FICS: recusado sem rede.
    env.session.online_request(OnlineIntent::ClaimVictory);
    env.frames_for(Duration::from_millis(50));
    assert!(env.notices.iter().any(|n| matches!(
        n,
        OnlineNotice::Fics(FicsInfo::Refused(FicsError::NotAllowed(_)))
    )));
    // O FICS confirma o abandono.
    env.server.send(&format!(
        "\n{{Game {GAME} ({ME} vs. {OPPONENT}) {ME} resigns}} 0-1\nfics% "
    ));
    assert!(env.until(|e| e.session.snapshot().is_over()));
    let outcome = env.session.snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::BlackWins, EndReason::Resignation)
    );
    assert!(env.until(|e| !e.fair.any()));
    for line in ["draw", "abort", "resign", "decline"] {
        assert_eq!(env.sent(line), 1, "{line} nunca reenviado");
    }
}

// ---- Trava de fair play e queda de conexão ---------------------------------------------------

#[test]
fn the_lock_refuses_analysis_and_a_drop_keeps_it_until_the_session_is_discarded() {
    let mut env = env();
    env.login();
    env.start_game();
    // Durante a Partida: nada calcula.
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert!(matches!(analysis.status(), Status::Locked(_)));
    assert!(!analysis.has_engine(), "nenhum processo de Engine sobe");
    let subject = Subject::shown(&env.session).unwrap();
    let refused = env
        .fair
        .check(Computation::EngineAnalysis, &subject)
        .unwrap_err();
    assert_eq!(refused.scope, LockScope::Everything);

    // O Lance sai; a conexão cai antes da confirmação.
    env.tap("e2", "e4");
    assert!(env.until(|e| e.sent("e2e4") == 1));
    env.server.disconnect();
    assert!(
        env.until(|e| e.saw(&FicsInfo::Closed(FicsError::ConnectionClosed {
            game_in_progress: true
        })))
    );
    assert!(env.saw(&FicsInfo::MoveLost { mv: mv("e2e4") }));
    // O aviso: o FICS dá a Partida de visitante como perdida; o Painel mostra a queda.
    let panel = env.panel();
    assert!(panel.dropped);
    assert_eq!(panel.link, Some(LinkStatus::Closed));
    let i18n = I18n::pt_br();
    let (_, text) = fics_view::info_text(
        &i18n,
        &FicsInfo::Closed(FicsError::ConnectionClosed {
            game_in_progress: true,
        }),
    );
    assert!(
        text.contains("perdida") && text.contains("não reconecta"),
        "{text}"
    );
    // Nada é reenviado nem reconectado: o tabuleiro não aceita Lance, nenhuma conexão nova.
    assert!(env.session.snapshot().made_move.is_none(), "a peça voltou");
    assert!(!env.session.online_can_move());
    env.tap("e2", "e4");
    env.frames_for(Duration::from_millis(200));
    assert_eq!(env.sent("e2e4"), 1);
    assert_eq!(env.server.connections(), 1, "sem reconexão automática");
    // O fim não foi confirmado: a trava fica, e entrar de novo pede o descarte antes.
    assert!(env.service.net().fair_play_guard().is_engaged());
    assert!(env.fair.everything_locked());
    assert!(matches!(
        env.play.fics.connect(&env.service),
        Err(Unavailable::Fics(FicsError::NotAllowed(_)))
    ));
    analysis.drive(&env.session, false, &env.fair);
    assert!(!analysis.has_engine());

    // Descarte explícito: a sessão vai embora e a trava cai.
    env.play.fics.leave();
    assert!(env.until(|e| !e.fair.any()));
    assert!(!env.service.net().fair_play_guard().is_engaged());
    let panel = env.panel();
    assert_eq!(panel.link, None);
    assert!(!panel.dropped);
    // Nada foi inventado: a Partida do banco segue sem resultado, só com o confirmado.
    let id = env.session.online_store_id().unwrap();
    let summary = env.stored().into_iter().find(|g| g.id == id).unwrap();
    assert!(!summary.finished, "o FICS não confirmou o fim");
    assert_eq!(
        env.store.load_game(id).unwrap().events().len(),
        env.session.game().events().len()
    );
    assert_eq!(env.server.connections(), 1);
}

#[test]
fn with_the_fics_off_no_connection_is_ever_opened() {
    let mut env = env_with(false);
    assert_eq!(
        env.play.fics.connect(&env.service),
        Err(Unavailable::ProviderOff)
    );
    env.frames_for(Duration::from_millis(150));
    assert_eq!(env.server.connections(), 0);
    assert_eq!(env.play.fics.status(), None, "nenhuma sessão");
    let i18n = I18n::pt_br();
    assert!(fics_view::unavailable_text(&i18n, &Unavailable::ProviderOff).contains("desligado"));

    // Ligado, entra; desligar no meio fecha a conexão (consentimento por Provedor).
    env.service.set_enabled(ProviderId::Fics, true).unwrap();
    env.login();
    env.service.set_enabled(ProviderId::Fics, false).unwrap();
    assert!(env.until(|e| e.play.fics.status() == Some(FicsStatus::Closed)));
    assert!(env.server.wait_disconnected(WAIT));
    assert_eq!(
        env.play.fics.connect(&env.service),
        Err(Unavailable::ProviderOff)
    );
    env.frames_for(Duration::from_millis(100));
    assert_eq!(env.server.connections(), 1);
    // Só "guest" e Enter na entrada; nenhuma senha, nenhuma linha com "password".
    let lines = env.server.received_lines();
    assert_eq!(lines.first().map(String::as_str), Some("guest"));
    assert!(!lines.iter().any(|l| l.to_lowercase().contains("pass")));
}

// ---- Variantes, dessincronização e observação ------------------------------------------------

#[test]
fn variants_are_refused_with_a_user_message_and_never_become_a_game() {
    let mut env = env();
    env.login();
    env.server.send(&format!(
        "\nChallenge: {OPPONENT} (----) {ME} (----) unrated wild/fr 5 0.\nfics% "
    ));
    assert!(env.until(|e| !e.play.fics.challenges().is_empty()));
    assert!(env.saw(&FicsInfo::Challenge {
        name: OPPONENT.to_owned()
    }));
    let refused = env.play.fics.accept(OPPONENT).unwrap_err();
    let i18n = I18n::pt_br();
    let text = fics_view::unavailable_text(&i18n, &refused);
    assert!(
        text.contains("xadrez padrão") && text.contains("wild/fr"),
        "{text}"
    );
    assert_eq!(env.sent(&format!("accept {OPPONENT}")), 0, "nada saiu");

    // Uma Partida de variante que começa mesmo assim não vira Partida local.
    env.server.send(&format!(
        "\n{{Game {GAME} ({ME} vs. {OPPONENT}) Creating unrated crazyhouse match.}}\n{}\nfics% ",
        board(&CanonicalPosition::start(), 300, 300)
    ));
    let variant = FicsInfo::VariantRefused {
        variant: "crazyhouse".to_owned(),
    };
    assert!(env.until(|e| e.saw(&variant)));
    let (_, text) = fics_view::info_text(&i18n, &variant);
    assert!(text.contains("crazyhouse") && text.contains("xadrez padrão"));
    assert!(!env.session.is_online());
    assert!(env.stored().is_empty(), "nada no banco");
    assert!(!env.fair.any());
    // Recusar o desafio sai uma vez.
    env.play.fics.decline(OPPONENT).unwrap();
    assert!(env.server.wait_line(&format!("decline {OPPONENT}"), WAIT));
}

#[test]
fn a_desync_is_shown_without_inventing_a_move_until_a_coherent_board() {
    let mut env = env();
    env.login();
    env.start_game();
    let id = env.session.online_store_id().unwrap();
    let events = env.store.load_game(id).unwrap().events().len();
    // Um salto de três meios-lances: não há caminho único; nada é reescrito.
    let p = positions(&["e2e4", "e7e5", "g1f3", "b8c6"]);
    env.board(&p[3], 290, 290);
    assert!(env.until(|e| e.saw(&FicsInfo::Desynced)));
    let panel = env.panel();
    assert!(panel.desynced, "o Painel mostra a dessincronização");
    assert_eq!(env.plies(), 0, "nenhum Lance inventado");
    assert_eq!(env.store.load_game(id).unwrap().events().len(), events);
    assert!(!env.session.online_can_move(), "sem Lance local");
    env.tap("e2", "e4");
    env.frames_for(Duration::from_millis(100));
    assert_eq!(env.sent("e2e4"), 0, "nada sai durante a dessincronização");
    // Um tabuleiro coerente (um Lance a partir do conhecido) desfaz.
    env.board(&p[1], 300, 300);
    assert!(env.until(|e| e.saw(&FicsInfo::Resynced) && e.plies() == 1));
    assert!(!env.panel().desynced);
}

#[test]
fn observing_stays_in_memory_never_reaches_the_library_and_never_locks() {
    let mut env = env();
    env.login();
    let p = positions(&["d2d4", "d7d5", "c2c4", "e7e6", "b1c3"]);
    env.play.fics.observe(77).unwrap();
    assert!(env.server.wait_line("observe 77", WAIT));
    env.server.send(&format!(
        "You are now observing game 77.\nGame 77: {ME} (++++) {OPPONENT} (++++) unrated blitz 3 0\n\n{}\nfics% ",
        style12(&p[3], 77, 0, 170, 175)
    ));
    assert!(env.until(|e| e.play.fics.observing()));
    assert!(env.saw(&FicsInfo::ObservationStarted { game: 77 }));
    env.server
        .send(&format!("\n{}\n", style12(&p[4], 77, 0, 170, 170)));
    env.server
        .send(&format!("\n{}\n", style12(&p[5], 77, 0, 165, 170)));
    assert!(env.until(|e| {
        e.play
            .fics
            .observed()
            .is_some_and(|o| o.snapshot.history.len() == 2)
    }));
    let observed = env.play.fics.observed().unwrap();
    assert_eq!(observed.game, 77);
    assert_eq!(observed.snapshot.position, p[5]);
    // Só na memória: nada no banco, nenhuma trava, o tabuleiro não muda.
    assert!(env.stored().is_empty());
    assert!(!env.fair.any());
    assert!(!env.service.net().fair_play_guard().is_engaged());
    assert!(!env.session.is_online());
    assert!(!env.play.realtime_in_progress());
    env.play.fics.unobserve().unwrap();
    assert!(env.server.wait_line("unobserve", WAIT));
    env.server
        .send("\nRemoving game 77 from observation list.\nfics% ");
    assert!(env.until(|e| !e.play.fics.observing()));
    assert!(env.stored().is_empty());
}

// ---- Textos ----------------------------------------------------------------------------------

#[test]
fn every_fics_text_exists_and_the_warning_is_honest() {
    let i18n = I18n::pt_br();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = [
        dir.join("online/fics.rs"),
        dir.join("online/fics_view.rs"),
        dir.join("online/view.rs"),
        dir.join("online/demo.rs"),
        dir.join("connections/view_fics.rs"),
        dir.join("connections/model.rs"),
    ];
    let mut checked = 0;
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for literal in text.split('"').skip(1).step_by(2) {
            let is_id = literal.starts_with("fics-")
                && !literal.ends_with('-')
                && literal
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            // Nomes de cena das capturas (`fics-lobby`…) não são textos.
            let scene = path.ends_with("demo.rs");
            if is_id && !scene {
                checked += 1;
                assert!(i18n.has(literal), "{}: falta `{literal}`", path.display());
            }
        }
    }
    assert!(checked > 30, "o teste não achou os textos ({checked})");
    for (_, key) in fics_view::WARNINGS {
        assert!(i18n.has(key), "{key}");
    }
    let warning = fics_view::WARNINGS
        .iter()
        .map(|(_, key)| i18n.t(key))
        .collect::<Vec<_>>()
        .join(" ");
    for needed in [
        "sem criptografia",
        "texto puro",
        "visitante",
        "nunca pede nem envia senha",
    ] {
        assert!(warning.contains(needed), "{needed}: {warning}");
    }
    // Nenhum texto do FICS pede senha.
    let form = FicsForm::default();
    assert_eq!(form.observe_number(), None);
    for key in [
        "fics-guest-text",
        "fics-connect",
        "consent-sends-fics-play",
        "fics-auth-guest",
    ] {
        let text = i18n.t(key).to_lowercase();
        assert!(
            !text.contains("digite sua senha") && !text.contains("sua senha"),
            "{key}"
        );
    }
}

// ---- A tela do FICS --------------------------------------------------------------------------

/// Todo texto desenhado no quadro, com o canto direito de cada um.
fn texts(shape: &bevy_egui::egui::Shape, out: &mut Vec<(String, f32)>) {
    use bevy_egui::egui::Shape;
    match shape {
        Shape::Text(text) => {
            // O retângulo visível (o texto centrado tem as linhas em volta da âncora).
            let right = text.visual_bounding_rect().right();
            out.push((text.galley.text().to_owned(), right));
        }
        Shape::Vec(list) => list.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

#[test]
fn the_fics_screen_draws_every_state_in_plain_language_with_no_password_field() {
    use crate::ui_theme::{Density, ThemeId, test_context};
    use bevy_egui::egui;
    const WIDTH: f32 = 560.0;
    let i18n = I18n::pt_br();
    for theme in ["dark", "light"] {
        let ctx = test_context(ThemeId::from_key(theme).unwrap(), Density::default());
        for name in [
            "fics-off",
            "fics-guest",
            "fics-lobby",
            "fics-waiting",
            "fics-challenges",
            "fics-observe",
            "fics-dropped",
        ] {
            let scene = super::demo::Scene::named(name).unwrap();
            let mut state = super::OnlineUi::default();
            scene.open_dialog(&mut state);
            assert_eq!(state.provider, Provider::Fics, "{name}");
            let view = scene.fics_view(T0);
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(WIDTH, 3000.0),
                )),
                ..Default::default()
            };
            let mut found = Vec::new();
            // Dois quadros: o egui só acerta alturas e fontes depois do primeiro.
            for _ in 0..2 {
                found.clear();
                let mut intents = Vec::new();
                let mut out = ctx.run_ui(raw.clone(), |ui| {
                    fics_view::body(ui, &i18n, &mut state, &view, &mut intents);
                });
                out.textures_delta.clear();
                for clipped in &out.shapes {
                    texts(&clipped.shape, &mut found);
                }
            }
            assert!(found.len() > 3, "{name}: desenhou");
            for (text, right) in &found {
                let lower = text.to_lowercase();
                for banned in [
                    "token",
                    "login",
                    "credencial",
                    "password",
                    "sua senha",
                    "etapa",
                ] {
                    assert!(!lower.contains(banned), "{banned:?} em {text:?} ({name})");
                }
                // Nenhum rótulo de campo de senha ("Senha", "Senha:").
                assert!(!lower.trim().starts_with("senha"), "{text:?} ({name})");
                assert!(
                    *right <= WIDTH + 1.0,
                    "{text:?} passa da borda ({name}, {theme})"
                );
            }
            let all: String = found.iter().map(|(t, _)| t.as_str()).collect();
            if matches!(name, "fics-off" | "fics-guest") {
                // O aviso honesto antes de qualquer conexão.
                for needed in [
                    "sem criptografia",
                    "visitante",
                    "nunca pede nem envia senha",
                ] {
                    assert!(all.contains(needed), "{name}: falta {needed:?}");
                }
            }
            if name == "fics-dropped" {
                assert!(all.contains("perdida") && all.contains("Descartar a sessão"));
            }
            if name == "fics-challenges" {
                assert!(all.contains("só joga xadrez padrão"), "variante recusada");
            }
        }
    }
}
