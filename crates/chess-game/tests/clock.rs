#![allow(clippy::unwrap_used)]
//! Relógio derivado dos Eventos (ADR-0002): base + incremento, Pausa (DEC-42),
//! bandeira (art. 6.9, com a exceção de impossibilidade de mate) e Engine (DEC-33).
//! O tempo vem só dos `Timestamp`s injetados.

mod common;
use chess_core::Color;
use chess_game::{
    Command, EndReason, EventKind, Game, GameResult, GameSetup, PolicyKind, Rejection,
};
use common::*;

fn assisted_timed(base: u64, inc: u64) -> Game {
    timed(GameSetup::standard(PolicyKind::Assisted), base, inc)
}

fn kinds_of(events: &[chess_game::Event]) -> Vec<EventKind> {
    events.iter().map(|e| e.kind.clone()).collect()
}

#[test]
fn sem_controle_de_tempo_nao_ha_relogio() {
    let game = assisted();
    assert!(game.clock_at(at(5000)).is_none());
    assert!(game.snapshot().clock.is_none());
}

#[test]
fn as_brancas_correm_desde_o_inicio_e_o_tempo_restante_e_consultavel_por_instante() {
    let game = assisted_timed(60_000, 0);
    let r = game.clock_at(at(4000)).unwrap();
    assert_eq!(r.running, Some(Color::White));
    assert_eq!((r.white_ms, r.black_ms), (56_000, 60_000));
    assert!(r.flag.is_none());
    // Consultar não muda a Partida.
    assert!(game.events().is_empty());
}

#[test]
fn lance_concluido_soma_o_incremento_e_passa_a_vez_do_relogio() {
    let mut game = assisted_timed(60_000, 5_000);
    drop_piece(&mut game, 1_000, "e2", "e4").unwrap();
    let r = game.clock_at(at(1_000)).unwrap();
    assert_eq!(r.running, Some(Color::Black));
    assert_eq!((r.white_ms, r.black_ms), (64_000, 60_000));

    drop_piece(&mut game, 4_000, "e7", "e5").unwrap();
    let r = game.clock_at(at(4_000)).unwrap();
    assert_eq!(r.running, Some(Color::White));
    assert_eq!((r.white_ms, r.black_ms), (64_000, 62_000));
    // E o relógio das brancas volta a correr.
    assert_eq!(game.clock_at(at(6_000)).unwrap().white_ms, 62_000);
}

#[test]
fn assistido_com_relogio_troca_sozinho_e_nao_penaliza_lance_ilegal() {
    let mut game = assisted_timed(60_000, 0);
    let events = drop_piece(&mut game, 2_000, "e2", "e5").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::IllegalMoveRejected { .. }]
    ));
    let r = game.clock_at(at(2_000)).unwrap();
    assert_eq!(r.running, Some(Color::White), "a vez não passou");
    assert_eq!((r.white_ms, r.black_ms), (58_000, 60_000), "sem penalidade");
}

#[test]
fn pausa_para_o_relogio_e_retomar_continua_de_onde_parou() {
    let mut game = assisted_timed(60_000, 0);
    game.handle(Command::Pause, at(10_000)).unwrap();
    let r = game.clock_at(at(30_000)).unwrap();
    assert!(r.paused);
    assert_eq!(r.white_ms, 50_000, "parado desde 10 s");
    assert!(game.snapshot().paused);

    // Em Pausa nada se joga; só Retomar (e Abandonar).
    assert_eq!(
        drop_piece(&mut game, 31_000, "e2", "e4"),
        Err(Rejection::Paused)
    );
    assert_eq!(
        game.handle(Command::Pause, at(31_000)),
        Err(Rejection::AlreadyPaused)
    );
    game.handle(Command::Resume, at(40_000)).unwrap();
    let r = game.clock_at(at(45_000)).unwrap();
    assert!(!r.paused);
    assert_eq!(r.white_ms, 45_000);
    assert_eq!(
        game.handle(Command::Resume, at(46_000)),
        Err(Rejection::NotPaused)
    );
}

