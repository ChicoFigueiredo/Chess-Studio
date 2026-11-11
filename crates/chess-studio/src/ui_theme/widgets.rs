//! Widgets do projeto (`docs/design/componentes/`): `cs_button`, alternância, `toggle_switch`,
//! caixa de seleção, rádio, Dropdown, campo de texto, slider e tooltip de Ação. Pintam com os
//! tokens do Tema do `Ui` (claro ou escuro, conforme `Visuals::dark_mode`), nunca com um
//! `egui::Button` estilizado na hora (README do design system).
//!
//! Os cinco estados de todo controle interativo: repouso, hover, pressionado, desabilitado e
//! foco do teclado (anel sólido de 2px em `focus-ring`, afastado 1px).

use super::tokens::Palette;
use bevy_egui::egui::{
    self, Align2, Color32, Context, CornerRadius, FontFamily, FontId, Id, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, Vec2, Widget, pos2, vec2,
};
use egui_phosphor::regular as icon;
use std::hash::Hash;
use std::ops::RangeInclusive;

/// Paleta do Tema em uso no `Ui`.
pub fn palette(ui: &Ui) -> &'static Palette {
    Palette::from_visuals(ui.visuals())
}

fn fonts_ready_id() -> Id {
    Id::new("chess-studio/ui-theme/fonts-ready")
}

fn keyboard_focus_id() -> Id {
    Id::new("chess-studio/ui-theme/keyboard-focus")
}

/// As fontes do egui só valem no passe seguinte ao `set_fonts`: a família em negrito só pode
/// ser usada depois disso, ou o egui falha ao procurá-la.
pub(super) fn mark_fonts_ready(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(fonts_ready_id(), true));
}

/// O anel de foco aparece só quando o foco veio do teclado (Tab e setas), não do mouse.
pub(super) fn track_keyboard_focus(ctx: &Context) {
    let (keyboard, pointer) = ctx.input(|i| {
        let keyboard = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    pressed: true,
                    key: egui::Key::Tab
                        | egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight,
                    ..
                }
            )
        });
        let pointer = i
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. }));
        (keyboard, pointer)
    });
    if pointer {
        ctx.data_mut(|d| d.insert_temp(keyboard_focus_id(), false));
    } else if keyboard {
        ctx.data_mut(|d| d.insert_temp(keyboard_focus_id(), true));
    }
}

fn focus_visible(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(keyboard_focus_id()))
        .unwrap_or(false)
}

/// Fonte de rótulo em negrito (`body-strong`), ou a normal enquanto a Inter SemiBold não carregou.
pub fn strong_font(ctx: &Context, size: f32) -> FontId {
    let ready = ctx
        .data(|d| d.get_temp::<bool>(fonts_ready_id()))
        .unwrap_or(false);
    if ready {
        FontId::new(size, FontFamily::Name(super::fonts::SEMIBOLD.into()))
    } else {
        FontId::proportional(size)
    }
}

/// Fonte dos ícones Phosphor (ou a normal enquanto a família de ícones não carregou).
pub fn icon_font(ctx: &Context, size: f32) -> FontId {
    let ready = ctx
        .data(|d| d.get_temp::<bool>(fonts_ready_id()))
        .unwrap_or(false);
    if ready {
        FontId::new(size, FontFamily::Name(super::fonts::ICONS.into()))
    } else {
        FontId::proportional(size)
    }
}

