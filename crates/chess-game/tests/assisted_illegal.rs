//! AC-03 / UX-04 / BDD-01 (domínio) / DB-04: o Assistido recusa o Lance ilegal sem
//! penalidade; o ilegal nunca entra no histórico legal.

mod common;
use chess_core::Color;
use chess_game::{Command, EventKind, IllegalReason, Rejection};
use common::*;

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[test]
fn e2_e5_e_recusado_sem_virar_e4() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    let before = game.snapshot();

    let events = release(&mut game, &mut clock, "e2", "e5");

    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::IllegalMoveRejected {
            side: Color::White,
            reason: IllegalReason::NotLegal { in_check: false },
            ..
        }]
    ));
    let after = game.snapshot();
    assert_eq!(after, before, "Posição e histórico inalterados");
    assert_eq!(after.position.to_fen(), START_FEN);
    assert!(after.history.is_empty());
    assert_eq!(
        after.side_to_move,
        Color::White,
        "sem penalidade: a vez é a mesma"
    );
}

#[test]
fn o_evento_de_lance_ilegal_fica_no_log_e_distingue_do_legal() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    release(&mut game, &mut clock, "e2", "e5");
    release(&mut game, &mut clock, "e2", "e4");

    let log = kinds(game.events());
    assert!(matches!(log[0], EventKind::IllegalMoveRejected { .. }));
    assert!(matches!(log[1], EventKind::MoveCompleted { .. }));
    assert_eq!(game.snapshot().history.len(), 1);
}

#[test]
fn motivos_de_recusa_sao_explicaveis() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    let reason =
        |game: &mut chess_game::Game, clock: &mut FakeClock, f: &str, t: &str| match &release(
            game, clock, f, t,
        )[0]
        .kind
        {
            EventKind::IllegalMoveRejected { reason, .. } => *reason,
            other => panic!("esperava lance ilegal, veio {other:?}"),
        };
    assert_eq!(
        reason(&mut game, &mut clock, "e4", "e5"),
        IllegalReason::EmptyOrigin
    );
    assert_eq!(
        reason(&mut game, &mut clock, "e7", "e5"),
        IllegalReason::OpponentPiece
    );
    assert_eq!(
        reason(&mut game, &mut clock, "e2", "e2"),
        IllegalReason::SameSquare
    );
    assert_eq!(
        reason(&mut game, &mut clock, "b1", "b3"),
        IllegalReason::NotLegal { in_check: false }
    );
}

#[test]
fn em_xeque_um_lance_que_nao_resolve_e_recusado_e_informa_o_xeque() {
    // Rei branco em e1 em xeque da torre e8; a4 do peão não resolve.
    let mut game = assisted_from("4r2k/8/8/8/8/8/P7/4K3 w - - 0 1");
    let mut clock = FakeClock::new();
    let events = release(&mut game, &mut clock, "a2", "a3");
    assert!(matches!(
        &events[0].kind,
        EventKind::IllegalMoveRejected {
            reason: IllegalReason::NotLegal { in_check: true },
            ..
        }
    ));
    assert!(game.snapshot().in_check);
}

#[test]
fn rei_nao_pode_entrar_em_casa_atacada() {
    let mut game = assisted_from("7k/8/8/8/8/8/r7/4K3 w - - 0 1");
    let mut clock = FakeClock::new();
    let events = release(&mut game, &mut clock, "e1", "e2");
    assert!(matches!(
        &events[0].kind,
        EventKind::IllegalMoveRejected { .. }
    ));
    assert!(game.snapshot().history.is_empty());
}

#[test]
fn roque_atravessando_casa_atacada_e_ilegal() {
    let mut game = assisted_from("5r2/6k1/8/8/8/8/8/4K2R w K - 0 1");
    let mut clock = FakeClock::new();
    let events = release(&mut game, &mut clock, "e1", "g1");
    assert!(matches!(
        &events[0].kind,
        EventKind::IllegalMoveRejected { .. }
    ));
}

#[test]
fn destinos_legais_para_auxilios_nao_incluem_ilegais() {
    let game = assisted();
    let dests = game.snapshot().legal_destinations(sq("e2"));
    assert_eq!(dests, vec![sq("e3"), sq("e4")]);
    assert!(game.snapshot().legal_destinations(sq("e7")).is_empty());
    assert!(game.snapshot().legal_destinations(sq("e4")).is_empty());
}

#[test]
fn toque_ajuste_relogio_e_reclamacao_nao_existem_no_assistido() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    for command in [
        Command::Touch { square: sq("e2") },
        Command::Adjust { square: sq("e2") },
        Command::PressClock,
        Command::ClaimDraw,
    ] {
        let kind = command.kind();
        assert_eq!(
            game.handle(command, clock.now()),
            Err(Rejection::NotSupportedByPolicy(kind))
        );
    }
    assert!(game.events().is_empty());
}

#[test]
fn pausa_e_engine_ainda_nao_implementadas_sao_rejeicoes_tipadas() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    for command in [
        Command::Pause,
        Command::Resume,
        Command::EngineMove { uci: "e2e4".into() },
    ] {
        let kind = command.kind();
        assert_eq!(
            game.handle(command, clock.now()),
            Err(Rejection::NotYetImplemented(kind))
        );
    }
}

#[test]
fn controle_de_tempo_e_aceito_e_ignorado() {
    let setup = chess_game::GameSetup::standard(chess_game::PolicyKind::Assisted)
        .with_time_control(chess_game::TimeControl {
            base_ms: 60_000,
            increment_ms: 1000,
        });
    let mut game = chess_game::Game::new(setup);
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    assert_eq!(game.snapshot().history.len(), 2);
    assert!(game.snapshot().outcome.is_none());
}
