//! Baixar pacotes de problemas do Lichess pela rede (CX-16, #238), em Treino > Pacotes.
//!
//! A pessoa escolhe temas (com nomes em português), a faixa de dificuldade e a quantidade; o
//! app baixa **em fluxo** a base pública CC0 do Lichess, escolhe os problemas e instala um
//! pacote com a Origem (Lichess, CC0, data). O trabalho todo é do adaptador
//! (`chess_providers::Lichess::download_puzzles`), que passa pelo gate da rede; aqui ficam só
//! o formulário, o andamento e o consentimento: com o Provedor desligado o app abre o diálogo
//! "Ligar o Lichess?" e retoma o download se a pessoa aceitar. Apresentação (ADR-0001): nada
//! aqui escreve Partida.

use super::packs::{self, Cmd};
use super::{TrainingUi, themes};
use crate::i18n::I18n;
use crate::net_service::{NetService, Readiness};
use crate::persistence::PersistenceHandle;
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, StatusTone, cs_button, cs_card_frame, cs_disclosure, cs_segmented,
    cs_status_badge, cs_text_field, cs_toggle_button, palette,
};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_net::ProviderId;
use chess_providers::{
    Lichess, MAX_PUZZLES, ProviderError, PuzzleDownload, PuzzleEvent, PuzzleOutcome,
    PuzzleProgress, PuzzleRequest,
};
use chess_training::{PackInfo, PuzzleFilter, ThemeGroup, themes_in};
use egui_phosphor::regular as icon;
use std::collections::BTreeSet;

/// Quantidades oferecidas (de problemas por pacote).
const COUNTS: [usize; 4] = [100, 250, 500, 1000];
const DEFAULT_COUNT: usize = 250;
/// Nomes de tema no nome do pacote antes de virar "N temas".
const NAME_THEMES: usize = 3;

/// Faixas de dificuldade prontas (o Lichess vai de uns 400 a uns 3000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Level {
    #[default]
    Any,
    Beginner,
    Intermediate,
    Advanced,
    Expert,
    Custom,
}

impl Level {
    const ALL: [Level; 6] = [
        Self::Any,
        Self::Beginner,
        Self::Intermediate,
        Self::Advanced,
        Self::Expert,
        Self::Custom,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Any => "training-download-level-any",
            Self::Beginner => "training-download-level-beginner",
            Self::Intermediate => "training-download-level-intermediate",
            Self::Advanced => "training-download-level-advanced",
            Self::Expert => "training-download-level-expert",
            Self::Custom => "training-download-level-custom",
        }
    }

    /// Faixa pronta (mínimo, máximo); a própria vem dos campos.
    fn preset(self) -> (Option<u32>, Option<u32>) {
        match self {
            Self::Any | Self::Custom => (None, None),
            Self::Beginner => (None, Some(1200)),
            Self::Intermediate => (Some(1200), Some(1800)),
            Self::Advanced => (Some(1800), Some(2400)),
            Self::Expert => (Some(2400), None),
        }
    }
}

/// Download em andamento. `handle` é `None` só nas capturas de demonstração.
pub(super) struct Running {
    handle: Option<PuzzleDownload>,
    progress: PuzzleProgress,
    target: usize,
    cancelling: bool,
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Running")
            .field("progress", &self.progress)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

/// O que a última baixada deixou, mostrado até a próxima.
#[derive(Debug)]
pub(super) enum Message {
    Done {
        name: String,
        count: usize,
        date: String,
        short: bool,
    },
    Empty,
    Cancelled,
    Declined,
    Unavailable,
    Failed(ProviderError),
}

#[derive(Debug)]
pub(super) struct DownloadUi {
    /// O formulário está aberto.
    open: bool,
    themes: BTreeSet<String>,
    /// Grupos de temas abertos.
    groups_open: [bool; 9],
    level: Level,
    custom_min: String,
    custom_max: String,
    count: usize,
    running: Option<Running>,
    /// O diálogo "Ligar o Lichess?" está aberto e o download espera a resposta.
    waiting_consent: bool,
    message: Option<Message>,
    /// O Lichess está ligado? Atualizado pelo painel a cada quadro.
    pub(super) provider_on: Option<bool>,
}

impl Default for DownloadUi {
    fn default() -> Self {
        Self {
            open: false,
            themes: BTreeSet::new(),
            // Motivos táticos aberto: é o que a maioria procura primeiro.
            groups_open: [false, true, false, false, false, false, false, false, false],
            level: Level::Any,
            custom_min: String::new(),
            custom_max: String::new(),
            count: DEFAULT_COUNT,
            running: None,
            waiting_consent: false,
            message: None,
            provider_on: None,
        }
    }
}

impl DownloadUi {
    /// Só para a captura do diálogo de consentimento.
    pub(super) fn set_waiting(&mut self) {
        self.waiting_consent = true;
    }

