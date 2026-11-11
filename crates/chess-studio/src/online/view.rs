//! A interface da Partida online: a tela "Partida online no Lichess" (buscar adversário,
//! desafiar, Engine do Lichess, Partidas em andamento), o Painel Partida online, o aviso
//! discreto de desafio recebido e a confirmação de abandono. Só widgets e tokens do design
//! system; textos em Fluent, na linguagem do usuário. Nada aqui escreve a Partida: as
//! intenções vão à `Session` ou ao serviço online.

use super::service::{OnlineNotice, OnlinePlay, OutgoingState, PlayingRow, Unavailable};
use super::{OnlineService, OnlineUi, Tab, board_error_text, provider_error_text};
use crate::actions::ActionRequested;
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::session::{OnlineIntent, Session};
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, StatusTone, cs_card_frame, cs_dropdown, cs_segmented,
    cs_status_badge, cs_switch, cs_text_field, icon_font, palette, strong_font, tabular_text,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use chess_core::Color;
use chess_game::Opponent;
use chess_input::controls::Action;
use chess_net::Timestamp;
use chess_providers::{
    CORRESPONDENCE_DAYS, Challenge, ChallengeSpec, DeclineReason, LinkStatus, OnlineTime, Seek,
};
use egui_phosphor::regular as icon;
use std::collections::BTreeMap;
use std::time::Duration;

/// Largura da tela da Partida online.
const DIALOG_W: f32 = 520.0;

/// Ritmos oferecidos (base e incremento em segundos).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pace {
    Blitz3_2,
    Blitz5_0,
    Blitz5_3,
    Rapid10_0,
    Rapid10_5,
    Rapid15_10,
    Classical30_0,
    Classical30_20,
    Correspondence,
}

impl Pace {
    const BLITZ: [Pace; 3] = [Pace::Blitz3_2, Pace::Blitz5_0, Pace::Blitz5_3];
    const RAPID: [Pace; 3] = [Pace::Rapid10_0, Pace::Rapid10_5, Pace::Rapid15_10];
    const CLASSICAL: [Pace; 2] = [Pace::Classical30_0, Pace::Classical30_20];

    fn clock(self) -> Option<(u32, u32)> {
        Some(match self {
            Pace::Blitz3_2 => (180, 2),
            Pace::Blitz5_0 => (300, 0),
            Pace::Blitz5_3 => (300, 3),
            Pace::Rapid10_0 => (600, 0),
            Pace::Rapid10_5 => (600, 5),
            Pace::Rapid15_10 => (900, 10),
            Pace::Classical30_0 => (1800, 0),
            Pace::Classical30_20 => (1800, 20),
            Pace::Correspondence => return None,
        })
    }

    /// "10+5" (minutos + segundos).
    fn short(self) -> String {
        match self.clock() {
            Some((base, inc)) => format!("{}+{inc}", base / 60),
            None => String::new(),
        }
    }

    pub fn time(self, days: u8) -> OnlineTime {
        match self.clock() {
            Some((base_s, increment_s)) => OnlineTime::RealTime {
                base_s,
                increment_s,
            },
            None => OnlineTime::Correspondence { days },
        }
    }
}

/// O que o jogador preenche na tela.
#[derive(Debug, Clone)]
pub struct Form {
    pub seek_pace: Pace,
    pub seek_rated: bool,
    pub days: u8,
    pub name: String,
    pub challenge_pace: Pace,
    pub challenge_rated: bool,
    pub challenge_color: Option<Color>,
    pub engine_level: u8,
    pub engine_pace: Pace,
    pub engine_color: Option<Color>,
    /// Motivo escolhido para recusar cada desafio recebido.
    pub reasons: BTreeMap<String, DeclineReason>,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            seek_pace: Pace::Rapid10_5,
            seek_rated: false,
            days: 3,
            name: String::new(),
            challenge_pace: Pace::Blitz5_3,
            challenge_rated: false,
            challenge_color: None,
            engine_level: 3,
            engine_pace: Pace::Rapid10_5,
            engine_color: None,
            reasons: BTreeMap::new(),
        }
    }
}

/// "Rápida 10+5" / "3 dias por Lance".
pub fn time_label(i18n: &I18n, time: OnlineTime) -> String {
    match time {
        OnlineTime::RealTime {
            base_s,
            increment_s,
        } => {
            let estimate = base_s + 40 * increment_s;
            let kind = if estimate < 8 * 60 {
                "online-pace-blitz"
            } else if estimate < 25 * 60 {
                "online-pace-rapid"
            } else {
                "online-pace-classical"
            };
            let minutes = if base_s % 60 == 0 {
                (base_s / 60).to_string()
            } else {
                format!("{},{}", base_s / 60, (base_s % 60) * 10 / 60)
            };
            format!("{} {minutes}+{increment_s}", i18n.t(kind))
        }
        OnlineTime::Correspondence { days } => days_label(i18n, days),
        OnlineTime::Unlimited => i18n.t("online-pace-correspondence"),
    }
}

fn days_label(i18n: &I18n, days: u8) -> String {
    if days == 1 {
        i18n.t("online-pace-days-one")
    } else {
        i18n.t_with("online-pace-days-other", &[("days", &days.to_string())])
    }
}

/// "2 dias", "5 h", "12 min".
pub fn left_label(i18n: &I18n, seconds: u64) -> String {
    let days = seconds / 86_400;
    if days >= 1 {
        return if days == 1 {
            i18n.t("online-time-days-one")
        } else {
            i18n.t_with("online-time-days-other", &[("n", &days.to_string())])
        };
    }
    let hours = seconds / 3600;
    if hours >= 1 {
        return i18n.t_with("online-time-hours", &[("n", &hours.to_string())]);
    }
    i18n.t_with(
        "online-time-minutes",
        &[("n", &(seconds / 60).max(1).to_string())],
    )
}

/// "0:42", "12:05".
fn elapsed_label(elapsed: Duration) -> String {
    let s = elapsed.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

fn rated_label(i18n: &I18n, rated: bool) -> String {
    i18n.t(if rated {
        "online-rated-short"
    } else {
        "online-casual"
    })
}

fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    let p = palette(ui);
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(p.ink_muted)).wrap());
}

fn section(ui: &mut egui::Ui, text: &str) {
    let p = palette(ui);
    ui.label(
        egui::RichText::new(text)
            .font(strong_font(ui.ctx(), 14.0))
            .color(p.ink),
    );
}

/// Linha com ícone e texto (avisos, Trava de fair play).
fn icon_line(ui: &mut egui::Ui, glyph: &str, tint: egui::Color32, text: &str) {
    let p = palette(ui);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(
            egui::RichText::new(glyph)
                .font(icon_font(ui.ctx(), 16.0))
                .color(tint),
        );
        ui.add(egui::Label::new(egui::RichText::new(text).color(p.ink)).wrap());
    });
}

