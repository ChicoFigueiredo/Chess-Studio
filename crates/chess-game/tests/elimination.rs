#![allow(clippy::unwrap_used)]
//! Eliminação Imediata (B8): regra personalizada de treinamento. Mesma mecânica
//! da Simulação FIDE, mas a primeira Infração concluída encerra a Partida
//! (AC-06, DEC-35, DEC-36).

mod common;
use chess_core::Color;
use chess_game::{
    Command, EndReason, EventKind, Game, GameResult, GameSetup, InfractionStage, PolicyKind,
    Rejection,
};
use common::*;

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
/// Só reis e uma torre branca: as pretas nunca poderiam dar mate.
const ROOK_ENDGAME: &str = "4k3/8/8/8/8/8/8/R3K3 w - - 0 1";
/// Torre branca contra torre preta: ambos podem, em tese, dar mate.
const TWO_ROOKS: &str = "r3k3/8/8/8/8/8/8/R3K3 w - - 0 1";

fn setup(fen: &str, stage: InfractionStage) -> GameSetup {
    GameSetup::from_fen(PolicyKind::SuddenElimination, fen)
        .unwrap()
        .with_infraction_stage(stage)
}

fn elimination(fen: &str, stage: InfractionStage) -> Game {
    Game::new(setup(fen, stage))
}

fn ended(events: &[chess_game::Event]) -> Option<chess_game::Outcome> {
    events.iter().find_map(|e| match e.kind {
        EventKind::GameEnded { outcome } => Some(outcome),
        _ => None,
    })
}

#[test]
fn ac_06_etapa_relogio_primeira_infracao_encerra_com_derrota_de_quem_cometeu() {
    let mut game = elimination(TWO_ROOKS, InfractionStage::Clock);
    touch(&mut game, 1_000, "a1").unwrap();
    let released = drop_piece(&mut game, 1_001, "a1", "b2").unwrap();
    assert!(
        ended(&released).is_none(),
        "na etapa relógio, soltar não encerra"
    );
    let events = press(&mut game, 1_002).unwrap();

    let EventKind::InfractionRecorded {
        side,
        count,
        stage,
        penalty,
        ..
    } = &events[0].kind
    else {
        panic!("esperava InfractionRecorded: {events:?}");
    };
    assert_eq!(
        (*side, *count, *stage, *penalty),
        (Color::White, 1, InfractionStage::Clock, None)
    );
    let outcome = ended(&events).expect("a Partida encerra");
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::EliminatedByInfraction);
    assert_eq!(game.snapshot().outcome, Some(outcome));
    // Terminada: nada mais é aceito.
    assert_eq!(touch(&mut game, 2_000, "e1"), Err(Rejection::GameOver));
}

#[test]
fn ac_06_etapa_soltura_encerra_ao_soltar_a_peca() {
    let mut game = elimination(TWO_ROOKS, InfractionStage::Release);
    touch(&mut game, 1_000, "a1").unwrap();
    let events = drop_piece(&mut game, 1_001, "a1", "b2").unwrap();
    let outcome = ended(&events).expect("encerra ao soltar");
    assert_eq!(outcome.result, GameResult::BlackWins);
    assert_eq!(outcome.reason, EndReason::EliminatedByInfraction);
    assert!(events.iter().any(|e| matches!(
        e.kind,
        EventKind::InfractionRecorded {
            stage: InfractionStage::Release,
            count: 1,
            ..
        }
    )));
}

#[test]
fn pretas_eliminadas_dao_vitoria_as_brancas() {
    let mut game = elimination(START, InfractionStage::Release);
    fide_play(&mut game, 0, "e4");
    touch(&mut game, 5_000, "e7").unwrap();
    let events = drop_piece(&mut game, 5_001, "e7", "e3").unwrap();
    let outcome = ended(&events).unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::WhiteWins, EndReason::EliminatedByInfraction)
    );
}