    pub(super) fn busy(&self) -> bool {
        self.running.is_some() || self.waiting_consent
    }

    fn bounds(&self) -> (Option<u32>, Option<u32>) {
        match self.level {
            Level::Custom => (
                self.custom_min.trim().parse().ok(),
                self.custom_max.trim().parse().ok(),
            ),
            other => other.preset(),
        }
    }

    fn filter(&self) -> PuzzleFilter {
        let (min_rating, max_rating) = self.bounds();
        PuzzleFilter {
            themes: self.themes.iter().cloned().collect(),
            min_rating,
            max_rating,
            max_count: self.count.min(MAX_PUZZLES),
        }
    }
}

// --- Texto -------------------------------------------------------------------------------

/// Inteiro com ponto de milhar: 1.234.567.
fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Tamanho legível, com vírgula decimal: 840 KB, 12,3 MB.
fn fmt_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    let value = n as f64;
    if value < KB {
        format!("{n} B")
    } else if value < KB * KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{:.1} MB", value / (KB * KB)).replace('.', ",")
    }
}

/// `2026-11-11` vira `11/11/2026`.
pub(super) fn br_date(iso: &str) -> String {
    let mut parts = iso.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => iso.to_owned(),
    }
}

/// A faixa em palavras: "de 1200 a 1800", "até 1200"…
fn level_label(i18n: &I18n, min: Option<u32>, max: Option<u32>) -> String {
    match (min, max) {
        (None, None) => i18n.t("training-download-level-label-any"),
        (Some(min), Some(max)) => i18n.t_with(
            "training-download-level-label-range",
            &[("min", &min.to_string()), ("max", &max.to_string())],
        ),
        (Some(min), None) => i18n.t_with(
            "training-download-level-label-from",
            &[("min", &min.to_string())],
        ),
        (None, Some(max)) => i18n.t_with(
            "training-download-level-label-until",
            &[("max", &max.to_string())],
        ),
    }
}

/// Os temas escolhidos em palavras, para a frase e o nome do pacote.
fn themes_label(i18n: &I18n, themes: &BTreeSet<String>, limit: usize) -> String {
    if themes.is_empty() {
        return i18n.t("training-download-summary-themes-any");
    }
    if themes.len() > limit {
        return i18n.t_with(
            "training-download-pack-name-many",
            &[("count", &themes.len().to_string())],
        );
    }
    let names: Vec<String> = themes
        .iter()
        .map(|id| themes::theme_name(i18n, id))
        .collect();
    names.join(", ")
}

/// Nome do pacote para a lista: "Lichess: Mate em 2, Garfo (de 1200 a 1800)".
fn pack_name(i18n: &I18n, form: &DownloadUi) -> String {
    let (min, max) = form.bounds();
    let themes = if form.themes.is_empty() {
        i18n.t("training-download-pack-name-any")
    } else {
        themes_label(i18n, &form.themes, NAME_THEMES)
    };
    if min.is_none() && max.is_none() {
        i18n.t_with("training-download-pack-name", &[("themes", &themes)])
    } else {
        i18n.t_with(
            "training-download-pack-name-level",
            &[("themes", &themes), ("level", &level_label(i18n, min, max))],
        )
    }
}

