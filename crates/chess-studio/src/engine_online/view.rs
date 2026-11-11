//! A tela da Engine online (#252): ligar uma Engine local para responder desafios no Lichess,
//! com estado visível, limites do usuário e botão de parar. Só desenha e junta intenções
//! ([`Intent`]); quem age é o [`super::service::EngineOnline`].

use super::service::{
    Activity, Conversion, EngineSpec, GameRow, RunState, Settings, StopReason, Unavailable,
};
use crate::i18n::I18n;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, StatusTone, cs_card_frame, cs_checkbox, cs_dropdown,
    cs_segmented, cs_status_badge, palette, strong_font,
};
use bevy_egui::egui;
use chess_providers::{ChallengeSpeed, DeclineReason, DrawResponse, OpponentPolicy, RatedPolicy};
use egui_phosphor::regular as icon;
use std::path::PathBuf;

/// Largura do corpo da janela.
pub const WINDOW_W: f32 = 540.0;

/// O resultado da conversão em BOT, para a tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionView {
    Sending,
    Done,
    Failed,
}

impl ConversionView {
    pub fn of(conversion: Option<&Conversion>) -> Option<Self> {
        conversion.map(|c| match c {
            Conversion::Sending(_) => Self::Sending,
            Conversion::Done => Self::Done,
            Conversion::Failed => Self::Failed,
        })
    }
}

/// O que a tela precisa saber, já copiado do serviço e da Central.
#[derive(Debug, Clone)]
pub struct ViewModel {
    /// Há Conexão de Engine autorizada.
    pub connected: bool,
    /// O Lichess está ligado.
    pub provider_on: bool,
    pub engines: Vec<EngineSpec>,
    pub state: RunState,
    pub stop_reason: Option<StopReason>,
    pub running_engine: Option<String>,
    pub rows: Vec<GameRow>,
    pub activity: Vec<Activity>,
    /// Aceitos, recusados e terminados nesta execução.
    pub counts: (u32, u32, u32),
    pub conversion: Option<ConversionView>,
}

/// O formulário: o que o usuário escolheu (vale só até ligar).
#[derive(Debug, Clone, Default)]
pub struct Form {
    pub engine: Option<PathBuf>,
    pub settings: Settings,
    /// Confirmação de "Parar já" aberta.
    pub confirm_stop_now: bool,
    /// Área da conversão em BOT aberta, e a confirmação marcada.
    pub upgrade_open: bool,
    pub upgrade_understood: bool,
}

/// O que a tela pediu neste quadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Start,
    Stop,
    StopNow,
    Upgrade,
    OpenCenter,
    Close,
}

/// Ritmos oferecidos (a Engine online não joga ultrabullet nem correspondência).
const SPEEDS: [(ChallengeSpeed, &str); 4] = [
    (ChallengeSpeed::Bullet, "engine-online-speed-bullet"),
    (ChallengeSpeed::Blitz, "engine-online-speed-blitz"),
    (ChallengeSpeed::Rapid, "engine-online-speed-rapid"),
    (ChallengeSpeed::Classical, "engine-online-speed-classical"),
];

/// Por que o Lichess recusou, em linguagem de usuário.
pub fn decline_key(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::Later => "engine-online-decline-later",
        DeclineReason::TimeControl | DeclineReason::TooFast | DeclineReason::TooSlow => {
            "engine-online-decline-pace"
        }
        DeclineReason::Rated => "engine-online-decline-rated",
        DeclineReason::Casual => "engine-online-decline-casual",
        DeclineReason::Standard | DeclineReason::Variant => "engine-online-decline-variant",
        DeclineReason::NoBot => "engine-online-decline-nobot",
        DeclineReason::OnlyBot => "engine-online-decline-onlybot",
        DeclineReason::Generic => "engine-online-decline-generic",
    }
}

/// Um problema do Lichess em linguagem de usuário.
fn problem_text(i18n: &I18n, error: &chess_providers::ProviderError) -> String {
    crate::online::provider_error_text(i18n, error)
}

