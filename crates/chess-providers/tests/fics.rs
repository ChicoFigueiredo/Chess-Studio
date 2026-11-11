#![allow(clippy::unwrap_used)]
//! FICS como visitante (#237), contra o servidor ICS falso em loopback e o relógio falso
//! (CX-17: nenhuma conexão real, nenhum login real). Todas as transcrições são
//! **sintéticas**: o `style 12` segue a ajuda oficial do FICS (`help style12`); as demais
//! linhas seguem o texto que os clientes de código aberto do FICS reconhecem. Nenhuma foi
//! gravada de um servidor real.

use chess_core::{CanonicalPosition, Color, Move};
use chess_game::{EndReason, Event, EventKind, Game, GameResult};
use chess_net::testing::FakeIcsServer;
use chess_net::{FakeClock, Net, NetConfig, ProviderId, Secret, Timestamp};
use chess_providers::{
    Fics, FicsError, FicsLogin, FicsNotice, FicsSession, FicsStatus, ProviderError,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);
/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const ME: &str = "GuestQWER";
const OPPONENT: &str = "GuestEFGH";
const GAME: u32 = 42;
const PASSWORD: &str = "s3nh4-que-nunca-sai";

struct Env {
    clock: Arc<FakeClock>,
    server: FakeIcsServer,
    net: Net,
    fics: Fics,
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
    let fics = Fics::new(net.clone(), clock.clone());
    Env {
        clock,
        server,
        net,
        fics,
    }
}

fn env() -> Env {
    env_with(true)
}

/// Tudo o que as consultas devolveram até agora.
#[derive(Default)]
struct Seen {
    created: bool,
    events: Vec<Event>,
    notices: Vec<FicsNotice>,
}

impl Seen {
    fn kinds(&self) -> Vec<&EventKind> {
        self.events.iter().map(|e| &e.kind).collect()
    }
}

