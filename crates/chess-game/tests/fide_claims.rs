#![allow(clippy::unwrap_used)]
//! Simulação FIDE: Reclamação de empate (arts. 9.2, 9.3, 9.4, 9.5) e encerramentos
//! automáticos (arts. 9.6.1 e 9.6.2), DEC-38.

mod common;
use chess_core::Color;
use chess_game::{Command, DrawBasis, EndReason, EventKind, GameResult, Rejection, TWO_MINUTES_MS};
use common::*;

const CYCLE: &str = "Nf3 Nf6 Ng1 Ng8";

fn kinds_of(events: &[chess_game::Event]) -> Vec<EventKind> {
    events.iter().map(|e| e.kind.clone()).collect()
}

fn reason(game: &chess_game::Game) -> Option<EndReason> {
    game.snapshot().outcome.map(|o| o.reason)
}

#[test]
fn tripla_repeticao_nao_encerra_sozinha_exige_reclamacao_dec_38() {
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, "Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1 Ng8");
    assert_eq!(reason(&game), None, "terceira ocorrência sem reclamar");
    assert_eq!(
        game.snapshot().claimable_draw,
        Some(DrawBasis::ThreefoldRepetition)
    );

    let events = game.handle(Command::ClaimDraw, at(t)).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::DrawClaimAccepted {
                by: Color::White,
                basis: DrawBasis::ThreefoldRepetition,
                intended: None
            },
            EventKind::GameEnded { .. }
        ]
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::ThreefoldRepetition);
}

#[test]
fn reclamacao_incorreta_da_dois_minutos_ao_adversario_e_a_partida_continua_art_9_5_3() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        90 * 60_000,
        0,
    );
    let t = fide_play_all(&mut game, 0, CYCLE); // só a 2ª ocorrência
    assert_eq!(game.snapshot().claimable_draw, None);
    let events = game.handle(Command::ClaimDraw, at(t)).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::DrawClaimRejected {
            by: Color::White,
            intended: None,
            penalty: chess_game::TimePenalty {
                to: Color::Black,
                ms: TWO_MINUTES_MS
            }
        }]
    ));
    assert_eq!(reason(&game), None);
    let r = game.clock_at(at(t)).unwrap();
    assert_eq!(r.running, Some(Color::White));
    // As pretas ganham 2 min, além do tempo que não gastaram.
    assert!(r.black_ms >= 90 * 60_000 - t + 120_000 - 10 && r.black_ms > 90 * 60_000);
    // E o jogo segue.
    fide_play(&mut game, t + 1, "Nf3");
}

#[test]
fn cinquenta_lances_reclamacao_correta_e_incorreta_art_9_3() {
    let mut game = fide_from("4k3/8/8/8/8/8/8/R3K3 w - - 100 80");
    assert_eq!(reason(&game), None, "FIDE: exige reclamação");
    assert_eq!(
        game.snapshot().claimable_draw,
        Some(DrawBasis::FiftyMoveRule)
    );
    game.handle(Command::ClaimDraw, at(1)).unwrap();
    assert_eq!(reason(&game), Some(EndReason::FiftyMoveRule));

    let mut game = fide_from("4k3/8/8/8/8/8/8/R3K3 w - - 99 80");
    game.handle(Command::ClaimDraw, at(1)).unwrap();
    assert_eq!(reason(&game), None, "99 meios-lances não bastam");
    assert_eq!(game.snapshot().infractions_white, 0, "não é Infração");
}

#[test]
fn reclamacao_com_lance_anunciado_cinquenta_lances_art_9_3_1() {
    // O Lance anunciado completa os 50 lances: a Reclamação é correta e o Lance
    // se completa com ela (art. 6.2.1.1).
    let mut game = fide_from("4k3/8/8/8/8/8/8/R3K3 w - - 99 80");
    let mv = "a1a2".parse().unwrap();
    let events = game
        .handle(Command::ClaimDrawWithMove { mv }, at(1))
        .unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::DrawClaimAccepted {
                basis: DrawBasis::FiftyMoveRule,
                intended: Some(_),
                ..
            },
            EventKind::MoveCompleted { .. },
            EventKind::GameEnded { .. }
        ]
    ));
    assert_eq!(game.snapshot().history[0].san, "Ra2");
    assert_eq!(reason(&game), Some(EndReason::FiftyMoveRule));
}

#[test]
fn reclamacao_com_lance_anunciado_tripla_art_9_2_1() {
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, "Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1");
    // Pretas anunciam Cg8, que produz a 3ª ocorrência da posição inicial.
    let mv = "f6g8".parse().unwrap();
    game.handle(Command::ClaimDrawWithMove { mv }, at(t))
        .unwrap();
    assert_eq!(reason(&game), Some(EndReason::ThreefoldRepetition));
    assert_eq!(game.snapshot().history.len(), 8);
}

