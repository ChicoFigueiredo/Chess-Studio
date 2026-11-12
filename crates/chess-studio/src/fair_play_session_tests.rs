//! A Trava de fair play segue a Partida do **usuário**, não a Sessão que o tabuleiro mostra
//! (#304, FP-06). O Treino (Resolução, memória de sequência e de posição, revisão do Treino) e
//! o modo espectador (#302) põem outra Sessão no tabuleiro e guardam a do usuário por baixo,
//! intacta: a Partida competitiva local, a Partida online em tempo real e a correspondência que
//! estavam no tabuleiro continuam em andamento e travam como se estivessem nele.
//!
//! Cada teste chama o **serviço** que calcula (o ponto único `FairPlay`, `GuardedAnalysis`,
//! `GuardedOpponent`, `GuardedMoveEngine`, `hud_policy::effective`, `opening_stats`,
//! `search_library`, a avaliação em cache), nunca só o estado de um botão. A Partida online é a
//! de verdade, contra o servidor falso e o relógio falso (nenhuma chamada ao Lichess).
//!
//! Os outros caminhos que trocam a Partida do tabuleiro sem encerrá-la (Nova Partida,
//! Biblioteca, Retomar, FEN, troca de Perfil local) **interrompem** a Partida competitiva local:
//! ela vai para o banco, pausada (DEC-42), e volta por Retomar (#193). A revisão não troca nada.

#![allow(clippy::unwrap_used)]

use super::service_tests::{
    NAJDORF, by_position, correspondence_after, engine_config, env, position_after, seat,
};
use super::*;
use crate::explorer::{CloudDriver, CloudRow, Notice};
use crate::net_service::NetService;
use crate::new_game::{NewGameConfig, Opponent as NewGameOpponent};
use crate::persistence::Persistence;
use crate::training_ui::TrainingUi;
use chess_game::{Command, GameSetup, OnlineGame, OnlinePlayer, Opponent, PolicyKind, Variant};
use chess_store::Store;
use std::time::Instant;

/// O que o sistema `follow_app_state` guarda entre quadros.
type Last = CompetitiveGame;

/// Um quadro de `net_service::follow_app_state`: a trava acompanha a Partida do usuário.
fn follow(service: &NetService, shown: &Session, training: &TrainingUi, last: &mut Last) {
    service.follow_session(shown, Some(training), last);
}

/// Os três tipos de Exercício que trocam a Sessão: Resolução, memória de sequência e memória
/// de posição.
const EXERCISES: [&str; 3] = [
    "afogamento-001",
    "memoria-italiana-001",
    "memoria-posicao-003",
];

fn play(session: &mut Session, uci: &str) {
    let mv: chess_core::Move = uci.parse().unwrap();
    session.command(Command::Release {
        from: mv.from(),
        to: mv.to(),
    });
}

/// A Partida competitiva local do usuário (Simulação FIDE) em andamento.
fn competitive() -> Session {
    let mut session = Session::new();
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2e4");
    assert!(session.competitive_in_progress());
    session
}

/// A Partida competitiva local contra a Engine (a Engine de pretas; o executável não existe:
/// o `EnginePlayer` nasce, o processo não).
fn competitive_vs_engine(fair: &FairPlay) -> Session {
    let mut session = Session::new();
    session
        .start_vs_engine(
            &NewGameConfig {
                opponent: NewGameOpponent::Engine,
                user_side: chess_core::Color::White,
                ..NewGameConfig::for_policy(PolicyKind::FideSimulation)
            },
            engine_config(),
            fair,
        )
        .unwrap();
    assert!(session.engine_active() && session.competitive_in_progress());
    session
}

/// Uma Partida Assistida contra a Engine (outra Partida, não a competitiva).
fn assisted_vs_engine() -> Game {
    Game::new(GameSetup::standard(PolicyKind::Assisted).with_engine(seat(chess_core::Color::Black)))
}