/// Consulta a sessão (como a interface, a cada quadro) até `cond` valer.
fn poll_until(
    session: &mut FicsSession,
    seen: &mut Seen,
    mut cond: impl FnMut(&Seen, &FicsSession) -> bool,
) -> bool {
    let start = Instant::now();
    loop {
        let polled = session.poll();
        seen.created |= polled.created;
        seen.events.extend(polled.events);
        seen.notices.extend(polled.notices);
        if cond(seen, session) {
            return true;
        }
        if start.elapsed() > WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Consulta por um tempo curto, para provar que algo **não** acontece.
fn poll_for(session: &mut FicsSession, seen: &mut Seen, time: Duration) {
    let start = Instant::now();
    while start.elapsed() < time {
        poll_until(session, seen, |_, _| true);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Entrada de visitante sintética: boas-vindas, `login:`, nome, Enter e início da sessão.
fn login(env: &Env) -> (FicsSession, Seen) {
    let mut session = env.fics.connect(FicsLogin::Guest).unwrap();
    let mut seen = Seen::default();
    assert!(env.server.wait_connected(WAIT));
    env.server
        .send("\n    Free Internet Chess Server (sintético)\n\nlogin: ");
    assert!(poll_until(&mut session, &mut seen, |_, _| env
        .server
        .wait_line("guest", Duration::ZERO)));
    env.server.send(&format!(
        "\nLogging you in as \"{ME}\"; you may use this name to play unrated games.\n\
         (After logging in, do \"help register\" for more info on how to register.)\n\n\
         Press return to enter the server as \"{ME}\": "
    ));
    assert!(poll_until(&mut session, &mut seen, |_, _| env
        .server
        .wait_lines(2, Duration::ZERO)));
    env.server.send(&format!(
        "\n**** Starting FICS session as {ME}(U) ****\n\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::LoggedIn {
            handle: ME.to_owned()
        })));
    assert_eq!(session.status(), FicsStatus::Ready);
    assert!(env.server.wait_line("set style 12", WAIT));
    (session, seen)
}

/// Linha `style 12` sintética da Posição (formato de `help style12`, mais os campos de
/// relógio correndo e atraso que o FICS acrescenta).
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

/// Tabuleiro do jogador local (brancas) na Partida `GAME`.
fn my_board(position: &CanonicalPosition, white_s: i64, black_s: i64) -> String {
    let relation = if position.side_to_move() == Color::White {
        1
    } else {
        -1
    };
    style12(position, GAME, relation, white_s, black_s)
}

fn sans(session: &FicsSession) -> Vec<String> {
    session
        .game()
        .map(|g| g.snapshot().history.iter().map(|h| h.san.clone()).collect())
        .unwrap_or_default()
}

/// Começa a Partida `GAME` com o jogador local de brancas, depois da busca.
fn start_game(env: &Env, session: &mut FicsSession, seen: &mut Seen) {
    session.seek(5, 0).unwrap();
    assert!(env.server.wait_line("seek 5 0 unrated", WAIT));
    env.server.send(&format!(
        "Your seek has been posted with index 7.\n(2 player(s) saw the seek.)\nfics% \n\
         Creating: {ME} (++++) {OPPONENT} (++++) unrated blitz 5 0\n\
         {{Game {GAME} ({ME} vs. {OPPONENT}) Creating unrated blitz match.}}\n\n{}\nfics% ",
        my_board(&CanonicalPosition::start(), 300, 300)
    ));
    assert!(poll_until(session, seen, |s, _| s.created));
    assert!(seen.notices.contains(&FicsNotice::SeekPosted));
    assert!(seen.notices.contains(&FicsNotice::GameStarted {
        game: GAME,
        local_color: Color::White
    }));
}

#[test]
fn a_full_guest_game_with_repeated_late_and_cut_boards() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    start_game(&env, &mut session, &mut seen);
    assert!(session.holds_fair_play());
    assert!(env.net.fair_play_guard().is_engaged());
    let game = ["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"];
    let p = positions(&game);

    // 1. e4: intenção enviada, um único comando; o tabuleiro do FICS confirma.
    session.play(mv("e2e4")).unwrap();
    assert!(env.server.wait_line("e2e4", WAIT));
    assert!(session.game().unwrap().sent_move().is_some());
    env.server
        .send(&format!("\n{}\nfics% ", my_board(&p[1], 300, 300)));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::MoveConfirmed { mv: mv("e2e4") })));
    // O mesmo tabuleiro de novo: nada muda.
    env.server
        .send(&format!("\n{}\n", my_board(&p[1], 300, 300)));
    // 1... e5.
    env.server
        .send(&format!("\n{}\n", my_board(&p[2], 298, 297)));
    assert!(poll_until(&mut session, &mut seen, |_, s| sans(s).len() == 2));
    // Um tabuleiro atrasado (depois de 1. e4) não regride a Partida.
    env.server
        .send(&format!("\n{}\n", my_board(&p[1], 300, 300)));
    poll_for(&mut session, &mut seen, Duration::from_millis(150));
    assert_eq!(sans(&session), ["e4", "e5"]);

    // 2. Bc4: o tabuleiro dele chega cortado; o seguinte (2... Nc6) fecha o buraco.
    session.play(mv("f1c4")).unwrap();
    assert!(env.server.wait_line("f1c4", WAIT));
    let cut = my_board(&p[3], 290, 297);
    env.server.send(&format!("\n{}\n", &cut[..cut.len() / 2]));
    env.server
        .send(&format!("\n{}\n", my_board(&p[4], 290, 280)));
    assert!(poll_until(&mut session, &mut seen, |_, s| sans(s).len() == 4));
    assert!(
        seen.notices
            .contains(&FicsNotice::MoveConfirmed { mv: mv("f1c4") })
    );

    // 3. Qh5 Nf6, e o adversário propõe empate; o jogador recusa.
    session.play(mv("d1h5")).unwrap();
    assert!(env.server.wait_line("d1h5", WAIT));
    env.server
        .send(&format!("\n{}\n", my_board(&p[5], 280, 280)));
    env.server
        .send(&format!("\n{}\n", my_board(&p[6], 280, 270)));
    env.server
        .send(&format!("\n{OPPONENT} offers you a draw.\nfics% "));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::DrawOffered)));
    assert_eq!(
        session.game().unwrap().snapshot().pending_draw_offer,
        Some(Color::Black)
    );
    // Recados de outros jogadores não mexem na proposta.
    env.server.send(
        "\nGuestZZZZ tells you: I decline the draw offer, password:\n\
         GuestZZZZ(53): withdraw the draw!\nfics% ",
    );
    poll_for(&mut session, &mut seen, Duration::from_millis(100));
    assert_eq!(
        session.game().unwrap().snapshot().pending_draw_offer,
        Some(Color::Black)
    );
    assert_eq!(session.status(), FicsStatus::Ready);
    session.decline_draw().unwrap();
    assert!(env.server.wait_line("decline", WAIT));
    env.server.send(&format!(
        "\nYou decline the draw request from {OPPONENT}.\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |_, s| s
        .game()
        .unwrap()
        .snapshot()
        .pending_draw_offer
        .is_none()));

    // 4. Qxf7#: o FICS confirma o Lance e o mate.
    assert!(session.holds_fair_play());
    session.play(mv("h5f7")).unwrap();
    assert!(env.server.wait_line("h5f7", WAIT));
    env.server.send(&format!(
        "\n{}\n{{Game {GAME} ({ME} vs. {OPPONENT}) {OPPONENT} checkmated}} 1-0\n\
         No ratings adjustment done.\nfics% ",
        my_board(&p[7], 279, 270)
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::GameEnded { game: GAME })));

    assert_eq!(
        sans(&session),
        ["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"]
    );
    let snapshot = session.game().unwrap().snapshot();
    let outcome = snapshot.outcome.unwrap();
    assert_eq!(outcome.result, GameResult::WhiteWins);
    assert_eq!(outcome.reason, EndReason::Checkmate);
    // A trava só cai com o fim confirmado pelo FICS.
    assert!(!session.holds_fair_play());
    assert!(!env.net.fair_play_guard().is_engaged());
    // Envio único: cada Lance saiu uma vez, e nada além do esperado saiu.
    let sent: Vec<String> = env.server.received_lines();
    for m in ["e2e4", "f1c4", "d1h5", "h5f7"] {
        assert_eq!(sent.iter().filter(|l| *l == m).count(), 1, "{m}: {sent:?}");
    }
    assert!(!sent.iter().any(|l| l.contains(PASSWORD)));
    // Os Eventos devolvidos refazem exatamente a mesma Partida (o que a interface grava).
    assert!(seen.created);
    let replayed = Game::replay(session.game().unwrap().setup().clone(), &seen.events);
    assert_eq!(replayed.snapshot().history, snapshot.history);
    assert_eq!(replayed.snapshot().outcome, snapshot.outcome);
    assert!(
        seen.kinds()
            .iter()
            .any(|k| matches!(k, EventKind::DrawOffered { by: Color::Black }))
    );
    let setup = session.game().unwrap().setup();
    let online = setup.online_game().unwrap();
    assert_eq!(online.provider, "fics");
    assert_eq!(online.remote_id, "42");
    assert_eq!(online.local_color, Color::White);
    assert!(!online.rated);
    assert_eq!(setup.time_control().unwrap().base_ms, 5 * 60_000);
}