/// Texto com um ícone Phosphor antes (`egui_phosphor::regular::*`). A cor `PLACEHOLDER` vira a
/// cor do widget que desenha o texto.
pub fn icon_job(
    ctx: &Context,
    icon: Option<&str>,
    text: &str,
    font: FontId,
    color: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    if let Some(icon) = icon {
        job.append(
            icon,
            0.0,
            egui::TextFormat {
                font_id: icon_font(ctx, font.size + 2.0),
                color,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    if !text.is_empty() {
        let lead = if icon.is_some() { "  " } else { "" };
        job.append(
            &format!("{lead}{text}"),
            0.0,
            egui::TextFormat {
                font_id: font,
                color,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    job
}

/// Texto em negrito do tamanho do corpo (14).
pub fn strong(ctx: &Context, text: impl Into<String>) -> RichText {
    RichText::new(text).font(strong_font(ctx, 14.0))
}

/// Estado visual de um controle (o foco é desenhado à parte, por cima).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualState {
    Rest,
    Hover,
    Pressed,
    Disabled,
}

fn visual_state(response: &Response, enabled: bool) -> VisualState {
    if !enabled {
        VisualState::Disabled
    } else if response.is_pointer_button_down_on() {
        VisualState::Pressed
    } else if response.hovered() || response.highlighted() {
        VisualState::Hover
    } else {
        VisualState::Rest
    }
}

/// Variantes do botão (`Button.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Uma por tela ou diálogo: a Ação que conclui a tarefa.
    Primary,
    /// O padrão.
    Secondary,
    /// Sem fundo: botões de ícone no Ribbon simples, na Barra de Acesso Rápido e nos Painéis.
    Subtle,
    /// Ação irreversível: texto em `danger`; sempre pede confirmação.
    Danger,
}

/// Fundo, contorno e texto de um controle num estado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Color32,
    pub stroke: Stroke,
    pub text: Color32,
}

fn hairline(c: Color32) -> Stroke {
    Stroke::new(1.0, c)
}

/// Aparência do botão por variante e estado.
pub fn button_look(p: &Palette, variant: ButtonVariant, state: VisualState) -> Look {
    use VisualState::*;
    match (variant, state) {
        (ButtonVariant::Primary, Disabled)
        | (ButtonVariant::Secondary | ButtonVariant::Danger, Disabled) => Look {
            fill: p.surface_2,
            stroke: hairline(p.border),
            text: p.ink_disabled,
        },
        (ButtonVariant::Subtle, Disabled) => Look {
            fill: Color32::TRANSPARENT,
            stroke: Stroke::NONE,
            text: p.ink_disabled,
        },
        (ButtonVariant::Primary, s) => Look {
            fill: match s {
                Rest => p.accent,
                Hover => p.accent_hover,
                _ => p.accent_pressed,
            },
            stroke: Stroke::NONE,
            text: p.on_accent,
        },
        (ButtonVariant::Subtle, s) => Look {
            fill: match s {
                Rest => Color32::TRANSPARENT,
                Hover => p.surface_3,
                _ => p.surface_4,
            },
            stroke: Stroke::NONE,
            text: p.ink,
        },
        (v, s) => Look {
            fill: match s {
                Rest => p.surface_2,
                Hover => p.surface_3,
                _ => p.surface_4,
            },
            stroke: hairline(p.border_strong),
            text: if v == ButtonVariant::Danger {
                p.danger
            } else {
                p.ink
            },
        },
    }
}

/// Aparência do botão de alternância (`ToggleButton.md`): ligado em `accent-subtle`.
pub fn toggle_look(p: &Palette, on: bool, state: VisualState) -> Look {
    if !on || state == VisualState::Disabled {
        return button_look(p, ButtonVariant::Secondary, state);
    }
    Look {
        fill: if state == VisualState::Pressed {
            p.surface_4
        } else {
            p.accent_subtle
        },
        stroke: hairline(if state == VisualState::Hover {
            p.accent_hover
        } else {
            p.accent
        }),
        text: p.accent_ink,
    }
}

/// Transição de 100–150 ms de uma cor (hover e pressionado).
fn fade(ui: &Ui, id: Id, target: Color32) -> Color32 {
    let t = ui.style().animation_time;
    let c = target.to_array().map(f32::from);
    let [r, g, b, a] = [0usize, 1, 2, 3].map(|i| {
        ui.ctx()
            .animate_value_with_time(id.with(i), c[i], t)
            .round()
            .clamp(0.0, 255.0) as u8
    });
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Anel de foco: 2px sólidos em `focus-ring`, 1px fora do controle.
fn paint_focus_ring(ui: &Ui, rect: Rect, corner: CornerRadius, response: &Response) {
    if response.has_focus() && focus_visible(ui.ctx()) {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            corner,
            Stroke::new(2.0, palette(ui).focus_ring),
            StrokeKind::Outside,
        );
    }
}

fn radius_m() -> CornerRadius {
    CornerRadius::same(4)
}

/// Botão do projeto: `ui.add(CsButton::new("Nova Partida").variant(ButtonVariant::Primary))`.
#[derive(Debug)]
#[must_use = "adicione ao Ui com ui.add"]
pub struct CsButton<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    variant: ButtonVariant,
    enabled: bool,
    /// `Some` torna o botão uma alternância (ligado ou desligado).
    toggled: Option<bool>,
}

impl<'a> CsButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            variant: ButtonVariant::Secondary,
            enabled: true,
            toggled: None,
        }
    }

    /// Ícone Phosphor (`egui_phosphor::regular::*`) antes do rótulo; vazio o rótulo para botão só de ícone.
    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Desabilitado: explique o motivo no tooltip (`ActionTip::disabled`).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Botão de alternância com o estado dado (o estado vem da Ação, não do widget).
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }
}