/// A Partida da Engine online (AD-05), que o modo espectador mostra.
fn online_engine_game() -> Game {
    let setup = GameSetup::online(
        OnlineGame {
            provider: "lichess-bot".into(),
            remote_id: "En9ineQ1".into(),
            local_color: chess_core::Color::White,
            opponent: Opponent::Human,
            white: OnlinePlayer {
                name: "Motor".into(),
                rating: None,
            },
            black: OnlinePlayer {
                name: "Salma".into(),
                rating: None,
            },
            rated: false,
            days_per_move: None,
        },
        Variant::Standard,
        None,
    )
    .unwrap();
    Game::new(setup)
}

/// Engine que só conta as buscas que chegaram a ela (a Engine online).
struct Counting(Arc<Mutex<usize>>);

impl chess_providers::MoveEngine for Counting {
    fn search(&mut self, _request: chess_uci::SearchRequest) -> u64 {
        *self.0.lock().unwrap() += 1;
        1
    }

    fn cancel(&mut self) {}

    fn poll(&mut self) -> Option<chess_uci::ControllerEvent> {
        None
    }
}

/// A busca que a Engine online faria para chegar a `position` a partir da inicial.
fn search_for(moves: &[chess_core::Move]) -> chess_uci::SearchRequest {
    chess_uci::SearchRequest {
        position: chess_uci::PositionSpec::startpos().with_moves(moves.iter().copied()),
        go: chess_uci::GoParams::default(),
        limit: std::time::Duration::from_secs(1),
    }
}

/// Cada serviço que calcula, chamado como o app o chama, sobre o que o tabuleiro mostra e sobre
/// a Posição da Partida do usuário (digitada num FEN, na Biblioteca, num desafio). Devolve os
/// serviços que calcularam: vazio quando a trava vale para todos. Com a Partida do usuário no
/// tabuleiro, a Engine adversária de outra Partida não entra: a Nova Partida contra a Engine
/// interrompe a competitiva (seção 3) e a trava dela não vale para a adversária (FP-02).
fn leaks(
    service: &NetService,
    shown: &Session,
    user: &Game,
    users_game_on_board: bool,
) -> Vec<String> {
    let fair = service.fair_play();
    let mut out = Vec::new();
    let user_position = user.snapshot().position.clone();
    let user_fen = user_position.to_fen();
    let user_moves: Vec<chess_core::Move> = user.snapshot().history.iter().map(|h| h.mv).collect();
    let board = Subject::shown(shown).unwrap();
    let typed = Subject::position(&user_position);
    for (place, subject) in [("tabuleiro", &board), ("Posição da Partida", &typed)] {
        let analysis = chess_uci::AnalysisConfig::new(chess_uci::EngineConfig::new(
            "/nao/existe/engine-de-analise",
        ));
        if GuardedAnalysis::start(fair, subject, analysis).is_ok() {
            out.push(format!("análise da Engine ({place})"));
        }
        let api = chess_providers::ChessApi::new(service.net().clone(), service.clock());
        if GuardedAnalysis::start_remote(fair, subject, api).is_ok() {
            out.push(format!("Engine remota ({place})"));
        }
        if fair.check(Computation::Explorer, subject).is_ok() {
            out.push(format!("Explorador ({place})"));
        }
        if fair.check(Computation::Endgames, subject).is_ok() {
            out.push(format!("finais ({place})"));
        }
        let store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        if opening_stats(fair, subject, &store, profile, &user_fen).is_ok() {
            out.push(format!("estatística da Biblioteca ({place})"));
        }
    }
    // A avaliação em cache da Posição do tabuleiro (Painel Análise).
    let row = CloudDriver::default().drive(
        &shown.snapshot().position,
        Instant::now(),
        service.clock().now(),
        None,
        Some(&board),
        service,
    );
    if row != CloudRow::Notice(Notice::FairPlay) {
        out.push("avaliação em cache".to_owned());
    }
    // A dica e os Auxílios que calculam, no tabuleiro.
    let allowed = crate::hud_policy::effective(shown, fair);
    if allowed.hints
        || allowed.suggestion_arrows
        || allowed.attacked_squares
        || allowed.threats
        || allowed.ghosts
        || allowed.engine_line
    {
        out.push("dica do HUD".to_owned());
    }
    // A busca da Biblioteca pela Posição da Partida do usuário, digitada no filtro.
    let store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    if search_library(Some(fair), None, &store, &by_position(&user_fen, profile)).is_ok() {
        out.push("busca por Posição da Biblioteca".to_owned());
    }
    // A Engine adversária de outra Partida (Nova Partida Assistida contra a Engine).
    if !users_game_on_board
        && GuardedOpponent::start(
            fair,
            &assisted_vs_engine(),
            seat(chess_core::Color::Black),
            engine_config(),
        )
        .is_ok()
    {
        out.push("Engine adversária de outra Partida".to_owned());
    }
    // A Engine online: ligar, aceitar o desafio a partir da Posição e pensar nela.
    if fair.check_anywhere(Computation::OnlineEngine).is_ok() {
        out.push("Engine online (ligar)".to_owned());
    }
    if fair
        .check_start(Computation::OnlineEngine, Some(&user_position))
        .is_ok()
    {
        out.push("Engine online (desafio da Posição)".to_owned());
    }
    let searches = Arc::new(Mutex::new(0));
    let mut engine = GuardedMoveEngine::new(fair.clone(), Box::new(Counting(searches.clone())));
    chess_providers::MoveEngine::search(&mut engine, search_for(&user_moves));
    if *searches.lock().unwrap() > 0 {
        out.push("Engine online (busca)".to_owned());
    }
    out
}

