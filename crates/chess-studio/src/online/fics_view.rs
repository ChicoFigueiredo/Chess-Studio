//! A tela do FICS como visitante (#254), dentro da tela da Partida online: o aviso honesto
//! (conexão sem criptografia, só visitante, sem senha, sem reconexão), entrar e sair, buscar
//! adversário, os desafios recebidos e a observação (um tabuleiro pequeno, só na memória). O
//! Painel Partida online é o mesmo do Lichess ([`super::view::online_panel`]). Só widgets e
//! tokens do design system; textos em Fluent. Nada aqui escreve a Partida.

use super::fics::{FicsInfo, FicsSeek};
use super::service::{OnlinePlay, Unavailable};
use super::view::{elapsed_label, footer, icon_line, muted, notice_line, section, state_block};
use super::{OnlineService, OnlineUi};
use crate::actions::ActionRequested;
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::provider_consent::ConsentDialog;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, StatusTone, cs_card_frame, cs_segmented, cs_text_field,
    palette, strong_font, tabular_text,
};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use chess_input::controls::Action;
use chess_net::{ProviderId, Timestamp};
use chess_providers::{
    Fics, FicsChallenge, FicsError, FicsOutgoing, FicsStatus, ObservedGame, ProviderError,
};
use egui_phosphor::regular as icon;
use std::time::Duration;

/// Aba da tela do FICS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FicsTab {
    #[default]
    Seek,
    Challenges,
    Observe,
}

/// Ritmos de busca no FICS (minutos e incremento em segundos), pela categoria que o FICS dá
/// (duração estimada: base + 2/3 do incremento; relâmpago abaixo de 3 min, padrão a partir
/// de 15).
pub const PACES: [(&str, &[(u32, u32)]); 3] = [
    ("fics-pace-lightning", &[(1, 0), (2, 1)]),
    (
        "fics-pace-blitz",
        &[(3, 0), (3, 2), (5, 0), (5, 3), (10, 0)],
    ),
    ("fics-pace-standard", &[(15, 0), (15, 10), (30, 0)]),
];

/// O que o jogador preenche na tela do FICS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicsForm {
    /// Minutos e incremento da busca.
    pub pace: (u32, u32),
    /// Número da Partida a observar.
    pub observe: String,
    /// Nome de quem desafiar (`match`).
    pub name: String,
}

impl Default for FicsForm {
    fn default() -> Self {
        Self {
            pace: (5, 3),
            observe: String::new(),
            name: String::new(),
        }
    }
}

impl FicsForm {
    /// O número digitado, se for um número de Partida válido.
    pub fn observe_number(&self) -> Option<u32> {
        self.observe.trim().parse().ok().filter(|n| *n > 0)
    }

    /// O nome digitado, se for um nome do FICS (só letras: nada de variante na linha).
    pub fn challenge_name(&self) -> Option<&str> {
        let name = self.name.trim();
        Fics::valid_handle(name).then_some(name)
    }
}

/// "5+3".
pub fn pace_text(minutes: u32, increment_s: u32) -> String {
    format!("{minutes}+{increment_s}")
}

/// Um retrato da conexão para desenhar sem segurar o `Mutex`.
#[derive(Debug, Clone)]
pub struct FicsView {
    /// O FICS está ligado no Perfil local.
    pub enabled: bool,
    pub status: Option<FicsStatus>,
    pub handle: Option<String>,
    pub closed: Option<FicsError>,
    /// A conexão caiu com a Partida em andamento (a sessão segura a trava até o descarte).
    pub dropped: bool,
    pub seek: Option<FicsSeek>,
    pub challenges: Vec<FicsChallenge>,
    /// O desafio por nome enviado, até a resposta.
    pub outgoing: Option<FicsOutgoing>,
    pub observe_requested: Option<u32>,
    pub observing: bool,
    pub observed: Option<ObservedGame>,
    /// Há Partida jogada em andamento.
    pub in_game: bool,
    pub now: Timestamp,
}

