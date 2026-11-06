#![allow(clippy::unwrap_used)]
//! DEC-39: desfazer/refazer no Assistido, registrados, marcando a Partida "com desfazer".

mod common;
use chess_game::{Command, EndReason, EventKind, Rejection};
use common::*;

#[test]
fn desfazer_volta_a_posicao_registra_evento_e_marca_a_partida() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    let start = game.snapshot().position.to_fen();
    play_san(&mut game, &mut clock, "e4");
    assert!(!game.snapshot().with_undo);

    let events = game.handle(Command::Undo, clock.now()).unwrap();

    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::MoveUndone { .. }]
    ));
    let snap = game.snapshot();
    assert_eq!(snap.position.to_fen(), start);
    assert!(snap.history.is_empty());
    assert!(snap.with_undo);
    assert!(snap.can_redo && !snap.can_undo);
}

#[test]
fn refazer_reaplica_o_lance_desfeito() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    let before = game.snapshot().position.to_fen();
    game.handle(Command::Undo, clock.now()).unwrap();
    game.handle(Command::Undo, clock.now()).unwrap();

    let events = game.handle(Command::Redo, clock.now()).unwrap();
    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::MoveRedone { .. }]
    ));
    game.handle(Command::Redo, clock.now()).unwrap();

    assert_eq!(game.snapshot().position.to_fen(), before);
    assert_eq!(game.snapshot().history.len(), 2);
    assert!(game.snapshot().with_undo, "a marca permanece");
}

#[test]
fn novo_lance_apaga_a_pilha_de_refazer() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    game.handle(Command::Undo, clock.now()).unwrap();
    play_san(&mut game, &mut clock, "c5");
    assert!(!game.snapshot().can_redo);
    assert_eq!(
        game.handle(Command::Redo, clock.now()),
        Err(Rejection::NothingToRedo)
    );
}

#[test]
fn desfazer_sem_lances_e_rejeitado() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    assert_eq!(
        game.handle(Command::Undo, clock.now()),
        Err(Rejection::NothingToUndo)
    );
}

#[test]
fn desfazer_reabre_partida_encerrada_por_mate_e_refazer_encerra_de_novo() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "f3 e5 g4 Qh4#");
    assert!(game.snapshot().outcome.is_some());

    game.handle(Command::Undo, clock.now()).unwrap();
    assert!(game.snapshot().outcome.is_none());

    let events = game.handle(Command::Redo, clock.now()).unwrap();
    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::MoveRedone { .. }, EventKind::GameEnded { .. }]
    ));
    assert_eq!(
        game.snapshot().outcome.unwrap().reason,
        EndReason::Checkmate
    );
}

#[test]
fn desfazer_nao_reabre_abandono() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "e4");
    game.handle(
        Command::Resign {
            side: chess_core::Color::Black,
        },
        clock.now(),
    )
    .unwrap();
    assert_eq!(
        game.handle(Command::Undo, clock.now()),
        Err(Rejection::GameOver)
    );
}

#[test]
fn desfazer_a_tripla_repeticao_libera_a_posicao_para_continuar() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1 Ng8");
    assert!(game.snapshot().outcome.is_some());
    game.handle(Command::Undo, clock.now()).unwrap();
    assert!(game.snapshot().outcome.is_none());
    play_san(&mut game, &mut clock, "d5");
    assert!(game.snapshot().outcome.is_none());
}

#[test]
fn desfazer_com_promocao_pendente_exige_cancelar_antes() {
    let mut game = assisted_from("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Kb1");
    play_san(&mut game, &mut clock, "Kg7");
    release(&mut game, &mut clock, "a7", "a8");
    assert_eq!(
        game.handle(Command::Undo, clock.now()),
        Err(Rejection::PromotionPending)
    );
}
