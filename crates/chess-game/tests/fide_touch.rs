#![allow(clippy::unwrap_used)]
//! Simulação FIDE: Toque, Ajuste e Obrigação de peça tocada (arts. 4.2 a 4.7),
//! separação entre mover, soltar e concluir (AC-04) e fim sem acionar o relógio
//! (art. 6.2.1.1). RULE-02 e RULE-03 / BDD-08.

mod common;
use chess_core::{Color, Piece};
use chess_game::{Command, EndReason, EventKind, GameResult, IllegalReason, Rejection};
use common::*;

fn kinds_of(events: &[chess_game::Event]) -> Vec<EventKind> {
    events.iter().map(|e| e.kind.clone()).collect()
}

const CASTLING: &str = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1";

#[test]
fn rule_02_selecao_nao_e_toque_so_touch_cria_obrigacao() {
    let mut game = fide();
    // A "seleção" da UI não chega ao domínio: sem Touch, não há obrigação.
    assert!(game.snapshot().touched.is_empty());
    assert!(game.snapshot().must_move_from.is_empty());
    // Tocar uma casa vazia não conta.
    assert_eq!(
        touch(&mut game, 1, "e4"),
        Err(Rejection::NoPieceAt(sq("e4")))
    );
    assert!(game.snapshot().touched.is_empty());
    // Sem toque, qualquer peça legal pode ser jogada.
    drop_piece(&mut game, 2, "d2", "d4").unwrap();
    assert!(game.snapshot().made_move.is_some());
}

#[test]
fn touch_cria_a_obrigacao_e_ajuste_nao_art_4_2() {
    let mut game = fide();
    let events = game
        .handle(Command::Adjust { square: sq("e2") }, at(1))
        .unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::PieceAdjusted { .. }]
    ));
    assert!(game.snapshot().must_move_from.is_empty());
    drop_piece(&mut game, 2, "d2", "d4").unwrap(); // livre

    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e2")]);
}

#[test]
fn bdd_08_obrigacao_persiste_apos_cancelar_o_gesto() {
    // Esc/perda de foco cancelam o arrasto na UI: o domínio só viu o Touch.
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    // O próximo Release de outra peça é recusado: há Lance legal com a tocada.
    assert_eq!(
        drop_piece(&mut game, 5, "d2", "d4"),
        Err(Rejection::TouchedPieceObligation {
            must_move_from: vec![sq("e2")]
        })
    );
    assert!(game.snapshot().made_move.is_none());
    assert!(game.snapshot().history.is_empty());
    // Mover a tocada é aceito.
    drop_piece(&mut game, 6, "e2", "e4").unwrap();
}

#[test]
fn peca_tocada_sem_lance_legal_libera_qualquer_lance_art_4_5() {
    let mut game = fide();
    touch(&mut game, 1, "a1").unwrap(); // torre bloqueada
    assert!(game.snapshot().must_move_from.is_empty());
    drop_piece(&mut game, 2, "d2", "d4").unwrap();
}

#[test]
fn a_obrigacao_vale_so_na_vez_e_o_lance_concluido_a_limpa() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    let t = fide_play(&mut game, 2, "e4");
    assert!(game.snapshot().touched.is_empty());
    // Agora são as pretas: nada herdado das brancas.
    drop_piece(&mut game, t, "d7", "d5").unwrap();
}

