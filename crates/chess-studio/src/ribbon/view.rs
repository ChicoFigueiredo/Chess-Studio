//! Desenho do Ribbon em egui: faixa da Barra de Acesso Rápido, abas, corpo (clássico, simples
//! e recolhido) e selos dos KeyTips. Nada aqui decide o que uma Ação faz: os botões só
//! escrevem `ActionRequested`.
//!
//! O `bevy_egui` entrega só o `Context`, sem a raiz `Ui` que o `egui::Panel` exige; por isso o
//! Ribbon é uma `Area` em largura total no topo, e o dock e as câmeras usam
//! [`super::usable_rect`] para ficar abaixo dele.

use super::icons::glyph;
use super::layout_dialog;
use super::model::{
    Group, KeyTipLevel, LAYOUTS_GROUP, RibbonView, board_tab_visible, groups, split_overflow,
    visible_tabs_for,
};
use super::{KeyTipSpot, RibbonActionContext, RibbonMode, RibbonState, set_top_inset};
use crate::actions::ActionRequested;
use crate::controls::{Controls, trigger_label};
use crate::focus::ViewState;
use crate::i18n::I18n;
use crate::screen_layouts::{LayoutRef, ScreenLayoutRequested, ScreenLayouts, choose};
use crate::ui_theme::{
    ActionTip, ButtonVariant, Look, Palette, VisualState, action_tooltip, button_look, cs_checkbox,
    cs_dropdown_placeholder, icon_font, icon_job, palette, strong_font, toggle_look,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use chess_input::actions::{ActionContext, ActionKind, meta, state};
use chess_input::controls::Action;
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};

/// Alturas da faixa da Barra de Acesso Rápido e das abas (`ribbon-tabs-h`).
const STRIP_H: f32 = 28.0;
const TABS_H: f32 = 32.0;
/// Corpo do Ribbon clássico (`ribbon-classic-h`) e simples (`ribbon-simple-h`).
const CLASSIC_H: f32 = 96.0;
const SIMPLE_H: f32 = 40.0;
/// Alvo mínimo de clique dos botões pequenos: pilhas de até três de 24px (72px) mais o rótulo
/// do grupo de 16px dão os 88px de conteúdo do corpo clássico.
const SMALL_H: f32 = crate::ui_theme::MIN_TARGET;
const STACK_H: f32 = 3.0 * SMALL_H;
const BIG_H: f32 = STACK_H;
const GROUP_LABEL_H: f32 = 16.0;
/// Largura do botão grande: de 56 a 80px, o rótulo em até duas linhas.
const BIG_MIN_W: f32 = 56.0;
const BIG_MAX_W: f32 = 80.0;

/// Formas do botão de Ação.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Ícone `icon-l` com o rótulo embaixo (Ribbon clássico).
    Big,
    /// Ícone `icon-s` e rótulo.
    Small,
    /// Só o ícone (Barra de Acesso Rápido).
    IconOnly,
}

/// O que um quadro do Ribbon precisa e produz.
struct Env<'a> {
    i18n: &'a I18n,
    ctx: &'a ActionContext,
    controls: &'a Controls,
    /// Ações clicadas neste quadro.
    out: Vec<Action>,
    /// Retângulos dos itens do corpo, para os KeyTips.
    rects: Vec<(Action, Rect, KeyTipSpot)>,
    register: bool,
    /// Layouts de Tela do Perfil local (#186) e os pedidos feitos neste quadro.
    sets: &'a ScreenLayouts,
    set_out: Vec<ScreenLayoutRequested>,
}

/// Cor, tamanho e retângulo comuns a todo botão do Ribbon.
fn button_core(ui: &mut Ui, size: Vec2, enabled: bool, checked: bool) -> (Response, Rect, Look) {
    let p = palette(ui);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let visual = if !enabled {
        VisualState::Disabled
    } else if response.is_pointer_button_down_on() {
        VisualState::Pressed
    } else if response.hovered() {
        VisualState::Hover
    } else {
        VisualState::Rest
    };
    let look = if checked {
        toggle_look(p, true, visual)
    } else {
        button_look(p, ButtonVariant::Subtle, visual)
    };
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(4);
        ui.painter()
            .rect(rect, corner, look.fill, look.stroke, StrokeKind::Inside);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(1.0),
                corner,
                Stroke::new(2.0, p.focus_ring),
                StrokeKind::Outside,
            );
        }
    }
    (response, rect, look)
}

fn body_font() -> FontId {
    FontId::proportional(13.0)
}

/// Rótulo do botão grande: `ribbon-label` (12/14), centralizado, em até duas linhas; o que
/// passar disso termina em reticências.
fn big_label(ui: &Ui, text: &str) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(12.0),
        Color32::PLACEHOLDER,
        BIG_MAX_W - 12.0,
    );
    job.halign = Align::Center;
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// Largura do botão grande para o rótulo medido.
fn big_width(label: &egui::Galley) -> f32 {
    (label.size().x + 12.0).clamp(BIG_MIN_W, BIG_MAX_W)
}