/// Bloco centrado de estado (sem Conexão, espera…).
fn state_block(ui: &mut egui::Ui, glyph: Option<&str>, title: &str, text: &str) {
    let p = palette(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        match glyph {
            Some(glyph) => {
                ui.label(
                    egui::RichText::new(glyph)
                        .font(icon_font(ui.ctx(), 32.0))
                        .color(p.accent),
                );
            }
            None => {
                ui.add(egui::Spinner::new().size(32.0).color(p.accent));
            }
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(title)
                .font(strong_font(ui.ctx(), 16.0))
                .color(p.ink),
        );
        if !text.is_empty() {
            ui.add_space(4.0);
            ui.add(egui::Label::new(egui::RichText::new(text).color(p.ink_muted)).wrap());
        }
        ui.add_space(8.0);
    });
}

/// Grade de ritmos: rótulo da categoria e os botões de alternância.
fn pace_grid(
    ui: &mut egui::Ui,
    i18n: &I18n,
    id: &str,
    rows: &[(&str, &[Pace])],
    value: &mut Pace,
    days: u8,
) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            for (label, paces) in rows {
                let p = palette(ui);
                ui.label(egui::RichText::new(i18n.t(label)).color(p.ink_muted));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for pace in *paces {
                        // Correspondência: o botão já diz os dias escolhidos.
                        let text = if *pace == Pace::Correspondence {
                            days_label(i18n, days)
                        } else {
                            pace.short()
                        };
                        if ui
                            .add(CsButton::new(&text).toggled(*value == *pace))
                            .clicked()
                        {
                            *value = *pace;
                        }
                    }
                });
                ui.end_row();
            }
        });
}

fn color_choice(ui: &mut egui::Ui, i18n: &I18n, value: &mut Option<Color>) {
    let options = [
        (None, i18n.t("online-color-random")),
        (Some(Color::White), i18n.t("online-color-white")),
        (Some(Color::Black), i18n.t("online-color-black")),
    ];
    cs_segmented(ui, value, &options);
}

fn days_choice(ui: &mut egui::Ui, i18n: &I18n, days: &mut u8) {
    let options: Vec<(u8, String)> = CORRESPONDENCE_DAYS
        .iter()
        .map(|d| (*d, days_label(i18n, *d)))
        .collect();
    ui.horizontal(|ui| {
        let p = palette(ui);
        ui.label(egui::RichText::new(i18n.t("online-days-label")).color(p.ink_muted));
        cs_dropdown(ui, "online-days", days, &options);
    });
}

fn unavailable_text(i18n: &I18n, error: &Unavailable) -> String {
    match error {
        Unavailable::NoConnection => i18n.t("online-error-sign-in"),
        Unavailable::ProviderOff => i18n.t("online-error-disabled"),
        Unavailable::Board(error) => board_error_text(i18n, error),
    }
}

/// O texto de um aviso do serviço.
pub fn notice_text(i18n: &I18n, notice: &OnlineNotice) -> Option<(StatusTone, String)> {
    Some(match notice {
        OnlineNotice::GameStarted { .. } => (StatusTone::Success, i18n.t("online-notice-started")),
        OnlineNotice::ChallengeReceived { .. } => return None,
        OnlineNotice::ChallengeDeclined { name } => (
            StatusTone::Warning,
            i18n.t_with("online-notice-declined", &[("name", name)]),
        ),
        OnlineNotice::Failed(error) => (StatusTone::Danger, provider_error_text(i18n, error)),
        OnlineNotice::MoveRefused { mv } => (
            StatusTone::Warning,
            i18n.t_with("online-notice-move-refused", &[("mv", &mv.to_string())]),
        ),
        OnlineNotice::MoveLost { mv } => (
            StatusTone::Warning,
            i18n.t_with("online-notice-move-lost", &[("mv", &mv.to_string())]),
        ),
        OnlineNotice::MoveReplaced { mv } => (
            StatusTone::Info,
            i18n.t_with("online-notice-move-replaced", &[("mv", &mv.to_string())]),
        ),
        OnlineNotice::RequestFailed { .. } => {
            (StatusTone::Warning, i18n.t("online-notice-request-failed"))
        }
        OnlineNotice::Desynced => (StatusTone::Warning, i18n.t("online-notice-desync")),
        OnlineNotice::SessionClosed(error) => (StatusTone::Danger, board_error_text(i18n, error)),
        OnlineNotice::VariantIgnored { variant } => (
            StatusTone::Info,
            i18n.t_with("online-notice-variant", &[("variant", variant)]),
        ),
        OnlineNotice::SeekEnded => (StatusTone::Info, i18n.t("online-notice-seek-ended")),
        OnlineNotice::EventsClosed(_) => {
            (StatusTone::Danger, i18n.t("online-notice-events-closed"))
        }
        OnlineNotice::StoreFailed(error) => (
            StatusTone::Danger,
            i18n.t_with("online-notice-store", &[("error", error)]),
        ),
    })
}

fn notice_glyph(tone: StatusTone) -> &'static str {
    match tone {
        StatusTone::Success => icon::CHECK_CIRCLE,
        StatusTone::Warning => icon::WARNING,
        StatusTone::Danger => icon::WARNING_OCTAGON,
        _ => icon::INFO,
    }
}

fn notice_line(ui: &mut egui::Ui, tone: StatusTone, text: &str) {
    let p = palette(ui);
    let tint = match tone {
        StatusTone::Success => p.success,
        StatusTone::Warning => p.warning,
        StatusTone::Danger => p.danger,
        _ => p.info,
    };
    icon_line(ui, notice_glyph(tone), tint, text);
}

// --- A tela "Partida online no Lichess" -----------------------------------------------------

/// O que a tela pediu neste quadro.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Intent {
    Close,
    OpenCenter,
    Seek,
    CancelSeek,
    Challenge,
    CancelChallenge(usize),
    Engine,
    Accept(String),
    Decline(String),
    Refresh,
    Open(String),
}

/// Um retrato do serviço para desenhar sem segurar o `Mutex`.
pub(super) struct DialogView {
    pub(super) connected: bool,
    pub(super) provider_on: bool,
    pub(super) seek: Option<(Seek, Timestamp, bool)>,
    pub(super) engine_pending: bool,
    pub(super) outgoing: Vec<(String, OnlineTime, OutgoingState)>,
    pub(super) incoming: Vec<Challenge>,
    pub(super) rows: Vec<PlayingRow>,
    pub(super) refreshing: bool,
    pub(super) refreshed: Option<Timestamp>,
    pub(super) now: Timestamp,
}