#[test]
fn toque_em_peca_do_adversario_obriga_a_captura_art_4_3_2() {
    let mut game = fide_from("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1");
    touch(&mut game, 1, "d5").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e4")]);
    // Um lance legal qualquer (e4-e5) não cumpre a captura.
    assert!(matches!(
        drop_piece(&mut game, 2, "e4", "e5"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    assert!(matches!(
        drop_piece(&mut game, 2, "e1", "e2"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 3, "e4", "d5").unwrap();
    press(&mut game, 4).unwrap();
    assert_eq!(game.snapshot().history[0].san, "exd5");
}

#[test]
fn peca_do_adversario_que_nao_pode_ser_capturada_nao_obriga_art_4_5() {
    let mut game = fide();
    touch(&mut game, 1, "e7").unwrap();
    assert!(game.snapshot().must_move_from.is_empty());
    drop_piece(&mut game, 2, "e2", "e4").unwrap();
}

#[test]
fn pecas_das_duas_cores_capturar_com_a_propria_art_4_3_3() {
    let mut game = fide_from("4k3/8/8/3p4/4P3/8/8/R3K3 w - - 0 1");
    touch(&mut game, 1, "d5").unwrap();
    touch(&mut game, 2, "a1").unwrap(); // torre não captura d5
    // A primeira tocada que pode ser capturada é d5, pela peça que puder.
    assert_eq!(game.snapshot().must_move_from, vec![sq("e4")]);
    assert!(matches!(
        drop_piece(&mut game, 3, "a1", "a2"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 4, "e4", "d5").unwrap();

    // Se a própria tocada captura a do adversário, é ela que deve capturar.
    let mut game = fide_from("4k3/8/8/3p4/4P3/8/8/R3K3 w - - 0 1");
    touch(&mut game, 1, "e4").unwrap();
    touch(&mut game, 2, "d5").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e4")]);
}

#[test]
fn captura_en_passant_da_peca_do_adversario_tocada_conta() {
    let mut game = fide_from("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2");
    touch(&mut game, 1, "d5").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e5")]);
    assert!(matches!(
        drop_piece(&mut game, 2, "e5", "e6"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 3, "e5", "d6").unwrap();
}

#[test]
fn roque_tocando_o_rei_e_depois_a_torre_obriga_a_rocar_art_4_4_1() {
    let mut game = fide_from(CASTLING);
    touch(&mut game, 1, "e1").unwrap();
    touch(&mut game, 2, "h1").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e1")]);
    assert!(matches!(
        drop_piece(&mut game, 3, "e1", "d1"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 4, "e1", "g1").unwrap();
    press(&mut game, 5).unwrap();
    assert_eq!(game.snapshot().history[0].san, "O-O");
}

#[test]
fn roque_tocando_a_torre_primeiro_nao_pode_rocar_com_ela_art_4_4_2() {
    let mut game = fide_from(CASTLING);
    touch(&mut game, 1, "h1").unwrap();
    touch(&mut game, 2, "e1").unwrap();
    // Vale o art. 4.3.1: a torre, tocada primeiro, é que deve se mover.
    assert_eq!(game.snapshot().must_move_from, vec![sq("h1")]);
    assert!(matches!(
        drop_piece(&mut game, 3, "e1", "g1"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 4, "h1", "g1").unwrap();
}

#[test]
fn roque_ilegal_com_rei_e_torre_tocados_exige_outro_lance_do_rei_art_4_4_3() {
    // A torre preta em f8 ataca f1: o roque curto é ilegal; o longo, legal.
    let mut game = fide_from("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    touch(&mut game, 1, "e1").unwrap();
    touch(&mut game, 2, "h1").unwrap();
    assert_eq!(game.snapshot().must_move_from, vec![sq("e1")]);
    assert!(matches!(
        drop_piece(&mut game, 3, "h1", "g1"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    // Outro lance do rei, inclusive o roque com a outra torre, é aceito.
    drop_piece(&mut game, 4, "e1", "c1").unwrap();
}

#[test]
fn lance_feito_nao_pode_ser_retraido_nem_trocado_art_4_7() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    drop_piece(&mut game, 2, "e2", "e4").unwrap();
    assert_eq!(
        game.handle(Command::RetractMove, at(3)),
        Err(Rejection::MoveAlreadyMade)
    );
    assert_eq!(
        drop_piece(&mut game, 4, "e4", "e5"),
        Err(Rejection::MoveAlreadyMade)
    );
    assert_eq!(touch(&mut game, 5, "d2"), Err(Rejection::MoveAlreadyMade));
}

#[test]
fn ac_04_mover_soltar_e_concluir_sao_etapas_separadas() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        60_000,
        0,
    );
    touch(&mut game, 1_000, "e2").unwrap();
    let events = drop_piece(&mut game, 2_000, "e2", "e4").unwrap();
    // Lance feito: ainda não é concluído, a Posição canônica é a anterior.
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::MoveMade { .. }]
    ));
    let snap = game.snapshot();
    assert!(snap.history.is_empty());
    assert_eq!(snap.side_to_move, Color::White);
    assert_eq!(snap.made_move.as_ref().unwrap().san, "e4");
    // O relógio das brancas segue correndo até serem acionados.
    assert_eq!(game.clock_at(at(5_000)).unwrap().white_ms, 55_000);
    assert_eq!(snap.clock.unwrap().running, Some(Color::White));

    let events = press(&mut game, 5_000).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::MoveCompleted { .. }]
    ));
    let snap = game.snapshot();
    assert_eq!(snap.history.len(), 1);
    assert_eq!(snap.side_to_move, Color::Black);
    assert!(snap.made_move.is_none());
    assert_eq!(snap.clock.unwrap().running, Some(Color::Black));
}

#[test]
fn peca_solta_em_casa_ilegal_fica_provisoria_e_a_posicao_canonica_nao_muda() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    let events = drop_piece(&mut game, 2, "e2", "e5").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::IllegalMoveReleased {
            reason: IllegalReason::NotLegal { in_check: false },
            ..
        }]
    ));
    let snap = game.snapshot();
    assert_eq!(snap.position, chess_core::CanonicalPosition::start());
    let view = snap.provisional_release.unwrap();
    assert!(view.pieces.contains(&(sq("e5"), Color::White, Piece::Pawn)));
    assert!(!view.pieces.iter().any(|(s, ..)| *s == sq("e2")));
    assert!(snap.history.is_empty());
    // Nada mais se faz com a peça solta: retratar ou acionar o relógio.
    assert_eq!(
        drop_piece(&mut game, 3, "d2", "d4"),
        Err(Rejection::ProvisionalReleasePending)
    );
    assert_eq!(
        touch(&mut game, 3, "d2"),
        Err(Rejection::ProvisionalReleasePending)
    );
}

#[test]
fn retratar_devolve_a_peca_e_mantem_a_obrigacao_rule_03() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    drop_piece(&mut game, 2, "e2", "e5").unwrap();
    let events = game.handle(Command::RetractMove, at(3)).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::ReleaseRetracted { .. }]
    ));
    let snap = game.snapshot();
    assert!(snap.provisional_release.is_none());
    assert_eq!(snap.must_move_from, vec![sq("e2")]);
    assert!(matches!(
        drop_piece(&mut game, 4, "d2", "d4"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 5, "e2", "e4").unwrap();
    assert_eq!(
        game.handle(Command::RetractMove, at(6)),
        Err(Rejection::MoveAlreadyMade)
    );
}

#[test]
fn nada_a_retratar_sem_peca_solta() {
    let mut game = fide();
    assert_eq!(
        game.handle(Command::RetractMove, at(1)),
        Err(Rejection::NothingToRetract)
    );
}

#[test]
fn lance_que_deixa_o_rei_em_xeque_e_roque_ilegal_ficam_provisorios() {
    // Rei em xeque da torre em e8; mover a torre branca não resolve.
    let mut game = fide_from("4r2k/8/8/8/8/8/R7/4K3 w - - 0 1");
    touch(&mut game, 1, "a2").unwrap();
    let events = drop_piece(&mut game, 2, "a2", "a3").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::IllegalMoveReleased {
            reason: IllegalReason::NotLegal { in_check: true },
            ..
        }]
    ));
    // Roque através de casa atacada.
    let mut game = fide_from("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    touch(&mut game, 1, "e1").unwrap();
    let events = drop_piece(&mut game, 2, "e1", "g1").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::IllegalMoveReleased { .. }]
    ));
}

