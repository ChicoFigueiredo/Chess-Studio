//! Os bastidores (`Backstage.md`): a janela inteira aberta pela aba Arquivo. Navegação à
//! esquerda em `accent`/`on-accent`; conteúdo em `surface-1`. "Voltar" e Esc fecham.
//!
//! Nada aqui altera a Partida (ADR-0001): os itens que são Ações só escrevem
//! `ActionRequested`; Perfis e Perfil local mostram as seções que já existem.

use super::RibbonState;
use super::icons::glyph;
use crate::actions::ActionRequested;
use crate::i18n::I18n;
use crate::ui_theme::{icon_job, palette, strong_font};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use chess_input::actions::{ActionContext, meta, state};
use chess_input::controls::Action;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Rect, Sense, vec2};

/// Página dos bastidores com conteúdo próprio (as demais entradas são Ações).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackstagePage {
    /// Perfis de interface.
    Profiles,
    /// Perfil local (quem está jogando).
    LocalProfile,
}

/// Uma entrada da navegação: abre uma página ou manda uma Ação (e fecha os bastidores).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Run(Action),
    Page(BackstagePage),
}

/// A ordem da navegação: Nova, Abrir PGN, Biblioteca, Perfis, Perfil local, Controles e Opções.
pub const ENTRIES: [(Entry, &str); 7] = [
    (Entry::Run(Action::NewGame), "file-plus"),
    // "Abrir PGN" abre o Painel Biblioteca, onde a importação vive.
    (Entry::Run(Action::PanelLibrary), "folder-open"),
    (Entry::Run(Action::PanelLibrary), "books"),
    (Entry::Page(BackstagePage::Profiles), "users"),
    (Entry::Page(BackstagePage::LocalProfile), "user"),
    (Entry::Run(Action::OpenControls), "keyboard"),
    (Entry::Run(Action::OpenOptions), "gear"),
];

/// Texto da entrada (id Fluent).
fn label_id(index: usize) -> &'static str {
    match (index, ENTRIES[index].0) {
        (1, _) => "backstage-open-pgn",
        (_, Entry::Run(action)) => meta(action).label,
        (_, Entry::Page(BackstagePage::Profiles)) => "backstage-profiles",
        (_, Entry::Page(BackstagePage::LocalProfile)) => "backstage-local-profile",
    }
}

/// Título da página aberta, no corpo dos bastidores.
fn page_title_id(page: BackstagePage) -> &'static str {
    match page {
        BackstagePage::Profiles => "backstage-profiles",
        BackstagePage::LocalProfile => "backstage-local-profile",
    }
}

/// Resultado de um quadro dos bastidores.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BackstageOutcome {
    pub run: Option<Action>,
    pub page: Option<BackstagePage>,
    pub close: bool,
}