impl DialogView {
    fn of(play: &OnlinePlay, net: &NetService) -> Self {
        Self {
            connected: play.connected(),
            provider_on: net.is_enabled(chess_net::ProviderId::Lichess),
            seek: play.seek_run().map(|r| (r.seek, r.since, r.posted)),
            engine_pending: play.engine_pending(),
            outgoing: play
                .outgoing
                .iter()
                .map(|o| (o.name.clone(), o.spec.time, o.state.clone()))
                .collect(),
            incoming: play.incoming.clone(),
            rows: play.playing_rows(),
            refreshing: play.refreshing(),
            refreshed: play.refreshed_at(),
            now: net.clock().now(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn online_dialog(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut ui_state: ResMut<OnlineUi>,
    service: Res<OnlineService>,
    net: Option<Res<NetService>>,
    mut actions: MessageWriter<ActionRequested>,
) -> Result {
    if !ui_state.open {
        return Ok(());
    }
    let Some(net) = net else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let view = match ui_state.demo.as_ref() {
        Some(scene) => scene.view_for_dialog(),
        None => service.with(|play| DialogView::of(play, &net)),
    };
    egui::Area::new(egui::Id::new("online-dialog-dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(&ctx, |ui| {
            let rect = ctx.viewport_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
        });
    let mut intents = Vec::new();
    // O egui guarda o tamanho da janela: um id por conteúdo faz ela encolher ao trocar de aba
    // ou de estado, em vez de herdar a altura da aba mais longa.
    let content = (
        ui_state.tab,
        view.connected && view.provider_on,
        view.seek.map(|(_, _, posted)| posted),
        view.engine_pending,
        view.incoming.len(),
        view.outgoing.len(),
        view.rows.len(),
        ui_state.error.is_some(),
    );
    let max_height = (ctx.viewport_rect().height() - 160.0).max(240.0);
    egui::Window::new(i18n.t("online-title"))
        .id(egui::Id::new(("online-dialog", content)))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.set_width(DIALOG_W);
            // Alta até caber na janela; o que passa disso rola (a aba Desafiar com desafios
            // recebidos é a mais longa).
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .min_scrolled_height(max_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    dialog_body(ui, &i18n, &mut ui_state, &view, &mut intents);
                });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        intents.push(Intent::Close);
    }
    if ui_state.demo.is_some() {
        return Ok(());
    }
    for intent in intents {
        apply_intent(intent, &mut ui_state, &service, &net, &mut actions);
    }
    Ok(())
}

fn apply_intent(
    intent: Intent,
    ui_state: &mut OnlineUi,
    service: &OnlineService,
    net: &NetService,
    actions: &mut MessageWriter<ActionRequested>,
) {
    let form = ui_state.form.clone();
    let result = service.with(|play| match intent {
        Intent::Close => {
            ui_state.open = false;
            Ok(())
        }
        Intent::OpenCenter => {
            ui_state.open = false;
            actions.write(ActionRequested(Action::OpenConnections));
            Ok(())
        }
        Intent::Seek => play.seek(
            net,
            Seek {
                time: form.seek_pace.time(form.days),
                rated: form.seek_rated,
                color: None,
            },
        ),
        Intent::CancelSeek => {
            play.cancel_seek();
            Ok(())
        }
        Intent::Challenge => play.challenge(
            net,
            &form.name,
            ChallengeSpec {
                time: form.challenge_pace.time(form.days),
                rated: form.challenge_rated,
                color: form.challenge_color,
            },
        ),
        Intent::CancelChallenge(index) => {
            play.cancel_challenge(index);
            Ok(())
        }
        Intent::Engine => play.play_engine(
            net,
            form.engine_level,
            ChallengeSpec {
                time: form.engine_pace.time(form.days),
                rated: false,
                color: form.engine_color,
            },
        ),
        Intent::Accept(id) => play.accept(net, &id),
        Intent::Decline(id) => {
            let reason = form
                .reasons
                .get(&id)
                .copied()
                .unwrap_or(DeclineReason::Generic);
            play.decline(&id, reason);
            Ok(())
        }
        Intent::Refresh => play.refresh_playing(net),
        Intent::Open(id) => play.open(net, &id).map(|()| {
            ui_state.open = false;
        }),
    });
    match result {
        Ok(()) => ui_state.error = None,
        Err(error) => ui_state.error = Some(error),
    }
}

fn dialog_body(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &DialogView,
    intents: &mut Vec<Intent>,
) {
    if !view.connected || !view.provider_on {
        let (title, text) = if view.connected {
            ("online-provider-off-title", "online-provider-off-text")
        } else {
            (
                "online-needs-connection-title",
                "online-needs-connection-text",
            )
        };
        state_block(ui, Some(icon::PLUGS), &i18n.t(title), &i18n.t(text));
        footer(ui, |ui| {
            if ui
                .add(CsButton::new(&i18n.t("online-open-center")).variant(ButtonVariant::Primary))
                .clicked()
            {
                intents.push(Intent::OpenCenter);
            }
            if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
                intents.push(Intent::Close);
            }
        });
        return;
    }
    let tabs = [
        (Tab::Seek, i18n.t("online-tab-seek")),
        (Tab::Challenge, i18n.t("online-tab-challenge")),
        (Tab::Engine, i18n.t("online-tab-engine")),
        (Tab::Playing, i18n.t("online-tab-playing")),
    ];
    cs_segmented(ui, &mut state.tab, &tabs);
    ui.add_space(16.0);
    let error = state.error.as_ref().map(|e| unavailable_text(i18n, e));
    match state.tab {
        Tab::Seek => seek_tab(ui, i18n, state, view, intents),
        Tab::Challenge => challenge_tab(ui, i18n, state, view, intents),
        Tab::Engine => engine_tab(ui, i18n, state, view, intents),
        Tab::Playing => playing_tab(ui, i18n, view, intents),
    }
    if let Some(error) = error {
        ui.add_space(8.0);
        notice_line(ui, StatusTone::Danger, &error);
    }
}

/// Rodapé com os botões à direita (o primário por último, à direita).
fn footer(ui: &mut egui::Ui, buttons: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(16.0);
    // Uma linha da altura de um controle: sem isso o layout da direita ocuparia o resto da
    // janela e o rodapé flutuaria no meio dela.
    let height = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            buttons(ui);
        },
    );
}

fn fairplay_note(ui: &mut egui::Ui, i18n: &I18n) {
    let p = palette(ui);
    icon_line(
        ui,
        icon::SHIELD_CHECK,
        p.accent,
        &i18n.t("online-fairplay-note"),
    );
}