/// Largura de um botão pequeno com ícone e rótulo.
fn small_width(ui: &Ui, icon: Option<&str>, text: &str) -> f32 {
    let job = icon_job(ui.ctx(), icon, text, body_font(), Color32::PLACEHOLDER);
    ui.painter().layout_job(job).size().x + 16.0
}

/// Ajustes de um botão de Ação.
#[derive(Default, Clone, Copy)]
struct Extra<'a> {
    /// Troca o ícone da Ação (a Paleta usa a lupa).
    icon: Option<&'static str>,
    /// Texto no lugar do rótulo (ou ao lado do ícone, no `IconOnly`).
    text: Option<&'a str>,
    min_width: f32,
}

/// O botão de uma Ação: rótulo, ícone, estado, tooltip e clique vêm todos da lista única.
fn action_button(
    ui: &mut Ui,
    env: &mut Env<'_>,
    action: Action,
    form: Form,
    extra: Extra<'_>,
) -> Response {
    let m = meta(action);
    let st = state(action, env.ctx);
    let enabled = st.is_enabled();
    let checked = st.checked == Some(true) && m.kind != ActionKind::Command;
    let label = env.i18n.t(m.label);
    let icon = glyph(extra.icon.unwrap_or(m.icon));
    let ctx = ui.ctx().clone();

    let response = match form {
        Form::Big => {
            let icon_galley = ui.painter().layout_no_wrap(
                icon.unwrap_or("").to_owned(),
                icon_font(&ctx, 32.0),
                Color32::PLACEHOLDER,
            );
            let text_galley = big_label(ui, &label);
            let width = big_width(&text_galley);
            let (response, rect, look) = button_core(
                ui,
                vec2(width.max(extra.min_width), BIG_H),
                enabled,
                checked,
            );
            let icon_pos = Align2::CENTER_TOP
                .align_size_within_rect(icon_galley.size(), rect.shrink2(vec2(0.0, 5.0)))
                .min;
            ui.painter().galley(icon_pos, icon_galley, look.text);
            // `rect.min` do galley não é a origem quando o texto é centralizado.
            let text_pos = Align2::CENTER_BOTTOM
                .align_size_within_rect(text_galley.rect.size(), rect.shrink2(vec2(0.0, 4.0)))
                .min
                - text_galley.rect.min.to_vec2();
            ui.painter().galley(text_pos, text_galley, look.text);
            (response, rect, KeyTipSpot::BigIcon)
        }
        Form::Small | Form::IconOnly => {
            let text = match (form, extra.text) {
                (_, Some(t)) => t.to_owned(),
                (Form::IconOnly, None) => String::new(),
                _ => label.clone(),
            };
            let job = icon_job(&ctx, icon, &text, body_font(), Color32::PLACEHOLDER);
            let galley = ui.painter().layout_job(job);
            let (height, pad) = if form == Form::Small {
                (SMALL_H, 16.0)
            } else {
                (24.0, 12.0)
            };
            let width = (galley.size().x + pad).max(extra.min_width);
            let (response, rect, look) = button_core(ui, vec2(width, height), enabled, checked);
            let pos = Align2::CENTER_CENTER
                .align_size_within_rect(galley.size(), rect)
                .min;
            ui.painter().galley(pos, galley, look.text);
            let spot = if form == Form::Small {
                KeyTipSpot::Leading
            } else {
                KeyTipSpot::Below
            };
            (response, rect, spot)
        }
    };
    let (response, rect, spot) = response;

    if env.register {
        env.rects.push((action, rect, spot));
    }
    if enabled && response.clicked() {
        env.out.push(action);
    }
    let response = if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    if response.contains_pointer() || response.has_focus() {
        let description = env.i18n.t(m.description);
        let reason = st.disabled_reason.map(|r| env.i18n.t(r));
        let shortcut = env
            .controls
            .map()
            .triggers(action)
            .first()
            .map(|t| trigger_label(env.i18n, t));
        let mut tip = ActionTip::new(&label).description(&description);
        if let Some(s) = shortcut.as_deref() {
            tip = tip.shortcut(s);
        }
        if let Some(r) = reason.as_deref() {
            tip = tip.disabled(r);
        }
        return action_tooltip(response, &tip);
    }
    response
}

/// Botão de ícone que não é Ação (modo do Ribbon, "⋯", personalizar).
fn icon_button(
    ui: &mut Ui,
    i18n: &I18n,
    icon: &str,
    title_id: &str,
    desc_id: &str,
    checked: bool,
) -> Response {
    let ctx = ui.ctx().clone();
    let job = icon_job(&ctx, glyph(icon), "", body_font(), Color32::PLACEHOLDER);
    let galley = ui.painter().layout_job(job);
    let (response, rect, look) = button_core(ui, vec2(28.0, 24.0), true, checked);
    let pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(pos, galley, look.text);
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.contains_pointer() {
        let title = i18n.t(title_id);
        let description = i18n.t(desc_id);
        return action_tooltip(response, &ActionTip::new(&title).description(&description));
    }
    response
}

fn separator(ui: &mut Ui, p: &Palette, height: f32) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, p.border));
    ui.add_space(4.0);
}

// --- Grupo Painéis: caixas de seleção (#183) ---

