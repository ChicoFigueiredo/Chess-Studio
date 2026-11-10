use super::inventory::{Destination, INVENTORY};
use super::*;
use crate::actions::{ActionRequested, press, test_app};
use bevy::ecs::system::RunSystemOnce;

/// App mínimo com todos os recursos que o diálogo edita.
fn options_app() -> App {
    let mut app = test_app();
    app.init_resource::<GraphicsQuality>()
        .init_resource::<UltraEffects>()
        .init_resource::<GpuSupport>()
        .init_resource::<EngineUi>()
        .init_resource::<HudSettings>()
        .init_resource::<LocalProfiles>()
        .init_resource::<InterfaceProfiles>();
    app
}

fn apply(app: &mut App, change: OptionChange) {
    app.world_mut()
        .run_system_once(move |mut env: OptionsEnv| env.apply(change))
        .unwrap();
}

// --- inventário ---------------------------------------------------------------------------

#[test]
fn every_inventory_item_has_a_destination() {
    for item in INVENTORY {
        assert!(!item.dests.is_empty(), "{} não tem destino", item.name);
        for dest in item.dests {
            match dest {
                Destination::Actions(actions) => assert!(!actions.is_empty(), "{}", item.name),
                Destination::Surface(name) => assert!(!name.is_empty(), "{}", item.name),
                Destination::Option(_) => {}
            }
        }
    }
}

#[test]
fn every_option_destination_is_a_real_row_with_texts() {
    let i18n = I18n::pt_br();
    for item in INVENTORY {
        for dest in item.dests {
            if let Destination::Option(id) = dest {
                assert!(OptionId::ALL.contains(id), "{id:?}");
            }
        }
    }
    for id in OptionId::ALL {
        let (label, hint) = id.texts();
        for key in [label, hint] {
            assert_ne!(i18n.t(key), key, "chave i18n ausente: {key}");
        }
        assert!(Category::ALL.contains(&id.category()));
    }
    for category in Category::ALL {
        assert_ne!(i18n.t(category.key()), category.key());
        assert!(
            OptionId::ALL.iter().any(|id| id.category() == category),
            "{category:?} sem linhas"
        );
    }
}

/// Chaves i18n usadas literalmente no corpo de `fn_name` em `source`.
fn keys_in(source: &str, fn_name: &str) -> Vec<String> {
    let start = source
        .find(&format!("fn {fn_name}("))
        .unwrap_or_else(|| panic!("fn {fn_name} não encontrada"));
    let body = &source[start..];
    let end = body.find("\n}\n").map_or(body.len(), |e| e + 3);
    let body = &body[..end];
    let mut keys = Vec::new();
    for marker in [".t(\"", ".t_with(\""] {
        let mut rest = body;
        while let Some(at) = rest.find(marker) {
            rest = &rest[at + marker.len()..];
            if let Some(close) = rest.find('"') {
                keys.push(rest[..close].to_owned());
            }
        }
    }
    keys
}

