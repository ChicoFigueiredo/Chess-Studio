#![allow(clippy::unwrap_used)]
//! Mapa de controles (US-28): padrões da DEC-51, conflitos, atribuição e persistência.

use chess_input::controls::{
    Action, AssignError, AssignMode, ControlMap, GestureKind, Input, ModSet, MouseBtn, Trigger,
};

fn key(name: &str) -> Input {
    Input::Key(name.to_owned())
}

fn resolve(map: &ControlMap, input: Input, mods: ModSet) -> Option<Action> {
    map.resolve(&input, mods)
}

/// DEC-51: o mapa de fábrica reproduz os controles fixos do P0.
#[test]
fn defaults_follow_dec_51() {
    let map = ControlMap::default();
    assert!(map.is_default());
    let none = ModSet::NONE;
    // Mouse.
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Left), none),
        Some(Action::Select)
    );
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Left), ModSet::ALT),
        Some(Action::Adjust)
    );
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Middle), none),
        Some(Action::Orbit)
    );
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Middle), ModSet::SHIFT),
        Some(Action::Pan)
    );
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Right), none),
        Some(Action::Pan)
    );
    assert_eq!(resolve(&map, Input::Wheel, none), Some(Action::Zoom));
    // Trackpad.
    assert_eq!(
        resolve(&map, Input::Gesture(GestureKind::TwoFinger), none),
        Some(Action::Orbit)
    );
    assert_eq!(
        resolve(&map, Input::Gesture(GestureKind::TwoFinger), ModSet::SHIFT),
        Some(Action::Pan)
    );
    assert_eq!(
        resolve(&map, Input::Gesture(GestureKind::Pinch), none),
        Some(Action::Zoom)
    );
    // Teclado.
    for (name, action) in [
        ("Escape", Action::Cancel),
        ("Space", Action::ClockPress),
        ("Digit1", Action::ViewTop),
        ("Digit2", Action::ViewOblique),
        ("Digit3", Action::ViewStandard),
        ("Digit4", Action::ViewInverted),
        ("Numpad1", Action::ViewTop),
        ("Numpad4", Action::ViewInverted),
        ("Digit0", Action::ResetCamera),
        ("Numpad0", Action::ResetCamera),
        ("Tab", Action::CyclePresentation),
        ("F11", Action::ToggleFullscreen),
        ("F10", Action::ToggleFocus),
    ] {
        assert_eq!(resolve(&map, key(name), none), Some(action), "{name}");
    }
    assert_eq!(resolve(&map, key("KeyQ"), none), None);
}

#[test]
fn the_most_specific_binding_wins_and_extra_modifiers_do_not_block() {
    let map = ControlMap::default();
    let both = ModSet {
        ctrl: false,
        shift: true,
        alt: true,
    };
    // Shift+Alt+Esquerdo: o Ajuste (Alt) é mais específico que a seleção.
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Left), both),
        Some(Action::Adjust)
    );
    // Shift+Esquerdo ainda seleciona; Shift+1 ainda troca a Vista.
    assert_eq!(
        resolve(&map, Input::Mouse(MouseBtn::Left), ModSet::SHIFT),
        Some(Action::Select)
    );
    assert_eq!(
        resolve(&map, key("Digit1"), ModSet::SHIFT),
        Some(Action::ViewTop)
    );
}

#[test]
fn assigning_replaces_or_adds() {
    let mut map = ControlMap::default();
    map.assign(Action::ViewTop, Trigger::key("KeyT"), AssignMode::Replace)
        .unwrap();
    assert_eq!(map.triggers(Action::ViewTop), [Trigger::key("KeyT")]);
    assert_eq!(resolve(&map, key("Digit1"), ModSet::NONE), None);
    map.assign(Action::ViewTop, Trigger::key("KeyY"), AssignMode::Add)
        .unwrap();
    assert_eq!(map.triggers(Action::ViewTop).len(), 2);
    assert!(!map.is_default());
    assert!(!map.is_action_default(Action::ViewTop));
    assert!(map.is_action_default(Action::ViewOblique));
}

#[test]
fn a_trigger_already_used_by_another_action_is_a_conflict_and_changes_nothing() {
    let mut map = ControlMap::default();
    let before = map.clone();
    let err = map
        .assign(Action::ViewTop, Trigger::key("Digit2"), AssignMode::Replace)
        .unwrap_err();
    assert_eq!(err, AssignError::Conflict(Action::ViewOblique));
    assert_eq!(map, before);
    // O mesmo atalho com outro modificador não conflita.
    map.assign(
        Action::ViewTop,
        Trigger::key("Digit2").with_mods(ModSet::SHIFT),
        AssignMode::Add,
    )
    .unwrap();
    // Reatribuir a própria ação ao que ela já tem não é conflito.
    map.assign(Action::ViewOblique, Trigger::key("Digit2"), AssignMode::Add)
        .unwrap();
}

#[test]
fn stealing_moves_the_trigger_to_the_new_action() {
    let mut map = ControlMap::default();
    let stolen = map
        .assign_stealing(Action::ViewTop, Trigger::key("Digit2"), AssignMode::Replace)
        .unwrap();
    assert_eq!(stolen, Some(Action::ViewOblique));
    assert_eq!(
        resolve(&map, key("Digit2"), ModSet::NONE),
        Some(Action::ViewTop)
    );
    assert_eq!(
        map.triggers(Action::ViewOblique),
        [Trigger::key("Numpad2")]
    );
}