#[test]
fn pausa_existe_sem_controle_de_tempo_e_fica_registrada() {
    let mut game = assisted();
    let events = game.handle(Command::Pause, at(1_000)).unwrap();
    assert!(matches!(kinds_of(&events).as_slice(), [EventKind::Paused]));
    game.handle(Command::Resume, at(2_000)).unwrap();
    assert!(matches!(game.events()[1].kind, EventKind::Resumed));
}

#[test]
fn pausa_nao_deixa_a_bandeira_cair() {
    let mut game = assisted_timed(10_000, 0);
    game.handle(Command::Pause, at(5_000)).unwrap();
    // Muito tempo depois, ainda em Pausa: nada de bandeira.
    game.handle(Command::Resume, at(500_000)).unwrap();
    assert!(game.snapshot().outcome.is_none());
    assert_eq!(game.clock_at(at(500_000)).unwrap().white_ms, 5_000);
}

#[test]
fn abandonar_vale_em_pausa() {
    let mut game = assisted_timed(10_000, 0);
    game.handle(Command::Pause, at(1_000)).unwrap();
    game.handle(Command::Resign { side: Color::White }, at(2_000))
        .unwrap();
    assert_eq!(
        game.snapshot().outcome.unwrap().reason,
        EndReason::Resignation
    );
}

#[test]
fn reabrir_congela_o_relogio_no_ultimo_evento_gravado() {
    let mut game = assisted_timed(60_000, 0);
    drop_piece(&mut game, 3_000, "e2", "e4").unwrap();
    // O programa fecha; reabre com o log e a Partida volta pausada (DEC-42).
    let mut reopened = Game::replay(game.setup().clone(), game.events());
    let events = reopened.reopen_paused().unwrap();
    assert!(matches!(kinds_of(&events).as_slice(), [EventKind::Paused]));
    let r = reopened.clock_at(at(900_000)).unwrap();
    assert!(r.paused);
    assert_eq!((r.white_ms, r.black_ms), (57_000, 60_000));
    // Idempotente.
    assert!(reopened.reopen_paused().unwrap().is_empty());
}

#[test]
fn comando_depois_do_tempo_registra_a_bandeira_e_encerra_com_derrota() {
    let mut game = assisted_timed(10_000, 0);
    drop_piece(&mut game, 9_999, "e2", "e4").unwrap();
    // Pretas têm 10 s desde 9 999 ms: o comando em 20 000 chega tarde.
    let events = drop_piece(&mut game, 20_000, "e7", "e5").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::FlagFell {
                side: Color::Black,
                flag_at_ms: 19_999
            },
            EventKind::GameEnded { .. }
        ]
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::WhiteWins);
    assert_eq!(outcome.reason, EndReason::Timeout);
    assert_eq!(game.snapshot().history.len(), 1, "o lance tardio não vale");
}

#[test]
fn bandeira_e_consultavel_por_instante_sem_comando() {
    let game = assisted_timed(10_000, 0);
    assert!(game.clock_at(at(9_999)).unwrap().flag.is_none());
    let flag = game.clock_at(at(10_000)).unwrap().flag.unwrap();
    assert_eq!(flag.side, Color::White);
    assert_eq!(flag.at_ms, 10_000);
    assert_eq!(flag.result, GameResult::BlackWins);
    assert_eq!(game.clock_at(at(12_000)).unwrap().white_ms, 0);
}

#[test]
fn bandeira_sem_material_de_mate_do_adversario_e_empate() {
    // Art. 6.9: as brancas perdem por tempo, mas as pretas só têm o rei.
    let mut game = timed(
        GameSetup::from_fen(PolicyKind::Assisted, "7k/8/8/8/8/8/8/K5R1 w - - 0 1").unwrap(),
        10_000,
        0,
    );
    let flag = game.clock_at(at(10_000)).unwrap().flag.unwrap();
    assert_eq!(flag.result, GameResult::Draw);
    // Qualquer Comando que chegue com a bandeira caída só registra a bandeira.
    let events = game
        .handle(Command::Resign { side: Color::Black }, at(10_000))
        .unwrap();
    assert!(matches!(events[0].kind, EventKind::FlagFell { .. }));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::TimeoutDraw);
}