#[test]
fn promocao_exige_escolha_e_so_entao_o_lance_esta_feito() {
    let mut game = fide_from("8/P6k/8/8/8/8/6r1/K7 w - - 0 1");
    touch(&mut game, 1, "a7").unwrap();
    let events = drop_piece(&mut game, 2, "a7", "a8").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::PromotionPending { .. }]
    ));
    assert!(game.snapshot().provisional.is_some());
    // Nas Políticas competitivas não há dama automática (DEC-40).
    assert!(game.snapshot().made_move.is_none());
    game.handle(
        Command::ChoosePromotion {
            piece: Piece::Knight,
        },
        at(3),
    )
    .unwrap();
    assert_eq!(game.snapshot().made_move.unwrap().san, "a8=N");
    press(&mut game, 4).unwrap();
    assert_eq!(game.snapshot().history[0].san, "a8=N");
}

#[test]
fn promocao_pendente_pode_ser_retratada_e_mantem_a_obrigacao() {
    let mut game = fide_from("8/P6k/8/8/8/8/6r1/K7 w - - 0 1");
    touch(&mut game, 1, "a7").unwrap();
    drop_piece(&mut game, 2, "a7", "a8").unwrap();
    game.handle(Command::RetractMove, at(3)).unwrap();
    assert!(game.snapshot().provisional.is_none());
    assert_eq!(game.snapshot().must_move_from, vec![sq("a7")]);
}