#[test]
fn a_password_login_is_refused_without_a_secure_channel_and_nothing_leaves() {
    let env = env();
    let login = FicsLogin::Account {
        username: "chico".to_owned(),
        password: Secret::new(PASSWORD),
    };
    assert!(
        !format!("{login:?}").contains(PASSWORD),
        "Debug esconde a senha"
    );

    let err = env.fics.connect(login).unwrap_err();

    assert_eq!(err, FicsError::InsecureChannel);
    let text = err.to_string();
    assert!(text.contains("texto puro"), "{text}");
    assert!(text.contains("visitante"), "{text}");
    assert!(!Fics::password_login_available());
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(env.server.connections(), 0, "nenhuma conexão foi aberta");
    assert!(env.server.received().is_empty(), "nada saiu");
}

#[test]
fn a_server_asking_for_a_password_gets_none_and_the_connection_closes() {
    let env = env();
    let mut session = env.fics.connect(FicsLogin::Guest).unwrap();
    let mut seen = Seen::default();
    assert!(env.server.wait_connected(WAIT));
    env.server.send("login: ");
    assert!(poll_until(&mut session, &mut seen, |_, _| env
        .server
        .wait_line("guest", Duration::ZERO)));
    // Sintético: um servidor que, em vez de aceitar o visitante, pede senha.
    env.server.send(
        "\n\"guest\" is a registered name.  If it is yours, type the password.\n\
         If not, just hit return to try another name.\n\npassword: ",
    );

    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::Closed(FicsError::PasswordRequested))));
    assert_eq!(session.status(), FicsStatus::Closed);
    assert!(env.server.wait_disconnected(WAIT));
    assert_eq!(
        env.server.received(),
        b"guest\n",
        "o servidor falso só viu o visitante"
    );
    assert!(session.seek(5, 0).is_err());
}

