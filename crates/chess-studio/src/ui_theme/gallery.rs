//! Galeria do design system: `chess-studio --gallery` abre o app com uma janela que mostra todos
//! os widgets nos dois Temas e nas duas Densidades, lado a lado. Também permite trocar o Tema, a
//! Densidade e a Escala do app (preferências do Perfil local).

use super::appearance::{Appearance, Density, ThemeMode, UiScale};
use super::style::build_style;
use super::tokens::{Palette, ThemeKind, Tokens};
use super::widgets::*;
use crate::i18n::I18n;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use egui_phosphor::regular as icon;
use std::sync::Arc;

/// Quais Densidades a galeria mostra.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Show {
    #[default]
    Both,
    One(Density),
}

/// Presença deste recurso liga a galeria (`--gallery`).
#[derive(Resource, Debug, Default)]
pub struct GalleryMode {
    show: Show,
    demo: Demo,
}

/// Estado dos widgets de demonstração (compartilhado entre os quatro painéis).
#[derive(Debug)]
struct Demo {
    view: u8,
    coordinates: bool,
    highlight: bool,
    ghosts: bool,
    hud: bool,
    policy: u8,
    quality: u8,
    name: String,
    seconds: f32,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            view: 0,
            coordinates: true,
            highlight: false,
            ghosts: true,
            hud: false,
            policy: 0,
            quality: 1,
            name: String::new(),
            seconds: 5.0,
        }
    }
}

impl Demo {
    fn all_aids(&self) -> CheckState {
        match (self.ghosts, self.hud) {
            (true, true) => CheckState::On,
            (false, false) => CheckState::Off,
            _ => CheckState::Mixed,
        }
    }
}

pub fn gallery_window(
    mut contexts: EguiContexts,
    gallery: Option<ResMut<GalleryMode>>,
    mut appearance: ResMut<Appearance>,
    i18n: Res<I18n>,
) -> Result {
    let Some(mut gallery) = gallery else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let area = ctx.content_rect().shrink(16.0);
    egui::Window::new(i18n.t("gallery-title"))
        .default_pos(area.min)
        .default_size(area.size())
        .show(ctx, |ui| {
            controls(ui, &mut gallery, &mut appearance, &i18n);
            ui.separator();
            egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                panels(ui, &mut gallery, &i18n);
            });
        });
    Ok(())
}

fn controls(
    ui: &mut egui::Ui,
    gallery: &mut GalleryMode,
    appearance: &mut Appearance,
    i18n: &I18n,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(i18n.t("gallery-theme"));
        let system = i18n.t(ThemeMode::SYSTEM_LABEL_KEY);
        let options: Vec<_> = ThemeMode::all()
            .into_iter()
            .map(|m| (m, m.label(&system)))
            .collect();
        cs_dropdown(ui, "gallery-theme", &mut appearance.theme, &options);
        ui.add_space(12.0);
        ui.label(i18n.t("gallery-density"));
        for d in Density::ALL {
            cs_radio_value(ui, &mut appearance.density, d, &i18n.t(d.key()));
        }
        ui.add_space(12.0);
        let mut percent = f32::from(appearance.scale.percent());
        ui.label(i18n.t_with(
            "gallery-scale",
            &[("percent", &appearance.scale.percent().to_string())],
        ));
        if cs_slider(
            ui,
            &mut percent,
            f32::from(UiScale::MIN_PERCENT)..=f32::from(UiScale::MAX_PERCENT),
            5.0,
        )
        .changed()
        {
            appearance.scale = UiScale::from_percent(percent.round() as i64);
        }
        ui.add_space(12.0);
        ui.label(i18n.t("gallery-show"));
        cs_radio_value(
            ui,
            &mut gallery.show,
            Show::Both,
            &i18n.t("gallery-show-both"),
        );
        for d in Density::ALL {
            cs_radio_value(ui, &mut gallery.show, Show::One(d), &i18n.t(d.key()));
        }
    });
}

fn panels(ui: &mut egui::Ui, gallery: &mut GalleryMode, i18n: &I18n) {
    let densities: &[Density] = match gallery.show {
        Show::Both => &Density::ALL,
        Show::One(Density::Comfortable) => &[Density::Comfortable],
        Show::One(Density::Compact) => &[Density::Compact],
    };
    for &density in densities {
        ui.columns(2, |cols| {
            for (col, theme) in cols.iter_mut().zip(ThemeKind::ALL) {
                panel(col, theme, density, &mut gallery.demo, i18n);
            }
        });
        ui.add_space(8.0);
    }
}

/// Um painel: os widgets no Tema e na Densidade dados, independentes do Tema do app.
fn panel(ui: &mut egui::Ui, theme: ThemeKind, density: Density, demo: &mut Demo, i18n: &I18n) {
    let tokens = Tokens::embedded();
    let style = Arc::new(build_style(theme, density, tokens));
    let p = Palette::of(theme);
    ui.push_id((theme, density), |ui| {
        ui.set_style(style.clone());
        egui::Frame::new()
            .fill(p.surface_1)
            .stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(8)
            .inner_margin(style.spacing.window_margin)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let title = i18n.t_with(
                    "gallery-panel-title",
                    &[
                        (
                            "theme",
                            &i18n.t(match theme {
                                ThemeKind::Dark => "theme-mode-dark",
                                ThemeKind::Light => "theme-mode-light",
                            }),
                        ),
                        ("density", &i18n.t(density.key())),
                    ],
                );
                ui.label(egui::RichText::new(title).size(16.0).color(p.ink));
                widgets_demo(ui, p, demo, i18n);
            });
    });
}