/// A Partida do usuário (`user`) está guardada por baixo do que o tabuleiro mostra: nada calcula.
fn assert_locked(service: &NetService, shown: &Session, user: &Game, case: &str) {
    let leaked = leaks(service, shown, user, false);
    assert!(leaked.is_empty(), "{case}: calcularam {leaked:?}");
    assert!(service.fair_play().everything_locked(), "{case}");
}

/// A Partida do usuário está no tabuleiro: nada calcula, fora a adversária dela (FP-02).
fn assert_locked_on_board(service: &NetService, shown: &Session, case: &str) {
    let leaked = leaks(service, shown, shown.game(), true);
    assert!(leaked.is_empty(), "{case}: calcularam {leaked:?}");
    assert!(service.fair_play().everything_locked(), "{case}");
}

/// A Partida competitiva do usuário (`user`) foi interrompida: nada segura a trava dela e a
/// análise da Posição dela, digitada num FEN, sai. É o comportamento de hoje; a regra para a
/// Partida interrompida está pendente (#305).
fn assert_interrupted(service: &NetService, shown: &Session, user: &Game) {
    let leaked = leaks(service, shown, user, false);
    assert!(
        leaked
            .iter()
            .any(|l| l == "análise da Engine (Posição da Partida)"),
        "#305: {leaked:?}"
    );
    assert!(!service.fair_play().any(), "#305");
}

// ---- 1. Partida competitiva local + Treino ----------------------------------------------------

#[test]
fn opening_the_training_over_a_local_competitive_game_keeps_every_service_locked() {
    for id in EXERCISES {
        let service = NetService::in_memory();
        let mut training = TrainingUi::default();
        let mut session = competitive();
        let user = session.game().clone();
        let mut last = Last::default();
        follow(&service, &session, &training, &mut last);
        assert_locked_on_board(&service, &session, "no tabuleiro");

        training.start_for_tests(id, &mut session);
        assert!(
            training.session_swapped(),
            "{id}: o Exercício está no tabuleiro"
        );
        assert!(
            !session.competitive_in_progress(),
            "{id}: a Sessão mostrada é Assistida"
        );
        follow(&service, &session, &training, &mut last);
        assert_locked(&service, &session, &user, id);
    }
}

#[test]
fn the_recall_and_the_training_review_keep_the_lock() {
    // Memória de sequência: a reprodução troca a Sessão de novo.
    let service = NetService::in_memory();
    let mut training = TrainingUi::default();
    let mut session = competitive();
    let user = session.game().clone();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    training.start_for_tests("memoria-italiana-001", &mut session);
    training.recall_for_tests(&mut session);
    assert!(session.review().is_none(), "reprodução: os gestos valem");
    follow(&service, &session, &training, &mut last);
    assert_locked(
        &service,
        &session,
        &user,
        "reprodução da memória de sequência",
    );

    // Resolução que termina: a revisão do Treino fica no tabuleiro.
    let service = NetService::in_memory();
    let mut training = TrainingUi::default();
    let mut session = competitive();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    training.start_for_tests("afogamento-001", &mut session);
    training.finish_for_tests(&mut session);
    assert!(session.review().is_some(), "revisão do Treino");
    follow(&service, &session, &training, &mut last);
    assert_locked(&service, &session, &user, "revisão do Treino");
}

