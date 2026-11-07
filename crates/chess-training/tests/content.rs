#![allow(clippy::unwrap_used)]
//! Curadoria do P0 (B14): o conteúdo do catálogo embutido é válido, cumprível e
//! faz o que o texto promete.

use chess_core::{CanonicalPosition, GameStatus, Piece};
use chess_training::{Catalog, Exercise, ExerciseKind, Solution};
use std::collections::HashSet;
use std::path::Path;

fn catalog() -> Catalog {
    Catalog::bundled().unwrap()
}

fn of_kind(catalog: &Catalog, kind: ExerciseKind) -> Vec<&Exercise> {
    catalog.iter().filter(|e| e.kind == kind).collect()
}

#[test]
fn the_bundled_catalog_is_the_exercises_directory() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("exercises");
    let from_dir = Catalog::from_dir(&dir).unwrap();
    let ids = |c: &Catalog| c.iter().map(|e| e.id.clone()).collect::<HashSet<_>>();
    assert_eq!(ids(&catalog()), ids(&from_dir), "BUNDLED fora de sincronia");
    assert_eq!(catalog().len(), from_dir.len());
}

#[test]
fn every_kind_has_at_least_three_exercises() {
    let catalog = catalog();
    for kind in ExerciseKind::ALL {
        let n = of_kind(&catalog, kind).len();
        assert!(n >= 3, "{kind:?} tem só {n} Exercício(s)");
    }
}

#[test]
fn ids_and_titles_are_unique_and_every_text_is_filled_in() {
    let catalog = catalog();
    let mut titles = HashSet::new();
    let mut fens = HashSet::new();
    for e in catalog.iter() {
        assert!(
            titles.insert(e.title.clone()),
            "título repetido: {}",
            e.title
        );
        // Memória parte da posição inicial: o que as distingue é a linha, não o FEN.
        let key = match &e.solution {
            Solution::Memorize { line, .. } => format!("{} {}", e.fen, line.join(" ")),
            _ => e.fen.clone(),
        };
        assert!(fens.insert(key), "FEN (ou linha) repetido: {}", e.id);
        assert!(
            e.explanation.trim().chars().count() >= 60,
            "{}: explicação curta demais",
            e.id
        );
        assert!(
            e.objective.trim().chars().count() >= 10,
            "{}: objetivo curto demais",
            e.id
        );
        assert!(e.title.chars().count() <= 48, "{}: título longo", e.id);
        // Memória treina sequências de abertura, não uma regra: não cita artigo.
        assert!(
            e.kind == ExerciseKind::Memory || e.explanation.contains("art. "),
            "{}: cite a regra (art. ...)",
            e.id
        );
        e.validate().unwrap();
        assert!(e.new_game().is_ok(), "{}: partida não inicia", e.id);
    }
}

#[test]
fn every_objective_can_be_met_from_the_original_position() {
    for e in catalog().iter() {
        let start = e.position().unwrap();
        match &e.solution {
            Solution::Moves { line } => {
                assert!(!line.is_empty(), "{}", e.id);
                assert_eq!(e.solution_line().len(), line.len(), "{}", e.id);
            }
            Solution::RefuseIllegal { uci } => {
                let mv = uci.parse().unwrap();
                assert!(!start.is_legal(mv), "{}: devia ser ilegal", e.id);
            }
            // Memória (B15, DEC-45): a linha a memorizar é jogável desde a posição inicial.
            Solution::Memorize { line, .. } => {
                let mut position = start.clone();
                for uci in line {
                    let mv = uci.parse().unwrap();
                    assert!(position.is_legal(mv), "{}: {uci} ilegal na linha", e.id);
                    position = position.play(mv).unwrap();
                }
            }
            // `validate` já exige ao menos um Lance que cumpra o critério.
            _ => assert_eq!(e.solution_line().len(), 1, "{}", e.id),
        }
    }
}

/// Posição depois de jogar toda a linha (ou o primeiro Lance que cumpre o critério).
fn after_solution(e: &Exercise) -> (CanonicalPosition, CanonicalPosition) {
    let start = e.position().unwrap();
    let mut position = start.clone();
    for mv in e.solution_line() {
        position = position.play(mv).unwrap();
    }
    (start, position)
}

