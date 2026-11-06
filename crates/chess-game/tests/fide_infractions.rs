#![allow(clippy::unwrap_used)]
//! Simulação FIDE: Lance ilegal concluído e Infrações (art. 7.5), BDD-03, BDD-04,
//! AC-05, RULE-04, e a etapa de Infração parametrizável (DEC-36, para o B8).

mod common;
use chess_core::{Color, Piece};
use chess_game::{
    Command, EndReason, EventKind, Game, GameResult, GameSetup, InfractionCause, InfractionStage,
    PolicyKind, Rejection, TWO_MINUTES_MS, TimePenalty,
};
use common::*;

const CLASSICAL: u64 = 90 * 60_000;
const INC: u64 = 30_000;

fn kinds_of(events: &[chess_game::Event]) -> Vec<EventKind> {
    events.iter().map(|e| e.kind.clone()).collect()
}

/// Brancas tocam e1... não: a torre a1 solta em b2 (ilegal) e aciona o relógio.
fn illegal_rook_and_press(game: &mut Game, ms: u64) -> Vec<chess_game::Event> {
    touch(game, ms, "a1").unwrap();
    drop_piece(game, ms + 1, "a1", "b2").unwrap();
    press(game, ms + 2).unwrap()
}

#[test]
fn bdd_03_primeira_infracao_restaura_a_posicao_e_da_dois_minutos_ao_adversario() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        CLASSICAL,
        INC,
    );
    touch(&mut game, 1_000, "e2").unwrap();
    drop_piece(&mut game, 2_000, "e2", "e5").unwrap(); // peão não anda 3 casas
    let events = press(&mut game, 3_000).unwrap();

    // Uma Infração concluída, com causa, número e penalidade (art. 7.5.5).
    assert_eq!(events.len(), 1);
    let EventKind::InfractionRecorded {
        side,
        cause,
        count,
        stage,
        penalty,
    } = &events[0].kind
    else {
        panic!("esperava InfractionRecorded: {events:?}");
    };
    assert_eq!(
        (*side, *count, *stage),
        (Color::White, 1, InfractionStage::Clock)
    );
    assert!(matches!(cause, InfractionCause::IllegalMove { .. }));
    assert_eq!(
        *penalty,
        Some(TimePenalty {
            to: Color::Black,
            ms: TWO_MINUTES_MS
        })
    );

    // A posição regular volta; as pretas ganham 2 min; a vez continua das brancas.
    let snap = game.snapshot();
    assert_eq!(snap.position, chess_core::CanonicalPosition::start());
    assert!(snap.provisional_release.is_none());
    assert!(snap.history.is_empty());
    assert_eq!(snap.side_to_move, Color::White);
    assert_eq!((snap.infractions_white, snap.infractions_black), (1, 0));
    let r = game.clock_at(at(3_000)).unwrap();
    assert_eq!(r.black_ms, CLASSICAL + 120_000);
    assert_eq!(r.running, Some(Color::White));
    assert_eq!(
        r.white_ms,
        CLASSICAL - 3_000,
        "o tempo gasto não é devolvido"
    );

    // O jogador continua sujeito à Obrigação de peça tocada (art. 7.5.1 -> 4.3).
    assert_eq!(snap.must_move_from, vec![sq("e2")]);
    assert!(matches!(
        drop_piece(&mut game, 4_000, "d2", "d4"),
        Err(Rejection::TouchedPieceObligation { .. })
    ));
    drop_piece(&mut game, 5_000, "e2", "e4").unwrap();
    press(&mut game, 6_000).unwrap();
    // Incremento do lance concluído (6.? Fischer): +30 s às brancas.
    assert_eq!(
        game.clock_at(at(6_000)).unwrap().white_ms,
        CLASSICAL - 6_000 + INC
    );
}

