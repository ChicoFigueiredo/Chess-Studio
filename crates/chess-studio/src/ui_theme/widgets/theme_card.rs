//! Cartão de Tema da galeria de Opções > Aparência (issue #206): amostra do Tema (fundo
//! `surface-1`, uma faixa em `accent`, um traço em `ink`) e o nome. Contorno `border-strong`; o
//! escolhido em `accent` de 2px com visto; foco do teclado com `focus-ring`.

use super::*;
use crate::ui_theme::ThemeSwatch;

/// Como o cartão aparece na galeria.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeCardMark {
    /// Nem escolhido nem em uso.
    None,
    /// O Tema fixo escolhido: contorno `accent` de 2px e visto.
    Selected,
    /// Em "Seguir o sistema": o Tema que está valendo agora (contorno `accent` fino, sem visto).
    Effective,
}

/// Largura e altura do cartão (≥ 24px de alvo).
const THEME_CARD_SIZE: Vec2 = Vec2::new(132.0, 76.0);

/// Cartão de Tema. O tooltip leva o nome e a inspiração do Tema.
pub fn cs_theme_card(ui: &mut Ui, swatch: &ThemeSwatch, mark: ThemeCardMark) -> Response {
    let (rect, response) = ui.allocate_exact_size(THEME_CARD_SIZE, Sense::click());
    let selected = mark == ThemeCardMark::Selected;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, swatch.name)
    });
    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let state = visual_state(&response, true);
        let fill = match state {
            VisualState::Hover => p.surface_3,
            VisualState::Pressed => p.surface_4,
            _ => p.surface_2,
        };
        let radius = CornerRadius::same(6);
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);

        // Amostra: o fundo do painel do Tema, com a faixa de destaque e um traço de texto.
        let sample =
            Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(rect.width() - 12.0, 44.0));
        painter.rect(
            sample,
            CornerRadius::same(4),
            swatch.surface_1,
            hairline(p.border),
            StrokeKind::Inside,
        );
        let bar = Rect::from_min_size(sample.min + vec2(8.0, 9.0), vec2(30.0, 6.0));
        painter.rect_filled(bar, CornerRadius::same(3), swatch.accent);
        let line = Rect::from_min_size(sample.min + vec2(8.0, 24.0), vec2(64.0, 3.0));
        painter.rect_filled(line, CornerRadius::same(2), swatch.ink);
        let short = Rect::from_min_size(sample.min + vec2(8.0, 31.0), vec2(40.0, 3.0));
        painter.rect_filled(
            short,
            CornerRadius::same(2),
            swatch.ink.gamma_multiply(0.55),
        );

        // Nome.
        let font = egui::TextStyle::Body.resolve(ui.style());
        let galley = painter.layout(swatch.name.to_owned(), font, p.ink, rect.width() - 16.0);
        painter.galley(
            pos2(rect.left() + 8.0, rect.bottom() - 8.0 - galley.size().y),
            galley,
            p.ink,
        );

        // Contorno e visto.
        match mark {
            ThemeCardMark::Selected => {
                painter.rect_stroke(rect, radius, Stroke::new(2.0, p.accent), StrokeKind::Inside);
                let center = pos2(sample.right() - 11.0, sample.top() + 11.0);
                painter.circle_filled(center, 8.0, p.accent);
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    icon::CHECK,
                    icon_font(ui.ctx(), 11.0),
                    p.on_accent,
                );
            }
            ThemeCardMark::Effective => {
                painter.rect_stroke(rect, radius, hairline(p.accent), StrokeKind::Inside);
            }
            ThemeCardMark::None => {
                painter.rect_stroke(rect, radius, hairline(p.border_strong), StrokeKind::Inside);
            }
        }
        paint_focus_ring(ui, rect, radius, &response);
    }
    let tip = ActionTip::new(swatch.name).description(swatch.inspiration);
    action_tooltip(response, &tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::{ThemeId, themes_by_family};

    fn swatch_of(key: &str) -> ThemeSwatch {
        let id = ThemeId::from_key(key).expect("tema");
        themes_by_family()
            .into_iter()
            .flat_map(|(_, list)| list)
            .find(|s| s.id == id)
            .expect("amostra")
    }

    fn frame(
        ctx: &Context,
        events: Vec<egui::Event>,
        sw: &ThemeSwatch,
        mark: ThemeCardMark,
    ) -> Response {
        let raw = egui::RawInput {
            events,
            ..egui::RawInput::default()
        };
        let mut out = None;
        let mut full = ctx.run_ui(raw, |ui| {
            out = Some(cs_theme_card(ui, sw, mark));
        });
        full.textures_delta.clear();
        out.expect("resposta")
    }

    #[test]
    fn the_card_is_at_least_24px_and_clickable() {
        const { assert!(THEME_CARD_SIZE.x >= 24.0 && THEME_CARD_SIZE.y >= 24.0) };
        let ctx = Context::default();
        let sw = swatch_of("feltro");
        let rect = frame(&ctx, vec![], &sw, ThemeCardMark::None).rect;
        let at = rect.center();
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            vec![egui::Event::PointerMoved(at)],
            &sw,
            ThemeCardMark::None,
        );
        frame(&ctx, vec![click(true)], &sw, ThemeCardMark::None);
        assert!(frame(&ctx, vec![click(false)], &sw, ThemeCardMark::None).clicked());
    }

    #[test]
    fn enter_activates_the_focused_card() {
        let ctx = Context::default();
        let sw = swatch_of("torneio");
        let first = frame(&ctx, vec![], &sw, ThemeCardMark::None);
        ctx.memory_mut(|m| m.request_focus(first.id));
        frame(&ctx, vec![], &sw, ThemeCardMark::None);
        let key = |pressed| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let r = frame(&ctx, vec![key(true), key(false)], &sw, ThemeCardMark::None);
        assert!(r.clicked(), "Enter no cartão com foco aplica o Tema");
    }
}