/// A brecha da issue, passo a passo: durante a Partida competitiva, abrir o Treino e carregar no
/// tabuleiro dele (Painel Auxílios > Carregar FEN) a Posição da Partida guardada.
#[test]
fn the_position_of_the_kept_game_typed_on_the_training_board_stays_locked() {
    let service = NetService::in_memory();
    let mut training = TrainingUi::default();
    let mut session = competitive();
    let user = session.game().clone();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    training.start_for_tests("afogamento-001", &mut session);
    let mut tools = crate::fen_pgn::FileTools {
        fen_text: user.snapshot().position.to_fen(),
        ..crate::fen_pgn::FileTools::default()
    };
    tools.load_fen(&mut session);
    assert_eq!(tools.fen_result, Some(Ok(())));
    assert_eq!(session.snapshot().position, user.snapshot().position);
    assert!(
        training.session_swapped(),
        "o Treino ainda guarda a Partida"
    );
    follow(&service, &session, &training, &mut last);
    assert_locked(&service, &session, &user, "FEN no tabuleiro do Treino");
}

/// FP-02: a Engine adversária da Partida competitiva joga com a trava dela; durante o Treino, a
/// Engine de outra Partida não. Ao sair do Treino, a adversária da Partida volta a jogar mesmo
/// no quadro em que a trava ainda não viu a troca (a ordem dos sistemas não importa).
#[test]
fn the_opponent_engine_of_the_kept_game_plays_again_when_the_training_closes() {
    let service = NetService::in_memory();
    let fair = service.fair_play().clone();
    let mut training = TrainingUi::default();
    let mut session = competitive_vs_engine(&fair);
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    session.tick_engine(&fair);
    assert!(
        session.engine_active(),
        "a adversária da própria Partida joga"
    );

    training.start_for_tests("afogamento-001", &mut session);
    follow(&service, &session, &training, &mut last);
    let mut other = Session::new();
    let refused = other.start_vs_engine(
        &NewGameConfig {
            opponent: NewGameOpponent::Engine,
            ..NewGameConfig::default()
        },
        engine_config(),
        &fair,
    );
    assert_eq!(
        refused.map_err(|l| l.scope),
        Err(LockScope::Everything),
        "outra Partida contra a Engine, com a competitiva guardada"
    );

    training.leave_for_tests(&mut session);
    assert!(session.competitive_in_progress());
    // A Engine pede licença antes de a trava acompanhar a troca.
    session.tick_engine(&fair);
    assert!(
        session.engine_active(),
        "a adversária não é descartada na volta"
    );
    follow(&service, &session, &training, &mut last);
    session.tick_engine(&fair);
    assert!(session.engine_active());
    assert_eq!(session.engine_locked(), None);
    assert!(fair.everything_locked(), "o resto continua travado");
}

/// O Treino por cima do modo espectador (#302), e o inverso: a Partida do usuário continua por
/// baixo dos dois.
#[test]
fn the_training_over_the_spectator_mode_keeps_the_users_game_locked() {
    let service = NetService::in_memory();
    let mut training = TrainingUi::default();
    let mut session = competitive();
    let user = session.game().clone();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    assert!(session.watch(&online_engine_game()));
    follow(&service, &session, &training, &mut last);
    assert_locked(&service, &session, &user, "modo espectador");
    training.start_for_tests("afogamento-001", &mut session);
    follow(&service, &session, &training, &mut last);
    assert_locked(&service, &session, &user, "Treino sobre o modo espectador");
    training.leave_for_tests(&mut session);
    assert!(session.is_spectator());
    follow(&service, &session, &training, &mut last);
    assert_locked(&service, &session, &user, "de volta ao modo espectador");
    assert!(session.leave_spectator());
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "de volta à Partida");
}

// ---- 2. Partida online em andamento + Treino --------------------------------------------------