/// Nada se perde: todo texto de controle das seções do painel esquerdo precisa estar no
/// inventário. Quem acrescentar um controle ao painel sem destino quebra este teste.
#[test]
fn every_left_panel_item_in_the_source_is_in_the_inventory() {
    let sections: [(&str, &str, &str); 8] = [
        (include_str!("../ui.rs"), "info_panel", "ui.rs::info_panel"),
        (
            include_str!("../ui.rs"),
            "game_section",
            "ui.rs::game_section",
        ),
        (
            include_str!("../dock.rs"),
            "panels_section",
            "dock.rs::panels_section",
        ),
        (
            include_str!("../focus.rs"),
            "view_section",
            "focus.rs::view_section",
        ),
        (
            include_str!("../profiles.rs"),
            "profiles_section",
            "profiles.rs::profiles_section",
        ),
        (
            include_str!("../local_profiles.rs"),
            "section",
            "local_profiles.rs::section",
        ),
        (
            include_str!("../controls.rs"),
            "controls_section",
            "controls.rs::controls_section",
        ),
        (
            include_str!("../engine.rs"),
            "engine_section",
            "engine.rs::engine_section",
        ),
    ];
    let known: Vec<&str> = INVENTORY
        .iter()
        .flat_map(|i| i.keys.iter().copied())
        .collect();
    let mut missing = Vec::new();
    for (source, fn_name, origin) in sections {
        let keys = keys_in(source, fn_name);
        assert!(!keys.is_empty(), "{origin}: nenhuma chave lida");
        for key in keys {
            if !known.contains(&key.as_str()) {
                missing.push(format!("{origin}: {key}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "sem destino no inventário: {missing:#?}"
    );
}

#[test]
fn inventory_keys_exist_in_the_translations() {
    let i18n = I18n::pt_br();
    for item in INVENTORY {
        for key in item.keys {
            assert_ne!(i18n.t(key), *key, "{}: chave i18n ausente {key}", item.name);
        }
    }
}

// --- as mudanças vão para o mesmo recurso do controle antigo ---------------------------------

#[test]
fn a_dialog_change_updates_the_same_resource_the_old_control_did() {
    let mut app = options_app();
    apply(&mut app, OptionChange::Theme(ThemeMode::Light));
    apply(&mut app, OptionChange::Density(Density::Compact));
    apply(&mut app, OptionChange::Scale(125));
    let appearance = *app.world().resource::<Appearance>();
    assert_eq!(appearance.theme, ThemeMode::Light);
    assert_eq!(appearance.density, Density::Compact);
    assert_eq!(appearance.scale.percent(), 125);

    apply(&mut app, OptionChange::Quality(GraphicsQuality::Ultra));
    apply(&mut app, OptionChange::Effects(false));
    assert_eq!(
        *app.world().resource::<GraphicsQuality>(),
        GraphicsQuality::Ultra
    );
    assert!(!app.world().resource::<UltraEffects>().0);

    apply(&mut app, OptionChange::Coordinates(false));
    apply(&mut app, OptionChange::Hover(false));
    assert!(!app.world().resource::<BoardCoordinates>().0);
    assert!(!app.world().resource::<HoverHighlight>().0);

    apply(&mut app, OptionChange::MoveMethod(MoveMethod::Drag));
    assert_eq!(
        app.world().resource::<Session>().move_method(),
        MoveMethod::Drag
    );
    apply(&mut app, OptionChange::CameraView(CameraView::Top));
    assert_eq!(
        app.world().resource::<CameraRig>().view,
        Some(CameraView::Top)
    );

    apply(&mut app, OptionChange::Aids(AidSettings::physical()));
    assert_eq!(
        *app.world().resource::<AidSettings>(),
        AidSettings::physical()
    );
    apply(&mut app, OptionChange::Hud(HudSettings::study()));
    assert_eq!(*app.world().resource::<HudSettings>(), HudSettings::study());

    apply(&mut app, OptionChange::EngineElo(2000));
    apply(&mut app, OptionChange::EngineSide(Side::Black));
    let engine = app.world().resource::<EngineUi>();
    assert_eq!(engine.elo(), 2000);
    assert_eq!(engine.user_side(), Side::Black);
}

#[test]
fn the_dialog_and_the_actions_drive_the_same_resource() {
    let mut app = options_app();
    // O atalho (Ação) e o diálogo mexem no mesmo `BoardCoordinates`.
    press(&mut app, KeyCode::KeyC);
    assert!(!app.world().resource::<BoardCoordinates>().0);
    apply(&mut app, OptionChange::Coordinates(true));
    assert!(app.world().resource::<BoardCoordinates>().0);
}

#[test]
fn an_unchanged_value_does_not_mark_the_resource_as_changed() {
    let mut app = options_app();
    app.update();
    let tick = |app: &App| {
        (
            app.world()
                .resource_ref::<Appearance>()
                .last_changed()
                .get(),
            app.world()
                .resource_ref::<GraphicsQuality>()
                .last_changed()
                .get(),
        )
    };
    let before = tick(&app);
    apply(&mut app, OptionChange::Theme(ThemeMode::default()));
    apply(&mut app, OptionChange::Quality(GraphicsQuality::default()));
    assert_eq!(tick(&app), before);
    // Controle: uma mudança de verdade marca o recurso.
    apply(&mut app, OptionChange::Theme(ThemeMode::Dark));
    assert_ne!(tick(&app), before);
}

#[test]
fn the_engine_elo_stays_inside_the_allowed_range() {
    let mut app = options_app();
    apply(&mut app, OptionChange::EngineElo(99_999));
    assert_eq!(app.world().resource::<EngineUi>().elo(), MAX_ELO);
    apply(&mut app, OptionChange::EngineElo(1));
    assert_eq!(app.world().resource::<EngineUi>().elo(), MIN_ELO);
}

// --- Restaurar padrões ---------------------------------------------------------------------

fn change_everything(app: &mut App) {
    apply(app, OptionChange::Theme(ThemeMode::Dark));
    apply(app, OptionChange::Scale(150));
    apply(app, OptionChange::Quality(GraphicsQuality::Economic));
    apply(app, OptionChange::Coordinates(false));
    apply(app, OptionChange::Aids(AidSettings::physical()));
    apply(app, OptionChange::EngineElo(2400));
}

#[test]
fn restore_defaults_asks_for_confirmation_before_changing_anything() {
    let mut app = options_app();
    change_everything(&mut app);

    // Sem pedir antes, confirmar não faz nada.
    let restored = app
        .world_mut()
        .run_system_once(|mut env: OptionsEnv| OptionsUi::default().confirm_reset(&mut env))
        .unwrap();
    assert!(!restored);
    assert_eq!(app.world().resource::<Appearance>().scale.percent(), 150);

    // O primeiro clique só pede a confirmação.
    let mut ui = OptionsUi::default();
    ui.ask_reset();
    assert!(ui.is_confirming_reset());
    assert_eq!(app.world().resource::<Appearance>().scale.percent(), 150);
    assert_eq!(app.world().resource::<EngineUi>().elo(), 2400);

    // Cancelar mantém tudo como está.
    ui.cancel_reset();
    assert!(!ui.is_confirming_reset());
    assert_eq!(app.world().resource::<Appearance>().scale.percent(), 150);

    // Pedir e confirmar restaura os valores de fábrica.
    ui.ask_reset();
    let restored = app
        .world_mut()
        .run_system_once(|mut env: OptionsEnv| {
            let mut ui = OptionsUi::default();
            ui.ask_reset();
            ui.confirm_reset(&mut env)
        })
        .unwrap();
    assert!(restored);
    let world = app.world();
    assert_eq!(*world.resource::<Appearance>(), Appearance::default());
    assert_eq!(
        *world.resource::<GraphicsQuality>(),
        GraphicsQuality::default()
    );
    assert!(world.resource::<BoardCoordinates>().0);
    assert_eq!(*world.resource::<AidSettings>(), AidSettings::default());
    assert_eq!(world.resource::<EngineUi>().elo(), 1500);
}

#[test]
fn confirming_a_reset_closes_the_confirmation() {
    let mut ui = OptionsUi {
        open: true,
        ..OptionsUi::default()
    };
    ui.ask_reset();
    ui.close();
    assert!(!ui.open);
    assert!(!ui.is_confirming_reset());
}

// --- abertura pela Ação ------------------------------------------------------------------------

#[test]
fn the_open_options_action_opens_the_dialog_and_ctrl_comma_reaches_it() {
    let mut app = options_app();
    assert!(!app.world().resource::<OptionsUi>().open);
    app.world_mut()
        .resource_mut::<Messages<ActionRequested>>()
        .write(ActionRequested(chess_input::controls::Action::OpenOptions));
    app.update();
    assert!(app.world().resource::<OptionsUi>().open);
}

#[test]
fn ctrl_comma_opens_the_dialog() {
    let mut app = options_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    press(&mut app, KeyCode::Comma);
    assert!(app.world().resource::<OptionsUi>().open);
}

#[test]
fn opening_options_from_the_palette_closes_it_and_the_chord_fires_while_typing() {
    assert!(chess_input::controls::Action::OpenOptions.fires_while_typing());
    let mut app = options_app();
    app.world_mut()
        .resource_mut::<crate::palette::PaletteState>()
        .open();
    app.world_mut()
        .resource_mut::<Messages<ActionRequested>>()
        .write(ActionRequested(chess_input::controls::Action::OpenOptions));
    app.update();
    assert!(app.world().resource::<OptionsUi>().open);
    assert!(!app.world().resource::<crate::palette::PaletteState>().open);
}