fn seek_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &DialogView,
    intents: &mut Vec<Intent>,
) {
    if let Some((seek, since, posted)) = view.seek {
        if posted {
            state_block(
                ui,
                Some(icon::ENVELOPE_SIMPLE),
                &i18n.t("online-seek-posted-title"),
                &i18n.t("online-seek-posted-text"),
            );
            footer(ui, |ui| {
                if ui
                    .add(CsButton::new(&i18n.t("online-close")).variant(ButtonVariant::Primary))
                    .clicked()
                {
                    intents.push(Intent::Close);
                }
            });
            return;
        }
        waiting_block(ui, i18n, seek, view.now.since(since));
        footer(ui, |ui| {
            if ui
                .add(CsButton::new(&i18n.t("online-seek-cancel")))
                .clicked()
            {
                intents.push(Intent::CancelSeek);
            }
        });
        return;
    }
    section(ui, &i18n.t("online-pace-label"));
    ui.add_space(8.0);
    let rows: [(&str, &[Pace]); 3] = [
        ("online-pace-rapid", &Pace::RAPID),
        ("online-pace-classical", &Pace::CLASSICAL),
        ("online-pace-correspondence", &[Pace::Correspondence]),
    ];
    pace_grid(
        ui,
        i18n,
        "online-seek-paces",
        &rows,
        &mut state.form.seek_pace,
        state.form.days,
    );
    if state.form.seek_pace == Pace::Correspondence {
        ui.add_space(8.0);
        days_choice(ui, i18n, &mut state.form.days);
    }
    ui.add_space(12.0);
    cs_switch(ui, &mut state.form.seek_rated, &i18n.t("online-rated"));
    ui.add_space(12.0);
    muted(ui, i18n.t("online-seek-note"));
    ui.add_space(8.0);
    fairplay_note(ui, i18n);
    footer(ui, |ui| {
        if ui
            .add(CsButton::new(&i18n.t("online-seek-start")).variant(ButtonVariant::Primary))
            .clicked()
        {
            intents.push(Intent::Seek);
        }
        if ui.add(CsButton::new(&i18n.t("online-cancel"))).clicked() {
            intents.push(Intent::Close);
        }
    });
}

/// "Procurando adversário": ritmo, rating e o tempo de espera (números tabulares).
fn waiting_block(ui: &mut egui::Ui, i18n: &I18n, seek: Seek, elapsed: Duration) {
    let p = palette(ui);
    let detail = i18n.t_with(
        "online-seek-waiting-detail",
        &[
            ("pace", &time_label(i18n, seek.time)),
            ("rated", &rated_label(i18n, seek.rated)),
        ],
    );
    state_block(ui, None, &i18n.t("online-seek-waiting"), &detail);
    ui.vertical_centered(|ui| {
        let text = i18n.t_with("online-seek-elapsed", &[("time", &elapsed_label(elapsed))]);
        tabular_text(ui, &text, egui::FontId::proportional(14.0), p.ink_muted);
    });
    ui.ctx().request_repaint_after(Duration::from_millis(500));
}

fn challenge_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &DialogView,
    intents: &mut Vec<Intent>,
) {
    if !view.incoming.is_empty() {
        section(ui, &i18n.t("online-incoming-title"));
        ui.add_space(8.0);
        for challenge in &view.incoming {
            incoming_card(ui, i18n, challenge, &mut state.form.reasons, intents);
            ui.add_space(8.0);
        }
        ui.add_space(8.0);
    }
    section(ui, &i18n.t("online-challenge-name"));
    ui.add_space(4.0);
    cs_text_field(
        ui,
        &mut state.form.name,
        &i18n.t("online-challenge-name-hint"),
        None,
    );
    ui.add_space(12.0);
    section(ui, &i18n.t("online-pace-label"));
    ui.add_space(8.0);
    let rows: [(&str, &[Pace]); 4] = [
        ("online-pace-blitz", &Pace::BLITZ),
        ("online-pace-rapid", &Pace::RAPID),
        ("online-pace-classical", &Pace::CLASSICAL),
        ("online-pace-correspondence", &[Pace::Correspondence]),
    ];
    pace_grid(
        ui,
        i18n,
        "online-challenge-paces",
        &rows,
        &mut state.form.challenge_pace,
        state.form.days,
    );
    if state.form.challenge_pace == Pace::Correspondence {
        ui.add_space(8.0);
        days_choice(ui, i18n, &mut state.form.days);
    }
    ui.add_space(12.0);
    section(ui, &i18n.t("online-color-label"));
    ui.add_space(4.0);
    color_choice(ui, i18n, &mut state.form.challenge_color);
    ui.add_space(12.0);
    cs_switch(ui, &mut state.form.challenge_rated, &i18n.t("online-rated"));
    if !view.outgoing.is_empty() {
        ui.add_space(12.0);
        for (index, (name, time, out)) in view.outgoing.iter().enumerate() {
            outgoing_row(ui, i18n, index, name, *time, out, intents);
        }
    }
    ui.add_space(8.0);
    fairplay_note(ui, i18n);
    let can_send = !state.form.name.trim().is_empty();
    footer(ui, |ui| {
        if ui
            .add(
                CsButton::new(&i18n.t("online-challenge-send"))
                    .variant(ButtonVariant::Primary)
                    .enabled(can_send),
            )
            .clicked()
        {
            intents.push(Intent::Challenge);
        }
        if ui.add(CsButton::new(&i18n.t("online-cancel"))).clicked() {
            intents.push(Intent::Close);
        }
    });
}

fn outgoing_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    index: usize,
    name: &str,
    time: OnlineTime,
    out: &OutgoingState,
    intents: &mut Vec<Intent>,
) {
    let p = palette(ui);
    let (tone, glyph, key) = match out {
        OutgoingState::Sending => (
            StatusTone::Info,
            icon::PAPER_PLANE_TILT,
            "online-challenge-sending",
        ),
        OutgoingState::Waiting => (
            StatusTone::Info,
            icon::HOURGLASS,
            "online-challenge-waiting",
        ),
        OutgoingState::Declined => (
            StatusTone::Warning,
            icon::X_CIRCLE,
            "online-challenge-declined",
        ),
        OutgoingState::Canceled => (
            StatusTone::Neutral,
            icon::X_CIRCLE,
            "online-challenge-canceled",
        ),
        OutgoingState::Failed(_) => (
            StatusTone::Danger,
            icon::WARNING_OCTAGON,
            "online-challenge-failed",
        ),
    };
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new(i18n.t_with("online-challenge-to", &[("name", name)]))
                .font(strong_font(ui.ctx(), 14.0))
                .color(p.ink),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            cs_status_badge(ui, tone, glyph, &i18n.t(key));
            muted(ui, time_label(i18n, time));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if matches!(out, OutgoingState::Waiting | OutgoingState::Sending)
                    && ui
                        .add(
                            CsButton::new(&i18n.t("online-challenge-cancel"))
                                .variant(ButtonVariant::Subtle),
                        )
                        .clicked()
                {
                    intents.push(Intent::CancelChallenge(index));
                }
            });
        });
    });
    ui.add_space(4.0);
}

fn reason_options(i18n: &I18n) -> Vec<(DeclineReason, String)> {
    [
        (DeclineReason::Generic, "online-reason-generic"),
        (DeclineReason::Later, "online-reason-later"),
        (DeclineReason::TooFast, "online-reason-too-fast"),
        (DeclineReason::TooSlow, "online-reason-too-slow"),
        (DeclineReason::TimeControl, "online-reason-time-control"),
        (DeclineReason::Rated, "online-reason-rated"),
        (DeclineReason::Casual, "online-reason-casual"),
        (DeclineReason::Standard, "online-reason-standard"),
        (DeclineReason::Variant, "online-reason-variant"),
    ]
    .into_iter()
    .map(|(r, key)| (r, i18n.t(key)))
    .collect()
}