fn state_badge(ui: &mut egui::Ui, i18n: &I18n, model: &ViewModel) {
    let (tone, glyph, key) = match model.state {
        RunState::Stopped => (
            StatusTone::Neutral,
            icon::STOP_CIRCLE,
            "engine-online-state-stopped",
        ),
        RunState::Waiting => (
            StatusTone::Success,
            icon::HOURGLASS_MEDIUM,
            "engine-online-state-waiting",
        ),
        RunState::Playing => (
            StatusTone::Info,
            icon::PLAY_CIRCLE,
            "engine-online-state-playing",
        ),
        RunState::Stopping => (
            StatusTone::Warning,
            icon::HOURGLASS_HIGH,
            "engine-online-state-stopping",
        ),
    };
    let label = if model.state == RunState::Playing {
        i18n.t_with(key, &[("count", &model.rows.len().max(1).to_string())])
    } else {
        i18n.t(key)
    };
    cs_status_badge(ui, tone, glyph, &label);
}

/// O corpo da janela. Desenha e devolve as intenções; o formulário é editado no lugar.
pub fn body(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    form: &mut Form,
    model: &ViewModel,
    error: Option<&Unavailable>,
    intents: &mut Vec<Intent>,
) {
    let p = palette(ui);
    ui.set_width(WINDOW_W);
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.label(egui::RichText::new(i18n.t("engine-online-intro")).color(p.ink_muted));

    // Estado e contagens.
    ui.horizontal(|ui| {
        state_badge(ui, i18n, model);
        if let Some(name) = &model.running_engine
            && model.state != RunState::Stopped
        {
            ui.label(
                egui::RichText::new(name)
                    .font(strong_font(ctx, 14.0))
                    .color(p.ink),
            );
        }
    });
    if model.state != RunState::Stopped || model.counts != (0, 0, 0) {
        let (accepted, declined, finished) = model.counts;
        ui.label(
            egui::RichText::new(i18n.t_with(
                "engine-online-counts",
                &[
                    ("accepted", &accepted.to_string()),
                    ("declined", &declined.to_string()),
                    ("finished", &finished.to_string()),
                ],
            ))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
        );
    }
    if let Some(reason) = &model.stop_reason {
        let text = match reason {
            StopReason::User => i18n.t("engine-online-stop-user"),
            StopReason::UserNow => i18n.t("engine-online-stop-user-now"),
            StopReason::LimitReached => i18n.t("engine-online-stop-limit"),
            StopReason::ConnectionLost(error) => i18n.t_with(
                "engine-online-stop-lost",
                &[("why", &problem_text(i18n, error))],
            ),
        };
        ui.colored_label(p.warning, text);
    }

    // Sem Conexão ou Lichess desligado: a Central resolve.
    if !model.connected {
        notice(ui, p, i18n.t("engine-online-needs-connection"));
        if ui
            .add(
                CsButton::new(&i18n.t("engine-online-open-center"))
                    .variant(ButtonVariant::Secondary),
            )
            .clicked()
        {
            intents.push(Intent::OpenCenter);
        }
    } else if !model.provider_on {
        notice(ui, p, i18n.t("engine-online-provider-off"));
        if ui
            .add(
                CsButton::new(&i18n.t("engine-online-open-center"))
                    .variant(ButtonVariant::Secondary),
            )
            .clicked()
        {
            intents.push(Intent::OpenCenter);
        }
    }
    if let Some(error) = error {
        let text = match error {
            Unavailable::NoConnection => i18n.t("engine-online-needs-connection"),
            Unavailable::ProviderOff => i18n.t("engine-online-provider-off"),
            Unavailable::AlreadyRunning => i18n.t("engine-online-already"),
            Unavailable::Bot(error) => crate::online::board_error_text(i18n, error),
        };
        ui.colored_label(p.danger, text);
    }

    settings(ui, ctx, i18n, form, model);
    controls(ui, i18n, form, model, intents);
    games(ui, ctx, i18n, model);
    activity(ui, ctx, i18n, model);
    conversion(ui, ctx, i18n, form, model, intents);

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(i18n.t("engine-online-footer"))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
    );
    ui.horizontal(|ui| {
        if ui
            .add(CsButton::new(&i18n.t("engine-online-close")).variant(ButtonVariant::Secondary))
            .clicked()
        {
            intents.push(Intent::Close);
        }
    });
}

fn notice(ui: &mut egui::Ui, p: &crate::ui_theme::Palette, text: String) {
    egui::Frame::new()
        .fill(p.surface_2)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).color(p.ink));
        });
}

