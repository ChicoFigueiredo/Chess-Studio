#![allow(clippy::unwrap_used)]
//! Catálogo de Exercícios: arquivo versionado, validado pelo chess-core.

use chess_training::{Catalog, CatalogError, ExerciseKind};

#[test]
fn bundled_catalog_loads_and_covers_the_p0_rule_kinds() {
    let catalog = Catalog::bundled().unwrap();
    for kind in [
        ExerciseKind::Stalemate,
        ExerciseKind::Mate,
        ExerciseKind::IllegalPawn,
        ExerciseKind::Castling,
        ExerciseKind::Promotion,
    ] {
        assert!(
            catalog.iter().any(|e| e.kind == kind),
            "falta um Exercício de {kind:?}"
        );
    }
    assert!(catalog.iter().all(|e| e.version >= 1));
}

fn one(fen: &str, solution: &str) -> String {
    format!(
        r#"{{"format":1,"exercises":[{{"id":"x-1","kind":"mate","title":"t","fen":"{fen}",
        "objective":"o","solution":{solution},"explanation":"e","version":1}}]}}"#
    )
}

const MATE_FEN: &str = "6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1";

#[test]
fn a_valid_file_loads() {
    let json = one(MATE_FEN, r#"{"type":"mate_in_one"}"#);
    assert_eq!(Catalog::from_json(&json).unwrap().iter().count(), 1);
}

#[test]
fn invalid_fen_is_rejected() {
    let json = one("isto nao e fen", r#"{"type":"mate_in_one"}"#);
    assert!(matches!(
        Catalog::from_json(&json),
        Err(CatalogError::Invalid { .. })
    ));
}

#[test]
fn an_illegal_solution_line_is_rejected() {
    let json = one(MATE_FEN, r#"{"type":"moves","line":["a1a5"]}"#);
    assert!(matches!(
        Catalog::from_json(&json),
        Err(CatalogError::Invalid { .. })
    ));
}

#[test]
fn an_objective_without_any_legal_solution_is_rejected() {
    // Nenhum Lance das brancas deixa as pretas afogadas aqui.
    let json = one(MATE_FEN, r#"{"type":"force_stalemate"}"#);
    assert!(matches!(
        Catalog::from_json(&json),
        Err(CatalogError::Invalid { .. })
    ));
    // "Recusar o Lance ilegal" exige que o Lance seja de fato ilegal.
    let json = one(MATE_FEN, r#"{"type":"refuse_illegal","uci":"a1a8"}"#);
    assert!(matches!(
        Catalog::from_json(&json),
        Err(CatalogError::Invalid { .. })
    ));
}

#[test]
fn duplicate_ids_and_unknown_format_are_rejected() {
    let ex = r#"{"id":"x-1","kind":"mate","title":"t","fen":"6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1",
        "objective":"o","solution":{"type":"mate_in_one"},"explanation":"e","version":1}"#;
    let dup = format!(r#"{{"format":1,"exercises":[{ex},{ex}]}}"#);
    assert!(matches!(
        Catalog::from_json(&dup),
        Err(CatalogError::DuplicateId(_))
    ));
    let future = format!(r#"{{"format":99,"exercises":[{ex}]}}"#);
    assert!(matches!(
        Catalog::from_json(&future),
        Err(CatalogError::UnsupportedFormat(99))
    ));
}