/// O grupo "Painéis" da aba Exibir: uma caixa de seleção por Painel, no estilo da aba Exibir
/// do Word. Nos modos simples e recolhido vira o menu "Painéis ▾".
fn is_panels_group(group: &Group) -> bool {
    group.label == "ribbon-group-panels"
}

/// Quantas caixas por coluna no corpo clássico.
const PANEL_ROWS: usize = 3;
/// Espaço entre colunas de caixas.
const PANEL_COLUMN_GAP: f32 = 14.0;

/// A caixa de seleção de um Painel: marcada se ele está visível; clicar escreve a mesma
/// `ActionRequested` que o atalho e a Paleta.
fn panel_checkbox(ui: &mut Ui, env: &mut Env<'_>, action: Action) -> Response {
    let m = meta(action);
    let st = state(action, env.ctx);
    let enabled = st.is_enabled();
    let label = env.i18n.t(m.label);
    let mut checked = st.checked == Some(true);
    let before = checked;
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            crate::ui_theme::cs_checkbox(ui, &mut checked, &label)
        })
        .inner;
    if env.register {
        env.rects.push((action, response.rect, KeyTipSpot::Leading));
    }
    if enabled && checked != before {
        env.out.push(action);
    }
    if response.contains_pointer() || response.has_focus() {
        let description = env.i18n.t(m.description);
        let reason = st.disabled_reason.map(|r| env.i18n.t(r));
        let shortcut = env
            .controls
            .map()
            .triggers(action)
            .first()
            .map(|t| trigger_label(env.i18n, t));
        let mut tip = ActionTip::new(&label).description(&description);
        if let Some(s) = shortcut.as_deref() {
            tip = tip.shortcut(s);
        }
        if let Some(r) = reason.as_deref() {
            tip = tip.disabled(r);
        }
        return action_tooltip(response, &tip);
    }
    response
}

fn panel_label_width(ui: &Ui, env: &Env<'_>, action: Action) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        env.i18n.t(meta(action).label),
        body_font(),
        Color32::PLACEHOLDER,
    );
    galley.size().x + ui.spacing().icon_width + 8.0 + 6.0
}

/// Largura do grupo Painéis no corpo clássico: colunas de `PANEL_ROWS` caixas.
fn panels_classic_width(ui: &Ui, env: &Env<'_>, group: &Group) -> f32 {
    group
        .actions
        .chunks(PANEL_ROWS)
        .map(|chunk| {
            chunk
                .iter()
                .map(|a| panel_label_width(ui, env, *a))
                .fold(0.0, f32::max)
                + PANEL_COLUMN_GAP
        })
        .sum()
}

/// As caixas, uma por linha (menu "Painéis ▾" e menu "⋯").
fn panel_checkbox_list(ui: &mut Ui, env: &mut Env<'_>, group: &Group) {
    for action in &group.actions {
        panel_checkbox(ui, env, *action);
    }
}

/// Botão "Painéis ▾": abre o menu com as mesmas caixas e fica aberto enquanto se marca.
fn panels_menu_button(ui: &mut Ui, env: &mut Env<'_>, group: &Group) {
    let label = env.i18n.t(group.label);
    let ctx = ui.ctx().clone();
    let caret = glyph("caret-down");
    let job = icon_job(&ctx, caret, &label, body_font(), Color32::PLACEHOLDER);
    let galley = ui.painter().layout_job(job);
    let (response, rect, look) =
        button_core(ui, vec2(galley.size().x + 20.0, SMALL_H), true, false);
    let pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(pos, galley, look.text);
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(180.0);
            panel_checkbox_list(ui, env, group);
        });
}

// --- Grupo Layout de Tela (#186, #196) ---

fn is_layouts_group(group: &Group) -> bool {
    group.label == LAYOUTS_GROUP
}

/// Largura mínima da lista suspensa de Layouts.
const LAYOUT_COMBO_W: f32 = 190.0;

fn only_id() -> Id {
    Id::new("chess-studio/screen-layouts/only")
}

/// Largura de "Só este layout" mais "Salvar layout…".
fn layouts_row_width(ui: &Ui, env: &Env<'_>) -> f32 {
    let toggle = small_width(ui, None, &env.i18n.t("screen-layout-only-toggle")) + 20.0;
    let save = small_width(ui, None, &env.i18n.t("screen-layout-save-button"));
    toggle + save + 6.0
}

fn layouts_classic_width(ui: &Ui, env: &Env<'_>) -> f32 {
    LAYOUT_COMBO_W.max(layouts_row_width(ui, env))
}

fn layouts_simple_width(ui: &Ui, env: &Env<'_>) -> f32 {
    LAYOUT_COMBO_W + layouts_row_width(ui, env) + 8.0
}