#[test]
fn bandeira_em_posicao_morta_por_bloqueio_de_pecoes_e_empate() {
    // Art. 6.9: as pretas têm peões, mas nenhuma série de lances legais dá mate.
    let game = timed(
        GameSetup::from_fen(
            PolicyKind::Assisted,
            "4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/4K3 w - - 0 1",
        )
        .unwrap(),
        10_000,
        0,
    );
    let flag = game.clock_at(at(10_000)).unwrap().flag.unwrap();
    assert_eq!(flag.result, GameResult::Draw);
}

#[test]
fn bandeira_com_material_de_mate_e_derrota_mesmo_com_pouco_material() {
    // Pretas têm rei e torre: podem dar mate; a bandeira branca é derrota.
    let game = timed(
        GameSetup::from_fen(PolicyKind::Assisted, "7k/8/8/8/8/8/r7/K7 w - - 0 1").unwrap(),
        10_000,
        0,
    );
    let flag = game.clock_at(at(10_000)).unwrap().flag.unwrap();
    assert_eq!(flag.result, GameResult::BlackWins);
}

#[test]
fn relogio_para_ao_fim_da_partida() {
    let mut game = assisted_timed(60_000, 0);
    game.handle(Command::Resign { side: Color::White }, at(5_000))
        .unwrap();
    let r = game.clock_at(at(100_000)).unwrap();
    assert_eq!(r.running, None);
    assert_eq!(r.white_ms, 55_000);
    assert!(r.flag.is_none());
}

#[test]
fn lance_da_engine_conclui_e_aciona_o_relogio_da_engine() {
    for policy in [PolicyKind::Assisted, PolicyKind::FideSimulation] {
        let mut game = timed(GameSetup::standard(policy), 60_000, 2_000);
        match policy {
            PolicyKind::FideSimulation => {
                fide_play(&mut game, 1_000, "e4");
            }
            _ => {
                drop_piece(&mut game, 1_000, "e2", "e4").unwrap();
            }
        }
        let events = game
            .handle(Command::EngineMove { uci: "e7e5".into() }, at(8_000))
            .unwrap();
        assert!(matches!(
            kinds_of(&events).as_slice(),
            [EventKind::MoveCompleted {
                side: Color::Black,
                ..
            }]
        ));
        assert_eq!(game.snapshot().history.len(), 2);
        let r = game.clock_at(at(8_000)).unwrap();
        assert_eq!(r.running, Some(Color::White), "{policy:?}");
        // Pretas gastaram ~7 s e ganharam o incremento.
        assert!(r.black_ms > 50_000 && r.black_ms < 60_000, "{policy:?}");
    }
}

#[test]
fn lance_ilegal_da_engine_e_logado_e_nao_muda_a_partida() {
    for uci in ["e7e5", "zz99", ""] {
        // É a vez das brancas: e7e5 é do lado errado; os outros nem são UCI.
        let mut game = assisted();
        let before = game.snapshot();
        let events = game
            .handle(Command::EngineMove { uci: uci.into() }, at(1_000))
            .unwrap();
        assert!(
            matches!(kinds_of(&events).as_slice(), [EventKind::EngineMoveRejected { uci: u, .. }] if u == uci),
            "{uci}"
        );
        assert_eq!(game.snapshot().position, before.position);
    }
}

#[test]
fn versao_das_leis_e_exposta_so_na_simulacao_fide() {
    assert_eq!(
        fide().snapshot().rules_version,
        Some("FIDE Laws of Chess, 1 Jan 2023")
    );
    assert_eq!(
        fide().setup().rules_version(),
        Some(chess_game::FIDE_LAWS_VERSION)
    );
    assert_eq!(assisted().snapshot().rules_version, None);
}

#[test]
fn aviso_de_controle_nao_classico_abaixo_de_60_minutos() {
    let fast = timed(
        GameSetup::standard(PolicyKind::FideSimulation),
        10 * 60_000,
        5_000,
    );
    assert!(fast.snapshot().non_classical_notice);
    let classical = timed(
        GameSetup::standard(PolicyKind::FideSimulation),
        90 * 60_000,
        30_000,
    );
    assert!(!classical.snapshot().non_classical_notice);
}