#[test]
fn lance_anunciado_numa_reclamacao_incorreta_passa_a_ser_obrigatorio_art_9_5_3() {
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, "Nf3 Nf6 Ng1");
    let mv = "f6g8".parse().unwrap();
    // 2ª ocorrência apenas: incorreta.
    let events = game
        .handle(Command::ClaimDrawWithMove { mv }, at(t))
        .unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::DrawClaimRejected {
            intended: Some(_),
            ..
        }]
    ));
    assert_eq!(game.snapshot().must_move_from, vec![sq("f6")]);
    assert!(matches!(
        drop_piece(&mut game, t + 1, "e7", "e5"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, t + 2, "f6", "g8").unwrap();
}

#[test]
fn lance_anunciado_ilegal_e_recusado() {
    let mut game = fide();
    let mv = "e2e5".parse().unwrap();
    assert_eq!(
        game.handle(Command::ClaimDrawWithMove { mv }, at(1)),
        Err(Rejection::ClaimedMoveIllegal)
    );
}

#[test]
fn art_9_4_tocar_uma_peca_perde_o_direito_de_reclamar_nesta_vez() {
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, "Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1 Ng8");
    touch(&mut game, t, "e2").unwrap();
    assert_eq!(game.snapshot().claimable_draw, None);
    assert_eq!(
        game.handle(Command::ClaimDraw, at(t + 1)),
        Err(Rejection::DrawClaimRightLost)
    );
    // Ajuste (art. 4.2) não é toque do art. 4.3: o direito permanece.
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, "Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1 Ng8");
    game.handle(Command::Adjust { square: sq("e2") }, at(t))
        .unwrap();
    game.handle(Command::ClaimDraw, at(t + 1)).unwrap();
    assert_eq!(reason(&game), Some(EndReason::ThreefoldRepetition));
}

#[test]
fn cinco_repeticoes_encerram_automaticamente_art_9_6_1() {
    let mut game = fide();
    let t = fide_play_all(&mut game, 0, &[CYCLE; 3].join(" "));
    assert_eq!(reason(&game), None);
    // O último Lance completa a 5ª ocorrência: encerra ao soltar, sem relógio.
    let t = fide_play_all(&mut game, t, "Nf3 Nf6 Ng1");
    touch(&mut game, t, "f6").unwrap();
    let events = drop_piece(&mut game, t + 1, "f6", "g8").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::MoveMade { .. },
            EventKind::MoveCompleted { .. },
            EventKind::GameEnded { .. }
        ]
    ));
    assert_eq!(reason(&game), Some(EndReason::FivefoldRepetition));
}

#[test]
fn setenta_e_cinco_lances_encerram_automaticamente_art_9_6_2() {
    let mut game = fide_from("4k3/8/8/8/8/8/8/R3K3 w - - 149 100");
    touch(&mut game, 1, "a1").unwrap();
    drop_piece(&mut game, 2, "a1", "a2").unwrap();
    assert_eq!(reason(&game), Some(EndReason::SeventyFiveMoveRule));
}

#[test]
fn material_insuficiente_encerra_automaticamente_art_5_2_2() {
    let mut game = fide_from("4k3/8/8/8/8/8/3r4/4K3 w - - 0 1");
    touch(&mut game, 1, "e1").unwrap();
    drop_piece(&mut game, 2, "e1", "d2").unwrap(); // Rxd2: K x K
    assert_eq!(reason(&game), Some(EndReason::InsufficientMaterial));
}

#[test]
fn posicao_morta_por_bloqueio_encerra_automaticamente_art_5_2_2() {
    let mut game = fide_from("4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/3r4/4K3 w - - 0 1");
    touch(&mut game, 1, "e1").unwrap();
    drop_piece(&mut game, 2, "e1", "d2").unwrap(); // Kxd2: só reis e peões travados
    assert_eq!(reason(&game), Some(EndReason::DeadPosition));
}

#[test]
fn a_engine_reclama_pela_mesma_via_quando_o_empate_e_reclamavel() {
    // API para o B9: o snapshot diz se há empate reclamável (para o lado da vez) e a
    // Engine, brancas aqui, envia ClaimDraw na sua vez; o usuário joga de pretas.
    let mut game = fide();
    let mut t = 0;
    for (engine, human) in [("g1f3", "Nf6"), ("f3g1", "Ng8")] {
        game.handle(Command::EngineMove { uci: engine.into() }, at(t))
            .unwrap();
        t = fide_play(&mut game, t + 1, human);
    }
    assert_eq!(game.snapshot().claimable_draw, None, "só a 2ª ocorrência");
    for (engine, human) in [("g1f3", "Nf6"), ("f3g1", "Ng8")] {
        game.handle(Command::EngineMove { uci: engine.into() }, at(t))
            .unwrap();
        t = fide_play(&mut game, t + 1, human);
    }
    assert_eq!(
        game.snapshot().claimable_draw,
        Some(DrawBasis::ThreefoldRepetition)
    );
    game.handle(Command::ClaimDraw, at(t)).unwrap();
    assert_eq!(reason(&game), Some(EndReason::ThreefoldRepetition));
}

#[test]
fn reclamacao_depois_do_fim_e_recusada() {
    let mut game = fide();
    game.handle(Command::Resign { side: Color::White }, at(1))
        .unwrap();
    assert_eq!(
        game.handle(Command::ClaimDraw, at(2)),
        Err(Rejection::GameOver)
    );
}

#[test]
fn reclamacao_nao_existe_no_assistido() {
    let mut game = assisted();
    assert!(matches!(
        game.handle(Command::ClaimDraw, at(1)),
        Err(Rejection::NotSupportedByPolicy(_))
    ));
}
