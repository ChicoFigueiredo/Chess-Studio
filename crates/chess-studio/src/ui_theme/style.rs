//! Tokens -> `egui::Style` e `Visuals` (README do design system, "Implementação em egui").
//! O Tema troca o `Visuals` inteiro; a Densidade muda só o `Spacing`.

use super::appearance::Density;
use super::themes::ThemeId;
use super::tokens::{Palette, ThemeKind, Tokens};
use bevy_egui::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Vec2,
    Visuals, vec2,
};

/// Medidas que dependem da Densidade (alturas e respiros, em pixels lógicos na Escala 100%).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DensityMetrics {
    /// `control-h` / `control-h-compact`: altura de botão, campo e Dropdown.
    pub control_height: f32,
    /// Padding de Painel (`space-4` / `space-3`).
    pub panel_padding: f32,
    pub button_padding: Vec2,
    pub item_spacing: Vec2,
}

impl DensityMetrics {
    pub fn of(density: Density, tokens: &Tokens) -> Self {
        let l = |name: &str| tokens.length(name).unwrap_or(0.0);
        match density {
            Density::Comfortable => Self {
                control_height: l("control-h"),
                panel_padding: l("space-4"),
                button_padding: vec2(l("space-2"), l("space-1") + l("space-05")),
                item_spacing: vec2(l("space-2"), l("space-2")),
            },
            // Um passo abaixo nos respiros (grade de 4px).
            Density::Compact => Self {
                control_height: l("control-h-compact"),
                panel_padding: l("space-3"),
                button_padding: vec2(l("space-2"), l("space-05")),
                item_spacing: vec2(l("space-1"), l("space-1")),
            },
        }
    }
}

fn radius(px: f32) -> CornerRadius {
    CornerRadius::same(px.round().clamp(0.0, 255.0) as u8)
}

/// `Visuals` de um Tema, preenchido só com tokens.
pub fn build_visuals(theme: impl Into<ThemeId>, tokens: &Tokens) -> Visuals {
    let theme: ThemeId = theme.into();
    let base = theme.base();
    let p = Palette::of_theme(theme);
    let mut v = match base {
        ThemeKind::Dark => Visuals::dark(),
        ThemeKind::Light => Visuals::light(),
    };
    let r = |name: &str| radius(tokens.length(name).unwrap_or(4.0));
    let shadow = |name: &str| tokens.shadow(name, base).unwrap_or_default();
    let hairline = |c: Color32| Stroke::new(1.0, c);

    v.override_text_color = None;
    v.weak_text_color = Some(p.ink_muted);
    v.hyperlink_color = p.accent_ink;
    v.faint_bg_color = p.surface_2;
    // Fundo de campos de texto e áreas de código: `surface-2`.
    v.extreme_bg_color = p.surface_2;
    v.text_edit_bg_color = None;
    v.code_bg_color = p.surface_2;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;

    v.panel_fill = p.surface_1;
    v.window_fill = p.surface_1;
    v.window_stroke = hairline(p.border);
    v.window_corner_radius = r("radius-xl");
    v.window_shadow = shadow("shadow-floating");
    v.menu_corner_radius = r("radius-l");
    v.popup_shadow = shadow("shadow-flyout");

    // Seleção e estado ligado em superfícies: `accent-subtle` com texto `accent-ink`.
    // Slider e Switch usam `accent` em escopo próprio (ver `widgets`).
    v.selection.bg_fill = p.accent_subtle;
    v.selection.stroke = hairline(p.accent_ink);
    v.text_cursor.stroke = Stroke::new(2.0, p.ink);
    v.slider_trailing_fill = true;
    v.striped = false;

    let corner = r("radius-m");
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface_1;
    w.noninteractive.weak_bg_fill = p.surface_1;
    w.noninteractive.bg_stroke = hairline(p.border);
    w.noninteractive.fg_stroke = hairline(p.ink);
    w.noninteractive.corner_radius = corner;

    w.inactive.bg_fill = p.surface_2;
    w.inactive.weak_bg_fill = p.surface_2;
    w.inactive.bg_stroke = hairline(p.border_strong);
    w.inactive.fg_stroke = hairline(p.ink);
    w.inactive.corner_radius = corner;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.surface_3;
    w.hovered.weak_bg_fill = p.surface_3;
    w.hovered.bg_stroke = hairline(p.border_strong);
    w.hovered.fg_stroke = hairline(p.ink);
    w.hovered.corner_radius = corner;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.surface_4;
    w.active.weak_bg_fill = p.surface_4;
    w.active.bg_stroke = hairline(p.border_strong);
    w.active.fg_stroke = hairline(p.ink);
    w.active.corner_radius = corner;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.surface_3;
    w.open.weak_bg_fill = p.surface_3;
    w.open.bg_stroke = hairline(p.border_strong);
    w.open.fg_stroke = hairline(p.ink);
    w.open.corner_radius = corner;

    // Desabilitado: o egui mistura o texto com o fundo por este alfa; `ink-disabled` é isento
    // de contraste e nunca o único sinal.
    v.disabled_alpha = 0.5;
    v
}