fn incoming_card(
    ui: &mut egui::Ui,
    i18n: &I18n,
    challenge: &Challenge,
    reasons: &mut BTreeMap<String, DeclineReason>,
    intents: &mut Vec<Intent>,
) {
    let p = palette(ui);
    let standard = challenge.variant == "standard" || challenge.variant == "fromPosition";
    cs_card_frame(p, CardEmphasis::Full).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let name = match challenge.challenger.rating {
            Some(r) => format!("{} ({r})", challenge.challenger.name),
            None => challenge.challenger.name.clone(),
        };
        ui.label(
            egui::RichText::new(i18n.t_with("online-incoming-line", &[("name", &name)]))
                .font(strong_font(ui.ctx(), 14.0))
                .color(p.ink),
        );
        muted(
            ui,
            format!(
                "{} · {}",
                time_label(i18n, challenge.time),
                rated_label(i18n, challenge.rated)
            ),
        );
        if !standard {
            ui.add_space(4.0);
            notice_line(
                ui,
                StatusTone::Warning,
                &i18n.t_with(
                    "online-incoming-variant",
                    &[("variant", &challenge.variant)],
                ),
            );
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if standard
                && ui
                    .add(CsButton::new(&i18n.t("online-accept")).variant(ButtonVariant::Primary))
                    .clicked()
            {
                intents.push(Intent::Accept(challenge.id.clone()));
            }
            if ui.add(CsButton::new(&i18n.t("online-decline"))).clicked() {
                intents.push(Intent::Decline(challenge.id.clone()));
            }
            let default = if standard {
                DeclineReason::Generic
            } else {
                DeclineReason::Standard
            };
            let reason = reasons.entry(challenge.id.clone()).or_insert(default);
            ui.label(egui::RichText::new(i18n.t("online-decline-reason")).color(p.ink_muted));
            cs_dropdown(
                ui,
                ("online-reason", &challenge.id),
                reason,
                &reason_options(i18n),
            );
        });
    });
}

fn engine_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &DialogView,
    intents: &mut Vec<Intent>,
) {
    if view.engine_pending {
        state_block(ui, None, &i18n.t("online-engine-waiting"), "");
        footer(ui, |ui| {
            if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
                intents.push(Intent::Close);
            }
        });
        return;
    }
    section(ui, &i18n.t("online-engine-level"));
    ui.add_space(4.0);
    let levels: Vec<(u8, String)> = (1..=8).map(|l| (l, l.to_string())).collect();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (level, label) in &levels {
            if ui
                .add(CsButton::new(label).toggled(state.form.engine_level == *level))
                .clicked()
            {
                state.form.engine_level = *level;
            }
        }
    });
    ui.add_space(12.0);
    section(ui, &i18n.t("online-color-label"));
    ui.add_space(4.0);
    color_choice(ui, i18n, &mut state.form.engine_color);
    ui.add_space(12.0);
    section(ui, &i18n.t("online-pace-label"));
    ui.add_space(8.0);
    let rows: [(&str, &[Pace]); 3] = [
        ("online-pace-blitz", &Pace::BLITZ),
        ("online-pace-rapid", &Pace::RAPID),
        ("online-pace-classical", &Pace::CLASSICAL),
    ];
    pace_grid(
        ui,
        i18n,
        "online-engine-paces",
        &rows,
        &mut state.form.engine_pace,
        state.form.days,
    );
    ui.add_space(12.0);
    let p = palette(ui);
    icon_line(ui, icon::INFO, p.info, &i18n.t("online-engine-note"));
    footer(ui, |ui| {
        if ui
            .add(CsButton::new(&i18n.t("online-engine-start")).variant(ButtonVariant::Primary))
            .clicked()
        {
            intents.push(Intent::Engine);
        }
        if ui.add(CsButton::new(&i18n.t("online-cancel"))).clicked() {
            intents.push(Intent::Close);
        }
    });
}

fn playing_tab(ui: &mut egui::Ui, i18n: &I18n, view: &DialogView, intents: &mut Vec<Intent>) {
    ui.horizontal(|ui| {
        section(ui, &i18n.t("online-playing-title"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if view.refreshing {
                i18n.t("online-playing-refreshing")
            } else {
                i18n.t("online-playing-refresh")
            };
            if ui
                .add(
                    CsButton::new(&label)
                        .icon(icon::ARROWS_CLOCKWISE)
                        .enabled(!view.refreshing),
                )
                .clicked()
            {
                intents.push(Intent::Refresh);
            }
        });
    });
    ui.add_space(4.0);
    match view.refreshed {
        Some(at) if view.now.since(at) < Duration::from_secs(60) => {
            muted(ui, i18n.t("online-playing-refreshed-now"));
        }
        Some(at) => muted(
            ui,
            i18n.t_with(
                "online-playing-refreshed",
                &[("time", &left_label(i18n, view.now.since(at).as_secs()))],
            ),
        ),
        None => muted(ui, i18n.t("online-playing-local")),
    }
    ui.add_space(8.0);
    if view.rows.is_empty() {
        state_block(
            ui,
            Some(icon::CHECKERBOARD),
            &i18n.t("online-playing-empty"),
            "",
        );
    }
    egui::ScrollArea::vertical()
        .max_height(320.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for row in &view.rows {
                playing_row(ui, i18n, row, intents);
                ui.add_space(4.0);
            }
        });
    footer(ui, |ui| {
        if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
            intents.push(Intent::Close);
        }
    });
}

fn playing_row(ui: &mut egui::Ui, i18n: &I18n, row: &PlayingRow, intents: &mut Vec<Intent>) {
    let p = palette(ui);
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let name = match row.rating {
                    Some(r) => format!("{} ({r})", row.opponent),
                    None => row.opponent.clone(),
                };
                ui.label(
                    egui::RichText::new(i18n.t_with("online-playing-against", &[("name", &name)]))
                        .font(strong_font(ui.ctx(), 14.0))
                        .color(p.ink),
                );
                let color = i18n.t(match row.color {
                    Color::White => "online-color-white",
                    Color::Black => "online-color-black",
                });
                let mut meta = vec![color];
                if let Some(days) = row.days {
                    meta.push(days_label(i18n, u8::try_from(days).unwrap_or(u8::MAX)));
                }
                if let Some(left) = row.seconds_left {
                    meta.push(
                        i18n.t_with("online-playing-left", &[("time", &left_label(i18n, left))]),
                    );
                }
                muted(ui, meta.join(" · "));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(CsButton::new(&i18n.t("online-playing-open")))
                    .clicked()
                {
                    intents.push(Intent::Open(row.remote_id.clone()));
                }
                if row.my_turn {
                    cs_status_badge(
                        ui,
                        StatusTone::Accent,
                        icon::HAND_POINTING,
                        &i18n.t("online-playing-your-turn"),
                    );
                } else {
                    cs_status_badge(
                        ui,
                        StatusTone::Neutral,
                        icon::HOURGLASS,
                        &i18n.t("online-playing-their-turn"),
                    );
                }
            });
        });
    });
}

// --- Painel Partida online -----------------------------------------------------------------

