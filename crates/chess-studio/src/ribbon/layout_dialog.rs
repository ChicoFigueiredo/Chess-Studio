//! Diálogo "Salvar layout…" (#196): nome do novo Layout de Tela (com aviso se já existe) e a
//! lista dos salvos, com renomear e excluir. É um `egui::Modal` do design system (botões,
//! campo de texto e cores de `ui_theme`); a validação vive em [`ScreenLayouts`] e o diálogo só
//! a mostra. Pedidos saem como [`ScreenLayoutRequested`].

use crate::i18n::I18n;
use crate::screen_layouts::{SaveCheck, ScreenLayoutRequested, ScreenLayouts};
use crate::ui_theme::{
    ButtonVariant, CsButton, Palette, ThemeKind, Tokens, cs_button, cs_text_field, icon_job,
};
use bevy_egui::egui::{self, Id};
use egui_phosphor::regular as icon;

const WIDTH: f32 = 420.0;

fn open_id() -> Id {
    Id::new("chess-studio/screen-layouts/dialog-open")
}

fn state_id() -> Id {
    Id::new("chess-studio/screen-layouts/dialog-state")
}

/// Texto digitado: o nome novo e o renomear em andamento (nome atual, nome novo).
#[derive(Clone, Default)]
struct DialogState {
    name: String,
    renaming: Option<(String, String)>,
    /// Layout cuja exclusão espera confirmação (ação destrutiva).
    confirming: Option<String>,
}

/// Abre o diálogo (botão "Salvar layout…").
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(open_id(), true));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false)
}

fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.insert_temp(open_id(), false);
        d.remove::<DialogState>(state_id());
    });
}

/// Evidência (`--screenshot <dir> --layout-dialog <nome>`): abre o diálogo com o nome digitado.
fn capture_open(ctx: &egui::Context) {
    let once = Id::new("chess-studio/screen-layouts/dialog-capture");
    if ctx.data(|d| d.get_temp::<bool>(once)).unwrap_or(false) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(once, true));
    if let Some(name) = crate::capture::option_value("--layout-dialog") {
        open(ctx);
        ctx.data_mut(|d| {
            d.insert_temp(
                state_id(),
                DialogState {
                    name,
                    ..DialogState::default()
                },
            )
        });
    }
}

/// Desenha o diálogo se estiver aberto e devolve os pedidos feitos neste quadro.
pub fn show(
    ctx: &egui::Context,
    i18n: &I18n,
    layouts: &ScreenLayouts,
    out: &mut Vec<ScreenLayoutRequested>,
) {
    capture_open(ctx);
    if !is_open(ctx) {
        return;
    }
    let theme = ThemeKind::from_dark_mode(ctx.global_style().visuals.dark_mode);
    let p = Palette::from_visuals(&ctx.global_style().visuals);
    let tokens = Tokens::embedded();
    let frame = egui::Frame::new()
        .fill(p.surface_1)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(tokens.length("radius-xl").unwrap_or(8.0))
        .shadow(tokens.shadow("shadow-dialog", theme).unwrap_or_default())
        .inner_margin(egui::Margin::same(24));
    let mut state: DialogState = ctx.data(|d| d.get_temp(state_id())).unwrap_or_default();
    let mut close_now = false;
    let response = egui::Modal::new(Id::new("screen-layouts-dialog"))
        .backdrop_color(p.scrim)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            let title = icon_job(
                ui.ctx(),
                Some(icon::FLOPPY_DISK),
                &i18n.t("screen-layout-dialog-title"),
                crate::ui_theme::strong_font(ui.ctx(), 20.0),
                p.ink,
            );
            ui.label(title);
            save_section(ui, p, i18n, layouts, &mut state, out);
            ui.separator();
            saved_section(ui, p, i18n, layouts, &mut state, out);
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if cs_button(
                    ui,
                    &i18n.t("screen-layout-dialog-close"),
                    ButtonVariant::Secondary,
                )
                .clicked()
                {
                    close_now = true;
                }
            });
        });
    if close_now || response.should_close() {
        close(ctx);
    } else {
        ctx.data_mut(|d| d.insert_temp(state_id(), state));
    }
}

fn error_text(i18n: &I18n, check: SaveCheck) -> Option<String> {
    match check {
        SaveCheck::Invalid(e) => Some(i18n.t(e.key())),
        _ => None,
    }
}