#[test]
fn with_the_provider_off_no_connection_is_opened() {
    let env = env_with(false);
    let mut session = env.fics.connect(FicsLogin::Guest).unwrap();
    let mut seen = Seen::default();

    assert!(poll_until(&mut session, &mut seen, |s, _| !s
        .notices
        .is_empty()));
    assert_eq!(
        seen.notices,
        [FicsNotice::Closed(FicsError::Provider(
            ProviderError::Disabled {
                provider: ProviderId::Fics
            }
        ))]
    );
    assert_eq!(session.status(), FicsStatus::Closed);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(env.server.connections(), 0);
    let text = seen.notices[0].clone();
    if let FicsNotice::Closed(error) = text {
        assert!(error.to_string().contains("Arquivo > Conexões"), "{error}");
    }
}

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

/// Linhas hostis: `style 12` com campos trocados, cortados ou absurdos, mensagens quase
/// certas, bytes aleatórios.
fn hostile_line(rng: &mut Rng) -> Vec<u8> {
    let good = style12(&CanonicalPosition::start(), 999, 1, 300, 300);
    match rng.below(8) {
        0 => {
            let mut fields: Vec<String> = good.split(' ').map(str::to_owned).collect();
            let i = 1 + rng.below(fields.len() - 1);
            fields[i] = ["-1", "99999999999", "", "zz", "<12>", "-"][rng.below(6)].to_owned();
            fields.join(" ").into_bytes()
        }
        1 => good.as_bytes()[..rng.below(good.len())].to_vec(),
        2 => format!("<12> {}", "x ".repeat(rng.below(80))).into_bytes(),
        3 => [
            "{Game 42 (",
            "{Game 99999999999 (A vs. B) A resigns} 1-0",
            "{Game 42 (GuestQWER vs. GuestEFGH) Creating unrated",
            "Challenge: (----) unrated",
            "Challenge: GuestEFGH (----) GuestQWER (----) unrated blitz x y.",
            " offers you a draw.",
            "You are now observing game -1.",
            "Removing game  from observation list.",
            "Game 7: a b c",
            "Press return to enter the server as \"",
        ][rng.below(10)]
        .as_bytes()
        .to_vec(),
        4 => (0..rng.below(400))
            .map(|_| u8::try_from(rng.below(256)).unwrap())
            .collect(),
        5 => vec![0xff, 0xfd, 0x18, 0xff, 0xfa, 0x18, 0x01, b'x'],
        6 => vec![b'Z'; 20_000 + rng.below(50_000)],
        _ => format!("<12> {}", "rnbqkbnr ".repeat(rng.below(40))).into_bytes(),
    }
}

#[test]
fn hostile_input_neither_hangs_nor_breaks_the_session() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    for _ in 0..400 {
        let mut line = hostile_line(&mut rng);
        line.extend_from_slice(b"\n\r");
        env.server.send_raw(&line);
    }
    env.server.send("fics% ");
    poll_for(&mut session, &mut seen, Duration::from_millis(400));

    assert_eq!(session.status(), FicsStatus::Ready);
    assert!(!seen.created, "nada hostil cria Partida");
    assert!(seen.events.is_empty());
    assert!(session.game().is_none());
    // A sessão segue respondendo.
    session.seek(3, 2).unwrap();
    assert!(env.server.wait_line("seek 3 2 unrated", WAIT));
}