/// A lista suspensa: escolher um Layout o aplica. Mostra só o texto de espaço reservado, porque
/// o Layout aplicado não é um estado (os Painéis mudam depois); assim escolher o mesmo de novo
/// reaplica. "Só este layout" ligado, ou Shift na escolha, mostra só os Painéis do Layout.
fn layouts_combo(ui: &mut Ui, env: &mut Env<'_>) {
    let options: Vec<(Option<LayoutRef>, String)> = env
        .sets
        .combo_options(|key| env.i18n.t(key))
        .into_iter()
        .map(|(layout, label)| (Some(layout), label))
        .collect();
    let mut selected: Option<LayoutRef> = None;
    let response = cs_dropdown_placeholder(
        ui,
        "chess-studio/screen-layouts/combo",
        &mut selected,
        &options,
        &env.i18n.t("screen-layout-combo-placeholder"),
    );
    let changed = response.changed();
    let _ = response.on_hover_text(env.i18n.t("screen-layout-combo-tip"));
    if changed && let Some(layout) = selected {
        let toggle: bool = ui.data(|d| d.get_temp(only_id())).unwrap_or(false);
        let only = toggle || ui.input(|i| i.modifiers.shift);
        env.set_out.push(choose(layout, only));
    }
}

/// "Só este layout" (alternância guardada na sessão) e o botão "Salvar layout…".
fn layouts_toggle_and_save(ui: &mut Ui, env: &mut Env<'_>) {
    let mut only: bool = ui.data(|d| d.get_temp(only_id())).unwrap_or(false);
    ui.spacing_mut().interact_size.y = SMALL_H;
    let toggle = cs_checkbox(ui, &mut only, &env.i18n.t("screen-layout-only-toggle"))
        .on_hover_text(env.i18n.t("screen-layout-only-tip"));
    if toggle.changed() {
        ui.data_mut(|d| d.insert_temp(only_id(), only));
    }
    let label = env.i18n.t("screen-layout-save-button");
    let job = icon_job(ui.ctx(), None, &label, body_font(), Color32::PLACEHOLDER);
    let galley = ui.painter().layout_job(job);
    let size = vec2(galley.size().x + 16.0, SMALL_H);
    let (response, rect, look) = button_core(ui, size, true, false);
    let pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(pos, galley, look.text);
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(env.i18n.t("screen-layout-save-button-tip"));
    if response.clicked() {
        layout_dialog::open(ui.ctx());
    }
}

/// Corpo clássico: a lista suspensa e, embaixo, "Só este layout" e "Salvar layout…".
fn layouts_classic(ui: &mut Ui, env: &mut Env<'_>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        layouts_combo(ui, env);
        ui.horizontal(|ui| layouts_toggle_and_save(ui, env));
    });
}

/// Modo simples: tudo em uma linha.
fn layouts_simple(ui: &mut Ui, env: &mut Env<'_>) {
    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
    layouts_combo(ui, env);
    layouts_toggle_and_save(ui, env);
}

/// Menu "⋯": a mesma lista e botões, empilhados.
fn layouts_menu_contents(ui: &mut Ui, env: &mut Env<'_>) {
    layouts_combo(ui, env);
    layouts_toggle_and_save(ui, env);
}

// --- Corpo clássico ---

fn classic_group(ui: &mut Ui, env: &mut Env<'_>, group: &Group) {
    let p = palette(ui);
    let label = env.i18n.t(group.label);
    let panels = is_panels_group(group);
    let sets = is_layouts_group(group);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
        let row = ui.horizontal_top(|ui| {
            // Toda linha de grupo tem a mesma altura: o rótulo fica na mesma linha de base.
            ui.set_min_height(STACK_H);
            if sets {
                layouts_classic(ui, env);
                return;
            }
            if panels {
                ui.spacing_mut().item_spacing.x = PANEL_COLUMN_GAP;
                for chunk in group.actions.chunks(PANEL_ROWS) {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                        // Três caixas cabem na altura do corpo, como três botões pequenos.
                        ui.spacing_mut().interact_size.y = SMALL_H;
                        for action in chunk {
                            panel_checkbox(ui, env, *action);
                        }
                    });
                }
                return;
            }
            action_button(ui, env, group.actions[0], Form::Big, Extra::default());
            for chunk in group.actions[1..].chunks(3) {
                let width = chunk
                    .iter()
                    .map(|a| {
                        let m = meta(*a);
                        small_width(ui, glyph(m.icon), &env.i18n.t(m.label))
                    })
                    .fold(0.0, f32::max);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                    for action in chunk {
                        let extra = Extra {
                            min_width: width,
                            ..Extra::default()
                        };
                        action_button(ui, env, *action, Form::Small, extra);
                    }
                });
            }
        });
        let caption = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(12.0), p.ink_muted);
        let width = row.response.rect.width().max(caption.size().x);
        let (rect, _) = ui.allocate_exact_size(vec2(width, GROUP_LABEL_H), Sense::hover());
        let pos = Align2::CENTER_CENTER
            .align_size_within_rect(caption.size(), rect)
            .min;
        ui.painter().galley(pos, caption, p.ink_muted);
    });
}

/// Largura estimada de um grupo clássico: botão grande mais as colunas de pequenos.
fn classic_group_width(ui: &Ui, env: &Env<'_>, group: &Group) -> f32 {
    if is_layouts_group(group) {
        return layouts_classic_width(ui, env);
    }
    if is_panels_group(group) {
        return panels_classic_width(ui, env, group);
    }
    let big = meta(group.actions[0]);
    let mut width = big_width(&big_label(ui, &env.i18n.t(big.label)));
    for chunk in group.actions[1..].chunks(3) {
        let column = chunk
            .iter()
            .map(|a| {
                let m = meta(*a);
                small_width(ui, glyph(m.icon), &env.i18n.t(m.label))
            })
            .fold(0.0, f32::max);
        width += 2.0 + column;
    }
    width
}