/// Partida online em tempo real contra pessoa no tabuleiro (servidor falso); o jogador abre o
/// Treino. A sessão continua aberta (PO-17) e nada calcula.
#[test]
fn opening_the_training_during_a_real_time_online_game_keeps_every_service_locked() {
    let mut env = env();
    let _feed = env.real_time_game();
    let user = env.session.game().clone();
    let mut training = TrainingUi::default();
    let mut last = Last::default();
    follow(&env.service, &env.session, &training, &mut last);
    for id in EXERCISES {
        training.start_for_tests(id, &mut env.session);
        for _ in 0..3 {
            env.frame();
        }
        follow(&env.service, &env.session, &training, &mut last);
        assert!(
            !env.session.is_online(),
            "{id}: o Exercício está no tabuleiro"
        );
        assert_locked(&env.service, &env.session, &user, id);
        training.leave_for_tests(&mut env.session);
        env.frame();
        assert!(
            env.session.is_online(),
            "a Partida online volta ao tabuleiro"
        );
    }
}

/// Correspondência contra pessoa no tabuleiro; o jogador abre o Treino. A Partida e a estrutura
/// dela continuam travadas (PO-13) e, como a Partida do usuário segue por baixo do Treino, o
/// resto também (FP-06), como no modo espectador.
#[test]
fn opening_the_training_over_a_correspondence_game_keeps_it_locked() {
    let service = NetService::in_memory();
    let fair = service.fair_play().clone();
    let corr = correspondence_after(&NAJDORF);
    fair.follow_games([&corr]);
    let mut session = Session::new();
    session.show_online(corr.clone(), None, false);
    let mut training = TrainingUi::default();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    training.start_for_tests("afogamento-001", &mut session);
    follow(&service, &session, &training, &mut last);
    // PO-13: a Posição da correspondência, onde quer que apareça.
    let near = Subject::position(&position_after(&NAJDORF));
    for computation in Computation::ALL {
        assert!(fair.check(computation, &near).is_err(), "{computation:?}");
    }
    assert_locked(&service, &session, &corr, "Treino sobre a correspondência");
    training.leave_for_tests(&mut session);
    follow(&service, &session, &training, &mut last);
    assert!(session.is_online());
    assert!(fair.everything_locked());
}

// ---- 3. Os outros caminhos que trocam a Partida do tabuleiro ----------------------------------

/// Revisar a Partida competitiva em andamento não troca a Sessão nem solta nada.
#[test]
fn reviewing_the_competitive_game_in_progress_keeps_the_lock() {
    let service = NetService::in_memory();
    let training = TrainingUi::default();
    let mut session = competitive();
    let mut last = Last::default();
    session.set_review_ply(Some(0));
    assert!(session.review().is_some());
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "revisão da Partida em andamento");
}

/// Uma Partida guardada no banco de `persistence`, como a Biblioteca e o Retomar a reabrem
/// (`switch_to_game`, pausada) e a põem no tabuleiro.
fn reopen(persistence: &mut Persistence, session: &mut Session, id: chess_store::GameId) {
    let game = persistence.switch_to_game(id).unwrap();
    session.open_game(game);
    persistence.track(session);
}