#[test]
fn variants_are_refused_before_the_network_and_never_become_a_game() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    env.server.send(&format!(
        "\nChallenge: {OPPONENT} (----) {ME} (----) unrated wild/fr 5 0.\n\
         You can \"accept\" or \"decline\", or propose different parameters.\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |_, s| !s
        .challenges()
        .is_empty()));
    assert_eq!(session.challenges()[0].kind, "wild/fr");

    assert_eq!(
        session.accept(OPPONENT),
        Err(FicsError::UnsupportedVariant("wild/fr".to_owned()))
    );

    // Uma Partida de variante que começa mesmo assim não vira Partida local.
    env.server.send(&format!(
        "\n{{Game {GAME} ({ME} vs. {OPPONENT}) Creating unrated crazyhouse match.}}\n{}\n",
        my_board(&CanonicalPosition::start(), 300, 300)
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::VariantRefused {
            game: GAME,
            variant: "crazyhouse".to_owned()
        })));
    assert!(!seen.created);
    assert!(session.game().is_none());
    assert!(!session.holds_fair_play());
    assert!(
        !env.server
            .received_lines()
            .iter()
            .any(|l| l.starts_with("accept")),
        "nenhum aceite saiu"
    );

    // Um desafio de xadrez padrão é aceito.
    env.server.send(&format!(
        "\nChallenge: {OPPONENT} (1500) {ME} (----) unrated blitz 3 0.\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |_, s| s
        .challenges()
        .iter()
        .any(|c| c.kind == "blitz")));
    session.accept(OPPONENT).unwrap();
    assert!(env.server.wait_line(&format!("accept {OPPONENT}"), WAIT));
}

#[test]
fn observing_a_game_follows_it_without_storing_or_locking_anything() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    let line = ["d2d4", "d7d5", "c2c4", "e7e6", "b1c3"];
    let p = positions(&line);
    session.observe(77).unwrap();
    assert!(env.server.wait_line("observe 77", WAIT));
    env.server.send(&format!(
        "You are now observing game 77.\nGame 77: {ME} (++++) {OPPONENT} (++++) unrated blitz 3 0\n\n{}\nfics% ",
        style12(&p[3], 77, 0, 170, 175)
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::ObservationStarted { game: 77 })));
    let observed = session.observed().unwrap();
    assert_eq!(observed.game, 77);
    assert_eq!(observed.snapshot.position, p[3]);

    env.server
        .send(&format!("\n{}\n", style12(&p[4], 77, 0, 170, 170)));
    env.server
        .send(&format!("\n{}\n", style12(&p[5], 77, 0, 165, 170)));
    env.server.send(&format!(
        "\n{{Game 77 ({ME} vs. {OPPONENT}) {OPPONENT} resigns}} 1-0\nRemoving game 77 from observation list.\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::ObservationEnded { game: 77 })));

    let observed = session.observed().unwrap();
    let sans: Vec<&str> = observed
        .snapshot
        .history
        .iter()
        .map(|h| h.san.as_str())
        .collect();
    assert_eq!(sans, ["e6", "Nc3"]);
    assert_eq!(
        observed.snapshot.outcome.unwrap().reason,
        EndReason::Resignation
    );
    assert!(!seen.created, "observar não cria Partida do jogador");
    assert!(seen.events.is_empty(), "nada da observação vai ao banco");
    assert!(!session.holds_fair_play());
    assert!(!env.net.fair_play_guard().is_engaged());
}

#[test]
fn losing_the_connection_mid_game_loses_the_unconfirmed_move_and_keeps_the_guard_until_dropped() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    start_game(&env, &mut session, &mut seen);
    session.play(mv("e2e4")).unwrap();
    assert!(env.server.wait_line("e2e4", WAIT));

    env.server.disconnect();

    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::Closed(FicsError::ConnectionClosed {
            game_in_progress: true
        }))));
    assert!(
        seen.notices
            .contains(&FicsNotice::MoveLost { mv: mv("e2e4") })
    );
    assert!(session.game().unwrap().sent_move().is_none());
    assert!(session.play(mv("e2e4")).is_err(), "nada é reenviado");
    // O FICS não confirmou o fim: a trava fica até a interface descartar a sessão.
    assert!(env.net.fair_play_guard().is_engaged());
    drop(session);
    assert!(!env.net.fair_play_guard().is_engaged());
    assert_eq!(
        env.server
            .received_lines()
            .iter()
            .filter(|l| *l == "e2e4")
            .count(),
        1
    );
}

#[test]
fn the_login_has_a_deadline_on_the_injected_clock() {
    let env = env();
    let mut session = env.fics.connect(FicsLogin::Guest).unwrap();
    let mut seen = Seen::default();
    assert!(env.server.wait_connected(WAIT));
    env.clock.advance(Duration::from_secs(61));

    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::Closed(FicsError::LoginTimeout))));
    assert!(env.server.wait_disconnected(WAIT));
}