fn heading(ui: &mut egui::Ui, ctx: &egui::Context, text: String) {
    let p = palette(ui);
    ui.label(
        egui::RichText::new(text)
            .font(strong_font(ctx, 14.0))
            .color(p.ink),
    );
}

/// Os limites. Só se editam com a Engine parada: o que vale é o que foi ligado.
fn settings(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    form: &mut Form,
    model: &ViewModel,
) {
    let p = palette(ui);
    let editable = model.state == RunState::Stopped;
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add_enabled_ui(editable, |ui| {
            heading(ui, ctx, i18n.t("engine-online-settings"));
            ui.horizontal(|ui| {
                ui.label(i18n.t("engine-online-engine"));
                if model.engines.is_empty() {
                    ui.label(
                        egui::RichText::new(i18n.t("engine-online-no-engines")).color(p.warning),
                    );
                } else {
                    let options: Vec<(Option<PathBuf>, String)> = model
                        .engines
                        .iter()
                        .map(|e| (Some(e.path.clone()), e.name.clone()))
                        .collect();
                    cs_dropdown(ui, "engine-online-engine", &mut form.engine, &options);
                }
            });

            let mut limited = form.settings.elo.is_some();
            if cs_checkbox(ui, &mut limited, &i18n.t("engine-online-limit-strength")).changed() {
                form.settings.elo = limited.then_some(1500);
            }
            if let Some(elo) = form.settings.elo.as_mut() {
                ui.add(
                    egui::Slider::new(elo, crate::engine::MIN_ELO..=crate::engine::MAX_ELO)
                        .text(i18n.t("engine-elo-label")),
                );
                ui.small(i18n.t("engine-online-strength-hint"));
            }

            ui.label(i18n.t("engine-online-speeds"));
            ui.horizontal_wrapped(|ui| {
                for (speed, key) in SPEEDS {
                    let mut on = form.settings.policy.speeds.contains(&speed);
                    if cs_checkbox(ui, &mut on, &i18n.t(key)).changed() {
                        let speeds = &mut form.settings.policy.speeds;
                        if on {
                            speeds.push(speed);
                        } else {
                            speeds.retain(|s| *s != speed);
                        }
                    }
                }
            });

            ui.label(i18n.t("engine-online-rated"));
            cs_segmented(
                ui,
                &mut form.settings.policy.rated,
                &[
                    (RatedPolicy::Both, i18n.t("engine-online-rated-both")),
                    (
                        RatedPolicy::CasualOnly,
                        i18n.t("engine-online-rated-casual"),
                    ),
                    (RatedPolicy::RatedOnly, i18n.t("engine-online-rated-only")),
                ],
            );

            ui.label(i18n.t("engine-online-opponents"));
            cs_segmented(
                ui,
                &mut form.settings.policy.opponents,
                &[
                    (
                        OpponentPolicy::PeopleOnly,
                        i18n.t("engine-online-opp-people"),
                    ),
                    (
                        OpponentPolicy::PeopleAndEngines,
                        i18n.t("engine-online-opp-any"),
                    ),
                ],
            );

            ui.label(i18n.t("engine-online-concurrent"));
            cs_segmented(
                ui,
                &mut form.settings.policy.max_concurrent,
                &[
                    (1, "1".to_owned()),
                    (2, "2".to_owned()),
                    (3, "3".to_owned()),
                ],
            );

            ui.label(i18n.t("engine-online-max-games"));
            let mut max = form.settings.max_games.unwrap_or(0);
            cs_segmented(
                ui,
                &mut max,
                &[
                    (1, "1".to_owned()),
                    (3, "3".to_owned()),
                    (5, "5".to_owned()),
                    (10, "10".to_owned()),
                    (0, i18n.t("engine-online-max-none")),
                ],
            );
            form.settings.max_games = (max != 0).then_some(max);

            ui.label(i18n.t("engine-online-draw"));
            cs_segmented(
                ui,
                &mut form.settings.draw,
                &[
                    (DrawResponse::Decline, i18n.t("engine-online-draw-decline")),
                    (DrawResponse::Accept, i18n.t("engine-online-draw-accept")),
                ],
            );
        });
    });
}