#[test]
fn bdd_04_segunda_infracao_do_mesmo_jogador_perde_a_partida() {
    let mut game = fide();
    illegal_rook_and_press(&mut game, 0); // 1ª: nada acontece além da penalidade
    // Mesmo jogador, ilegal de novo (agora a obrigação é da torre a1, que está bloqueada
    // e por isso não obriga: qualquer lance legal vale, mas este é ilegal).
    touch(&mut game, 10, "a1").unwrap();
    drop_piece(&mut game, 11, "a1", "b2").unwrap();
    let events = press(&mut game, 12).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::InfractionRecorded {
                count: 2,
                penalty: None,
                ..
            },
            EventKind::GameEnded { .. }
        ]
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::SecondIllegalMove);
    assert_eq!(press(&mut game, 13), Err(Rejection::GameOver));
}

#[test]
fn bdd_04_excecao_adversario_sem_possibilidade_de_mate_e_empate() {
    // Pretas só com o rei: por nenhuma série de lances legais dão mate.
    let mut game = fide_from("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    illegal_rook_and_press(&mut game, 0);
    let events = illegal_rook_and_press(&mut game, 10);
    assert!(matches!(
        events.last().map(|e| &e.kind),
        Some(EventKind::GameEnded { .. })
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::SecondIllegalMoveDraw);
}

#[test]
fn bdd_04_sem_a_excecao_quando_o_adversario_tem_material_de_mate() {
    // Pretas com torre: podem dar mate; segunda Infração das brancas é derrota.
    let mut game = fide_from("4k2r/8/8/8/8/8/8/R3K3 w - - 0 1");
    illegal_rook_and_press(&mut game, 0);
    illegal_rook_and_press(&mut game, 10);
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::SecondIllegalMove);
}

#[test]
fn infracoes_sao_contadas_por_jogador() {
    let mut game = fide();
    illegal_rook_and_press(&mut game, 0); // brancas: 1
    // As brancas jogam legalmente; agora as pretas erram uma vez.
    let t = fide_play(&mut game, 10, "e4");
    touch(&mut game, t, "a8").unwrap();
    drop_piece(&mut game, t + 1, "a8", "b7").unwrap();
    let events = press(&mut game, t + 2).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::InfractionRecorded {
            side: Color::Black,
            count: 1,
            ..
        }]
    ));
    assert!(game.snapshot().outcome.is_none());
    assert_eq!(
        (
            game.snapshot().infractions_white,
            game.snapshot().infractions_black
        ),
        (1, 1)
    );
}

#[test]
fn rule_04_acionar_o_relogio_sem_jogar_e_lance_ilegal_art_7_5_3() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        CLASSICAL,
        0,
    );
    let events = press(&mut game, 2_000).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::InfractionRecorded {
            cause: InfractionCause::ClockPressedWithoutMove,
            count: 1,
            penalty: Some(_),
            ..
        }]
    ));
    let r = game.clock_at(at(2_000)).unwrap();
    assert_eq!(r.black_ms, CLASSICAL + 120_000);
    assert_eq!(r.running, Some(Color::White));
    // Segunda vez: derrota.
    press(&mut game, 3_000).unwrap();
    assert_eq!(
        game.snapshot().outcome.unwrap().reason,
        EndReason::SecondIllegalMove
    );
}

#[test]
fn acionar_o_relogio_depois_de_so_tocar_mantem_a_obrigacao() {
    let mut game = fide();
    touch(&mut game, 1, "e2").unwrap();
    press(&mut game, 2).unwrap();
    assert_eq!(game.snapshot().infractions_white, 1);
    assert_eq!(game.snapshot().must_move_from, vec![sq("e2")]);
}