#[test]
fn a_new_game_with_the_number_of_a_finished_one_starts_fresh() {
    let env = env();
    let (mut session, mut seen) = login(&env);
    start_game(&env, &mut session, &mut seen);
    session.resign().unwrap();
    assert!(env.server.wait_line("resign", WAIT));
    env.server.send(&format!(
        "\n{{Game {GAME} ({ME} vs. {OPPONENT}) {ME} resigns}} 0-1\nfics% "
    ));
    assert!(poll_until(&mut session, &mut seen, |s, _| s
        .notices
        .contains(&FicsNotice::GameEnded { game: GAME })));
    let first = session.game().unwrap().snapshot().outcome.unwrap();
    assert_eq!(first.result, GameResult::BlackWins);
    assert_eq!(first.reason, EndReason::Resignation);
    assert!(!session.holds_fair_play());

    // O FICS reaproveita o número 42 para a Partida seguinte.
    let mut second = Seen::default();
    start_game(&env, &mut session, &mut second);
    assert!(session.game().unwrap().snapshot().outcome.is_none());
    assert!(session.holds_fair_play());
    session.play(mv("d2d4")).unwrap();
    assert!(env.server.wait_line("d2d4", WAIT));
}

/// Propriedade: tabuleiros de uma Partida aleatória entregues com repetição, atraso, cortes
/// (nunca dois seguidos) e lixo nunca fazem a Partida local regredir nem divergir, e o
/// último tabuleiro deixa a Partida igual à do FICS.
#[test]
fn boards_repeated_late_or_cut_never_regress_or_diverge() {
    for seed in [3_u64, 0x00c0_ffee, 0xdead_beef_0042] {
        let env = env();
        let (mut session, mut seen) = login(&env);
        start_game(&env, &mut session, &mut seen);
        let mut rng = Rng(seed);
        // Uma Partida aleatória de até 30 meio-lances.
        let mut positions = vec![CanonicalPosition::start()];
        let mut truth: Vec<Move> = Vec::new();
        for _ in 0..30 {
            let legal = positions.last().unwrap().legal_moves();
            if legal.is_empty() {
                break;
            }
            let chosen = legal[rng.below(legal.len())];
            positions.push(positions.last().unwrap().play(chosen).unwrap());
            truth.push(chosen);
        }
        let mut cut_before = false;
        for ply in 1..positions.len() {
            for _ in 0..rng.below(3) {
                match rng.below(3) {
                    0 => {
                        let old = rng.below(ply);
                        env.server
                            .send(&format!("\n{}\n", my_board(&positions[old], 100, 100)));
                    }
                    1 => env.server.send("\n<12> rnbqkbnr pppp\n"),
                    _ => env.server.send("\nfics% \n"),
                }
            }
            let board = my_board(&positions[ply], 100, 100);
            let last = ply + 1 == positions.len();
            if !last && !cut_before && rng.below(4) == 0 {
                env.server
                    .send(&format!("\n{}\n", &board[..board.len() * 2 / 3]));
                cut_before = true;
            } else {
                env.server.send(&format!("\n{board}\n"));
                cut_before = false;
            }
            poll_for(&mut session, &mut seen, Duration::from_millis(15));
            let history: Vec<Move> = session
                .game()
                .unwrap()
                .snapshot()
                .history
                .iter()
                .map(|h| h.mv)
                .collect();
            assert!(truth.starts_with(&history), "semente {seed}: divergiu");
        }
        assert!(poll_until(&mut session, &mut seen, |_, s| s
            .game()
            .unwrap()
            .snapshot()
            .history
            .len()
            == truth.len()));
        let history: Vec<Move> = session
            .game()
            .unwrap()
            .snapshot()
            .history
            .iter()
            .map(|h| h.mv)
            .collect();
        assert_eq!(history, truth, "semente {seed}");
        assert!(!session.is_desynced());
        // Nenhum Evento de Lance repetido: o log é a Partida.
        let moves = seen
            .kinds()
            .iter()
            .filter(|k| matches!(k, EventKind::MoveCompleted { .. }))
            .count();
        assert_eq!(moves, truth.len(), "semente {seed}");
    }
}