fn save_section(
    ui: &mut egui::Ui,
    p: &Palette,
    i18n: &I18n,
    layouts: &ScreenLayouts,
    state: &mut DialogState,
    out: &mut Vec<ScreenLayoutRequested>,
) {
    ui.label(egui::RichText::new(i18n.t("screen-layout-dialog-name-label")).color(p.ink_muted));
    let check = layouts.check_save(&state.name);
    // Campo vazio ainda não é erro: só some o botão. Erro vale depois de digitar.
    let shown_error = if state.name.trim().is_empty() {
        None
    } else {
        error_text(i18n, check)
    };
    let field = cs_text_field(
        ui,
        &mut state.name,
        &i18n.t("screen-layout-save-hint"),
        shown_error.as_deref(),
    );
    if check == SaveCheck::Replaces {
        ui.label(icon_job(
            ui.ctx(),
            Some(icon::WARNING),
            &i18n.t("screen-layout-dialog-replace-warning"),
            egui::TextStyle::Body.resolve(ui.style()),
            p.warning,
        ));
    }
    if let Some(error) = layouts.error {
        ui.label(icon_job(
            ui.ctx(),
            Some(icon::WARNING_CIRCLE),
            &i18n.t(error.key()),
            egui::TextStyle::Body.resolve(ui.style()),
            p.danger,
        ));
    }
    let valid = matches!(check, SaveCheck::New | SaveCheck::Replaces);
    let label = if check == SaveCheck::Replaces {
        i18n.t("screen-layout-dialog-replace")
    } else {
        i18n.t("screen-layout-dialog-save")
    };
    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let clicked = ui
        .add(
            CsButton::new(&label)
                .variant(ButtonVariant::Primary)
                .enabled(valid),
        )
        .clicked();
    if valid && (clicked || enter) {
        out.push(ScreenLayoutRequested::SaveCurrent(state.name.clone()));
        state.name.clear();
    }
}

fn saved_section(
    ui: &mut egui::Ui,
    p: &Palette,
    i18n: &I18n,
    layouts: &ScreenLayouts,
    state: &mut DialogState,
    out: &mut Vec<ScreenLayoutRequested>,
) {
    ui.label(egui::RichText::new(i18n.t("screen-layout-dialog-saved-heading")).color(p.ink_muted));
    if layouts.saved().is_empty() {
        ui.label(egui::RichText::new(i18n.t("screen-layout-none-saved")).color(p.ink_muted));
        return;
    }
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for saved in layouts.saved() {
                match state.renaming.clone() {
                    Some((from, mut text)) if from == saved.name => {
                        let check = layouts.check_rename(&from, &text);
                        let error = check.err().map(|e| i18n.t(e.key()));
                        let mut done = false;
                        ui.horizontal(|ui| {
                            let field = cs_text_field(ui, &mut text, "", error.as_deref());
                            let enter =
                                field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let ok = ui
                                .add(
                                    CsButton::new(&i18n.t("screen-layout-rename-confirm"))
                                        .variant(ButtonVariant::Primary)
                                        .enabled(check.is_ok()),
                                )
                                .clicked();
                            if check.is_ok() && (ok || enter) {
                                out.push(ScreenLayoutRequested::Rename {
                                    from: from.clone(),
                                    to: text.clone(),
                                });
                                state.renaming = None;
                                done = true;
                            }
                            if cs_button(
                                ui,
                                &i18n.t("screen-layout-rename-cancel"),
                                ButtonVariant::Secondary,
                            )
                            .clicked()
                            {
                                state.renaming = None;
                                done = true;
                            }
                        });
                        if !done {
                            state.renaming = Some((from, text));
                        }
                    }
                    _ if state.confirming.as_deref() == Some(saved.name.as_str()) => {
                        ui.horizontal(|ui| {
                            ui.label(&saved.name);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if cs_button(
                                        ui,
                                        &i18n.t("screen-layout-delete-confirm"),
                                        ButtonVariant::Danger,
                                    )
                                    .clicked()
                                    {
                                        out.push(ScreenLayoutRequested::Delete(saved.name.clone()));
                                        state.confirming = None;
                                    }
                                    if cs_button(
                                        ui,
                                        &i18n.t("screen-layout-rename-cancel"),
                                        ButtonVariant::Secondary,
                                    )
                                    .clicked()
                                    {
                                        state.confirming = None;
                                    }
                                },
                            );
                        });
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.label(&saved.name);
                            ui.label(
                                egui::RichText::new(i18n.t_with(
                                    "screen-layout-panels-count",
                                    &[("count", &saved.panels.len().to_string())],
                                ))
                                .small()
                                .color(p.ink_muted),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if cs_button(
                                        ui,
                                        &i18n.t("screen-layout-delete"),
                                        ButtonVariant::Danger,
                                    )
                                    .clicked()
                                    {
                                        state.confirming = Some(saved.name.clone());
                                    }
                                    if cs_button(
                                        ui,
                                        &i18n.t("screen-layout-rename"),
                                        ButtonVariant::Secondary,
                                    )
                                    .clicked()
                                    {
                                        state.renaming =
                                            Some((saved.name.clone(), saved.name.clone()));
                                    }
                                },
                            );
                        });
                    }
                }
            }
        });
}