#[test]
fn excecao_de_material_adversario_sem_como_dar_mate_resulta_em_empate_nas_duas_etapas() {
    for stage in [InfractionStage::Release, InfractionStage::Clock] {
        let mut game = elimination(ROOK_ENDGAME, stage);
        touch(&mut game, 1_000, "a1").unwrap();
        let mut events = drop_piece(&mut game, 1_001, "a1", "b2").unwrap();
        if stage == InfractionStage::Clock {
            events = press(&mut game, 1_002).unwrap();
        }
        let outcome = ended(&events).unwrap_or_else(|| panic!("{stage:?}: encerra"));
        assert_eq!(
            (outcome.result, outcome.reason),
            (GameResult::Draw, EndReason::EliminatedByInfractionDraw),
            "{stage:?}"
        );
    }
}

#[test]
fn retratar_antes_da_etapa_relogio_nao_encerra() {
    let mut game = elimination(START, InfractionStage::Clock);
    touch(&mut game, 1_000, "e2").unwrap();
    drop_piece(&mut game, 1_001, "e2", "e5").unwrap(); // ilegal, ainda não é Infração
    let retracted = game.handle(Command::RetractMove, at(1_002)).unwrap();
    assert!(ended(&retracted).is_none());
    assert_eq!(game.snapshot().infractions_white, 0);
    // Joga um Lance legal com a peça tocada e a vez passa normalmente.
    drop_piece(&mut game, 1_003, "e2", "e4").unwrap();
    press(&mut game, 1_004).unwrap();
    let snap = game.snapshot();
    assert!(snap.outcome.is_none());
    assert_eq!(snap.side_to_move, Color::Black);
    assert_eq!(snap.history.len(), 1);
}

#[test]
fn acionar_o_relogio_sem_lance_tambem_elimina_na_etapa_relogio() {
    let mut game = elimination(START, InfractionStage::Clock);
    let events = press(&mut game, 1_000).unwrap();
    assert_eq!(
        ended(&events).unwrap().reason,
        EndReason::EliminatedByInfraction
    );
}

#[test]
fn nao_ha_desfazer_nem_refazer() {
    let mut game = elimination(START, InfractionStage::Clock);
    assert!(matches!(
        game.handle(Command::Undo, at(1)),
        Err(Rejection::NotSupportedByPolicy(_))
    ));
}

#[test]
fn partida_legal_funciona_como_na_simulacao_fide() {
    let mut game = elimination(START, InfractionStage::Clock);
    fide_play_all(&mut game, 0, "f3 e5 g4 Qh4#");
    let outcome = game.snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::BlackWins, EndReason::Checkmate)
    );
}

#[test]
fn a_etapa_escolhida_fica_gravada_e_sobrevive_ao_serde_e_ao_replay() {
    for stage in [InfractionStage::Clock, InfractionStage::Release] {
        let setup = setup(TWO_ROOKS, stage);
        assert_eq!(setup.infraction_stage(), stage);
        let json = serde_json::to_string(&setup).unwrap();
        let back: GameSetup = serde_json::from_str(&json).unwrap();
        assert_eq!(back, setup);
        assert_eq!(back.infraction_stage(), stage);
        assert_eq!(back.policy(), PolicyKind::SuddenElimination);

        let mut game = Game::new(setup.clone());
        touch(&mut game, 1_000, "a1").unwrap();
        drop_piece(&mut game, 1_001, "a1", "b2").unwrap();
        let _ = press(&mut game, 1_002);
        let replayed = Game::replay(back, game.events());
        assert_eq!(replayed.snapshot(), game.snapshot());
        assert_eq!(
            replayed.snapshot().outcome.unwrap().reason,
            EndReason::EliminatedByInfraction
        );
    }
}

#[test]
fn snapshot_rotula_a_regra_como_personalizada_e_nunca_como_fide() {
    let game = elimination(START, InfractionStage::Clock);
    let label = game.snapshot().rules_version.expect("rótulo presente");
    assert_eq!(
        label,
        "Regra personalizada de treinamento — Eliminação Imediata"
    );
    assert!(!label.contains("FIDE"));
    assert!(!game.snapshot().non_classical_notice);
}

#[test]
fn a_bandeira_continua_valendo_na_eliminacao() {
    let mut game = timed(setup(START, InfractionStage::Clock), 60_000, 0);
    fide_play(&mut game, 0, "e4");
    let _ = press(&mut game, 200_000);
    assert_eq!(
        game.snapshot().outcome.map(|o| o.reason),
        Some(EndReason::Timeout)
    );
}