/// Nome do estilo `ribbon-label` (12/14, botões grandes do Ribbon clássico).
pub const RIBBON_LABEL: &str = "ribbon-label";
/// Nome do estilo `body-large` (16/22, campo de busca da Paleta de Ações).
pub const BODY_LARGE: &str = "body-large";

/// Texto num estilo do design system, com o espaçamento entre letras do token (`title` −0,01em,
/// `display` −0,02em; os demais 0). O `TextStyle` do egui só leva a fonte; o espaçamento vai
/// no `RichText` (`extra_letter_spacing`, em pontos).
#[allow(dead_code)] // API para os componentes que usam title/display (bastidores, fim de Partida).
pub fn styled_text(tokens: &Tokens, name: &str, text: impl Into<String>) -> egui::RichText {
    let token = tokens.text(name);
    let (size, spacing) = token.map_or((14.0, 0.0), |t| (t.size, t.size * t.letter_spacing_em));
    egui::RichText::new(text)
        .size(size)
        .extra_letter_spacing(spacing)
}

/// Estilos de texto da escala tipográfica (`caption`, `body`, `subtitle`, `title`, `log`).
pub fn text_styles(tokens: &Tokens) -> std::collections::BTreeMap<TextStyle, FontId> {
    let size = |name: &str, fallback: f32| tokens.text(name).map_or(fallback, |t| t.size);
    let sans = |s| FontId::new(s, FontFamily::Proportional);
    [
        (TextStyle::Small, sans(size("caption", 12.0))),
        (TextStyle::Body, sans(size("body", 14.0))),
        (TextStyle::Button, sans(size("body", 14.0))),
        (TextStyle::Heading, sans(size("title", 20.0))),
        // Estilos só de alguns componentes: rótulo dos botões grandes do Ribbon (12/14) e campo
        // da Paleta de Ações (16/22). Usados com `TextStyle::Name`.
        (
            TextStyle::Name(RIBBON_LABEL.into()),
            sans(size(RIBBON_LABEL, 12.0)),
        ),
        (
            TextStyle::Name(BODY_LARGE.into()),
            sans(size(BODY_LARGE, 16.0)),
        ),
        (
            TextStyle::Monospace,
            FontId::new(size("log", 12.0), FontFamily::Monospace),
        ),
    ]
    .into_iter()
    .collect()
}