#[test]
fn the_solution_does_what_the_kind_promises() {
    let catalog = catalog();
    for e in catalog.iter() {
        let (start, end) = after_solution(e);
        let first = e.solution_line().first().copied();
        match e.kind {
            ExerciseKind::Stalemate => {
                assert!(
                    matches!(end.status(), GameStatus::Stalemate),
                    "{}: não termina em afogamento",
                    e.id
                );
                assert!(!end.is_check(), "{}", e.id);
            }
            ExerciseKind::Mate => {
                assert!(
                    matches!(end.status(), GameStatus::Checkmate { .. }),
                    "{}: não termina em xeque-mate",
                    e.id
                );
            }
            ExerciseKind::Check => {
                assert!(end.is_check(), "{}: não termina em xeque", e.id);
                assert!(
                    !matches!(end.status(), GameStatus::Checkmate { .. }),
                    "{}: xeque simples não pode ser mate",
                    e.id
                );
            }
            ExerciseKind::Promotion => {
                let promoted = e
                    .solution_line()
                    .iter()
                    .filter(|m| m.promotion().is_some())
                    .count();
                assert!(promoted >= 1, "{}: nenhuma promoção na linha", e.id);
            }
            ExerciseKind::Castling => {
                let (uci, legal_castle) = match &e.solution {
                    Solution::Moves { line } => (line[0].clone(), true),
                    Solution::RefuseIllegal { uci } => (uci.clone(), false),
                    other => panic!("{}: solução de roque inesperada {other:?}", e.id),
                };
                let mv: chess_core::Move = uci.parse().unwrap();
                let (_, piece) = start.piece_at(mv.from()).unwrap();
                assert_eq!(piece, Piece::King, "{}: roque começa pelo rei", e.id);
                assert_eq!(mv.from().file().abs_diff(mv.to().file()), 2, "{}", e.id);
                assert_eq!(start.is_legal(mv), legal_castle, "{}", e.id);
                if legal_castle {
                    // O rei e a torre mudaram de lugar: o rei na casa de destino.
                    let (_, moved) = end.piece_at(mv.to()).unwrap();
                    assert_eq!(moved, Piece::King, "{}", e.id);
                }
            }
            ExerciseKind::IllegalPawn => {
                let Solution::RefuseIllegal { uci } = &e.solution else {
                    panic!("{}: peão ilegal usa refuse_illegal", e.id);
                };
                let mv: chess_core::Move = uci.parse().unwrap();
                let (_, piece) = start.piece_at(mv.from()).unwrap();
                assert_eq!(piece, Piece::Pawn, "{}: o Lance ilegal é de peão", e.id);
                assert!(first.is_none(), "{}", e.id);
            }
            ExerciseKind::Memory => {
                let Solution::Memorize { line, .. } = &e.solution else {
                    panic!("{}: memória usa memorize", e.id);
                };
                assert!(
                    (3..=8).contains(&line.len()),
                    "{}: memória pede de 3 a 8 Lances, tem {}",
                    e.id,
                    line.len()
                );
                assert_eq!(e.solution_line().len(), line.len(), "{}", e.id);
            }
        }
    }
}

#[test]
fn castling_covers_legal_and_the_three_classic_illegal_cases() {
    let catalog = catalog();
    let castling = of_kind(&catalog, ExerciseKind::Castling);
    let legal = castling
        .iter()
        .filter(|e| matches!(e.solution, Solution::Moves { .. }))
        .count();
    let illegal = castling
        .iter()
        .filter(|e| matches!(e.solution, Solution::RefuseIllegal { .. }))
        .count();
    assert!(legal >= 2);
    assert!(illegal >= 3, "xeque, casa atravessada e direito perdido");
}

#[test]
fn ids_used_by_the_training_tests_are_still_there() {
    let catalog = catalog();
    for id in [
        "afogamento-001",
        "mate-em-1-001",
        "roque-001",
        "peao-ilegal-001",
        "promocao-001",
    ] {
        assert!(catalog.get(id).is_some(), "{id} sumiu");
    }
}