impl FicsView {
    pub fn of(play: &OnlinePlay, net: &NetService) -> Self {
        let fics = &play.fics;
        Self {
            enabled: net.is_enabled(ProviderId::Fics),
            status: fics.status(),
            handle: fics.handle().map(str::to_owned),
            closed: fics.closed_reason().cloned(),
            dropped: fics.dropped_mid_game(),
            seek: fics.seek_state(),
            challenges: fics.challenges().to_vec(),
            outgoing: fics.outgoing().cloned(),
            observe_requested: fics.observe_requested(),
            observing: fics.observing(),
            observed: fics.observed(),
            in_game: fics.game_in_progress(),
            now: net.clock().now(),
        }
    }
}

/// O que a tela do FICS pediu neste quadro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FicsIntent {
    Close,
    /// Abre o diálogo "Ligar FICS?" (consentimento, com o que será enviado).
    TurnOn,
    OpenCenter,
    Connect,
    /// Sair do FICS (com Partida em andamento, pede confirmação antes).
    Leave,
    /// Confirma a saída com a Partida em andamento: o descarte explícito.
    ConfirmLeave,
    KeepPlaying,
    Seek,
    Unseek,
    Accept(String),
    Decline(String),
    /// Desafia pelo nome digitado, no ritmo escolhido.
    Challenge,
    /// Retira o desafio por nome enviado.
    Withdraw,
    Observe,
    Unobserve,
    /// Encerra no app, sem resultado, a Partida que caiu (FC-22).
    CloseUnconfirmed,
}

// --- Textos -----------------------------------------------------------------------------------

/// Um erro do FICS em linguagem de usuário.
pub fn error_text(i18n: &I18n, error: &FicsError) -> String {
    match error {
        FicsError::InsecureChannel => i18n.t("fics-error-insecure"),
        FicsError::PasswordRequested => i18n.t("fics-error-password"),
        FicsError::LoginTimeout => i18n.t("fics-error-login-timeout"),
        FicsError::ConnectionClosed {
            game_in_progress: true,
        } => i18n.t("fics-dropped-text"),
        FicsError::ConnectionClosed {
            game_in_progress: false,
        } => i18n.t("fics-error-closed"),
        FicsError::UnsupportedVariant(variant) => {
            i18n.t_with("fics-error-variant", &[("variant", variant)])
        }
        FicsError::Desynced => i18n.t("fics-error-desync"),
        FicsError::NotAllowed(why) => i18n.t_with("fics-error-not-allowed", &[("why", why)]),
        FicsError::Rejected(_) => i18n.t("fics-error-rejected"),
        FicsError::Provider(ProviderError::Disabled { .. }) => i18n.t("fics-error-disabled"),
        FicsError::Provider(ProviderError::FairPlayLocked { .. }) => i18n.t("fics-error-fair-play"),
        FicsError::Provider(ProviderError::Net(
            chess_net::NetError::Unreachable { .. } | chess_net::NetError::Timeout { .. },
        )) => i18n.t("fics-error-offline"),
        _ => i18n.t("fics-error-generic"),
    }
}

/// Por que um pedido ao FICS não pôde nem começar.
pub fn unavailable_text(i18n: &I18n, error: &Unavailable) -> String {
    match error {
        Unavailable::ProviderOff => i18n.t("fics-error-disabled"),
        Unavailable::Fics(error) => error_text(i18n, error),
        Unavailable::Store(error) => i18n.t_with("online-notice-store", &[("error", error)]),
        Unavailable::NoConnection | Unavailable::Board(_) | Unavailable::GameAccount { .. } => {
            i18n.t("fics-error-generic")
        }
    }
}