fn controls(
    ui: &mut egui::Ui,
    i18n: &I18n,
    form: &mut Form,
    model: &ViewModel,
    intents: &mut Vec<Intent>,
) {
    ui.horizontal(|ui| match model.state {
        RunState::Stopped => {
            let ready = model.connected
                && model.provider_on
                && form.engine.is_some()
                && !form.settings.policy.speeds.is_empty();
            let start_label = i18n.t("engine-online-start");
            let button = CsButton::new(&start_label)
                .icon(icon::PLAY)
                .variant(ButtonVariant::Primary)
                .enabled(ready);
            if ui.add(button).clicked() {
                intents.push(Intent::Start);
            }
            if !ready && model.connected && model.provider_on {
                ui.label(
                    egui::RichText::new(i18n.t("engine-online-start-hint"))
                        .text_style(egui::TextStyle::Small)
                        .color(palette(ui).ink_muted),
                );
            }
        }
        RunState::Waiting | RunState::Playing => {
            if ui
                .add(
                    CsButton::new(&i18n.t("engine-online-stop"))
                        .icon(icon::STOP)
                        .variant(ButtonVariant::Secondary),
                )
                .clicked()
            {
                intents.push(Intent::Stop);
            }
            stop_now_button(ui, i18n, form, model, intents);
        }
        RunState::Stopping => stop_now_button(ui, i18n, form, model, intents),
    });
    if form.confirm_stop_now && model.state != RunState::Stopped {
        let p = palette(ui);
        cs_card_frame(p, CardEmphasis::Full).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(i18n.t("engine-online-stop-now-warning"));
            ui.horizontal(|ui| {
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-online-stop-now-confirm"))
                            .variant(ButtonVariant::Danger),
                    )
                    .clicked()
                {
                    form.confirm_stop_now = false;
                    intents.push(Intent::StopNow);
                }
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-online-cancel"))
                            .variant(ButtonVariant::Secondary),
                    )
                    .clicked()
                {
                    form.confirm_stop_now = false;
                }
            });
        });
    }
}

fn stop_now_button(
    ui: &mut egui::Ui,
    i18n: &I18n,
    form: &mut Form,
    model: &ViewModel,
    _intents: &mut Vec<Intent>,
) {
    // Sem Partida em andamento, "Parar" já basta.
    if model.rows.is_empty() || matches!(model.stop_reason, Some(StopReason::UserNow)) {
        return;
    }
    if ui
        .add(
            CsButton::new(&i18n.t("engine-online-stop-now"))
                .icon(icon::WARNING)
                .variant(ButtonVariant::Danger),
        )
        .clicked()
    {
        form.confirm_stop_now = true;
    }
}

fn games(ui: &mut egui::Ui, ctx: &egui::Context, i18n: &I18n, model: &ViewModel) {
    if model.rows.is_empty() {
        return;
    }
    let p = palette(ui);
    heading(ui, ctx, i18n.t("engine-online-games"));
    for row in &model.rows {
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let who = match row.rating {
                    Some(rating) => format!("{} ({rating})", row.opponent),
                    None => row.opponent.clone(),
                };
                ui.label(
                    egui::RichText::new(who)
                        .font(strong_font(ctx, 14.0))
                        .color(p.ink),
                );
                let side = if row.white {
                    i18n.t("engine-online-side-white")
                } else {
                    i18n.t("engine-online-side-black")
                };
                let mode = if row.rated {
                    i18n.t("engine-online-mode-rated")
                } else {
                    i18n.t("engine-online-mode-casual")
                };
                ui.label(
                    egui::RichText::new(format!("{side} · {mode}"))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                );
            });
            let status = if row.link_lost {
                i18n.t("engine-online-game-reconnecting")
            } else if row.over {
                i18n.t("engine-online-game-over")
            } else if row.engine_thinking {
                i18n.t("engine-online-game-thinking")
            } else {
                i18n.t("engine-online-game-waiting")
            };
            ui.label(
                egui::RichText::new(i18n.t_with(
                    "engine-online-game-line",
                    &[("plies", &row.plies.to_string()), ("status", &status)],
                ))
                .text_style(egui::TextStyle::Small)
                .color(p.ink_muted),
            );
        });
    }
}

