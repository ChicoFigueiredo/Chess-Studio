//! O Modo de estudo e a Trava de fair play (#325): o estudo só existe enquanto é permitido.
//! Mora aqui, dentro de `study_mode`, para forjar o estado que nenhum caminho público produz
//! (uma recusa que passa a valer com o estudo em curso).

#![allow(clippy::unwrap_used)]

use super::*;
use crate::followed::end_study_when_refused;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::World;

fn studying_session() -> Session {
    let mut session = Session::new();
    session.study_here().unwrap();
    assert!(session.study_play("e2e4".parse().unwrap()));
    assert!(session.studying());
    session
}

#[test]
fn the_per_frame_check_keeps_an_allowed_study_alone() {
    let mut world = World::new();
    world.insert_resource(studying_session());
    world.run_system_once(end_study_when_refused).unwrap();
    assert!(world.resource::<Session>().studying());
}

#[test]
fn the_per_frame_check_ends_a_study_that_stops_being_allowed() {
    let mut session = studying_session();
    // O Treino toma a Sessão (a recusa passa a valer com o estudo em curso).
    session.exercise = true;
    assert!(session.study_refusal().is_some());
    assert!(session.studying());
    let mut world = World::new();
    world.insert_resource(session);
    world.run_system_once(end_study_when_refused).unwrap();
    let session = world.resource::<Session>();
    assert!(!session.studying(), "o estudo se encerra");
    assert!(session.review().is_none(), "volta à posição da Partida");
}