fn classic_body(ui: &mut Ui, env: &mut Env<'_>, view: RibbonView, available: f32) {
    let p = palette(ui);
    let groups = groups(view);
    let widths: Vec<f32> = groups
        .iter()
        .map(|g| classic_group_width(ui, env, g))
        .collect();
    // Entre dois grupos: 4 + 1 + 4 do separador mais o espaço entre itens.
    let split = split_overflow(&widths, 11.0, available, 32.0);
    ui.horizontal_top(|ui| {
        for (i, group) in groups.iter().take(split.visible).enumerate() {
            if i > 0 {
                separator(ui, p, CLASSIC_H - 20.0);
            }
            classic_group(ui, env, group);
        }
        if split.has_overflow() {
            separator(ui, p, CLASSIC_H - 20.0);
            overflow_menu(ui, env, p, &groups, &split.overflow);
        }
    });
}

// --- Corpo simples ---

fn group_width(ui: &Ui, env: &Env<'_>, group: &Group) -> f32 {
    if is_layouts_group(group) {
        return layouts_simple_width(ui, env);
    }
    if is_panels_group(group) {
        let label = env.i18n.t(group.label);
        return small_width(ui, glyph("caret-down"), &label) + 4.0;
    }
    let items: f32 = group
        .actions
        .iter()
        .map(|a| {
            let m = meta(*a);
            small_width(ui, glyph(m.icon), &env.i18n.t(m.label))
        })
        .sum();
    items + 2.0 * group.actions.len().saturating_sub(1) as f32
}

/// Botão "⋯" com os grupos que não cabem na linha.
fn overflow_menu(
    ui: &mut Ui,
    env: &mut Env<'_>,
    p: &Palette,
    groups: &[Group],
    overflow: &[usize],
) {
    let more = icon_button(
        ui,
        env.i18n,
        "dots-three",
        "ribbon-more",
        "ribbon-more-desc",
        false,
    );
    egui::Popup::menu(&more).show(|ui| {
        ui.set_min_width(200.0);
        for &index in overflow {
            let group = &groups[index];
            ui.label(
                egui::RichText::new(env.i18n.t(group.label))
                    .small()
                    .color(p.ink_muted),
            );
            if is_layouts_group(group) {
                layouts_menu_contents(ui, env);
                continue;
            }
            if is_panels_group(group) {
                panel_checkbox_list(ui, env, group);
                continue;
            }
            let width = group
                .actions
                .iter()
                .map(|a| {
                    let m = meta(*a);
                    small_width(ui, glyph(m.icon), &env.i18n.t(m.label))
                })
                .fold(200.0, f32::max);
            for action in &group.actions {
                let extra = Extra {
                    min_width: width,
                    ..Extra::default()
                };
                action_button(ui, env, *action, Form::Small, extra);
            }
        }
    });
}

fn simple_body(ui: &mut Ui, env: &mut Env<'_>, view: RibbonView, available: f32) {
    let p = palette(ui);
    let groups = groups(view);
    let widths: Vec<f32> = groups.iter().map(|g| group_width(ui, env, g)).collect();
    // Entre dois grupos: 4 + 1 + 4 do separador mais o espaço entre itens.
    let split = split_overflow(&widths, 11.0, available, 32.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
        for (i, group) in groups.iter().take(split.visible).enumerate() {
            if i > 0 {
                separator(ui, p, 24.0);
            }
            if is_layouts_group(group) {
                layouts_simple(ui, env);
                continue;
            }
            if is_panels_group(group) {
                panels_menu_button(ui, env, group);
                continue;
            }
            for action in &group.actions {
                action_button(ui, env, *action, Form::Small, Extra::default());
            }
        }
        if split.has_overflow() {
            separator(ui, p, 24.0);
            overflow_menu(ui, env, p, &groups, &split.overflow);
        }
    });
}

fn body(ui: &mut Ui, env: &mut Env<'_>, view: RibbonView, mode: RibbonMode, width: f32) {
    ui.set_min_width(width);
    match mode {
        RibbonMode::Classic => {
            ui.set_min_height(CLASSIC_H - 8.0);
            classic_body(ui, env, view, width - 24.0);
        }
        RibbonMode::Simple => {
            ui.set_min_height(SIMPLE_H - 8.0);
            simple_body(ui, env, view, width - 24.0);
        }
    }
}

// --- Abas ---