/// Identificador do pacote: `lichess-` e os temas e a faixa em minúsculas, sem acento.
fn pack_id(form: &DownloadUi) -> String {
    let (min, max) = form.bounds();
    let mut text = String::from("lichess");
    for theme in form.themes.iter().take(NAME_THEMES) {
        text.push('-');
        text.push_str(theme);
    }
    if form.themes.len() > NAME_THEMES {
        text.push_str(&format!("-{}-temas", form.themes.len()));
    }
    if min.is_some() || max.is_some() {
        text.push_str(&format!(
            "-{}-{}",
            min.map_or_else(String::new, |v| v.to_string()),
            max.map_or_else(String::new, |v| v.to_string())
        ));
    }
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '-' };
        if c != '-' || !out.ends_with('-') {
            out.push(c);
        }
    }
    out.trim_matches('-').chars().take(56).collect()
}

fn failure_text(i18n: &I18n, error: &ProviderError) -> String {
    use chess_net::NetError;
    match error {
        ProviderError::Disabled { .. } => i18n.t("training-download-error-disabled"),
        ProviderError::RateLimited { .. } => i18n.t("training-download-error-paused"),
        ProviderError::Malformed { .. } => i18n.t("training-download-error-format"),
        ProviderError::Net(NetError::Http { status, .. }) => i18n.t_with(
            "training-download-error-http",
            &[("status", &status.to_string())],
        ),
        ProviderError::Net(NetError::Unreachable { .. } | NetError::Timeout { .. }) => {
            i18n.t("training-download-error-offline")
        }
        other => i18n.t_with(
            "training-download-error-other",
            &[("reason", &other.to_string())],
        ),
    }
}

// --- Fluxo ---------------------------------------------------------------------------------

impl TrainingUi {
    /// Começa a baixada com o cliente do Lichess (o Provedor já está ligado).
    fn start_download(&mut self, lichess: &Lichess, i18n: &I18n) {
        let Some(dir) = self.packs.dir.clone() else {
            self.packs.download.message = Some(Message::Unavailable);
            return;
        };
        if self.packs.download.running.is_some() {
            return;
        }
        let form = &self.packs.download;
        let filter = form.filter();
        let target = filter.max_count;
        let id = packs::unique_id(pack_id(form), &self.packs.installed);
        let name = pack_name(i18n, form);
        let handle = lichess.download_puzzles(PuzzleRequest {
            filter,
            dir,
            id,
            name,
        });
        let form = &mut self.packs.download;
        form.message = None;
        form.waiting_consent = false;
        form.running = Some(Running {
            handle: Some(handle),
            progress: PuzzleProgress::default(),
            target,
            cancelling: false,
        });
    }

    fn finish_download(&mut self, outcome: PuzzleOutcome, handle: Option<&PersistenceHandle>) {
        let target = self
            .packs
            .download
            .running
            .as_ref()
            .map_or(usize::MAX, |r| r.target);
        let message = if outcome.cancelled {
            Message::Cancelled
        } else if let Some(info) = outcome.pack {
            let PackInfo {
                name,
                count,
                downloaded_on,
                ..
            } = info;
            self.reload_packs_from(self.packs.dir.clone());
            self.sync_catalog(handle);
            Message::Done {
                name,
                count,
                date: downloaded_on.map_or_else(String::new, |d| br_date(&d)),
                short: count < target,
            }
        } else {
            Message::Empty
        };
        self.packs.download.running = None;
        self.packs.download.message = Some(message);
    }

    /// Consome o que a baixada em andamento avisou (chamado a cada quadro).
    pub(super) fn poll_download(&mut self, handle: Option<&PersistenceHandle>) {
        let Some(running) = self.packs.download.running.as_mut() else {
            return;
        };
        let mut finished = None;
        if let Some(job) = running.handle.as_mut() {
            while let Some(event) = job.try_next() {
                match event {
                    PuzzleEvent::Progress(p) => running.progress = p,
                    PuzzleEvent::Finished(outcome) => finished = Some(Ok(outcome)),
                    PuzzleEvent::Failed(error) => finished = Some(Err(error)),
                }
            }
        }
        match finished {
            None => {}
            Some(Ok(outcome)) => self.finish_download(outcome, handle),
            Some(Err(error)) => {
                self.packs.download.running = None;
                self.packs.download.message = Some(Message::Failed(error));
            }
        }
    }
}