#[test]
fn input_kind_must_suit_the_action() {
    let mut map = ControlMap::default();
    assert_eq!(
        map.assign(Action::Orbit, Trigger::key("KeyO"), AssignMode::Add),
        Err(AssignError::Unsupported)
    );
    assert_eq!(
        map.assign(
            Action::ViewTop,
            Trigger::mouse(MouseBtn::Back),
            AssignMode::Add
        ),
        Err(AssignError::Unsupported)
    );
    assert_eq!(
        map.assign(
            Action::Zoom,
            Trigger::new(Input::Gesture(GestureKind::TwoFinger), ModSet::NONE),
            AssignMode::Add
        ),
        Err(AssignError::Unsupported)
    );
    map.assign(
        Action::Orbit,
        Trigger::mouse(MouseBtn::Back),
        AssignMode::Add,
    )
    .unwrap();
}

#[test]
fn selection_never_loses_its_last_trigger() {
    let mut map = ControlMap::default();
    assert_eq!(map.remove(Action::Select, 0), Err(AssignError::WouldLeaveEmpty));
    assert_eq!(
        map.assign_stealing(
            Action::Orbit,
            Trigger::mouse(MouseBtn::Left),
            AssignMode::Add
        ),
        Err(AssignError::WouldLeaveEmpty)
    );
    // Outras ações podem ficar sem atalho.
    map.remove(Action::Cancel, 0).unwrap();
    assert!(map.triggers(Action::Cancel).is_empty());
    assert_eq!(resolve(&map, key("Escape"), ModSet::NONE), None);
}

#[test]
fn resetting_restores_defaults_even_when_another_action_took_them() {
    let mut map = ControlMap::default();
    map.assign_stealing(Action::ViewTop, Trigger::key("Digit2"), AssignMode::Replace)
        .unwrap();
    map.reset_action(Action::ViewOblique);
    assert_eq!(
        map.triggers(Action::ViewOblique),
        [Trigger::key("Digit2"), Trigger::key("Numpad2")]
    );
    assert!(!map.triggers(Action::ViewTop).contains(&Trigger::key("Digit2")));
    map.reset_all();
    assert!(map.is_default());
}

#[test]
fn trigger_text_round_trips() {
    for t in [
        Trigger::key("Digit1"),
        Trigger::key("F11").with_mods(ModSet {
            ctrl: true,
            shift: true,
            alt: true,
        }),
        Trigger::mouse(MouseBtn::Middle).with_mods(ModSet::SHIFT),
        Trigger::mouse(MouseBtn::Forward),
        Trigger::new(Input::Wheel, ModSet::ALT),
        Trigger::new(Input::Gesture(GestureKind::Pinch), ModSet::NONE),
        Trigger::new(Input::Gesture(GestureKind::TwoFinger), ModSet::SHIFT),
    ] {
        assert_eq!(t.to_string().parse::<Trigger>().unwrap(), t, "{t}");
    }
    assert!("Mouse:Nope".parse::<Trigger>().is_err());
    assert!("Key:".parse::<Trigger>().is_err());
    assert!("lixo".parse::<Trigger>().is_err());
}

fn roundtrip(map: &ControlMap) -> ControlMap {
    let entries = map.to_entries();
    ControlMap::from_entries(entries.iter().map(|(k, v)| (k.as_str(), v.clone())))
}

#[test]
fn the_map_serializes_and_loads_back() {
    let mut map = ControlMap::default();
    assert_eq!(roundtrip(&map), map);
    map.assign(Action::ViewTop, Trigger::key("KeyT"), AssignMode::Replace)
        .unwrap();
    map.assign(
        Action::Pan,
        Trigger::mouse(MouseBtn::Back),
        AssignMode::Add,
    )
    .unwrap();
    map.remove(Action::Cancel, 0).unwrap();
    assert_eq!(roundtrip(&map), map);
}

#[test]
fn loading_is_tolerant_of_missing_unknown_and_broken_entries() {
    let loaded = ControlMap::from_entries([
        ("view-top", vec!["Key:KeyT".to_owned()]),
        ("acao-do-futuro", vec!["Key:KeyZ".to_owned()]),
        ("orbit", vec!["Key:KeyO".to_owned()]), // tipo errado -> padrão
        ("pan", vec!["isto não é atalho".to_owned()]), // ilegível -> padrão
        ("select", vec![]),                     // Seleção sem atalho -> padrão
    ]);
    let defaults = ControlMap::default();
    assert_eq!(loaded.triggers(Action::ViewTop), [Trigger::key("KeyT")]);
    assert_eq!(loaded.triggers(Action::Orbit), defaults.triggers(Action::Orbit));
    assert_eq!(loaded.triggers(Action::Pan), defaults.triggers(Action::Pan));
    assert_eq!(
        loaded.triggers(Action::Select),
        defaults.triggers(Action::Select)
    );
    // Ausente: padrão.
    assert_eq!(
        loaded.triggers(Action::Cancel),
        defaults.triggers(Action::Cancel)
    );
}

#[test]
fn a_stored_map_with_conflicts_is_refused_as_a_whole() {
    let loaded = ControlMap::from_entries([
        ("view-top", vec!["Key:KeyT".to_owned()]),
        ("view-oblique", vec!["Key:KeyT".to_owned()]),
    ]);
    assert!(loaded.is_default());
}