fn tab_button(ui: &mut Ui, env: &Env<'_>, view: RibbonView, active: bool) -> (Response, Rect) {
    let p = palette(ui);
    let label = env.i18n.t(view.label());
    let font = strong_font(ui.ctx(), 13.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label, font, Color32::PLACEHOLDER);
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 28.0, TABS_H), Sense::click());
    let contextual = matches!(view, RibbonView::Scoresheet | RibbonView::Board);
    let (fill, text) = if view.is_file() {
        (
            if response.hovered() {
                p.accent_hover
            } else {
                p.accent
            },
            p.on_accent,
        )
    } else {
        (
            if response.hovered() {
                p.surface_3
            } else {
                Color32::TRANSPARENT
            },
            if contextual { p.accent_ink } else { p.ink },
        )
    };
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
        let pos = Align2::CENTER_CENTER
            .align_size_within_rect(galley.size(), rect)
            .min;
        ui.painter().galley(pos, galley, text);
        if active && !view.is_file() {
            let line = Rect::from_min_max(
                pos2(rect.min.x + 6.0, rect.max.y - 3.0),
                rect.max - vec2(6.0, 0.0),
            );
            ui.painter().rect_filled(line, CornerRadius::ZERO, p.accent);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                CornerRadius::ZERO,
                Stroke::new(2.0, p.focus_ring),
                StrokeKind::Inside,
            );
        }
    }
    (
        response.on_hover_cursor(egui::CursorIcon::PointingHand),
        rect,
    )
}

// --- Sistema ---

