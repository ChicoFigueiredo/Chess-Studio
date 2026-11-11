//! Selo de estado (`StatusBadge`): ícone e palavra num chip, nunca só a cor (README do design
//! system, "Cor"). O tom escolhe a cor do ícone; o texto é sempre `ink`, e o contorno é o
//! `border-strong`, de modo que o selo vale sobre `surface-1` e `surface-2` (onde `success`,
//! `warning`, `danger` e `info` cumprem 4,5:1).

use super::{icon_font, palette, strong_font};
use crate::ui_theme::tokens::Palette;
use bevy_egui::egui::{
    self, Align2, Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, vec2,
};

/// O sentido do estado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    /// Sem sinal: desligado, em breve, requer aprovação.
    Neutral,
    /// Funcionando.
    Success,
    /// Informação.
    Info,
    /// Atenção: limitado, pausa.
    Warning,
    /// Falha.
    Danger,
    /// Marca de identidade: "Padrão".
    Accent,
}

impl StatusTone {
    #[cfg(test)]
    pub const ALL: [StatusTone; 6] = [
        Self::Neutral,
        Self::Success,
        Self::Info,
        Self::Warning,
        Self::Danger,
        Self::Accent,
    ];
}

/// Cores do selo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeLook {
    pub fill: Color32,
    pub stroke: Stroke,
    pub icon: Color32,
    pub text: Color32,
}

pub fn badge_look(p: &Palette, tone: StatusTone) -> BadgeLook {
    let outlined = |icon| BadgeLook {
        fill: Color32::TRANSPARENT,
        stroke: Stroke::new(1.0, p.border_strong),
        icon,
        text: p.ink,
    };
    match tone {
        StatusTone::Neutral => outlined(p.ink_muted),
        StatusTone::Success => outlined(p.success),
        StatusTone::Info => outlined(p.info),
        StatusTone::Warning => outlined(p.warning),
        StatusTone::Danger => outlined(p.danger),
        StatusTone::Accent => BadgeLook {
            fill: p.accent_subtle,
            stroke: Stroke::NONE,
            icon: p.accent_ink,
            text: p.accent_ink,
        },
    }
}

/// Altura do selo: `control-h-compact`.
pub const BADGE_HEIGHT: f32 = 24.0;

/// Selo com ícone Phosphor e palavra. `icon` pode ser vazio (só texto, como o "Padrão").
pub fn cs_status_badge(ui: &mut Ui, tone: StatusTone, icon: &str, label: &str) -> Response {
    let p = palette(ui);
    let look = badge_look(p, tone);
    let ctx = ui.ctx().clone();
    let mut job = egui::text::LayoutJob::default();
    let valign = egui::Align::Center;
    if !icon.is_empty() {
        job.append(
            icon,
            0.0,
            egui::TextFormat {
                font_id: icon_font(&ctx, 14.0),
                color: look.icon,
                valign,
                ..Default::default()
            },
        );
    }
    job.append(
        label,
        if icon.is_empty() { 0.0 } else { 6.0 },
        egui::TextFormat {
            font_id: strong_font(&ctx, 12.0),
            color: look.text,
            valign,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);
    let size = vec2(galley.size().x + 16.0, BADGE_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.to_owned()));
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same((BADGE_HEIGHT / 2.0) as u8);
        ui.painter()
            .rect(rect, corner, look.fill, look.stroke, StrokeKind::Inside);
        let pos = Align2::CENTER_CENTER
            .align_size_within_rect(galley.size(), rect)
            .min;
        ui.painter().galley(pos, galley, look.text);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::tokens::contrast_ratio;
    use crate::ui_theme::{ThemeKind, all_themes};

    #[test]
    fn every_tone_is_readable_on_the_surfaces_a_card_sits_on_in_all_22_themes() {
        for theme in all_themes() {
            let p = Palette::of_theme(theme);
            for tone in StatusTone::ALL {
                let look = badge_look(p, tone);
                let surfaces: &[Color32] = if tone == StatusTone::Accent {
                    &[p.accent_subtle]
                } else {
                    &[p.surface_1, p.surface_2]
                };
                for &surface in surfaces {
                    let text = contrast_ratio(look.text, surface);
                    assert!(
                        text >= 4.5,
                        "{} {tone:?}: texto {text:.2}:1",
                        theme.info().name
                    );
                    let glyph = contrast_ratio(look.icon, surface);
                    assert!(
                        glyph >= 4.5,
                        "{} {tone:?}: ícone {glyph:.2}:1",
                        theme.info().name
                    );
                }
            }
        }
    }

    #[test]
    fn the_tones_differ_so_the_badge_never_depends_on_one_colour() {
        let p = Palette::of(ThemeKind::Dark);
        let icons: Vec<Color32> = StatusTone::ALL
            .iter()
            .map(|&tone| badge_look(p, tone).icon)
            .collect();
        for (i, a) in icons.iter().enumerate() {
            for b in &icons[i + 1..] {
                assert_ne!(a, b);
            }
        }
        // Todo selo tem contorno ou fundo: nunca é só texto solto.
        for tone in StatusTone::ALL {
            let look = badge_look(p, tone);
            assert!(look.stroke.width > 0.0 || look.fill != Color32::TRANSPARENT);
        }
    }

    #[test]
    fn the_badge_has_the_compact_height_and_grows_with_its_label() {
        let ctx = crate::ui_theme::test_context(
            crate::ui_theme::ThemeId::factory(crate::ui_theme::ThemeKind::Dark),
            crate::ui_theme::Density::Comfortable,
        );
        let (mut short, mut long) = (None, None);
        crate::ui_theme::run_quiet(&ctx, egui::RawInput::default(), |ui| {
            short = Some(cs_status_badge(ui, StatusTone::Accent, "", "Padrão").rect);
            long = Some(
                cs_status_badge(
                    ui,
                    StatusTone::Success,
                    egui_phosphor::regular::LINK_SIMPLE,
                    "Ligado · nome vinculado",
                )
                .rect,
            );
        });
        let (short, long) = (short.unwrap(), long.unwrap());
        assert_eq!(short.height(), BADGE_HEIGHT);
        assert_eq!(long.height(), BADGE_HEIGHT);
        assert!(long.width() > short.width() + 40.0);
    }
}