/// O texto de um aviso do FICS.
pub fn info_text(i18n: &I18n, info: &FicsInfo) -> (StatusTone, String) {
    let mv = |mv: &chess_core::Move| mv.to_string();
    match info {
        FicsInfo::LoggedIn { handle } => (
            StatusTone::Success,
            i18n.t_with("fics-notice-logged-in", &[("handle", handle)]),
        ),
        FicsInfo::SeekPosted => (StatusTone::Info, i18n.t("fics-notice-seek-posted")),
        FicsInfo::Challenge { name } => (
            StatusTone::Info,
            i18n.t_with("fics-notice-challenge", &[("name", name)]),
        ),
        FicsInfo::ChallengeIssued { to } => (
            StatusTone::Info,
            i18n.t_with("fics-notice-challenge-issued", &[("name", to)]),
        ),
        FicsInfo::ChallengeDeclined { by } => (
            StatusTone::Warning,
            i18n.t_with("fics-notice-challenge-declined", &[("name", by)]),
        ),
        FicsInfo::ChallengeFailed { name } => (
            StatusTone::Warning,
            i18n.t_with("fics-notice-challenge-failed", &[("name", name)]),
        ),
        FicsInfo::MoveRefused { mv: m } => (
            StatusTone::Warning,
            i18n.t_with("fics-notice-move-refused", &[("mv", &mv(m))]),
        ),
        FicsInfo::MoveLost { mv: m } => (
            StatusTone::Warning,
            i18n.t_with("fics-notice-move-lost", &[("mv", &mv(m))]),
        ),
        FicsInfo::MoveReplaced { mv: m } => (
            StatusTone::Info,
            i18n.t_with("fics-notice-move-replaced", &[("mv", &mv(m))]),
        ),
        FicsInfo::VariantRefused { variant } => (
            StatusTone::Info,
            i18n.t_with("fics-notice-variant", &[("variant", variant)]),
        ),
        FicsInfo::Desynced => (StatusTone::Warning, i18n.t("fics-notice-desync")),
        FicsInfo::Resynced => (StatusTone::Success, i18n.t("fics-notice-resync")),
        FicsInfo::AbortRequested => (StatusTone::Info, i18n.t("fics-notice-abort-requested")),
        FicsInfo::ObservationStarted { game } => (
            StatusTone::Info,
            i18n.t_with(
                "fics-notice-observe-started",
                &[("game", &game.to_string())],
            ),
        ),
        FicsInfo::ObservationEnded { game } => (
            StatusTone::Info,
            i18n.t_with("fics-notice-observe-ended", &[("game", &game.to_string())]),
        ),
        FicsInfo::Closed(error @ FicsError::ConnectionClosed { .. }) => {
            (StatusTone::Danger, error_text(i18n, error))
        }
        FicsInfo::Closed(error) | FicsInfo::Refused(error) => {
            (StatusTone::Warning, error_text(i18n, error))
        }
    }
}

/// O aviso honesto do FICS, linha a linha (diálogo e cartão da Central): sem criptografia, só
/// visitante e sem senha, sem reconexão, sem timeseal.
pub const WARNINGS: [(&str, &str); 4] = [
    (icon::LOCK_SIMPLE_OPEN, "fics-warning-plain"),
    (icon::USER_CIRCLE, "fics-warning-guest"),
    (icon::PLUGS, "fics-warning-drop"),
    (icon::TIMER, "fics-warning-lag"),
];

/// Desenha o aviso: a primeira linha (sem criptografia) em tom de atenção.
pub fn warning_lines(ui: &mut egui::Ui, i18n: &I18n) {
    let p = palette(ui);
    for (index, (glyph, key)) in WARNINGS.iter().enumerate() {
        let tint = if index == 0 { p.warning } else { p.ink_muted };
        icon_line(ui, glyph, tint, &i18n.t(key));
        ui.add_space(2.0);
    }
}

// --- A tela -----------------------------------------------------------------------------------