/// Desabilitada por não existir ainda (#164) ou por regra: o item aparece apagado.
pub fn entry_enabled(entry: Entry, ctx: &ActionContext) -> bool {
    match entry {
        Entry::Run(action) => state(action, ctx).is_enabled(),
        Entry::Page(_) => true,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn backstage_ui(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    context: Res<super::RibbonActionContext>,
    mut ribbon: ResMut<RibbonState>,
    mut profiles: ResMut<crate::profiles::InterfaceProfiles>,
    mut local: ResMut<crate::local_profiles::LocalProfiles>,
    training: Res<crate::training_ui::TrainingUi>,
    mut requests: MessageWriter<ActionRequested>,
) -> Result {
    let Some(page) = ribbon.backstage else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let p = palette_for(&ctx);
    let screen = ctx.viewport_rect();
    let nav_w = 260.0_f32.min(screen.width() * 0.4);
    let nav = Rect::from_min_max(screen.min, egui::pos2(screen.min.x + nav_w, screen.max.y));
    let content = Rect::from_min_max(egui::pos2(nav.max.x, screen.min.y), screen.max);
    let mut outcome = BackstageOutcome::default();

    let area = egui::Area::new(Id::new("chess-studio/backstage"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(&ctx, |ui| {
            // Cobre a janela e engole o ponteiro: nada atrás reage.
            ui.allocate_rect(screen, Sense::click_and_drag());
            ui.painter()
                .rect_filled(screen, CornerRadius::ZERO, p.surface_1);
            ui.painter().rect_filled(nav, CornerRadius::ZERO, p.accent);

            // Navegação.
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(nav.shrink2(vec2(12.0, 16.0))),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                    let back = nav_item(
                        ui,
                        &ctx,
                        "arrow-left",
                        &i18n.t("backstage-back"),
                        false,
                        true,
                    );
                    if back.clicked() {
                        outcome.close = true;
                    }
                    ui.add_space(12.0);
                    for (index, (entry, icon)) in ENTRIES.iter().enumerate() {
                        let selected = matches!(entry, Entry::Page(pg) if *pg == page);
                        let enabled = entry_enabled(*entry, &context.0);
                        let response =
                            nav_item(ui, &ctx, icon, &i18n.t(label_id(index)), selected, enabled);
                        if enabled && response.clicked() {
                            match entry {
                                Entry::Run(action) => outcome.run = Some(*action),
                                Entry::Page(pg) => outcome.page = Some(*pg),
                            }
                        }
                        if let Entry::Run(action) = entry
                            && !enabled
                            && let Some(reason) = state(*action, &context.0).disabled_reason
                        {
                            response.on_hover_text(i18n.t(reason));
                        }
                    }
                },
            );

            // Conteúdo.
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(content.shrink2(vec2(40.0, 32.0))),
                |ui| {
                    ui.label(
                        egui::RichText::new(i18n.t(page_title_id(page)))
                            .font(strong_font(&ctx, 28.0))
                            .color(p.ink),
                    );
                    ui.add_space(16.0);
                    ui.set_max_width(520.0);
                    match page {
                        BackstagePage::Profiles => {
                            crate::profiles::profiles_section(ui, &i18n, &mut profiles);
                        }
                        BackstagePage::LocalProfile => {
                            crate::local_profiles::section(
                                ui,
                                &i18n,
                                &mut local,
                                training.session_swapped(),
                            );
                        }
                    }
                },
            );
        });
    ctx.move_to_top(area.response.layer_id);

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome.close = true;
    }
    if let Some(next) = outcome.page {
        ribbon.backstage = Some(next);
    }
    if let Some(action) = outcome.run {
        requests.write(ActionRequested(action));
        ribbon.backstage = None;
    }
    if outcome.close {
        ribbon.backstage = None;
    }
    Ok(())
}

fn palette_for(ctx: &egui::Context) -> &'static crate::ui_theme::Palette {
    use crate::ui_theme::{Palette, ThemeKind};
    Palette::of(ThemeKind::from_dark_mode(
        ctx.global_style().visuals.dark_mode,
    ))
}

/// Uma entrada da navegação, em `on-accent` sobre `accent`.
fn nav_item(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    icon: &str,
    text: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let p = palette(ui);
    let job = icon_job(
        ctx,
        glyph(icon),
        text,
        FontId::proportional(15.0),
        Color32::PLACEHOLDER,
    );
    let galley = ui.painter().layout_job(job);
    let width = ui.available_width();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 36.0), sense);
    let fill = if selected {
        p.accent_pressed
    } else if response.hovered() && enabled {
        p.accent_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    let color = if enabled {
        p.on_accent
    } else {
        p.on_accent.gamma_multiply(0.5)
    };
    let pos = Align2::LEFT_CENTER
        .align_size_within_rect(galley.size(), rect.shrink2(vec2(12.0, 0.0)))
        .min;
    ui.painter().galley(pos, galley, color);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_navigation_lists_the_agreed_entries_in_order() {
        let ids: Vec<_> = (0..ENTRIES.len()).map(label_id).collect();
        assert_eq!(
            &ids[..5],
            [
                "action-new-game",
                "backstage-open-pgn",
                "action-panel-library",
                "backstage-profiles",
                "backstage-local-profile",
            ]
        );
        assert_eq!(*ids.last().unwrap(), "action-open-options");
    }

    #[test]
    fn options_stay_disabled_until_the_dialog_exists_and_pages_are_always_open() {
        let ctx = ActionContext::default();
        assert!(entry_enabled(Entry::Run(Action::NewGame), &ctx));
        assert!(entry_enabled(Entry::Page(BackstagePage::Profiles), &ctx));
        // #164 entrega as Opções; enquanto não, a Ação traz o motivo.
        assert_eq!(
            entry_enabled(Entry::Run(Action::OpenOptions), &ctx),
            state(Action::OpenOptions, &ctx).is_enabled()
        );
    }

    #[test]
    fn every_backstage_text_exists_in_pt_br() {
        let i18n = I18n::pt_br();
        for index in 0..ENTRIES.len() {
            let id = label_id(index);
            assert_ne!(i18n.t(id), id, "{id}");
        }
        assert_ne!(i18n.t("backstage-back"), "backstage-back");
    }
}
