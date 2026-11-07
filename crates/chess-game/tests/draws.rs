#![allow(clippy::unwrap_used)]
//! Empates e abandono: automáticos no Assistido (DEC-38), propostas (DEC-41).

mod common;
use chess_core::Color;
use chess_game::{Command, EndReason, EventKind, GameResult, Rejection};
use common::*;

fn reason(game: &chess_game::Game) -> Option<EndReason> {
    game.snapshot().outcome.map(|o| o.reason)
}

#[test]
fn tripla_repeticao_empata_automaticamente_no_assistido() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, KNIGHT_CYCLE);
    assert_eq!(reason(&game), None, "segunda ocorrência ainda não encerra");
    play_all(&mut game, &mut clock, "Nf3 Nf6 Ng1");
    assert_eq!(reason(&game), None);
    let events = play_san(&mut game, &mut clock, "Ng8");
    assert!(matches!(
        kinds(&events).as_slice(),
        [EventKind::MoveCompleted { .. }, EventKind::GameEnded { .. }]
    ));
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::ThreefoldRepetition);
}

#[test]
fn a_contagem_vale_para_qualquer_posicao_do_historico_nao_so_a_inicial() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5 Nf3 Nf6 Ng1 Ng8 Nf3 Nf6 Ng1");
    assert_eq!(reason(&game), None);
    play_san(&mut game, &mut clock, "Ng8");
    assert_eq!(reason(&game), Some(EndReason::ThreefoldRepetition));
}

#[test]
fn cinquenta_lances_empatam_automaticamente_no_assistido() {
    let mut game = assisted_from("4k3/8/8/8/8/8/8/R3K3 w - - 99 100");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Ra2");
    assert_eq!(reason(&game), Some(EndReason::FiftyMoveRule));
    assert_eq!(game.snapshot().outcome.unwrap().result, GameResult::Draw);
}

#[test]
fn cinquenta_lances_nao_encerram_antes_de_cem_meios_lances() {
    let mut game = assisted_from("4k3/8/8/8/8/8/8/R3K3 w - - 98 100");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Ra2");
    assert_eq!(reason(&game), None);
}

#[test]
fn setenta_e_cinco_lances_encerram_automaticamente() {
    let mut game = assisted_from("4k3/8/8/8/8/8/8/R3K3 w - - 149 100");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Ra2");
    assert_eq!(reason(&game), Some(EndReason::SeventyFiveMoveRule));
}

#[test]
fn mate_no_150o_meio_lance_vence_o_empate_dos_75() {
    // Ra8# com o relógio em 149 -> 150: mate prevalece (art. 9.6.2).
    let mut game = assisted_from("7k/6pp/8/8/8/8/8/R3K3 w - - 149 100");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Ra8#");
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.reason, EndReason::Checkmate);
    assert_eq!(outcome.result, GameResult::WhiteWins);
}

#[test]
fn no_assistido_a_tripla_encerra_antes_de_a_quintupla_ser_possivel() {
    // As 5 repetições (sempre automáticas) só são alcançáveis quando a tripla
    // exige Reclamação (B6); a regra pura é coberta no teste unitário da Política.
    let mut game = assisted();
    let mut clock = FakeClock::new();
    for _ in 0..2 {
        play_all(&mut game, &mut clock, KNIGHT_CYCLE);
    }
    assert_eq!(reason(&game), Some(EndReason::ThreefoldRepetition));
}

#[test]
fn material_insuficiente_empata_ao_ficar_rei_contra_rei() {
    let mut game = assisted_from("4k3/8/8/8/8/8/3r4/4K3 w - - 0 1");
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "Kxd2");
    assert_eq!(reason(&game), Some(EndReason::InsufficientMaterial));
}

#[test]
fn posicao_morta_por_bloqueio_de_pecoes_empata_automaticamente() {
    // Muro total de peões travados; a captura da torre deixa só reis e peões travados.
    let mut game = assisted_from("4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/3r4/4K3 w - - 0 1");
    let mut clock = FakeClock::new();
    assert_eq!(reason(&game), None);
    play_san(&mut game, &mut clock, "Kxd2");
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::DeadPosition);
}

#[test]
fn proposta_aceita_encerra_em_empate_por_acordo() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_san(&mut game, &mut clock, "e4");
    let offer = game
        .handle(Command::OfferDraw { side: Color::Black }, clock.now())
        .unwrap();
    assert!(matches!(
        kinds(&offer).as_slice(),
        [EventKind::DrawOffered { by: Color::Black }]
    ));
    assert_eq!(game.snapshot().pending_draw_offer, Some(Color::Black));

    game.handle(Command::AcceptDraw, clock.now()).unwrap();

    assert_eq!(reason(&game), Some(EndReason::DrawAgreement));
    assert_eq!(game.snapshot().outcome.unwrap().result, GameResult::Draw);
    assert_eq!(game.snapshot().pending_draw_offer, None);
}

#[test]
fn proposta_recusada_segue_o_jogo() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    game.handle(Command::OfferDraw { side: Color::White }, clock.now())
        .unwrap();
    game.handle(Command::DeclineDraw, clock.now()).unwrap();
    assert_eq!(game.snapshot().pending_draw_offer, None);
    assert!(game.snapshot().outcome.is_none());
    play_san(&mut game, &mut clock, "e4");
}

#[test]
fn proposta_caduca_quando_o_adversario_joga() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    game.handle(Command::OfferDraw { side: Color::White }, clock.now())
        .unwrap();
    play_san(&mut game, &mut clock, "e4");
    assert_eq!(game.snapshot().pending_draw_offer, None);
    assert_eq!(
        game.handle(Command::AcceptDraw, clock.now()),
        Err(Rejection::NoDrawOffer)
    );
}

#[test]
fn proposta_duplicada_e_resposta_sem_proposta_sao_rejeitadas() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    assert_eq!(
        game.handle(Command::AcceptDraw, clock.now()),
        Err(Rejection::NoDrawOffer)
    );
    assert_eq!(
        game.handle(Command::DeclineDraw, clock.now()),
        Err(Rejection::NoDrawOffer)
    );
    game.handle(Command::OfferDraw { side: Color::White }, clock.now())
        .unwrap();
    assert_eq!(
        game.handle(Command::OfferDraw { side: Color::Black }, clock.now()),
        Err(Rejection::DrawOfferPending)
    );
}

#[test]
fn abandono_vale_para_qualquer_dos_lados_a_qualquer_momento() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    // Branco abandona sem ser a vez dele... a vez é das brancas; as pretas abandonam.
    game.handle(Command::Resign { side: Color::Black }, clock.now())
        .unwrap();
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::WhiteWins, EndReason::Resignation)
    );
}