/// O corpo da tela quando o Provedor escolhido é o FICS.
pub fn body(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    if !view.enabled {
        state_block(
            ui,
            Some(icon::PLUGS),
            &i18n.t("fics-off-title"),
            &i18n.t("fics-off-text"),
        );
        warning_lines(ui, i18n);
        footer(ui, |ui| {
            if ui
                .add(CsButton::new(&i18n.t("fics-turn-on")).variant(ButtonVariant::Primary))
                .clicked()
            {
                intents.push(FicsIntent::TurnOn);
            }
            if ui
                .add(CsButton::new(&i18n.t("online-open-center")))
                .clicked()
            {
                intents.push(FicsIntent::OpenCenter);
            }
            if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
                intents.push(FicsIntent::Close);
            }
        });
        return;
    }
    match view.status {
        None => guest_intro(ui, i18n, intents),
        Some(FicsStatus::Connecting | FicsStatus::LoggingIn) => {
            state_block(
                ui,
                None,
                &i18n.t("fics-connecting"),
                &i18n.t("fics-connecting-text"),
            );
            footer(ui, |ui| {
                if ui.add(CsButton::new(&i18n.t("online-cancel"))).clicked() {
                    intents.push(FicsIntent::Leave);
                }
            });
        }
        Some(FicsStatus::Closed) => closed(ui, i18n, view, intents),
        Some(FicsStatus::Ready) => ready(ui, i18n, state, view, intents),
    }
}

fn guest_intro(ui: &mut egui::Ui, i18n: &I18n, intents: &mut Vec<FicsIntent>) {
    section(ui, &i18n.t("fics-guest-title"));
    ui.add_space(4.0);
    muted(ui, i18n.t("fics-guest-text"));
    ui.add_space(12.0);
    warning_lines(ui, i18n);
    footer(ui, |ui| {
        if ui
            .add(
                CsButton::new(&i18n.t("fics-connect"))
                    .icon(icon::SIGN_IN)
                    .variant(ButtonVariant::Primary),
            )
            .clicked()
        {
            intents.push(FicsIntent::Connect);
        }
        if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
            intents.push(FicsIntent::Close);
        }
    });
}

fn closed(ui: &mut egui::Ui, i18n: &I18n, view: &FicsView, intents: &mut Vec<FicsIntent>) {
    if view.dropped {
        // Caiu durante a Partida: o FICS a dá como perdida; nada reconecta sozinho.
        state_block(
            ui,
            Some(icon::CELL_SIGNAL_SLASH),
            &i18n.t("fics-dropped-title"),
            "",
        );
        notice_line(ui, StatusTone::Danger, &i18n.t("fics-dropped-text"));
        ui.add_space(4.0);
        muted(ui, i18n.t("fics-dropped-lock"));
        ui.add_space(4.0);
        muted(ui, i18n.t("fics-close-unconfirmed-text"));
        footer(ui, |ui| {
            if ui
                .add(
                    CsButton::new(&i18n.t("fics-close-unconfirmed"))
                        .variant(ButtonVariant::Primary),
                )
                .clicked()
            {
                intents.push(FicsIntent::CloseUnconfirmed);
            }
            if ui
                .add(CsButton::new(&i18n.t("fics-discard")).variant(ButtonVariant::Subtle))
                .clicked()
            {
                intents.push(FicsIntent::ConfirmLeave);
            }
            if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
                intents.push(FicsIntent::Close);
            }
        });
        return;
    }
    let reason = view
        .closed
        .as_ref()
        .map(|e| error_text(i18n, e))
        .unwrap_or_default();
    state_block(ui, Some(icon::PLUGS), &i18n.t("fics-closed-title"), &reason);
    warning_lines(ui, i18n);
    footer(ui, |ui| {
        if ui
            .add(
                CsButton::new(&i18n.t("fics-connect-again"))
                    .icon(icon::SIGN_IN)
                    .variant(ButtonVariant::Primary),
            )
            .clicked()
        {
            intents.push(FicsIntent::Connect);
        }
        if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
            intents.push(FicsIntent::Close);
        }
    });
}

