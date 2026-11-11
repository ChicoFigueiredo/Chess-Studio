//! Faixa de Painéis (issue #185, estilo JetBrains): uma coluna de 32 px na borda direita, entre
//! o Ribbon e a barra de status, com um ícone por Painel. Clicar liga ou desliga o Painel
//! (escreve a mesma `ActionRequested` do atalho e da Paleta); Painel aberto fica realçado com
//! `accent-subtle`; Painel em janela do sistema ganha um contorno e o clique só traz a aba à
//! frente. A faixa segue visível com "Ocultar todos os Painéis" e serve de alvo de soltura de
//! um Painel flutuante arrastado (soltar nela o encaixa de volta). Some no modo Foco.
//!
//! Só Apresentação (ADR-0001): nada aqui toca a Partida. A geometria e o mapeamento clique ->
//! efeito são funções puras, testadas sem egui nem Bevy.

use crate::actions::ActionRequested;
use crate::controls::{Controls, trigger_label};
use crate::dock::Dock;
use crate::focus::ViewState;
use crate::i18n::I18n;
use crate::panel_chrome::{palette_of, panel_icon, radius};
use crate::ribbon::RibbonActionContext;
use crate::ui_theme::{
    ActionTip, ButtonVariant, VisualState, action_tooltip, button_look, icon_font, toggle_look,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_input::actions::{ActionContext, meta, state};
use chess_input::controls::Action;

/// Largura da faixa, em pontos.
pub const STRIPE_WIDTH: f32 = 32.0;
/// Lado de cada botão e espaço entre eles / margem do topo.
const BUTTON: f32 = 26.0;
const GAP: f32 = 4.0;

/// As Ações de Painel, na ordem da faixa (mesmas do grupo Exibir > Painéis e das abas).
pub const PANEL_ACTIONS: [Action; 16] = [
    Action::PanelNotation,
    Action::PanelClock,
    Action::PanelCaptured,
    Action::PanelBoard2d,
    Action::PanelAids,
    Action::PanelReview,
    Action::PanelArbitration,
    Action::PanelScoresheet,
    Action::PanelAnalysis,
    Action::PanelEngines,
    Action::PanelStudy,
    Action::PanelLibrary,
    Action::PanelOpenings,
    Action::PanelExplorer,
    Action::PanelTraining,
    Action::PanelOnline,
];

/// Situação de um Painel para a faixa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Closed,
    Open,
    /// Está numa janela do sistema.
    Detached,
}

/// O que o clique no botão faz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Escreve esta Ação (liga ou desliga o Painel).
    Toggle(Action),
    /// Só traz a aba do Painel à frente na janela dele; não alterna.
    FocusWindow,
}

pub fn mark_of(open: bool, detached: bool) -> Mark {
    if detached {
        Mark::Detached
    } else if open {
        Mark::Open
    } else {
        Mark::Closed
    }
}

pub fn click_effect(mark: Mark, action: Action) -> Click {
    match mark {
        Mark::Detached => Click::FocusWindow,
        _ => Click::Toggle(action),
    }
}

/// A faixa mostra-se fora do Foco e das capturas limpas.
pub fn stripe_shown(focus: bool, clean_capture: bool) -> bool {
    !focus && !clean_capture
}

/// Retângulo da faixa dentro de `area` (a área abaixo do Ribbon e acima da barra de status).
pub fn stripe_rect(area: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(area.max.x - STRIPE_WIDTH, area.min.y), area.max)
}

/// Retângulo do botão `index`, empilhados do topo.
pub fn button_rect(stripe: egui::Rect, index: usize) -> egui::Rect {
    let x = stripe.center().x - BUTTON * 0.5;
    let y = stripe.min.y + GAP + index as f32 * (BUTTON + GAP);
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(BUTTON, BUTTON))
}

/// Índice do botão sob `pos`, se houver (o egui faz o hit-test real; isto fixa a geometria).
#[cfg(test)]
pub fn hit_test(stripe: egui::Rect, count: usize, pos: egui::Pos2) -> Option<usize> {
    (0..count).find(|&i| button_rect(stripe, i).contains(pos))
}

/// O ponteiro está sobre a faixa durante o arrasto de um Painel flutuante.
pub fn drop_on_stripe(pointer: Option<egui::Pos2>, stripe: Option<egui::Rect>) -> bool {
    matches!((pointer, stripe), (Some(p), Some(r)) if r.contains(p))
}

const INSET_ID: &str = "chess-studio/panel-stripe/right-inset";

/// Largura que a faixa reserva na borda direita; 0 quando oculta.
pub fn right_inset(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new(INSET_ID)))
        .unwrap_or(0.0)
}

pub(crate) fn set_right_inset(ctx: &egui::Context, inset: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INSET_ID), inset));
}

