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

/// Troca única (#167): o painel esquerdo antigo não existe mais, então cada item do inventário
/// precisa ter um destino alcançável na interface nova. Uma Ação é alcançável quando está na
/// Paleta (toda Ação que não é gesto) e num grupo do Ribbon (ou nos bastidores, na aba Arquivo).
#[test]
fn every_inventory_action_is_reachable_in_the_ribbon_and_the_palette() {
    use crate::palette::entries;
    use crate::ribbon::model::{actions_of, visible_tabs};
    use chess_input::actions::{ActionKind, RibbonTab, meta};
    use chess_input::controls::Action;

    let palette: Vec<Action> = entries(
        &I18n::pt_br(),
        &Controls::default(),
        &chess_input::actions::ActionContext::default(),
        &crate::panel_sets::PanelSets::default(),
    )
    .into_iter()
    .filter_map(|e| e.action())
    .collect();
    let in_ribbon: Vec<Action> = visible_tabs(true)
        .into_iter()
        .flat_map(actions_of)
        .collect();
    let mut gaps = Vec::new();
    for item in INVENTORY {
        for dest in item.dests {
            let Destination::Actions(actions) = dest else {
                continue;
            };
            for action in *actions {
                let m = meta(*action);
                let ribbon_tab_without_body = m.tab == Some(RibbonTab::File);
                if m.kind == ActionKind::Gesture {
                    gaps.push(format!("{}: {action:?} é só gesto", item.name));
                } else if !palette.contains(action) {
                    gaps.push(format!("{}: {action:?} fora da Paleta", item.name));
                } else if !in_ribbon.contains(action) && !ribbon_tab_without_body {
                    gaps.push(format!("{}: {action:?} fora do Ribbon", item.name));
                }
            }
        }
    }
    assert!(gaps.is_empty(), "sem caminho na interface nova: {gaps:#?}");
}

/// Todo painel do dock tem uma Ação de alternar (Ribbon e Paleta): nada fica só no painel antigo.
#[test]
fn every_dock_panel_has_a_toggle_action() {
    use chess_input::controls::Action;
    let panels: Vec<&Action> = INVENTORY
        .iter()
        .flat_map(|i| i.dests.iter())
        .filter_map(|d| match d {
            Destination::Actions(a) => Some(*a),
            _ => None,
        })
        .flatten()
        .collect();
    for action in [
        Action::PanelNotation,
        Action::PanelClock,
        Action::PanelCaptured,
        Action::PanelStudy,
        Action::PanelAids,
        Action::PanelReview,
        Action::PanelArbitration,
        Action::PanelTraining,
        Action::PanelAnalysis,
        Action::PanelLibrary,
        Action::PanelOpenings,
        Action::PanelScoresheet,
        Action::PanelBoard2d,
        Action::PanelEngines,
    ] {
        assert!(panels.contains(&&action), "{action:?} fora do inventário");
    }
}

/// As superfícies prometidas à barra de status existem de fato: casa sob o cursor e Vista.
#[test]
fn the_status_bar_text_covers_its_inventory_surfaces() {
    use crate::status_bar::{StatusSnapshot, status_text};
    let snapshot = StatusSnapshot {
        profile: "Estudo e análise".into(),
        engine_name: None,
        engine_consented: false,
        side_to_move: Side::White,
        move_number: 1,
        game_over: false,
        in_check: false,
        low_clock: None,
        fps: None,
        scale_percent: 100,
        hovered: "e4".parse().ok(),
        view: Some(CameraView::Oblique),
    };
    let text = status_text(&snapshot, &I18n::pt_br());
    assert_eq!(text.hover.as_deref(), Some("Casa: e4"));
    assert_eq!(text.view, "Vista: Oblíqua");
    assert!(text.turn.contains("Vez das brancas"));
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
