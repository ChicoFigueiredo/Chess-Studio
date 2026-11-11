//! Menu "⋯" (`OverflowMenu`): um botão só de ícone que abre as Ações secundárias de um cartão
//! ou de uma linha. Cada item é um verbo; o que não está disponível agora continua na lista,
//! desabilitado, e o tooltip diz o motivo em linguagem de usuário (README, "Conteúdo e voz").

use super::{
    ActionTip, ButtonVariant, CsButton, VisualState, action_tooltip, icon_job, paint_focus_ring,
    palette, strong_font, visual_state,
};
use crate::ui_theme::tokens::Palette;
use bevy_egui::egui::{self, Color32, CornerRadius, Sense, Ui, vec2};
use egui_phosphor::regular as icon;

/// Um item do menu.
#[derive(Clone, Copy, Debug)]
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub icon: Option<&'a str>,
    pub enabled: bool,
    /// Por que está desabilitado (tooltip).
    pub reason: Option<&'a str>,
    /// Ação destrutiva: texto em `danger`.
    pub danger: bool,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            enabled: true,
            reason: None,
            danger: false,
        }
    }

    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Desabilitado, com o motivo para o tooltip.
    pub fn disabled(mut self, reason: &'a str) -> Self {
        self.enabled = false;
        self.reason = Some(reason);
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

/// Largura mínima do menu aberto.
pub const MENU_MIN_WIDTH: f32 = 220.0;

/// Cor do texto de um item (o hover e o pressionado trocam só o fundo).
pub fn item_text_color(p: &Palette, item: &MenuItem<'_>) -> Color32 {
    if !item.enabled {
        p.ink_disabled
    } else if item.danger {
        p.danger
    } else {
        p.ink
    }
}

/// O botão "⋯" e o menu. Devolve o índice do item escolhido neste quadro.
pub fn cs_overflow_menu(
    ui: &mut Ui,
    id: egui::Id,
    tooltip: &str,
    items: &[MenuItem<'_>],
) -> Option<usize> {
    let mut chosen = None;
    ui.push_id(id, |ui| {
        let button = ui.add(
            CsButton::new("")
                .icon(icon::DOTS_THREE_OUTLINE)
                .variant(ButtonVariant::Subtle),
        );
        let button = action_tooltip(button, &ActionTip::new(tooltip));
        egui::Popup::menu(&button).show(|ui| {
            ui.set_min_width(MENU_MIN_WIDTH);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, item) in items.iter().enumerate() {
                if menu_item(ui, item).clicked() {
                    chosen = Some(index);
                }
            }
        });
    });
    chosen
}

fn menu_item(ui: &mut Ui, item: &MenuItem<'_>) -> egui::Response {
    let p = palette(ui);
    let color = item_text_color(p, item);
    let galley = ui.painter().layout_job(icon_job(
        ui.ctx(),
        item.icon,
        item.label,
        strong_font(ui.ctx(), 14.0),
        color,
    ));
    let height = ui.spacing().interact_size.y.max(24.0);
    let width = ui.available_width().max(galley.size().x + 24.0);
    let sense = if item.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            item.enabled,
            item.label.to_owned(),
        )
    });
    if ui.is_rect_visible(rect) {
        let state = visual_state(&response, item.enabled);
        let fill = match state {
            VisualState::Hover => p.surface_3,
            VisualState::Pressed => p.surface_4,
            _ => Color32::TRANSPARENT,
        };
        let corner = CornerRadius::same(4);
        ui.painter().rect_filled(rect, corner, fill);
        let pos = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(pos, galley, color);
        paint_focus_ring(ui, rect, corner, &response);
    }
    match item.reason {
        Some(reason) if !item.enabled => {
            action_tooltip(response, &ActionTip::new(item.label).disabled(reason))
        }
        _ => response.on_hover_cursor(egui::CursorIcon::PointingHand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::ThemeKind;

    #[test]
    fn items_carry_their_icon_reason_and_tone() {
        let p = Palette::of(ThemeKind::Dark);
        let normal = MenuItem::new("Limpar cache").icon(icon::BROOM);
        assert!(normal.enabled);
        assert_eq!(item_text_color(p, &normal), p.ink);
        let off = MenuItem::new("Autorizar…").disabled("Em breve.");
        assert!(!off.enabled);
        assert_eq!(off.reason, Some("Em breve."));
        assert_eq!(item_text_color(p, &off), p.ink_disabled);
        let danger = MenuItem::new("Remover").danger();
        assert_eq!(item_text_color(p, &danger), p.danger);
        // Desabilitado vence o perigo: nunca parece clicável.
        let both = MenuItem::new("Remover").danger().disabled("Não agora.");
        assert_eq!(item_text_color(p, &both), p.ink_disabled);
    }

    #[test]
    fn the_closed_menu_is_one_icon_button_that_picks_nothing() {
        let ctx = crate::ui_theme::test_context(
            crate::ui_theme::ThemeId::factory(crate::ui_theme::ThemeKind::Dark),
            crate::ui_theme::Density::Comfortable,
        );
        let mut picked = Some(9);
        crate::ui_theme::run_quiet(&ctx, egui::RawInput::default(), |ui| {
            let items = [MenuItem::new("Um"), MenuItem::new("Dois")];
            picked = cs_overflow_menu(ui, egui::Id::new("menu"), "Mais ações", &items);
        });
        assert_eq!(picked, None);
    }
}
