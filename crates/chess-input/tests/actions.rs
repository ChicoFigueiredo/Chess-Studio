//! A lista única de Ações (DS-14): metadados, KeyTips, atalhos e estado habilitada/marcada.

use chess_input::CameraView;
use chess_input::actions::{ActionContext, ActionKind, PresentationId, RibbonTab, meta, state};
use chess_input::controls::{Action, ControlMap};
use std::collections::HashSet;

fn assisted() -> ActionContext {
    ActionContext {
        game_running: true,
        can_undo: true,
        engine_consented: true,
        ..ActionContext::default()
    }
}

fn fide() -> ActionContext {
    ActionContext {
        competitive: true,
        competitive_in_progress: true,
        ..assisted()
    }
}

#[test]
fn every_action_has_label_description_and_icon() {
    for action in Action::ALL {
        let m = meta(action);
        assert_eq!(m.label, format!("action-{}", action.id()), "{action:?}");
        assert_eq!(m.description, format!("action-{}-desc", action.id()));
        assert!(!m.icon.is_empty(), "{action:?} sem ícone");
        assert!(
            m.icon
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{action:?}: ícone `{}` não está em kebab-case",
            m.icon
        );
    }
}

#[test]
fn keytips_are_unique_within_a_tab_and_present_on_ribbon_actions() {
    for tab in RibbonTab::ALL {
        let mut seen = HashSet::new();
        for action in Action::ALL {
            let m = meta(action);
            if m.tab != Some(tab) {
                continue;
            }
            assert!(!m.keytip.is_empty(), "{action:?} está no Ribbon sem KeyTip");
            assert!(
                m.keytip
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
                "{action:?}: KeyTip `{}` deve ser maiúsculo",
                m.keytip
            );
            assert!(
                seen.insert(m.keytip),
                "KeyTip `{}` repetido em {tab:?}",
                m.keytip
            );
        }
        assert!(!seen.is_empty(), "aba {tab:?} vazia");
    }
}

#[test]
fn gesture_only_actions_stay_out_of_the_ribbon() {
    for action in Action::ALL {
        let m = meta(action);
        assert_eq!(m.tab.is_none(), m.group.is_none(), "{action:?}");
        assert_eq!(m.tab.is_none(), m.keytip.is_empty(), "{action:?}");
        assert_eq!(m.tab.is_none(), action.is_gesture_only(), "{action:?}");
    }
}

#[test]
fn tabs_and_groups_have_fluent_ids() {
    for tab in RibbonTab::ALL {
        assert!(tab.label().starts_with("ribbon-tab-"));
    }
    for action in Action::ALL {
        if let Some(group) = meta(action).group {
            assert!(group.starts_with("ribbon-group-"), "{action:?}: {group}");
        }
    }
}

#[test]
fn default_shortcuts_have_no_conflicts_and_come_from_the_control_map() {
    let map = ControlMap::default();
    let mut seen = HashSet::new();
    for action in Action::ALL {
        for t in map.triggers(action) {
            assert!(seen.insert(t.clone()), "atalho {t} repetido ({action:?})");
            assert!(action.accepts(&t.input), "{action:?} não aceita {t}");
        }
    }
    assert_eq!(map.triggers(Action::Undo)[0].to_string(), "Ctrl+Key:KeyZ");
    assert_eq!(
        map.triggers(Action::OpenPalette)[0].to_string(),
        "Ctrl+Key:KeyK"
    );
    assert_eq!(
        map.triggers(Action::ToggleRibbonCollapse)[0].to_string(),
        "Ctrl+Key:F1"
    );
}

#[test]
fn ids_stay_unique_and_round_trip() {
    let mut ids = HashSet::new();
    for action in Action::ALL {
        assert!(ids.insert(action.id()));
        assert_eq!(Action::from_id(action.id()), Some(action));
    }
}

#[test]
fn undo_is_disabled_with_a_reason_in_fide_simulation() {
    let s = state(Action::Undo, &fide());
    assert!(!s.is_enabled());
    assert_eq!(s.disabled_reason, Some("reason-assisted-only"));
    assert!(state(Action::Undo, &assisted()).is_enabled());
}

#[test]
fn undo_without_history_says_nothing_to_undo() {
    let ctx = ActionContext {
        can_undo: false,
        ..assisted()
    };
    assert_eq!(
        state(Action::Undo, &ctx).disabled_reason,
        Some("reason-nothing-to-undo")
    );
    assert_eq!(
        state(Action::Redo, &ctx).disabled_reason,
        Some("reason-nothing-to-redo")
    );
}