/// Retângulo da faixa na janela (independe de a faixa estar reservada agora).
fn current_rect(ctx: &egui::Context) -> egui::Rect {
    let area = crate::ribbon::usable_rect(ctx);
    stripe_rect(egui::Rect::from_min_max(
        area.min,
        egui::pos2(ctx.viewport_rect().max.x, area.max.y),
    ))
}

/// A zona de soltura da faixa, se ela está visível agora.
pub fn drop_zone(ctx: &egui::Context) -> Option<egui::Rect> {
    (right_inset(ctx) > 0.0).then(|| current_rect(ctx))
}

pub struct PanelStripePlugin;

impl Plugin for PanelStripePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            panel_stripe.after(crate::ui_theme::apply_appearance),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn panel_stripe(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    controls: Res<Controls>,
    snapshot: Res<RibbonActionContext>,
    view: Res<ViewState>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut dock: ResMut<Dock>,
    mut requests: MessageWriter<ActionRequested>,
    mut demo_step: Local<usize>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let clean = capture.as_ref().is_some_and(|c| !c.show_panel);
    if !stripe_shown(view.focus, clean) {
        set_right_inset(&ctx, 0.0);
        return Ok(());
    }
    // Evidência (`--stripe-demo hidden|partial`): "Ocultar todos os Painéis" e, em `partial`,
    // reabre só Relógio e Capturadas, um pedido por quadro.
    if capture.is_some()
        && let Some(demo) = crate::capture::option_value("--stripe-demo")
    {
        let steps: &[Action] = match demo.as_str() {
            "hidden" => &[Action::HideAllPanels],
            "partial" => &[
                Action::HideAllPanels,
                Action::PanelClock,
                Action::PanelCaptured,
            ],
            _ => &[],
        };
        if let Some(action) = steps.get(*demo_step) {
            *demo_step += 1;
            requests.write(ActionRequested(*action));
        }
    }
    set_right_inset(&ctx, STRIPE_WIDTH);
    let rect = current_rect(&ctx);
    let p = palette_of(&ctx.global_style().visuals);
    let action_ctx: &ActionContext = &snapshot.0;
    egui::Area::new(egui::Id::new("chess-studio-panel-stripe"))
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .show(&ctx, |ui| {
            ui.set_clip_rect(rect);
            ui.allocate_exact_size(rect.size(), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, p.surface_1);
            ui.painter()
                .vline(rect.min.x, rect.y_range(), egui::Stroke::new(1.0, p.border));
            // Soltar um Painel flutuante aqui o encaixa: realça a faixa durante o arrasto.
            let dragging_over = ui.ctx().dragged_id().is_some()
                && drop_on_stripe(ui.ctx().input(|i| i.pointer.latest_pos()), Some(rect))
                && !dock.layout.floating.is_empty();
            if dragging_over {
                ui.painter().rect_stroke(
                    rect.shrink(2.0),
                    radius("radius-s"),
                    egui::Stroke::new(2.0, p.accent),
                    egui::StrokeKind::Inside,
                );
            }
            for (i, action) in PANEL_ACTIONS.into_iter().enumerate() {
                let Some((id, _)) = action.panel() else {
                    continue;
                };
                let st = state(action, action_ctx);
                let enabled = st.is_enabled();
                let detached = dock.layout.window_tile_of(id).is_some();
                let mark = mark_of(st.checked == Some(true), detached);
                let b = button_rect(rect, i);
                let sense = if enabled {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                };
                let response = ui.interact(b, egui::Id::new(("panel-stripe", id)), sense);
                let visual = if !enabled {
                    VisualState::Disabled
                } else if response.is_pointer_button_down_on() {
                    VisualState::Pressed
                } else if response.hovered() {
                    VisualState::Hover
                } else {
                    VisualState::Rest
                };
                let look = if mark == Mark::Open {
                    toggle_look(p, true, visual)
                } else {
                    button_look(p, ButtonVariant::Subtle, visual)
                };
                ui.painter().rect(
                    b,
                    radius("radius-s"),
                    look.fill,
                    look.stroke,
                    egui::StrokeKind::Inside,
                );
                if mark == Mark::Detached {
                    // Contorno de acento: o Painel vive numa janela do sistema.
                    ui.painter().rect_stroke(
                        b,
                        radius("radius-s"),
                        egui::Stroke::new(1.5, p.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                ui.painter().text(
                    b.center(),
                    egui::Align2::CENTER_CENTER,
                    panel_icon(id),
                    icon_font(ui.ctx(), 16.0),
                    look.text,
                );
                if enabled && response.clicked() {
                    match click_effect(mark, action) {
                        Click::Toggle(a) => {
                            requests.write(ActionRequested(a));
                        }
                        Click::FocusWindow => {
                            dock.layout.activate_in_window(id);
                        }
                    }
                }
                if response.hovered() {
                    let m = meta(action);
                    let label = i18n.t(m.label);
                    let description = if detached {
                        i18n.t("panelstripe-detached")
                    } else {
                        i18n.t(m.description)
                    };
                    let reason = st.disabled_reason.map(|r| i18n.t(r));
                    let shortcut = controls
                        .map()
                        .triggers(action)
                        .first()
                        .map(|t| trigger_label(&i18n, t));
                    let mut tip = ActionTip::new(&label).description(&description);
                    if let Some(s) = shortcut.as_deref() {
                        tip = tip.shortcut(s);
                    }
                    if let Some(r) = reason.as_deref() {
                        tip = tip.disabled(r);
                    }
                    action_tooltip(response, &tip);
                }
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(1280.0, 696.0))
    }

    #[test]
    fn the_stripe_is_32px_on_the_right_edge_of_the_area() {
        let s = stripe_rect(area());
        assert_eq!(s.width(), 32.0);
        assert_eq!(s.max, area().max);
        assert_eq!(s.min.y, 100.0);
    }

    #[test]
    fn buttons_stack_from_the_top_inside_the_stripe_without_overlap() {
        let s = stripe_rect(area());
        for i in 0..PANEL_ACTIONS.len() {
            let b = button_rect(s, i);
            assert!(s.contains_rect(b), "botão {i} fora da faixa");
            if i > 0 {
                assert!(button_rect(s, i - 1).max.y < b.min.y);
            }
        }
    }

    #[test]
    fn hit_test_finds_the_button_under_the_pointer() {
        let s = stripe_rect(area());
        for i in 0..PANEL_ACTIONS.len() {
            let c = button_rect(s, i).center();
            assert_eq!(hit_test(s, PANEL_ACTIONS.len(), c), Some(i));
        }
        assert_eq!(hit_test(s, 14, egui::pos2(100.0, 150.0)), None);
        // O vão entre dois botões não acerta nenhum.
        let gap = egui::pos2(s.center().x, button_rect(s, 0).max.y + 1.0);
        assert_eq!(hit_test(s, 14, gap), None);
    }

    #[test]
    fn every_panel_action_has_a_button_exactly_once() {
        for a in Action::ALL.into_iter().filter(|a| a.panel().is_some()) {
            assert_eq!(
                PANEL_ACTIONS.iter().filter(|x| **x == a).count(),
                1,
                "{a:?}"
            );
        }
        assert!(PANEL_ACTIONS.iter().all(|a| a.panel().is_some()));
    }

    #[test]
    fn clicking_toggles_the_panel_action_except_when_detached() {
        assert_eq!(
            click_effect(mark_of(false, false), Action::PanelClock),
            Click::Toggle(Action::PanelClock)
        );
        assert_eq!(
            click_effect(mark_of(true, false), Action::PanelClock),
            Click::Toggle(Action::PanelClock)
        );
        // Em janela do sistema, mesmo "aberto": só traz à frente.
        assert_eq!(
            click_effect(mark_of(true, true), Action::PanelClock),
            Click::FocusWindow
        );
    }

    #[test]
    fn the_mark_reflects_open_closed_and_detached() {
        assert_eq!(mark_of(false, false), Mark::Closed);
        assert_eq!(mark_of(true, false), Mark::Open);
        assert_eq!(mark_of(false, true), Mark::Detached);
    }

    #[test]
    fn focus_and_clean_captures_hide_the_stripe() {
        assert!(stripe_shown(false, false));
        assert!(!stripe_shown(true, false));
        assert!(!stripe_shown(false, true));
    }

    #[test]
    fn the_usable_rect_ends_before_the_stripe() {
        let ctx = egui::Context::default();
        let full = ctx.viewport_rect();
        assert_eq!(crate::ribbon::usable_rect(&ctx), full);
        set_right_inset(&ctx, STRIPE_WIDTH);
        assert!((crate::ribbon::usable_rect(&ctx).max.x - (full.max.x - 32.0)).abs() < 1e-3);
        assert_eq!(drop_zone(&ctx).map(|r| r.max.x), Some(full.max.x));
        set_right_inset(&ctx, 0.0);
        assert_eq!(crate::ribbon::usable_rect(&ctx), full);
        assert_eq!(drop_zone(&ctx), None);
    }

    #[test]
    fn dropping_is_only_on_the_stripe() {
        let s = Some(stripe_rect(area()));
        assert!(drop_on_stripe(Some(egui::pos2(1270.0, 300.0)), s));
        assert!(!drop_on_stripe(Some(egui::pos2(600.0, 300.0)), s));
        assert!(!drop_on_stripe(None, s));
        assert!(!drop_on_stripe(Some(egui::pos2(1270.0, 300.0)), None));
    }
}