/// `Style` completo de um Tema e uma Densidade.
pub fn build_style(theme: impl Into<ThemeId>, density: Density, tokens: &Tokens) -> Style {
    let m = DensityMetrics::of(density, tokens);
    let mut style = Style {
        visuals: build_visuals(theme, tokens),
        text_styles: text_styles(tokens),
        ..Style::default()
    };
    let sp = &mut style.spacing;
    sp.item_spacing = m.item_spacing;
    sp.button_padding = m.button_padding;
    sp.interact_size = vec2(m.control_height + 8.0, m.control_height);
    sp.window_margin = Margin::same(m.panel_padding.round() as i8);
    sp.menu_margin = Margin::same((m.panel_padding / 4.0).round() as i8);
    sp.icon_width = tokens.length("icon-s").unwrap_or(16.0);
    sp.icon_width_inner = sp.icon_width - 6.0;
    sp.combo_height = 320.0;
    sp.tooltip_width = 320.0;
    style.interaction.tooltip_delay = 0.5;
    style.animation_time = 0.12;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> &'static Tokens {
        Tokens::embedded()
    }

    #[test]
    fn visuals_take_surfaces_borders_and_selection_from_the_tokens() {
        for theme in ThemeKind::ALL {
            let p = Palette::of(theme);
            let v = build_visuals(theme, tokens());
            assert_eq!(v.dark_mode, theme == ThemeKind::Dark);
            assert_eq!(v.panel_fill, p.surface_1);
            assert_eq!(v.window_fill, p.surface_1);
            assert_eq!(v.extreme_bg_color, p.surface_2);
            assert_eq!(v.widgets.inactive.weak_bg_fill, p.surface_2);
            assert_eq!(v.widgets.inactive.bg_stroke.color, p.border_strong);
            assert_eq!(v.widgets.hovered.weak_bg_fill, p.surface_3);
            assert_eq!(v.widgets.active.weak_bg_fill, p.surface_4);
            assert_eq!(v.widgets.noninteractive.bg_stroke.color, p.border);
            assert_eq!(v.widgets.inactive.fg_stroke.color, p.ink);
            assert_eq!(v.selection.bg_fill, p.accent_subtle);
            assert_eq!(v.selection.stroke.color, p.accent_ink);
            assert_eq!(v.weak_text_color, Some(p.ink_muted));
        }
    }

    #[test]
    fn dark_and_light_differ_in_the_whole_visuals() {
        let dark = build_visuals(ThemeKind::Dark, tokens());
        let light = build_visuals(ThemeKind::Light, tokens());
        assert_ne!(dark.panel_fill, light.panel_fill);
        assert_ne!(
            dark.widgets.inactive.fg_stroke,
            light.widgets.inactive.fg_stroke
        );
    }

    #[test]
    fn radii_and_shadows_follow_the_tokens() {
        let v = build_visuals(ThemeKind::Dark, tokens());
        assert_eq!(v.widgets.inactive.corner_radius, CornerRadius::same(4));
        assert_eq!(v.menu_corner_radius, CornerRadius::same(6));
        assert_eq!(v.window_corner_radius, CornerRadius::same(8));
        assert_eq!(v.popup_shadow.offset, [0, 4]);
        assert_eq!(v.popup_shadow.blur, 12);
        assert_eq!(v.window_shadow.offset, [0, 8]);
    }

    #[test]
    fn density_sets_control_height_and_panel_padding() {
        let c = DensityMetrics::of(Density::Comfortable, tokens());
        let k = DensityMetrics::of(Density::Compact, tokens());
        assert_eq!((c.control_height, c.panel_padding), (32.0, 16.0));
        assert_eq!((k.control_height, k.panel_padding), (24.0, 12.0));
        assert!(k.item_spacing.y < c.item_spacing.y);
        assert!(k.button_padding.y < c.button_padding.y);

        let sc = build_style(ThemeKind::Dark, Density::Comfortable, tokens());
        let sk = build_style(ThemeKind::Dark, Density::Compact, tokens());
        assert_eq!(sc.spacing.interact_size.y, 32.0);
        assert_eq!(sk.spacing.interact_size.y, 24.0);
        assert_eq!(sc.spacing.window_margin, Margin::same(16));
        assert_eq!(sk.spacing.window_margin, Margin::same(12));
    }

    #[test]
    fn density_does_not_change_the_colors() {
        let a = build_style(ThemeKind::Light, Density::Comfortable, tokens());
        let b = build_style(ThemeKind::Light, Density::Compact, tokens());
        assert_eq!(a.visuals.panel_fill, b.visuals.panel_fill);
        assert_eq!(a.visuals.selection.bg_fill, b.visuals.selection.bg_fill);
    }

    #[test]
    fn text_scale_comes_from_the_typography_tokens() {
        let s = build_style(ThemeKind::Dark, Density::Comfortable, tokens());
        assert_eq!(s.text_styles[&TextStyle::Body].size, 14.0);
        assert_eq!(s.text_styles[&TextStyle::Small].size, 12.0);
        assert_eq!(s.text_styles[&TextStyle::Heading].size, 20.0);
        assert_eq!(
            s.text_styles[&TextStyle::Monospace].family,
            FontFamily::Monospace
        );
    }
}