fn activity_text(i18n: &I18n, item: &Activity) -> (StatusTone, String) {
    match item {
        Activity::Accepted { name } => (
            StatusTone::Success,
            i18n.t_with("engine-online-act-accepted", &[("name", name)]),
        ),
        Activity::Declined { name, reason } => (
            StatusTone::Neutral,
            i18n.t_with(
                "engine-online-act-declined",
                &[("name", name), ("why", &i18n.t(decline_key(*reason)))],
            ),
        ),
        Activity::GameResumed { opponent } => (
            StatusTone::Info,
            i18n.t_with("engine-online-act-resumed", &[("name", opponent)]),
        ),
        Activity::GameStarted { opponent } => (
            StatusTone::Info,
            i18n.t_with("engine-online-act-started", &[("name", opponent)]),
        ),
        Activity::GameEnded { opponent, result } => (
            StatusTone::Neutral,
            i18n.t_with(
                "engine-online-act-ended",
                &[
                    ("name", opponent),
                    ("result", result.as_deref().unwrap_or("-")),
                ],
            ),
        ),
        Activity::AcceptFailed { name, error } => (
            StatusTone::Danger,
            i18n.t_with(
                "engine-online-act-accept-failed",
                &[("name", name), ("why", &problem_text(i18n, error))],
            ),
        ),
        Activity::EngineMoveRejected => (
            StatusTone::Warning,
            i18n.t("engine-online-act-move-rejected"),
        ),
        Activity::EngineFailed { .. } => (
            StatusTone::Danger,
            i18n.t("engine-online-act-engine-failed"),
        ),
        Activity::EngineStartFailed { .. } => (
            StatusTone::Danger,
            i18n.t("engine-online-act-engine-start-failed"),
        ),
        Activity::RequestFailed(error) => (
            StatusTone::Warning,
            i18n.t_with(
                "engine-online-act-request-failed",
                &[("why", &problem_text(i18n, error))],
            ),
        ),
        Activity::SessionClosed(error) => (
            StatusTone::Warning,
            i18n.t_with(
                "engine-online-act-session-closed",
                &[("why", &crate::online::board_error_text(i18n, error))],
            ),
        ),
        Activity::StoreFailed(_) => (
            StatusTone::Warning,
            i18n.t("engine-online-act-store-failed"),
        ),
    }
}

fn activity(ui: &mut egui::Ui, ctx: &egui::Context, i18n: &I18n, model: &ViewModel) {
    if model.activity.is_empty() {
        return;
    }
    let p = palette(ui);
    heading(ui, ctx, i18n.t("engine-online-activity"));
    for item in model.activity.iter().rev().take(6) {
        let (tone, text) = activity_text(i18n, item);
        let color = match tone {
            StatusTone::Success => p.success,
            StatusTone::Info => p.info,
            StatusTone::Warning => p.warning,
            StatusTone::Danger => p.danger,
            StatusTone::Neutral | StatusTone::Accent => p.ink_muted,
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("•").color(color));
            ui.label(egui::RichText::new(text).color(p.ink));
        });
    }
}

/// A conversão da conta em BOT: irreversível, nunca sozinha, com aviso e confirmação.
fn conversion(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    form: &mut Form,
    model: &ViewModel,
    intents: &mut Vec<Intent>,
) {
    if !model.connected {
        return;
    }
    let p = palette(ui);
    crate::ui_theme::cs_disclosure(
        ui,
        &mut form.upgrade_open,
        &i18n.t("engine-online-convert-title"),
        &i18n.t("engine-online-convert-hint"),
    );
    if !form.upgrade_open {
        return;
    }
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, ctx, i18n.t("engine-online-convert-title"));
        ui.colored_label(
            p.warning,
            chess_providers::IrreversibleConfirmation::WARNING,
        );
        match model.conversion {
            Some(ConversionView::Sending) => {
                ui.label(i18n.t("engine-online-convert-sending"));
            }
            Some(ConversionView::Done) => {
                ui.colored_label(p.success, i18n.t("engine-online-convert-done"));
            }
            other => {
                if other == Some(ConversionView::Failed) {
                    ui.colored_label(p.danger, i18n.t("engine-online-convert-failed"));
                }
                cs_checkbox(
                    ui,
                    &mut form.upgrade_understood,
                    &i18n.t("engine-online-convert-understood"),
                );
                let convert_label = i18n.t("engine-online-convert-act");
                let button = CsButton::new(&convert_label)
                    .variant(ButtonVariant::Danger)
                    .enabled(form.upgrade_understood && model.provider_on);
                if ui.add(button).clicked() {
                    form.upgrade_understood = false;
                    intents.push(Intent::Upgrade);
                }
            }
        }
    });
}
