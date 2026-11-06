#![allow(clippy::unwrap_used)]
//! Os Eventos viajam como JSON versionado (o B5 grava no SQLite).

mod common;
use chess_core::Piece;
use chess_core::{Color, Move};
use chess_game::{
    DrawBasis, EndReason, Event, EventKind, GameResult, GameSetup, IllegalReason, InfractionCause,
    InfractionStage, Outcome, PAYLOAD_VERSION, PolicyKind, TimeControl, TimePenalty, Timestamp,
};
use common::sq;

fn mv(uci: &str) -> Move {
    uci.parse().unwrap()
}

/// Uma instância de cada variante de `EventKind`.
fn every_kind() -> Vec<EventKind> {
    vec![
        EventKind::MoveCompleted {
            side: Color::White,
            mv: mv("e7e8q"),
            san: "e8=Q+".into(),
        },
        EventKind::IllegalMoveRejected {
            side: Color::Black,
            from: sq("e7"),
            to: sq("e4"),
            reason: IllegalReason::NotLegal { in_check: true },
        },
        EventKind::IllegalMoveRejected {
            side: Color::Black,
            from: sq("e5"),
            to: sq("e4"),
            reason: IllegalReason::EmptyOrigin,
        },
        EventKind::PromotionPending {
            side: Color::White,
            from: sq("a7"),
            to: sq("a8"),
        },
        EventKind::PromotionCancelled { side: Color::White },
        EventKind::DrawOffered { by: Color::White },
        EventKind::DrawDeclined { by: Color::Black },
        EventKind::Resigned { side: Color::Black },
        EventKind::MoveUndone {
            side: Color::White,
            mv: mv("e2e4"),
        },
        EventKind::MoveRedone {
            side: Color::White,
            mv: mv("e2e4"),
            san: "e4".into(),
        },
        EventKind::GameEnded {
            outcome: Outcome {
                result: GameResult::Draw,
                reason: EndReason::ThreefoldRepetition,
            },
        },
        EventKind::PieceTouched {
            side: Color::White,
            square: sq("d5"),
            owner: Color::Black,
            piece: Piece::Knight,
            implicit: false,
        },
        EventKind::PieceAdjusted {
            side: Color::White,
            square: sq("e2"),
        },
        EventKind::MoveMade {
            side: Color::White,
            mv: mv("e2e4"),
            san: "e4".into(),
        },
        EventKind::IllegalMoveReleased {
            side: Color::White,
            from: sq("e2"),
            to: sq("e5"),
            reason: IllegalReason::NotLegal { in_check: false },
        },
        EventKind::ReleaseRetracted { side: Color::White },
        EventKind::InfractionRecorded {
            side: Color::White,
            cause: InfractionCause::IllegalMove {
                from: sq("e2"),
                to: sq("e5"),
                reason: IllegalReason::SameSquare,
            },
            count: 1,
            stage: InfractionStage::Clock,
            penalty: Some(TimePenalty {
                to: Color::Black,
                ms: 120_000,
            }),
        },
        EventKind::InfractionRecorded {
            side: Color::Black,
            cause: InfractionCause::ClockPressedWithoutMove,
            count: 2,
            stage: InfractionStage::Release,
            penalty: None,
        },
        EventKind::InfractionRecorded {
            side: Color::White,
            cause: InfractionCause::PromotionNotCompleted {
                from: sq("a7"),
                to: sq("a8"),
            },
            count: 1,
            stage: InfractionStage::Clock,
            penalty: None,
        },
        EventKind::Paused,
        EventKind::Resumed,
        EventKind::FlagFell {
            side: Color::Black,
            flag_at_ms: 61_000,
        },
        EventKind::DrawClaimAccepted {
            by: Color::White,
            basis: DrawBasis::FiftyMoveRule,
            intended: Some(mv("a1a2")),
        },
        EventKind::DrawClaimRejected {
            by: Color::Black,
            intended: None,
            penalty: TimePenalty {
                to: Color::White,
                ms: 120_000,
            },
        },
        EventKind::EngineMoveRejected {
            side: Color::Black,
            uci: "e7e9".into(),
        },
    ]
}

#[test]
fn cada_variante_de_evento_faz_ida_e_volta_em_json() {
    for kind in every_kind() {
        let event = Event::new(Timestamp::with_wall(1500, 1_762_000_000_000), kind);
        let json = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event, "{json}");
    }
}