/// O que o Painel mostra da Partida online do tabuleiro.
#[derive(Debug, Clone)]
pub struct PanelView {
    pub link: Option<LinkStatus>,
    pub now: Timestamp,
    pub opponent_gone: bool,
    pub can_claim: bool,
    pub incoming: usize,
    pub stored: usize,
}

impl PanelView {
    fn of(play: &OnlinePlay, session: &Session, now: Timestamp) -> Self {
        let live = session.online_remote_id().and_then(|id| play.live(id));
        Self {
            link: live.map(super::service::Live::link),
            now,
            opponent_gone: live.is_some_and(super::service::Live::opponent_gone),
            can_claim: live.is_some_and(|l| l.can_claim_victory(now)),
            incoming: play.incoming.len(),
            stored: play.stored_in_progress(),
        }
    }
}

/// O que o Painel pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelIntent {
    Board(OnlineIntent),
    ConfirmResign,
    Continue,
    NewOnline,
    List,
    Analyze,
    Library,
}

pub fn online_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let Some(net) = world.get_resource::<NetService>() else {
        return;
    };
    let now = net.clock().now();
    let demo = world
        .get_resource::<OnlineUi>()
        .and_then(|u| u.demo.as_ref().map(|d| d.panel_view(now)));
    let view = match demo {
        Some(view) => view,
        None => {
            let Some(service) = world.get_resource::<OnlineService>() else {
                return;
            };
            let session = world.resource::<Session>();
            service.with(|play| PanelView::of(play, session, now))
        }
    };
    let notices: Vec<OnlineNotice> = world
        .get_resource::<OnlineUi>()
        .map(|u| u.recent().cloned().collect())
        .unwrap_or_default();
    let mut intents = Vec::new();
    {
        let session = world.resource::<Session>();
        panel_body(ui, i18n, session, &view, &notices, &mut intents);
    }
    for intent in intents {
        apply_panel_intent(world, intent);
    }
}

fn apply_panel_intent(world: &mut World, intent: PanelIntent) {
    match intent {
        PanelIntent::Board(intent) => world.resource_mut::<Session>().online_request(intent),
        PanelIntent::ConfirmResign => world.resource_mut::<OnlineUi>().confirm_resign = true,
        PanelIntent::Continue => {
            let remote = world
                .resource::<Session>()
                .online_remote_id()
                .map(str::to_owned);
            if let Some(remote) = remote {
                world.resource_scope(|world, net: Mut<NetService>| {
                    let error = world
                        .resource::<OnlineService>()
                        .with(|play| play.open(&net, &remote).err());
                    if let Some(error) = error {
                        let mut ui = world.resource_mut::<OnlineUi>();
                        ui.error = Some(error);
                        ui.open_at(Tab::Playing);
                    }
                });
            }
        }
        PanelIntent::NewOnline => world.resource_mut::<OnlineUi>().open_at(Tab::Seek),
        PanelIntent::List => world.resource_mut::<OnlineUi>().open_at(Tab::Playing),
        PanelIntent::Analyze => {
            world
                .resource_mut::<crate::analysis::Analysis>()
                .set_enabled(true);
            if let Some(mut dock) = world.get_resource_mut::<crate::dock::Dock>() {
                dock.layout.set_visible(crate::dock::ids::ANALYSIS, true);
                dock.layout.activate(crate::dock::ids::ANALYSIS);
            }
        }
        PanelIntent::Library => {
            world.write_message(ActionRequested(Action::OpenLibrary));
        }
    }
}

fn link_badge(
    ui: &mut egui::Ui,
    i18n: &I18n,
    link: Option<LinkStatus>,
    over: bool,
    now: Timestamp,
) {
    let (tone, glyph, text) = match (over, link) {
        (true, _) => (
            StatusTone::Neutral,
            icon::FLAG_CHECKERED,
            i18n.t("online-badge-ended"),
        ),
        (false, None) => (
            StatusTone::Neutral,
            icon::PLUGS,
            i18n.t("online-badge-not-open"),
        ),
        (false, Some(LinkStatus::Live)) => (
            StatusTone::Success,
            icon::CELL_SIGNAL_FULL,
            i18n.t("online-badge-live"),
        ),
        (false, Some(LinkStatus::Connecting)) => (
            StatusTone::Info,
            icon::CELL_SIGNAL_LOW,
            i18n.t("online-badge-connecting"),
        ),
        (false, Some(LinkStatus::Retrying { at, .. })) => {
            let wait = at.since(now);
            let text = if wait >= Duration::from_secs(1) {
                i18n.t_with(
                    "online-badge-reconnecting-in",
                    &[("time", &format!("{} s", wait.as_secs()))],
                )
            } else {
                i18n.t("online-badge-reconnecting")
            };
            (StatusTone::Warning, icon::CELL_SIGNAL_SLASH, text)
        }
        (false, Some(LinkStatus::Closed)) => (
            StatusTone::Danger,
            icon::CELL_SIGNAL_SLASH,
            i18n.t("online-badge-offline"),
        ),
    };
    cs_status_badge(ui, tone, glyph, &text);
}

fn player_line(name: &str, rating: Option<u32>) -> String {
    match rating {
        Some(r) => format!("{name} ({r})"),
        None => name.to_owned(),
    }
}

/// O fim como o Provedor o informou: "Brancas venceram por abandono."
fn ended_text(i18n: &I18n, session: &Session) -> String {
    let snapshot = session.snapshot();
    let Some(outcome) = snapshot.outcome else {
        return i18n.t("online-ended-aborted");
    };
    let result = i18n.t(crate::ui::result_key(outcome.result));
    let reason = i18n.t(crate::ui::reason_key(outcome.reason));
    format!("{result} · {reason}")
}