#[test]
fn game_actions_need_a_running_game() {
    let over = ActionContext {
        game_running: false,
        has_clock: true,
        ..assisted()
    };
    for a in [
        Action::Resign,
        Action::OfferDraw,
        Action::PauseClock,
        Action::ClockPress,
    ] {
        assert_eq!(
            state(a, &over).disabled_reason,
            Some("reason-no-game-running"),
            "{a:?}"
        );
        let running = ActionContext {
            has_clock: true,
            ..assisted()
        };
        assert!(state(a, &running).is_enabled(), "{a:?}");
    }
}

#[test]
fn clock_actions_need_a_clock() {
    let ctx = assisted();
    assert_eq!(
        state(Action::PauseClock, &ctx).disabled_reason,
        Some("reason-no-clock")
    );
    assert_eq!(
        state(Action::ClockPress, &ctx).disabled_reason,
        Some("reason-no-clock")
    );
}

#[test]
fn pause_is_a_toggle_that_reflects_the_pause() {
    let ctx = ActionContext {
        has_clock: true,
        paused: true,
        ..assisted()
    };
    assert_eq!(state(Action::PauseClock, &ctx).checked, Some(true));
}

#[test]
fn offering_a_draw_waits_for_the_pending_offer() {
    let ctx = ActionContext {
        draw_offer_pending: true,
        ..assisted()
    };
    assert_eq!(
        state(Action::OfferDraw, &ctx).disabled_reason,
        Some("reason-draw-offer-pending")
    );
}

#[test]
fn claiming_a_draw_needs_a_claimable_draw() {
    assert_eq!(
        state(Action::ClaimDraw, &assisted()).disabled_reason,
        Some("reason-no-claimable-draw")
    );
    let ctx = ActionContext {
        claimable_draw: true,
        ..assisted()
    };
    assert!(state(Action::ClaimDraw, &ctx).is_enabled());
}

#[test]
fn analysis_is_disabled_without_engine_consent() {
    let ctx = ActionContext {
        engine_consented: false,
        ..assisted()
    };
    let s = state(Action::ToggleAnalysis, &ctx);
    assert_eq!(s.disabled_reason, Some("reason-engine-consent"));
    assert!(state(Action::ToggleAnalysis, &assisted()).is_enabled());
}

#[test]
fn analysis_is_locked_in_competitive_games_and_during_training() {
    assert_eq!(
        state(Action::ToggleAnalysis, &fide()).disabled_reason,
        Some("reason-competitive-lock")
    );
    let training = ActionContext {
        training_on_screen: true,
        ..assisted()
    };
    assert_eq!(
        state(Action::ToggleAnalysis, &training).disabled_reason,
        Some("reason-training-lock")
    );
}

#[test]
fn analysis_toggle_reflects_the_state() {
    let on = ActionContext {
        analysis_on: true,
        ..assisted()
    };
    assert_eq!(state(Action::ToggleAnalysis, &on).checked, Some(true));
    assert_eq!(
        state(Action::ToggleAnalysis, &assisted()).checked,
        Some(false)
    );
}

#[test]
fn view_toggles_reflect_the_app_state() {
    let ctx = ActionContext {
        fullscreen: true,
        focus: false,
        hud_enabled: true,
        coordinates: false,
        hover_highlight: true,
        ..assisted()
    };
    assert_eq!(state(Action::ToggleFullscreen, &ctx).checked, Some(true));
    assert_eq!(state(Action::ToggleFocus, &ctx).checked, Some(false));
    assert_eq!(state(Action::ToggleHud, &ctx).checked, Some(true));
    assert_eq!(state(Action::ToggleCoordinates, &ctx).checked, Some(false));
    assert_eq!(
        state(Action::ToggleHoverHighlight, &ctx).checked,
        Some(true)
    );
    assert_eq!(
        state(Action::ToggleRibbonCollapse, &ctx).checked,
        Some(false)
    );
}

#[test]
fn presentations_and_views_are_radio_choices() {
    let ctx = ActionContext {
        presentation: PresentationId::Technical2d,
        camera_view: Some(CameraView::Top),
        ..assisted()
    };
    assert_eq!(state(Action::PresentTechnical, &ctx).checked, Some(true));
    assert_eq!(
        state(Action::PresentProfessional, &ctx).checked,
        Some(false)
    );
    assert_eq!(state(Action::ViewTop, &ctx).checked, Some(true));
    assert_eq!(state(Action::ViewInverted, &ctx).checked, Some(false));
    let free = ActionContext {
        camera_view: None,
        ..ctx
    };
    assert_eq!(state(Action::ViewTop, &free).checked, Some(false));
}