/// Começa a baixada pelo painel: liga o Provedor pelo diálogo de consentimento se preciso.
pub(super) fn begin(state: &mut TrainingUi, world: &mut World, i18n: &I18n) {
    if state.packs.download.busy() {
        return;
    }
    if !state.packs.available() {
        state.packs.download.message = Some(Message::Unavailable);
        return;
    }
    let Some(lichess) = world
        .get_resource::<NetService>()
        .map(|net| net.lichess().clone())
    else {
        state.packs.download.message = Some(Message::Unavailable);
        return;
    };
    let feature = i18n.t("training-download-feature");
    let readiness = world.resource_scope(|world, mut dialog: Mut<ConsentDialog>| {
        world
            .get_resource::<NetService>()
            .map(|net| net.ensure_enabled(ProviderId::Lichess, Some(&feature), &mut dialog))
    });
    match readiness {
        Some(Readiness::Ready) => state.start_download(&lichess, i18n),
        Some(Readiness::Asked) => {
            state.packs.download.message = None;
            state.packs.download.waiting_consent = true;
        }
        Some(Readiness::NoNetworkService) | None => {
            state.packs.download.message = Some(Message::Unavailable);
        }
    }
}

/// Sistema: acompanha a baixada em andamento.
pub(super) fn poll(mut state: ResMut<TrainingUi>, handle: Option<Res<PersistenceHandle>>) {
    if state.packs.download.running.is_some() {
        state.poll_download(handle.as_deref());
    }
}

/// Sistema: a resposta do diálogo "Ligar o Lichess?" retoma (ou desiste de) a baixada.
pub(super) fn on_consent(
    mut answers: MessageReader<ProviderConsentAnswered>,
    net: Option<Res<NetService>>,
    i18n: Res<I18n>,
    mut state: ResMut<TrainingUi>,
) {
    for answer in answers.read() {
        if answer.provider != ProviderId::Lichess || !state.packs.download.waiting_consent {
            continue;
        }
        state.packs.download.waiting_consent = false;
        match (&net, answer.accepted) {
            (Some(net), true) => {
                let lichess = net.lichess().clone();
                state.start_download(&lichess, &i18n);
            }
            _ => state.packs.download.message = Some(Message::Declined),
        }
    }
}

// --- Cartão --------------------------------------------------------------------------------

fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    let muted = palette(ui).ink_muted;
    ui.label(
        egui::RichText::new(text.into())
            .text_style(egui::TextStyle::Small)
            .color(muted),
    );
}

fn section_label(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.into()).strong());
}

/// O cartão "Baixar do Lichess" de Treino > Pacotes. Devolve o comando que a pessoa pediu.
pub(super) fn card(ui: &mut egui::Ui, i18n: &I18n, form: &mut DownloadUi) -> Option<Cmd> {
    let mut cmd = None;
    let p = palette(ui);
    cs_card_frame(p, CardEmphasis::Full).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(egui::RichText::new(i18n.t("training-download-heading")).strong());
        caption(ui, i18n.t("training-download-intro"));
        provider_badge(ui, i18n, form.provider_on);
        if form.running.is_some() {
            cmd = progress(ui, i18n, form);
        } else if form.waiting_consent {
            caption(ui, i18n.t("training-download-waiting-consent"));
        } else if form.open {
            cmd = formulary(ui, i18n, form);
        } else if cs_button(
            ui,
            &i18n.t("training-download-open"),
            ButtonVariant::Primary,
        )
        .clicked()
        {
            form.open = true;
            form.message = None;
        }
        if let Some(message) = &form.message {
            ui.add_space(2.0);
            message_view(ui, i18n, message);
        }
    });
    cmd
}

fn provider_badge(ui: &mut egui::Ui, i18n: &I18n, on: Option<bool>) {
    match on {
        Some(true) => {
            cs_status_badge(
                ui,
                StatusTone::Success,
                icon::CHECK_CIRCLE,
                &i18n.t("training-download-provider-on"),
            );
        }
        Some(false) => {
            ui.horizontal_wrapped(|ui| {
                cs_status_badge(
                    ui,
                    StatusTone::Neutral,
                    icon::PLUGS,
                    &i18n.t("training-download-provider-off-badge"),
                );
            });
            caption(ui, i18n.t("training-download-provider-off"));
        }
        None => {}
    }
}