fn panel_body(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &Session,
    view: &PanelView,
    notices: &[OnlineNotice],
    intents: &mut Vec<PanelIntent>,
) {
    let p = palette(ui);
    let Some(online) = session.game().setup().online_game() else {
        // Nenhuma Partida online no tabuleiro: o caminho para começar e o que há pendente.
        muted(ui, i18n.t("online-panel-none"));
        ui.add_space(8.0);
        if ui
            .add(CsButton::new(&i18n.t("online-panel-new")).icon(icon::GLOBE))
            .clicked()
        {
            intents.push(PanelIntent::NewOnline);
        }
        if view.incoming > 0 || view.stored > 0 {
            ui.add_space(8.0);
            if view.incoming > 0 {
                let text = if view.incoming == 1 {
                    i18n.t("online-panel-incoming-one")
                } else {
                    i18n.t_with(
                        "online-panel-incoming-other",
                        &[("count", &view.incoming.to_string())],
                    )
                };
                icon_line(ui, icon::SWORD, p.accent, &text);
            }
            if view.stored > 0 {
                let text = if view.stored == 1 {
                    i18n.t("online-panel-stored-one")
                } else {
                    i18n.t_with(
                        "online-panel-stored-other",
                        &[("count", &view.stored.to_string())],
                    )
                };
                icon_line(ui, icon::LIST_BULLETS, p.ink_muted, &text);
            }
            ui.add_space(4.0);
            if ui
                .add(CsButton::new(&i18n.t("online-panel-see-list")).variant(ButtonVariant::Subtle))
                .clicked()
            {
                intents.push(PanelIntent::List);
            }
        }
        recent_notices(ui, i18n, notices);
        return;
    };
    let snapshot = session.snapshot();
    let over = snapshot.is_over();
    let human = online.opponent == Opponent::Human;
    if over {
        // Acabou: o resultado vem primeiro (o grupo do dock costuma ser baixo).
        ended_section(ui, i18n, session, intents);
        ui.add_space(12.0);
        players_block(ui, i18n, session, online);
        recent_notices(ui, i18n, notices);
        return;
    }
    // Selos: Provedor e Conexão.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        cs_status_badge(
            ui,
            StatusTone::Neutral,
            icon::GLOBE,
            &i18n.t("online-badge-lichess"),
        );
        link_badge(ui, i18n, view.link, over, view.now);
    });
    ui.add_space(8.0);
    let opponent = match online.local_color {
        Color::White => &online.black,
        Color::Black => &online.white,
    };
    // O que pede resposta vem primeiro: proposta de empate e adversário ausente.
    if let Some(by) = snapshot.pending_draw_offer {
        if by == online.local_color {
            muted(ui, i18n.t("online-draw-offered-by-you"));
        } else {
            let text = i18n.t_with("online-draw-offered-by-them", &[("name", &opponent.name)]);
            notice_line(ui, StatusTone::Info, &text);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui
                    .add(CsButton::new(&i18n.t("online-accept")).variant(ButtonVariant::Primary))
                    .clicked()
                {
                    intents.push(PanelIntent::Board(OnlineIntent::AcceptDraw));
                }
                if ui.add(CsButton::new(&i18n.t("online-decline"))).clicked() {
                    intents.push(PanelIntent::Board(OnlineIntent::DeclineDraw));
                }
            });
        }
    }
    // Adversário ausente.
    if view.opponent_gone {
        notice_line(ui, StatusTone::Warning, &i18n.t("online-opponent-gone"));
        if ui
            .add(CsButton::new(&i18n.t("online-claim-victory")).enabled(view.can_claim))
            .clicked()
        {
            intents.push(PanelIntent::Board(OnlineIntent::ClaimVictory));
        }
    }
    if snapshot.pending_draw_offer.is_some() || view.opponent_gone {
        ui.add_space(8.0);
    }
    // Estado da vez.
    let online_view = snapshot.online.as_ref();
    if let Some(sent) = online_view.and_then(|o| o.sent_move.as_ref()) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add(egui::Spinner::new().size(16.0).color(p.info));
            ui.label(
                egui::RichText::new(i18n.t_with("online-status-sending", &[("san", &sent.san)]))
                    .color(p.ink),
            );
        });
    } else if online_view.is_some_and(|o| o.desync.is_some()) {
        notice_line(ui, StatusTone::Warning, &i18n.t("online-status-desync"));
    } else if view.link.is_none() {
        notice_line(ui, StatusTone::Info, &i18n.t("online-status-not-open"));
        ui.add_space(4.0);
        if ui
            .add(
                CsButton::new(&i18n.t("online-continue"))
                    .icon(icon::ARROWS_CLOCKWISE)
                    .variant(ButtonVariant::Primary),
            )
            .clicked()
        {
            intents.push(PanelIntent::Continue);
        }
    } else if matches!(view.link, Some(LinkStatus::Retrying { .. })) {
        notice_line(
            ui,
            StatusTone::Warning,
            &i18n.t("online-status-reconnecting"),
        );
    } else {
        let mine = snapshot.side_to_move == online.local_color;
        let (glyph, tint, key) = if mine {
            (icon::HAND_POINTING, p.accent, "online-status-your-turn")
        } else {
            (icon::HOURGLASS, p.ink_muted, "online-status-their-turn")
        };
        icon_line(ui, glyph, tint, &i18n.t(key));
    }
    // Ações da Partida.
    let live = view.link.is_some_and(|l| l != LinkStatus::Closed);
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let early = snapshot.history.len() < 2;
        if early {
            let abort = ui
                .add(
                    CsButton::new(&i18n.t("online-abort"))
                        .icon(icon::X_CIRCLE)
                        .enabled(live),
                )
                .on_hover_text(i18n.t("online-abort-tip"));
            if abort.clicked() {
                intents.push(PanelIntent::Board(OnlineIntent::Abort));
            }
        }
        let can_offer = live && snapshot.pending_draw_offer.is_none();
        if ui
            .add(
                CsButton::new(&i18n.t("online-offer-draw"))
                    .icon(icon::HANDSHAKE)
                    .enabled(can_offer),
            )
            .clicked()
        {
            intents.push(PanelIntent::Board(OnlineIntent::OfferDraw));
        }
        if ui
            .add(
                CsButton::new(&i18n.t("online-resign"))
                    .icon(icon::FLAG)
                    .variant(ButtonVariant::Danger)
                    .enabled(live),
            )
            .clicked()
        {
            intents.push(PanelIntent::ConfirmResign);
        }
    });
    recent_notices(ui, i18n, notices);
    ui.add_space(12.0);
    players_block(ui, i18n, session, online);
    ui.add_space(8.0);
    if human {
        icon_line(
            ui,
            icon::SHIELD_CHECK,
            p.accent,
            &i18n.t("online-fairplay-on"),
        );
    } else {
        icon_line(
            ui,
            icon::INFO,
            p.info,
            &i18n.t("online-fairplay-off-engine"),
        );
    }
}

/// Contra quem, de que cor e em que ritmo (os nomes e os relógios estão no Painel Relógio).
fn players_block(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &Session,
    online: &chess_game::OnlineGame,
) {
    let p = palette(ui);
    let opponent = match online.local_color {
        Color::White => &online.black,
        Color::Black => &online.white,
    };
    let title = match online.opponent {
        Opponent::ProviderEngine { level } => {
            i18n.t_with("online-vs-engine", &[("level", &level.to_string())])
        }
        Opponent::Human => i18n.t_with(
            "online-vs",
            &[("name", &player_line(&opponent.name, opponent.rating))],
        ),
    };
    ui.label(
        egui::RichText::new(title)
            .font(strong_font(ui.ctx(), 16.0))
            .color(p.ink),
    );
    let color = i18n.t(match online.local_color {
        Color::White => "online-color-white-lower",
        Color::Black => "online-color-black-lower",
    });
    let mut meta = vec![i18n.t_with("online-you-play", &[("color", &color)])];
    if let Some(days) = online.days_per_move {
        meta.push(days_label(i18n, u8::try_from(days).unwrap_or(u8::MAX)));
    } else if let Some(tc) = session.game().setup().time_control() {
        meta.push(time_label(
            i18n,
            OnlineTime::RealTime {
                base_s: u32::try_from(tc.base_ms / 1000).unwrap_or(u32::MAX),
                increment_s: u32::try_from(tc.increment_ms / 1000).unwrap_or(u32::MAX),
            },
        ));
    }
    meta.push(rated_label(i18n, online.rated));
    muted(ui, meta.join(" · "));
}