fn ready(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        let handle = view.handle.as_deref().unwrap_or("?");
        ui.label(
            egui::RichText::new(icon::USER_CIRCLE)
                .font(crate::ui_theme::icon_font(ui.ctx(), 16.0))
                .color(p.accent),
        );
        ui.label(
            egui::RichText::new(i18n.t_with("fics-logged-in", &[("handle", handle)]))
                .font(strong_font(ui.ctx(), 14.0))
                .color(p.ink),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !state.fics_confirm_leave
                && ui
                    .add(
                        CsButton::new(&i18n.t("fics-leave"))
                            .icon(icon::SIGN_OUT)
                            .variant(ButtonVariant::Subtle),
                    )
                    .clicked()
            {
                intents.push(FicsIntent::Leave);
            }
        });
    });
    muted(ui, i18n.t("fics-ready-note"));
    if state.fics_confirm_leave {
        ui.add_space(8.0);
        notice_line(ui, StatusTone::Danger, &i18n.t("fics-leave-warning"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ui
                .add(CsButton::new(&i18n.t("fics-leave-confirm")).variant(ButtonVariant::Danger))
                .clicked()
            {
                intents.push(FicsIntent::ConfirmLeave);
            }
            if ui
                .add(CsButton::new(&i18n.t("online-resign-keep")).variant(ButtonVariant::Primary))
                .clicked()
            {
                intents.push(FicsIntent::KeepPlaying);
            }
        });
    }
    ui.add_space(12.0);
    if view.in_game {
        icon_line(ui, icon::CHECKERBOARD, p.accent, &i18n.t("fics-in-game"));
        footer(ui, |ui| {
            if ui
                .add(CsButton::new(&i18n.t("online-close")).variant(ButtonVariant::Primary))
                .clicked()
            {
                intents.push(FicsIntent::Close);
            }
        });
        return;
    }
    let incoming = if view.challenges.is_empty() {
        i18n.t("fics-tab-challenges")
    } else {
        i18n.t_with(
            "fics-tab-challenges-count",
            &[("count", &view.challenges.len().to_string())],
        )
    };
    let tabs = [
        (FicsTab::Seek, i18n.t("fics-tab-seek")),
        (FicsTab::Challenges, incoming),
        (FicsTab::Observe, i18n.t("fics-tab-observe")),
    ];
    cs_segmented(ui, &mut state.fics_tab, &tabs);
    ui.add_space(16.0);
    match state.fics_tab {
        FicsTab::Seek => seek_tab(ui, i18n, state, view, intents),
        FicsTab::Challenges => challenges_tab(ui, i18n, state, view, intents),
        FicsTab::Observe => observe_tab(ui, i18n, state, view, intents),
    }
}

fn seek_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    if let Some(seek) = view.seek {
        let p = palette(ui);
        let detail = i18n.t_with(
            "fics-seek-detail",
            &[("pace", &pace_text(seek.minutes, seek.increment_s))],
        );
        state_block(ui, None, &i18n.t("fics-seek-waiting"), &detail);
        ui.vertical_centered(|ui| {
            let elapsed = view.now.since(seek.since);
            let text = i18n.t_with("online-seek-elapsed", &[("time", &elapsed_label(elapsed))]);
            tabular_text(ui, &text, egui::FontId::proportional(14.0), p.ink_muted);
            if !seek.posted {
                muted(ui, i18n.t("fics-seek-sent"));
            }
        });
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        footer(ui, |ui| {
            if ui.add(CsButton::new(&i18n.t("fics-seek-cancel"))).clicked() {
                intents.push(FicsIntent::Unseek);
            }
        });
        return;
    }
    section(ui, &i18n.t("online-pace-label"));
    ui.add_space(8.0);
    pace_grid(ui, i18n, "grid_fics_seek_paces", &mut state.fics_form.pace);
    ui.add_space(12.0);
    muted(ui, i18n.t("fics-seek-note"));
    ui.add_space(8.0);
    let p = palette(ui);
    icon_line(
        ui,
        icon::SHIELD_CHECK,
        p.accent,
        &i18n.t("fics-fairplay-note"),
    );
    footer(ui, |ui| {
        if ui
            .add(CsButton::new(&i18n.t("fics-seek-start")).variant(ButtonVariant::Primary))
            .clicked()
        {
            intents.push(FicsIntent::Seek);
        }
        if ui.add(CsButton::new(&i18n.t("online-cancel"))).clicked() {
            intents.push(FicsIntent::Close);
        }
    });
}

/// O ritmo e o tipo do desafio: "blitz 3+0 · sem rating".
pub fn challenge_detail(i18n: &I18n, challenge: &FicsChallenge) -> String {
    let rated = i18n.t(if challenge.rated {
        "online-rated-short"
    } else {
        "online-casual"
    });
    format!(
        "{} {} · {rated}",
        challenge.kind,
        pace_text(challenge.minutes, challenge.increment_s)
    )
}