#[test]
fn promocao_sem_escolha_e_lance_ilegal_e_o_peao_vira_dama_art_7_5_2() {
    let mut game = fide_timed_from("8/P6k/8/8/8/8/6r1/K7 w - - 0 1", CLASSICAL, 0);
    touch(&mut game, 1_000, "a7").unwrap();
    drop_piece(&mut game, 2_000, "a7", "a8").unwrap();
    let events = press(&mut game, 3_000).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::InfractionRecorded {
                cause: InfractionCause::PromotionNotCompleted { .. },
                count: 1,
                penalty: Some(_),
                ..
            },
            EventKind::MoveCompleted { .. }
        ]
    ));
    let snap = game.snapshot();
    assert_eq!(snap.history[0].san, "a8=Q");
    assert_eq!(snap.side_to_move, Color::Black);
    assert_eq!(
        game.clock_at(at(3_000)).unwrap().black_ms,
        CLASSICAL + 120_000
    );
    assert_eq!(snap.infractions_white, 1);
    assert_eq!(
        snap.position.piece_at(sq("a8")),
        Some((Color::White, Piece::Queen))
    );
}

#[test]
fn segunda_infracao_por_promocao_sem_escolha_perde() {
    let mut game = fide_from("8/P6k/8/8/8/8/6r1/K7 w - - 0 1");
    press(&mut game, 1).unwrap(); // 1ª: relógio sem jogar
    touch(&mut game, 2, "a7").unwrap();
    drop_piece(&mut game, 3, "a7", "a8").unwrap();
    press(&mut game, 4).unwrap();
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::SecondIllegalMove);
    assert!(game.snapshot().provisional.is_none());
}

#[test]
fn o_relogio_do_infrator_continua_correndo_apos_a_infracao() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        CLASSICAL,
        0,
    );
    illegal_rook_and_press(&mut game, 0);
    assert_eq!(
        game.clock_at(at(10_000)).unwrap().white_ms,
        CLASSICAL - 10_000
    );
}

#[test]
fn sem_controle_de_tempo_a_infracao_registra_a_penalidade_sem_relogio() {
    let mut game = fide();
    let events = illegal_rook_and_press(&mut game, 0);
    assert!(matches!(
        events[0].kind,
        EventKind::InfractionRecorded {
            penalty: Some(_),
            ..
        }
    ));
    assert!(game.clock_at(at(1)).is_none());
}

#[test]
fn a_politica_fide_ignora_a_etapa_de_infracao_do_setup() {
    // Parâmetro reservado à Eliminação Imediata (B8): FIDE é sempre no relógio.
    let setup = GameSetup::standard(PolicyKind::FideSimulation)
        .with_infraction_stage(InfractionStage::Release);
    let mut game = Game::new(setup);
    touch(&mut game, 1, "a1").unwrap();
    let events = drop_piece(&mut game, 2, "a1", "b2").unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [EventKind::IllegalMoveReleased { .. }]
    ));
}

#[test]
fn abandono_e_pausa_valem_com_peca_solta_ilegalmente() {
    let mut game = fide();
    touch(&mut game, 1, "a1").unwrap();
    drop_piece(&mut game, 2, "a1", "b2").unwrap();
    game.handle(Command::Pause, at(3)).unwrap();
    assert_eq!(press(&mut game, 4), Err(Rejection::Paused));
    game.handle(Command::Resume, at(5)).unwrap();
    game.handle(Command::Resign { side: Color::White }, at(6))
        .unwrap();
    assert_eq!(
        game.snapshot().outcome.unwrap().reason,
        EndReason::Resignation
    );
}

#[test]
fn fide_com_bandeira_durante_lance_feito_perde_por_tempo() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        10_000,
        0,
    );
    touch(&mut game, 1_000, "e2").unwrap();
    drop_piece(&mut game, 2_000, "e2", "e4").unwrap();
    // O relógio só é acionado depois de esgotado o tempo: bandeira, lance não vale.
    let events = press(&mut game, 10_500).unwrap();
    assert!(matches!(
        kinds_of(&events).as_slice(),
        [
            EventKind::FlagFell {
                side: Color::White,
                flag_at_ms: 10_000
            },
            EventKind::GameEnded { .. }
        ]
    ));
    assert_eq!(game.snapshot().outcome.unwrap().reason, EndReason::Timeout);
    assert!(game.snapshot().history.is_empty());
}