#[test]
fn only_toggles_and_choices_are_checkable() {
    for a in Action::ALL {
        let checkable = matches!(meta(a).kind, ActionKind::Toggle | ActionKind::Choice);
        assert_eq!(state(a, &assisted()).checked.is_some(), checkable, "{a:?}");
    }
}

#[test]
fn panel_toggles_follow_visibility_and_the_competitive_lock() {
    let mut ctx = assisted();
    ctx.visible_panels.insert("Notation".into());
    assert_eq!(state(Action::PanelNotation, &ctx).checked, Some(true));
    assert_eq!(state(Action::PanelClock, &ctx).checked, Some(false));
    // Painéis de dicas e análise ficam travados na Partida competitiva em andamento.
    assert_eq!(
        state(Action::PanelAnalysis, &fide()).disabled_reason,
        Some("reason-competitive-lock")
    );
    assert!(state(Action::PanelNotation, &fide()).is_enabled());
    assert_eq!(Action::PanelNotation.panel(), Some(("Notation", false)));
    assert_eq!(Action::PanelAnalysis.panel(), Some(("Analysis", true)));
}

#[test]
fn flipping_the_board_works_without_the_2d_panel() {
    // Vira a Vista 3D e o 2D técnico também (#162): não depende do Painel Tabuleiro 2D.
    assert!(state(Action::FlipBoard, &assisted()).is_enabled());
    let mut ctx = assisted();
    ctx.board_flipped = true;
    let s = state(Action::FlipBoard, &ctx);
    assert!(s.is_enabled());
    assert_eq!(s.checked, Some(true));
}

#[test]
fn float_and_dock_depend_on_the_panels() {
    assert_eq!(
        state(Action::FloatPanel, &assisted()).disabled_reason,
        Some("reason-no-docked-panel")
    );
    assert_eq!(
        state(Action::DockPanel, &assisted()).disabled_reason,
        Some("reason-no-floating-panel")
    );
    let ctx = ActionContext {
        has_docked_panel: true,
        has_floating_panel: true,
        ..assisted()
    };
    assert!(state(Action::FloatPanel, &ctx).is_enabled());
    assert!(state(Action::DockPanel, &ctx).is_enabled());
}

#[test]
fn surfaces_that_do_not_exist_yet_are_disabled_with_a_reason() {
    {
        let a = Action::DetachPanel;
        let ctx = ActionContext {
            has_docked_panel: true,
            ..assisted()
        };
        assert_eq!(
            state(a, &ctx).disabled_reason,
            Some("reason-coming-soon"),
            "{a:?}"
        );
    }
}

#[test]
fn the_palette_action_is_enabled() {
    for ctx in [assisted(), fide(), ActionContext::default()] {
        assert!(state(Action::OpenPalette, &ctx).is_enabled());
    }
}

#[test]
fn open_options_is_enabled_in_every_context() {
    for ctx in [assisted(), fide(), ActionContext::default()] {
        assert!(state(Action::OpenOptions, &ctx).is_enabled());
    }
}

#[test]
fn every_disabled_reason_is_a_reason_id() {
    let contexts = [assisted(), fide(), ActionContext::default()];
    for ctx in &contexts {
        for a in Action::ALL {
            if let Some(r) = state(a, ctx).disabled_reason {
                assert!(r.starts_with("reason-"), "{a:?}: {r}");
            }
        }
    }
}

#[test]
fn camera_views_do_not_exist_in_the_2d_presentation() {
    let ctx = ActionContext {
        presentation: PresentationId::Technical2d,
        ..assisted()
    };
    for a in [
        Action::ViewTop,
        Action::ViewOblique,
        Action::ViewStandard,
        Action::ViewInverted,
        Action::ResetCamera,
        Action::FrameBoard,
    ] {
        assert_eq!(
            state(a, &ctx).disabled_reason,
            Some("reason-3d-only"),
            "{a:?}"
        );
        assert!(state(a, &assisted()).is_enabled(), "{a:?}");
    }
}

#[test]
fn collapsing_the_ribbon_needs_the_ribbon_on_screen() {
    assert_eq!(
        state(Action::ToggleRibbonCollapse, &assisted()).disabled_reason,
        Some("reason-coming-soon")
    );
    let ctx = ActionContext {
        ribbon_visible: true,
        ribbon_collapsed: true,
        ..assisted()
    };
    let s = state(Action::ToggleRibbonCollapse, &ctx);
    assert!(s.is_enabled());
    assert_eq!(s.checked, Some(true));
}