/// Os ritmos de busca, em botões (a mesma escolha da busca e do desafio por nome).
fn pace_grid(ui: &mut egui::Ui, i18n: &I18n, salt: &str, pace: &mut (u32, u32)) {
    egui::Grid::new(salt)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            for (label, paces) in PACES {
                let p = palette(ui);
                ui.label(egui::RichText::new(i18n.t(label)).color(p.ink_muted));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for option in paces {
                        if ui
                            .add(
                                CsButton::new(&pace_text(option.0, option.1))
                                    .toggled(*pace == *option),
                            )
                            .clicked()
                        {
                            *pace = *option;
                        }
                    }
                });
                ui.end_row();
            }
        });
}

/// Desafiar pelo nome (`match`): o nome, o ritmo e o botão; enviado, o estado e "Retirar".
fn challenge_by_name(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    let p = palette(ui);
    section(ui, &i18n.t("fics-challenge-by-name"));
    ui.add_space(8.0);
    if let Some(sent) = &view.outgoing {
        cs_card_frame(p, CardEmphasis::Full).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if !sent.issued {
                    ui.add(egui::Spinner::new().size(14.0).color(p.info));
                }
                let text = if sent.issued {
                    i18n.t_with(
                        "fics-challenge-waiting",
                        &[
                            ("name", &sent.opponent),
                            ("pace", &pace_text(sent.minutes, sent.increment_s)),
                        ],
                    )
                } else {
                    i18n.t_with("fics-challenge-sending", &[("name", &sent.opponent)])
                };
                ui.add(egui::Label::new(egui::RichText::new(text).color(p.ink)).wrap());
            });
            ui.add_space(8.0);
            if ui
                .add(CsButton::new(&i18n.t("fics-challenge-withdraw")).icon(icon::X_CIRCLE))
                .clicked()
            {
                intents.push(FicsIntent::Withdraw);
            }
        });
        return;
    }
    let typed = !state.fics_form.name.trim().is_empty();
    let valid = state.fics_form.challenge_name().is_some();
    let error = (typed && !valid).then(|| i18n.t("fics-challenge-name-invalid"));
    ui.scope(|ui| {
        ui.spacing_mut().text_edit_width = 260.0;
        cs_text_field(
            ui,
            &mut state.fics_form.name,
            &i18n.t("fics-challenge-name-hint"),
            error.as_deref(),
        );
    });
    ui.add_space(8.0);
    pace_grid(
        ui,
        i18n,
        "grid_fics_challenge_paces",
        &mut state.fics_form.pace,
    );
    ui.add_space(8.0);
    muted(ui, i18n.t("fics-challenge-note"));
    ui.add_space(8.0);
    if ui
        .add(
            CsButton::new(&i18n.t("fics-challenge-send"))
                .icon(icon::SWORD)
                .variant(ButtonVariant::Primary)
                .enabled(valid),
        )
        .clicked()
    {
        intents.push(FicsIntent::Challenge);
    }
}