#[test]
fn o_payload_tem_versao_e_tipo_estaveis() {
    let event = Event::new(
        Timestamp::mono(42),
        EventKind::MoveCompleted {
            side: Color::White,
            mv: mv("e2e4"),
            san: "e4".into(),
        },
    );
    let value: serde_json::Value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["version"], PAYLOAD_VERSION);
    assert_eq!(value["at"]["mono_ms"], 42);
    assert_eq!(value["kind"]["type"], "move_completed");
    assert_eq!(value["kind"]["mv"], "e2e4");
    assert_eq!(value["kind"]["side"], "white");
}

#[test]
fn evento_com_lance_ou_casa_invalidos_nao_deserializa() {
    let bad = r#"{"version":1,"at":{"mono_ms":1},"kind":{"type":"move_completed","side":"white","mv":"zz99","san":"?"}}"#;
    assert!(serde_json::from_str::<Event>(bad).is_err());
}

#[test]
fn versao_de_payload_desconhecida_e_detectada_no_replay() {
    let mut event = Event::new(
        Timestamp::mono(1),
        EventKind::Resigned { side: Color::White },
    );
    event.version = PAYLOAD_VERSION + 1;
    let setup = GameSetup::standard(PolicyKind::Assisted);
    let err = chess_game::Game::try_replay(setup, &[event]).unwrap_err();
    assert_eq!(err.index, 0);
}

#[test]
fn setup_faz_ida_e_volta_com_fen_e_controle_de_tempo() {
    let setup = GameSetup::from_fen(PolicyKind::Assisted, "8/P6k/8/8/8/8/8/K7 w - - 0 1")
        .unwrap()
        .with_time_control(TimeControl {
            base_ms: 300_000,
            increment_ms: 2000,
        });
    let json = serde_json::to_string(&setup).unwrap();
    let back: GameSetup = serde_json::from_str(&json).unwrap();
    assert_eq!(back, setup);
    assert!(json.contains("start_fen"));
    assert!(
        serde_json::from_str::<GameSetup>(
            r#"{"policy":"assisted","start_fen":"lixo","time_control":null}"#
        )
        .is_err()
    );
}

#[test]
fn eventos_de_uma_partida_real_fazem_ida_e_volta() {
    let mut game = common::assisted();
    let mut clock = common::FakeClock::new();
    common::play_all(&mut game, &mut clock, "f3 e5 g4 Qh4#");
    let json = serde_json::to_string(game.events()).unwrap();
    let back: Vec<Event> = serde_json::from_str(&json).unwrap();
    let replayed = chess_game::Game::replay(game.setup().clone(), &back);
    assert_eq!(replayed.snapshot(), game.snapshot());
}

#[test]
fn eventos_do_b6_tem_tipo_estavel_e_setup_antigo_continua_legivel() {
    let event = Event::new(Timestamp::mono(7), EventKind::Paused);
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["kind"]["type"], "paused");
    assert_eq!(value["version"], PAYLOAD_VERSION);

    // Setup gravado antes do B6 (sem `infraction_stage`) continua legível.
    let old = r#"{"policy":"assisted","start_fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1","time_control":null}"#;
    let setup: GameSetup = serde_json::from_str(old).unwrap();
    assert_eq!(setup.infraction_stage(), InfractionStage::Clock);

    let fide = GameSetup::standard(PolicyKind::FideSimulation)
        .with_infraction_stage(InfractionStage::Release);
    let json = serde_json::to_string(&fide).unwrap();
    assert!(json.contains("fide_simulation") && json.contains("release"));
    assert_eq!(serde_json::from_str::<GameSetup>(&json).unwrap(), fide);
}

#[test]
fn partida_fide_com_infracao_pausa_e_bandeira_faz_ida_e_volta() {
    use common::*;
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        20_000,
        1_000,
    );
    touch(&mut game, 100, "a1").unwrap();
    drop_piece(&mut game, 200, "a1", "b2").unwrap();
    press(&mut game, 300).unwrap();
    game.handle(chess_game::Command::Pause, at(400)).unwrap();
    game.handle(chess_game::Command::Resume, at(500)).unwrap();
    // A bandeira cai: o Comando tardio só a registra.
    drop_piece(&mut game, 60_000, "e2", "e4").unwrap();
    assert!(game.snapshot().outcome.is_some());

    let json = serde_json::to_string(game.events()).unwrap();
    let back: Vec<Event> = serde_json::from_str(&json).unwrap();
    let setup_json = serde_json::to_string(game.setup()).unwrap();
    let setup: GameSetup = serde_json::from_str(&setup_json).unwrap();
    let replayed = chess_game::Game::try_replay(setup, &back).unwrap();
    assert_eq!(replayed.snapshot(), game.snapshot());
}