fn section(ui: &mut egui::Ui, p: &Palette, text: String) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).small().color(p.ink_muted));
}

fn widgets_demo(ui: &mut egui::Ui, p: &Palette, demo: &mut Demo, i18n: &I18n) {
    section(ui, p, i18n.t("gallery-section-buttons"));
    ui.horizontal_wrapped(|ui| {
        ui.add(
            CsButton::new(&i18n.t("gallery-button-primary"))
                .icon(icon::PLAY)
                .variant(ButtonVariant::Primary),
        );
        cs_button(
            ui,
            &i18n.t("gallery-button-secondary"),
            ButtonVariant::Secondary,
        );
        ui.add(
            CsButton::new(&i18n.t("gallery-button-subtle"))
                .icon(icon::FOLDER_OPEN)
                .variant(ButtonVariant::Subtle),
        );
        ui.add(
            CsButton::new(&i18n.t("gallery-button-danger"))
                .icon(icon::FLAG)
                .variant(ButtonVariant::Danger),
        );
        ui.add(
            CsButton::new(&i18n.t("gallery-button-disabled"))
                .icon(icon::ARROW_U_UP_LEFT)
                .enabled(false),
        );
        let only_icon = ui.add(
            CsButton::new("")
                .icon(icon::GEAR)
                .variant(ButtonVariant::Subtle),
        );
        action_tooltip(
            only_icon,
            &ActionTip::new(&i18n.t("gallery-button-icon-hint")).shortcut("Ctrl+,"),
        );
    });

    section(ui, p, i18n.t("gallery-section-toggles"));
    ui.horizontal(|ui| {
        for (i, key) in [
            "gallery-toggle-3d",
            "gallery-toggle-table",
            "gallery-toggle-2d",
        ]
        .into_iter()
        .enumerate()
        {
            let mut on = demo.view as usize == i;
            if cs_toggle_button(ui, &mut on, &i18n.t(key)).clicked() && on {
                demo.view = i as u8;
            }
        }
    });
    cs_switch(
        ui,
        &mut demo.coordinates,
        &i18n.t("gallery-switch-coordinates"),
    );
    cs_switch(ui, &mut demo.highlight, &i18n.t("gallery-switch-highlight"));

    section(ui, p, i18n.t("gallery-section-selection"));
    let mut all = demo.all_aids();
    if cs_checkbox_tri(ui, &mut all, &i18n.t("gallery-check-all")).changed() {
        let on = all == CheckState::On;
        demo.ghosts = on;
        demo.hud = on;
    }
    ui.indent("aids", |ui| {
        cs_checkbox(ui, &mut demo.ghosts, &i18n.t("gallery-check-ghosts"));
        cs_checkbox(ui, &mut demo.hud, &i18n.t("gallery-check-hud"));
    });
    ui.label(egui::RichText::new(i18n.t("gallery-radio-policy")).color(p.ink_muted));
    for (i, key) in [
        "gallery-radio-assisted",
        "gallery-radio-fide",
        "gallery-radio-training",
    ]
    .into_iter()
    .enumerate()
    {
        cs_radio_value(ui, &mut demo.policy, i as u8, &i18n.t(key));
    }

    section(ui, p, i18n.t("gallery-section-inputs"));
    ui.horizontal(|ui| {
        ui.label(i18n.t("gallery-dropdown-quality"));
        let options: Vec<_> = [
            (0u8, "gallery-quality-economic"),
            (1, "gallery-quality-balanced"),
            (2, "gallery-quality-ultra"),
        ]
        .into_iter()
        .map(|(v, k)| (v, i18n.t(k)))
        .collect();
        cs_dropdown(ui, "quality", &mut demo.quality, &options);
    });
    ui.label(i18n.t("gallery-field-name"));
    cs_text_field(ui, &mut demo.name, &i18n.t("gallery-field-hint"), None);
    let mut dup = String::from("Ana");
    cs_text_field(ui, &mut dup, "", Some(&i18n.t("gallery-field-error")));
    ui.label(i18n.t_with(
        "gallery-slider-label",
        &[("seconds", &format!("{:.0}", demo.seconds))],
    ));
    cs_slider(ui, &mut demo.seconds, 1.0..=30.0, 1.0);

    section(ui, p, i18n.t("gallery-section-tooltip"));
    ui.horizontal_top(|ui| {
        let hover = cs_button(
            ui,
            &i18n.t("gallery-tooltip-hover"),
            ButtonVariant::Secondary,
        );
        action_tooltip(
            hover,
            &ActionTip::new(&i18n.t("gallery-tooltip-title"))
                .description(&i18n.t("gallery-tooltip-description"))
                .shortcut(&i18n.t("gallery-tooltip-shortcut")),
        );
        // Prévia estática, como o tooltip aparece (Tooltip.md).
        for tip in [
            (
                i18n.t("gallery-tooltip-title"),
                Some(i18n.t("gallery-tooltip-description")),
                i18n.t("gallery-tooltip-shortcut"),
                None,
            ),
            (
                i18n.t("gallery-tooltip-disabled-title"),
                None,
                i18n.t("gallery-tooltip-disabled-shortcut"),
                Some(i18n.t("gallery-tooltip-disabled-reason")),
            ),
        ] {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let mut t = ActionTip::new(&tip.0).shortcut(&tip.2);
                if let Some(d) = tip.1.as_deref() {
                    t = t.description(d);
                }
                if let Some(r) = tip.3.as_deref() {
                    t = t.disabled(r);
                }
                action_tip_ui(ui, &t);
            });
        }
    });
}