fn challenges_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    challenge_by_name(ui, i18n, state, view, intents);
    ui.add_space(16.0);
    section(ui, &i18n.t("fics-challenges-received"));
    ui.add_space(8.0);
    if view.challenges.is_empty() {
        let handle = view.handle.as_deref().unwrap_or("?");
        state_block(
            ui,
            Some(icon::SWORD),
            &i18n.t("fics-challenges-empty-title"),
            &i18n.t_with("fics-challenges-empty", &[("handle", handle)]),
        );
    }
    for challenge in &view.challenges {
        let standard = ProviderId::Fics.info().accepts_variant(&challenge.kind);
        let p = palette(ui);
        cs_card_frame(p, CardEmphasis::Full).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let name = match challenge.rating {
                Some(r) => format!("{} ({r})", challenge.challenger),
                None => challenge.challenger.clone(),
            };
            ui.label(
                egui::RichText::new(i18n.t_with("online-incoming-line", &[("name", &name)]))
                    .font(strong_font(ui.ctx(), 14.0))
                    .color(p.ink),
            );
            muted(ui, challenge_detail(i18n, challenge));
            if !standard {
                ui.add_space(4.0);
                notice_line(
                    ui,
                    StatusTone::Warning,
                    &i18n.t_with("online-incoming-variant", &[("variant", &challenge.kind)]),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // Variante: sem "Aceitar" (o app só joga xadrez padrão; o serviço também recusa).
                if standard
                    && ui
                        .add(
                            CsButton::new(&i18n.t("online-accept")).variant(ButtonVariant::Primary),
                        )
                        .clicked()
                {
                    intents.push(FicsIntent::Accept(challenge.challenger.clone()));
                }
                if ui.add(CsButton::new(&i18n.t("online-decline"))).clicked() {
                    intents.push(FicsIntent::Decline(challenge.challenger.clone()));
                }
            });
        });
        ui.add_space(8.0);
    }
    footer(ui, |ui| {
        if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
            intents.push(FicsIntent::Close);
        }
    });
}

fn observe_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &mut OnlineUi,
    view: &FicsView,
    intents: &mut Vec<FicsIntent>,
) {
    let p = palette(ui);
    section(ui, &i18n.t("fics-observe-label"));
    ui.add_space(4.0);
    let typed = !state.fics_form.observe.trim().is_empty();
    let valid = state.fics_form.observe_number().is_some();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.scope(|ui| {
            ui.spacing_mut().text_edit_width = 120.0;
            let error = (typed && !valid).then(|| i18n.t("fics-observe-invalid"));
            cs_text_field(
                ui,
                &mut state.fics_form.observe,
                &i18n.t("fics-observe-hint"),
                error.as_deref(),
            );
        });
        if ui
            .add(
                CsButton::new(&i18n.t("fics-observe-start"))
                    .icon(icon::EYE)
                    .enabled(valid && !view.observing),
            )
            .clicked()
        {
            intents.push(FicsIntent::Observe);
        }
    });
    ui.add_space(8.0);
    icon_line(ui, icon::INFO, p.info, &i18n.t("fics-observe-note"));
    if view.observe_requested.is_some() && !view.observing {
        ui.add_space(4.0);
        muted(ui, i18n.t("fics-observe-waiting"));
    }
    if let Some(observed) = &view.observed {
        ui.add_space(12.0);
        observed_card(ui, i18n, observed, view.observing, intents);
    }
    footer(ui, |ui| {
        if ui.add(CsButton::new(&i18n.t("online-close"))).clicked() {
            intents.push(FicsIntent::Close);
        }
    });
}