/// Nova Partida (também contra a Engine), Retomar e a Biblioteca **interrompem** a Partida
/// competitiva local: ela sai do tabuleiro sem fim, fica no banco sem resultado e volta pausada
/// (DEC-42, #193). Interrompida, ela não está em andamento e nada segura a trava dela: os
/// serviços calculam inclusive a Posição dela, digitada num FEN. A regra para a Partida
/// interrompida está pendente (#305); as asserções `leaks(..)` não vazias abaixo são as que
/// mudam com ela. De volta ao tabuleiro, a trava liga no mesmo quadro.
#[test]
fn interrupting_the_competitive_game_releases_its_lock_until_it_comes_back() {
    let service = NetService::in_memory();
    let fair = service.fair_play().clone();
    let training = TrainingUi::default();
    let mut persistence = Persistence::new(Store::open_in_memory().unwrap());
    let mut session = persistence.restore().session;
    let mut last = Last::default();
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2e4");
    persistence.sync(&session);
    let competitive_id = persistence.current_game().unwrap();
    let user = session.game().clone();
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "no tabuleiro");

    // Nova Partida contra a Engine: a trava da competitiva no tabuleiro não vale para a
    // adversária (FP-02), e a Partida nova a interrompe.
    session
        .start_vs_engine(
            &NewGameConfig {
                opponent: NewGameOpponent::Engine,
                ..NewGameConfig::default()
            },
            engine_config(),
            &fair,
        )
        .unwrap();
    persistence.sync(&session);
    follow(&service, &session, &training, &mut last);
    assert_eq!(last, CompetitiveGame::None);
    assert_eq!(
        persistence.resumable_game(),
        Some(competitive_id),
        "retomável"
    );
    assert_interrupted(&service, &session, &user);
    let engine_game = persistence.current_game().unwrap();

    // Retomar: a Partida volta pausada e a trava liga no mesmo quadro.
    reopen(&mut persistence, &mut session, competitive_id);
    assert!(session.snapshot().paused, "DEC-42: reabre pausada");
    follow(&service, &session, &training, &mut last);
    assert_eq!(last, CompetitiveGame::OnBoard);
    assert_locked_on_board(&service, &session, "retomada");

    // Biblioteca: abrir outra Partida (a contra a Engine) e voltar à competitiva.
    reopen(&mut persistence, &mut session, engine_game);
    follow(&service, &session, &training, &mut last);
    assert_interrupted(&service, &session, &user);
    reopen(&mut persistence, &mut session, competitive_id);
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "reaberta pela Biblioteca");

    // Nova Partida Assistida e, nela, a Posição da competitiva por FEN.
    session.start(&NewGameConfig::default());
    session
        .load_fen(&user.snapshot().position.to_fen())
        .unwrap();
    persistence.sync(&session);
    follow(&service, &session, &training, &mut last);
    assert_interrupted(&service, &session, &user);
}

/// Carregar FEN durante a Partida competitiva mantém a Política: a Partida nova é competitiva e
/// trava; a Biblioteca, que estuda o FEN numa Partida Assistida, interrompe como a Nova Partida.
#[test]
fn a_fen_keeps_the_policy_and_the_library_study_interrupts() {
    let service = NetService::in_memory();
    let training = TrainingUi::default();
    let mut session = competitive();
    let user = session.game().clone();
    let mut last = Last::default();
    follow(&service, &session, &training, &mut last);
    let fen = position_after(&NAJDORF).to_fen();
    session.load_fen(&fen).unwrap();
    assert!(session.competitive_in_progress(), "mesma Política");
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "FEN numa Partida competitiva");
    session.study_from_fen(&fen).unwrap();
    assert!(!session.competitive_in_progress());
    follow(&service, &session, &training, &mut last);
    assert_eq!(last, CompetitiveGame::None, "#305");
    assert_interrupted(&service, &session, &user);
}

/// Trocar de Perfil local grava a Partida que sai e carrega a do outro Perfil, pausada: também
/// interrompe. Ao voltar, a Partida competitiva é a retomada e trava. (Com um Exercício no
/// tabuleiro, a troca é recusada: `local_profiles.rs`.)
#[test]
fn switching_the_local_profile_interrupts_and_switching_back_locks_again() {
    let service = NetService::in_memory();
    let training = TrainingUi::default();
    let mut persistence = Persistence::new(Store::open_in_memory().unwrap());
    let ana = persistence.profile();
    let beto = persistence.create_profile("Beto").unwrap();
    let mut session = persistence.restore().session;
    let mut last = Last::default();
    session.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation));
    play(&mut session, "e2e4");
    persistence.sync(&session);
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "no tabuleiro");

    session = persistence
        .switch_profile(beto, Some(&session))
        .unwrap()
        .restored
        .session;
    follow(&service, &session, &training, &mut last);
    assert_eq!(last, CompetitiveGame::None, "#305");

    session = persistence
        .switch_profile(ana, Some(&session))
        .unwrap()
        .restored
        .session;
    assert!(
        session.competitive_in_progress(),
        "a Partida de Ana volta, pausada"
    );
    assert!(session.snapshot().paused);
    follow(&service, &session, &training, &mut last);
    assert_locked_on_board(&service, &session, "de volta ao Perfil");
}