fn formulary(ui: &mut egui::Ui, i18n: &I18n, form: &mut DownloadUi) -> Option<Cmd> {
    let mut cmd = None;

    // Temas.
    section_label(ui, i18n.t("training-download-themes"));
    ui.horizontal_wrapped(|ui| {
        let summary = match form.themes.len() {
            0 => i18n.t("training-download-themes-any"),
            1 => i18n.t("training-download-themes-one"),
            n => i18n.t_with(
                "training-download-themes-many",
                &[("count", &n.to_string())],
            ),
        };
        ui.label(summary);
        if !form.themes.is_empty()
            && cs_button(
                ui,
                &i18n.t("training-download-themes-clear"),
                ButtonVariant::Subtle,
            )
            .clicked()
        {
            form.themes.clear();
        }
    });
    if form.themes.is_empty() {
        caption(ui, i18n.t("training-download-themes-hint"));
    } else {
        caption(ui, themes_label(i18n, &form.themes, usize::MAX));
    }
    for (index, group) in ThemeGroup::ALL.into_iter().enumerate() {
        let picked = themes_in(group)
            .filter(|id| form.themes.contains(*id))
            .count();
        let mut label = i18n.t(&format!("puzzle-theme-group-{}", group.key()));
        if picked > 0 {
            label = format!("{label} ({picked})");
        }
        cs_disclosure(ui, &mut form.groups_open[index], &label, &label);
        if form.groups_open[index] {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for id in themes_in(group) {
                    let mut on = form.themes.contains(id);
                    if cs_toggle_button(ui, &mut on, &themes::theme_name(i18n, id)).clicked() {
                        if on {
                            form.themes.insert(id.to_owned());
                        } else {
                            form.themes.remove(id);
                        }
                    }
                }
            });
        }
    }

    // Dificuldade.
    section_label(ui, i18n.t("training-download-level"));
    let options: Vec<(Level, String)> = Level::ALL
        .into_iter()
        .map(|level| (level, i18n.t(level.key())))
        .collect();
    crate::ui_theme::cs_dropdown(ui, "training-download-level", &mut form.level, &options);
    if form.level == Level::Custom {
        ui.horizontal_wrapped(|ui| {
            for (text, key) in [
                (&mut form.custom_min, "training-filter-min-hint"),
                (&mut form.custom_max, "training-filter-max-hint"),
            ] {
                ui.allocate_ui(egui::vec2(120.0, 0.0), |ui| {
                    cs_text_field(ui, text, &i18n.t(key), None);
                });
            }
        });
    }

    // Quantidade.
    section_label(ui, i18n.t("training-download-count"));
    let counts: Vec<(usize, String)> = COUNTS.iter().map(|n| (*n, fmt_int(*n as u64))).collect();
    cs_segmented(ui, &mut form.count, &counts);

    // Resumo e ação.
    ui.add_space(4.0);
    let (min, max) = form.bounds();
    caption(
        ui,
        i18n.t_with(
            "training-download-summary",
            &[
                ("count", &fmt_int(form.count as u64)),
                ("themes", &themes_label(i18n, &form.themes, NAME_THEMES)),
                ("level", &level_label(i18n, min, max)),
            ],
        ),
    );
    ui.horizontal_wrapped(|ui| {
        if cs_button(
            ui,
            &i18n.t("training-download-start"),
            ButtonVariant::Primary,
        )
        .clicked()
        {
            cmd = Some(Cmd::StartDownload);
        }
        if cs_button(
            ui,
            &i18n.t("training-download-close"),
            ButtonVariant::Secondary,
        )
        .clicked()
        {
            form.open = false;
        }
    });
    cmd
}