fn ended_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &Session,
    intents: &mut Vec<PanelIntent>,
) {
    let p = palette(ui);
    // O resultado primeiro: é o que importa quando a Partida acaba (e cabe num grupo baixo).
    ui.add(
        egui::Label::new(
            egui::RichText::new(ended_text(i18n, session))
                .font(strong_font(ui.ctx(), 16.0))
                .color(p.ink),
        )
        .wrap(),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        if ui
            .add(
                CsButton::new(&i18n.t("online-analyze"))
                    .icon(icon::CHART_LINE)
                    .variant(ButtonVariant::Primary),
            )
            .clicked()
        {
            intents.push(PanelIntent::Analyze);
        }
        if ui
            .add(CsButton::new(&i18n.t("online-open-library")).icon(icon::BOOKS))
            .clicked()
        {
            intents.push(PanelIntent::Library);
        }
    });
    ui.add_space(4.0);
    muted(ui, i18n.t("online-ended-saved"));
}

fn recent_notices(ui: &mut egui::Ui, i18n: &I18n, notices: &[OnlineNotice]) {
    let texts: Vec<(StatusTone, String)> = notices
        .iter()
        .filter_map(|n| notice_text(i18n, n))
        .collect();
    if texts.is_empty() {
        return;
    }
    ui.add_space(12.0);
    for (tone, text) in texts.iter().rev().take(2) {
        notice_line(ui, *tone, text);
        ui.add_space(4.0);
    }
}

// --- Confirmação de abandono ---------------------------------------------------------------

pub fn resign_dialog(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut ui_state: ResMut<OnlineUi>,
    mut session: ResMut<Session>,
) -> Result {
    if !ui_state.confirm_resign {
        return Ok(());
    }
    if !session.is_online() || session.snapshot().is_over() {
        ui_state.confirm_resign = false;
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    egui::Area::new(egui::Id::new("online-resign-dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(&ctx, |ui| {
            let rect = ctx.viewport_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
        });
    let mut answer = None;
    egui::Window::new(i18n.t("online-resign-title"))
        .id(egui::Id::new("online-resign"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.set_max_width(360.0);
            ui.add(egui::Label::new(i18n.t("online-resign-text")).wrap());
            footer(ui, |ui| {
                if ui
                    .add(
                        CsButton::new(&i18n.t("online-resign-confirm"))
                            .variant(ButtonVariant::Danger),
                    )
                    .clicked()
                {
                    answer = Some(true);
                }
                if ui
                    .add(
                        CsButton::new(&i18n.t("online-resign-keep"))
                            .variant(ButtonVariant::Primary),
                    )
                    .clicked()
                {
                    answer = Some(false);
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(false);
    }
    if let Some(resign) = answer {
        ui_state.confirm_resign = false;
        if resign && ui_state.demo.is_none() {
            session.online_request(OnlineIntent::Resign);
        }
    }
    Ok(())
}

// --- Aviso discreto: desafio recebido, oferta de atualizar ---------------------------------

#[allow(clippy::too_many_arguments)]
pub fn toasts(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut ui_state: ResMut<OnlineUi>,
    service: Res<OnlineService>,
    net: Option<Res<NetService>>,
    session: Res<Session>,
    dock: Option<Res<crate::dock::Dock>>,
) -> Result {
    let Some(net) = net else {
        return Ok(());
    };
    // Nunca durante outra Partida competitiva (local ou online em andamento).
    let busy = session.competitive_in_progress() || net.fair_play().everything_locked();
    let (challenge, offer) = match ui_state.demo.as_ref() {
        Some(scene) => (scene.toast_challenge(), scene.offer()),
        None => service.with(|play| {
            let challenge = play
                .incoming
                .iter()
                .find(|c| !ui_state.seen_challenges.contains(&c.id))
                .cloned();
            let offer = (!ui_state.offer_answered
                && play.connected()
                && !play.listening()
                && play.stored_in_progress() > 0)
                .then(|| play.stored_in_progress());
            (challenge, offer)
        }),
    };
    if busy || ui_state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let top_right = dock
        .as_deref()
        .and_then(crate::dock::Dock::board_area)
        .map_or(egui::pos2(ctx.viewport_rect().right() - 16.0, 140.0), |r| {
            egui::pos2(r.right() - 16.0, r.top() + 16.0)
        });
    let mut see = false;
    let mut dismiss = false;
    let mut refresh = false;
    let mut later = false;
    if let Some(challenge) = &challenge {
        egui::Area::new(egui::Id::new("online-toast-challenge"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(top_right)
            .show(&ctx, |ui| {
                let p = palette(ui);
                cs_card_frame(p, CardEmphasis::Full)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: p.scrim,
                    })
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        let name =
                            player_line(&challenge.challenger.name, challenge.challenger.rating);
                        icon_line(
                            ui,
                            icon::SWORD,
                            p.accent,
                            &i18n.t_with("online-toast-challenge", &[("name", &name)]),
                        );
                        muted(
                            ui,
                            format!(
                                "{} · {}",
                                time_label(&i18n, challenge.time),
                                rated_label(&i18n, challenge.rated)
                            ),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("online-toast-see"))
                                        .variant(ButtonVariant::Primary),
                                )
                                .clicked()
                            {
                                see = true;
                            }
                            if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
                                dismiss = true;
                            }
                        });
                    });
            });
    } else if let Some(count) = offer {
        egui::Area::new(egui::Id::new("online-toast-offer"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(top_right)
            .show(&ctx, |ui| {
                let p = palette(ui);
                cs_card_frame(p, CardEmphasis::Full)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: p.scrim,
                    })
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.label(
                            egui::RichText::new(i18n.t("online-offer-title"))
                                .font(strong_font(ui.ctx(), 14.0))
                                .color(p.ink),
                        );
                        let text = if count == 1 {
                            i18n.t("online-offer-text-one")
                        } else {
                            i18n.t_with("online-offer-text-other", &[("count", &count.to_string())])
                        };
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).color(p.ink_muted)).wrap(),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("online-offer-refresh"))
                                        .variant(ButtonVariant::Primary),
                                )
                                .clicked()
                            {
                                refresh = true;
                            }
                            if ui
                                .add(CsButton::new(&i18n.t("online-offer-later")))
                                .clicked()
                            {
                                later = true;
                            }
                        });
                    });
            });
    }
    if ui_state.demo.is_some() {
        return Ok(());
    }
    if let Some(challenge) = challenge
        && (see || dismiss)
    {
        ui_state.seen_challenges.push(challenge.id);
        if see {
            ui_state.open_at(Tab::Challenge);
        }
    }
    if refresh || later {
        ui_state.offer_answered = true;
    }
    if refresh {
        let error = service.with(|play| play.refresh_playing(&net).err());
        ui_state.open_at(Tab::Playing);
        ui_state.error = error;
    }
    Ok(())
}
