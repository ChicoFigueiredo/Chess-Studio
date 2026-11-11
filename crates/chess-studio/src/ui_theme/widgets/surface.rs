//! Superfícies de página: cartão (`CardFrame`), botão "Detalhes" que abre e fecha
//! (`Disclosure`) e a linha rótulo-valor dos detalhes (`DetailRow`). Servem às páginas dos
//! bastidores, como a Central de Conexões (#225); só usam tokens.

use super::{ActionTip, ButtonVariant, CsButton, action_tooltip, palette};
use crate::ui_theme::tokens::Palette;
use bevy_egui::egui::{self, CornerRadius, Margin, Response, RichText, Stroke, Ui, vec2};
use egui_phosphor::regular as icon;

/// Quanto o cartão pede atenção.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardEmphasis {
    /// Cartão completo: `surface-2` com contorno forte.
    Full,
    /// Cartão menor, sem fundo: Provedores que ainda não funcionam.
    Quiet,
}

/// Margem interna do cartão (`space-4`) e do cartão discreto (`space-3`).
pub fn card_margin(emphasis: CardEmphasis) -> i8 {
    match emphasis {
        CardEmphasis::Full => 16,
        CardEmphasis::Quiet => 12,
    }
}

/// O quadro do cartão, com os tokens do Tema.
pub fn cs_card_frame(p: &Palette, emphasis: CardEmphasis) -> egui::Frame {
    let frame = egui::Frame::new()
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(card_margin(emphasis)));
    match emphasis {
        CardEmphasis::Full => frame
            .fill(p.surface_2)
            .stroke(Stroke::new(1.0, p.border_strong)),
        CardEmphasis::Quiet => frame
            .fill(egui::Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, p.border)),
    }
}

/// Botão "Detalhes" com a seta para baixo (fechado) ou para cima (aberto). Clicar inverte
/// `open`. `hint` é o tooltip.
pub fn cs_disclosure(ui: &mut Ui, open: &mut bool, label: &str, hint: &str) -> Response {
    let glyph = if *open {
        icon::CARET_UP
    } else {
        icon::CARET_DOWN
    };
    let mut response = ui.add(
        CsButton::new(label)
            .icon(glyph)
            .variant(ButtonVariant::Subtle),
    );
    if response.clicked() {
        *open = !*open;
        response.mark_changed();
    }
    action_tooltip(response, &ActionTip::new(hint))
}

/// Largura da coluna de rótulos das linhas de detalhe.
pub const DETAIL_LABEL_WIDTH: f32 = 148.0;
/// Abaixo desta largura o rótulo vai acima do valor.
pub const DETAIL_STACK_BELOW: f32 = 340.0;

/// Uma linha dos detalhes: rótulo em `caption` e `ink-muted` à esquerda; o valor, desenhado por
/// `value`, à direita (ou abaixo, se o espaço é curto).
pub fn cs_detail_row(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
    let p = palette(ui);
    let caption = RichText::new(label)
        .text_style(egui::TextStyle::Small)
        .color(p.ink_muted);
    if ui.available_width() < DETAIL_STACK_BELOW {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(caption);
            value(ui);
        });
        return;
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.allocate_ui_with_layout(
            vec2(DETAIL_LABEL_WIDTH, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(DETAIL_LABEL_WIDTH);
                // Alinha o rótulo de 12/16 com a primeira linha do corpo de 14/20.
                ui.add_space(2.0);
                ui.label(caption);
            },
        );
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            value(ui);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::ThemeKind;

    #[test]
    fn the_full_card_is_surface_2_with_a_strong_border_and_the_quiet_one_has_no_fill() {
        for kind in ThemeKind::ALL {
            let p = Palette::of(kind);
            let full = cs_card_frame(p, CardEmphasis::Full);
            assert_eq!(full.fill, p.surface_2);
            assert_eq!(full.stroke.color, p.border_strong);
            let quiet = cs_card_frame(p, CardEmphasis::Quiet);
            assert_eq!(quiet.fill, egui::Color32::TRANSPARENT);
            assert_eq!(quiet.stroke.color, p.border);
        }
        // Margens na grade de 4px: `space-4` e `space-3`.
        assert_eq!(card_margin(CardEmphasis::Full) % 4, 0);
        assert_eq!(card_margin(CardEmphasis::Quiet) % 4, 0);
        assert!(card_margin(CardEmphasis::Full) > card_margin(CardEmphasis::Quiet));
    }

    fn row_height(width: f32) -> f32 {
        let ctx = crate::ui_theme::test_context(
            crate::ui_theme::ThemeId::factory(crate::ui_theme::ThemeKind::Dark),
            crate::ui_theme::Density::Comfortable,
        );
        let mut height = 0.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(width, 600.0),
            )),
            ..Default::default()
        };
        crate::ui_theme::run_quiet(&ctx, raw, |ui| {
            let top = ui.cursor().min.y;
            cs_detail_row(ui, "Custo", |ui| {
                ui.label("Gratuito, sem cadastro");
            });
            height = ui.cursor().min.y - top;
        });
        height
    }

    #[test]
    fn a_detail_row_puts_the_label_beside_the_value_and_stacks_when_narrow() {
        let wide = row_height(600.0);
        let narrow = row_height(DETAIL_STACK_BELOW - 40.0);
        assert!(
            narrow > wide + 8.0,
            "empilhada ({narrow}) é mais alta que lado a lado ({wide})"
        );
    }

    #[test]
    fn the_disclosure_flips_the_flag() {
        let ctx = crate::ui_theme::test_context(
            crate::ui_theme::ThemeId::factory(crate::ui_theme::ThemeKind::Dark),
            crate::ui_theme::Density::Comfortable,
        );
        let mut open = false;
        let mut rect = egui::Rect::NOTHING;
        crate::ui_theme::run_quiet(&ctx, egui::RawInput::default(), |ui| {
            rect = cs_disclosure(ui, &mut open, "Detalhes", "Mostrar detalhes").rect;
        });
        let click = |pressed| egui::Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(rect.center())],
            vec![click(true)],
            vec![click(false)],
        ] {
            let raw = egui::RawInput {
                events,
                ..Default::default()
            };
            crate::ui_theme::run_quiet(&ctx, raw, |ui| {
                cs_disclosure(ui, &mut open, "Detalhes", "Mostrar detalhes");
            });
        }
        assert!(open, "um clique abre os detalhes");
    }
}