fn progress(ui: &mut egui::Ui, i18n: &I18n, form: &mut DownloadUi) -> Option<Cmd> {
    let running = form.running.as_ref()?;
    let p = running.progress;
    ui.ctx().request_repaint();
    let mut cmd = None;
    if p.read == 0 && p.total.is_none() {
        ui.label(i18n.t("training-download-connecting"));
    } else {
        ui.label(i18n.t_with(
            "training-download-found",
            &[
                ("selected", &fmt_int(p.selected as u64)),
                ("target", &fmt_int(running.target as u64)),
            ],
        ));
    }
    let fraction = (p.selected as f32 / running.target.max(1) as f32).clamp(0.0, 1.0);
    bar(ui, fraction);
    if p.read > 0 || p.total.is_some() {
        let read = fmt_int(p.read);
        let downloaded = fmt_bytes(p.downloaded);
        caption(
            ui,
            match p.total {
                Some(total) => i18n.t_with(
                    "training-download-read",
                    &[
                        ("read", &read),
                        ("downloaded", &downloaded),
                        ("total", &fmt_bytes(total)),
                    ],
                ),
                None => i18n.t_with(
                    "training-download-read-unknown",
                    &[("read", &read), ("downloaded", &downloaded)],
                ),
            },
        );
    }
    caption(ui, i18n.t("training-download-stops"));
    let cancelling = running.cancelling;
    if ui
        .add_enabled_ui(!cancelling, |ui| {
            cs_button(
                ui,
                &i18n.t("training-download-cancel"),
                ButtonVariant::Secondary,
            )
        })
        .inner
        .clicked()
    {
        cmd = Some(Cmd::CancelDownload);
    }
    cmd
}

/// Barra de andamento com os tokens: trilho em `surface-4`, preenchimento em `accent`.
fn bar(ui: &mut egui::Ui, fraction: f32) {
    let p = palette(ui);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
    let corner = egui::CornerRadius::same(4);
    ui.painter().rect_filled(rect, corner, p.surface_4);
    let mut filled = rect;
    filled.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if filled.width() > 0.0 {
        ui.painter().rect_filled(filled, corner, p.accent);
    }
}

fn message_view(ui: &mut egui::Ui, i18n: &I18n, message: &Message) {
    match message {
        Message::Done {
            name,
            count,
            date,
            short,
        } => {
            cs_status_badge(
                ui,
                StatusTone::Success,
                icon::CHECK_CIRCLE,
                &i18n.t("training-download-done-badge"),
            );
            ui.label(i18n.t_with(
                "training-download-done",
                &[("name", name), ("count", &count.to_string())],
            ));
            if *short {
                caption(
                    ui,
                    i18n.t_with(
                        "training-download-done-short",
                        &[("count", &count.to_string())],
                    ),
                );
            }
            if !date.is_empty() {
                caption(
                    ui,
                    i18n.t_with("training-download-done-origin", &[("date", date)]),
                );
            }
        }
        Message::Empty => info(ui, i18n, "training-download-empty"),
        Message::Cancelled => info(ui, i18n, "training-download-cancelled"),
        Message::Declined => info(ui, i18n, "training-download-declined"),
        Message::Unavailable => info(ui, i18n, "training-download-unavailable"),
        Message::Failed(error) => {
            cs_status_badge(
                ui,
                StatusTone::Danger,
                icon::WARNING_CIRCLE,
                &i18n.t("training-download-failed-badge"),
            );
            ui.label(failure_text(i18n, error));
        }
    }
}

fn info(ui: &mut egui::Ui, i18n: &I18n, key: &str) {
    cs_status_badge(
        ui,
        StatusTone::Info,
        icon::INFO,
        &i18n.t("training-download-info-badge"),
    );
    ui.label(i18n.t(key));
}

impl TrainingUi {
    /// Pede o cancelamento da baixada em andamento: a thread para e nada é instalado.
    pub(super) fn cancel_download(&mut self) {
        if let Some(running) = self.packs.download.running.as_mut() {
            running.cancelling = true;
            if let Some(handle) = &running.handle {
                handle.cancel();
            }
        }
    }
}

// --- Evidências (`--training download|download-run|download-done|...`) --------------------

/// Sistema das capturas (`--screenshot ... --training download*`): liga ou desliga o Lichess
/// da rede em memória, prepara a cena e, na `download-consent`, abre o diálogo de verdade.
pub(super) fn demo_system(
    mut state: ResMut<TrainingUi>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    net: Option<Res<NetService>>,
    mut dialog: Option<ResMut<ConsentDialog>>,
    i18n: Res<I18n>,
    mut frame: Local<u32>,
) {
    if capture.is_none() {
        return;
    }
    let Some(script) = super::demo_script().filter(|s| s.starts_with("download")) else {
        return;
    };
    *frame += 1;
    demo(&mut state, &script, *frame);
    let Some(net) = &net else { return };
    if *frame == 3 {
        let on = !matches!(script.as_str(), "download-off" | "download-consent");
        let _ = net.set_enabled(ProviderId::Lichess, on);
    }
    if *frame == 8
        && script == "download-consent"
        && let Some(dialog) = dialog.as_deref_mut()
    {
        let feature = i18n.t("training-download-feature");
        net.ensure_enabled(ProviderId::Lichess, Some(&feature), dialog);
        state.packs.download.set_waiting();
    }
}