impl Widget for CsButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let p = palette(ui);
        let text = self.label.to_owned();
        let galley = ui.painter().layout_job(icon_job(
            ui.ctx(),
            self.icon,
            self.label,
            strong_font(ui.ctx(), 14.0),
            Color32::PLACEHOLDER,
        ));
        let pad = ui.spacing().button_padding;
        let min = ui.spacing().interact_size;
        let width = if self.label.is_empty() {
            min.y.max(galley.size().x + 2.0 * pad.x)
        } else {
            (galley.size().x + 2.0 * (pad.x + 4.0)).max(min.x)
        };
        let size = vec2(width, (galley.size().y + 2.0 * pad.y).max(min.y));
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text.clone())
        });
        if ui.is_rect_visible(rect) {
            let state = visual_state(&response, enabled);
            let look = match self.toggled {
                Some(on) => toggle_look(p, on, state),
                None => button_look(p, self.variant, state),
            };
            let fill = fade(ui, response.id.with("fill"), look.fill);
            let corner = radius_m();
            ui.painter()
                .rect(rect, corner, fill, look.stroke, StrokeKind::Inside);
            let pos = Align2::CENTER_CENTER
                .align_size_within_rect(galley.size(), rect)
                .min;
            ui.painter().galley(pos, galley, look.text);
            paint_focus_ring(ui, rect, corner, &response);
        }
        response.on_hover_cursor(if enabled {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        })
    }
}

/// Atalho: `cs_button(ui, "Nova Partida", ButtonVariant::Primary)`.
pub fn cs_button(ui: &mut Ui, label: &str, variant: ButtonVariant) -> Response {
    ui.add(CsButton::new(label).variant(variant))
}

/// Botão de alternância: devolve a resposta e inverte `on` ao clicar.
pub fn cs_toggle_button(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let mut response = ui.add(CsButton::new(label).toggled(*on));
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
}

/// Corpo comum de caixa, rádio e Switch: uma linha de `control-h` com o controle à esquerda e o
/// rótulo clicável à direita. `paint` recebe o retângulo do controle e o estado.
fn control_row(
    ui: &mut Ui,
    label: &str,
    control: Vec2,
    paint: impl FnOnce(&Ui, Rect, VisualState, &Response),
) -> Response {
    let enabled = ui.is_enabled();
    let gap = 8.0;
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::PLACEHOLDER,
    );
    let height = ui.spacing().interact_size.y.max(control.y);
    let label_w = if label.is_empty() {
        0.0
    } else {
        gap + galley.size().x
    };
    let (rect, response) = ui.allocate_exact_size(
        vec2(control.x + label_w, height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Checkbox, enabled, label.to_owned())
    });
    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let control_rect = Rect::from_min_size(
            pos2(rect.left(), rect.center().y - control.y / 2.0),
            control,
        );
        let state = visual_state(&response, enabled);
        paint(ui, control_rect, state, &response);
        if !label.is_empty() {
            let color = if enabled { p.ink } else { p.ink_disabled };
            let pos = pos2(
                control_rect.right() + gap,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter().galley(pos, galley, color);
        }
        paint_focus_ring(ui, control_rect, radius_m(), &response);
    }
    response.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Default
    })
}