/// "4:05" a partir de milissegundos (números tabulares no relógio).
fn clock_text(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A Partida observada: um tabuleiro pequeno (só leitura), os nomes, os relógios e o fim.
fn observed_card(
    ui: &mut egui::Ui,
    i18n: &I18n,
    observed: &ObservedGame,
    observing: bool,
    intents: &mut Vec<FicsIntent>,
) {
    let p = palette(ui);
    let snapshot = &observed.snapshot;
    cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let title = if observing {
            i18n.t_with("fics-observe-now", &[("game", &observed.game.to_string())])
        } else {
            i18n.t_with(
                "fics-observe-ended",
                &[("game", &observed.game.to_string())],
            )
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .font(strong_font(ui.ctx(), 14.0))
                    .color(p.ink),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if observing
                    && ui
                        .add(
                            CsButton::new(&i18n.t("fics-observe-stop"))
                                .variant(ButtonVariant::Subtle),
                        )
                        .clicked()
                {
                    intents.push(FicsIntent::Unobserve);
                }
            });
        });
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let side = 208.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
            let pieces = chess_input::all_squares()
                .filter_map(|s| snapshot.position.piece_at(s).map(|cp| (s, cp)))
                .collect();
            let marks = crate::aids::Marks {
                last_move: snapshot.history.last().map(|h| (h.mv.from(), h.mv.to())),
                ..Default::default()
            };
            crate::board2d_panel::paint_board(
                ui.painter(),
                rect,
                crate::board2d_panel::Orientation::WhiteBottom,
                &pieces,
                &marks,
                None,
                true,
            );
            ui.vertical(|ui| {
                let clock = snapshot.clock.as_ref();
                for (glyph, name, ms) in [
                    ("\u{2654}", &observed.white, clock.map(|c| c.white_ms)),
                    ("\u{265A}", &observed.black, clock.map(|c| c.black_ms)),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(glyph).color(p.ink_muted));
                        ui.label(egui::RichText::new(name).color(p.ink));
                        if let Some(ms) = ms {
                            tabular_text(
                                ui,
                                &clock_text(ms),
                                egui::FontId::proportional(14.0),
                                p.ink_muted,
                            );
                        }
                    });
                }
                ui.add_space(8.0);
                muted(
                    ui,
                    i18n.t_with(
                        "fics-observe-moves",
                        &[("count", &snapshot.history.len().to_string())],
                    ),
                );
                if let Some(last) = snapshot.history.last() {
                    muted(ui, i18n.t_with("fics-observe-last", &[("san", &last.san)]));
                }
                if let Some(outcome) = snapshot.outcome {
                    ui.add_space(4.0);
                    let result = i18n.t(crate::ui::result_key(outcome.result));
                    let reason = i18n.t(crate::ui::reason_key(outcome.reason));
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{result} · {reason}")).color(p.ink),
                        )
                        .wrap(),
                    );
                }
            });
        });
    });
    if observing {
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}

// --- Pedidos ----------------------------------------------------------------------------------

/// Executa o que a tela do FICS pediu.
pub fn apply(
    intent: FicsIntent,
    ui_state: &mut OnlineUi,
    service: &OnlineService,
    net: &NetService,
    consent: Option<&mut ConsentDialog>,
    i18n: &I18n,
    actions: &mut MessageWriter<ActionRequested>,
) {
    let form = ui_state.fics_form.clone();
    let now = net.clock().now();
    let result = service.with(|play| {
        let fics = &mut play.fics;
        match intent {
            FicsIntent::Close => {
                ui_state.open = false;
                Ok(())
            }
            FicsIntent::TurnOn => {
                if let Some(dialog) = consent {
                    dialog.ask(ProviderId::Fics, Some(i18n.t("fics-consent-feature")));
                }
                Ok(())
            }
            FicsIntent::OpenCenter => {
                ui_state.open = false;
                actions.write(ActionRequested(Action::OpenConnections));
                Ok(())
            }
            FicsIntent::Connect => fics.connect(net),
            FicsIntent::Leave if fics.game_in_progress() && !fics.dropped_mid_game() => {
                // Com a Partida em andamento, sair é perdê-la no FICS: pede confirmação.
                ui_state.fics_confirm_leave = true;
                Ok(())
            }
            FicsIntent::Leave | FicsIntent::ConfirmLeave => {
                ui_state.fics_confirm_leave = false;
                fics.leave();
                Ok(())
            }
            FicsIntent::KeepPlaying => {
                ui_state.fics_confirm_leave = false;
                Ok(())
            }
            FicsIntent::Seek => fics.seek(form.pace.0, form.pace.1, now),
            FicsIntent::Unseek => fics.unseek(),
            FicsIntent::Accept(name) => fics.accept(&name),
            FicsIntent::Decline(name) => fics.decline(&name),
            FicsIntent::Challenge => match form.challenge_name() {
                Some(name) => fics.challenge(name, form.pace.0, form.pace.1, None),
                None => Ok(()),
            },
            FicsIntent::Withdraw => fics.withdraw_challenge(),
            // Precisa do banco e do tabuleiro: o quadro da Partida online faz (`drive_online`).
            FicsIntent::CloseUnconfirmed => {
                ui_state.close_request = fics.dropped_store_id();
                Ok(())
            }
            FicsIntent::Observe => match form.observe_number() {
                Some(game) => fics.observe(game),
                None => Ok(()),
            },
            FicsIntent::Unobserve => fics.unobserve(),
        }
    });
    ui_state.fics_error = result.err();
}