/// Cenas de captura: o formulário preenchido, a baixada em andamento e os desfechos, com
/// dados fictícios (nenhuma chamada de rede). Só existe em `--screenshot`.
pub(super) fn demo(state: &mut TrainingUi, script: &str, frame: u32) {
    if frame != 3 {
        return;
    }
    state.sync_catalog(None);
    let dir = std::env::temp_dir().join("chess-studio-demo-pacotes");
    state.packs.dir = Some(dir);
    state.packs.installed = vec![
        PackInfo {
            id: "lichess-matein2".into(),
            name: "Lichess: Mate em 2 (de 1200 a 1800)".into(),
            count: 250,
            provider: Some("lichess".into()),
            license: Some("CC0-1.0".into()),
            downloaded_on: Some("2026-11-09".into()),
        },
        PackInfo {
            id: "problemas-do-clube".into(),
            name: "Lichess: lichess_db_puzzle.csv".into(),
            count: 500,
            provider: Some("lichess".into()),
            license: Some("CC0-1.0".into()),
            downloaded_on: None,
        },
    ];
    state.view = super::View::Packs;
    if script == "download-list" {
        demo_pack(state);
        state.packs.filter_pack = packs::PackChoice::Pack("lichess-matein2".into());
        state.view = super::View::List;
        return;
    }
    let form = &mut state.packs.download;
    form.open = true;
    form.themes = ["fork", "pin", "mateIn2"].map(String::from).into();
    form.groups_open = [false, true, false, true, false, false, false, false, false];
    form.level = Level::Intermediate;
    form.count = 500;
    match script {
        "download-running" => {
            form.running = Some(Running {
                handle: None,
                progress: PuzzleProgress {
                    downloaded: 12_900_000,
                    total: Some(268_000_000),
                    read: 183_000,
                    selected: 311,
                },
                target: 500,
                cancelling: false,
            });
        }
        "download-done" => {
            form.open = false;
            form.message = Some(Message::Done {
                name: "Lichess: Garfo (ataque duplo), Cravada, Mate em 2 (de 1200 a 1800)".into(),
                count: 500,
                date: "11/11/2026".into(),
                short: false,
            });
        }
        "download-error" => {
            form.open = false;
            form.message = Some(Message::Failed(ProviderError::Net(
                chess_net::NetError::Unreachable {
                    provider: ProviderId::Lichess,
                    detail: "rede indisponível".into(),
                },
            )));
        }
        _ => {}
    }
}