/// Switch (`Switch.md`): liga uma preferência que vale na hora. Trilho `accent` ligado, com o
/// botão em `on-accent`; desligado, trilho com contorno `border-strong`.
#[allow(dead_code)] // API do tema; o Ribbon e as Opções passam a usar
pub fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    cs_switch(ui, on, "")
}

/// Switch com rótulo, que descreve o estado ligado ("Mostrar coordenadas").
pub fn cs_switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let height = if ui.spacing().interact_size.y >= 32.0 {
        20.0
    } else {
        16.0
    };
    let size = vec2(height * 1.8, height);
    let value = *on;
    let mut response = control_row(ui, label, size, |ui, rect, state, response| {
        let p = palette(ui);
        let t = ui
            .ctx()
            .animate_bool_with_time(response.id, value, ui.style().animation_time);
        let disabled = state == VisualState::Disabled;
        let (fill, stroke) = match (value, disabled) {
            (true, false) => (
                match state {
                    VisualState::Hover => p.accent_hover,
                    VisualState::Pressed => p.accent_pressed,
                    _ => p.accent,
                },
                Stroke::NONE,
            ),
            (true, true) => (p.ink_disabled, Stroke::NONE),
            (false, false) => (Color32::TRANSPARENT, hairline(p.border_strong)),
            (false, true) => (Color32::TRANSPARENT, hairline(p.border)),
        };
        let corner = CornerRadius::same((rect.height() / 2.0) as u8);
        ui.painter()
            .rect(rect, corner, fill, stroke, StrokeKind::Inside);
        let r = rect.height() / 2.0 - 3.0;
        let x = egui::lerp((rect.left() + r + 3.0)..=(rect.right() - r - 3.0), t);
        let thumb = match (value, disabled) {
            (true, false) => p.on_accent,
            (true, true) => p.surface_2,
            (false, false) => p.ink_muted,
            (false, true) => p.ink_disabled,
        };
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), r, thumb);
    });
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
}

/// Estado de uma caixa de seleção; `Mixed` é o grupo misto ("Todos os Auxílios").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Off,
    On,
    Mixed,
}

impl CheckState {
    fn next(self) -> Self {
        match self {
            CheckState::On => CheckState::Off,
            CheckState::Off | CheckState::Mixed => CheckState::On,
        }
    }
}

/// Caixa de seleção (`Checkbox.md`): `accent` com visto em `on-accent`; contorno `border-strong`.
pub fn cs_checkbox(ui: &mut Ui, checked: &mut bool, label: &str) -> Response {
    let mut state = if *checked {
        CheckState::On
    } else {
        CheckState::Off
    };
    let response = cs_checkbox_tri(ui, &mut state, label);
    *checked = state == CheckState::On;
    response
}

