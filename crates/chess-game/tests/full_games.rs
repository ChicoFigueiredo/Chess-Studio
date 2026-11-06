//! CORE-01: partidas completas por Comandos, até mate e até afogamento.

mod common;
use chess_core::Color;
use chess_game::{Command, EndReason, EventKind, GameResult, Rejection};
use common::*;

fn assert_ended(game: &chess_game::Game, result: GameResult, reason: EndReason) {
    let outcome = game.snapshot().outcome.expect("Partida encerrada");
    assert_eq!((outcome.result, outcome.reason), (result, reason));
}

#[test]
fn mate_do_pastor_encerra_com_vitoria_das_brancas() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5 Qh5 Nc6 Bc4 Nf6");
    assert!(game.snapshot().outcome.is_none());

    let events = play_san(&mut game, &mut clock, "Qxf7#");

    assert!(matches!(
        kinds(&events).as_slice(),
        [
            EventKind::MoveCompleted { .. },
            EventKind::GameEnded { .. }
        ]
    ));
    assert_ended(&game, GameResult::WhiteWins, EndReason::Checkmate);
    assert_eq!(game.snapshot().history.len(), 7);
    assert_eq!(game.snapshot().history[6].san, "Qxf7#");
}

#[test]
fn mate_do_louco_encerra_com_vitoria_das_pretas() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "f3 e5 g4 Qh4#");
    assert_ended(&game, GameResult::BlackWins, EndReason::Checkmate);
}

#[test]
fn partida_real_da_opera_ate_o_mate_com_roque_longo_e_xeques() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(
        &mut game,
        &mut clock,
        "e4 e5 Nf3 d6 d4 Bg4 dxe5 Bxf3 Qxf3 dxe5 Bc4 Nf6 Qb3 Qe7 Nc3 c6 Bg5 b5 \
         Nxb5 cxb5 Bxb5+ Nbd7 O-O-O Rd8 Rxd7 Rxd7 Rd1 Qe6 Bxd7+ Nxd7 Qb8+ Nxb8 Rd8#",
    );
    assert_ended(&game, GameResult::WhiteWins, EndReason::Checkmate);
    assert_eq!(game.snapshot().history.len(), 33);
}

#[test]
fn afogamento_empata_e_nao_e_mate() {
    let mut game = assisted_from("7k/8/6K1/8/8/8/8/5Q2 w - - 0 1");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Qf7");
    assert_ended(&game, GameResult::Draw, EndReason::Stalemate);
}

#[test]
fn comandos_apos_o_fim_sao_rejeitados_sem_mudar_a_partida() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "f3 e5 g4 Qh4#");
    let before = game.snapshot();
    let events_before = game.events().len();

    let result = game.handle(
        Command::Release {
            from: sq("a2"),
            to: sq("a3"),
        },
        clock.now(),
    );

    assert_eq!(result, Err(Rejection::GameOver));
    assert_eq!(game.snapshot(), before);
    assert_eq!(game.events().len(), events_before);
}

#[test]
fn abandono_encerra_com_vitoria_do_adversario() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "e4");
    game.handle(Command::Resign { side: Color::Black }, clock.now())
        .expect("abandono aceito");
    assert_ended(&game, GameResult::WhiteWins, EndReason::Resignation);
}

#[test]
fn promocao_pendente_so_conclui_com_a_peca_escolhida() {
    let mut game = assisted_from("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    let mut clock = FakeClock::new();

    let pending = release(&mut game, &mut clock, "a7", "a8");
    assert!(matches!(
        kinds(&pending).as_slice(),
        [EventKind::PromotionPending { .. }]
    ));
    let snap = game.snapshot();
    assert!(snap.history.is_empty(), "o lance ainda não foi concluído");
    assert_eq!(snap.position.to_fen(), "8/P6k/8/8/8/8/8/K7 w - - 0 1");
    let view = snap.provisional.expect("Posição visual provisória");
    assert!(
        view.pieces
            .iter()
            .any(|(s, c, p)| *s == sq("a8") && *c == Color::White && *p == chess_core::Piece::Pawn)
    );
    assert!(view.pieces.iter().all(|(s, _, _)| *s != sq("a7")));

    // Enquanto pendente, outro Release é recusado.
    let again = game.handle(
        Command::Release {
            from: sq("a1"),
            to: sq("a2"),
        },
        clock.now(),
    );
    assert_eq!(again, Err(Rejection::PromotionPending));

    choose_promotion(&mut game, &mut clock, chess_core::Piece::Queen).unwrap();
    let snap = game.snapshot();
    assert_eq!(snap.history[0].san, "a8=Q");
    assert!(snap.provisional.is_none());
}

#[test]
fn promocao_invalida_e_sem_pendencia_sao_rejeitadas() {
    let mut game = assisted_from("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    let mut clock = FakeClock::new();
    assert_eq!(
        choose_promotion(&mut game, &mut clock, chess_core::Piece::Queen),
        Err(Rejection::NoPromotionPending)
    );
    release(&mut game, &mut clock, "a7", "a8");
    assert_eq!(
        choose_promotion(&mut game, &mut clock, chess_core::Piece::King),
        Err(Rejection::InvalidPromotionPiece(chess_core::Piece::King))
    );
}

#[test]
fn promocao_com_captura_e_subpromocao_a_cavalo() {
    let mut game = assisted_from("1r5k/P7/8/8/8/8/8/K7 w - - 0 1");
    let mut clock = FakeClock::new();
    release(&mut game, &mut clock, "a7", "b8");
    choose_promotion(&mut game, &mut clock, chess_core::Piece::Knight).unwrap();
    assert_eq!(game.snapshot().history[0].san, "axb8=N");
}

#[test]
fn retratar_cancela_a_promocao_pendente() {
    let mut game = assisted_from("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    let mut clock = FakeClock::new();
    release(&mut game, &mut clock, "a7", "a8");
    let events = game.handle(Command::RetractMove, clock.now()).unwrap();
    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::PromotionCancelled { .. }]
    ));
    let snap = game.snapshot();
    assert!(snap.provisional.is_none());
    assert_eq!(snap.position.to_fen(), "8/P6k/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(
        game.handle(Command::RetractMove, clock.now()),
        Err(Rejection::NothingToRetract)
    );
}

#[test]
fn tempo_que_volta_atras_e_rejeitado() {
    let mut game = assisted();
    game.handle(
        Command::Release {
            from: sq("e2"),
            to: sq("e4"),
        },
        chess_game::Timestamp::mono(5000),
    )
    .unwrap();
    let result = game.handle(
        Command::Release {
            from: sq("e7"),
            to: sq("e5"),
        },
        chess_game::Timestamp::mono(4000),
    );
    assert!(matches!(result, Err(Rejection::TimeWentBackwards { .. })));
}