#[test]
fn mate_encerra_sem_acionar_o_relogio_art_6_2_1_1() {
    let mut game = fide();
    fide_play_all(&mut game, 0, "f3 e5 g4");
    touch(&mut game, 100, "d8").unwrap();
    let events = drop_piece(&mut game, 101, "d8", "h4").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::MoveMade { .. },
            EventKind::MoveCompleted { .. },
            EventKind::GameEnded { .. }
        ]
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::Checkmate);
    assert_eq!(game.snapshot().history.len(), 4);
    assert_eq!(press(&mut game, 102), Err(Rejection::GameOver));
}

#[test]
fn afogamento_encerra_sem_acionar_o_relogio() {
    let mut game = fide_from("7k/8/5K2/6Q1/8/8/8/8 w - - 0 1");
    touch(&mut game, 1, "g5").unwrap();
    drop_piece(&mut game, 2, "g5", "g6").unwrap();
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::Stalemate);
}

#[test]
fn release_sem_touch_previo_conta_como_toque_implicito_da_peca_solta() {
    let mut game = fide();
    let events = drop_piece(&mut game, 1, "e2", "e4").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::PieceTouched { implicit: true, .. },
            EventKind::MoveMade { .. }
        ]
    ));
    // Sem peça na origem não há gesto.
    let mut game = fide();
    assert_eq!(
        drop_piece(&mut game, 1, "e4", "e5"),
        Err(Rejection::NoPieceAt(sq("e4")))
    );
}

#[test]
fn desfazer_e_refazer_nao_existem_nas_politicas_competitivas() {
    let mut game = fide();
    fide_play(&mut game, 0, "e4");
    assert!(!game.snapshot().can_undo && !game.snapshot().can_redo);
    for command in [Command::Undo, Command::Redo] {
        let kind = command.kind();
        assert_eq!(
            game.handle(command, at(100)),
            Err(Rejection::NotSupportedByPolicy(kind))
        );
    }
}

#[test]
fn rejeicao_nao_muda_a_partida() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    let before = (game.snapshot(), game.events().len());
    let _ = drop_piece(&mut game, 2, "d2", "d4");
    assert_eq!((game.snapshot(), game.events().len()), before);
}