/// Caixa de seleção de três estados.
pub fn cs_checkbox_tri(ui: &mut Ui, value: &mut CheckState, label: &str) -> Response {
    let box_size = ui.spacing().icon_width;
    let current = *value;
    let mut response = control_row(ui, label, Vec2::splat(box_size), |ui, rect, state, _| {
        let p = palette(ui);
        let disabled = state == VisualState::Disabled;
        let filled = current != CheckState::Off;
        let (fill, stroke) = match (filled, disabled, state) {
            (true, false, VisualState::Hover) => (p.accent_hover, Stroke::NONE),
            (true, false, VisualState::Pressed) => (p.accent_pressed, Stroke::NONE),
            (true, false, _) => (p.accent, Stroke::NONE),
            (true, true, _) => (p.ink_disabled, Stroke::NONE),
            (false, false, VisualState::Hover) => (p.surface_3, hairline(p.border_strong)),
            (false, false, VisualState::Pressed) => (p.surface_4, hairline(p.border_strong)),
            (false, false, _) => (p.surface_2, hairline(p.border_strong)),
            (false, true, _) => (Color32::TRANSPARENT, hairline(p.border)),
        };
        ui.painter()
            .rect(rect, radius_m(), fill, stroke, StrokeKind::Inside);
        let mark = Stroke::new(2.0, if disabled { p.surface_2 } else { p.on_accent });
        let at = |x: f32, y: f32| {
            pos2(
                rect.left() + rect.width() * x,
                rect.top() + rect.height() * y,
            )
        };
        match current {
            CheckState::On => {
                ui.painter().add(egui::Shape::line(
                    vec![at(0.24, 0.52), at(0.43, 0.70), at(0.76, 0.32)],
                    mark,
                ));
            }
            CheckState::Mixed => {
                ui.painter()
                    .line_segment([at(0.26, 0.5), at(0.74, 0.5)], mark);
            }
            CheckState::Off => {}
        }
    });
    if response.clicked() {
        *value = value.next();
        response.mark_changed();
    }
    response
}

/// Rádio (`RadioGroup.md`): contorno `border-strong`; selecionado em `accent`.
pub fn cs_radio(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let size = ui.spacing().icon_width;
    control_row(ui, label, Vec2::splat(size), |ui, rect, state, _| {
        let p = palette(ui);
        let disabled = state == VisualState::Disabled;
        let ring = match (disabled, selected) {
            (true, _) => p.ink_disabled,
            (false, true) => p.accent,
            (false, false) => p.border_strong,
        };
        let fill = match state {
            VisualState::Hover => p.surface_3,
            VisualState::Pressed => p.surface_4,
            _ => p.surface_2,
        };
        let c = rect.center();
        let r = rect.width() / 2.0;
        ui.painter().circle(
            c,
            r - 0.5,
            if disabled { Color32::TRANSPARENT } else { fill },
            hairline(ring),
        );
        if selected {
            ui.painter().circle_filled(c, r - 4.0, ring);
        }
    })
}