/// O que o desenho decidiu: aplicado ao estado depois, fora das closures.
#[derive(Default)]
struct Decisions {
    clicked_tab: Option<RibbonView>,
    toggle_mode: bool,
    pin: Vec<(Action, bool)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn ribbon_ui(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    controls: Res<Controls>,
    context: Res<RibbonActionContext>,
    mut ribbon: ResMut<RibbonState>,
    view_state: Option<Res<ViewState>>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    sets: Option<Res<ScreenLayouts>>,
    mut requests: MessageWriter<ActionRequested>,
    mut set_requests: MessageWriter<ScreenLayoutRequested>,
) -> Result {
    let sets = sets.as_deref().cloned().unwrap_or_default();
    let ctx = contexts.ctx_mut()?.clone();
    // Modo Foco: sem Ribbon. Capturas limpas (sem `--panel`) também não o mostram.
    if view_state.is_some_and(|v| v.focus) || capture.is_some_and(|c| !c.show_panel) {
        set_top_inset(&ctx, 0.0);
        return Ok(());
    }
    let screen = ctx.viewport_rect();
    let width = screen.width();
    let p = palette_of(&ctx);
    let scoresheet_open = context.0.visible_panels.contains("Scoresheet");
    let tabs = visible_tabs_for(scoresheet_open, board_tab_visible(context.0.presentation));
    // A aba contextual some quando o Painel fecha.
    if !tabs.contains(&ribbon.active) {
        ribbon.active = tabs[1];
    }

    let mode = ribbon.mode;
    let collapsed = ribbon.collapsed;
    let active = ribbon.active;
    let body_open = ribbon.body_open;
    let keytip_level = ribbon.keytips.level();
    let quick_access = ribbon.quick_access.clone();
    let mut env = Env {
        i18n: &i18n,
        ctx: &context.0,
        controls: &controls,
        out: Vec::new(),
        rects: Vec::new(),
        register: false,
        sets: &sets,
        set_out: Vec::new(),
    };
    let mut decisions = Decisions::default();
    let mut tab_rects: Vec<(RibbonView, Rect)> = Vec::new();

    let area = egui::Area::new(Id::new("chess-studio/ribbon"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            // Faixa da Barra de Acesso Rápido.
            egui::Frame::NONE.fill(p.surface_0).show(ui, |ui| {
                ui.set_min_width(width);
                let strip = Rect::from_min_size(ui.cursor().min, vec2(width, STRIP_H));
                ui.painter().text(
                    strip.center(),
                    Align2::CENTER_CENTER,
                    i18n.t("app-title"),
                    FontId::proportional(12.5),
                    p.ink_muted,
                );
                ui.allocate_ui_with_layout(
                    vec2(width, STRIP_H),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(8.0);
                        for action in &quick_access {
                            action_button(ui, &mut env, *action, Form::IconOnly, Extra::default());
                        }
                        let caret = icon_button(
                            ui,
                            &i18n,
                            "caret-down",
                            "qab-customize",
                            "qab-customize-desc",
                            false,
                        );
                        egui::Popup::menu(&caret)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.label(
                                    egui::RichText::new(i18n.t("qab-customize-heading")).strong(),
                                );
                                egui::ScrollArea::vertical()
                                    .max_height(360.0)
                                    .show(ui, |ui| {
                                        for action in Action::ALL {
                                            let m = meta(action);
                                            if m.kind == ActionKind::Gesture {
                                                continue;
                                            }
                                            let mut pinned = quick_access.contains(&action);
                                            let before = pinned;
                                            crate::ui_theme::cs_checkbox(
                                                ui,
                                                &mut pinned,
                                                &i18n.t(m.label),
                                            );
                                            if pinned != before {
                                                decisions.pin.push((action, pinned));
                                            }
                                        }
                                    });
                            });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(8.0);
                            let shortcut = controls
                                .map()
                                .triggers(Action::OpenPalette)
                                .first()
                                .map_or_else(|| "Ctrl+K".to_owned(), |t| trigger_label(&i18n, t));
                            let extra = Extra {
                                icon: Some("magnifying-glass"),
                                text: Some(&shortcut),
                                ..Extra::default()
                            };
                            action_button(ui, &mut env, Action::OpenPalette, Form::IconOnly, extra);
                        });
                    },
                );
            });
            // Abas.
            egui::Frame::NONE.fill(p.surface_1).show(ui, |ui| {
                ui.set_min_width(width);
                ui.horizontal(|ui| {
                    ui.set_min_height(TABS_H);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for view in &tabs {
                        let is_active = *view == active && (!collapsed || body_open);
                        let (response, rect) = tab_button(ui, &env, *view, is_active);
                        tab_rects.push((*view, rect));
                        if response.clicked() {
                            decisions.clicked_tab = Some(*view);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                        ui.add_space(8.0);
                        action_button(
                            ui,
                            &mut env,
                            Action::ToggleRibbonCollapse,
                            Form::IconOnly,
                            Extra::default(),
                        );
                        let (icon, title, desc) = match mode {
                            RibbonMode::Classic => (
                                "list",
                                "ribbon-mode-to-simple",
                                "ribbon-mode-to-simple-desc",
                            ),
                            RibbonMode::Simple => (
                                "rows",
                                "ribbon-mode-to-classic",
                                "ribbon-mode-to-classic-desc",
                            ),
                        };
                        if icon_button(ui, &i18n, icon, title, desc, false).clicked() {
                            decisions.toggle_mode = true;
                        }
                    });
                });
            });
            // Corpo (só com o Ribbon expandido).
            if !collapsed {
                env.register = true;
                egui::Frame::NONE
                    .fill(p.surface_1)
                    .stroke(Stroke::new(1.0, p.border))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        body(ui, &mut env, active, mode, width - 16.0);
                    });
                env.register = false;
            }
        });
    let area_rect = area.response.rect;
    let mut popup_rect = None;

    // Recolhido: o corpo da aba abre por cima do tabuleiro.
    if collapsed && body_open {
        env.register = true;
        let top = pos2(screen.min.x, area_rect.max.y);
        let popup = egui::Area::new(Id::new("chess-studio/ribbon-body"))
            .order(egui::Order::Foreground)
            .fixed_pos(top)
            .constrain(false)
            .show(&ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                egui::Frame::NONE
                    .fill(p.surface_1)
                    .stroke(Stroke::new(1.0, p.border_strong))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        body(ui, &mut env, active, mode, width - 16.0);
                    });
            });
        popup_rect = Some(popup.response.rect);
        env.register = false;
    }
    set_top_inset(&ctx, area_rect.max.y - screen.min.y);

    // --- Aplica o que o desenho decidiu ---
    if let Some(view) = decisions.clicked_tab {
        if view.is_file() {
            ribbon.backstage = Some(super::BackstagePage::Profiles);
            ribbon.body_open = false;
        } else if collapsed {
            ribbon.body_open = !(body_open && active == view);
            ribbon.active = view;
        } else {
            ribbon.active = view;
        }
    }
    if decisions.toggle_mode {
        ribbon.mode = match mode {
            RibbonMode::Classic => RibbonMode::Simple,
            RibbonMode::Simple => RibbonMode::Classic,
        };
    }
    for (action, pinned) in std::mem::take(&mut decisions.pin) {
        ribbon.set_pinned(action, pinned);
    }
    if collapsed && body_open {
        let (pressed, pos, escape) = ctx.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.latest_pos(),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let outside = pressed
            && !egui::Popup::is_any_open(&ctx)
            && pos.is_some_and(|pos| {
                !area_rect.contains(pos) && !popup_rect.is_some_and(|r| r.contains(pos))
            });
        if outside || (escape && !ribbon.keytips.is_active()) {
            ribbon.body_open = false;
        }
    }
    // Executar uma Ação fecha o corpo aberto por cima do tabuleiro.
    // (marcar Painéis em série não fecha: são caixas de seleção, como no menu "Painéis ▾").
    if env.out.iter().any(|a| a.panel().is_none()) && collapsed {
        ribbon.body_open = false;
    }
    for action in env.out.drain(..) {
        requests.write(ActionRequested(action));
    }
    // O diálogo "Salvar layout…" fica fora do corpo do Ribbon.
    layout_dialog::show(&ctx, env.i18n, env.sets, &mut env.set_out);
    // Abrir um Layout também fecha o corpo aberto por cima do tabuleiro; salvar e gerenciar não.
    if collapsed
        && env.set_out.iter().any(|r| {
            matches!(
                r,
                ScreenLayoutRequested::Open(_) | ScreenLayoutRequested::ShowOnly(_)
            )
        })
    {
        ribbon.body_open = false;
    }
    for request in env.set_out.drain(..) {
        set_requests.write(request);
    }
    ribbon.registered.tabs = tab_rects;
    ribbon.registered.actions = std::mem::take(&mut env.rects);

    paint_keytips(&ctx, p, &ribbon, keytip_level);
    Ok(())
}

fn palette_of(ctx: &egui::Context) -> &'static Palette {
    use crate::ui_theme::ThemeKind;
    Palette::of(ThemeKind::from_dark_mode(
        ctx.global_style().visuals.dark_mode,
    ))
}

// --- KeyTips ---