/// Um pacote fictício já instalado, com temas do Lichess, para mostrar a lista traduzida.
fn demo_pack(state: &mut TrainingUi) {
    const THEMES: [&[&str]; 6] = [
        &["mate", "mateIn2", "short", "middlegame"],
        &["fork", "advantage", "opening"],
        &["pin", "crushing", "middlegame", "kingsideAttack"],
        &["backRankMate", "mateIn1", "oneMove", "endgame"],
        &["skewer", "rookEndgame", "long"],
        &[
            "discoveredAttack",
            "sacrifice",
            "veryLong",
            "masterVsMaster",
        ],
    ];
    let exercises: Vec<chess_training::Exercise> = state
        .catalog
        .iter()
        .filter(|e| e.pack.is_none())
        .take(THEMES.len())
        .zip(THEMES)
        .enumerate()
        .map(|(i, (base, themes))| {
            let mut e = base.clone();
            e.id = format!("lichess-demo{i}");
            e.title = format!("Lichess 00{}Kq", i + 1);
            e.themes = themes.iter().map(|t| (*t).to_owned()).collect();
            e.rating = Some(1250 + 90 * i as u32);
            e.provenance = Some(chess_training::Provenance {
                provider: "lichess".into(),
                license: "CC0-1.0".into(),
                url: None,
                remote_id: None,
            });
            e
        })
        .collect();
    let text = serde_json::json!({
        "format": 1,
        "package": {"name": "Lichess: Mate em 2 (de 1200 a 1800)", "downloaded_on": "2026-11-09"},
        "exercises": exercises,
    })
    .to_string();
    if let Ok(pack) = chess_training::Catalog::from_pack_json("lichess-matein2", &text) {
        let _ = state.catalog.extend(pack);
    }
    let themes: BTreeSet<&str> = state
        .catalog
        .iter()
        .flat_map(|e| e.themes.iter().map(String::as_str))
        .collect();
    state.packs.theme_ids = themes.into_iter().map(str::to_owned).collect();
    if let Some(info) = state.packs.installed.first_mut() {
        info.count = exercises.len();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_sizes_use_brazilian_formatting() {
        assert_eq!(fmt_int(0), "0");
        assert_eq!(fmt_int(999), "999");
        assert_eq!(fmt_int(1000), "1.000");
        assert_eq!(fmt_int(4_123_456), "4.123.456");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(840 * 1024), "840 KB");
        assert_eq!(fmt_bytes(13_526_630), "12,9 MB");
        assert_eq!(br_date("2026-11-11"), "11/11/2026");
        assert_eq!(br_date("ontem"), "ontem");
    }

    #[test]
    fn the_form_becomes_a_filter_a_readable_name_and_a_safe_id() {
        let i18n = I18n::pt_br();
        let mut form = DownloadUi::default();
        assert_eq!(form.filter().max_count, DEFAULT_COUNT);
        assert!(form.filter().themes.is_empty());
        assert_eq!(pack_name(&i18n, &form), "Lichess: Problemas variados");
        assert_eq!(pack_id(&form), "lichess");

        form.themes = ["mateIn2", "fork"].map(String::from).into();
        form.level = Level::Intermediate;
        form.count = 500;
        let filter = form.filter();
        assert_eq!(filter.themes, ["fork", "mateIn2"]);
        assert_eq!(
            (filter.min_rating, filter.max_rating),
            (Some(1200), Some(1800))
        );
        assert_eq!(filter.max_count, 500);
        assert_eq!(
            pack_name(&i18n, &form),
            "Lichess: Garfo (ataque duplo), Mate em 2 (de 1200 a 1800)"
        );
        assert_eq!(pack_id(&form), "lichess-fork-matein2-1200-1800");

        form.themes = ["a", "b", "c", "d", "e"].map(String::from).into();
        assert_eq!(pack_name(&i18n, &form), "Lichess: 5 temas (de 1200 a 1800)");
        assert!(pack_id(&form).len() <= 56);

        form.level = Level::Custom;
        form.custom_min = " 1500 ".into();
        form.custom_max = "abc".into();
        assert_eq!(form.bounds(), (Some(1500), None));
        assert!(level_label(&i18n, Some(1500), None).contains("a partir de 1500"));
    }

    #[test]
    fn every_failure_has_a_plain_portuguese_message_that_says_what_to_do() {
        let i18n = I18n::pt_br();
        let lichess = ProviderId::Lichess;
        let errors = [
            ProviderError::Disabled { provider: lichess },
            ProviderError::Net(chess_net::NetError::Unreachable {
                provider: lichess,
                detail: "x".into(),
            }),
            ProviderError::Net(chess_net::NetError::Http {
                provider: lichess,
                status: 503,
            }),
            ProviderError::Malformed {
                provider: lichess,
                detail: "x".into(),
            },
        ];
        for error in errors {
            let text = failure_text(&i18n, &error);
            assert!(
                text.contains("nada foi") || text.contains("Nada foi"),
                "{text}"
            );
            assert!(
                !text.contains("training-download"),
                "chave sem texto: {text}"
            );
        }
    }

    #[test]
    fn the_cancel_command_flags_the_running_download() {
        let mut state = TrainingUi::default();
        state.packs.download.running = Some(Running {
            handle: None,
            progress: PuzzleProgress::default(),
            target: 10,
            cancelling: false,
        });
        state.cancel_download();
        assert!(state.packs.download.running.as_ref().unwrap().cancelling);
    }
}