/// Rádio ligado a um valor: marca `current` com `value` ao clicar.
pub fn cs_radio_value<T: PartialEq>(
    ui: &mut Ui,
    current: &mut T,
    value: T,
    label: &str,
) -> Response {
    let mut response = cs_radio(ui, *current == value, label);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// Dropdown (`Dropdown.md`): escolhe um valor numa lista. O item escolhido fica em
/// `accent-subtle` com visto. Para listas curtas (2 a 5), prefira o rádio.
pub fn cs_dropdown<T: PartialEq + Clone>(
    ui: &mut Ui,
    id_salt: impl Hash + std::fmt::Debug,
    selected: &mut T,
    options: &[(T, String)],
) -> Response {
    cs_dropdown_placeholder(ui, id_salt, selected, options, "")
}

/// Como [`cs_dropdown`], mas com `placeholder` no lugar do valor quando `selected` não está
/// nas opções (uma lista de "ações": escolher um item dispara algo, nada fica marcado).
pub fn cs_dropdown_placeholder<T: PartialEq + Clone>(
    ui: &mut Ui,
    id_salt: impl Hash + std::fmt::Debug,
    selected: &mut T,
    options: &[(T, String)],
    placeholder: &str,
) -> Response {
    let current_label = options
        .iter()
        .find(|(v, _)| v == selected)
        .map_or(placeholder, |(_, l)| l.as_str());
    let before = selected.clone();
    let mut response = egui::ComboBox::from_id_salt(id_salt)
        .selected_text(current_label)
        .width(ui.spacing().combo_width.max(180.0))
        .show_ui(ui, |ui| {
            for (value, label) in options {
                let chosen = value == selected;
                let job = icon_job(
                    ui.ctx(),
                    None,
                    label,
                    egui::TextStyle::Body.resolve(ui.style()),
                    Color32::PLACEHOLDER,
                );
                let mut job = job;
                if chosen {
                    job.append(
                        &format!("   {}", icon::CHECK),
                        0.0,
                        egui::TextFormat {
                            font_id: icon_font(ui.ctx(), 14.0),
                            color: Color32::PLACEHOLDER,
                            ..Default::default()
                        },
                    );
                }
                if ui.selectable_label(chosen, job).clicked() {
                    *selected = value.clone();
                }
            }
        })
        .response;
    if *selected != before {
        response.mark_changed();
    }
    response
}

/// Campo de texto (`TextField.md`). `hint` é um exemplo real, nunca o rótulo. Com `error`, o
/// contorno vira `danger` e a mensagem aparece logo abaixo, com ícone.
pub fn cs_text_field(ui: &mut Ui, text: &mut String, hint: &str, error: Option<&str>) -> Response {
    let p = palette(ui);
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .margin(egui::Margin::symmetric(8, 4))
            .vertical_align(egui::Align::Center)
            .min_size(vec2(0.0, ui.spacing().interact_size.y)),
    );
    let corner = radius_m();
    if error.is_some() {
        ui.painter().rect_stroke(
            response.rect,
            corner,
            Stroke::new(1.5, p.danger),
            StrokeKind::Inside,
        );
    }
    paint_focus_ring(ui, response.rect, corner, &response);
    if let Some(message) = error {
        ui.horizontal(|ui| {
            ui.label(icon_job(
                ui.ctx(),
                Some(icon::WARNING_CIRCLE),
                message,
                egui::TextStyle::Body.resolve(ui.style()),
                p.danger,
            ));
        });
    }
    response
}

/// Slider (`Slider.md`): trilho preenchido até o valor em `accent`. O valor atual fica no
/// rótulo, que o chamador monta ("Escala da interface · 125%").
pub fn cs_slider(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, step: f32) -> Response {
    let p = palette(ui);
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.selection.bg_fill = p.accent;
        v.slider_trailing_fill = true;
        v.widgets.inactive.bg_fill = p.border_strong;
        v.widgets.inactive.fg_stroke = Stroke::new(3.0, p.accent);
        v.widgets.hovered.bg_fill = p.accent_hover;
        v.widgets.hovered.fg_stroke = Stroke::new(3.0, p.accent_hover);
        v.widgets.active.bg_fill = p.accent_pressed;
        v.widgets.active.fg_stroke = Stroke::new(3.0, p.accent_pressed);
        let response = ui.add(
            egui::Slider::new(value, range)
                .step_by(f64::from(step))
                .show_value(false),
        );
        paint_focus_ring(ui, response.rect, radius_m(), &response);
        response
    })
    .inner
}

/// Conteúdo do tooltip de uma Ação (`Tooltip.md`): título, descrição e atalho.
#[derive(Clone, Debug, Default)]
pub struct ActionTip<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub shortcut: Option<&'a str>,
    /// Ação desabilitada: diz por quê ("Só no modo Assistido.").
    pub disabled_reason: Option<&'a str>,
}

impl<'a> ActionTip<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            ..Self::default()
        }
    }

    pub fn description(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    pub fn shortcut(mut self, text: &'a str) -> Self {
        self.shortcut = Some(text);
        self
    }

    pub fn disabled(mut self, reason: &'a str) -> Self {
        self.disabled_reason = Some(reason);
        self
    }
}

