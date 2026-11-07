#![allow(clippy::unwrap_used)]
//! Perfis de treino (US-27): sequência de Exercícios com dificuldade e avanço.

use chess_training::{Catalog, Difficulty, ExerciseKind, ProfileError, ProfileSet};
use std::collections::BTreeSet;

fn catalog() -> Catalog {
    Catalog::bundled().unwrap()
}

fn done(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

fn set(json: &str) -> Result<ProfileSet, ProfileError> {
    ProfileSet::from_json(json, &catalog())
}

#[test]
fn the_bundled_profiles_are_valid_against_the_catalog() {
    let catalog = catalog();
    let profiles = ProfileSet::bundled(&catalog).unwrap();
    let ids: Vec<_> = profiles.iter().map(|p| p.id.as_str()).collect();
    for expected in ["regras-basicas", "finais-afogamento", "memoria"] {
        assert!(ids.contains(&expected), "falta o Perfil {expected}");
    }
    for p in profiles.iter() {
        assert!(!p.name.trim().is_empty() && !p.description.trim().is_empty());
        assert!(p.steps.len() >= 3, "{}: poucos passos", p.id);
        for step in &p.steps {
            assert!(catalog.get(&step.exercise).is_some(), "{}", step.exercise);
        }
    }
}

#[test]
fn profiles_grow_from_easy_to_hard() {
    let profiles = ProfileSet::bundled(&catalog()).unwrap();
    for p in profiles.iter() {
        let levels: Vec<_> = p.steps.iter().map(|s| s.difficulty).collect();
        let mut sorted = levels.clone();
        sorted.sort();
        assert_eq!(levels, sorted, "{}: a dificuldade não pode regredir", p.id);
        assert_eq!(levels[0], Difficulty::Easy, "{} começa fácil", p.id);
    }
}

#[test]
fn the_memory_profile_uses_position_memory_exercises() {
    let catalog = catalog();
    let profiles = ProfileSet::bundled(&catalog).unwrap();
    let memory = profiles.get("memoria").unwrap();
    assert!(
        memory
            .exercise_ids()
            .any(|id| catalog.get(id).unwrap().kind == ExerciseKind::MemoryPosition)
    );
}

#[test]
fn progress_follows_the_order_of_the_steps() {
    let profiles = ProfileSet::bundled(&catalog()).unwrap();
    let p = profiles.get("finais-afogamento").unwrap();
    let first = p.steps[0].exercise.clone();
    let second = p.steps[1].exercise.clone();
    assert_eq!(p.completed(&done(&[])), 0);
    assert_eq!(p.next_step(&done(&[])).unwrap().exercise, first);
    assert_eq!(p.next_step(&done(&[&first])).unwrap().exercise, second);
    // Pular o primeiro: o próximo continua sendo o primeiro pendente.
    assert_eq!(p.next_step(&done(&[&second])).unwrap().exercise, first);
    assert_eq!(p.completed(&done(&[&second, "fora-do-perfil"])), 1);
}

#[test]
fn a_finished_profile_has_no_next_step() {
    let profiles = ProfileSet::bundled(&catalog()).unwrap();
    let p = profiles.get("memoria").unwrap();
    let all: BTreeSet<String> = p.exercise_ids().map(str::to_owned).collect();
    assert!(p.is_complete(&all));
    assert!(p.next_step(&all).is_none());
    assert_eq!(p.percent(&all), 100);
    assert_eq!(p.percent(&done(&[])), 0);
    assert!(!p.is_complete(&done(&[])));
}

#[test]
fn percent_rounds_to_the_nearest_integer() {
    let profiles = ProfileSet::bundled(&catalog()).unwrap();
    let p = profiles.get("regras-basicas").unwrap(); // 8 passos
    assert_eq!(p.steps.len(), 8);
    let some: Vec<&str> = p.exercise_ids().take(3).collect();
    assert_eq!(p.percent(&done(&some)), 38); // 37,5 arredonda para cima
}

#[test]
fn invalid_files_are_rejected() {
    let bad_exercise = r#"{"format":1,"profiles":[{"id":"x","name":"X","description":"d",
        "steps":[{"exercise":"nao-existe","difficulty":"easy"}]}]}"#;
    assert!(matches!(
        set(bad_exercise),
        Err(ProfileError::UnknownExercise { .. })
    ));
    let empty = r#"{"format":1,"profiles":[{"id":"x","name":"X","description":"d","steps":[]}]}"#;
    assert!(matches!(set(empty), Err(ProfileError::NoSteps(_))));
    let repeated = r#"{"format":1,"profiles":[{"id":"x","name":"X","description":"d",
        "steps":[{"exercise":"xeque-001","difficulty":"easy"},
                 {"exercise":"xeque-001","difficulty":"hard"}]}]}"#;
    assert!(matches!(
        set(repeated),
        Err(ProfileError::RepeatedExercise { .. })
    ));
    let twice = r#"{"format":1,"profiles":[
        {"id":"x","name":"X","description":"d","steps":[{"exercise":"xeque-001","difficulty":"easy"}]},
        {"id":"x","name":"Y","description":"d","steps":[{"exercise":"xeque-001","difficulty":"easy"}]}]}"#;
    assert!(matches!(set(twice), Err(ProfileError::DuplicateId(_))));
    let nameless = r#"{"format":1,"profiles":[{"id":"x","name":" ","description":"d",
        "steps":[{"exercise":"xeque-001","difficulty":"easy"}]}]}"#;
    assert!(matches!(
        set(nameless),
        Err(ProfileError::EmptyField { .. })
    ));
    assert!(matches!(
        set(r#"{"format":9,"profiles":[]}"#),
        Err(ProfileError::UnsupportedFormat(9))
    ));
    assert!(matches!(set("não é json"), Err(ProfileError::Parse(_))));
}