/// Metade da altura do rótulo de uma aba: o selo começa logo abaixo dela.
const TAB_LABEL_HALF_H: f32 = 9.0;
/// Selo do KeyTip: altura de 18px e largura mínima de 20px.
const KEYTIP_H: f32 = 18.0;
const KEYTIP_MIN_W: f32 = 20.0;

/// Retângulo do selo para um item. `tab` é o selo de uma aba: fica logo abaixo do rótulo
/// (centralizado no item). Nos botões grandes o selo cobre o ícone, nos pequenos o ícone ou a
/// caixa à esquerda, e nos só de ícone fica preso à borda de baixo; nunca cobre o rótulo.
pub(super) fn keytip_badge_rect(spot: Option<KeyTipSpot>, item: Rect, size: Vec2) -> Rect {
    let min = match spot {
        // Aba.
        None => pos2(
            item.center().x - size.x / 2.0,
            item.center().y + TAB_LABEL_HALF_H,
        ),
        Some(KeyTipSpot::BigIcon) => pos2(item.center().x - size.x / 2.0, item.top() + 11.0),
        Some(KeyTipSpot::Leading) => pos2(item.left() + 2.0, item.center().y - size.y / 2.0),
        Some(KeyTipSpot::Below) => pos2(item.center().x - size.x / 2.0, item.bottom() - 4.0),
    };
    Rect::from_min_size(min, size)
}

/// Selos dos KeyTips em `eval` (12/16 negrito): `ink` sobre `surface-1` invertido. Camada em
/// `Order::Tooltip`, sobre os retângulos que o Ribbon registrou neste quadro.
fn paint_keytips(ctx: &egui::Context, p: &Palette, ribbon: &RibbonState, level: KeyTipLevel) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("chess-studio/keytips"),
    ));
    let badge = |at: Rect, spot: Option<KeyTipSpot>, text: &str| {
        let galley = painter.layout_no_wrap(text.to_owned(), strong_font(ctx, 12.0), p.surface_1);
        let size = vec2((galley.size().x + 8.0).max(KEYTIP_MIN_W), KEYTIP_H);
        let rect = keytip_badge_rect(spot, at, size);
        painter.rect_filled(
            rect.translate(vec2(0.0, 1.0)),
            CornerRadius::same(3),
            p.scrim,
        );
        painter.rect_filled(rect, CornerRadius::same(3), p.ink);
        painter.galley(
            Align2::CENTER_CENTER
                .align_size_within_rect(galley.size(), rect)
                .min,
            galley,
            p.surface_1,
        );
    };
    match level {
        KeyTipLevel::Off => {}
        KeyTipLevel::Tabs => {
            for (view, rect) in &ribbon.registered.tabs {
                badge(*rect, None, view.keytip());
            }
        }
        KeyTipLevel::Actions(_) => {
            let typed = ribbon.keytips.typed();
            for (action, rect, spot) in &ribbon.registered.actions {
                let tip = meta(*action).keytip;
                if !tip.is_empty() && tip.starts_with(typed) {
                    badge(*rect, Some(*spot), tip);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = vec2(20.0, KEYTIP_H);

    #[test]
    fn a_tab_keytip_sits_right_below_the_label() {
        let tab = Rect::from_min_size(pos2(100.0, 0.0), vec2(80.0, TABS_H));
        let label = Rect::from_center_size(tab.center(), vec2(50.0, 16.0));
        let badge = keytip_badge_rect(None, tab, SIZE);
        assert!(!badge.intersects(label), "{badge:?} cobre {label:?}");
        assert!(badge.top() - label.bottom() <= 2.0, "logo abaixo do rótulo");
        assert_eq!(badge.center().x, tab.center().x);
    }

    #[test]
    fn a_big_button_keytip_covers_the_icon_and_never_the_label() {
        let button = Rect::from_min_size(pos2(0.0, 0.0), vec2(64.0, BIG_H));
        // Ícone de 32px no alto; o rótulo de até duas linhas (28px) preso à base, com 4px.
        let icon = Rect::from_min_size(pos2(16.0, 5.0), vec2(32.0, 32.0));
        let label = Rect::from_min_max(pos2(0.0, BIG_H - 4.0 - 28.0), pos2(64.0, BIG_H - 4.0));
        let badge = keytip_badge_rect(Some(KeyTipSpot::BigIcon), button, SIZE);
        assert!(badge.intersects(icon));
        assert!(!badge.intersects(label), "{badge:?} cobre {label:?}");
    }

    #[test]
    fn a_small_button_keytip_stays_over_the_icon_side() {
        let button = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, SMALL_H));
        // Ícone de 16px com 8px de respiro; o rótulo começa depois dele.
        let label = Rect::from_min_max(pos2(30.0, 4.0), pos2(116.0, 20.0));
        let badge = keytip_badge_rect(Some(KeyTipSpot::Leading), button, SIZE);
        assert!(button.contains_rect(badge));
        assert!(!badge.intersects(label), "{badge:?} cobre {label:?}");
    }

    #[test]
    fn the_group_row_fits_three_24px_buttons_and_the_caption() {
        assert_eq!(SMALL_H, 24.0);
        assert_eq!(STACK_H + GROUP_LABEL_H + 8.0, CLASSIC_H);
    }
}