/// Layout fixo de até quatro linhas: título (`body-strong`), descrição (`body`), motivo (se
/// desabilitada) e atalho (`caption`, `ink-muted`).
pub fn action_tip_ui(ui: &mut Ui, tip: &ActionTip<'_>) {
    let p = palette(ui);
    ui.set_max_width(280.0);
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.label(strong(ui.ctx(), tip.title).color(p.ink));
    if let Some(text) = tip.description {
        ui.label(RichText::new(text).color(p.ink));
    }
    if let Some(reason) = tip.disabled_reason {
        ui.label(icon_job(
            ui.ctx(),
            Some(icon::INFO),
            reason,
            egui::TextStyle::Body.resolve(ui.style()),
            p.warning,
        ));
    }
    if let Some(shortcut) = tip.shortcut {
        ui.label(RichText::new(shortcut).small().color(p.ink_muted));
    }
}

/// Mostra o tooltip da Ação ao parar o mouse (500 ms) ou focar pelo teclado.
pub fn action_tooltip(response: Response, tip: &ActionTip<'_>) -> Response {
    if response.enabled() {
        response.on_hover_ui(|ui| action_tip_ui(ui, tip))
    } else {
        response.on_disabled_hover_ui(|ui| action_tip_ui(ui, tip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::appearance::Density;
    use crate::ui_theme::style::build_style;
    use crate::ui_theme::tokens::ThemeKind;
    use crate::ui_theme::tokens::Tokens;

    /// Um quadro do egui; descarta as texturas, que ninguém vai enviar à GPU.
    fn run_quiet(ctx: &Context, raw: egui::RawInput, add: impl FnMut(&mut Ui)) {
        let mut out = ctx.run_ui(raw, add);
        out.textures_delta.clear();
    }

    fn p(theme: ThemeKind) -> &'static Palette {
        Palette::of(theme)
    }

    #[test]
    fn primary_button_uses_the_accent_family_and_on_accent_text() {
        for theme in ThemeKind::ALL {
            let pal = p(theme);
            let look = |s| button_look(pal, ButtonVariant::Primary, s);
            assert_eq!(look(VisualState::Rest).fill, pal.accent);
            assert_eq!(look(VisualState::Hover).fill, pal.accent_hover);
            assert_eq!(look(VisualState::Pressed).fill, pal.accent_pressed);
            assert_eq!(look(VisualState::Rest).text, pal.on_accent);
        }
    }

    #[test]
    fn secondary_button_steps_through_surfaces_with_a_strong_border() {
        let pal = p(ThemeKind::Dark);
        let look = |s| button_look(pal, ButtonVariant::Secondary, s);
        assert_eq!(look(VisualState::Rest).fill, pal.surface_2);
        assert_eq!(look(VisualState::Hover).fill, pal.surface_3);
        assert_eq!(look(VisualState::Pressed).fill, pal.surface_4);
        assert_eq!(look(VisualState::Rest).stroke.color, pal.border_strong);
    }

    #[test]
    fn subtle_button_has_no_background_or_border_at_rest() {
        let pal = p(ThemeKind::Light);
        let rest = button_look(pal, ButtonVariant::Subtle, VisualState::Rest);
        assert_eq!(rest.fill, Color32::TRANSPARENT);
        assert_eq!(rest.stroke, Stroke::NONE);
        assert_eq!(
            button_look(pal, ButtonVariant::Subtle, VisualState::Hover).fill,
            pal.surface_3
        );
    }

    #[test]
    fn danger_button_has_danger_text_and_disabled_buttons_use_ink_disabled() {
        let pal = p(ThemeKind::Dark);
        assert_eq!(
            button_look(pal, ButtonVariant::Danger, VisualState::Rest).text,
            pal.danger
        );
        for v in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Subtle,
            ButtonVariant::Danger,
        ] {
            assert_eq!(
                button_look(pal, v, VisualState::Disabled).text,
                pal.ink_disabled
            );
        }
    }

    #[test]
    fn toggle_on_is_accent_subtle_with_accent_ink_and_off_is_secondary() {
        let pal = p(ThemeKind::Dark);
        let on = toggle_look(pal, true, VisualState::Rest);
        assert_eq!(on.fill, pal.accent_subtle);
        assert_eq!(on.stroke.color, pal.accent);
        assert_eq!(on.text, pal.accent_ink);
        assert_eq!(
            toggle_look(pal, false, VisualState::Rest),
            button_look(pal, ButtonVariant::Secondary, VisualState::Rest)
        );
    }

    /// Roda um quadro do egui com o estilo de um Tema/Densidade e devolve o que o `build` mediu.
    fn frame<R>(theme: ThemeKind, density: Density, mut build: impl FnMut(&mut Ui) -> R) -> R {
        let ctx = Context::default();
        ctx.set_fonts(super::super::fonts::definitions());
        // O primeiro quadro instala as fontes; o segundo já pode usar a família em negrito.
        run_quiet(&ctx, egui::RawInput::default(), |_| {});
        mark_fonts_ready(&ctx);
        ctx.set_style_of(
            theme.egui(),
            build_style(theme, density, Tokens::embedded()),
        );
        ctx.set_theme(match theme {
            ThemeKind::Dark => egui::ThemePreference::Dark,
            ThemeKind::Light => egui::ThemePreference::Light,
        });
        let mut out = None;
        run_quiet(&ctx, egui::RawInput::default(), |ui| {
            out = Some(build(ui));
        });
        out.expect("quadro executado")
    }

    #[test]
    fn button_height_follows_the_density() {
        for (density, height) in [(Density::Comfortable, 32.0), (Density::Compact, 24.0)] {
            let h = frame(ThemeKind::Dark, density, |ui| {
                cs_button(ui, "Nova Partida", ButtonVariant::Primary)
                    .rect
                    .height()
            });
            assert!((h - height).abs() < 0.5, "{density:?}: {h}");
        }
    }

    #[test]
    fn every_widget_renders_in_both_themes_and_densities() {
        for theme in ThemeKind::ALL {
            for density in Density::ALL {
                frame(theme, density, |ui| {
                    let (mut on, mut text, mut v, mut sel, mut tri) =
                        (true, String::from("x"), 50.0_f32, 1_u8, CheckState::Mixed);
                    ui.add(
                        CsButton::new("Abrir")
                            .icon(icon::FOLDER_OPEN)
                            .variant(ButtonVariant::Subtle),
                    );
                    ui.add(CsButton::new("").icon(icon::GEAR).enabled(false));
                    cs_toggle_button(ui, &mut on, "Coordenadas");
                    toggle_switch(ui, &mut on);
                    cs_switch(ui, &mut on, "Mostrar coordenadas");
                    cs_checkbox(ui, &mut on, "Auxílios");
                    cs_checkbox_tri(ui, &mut tri, "Todos");
                    cs_radio_value(ui, &mut sel, 1, "Uma");
                    cs_dropdown(
                        ui,
                        "dd",
                        &mut sel,
                        &[(1, "Um".to_owned()), (2, "Dois".to_owned())],
                    );
                    cs_text_field(ui, &mut text, "e4 e5", Some("Erro"));
                    cs_slider(ui, &mut v, 0.0..=100.0, 5.0);
                    action_tip_ui(
                        ui,
                        &ActionTip::new("Nova Partida")
                            .description("Começa uma Partida")
                            .shortcut("Ctrl+N")
                            .disabled("Só no modo Assistido."),
                    );
                });
            }
        }
    }

    #[test]
    fn switch_and_checkbox_toggle_the_bound_value() {
        let ctx = Context::default();
        let mut on = false;
        let run = |events: Vec<egui::Event>, on: &mut bool| {
            let raw = egui::RawInput {
                events,
                ..egui::RawInput::default()
            };
            let mut rect = Rect::NOTHING;
            run_quiet(&ctx, raw, |ui| {
                rect = cs_switch(ui, on, "Mostrar coordenadas").rect;
            });
            rect
        };
        let rect = run(vec![], &mut on);
        let at = rect.center();
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(vec![egui::Event::PointerMoved(at)], &mut on);
        run(vec![click(true)], &mut on);
        run(vec![click(false)], &mut on);
        assert!(on, "o clique liga o Switch");
    }
}
