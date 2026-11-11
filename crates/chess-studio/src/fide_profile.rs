//! Rating FIDE no Perfil local (CX-19, #242; decisões CX-04 e a nota do Provedor FIDE na
//! ADR-0006).
//!
//! Com um ID FIDE vinculado na Central, mostra nome, federação, título e os ratings
//! **standard, rápido e blitz separados**, nunca comparados com ratings online, com a Origem
//! ("FIDE, via Lichess, consultado em ...") e o botão Atualizar. O último valor fica guardado
//! por Perfil local (`fide_ratings`, migração 17) e aparece sem rede. A consulta só sai com o
//! Lichess ligado (o diálogo "Ligar Lichess?" pergunta antes) e não é bloqueada pela Trava de
//! fair play: o rating não calcula nada.
//!
//! O dado vem do **Lichess**, que o reproduz da FIDE, e a resposta não traz o mês da lista
//! da FIDE: a data que a tela mostra é a da consulta. Este módulo só apresenta e consulta;
//! nunca escreve na Partida (ADR-0001).

use crate::connections::model::{self, Age};
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::short_date::date_text;
use crate::training_ui::with_store;
use crate::ui_theme::{
    ButtonVariant, CsButton, Palette, StatusTone, cs_status_badge, cs_text_field, strong_font,
};
use bevy_egui::egui::{self, RichText, Ui};
use chess_net::{ProviderId, Timestamp};
use chess_providers::{Answer, FideRating, ProviderError, Query};
use chess_store::{Connection, ConnectionKind, ConnectionRole, FideSnapshot, ProfileId, Store};
use egui_phosphor::regular as icon;
use std::sync::Mutex;

/// Quantos jogadores a busca por nome mostra de uma vez.
const MAX_HITS: usize = 8;

// --- Modelo (puro) -----------------------------------------------------------------------

/// O ritmo de um rating FIDE. Os três são listas separadas da FIDE: nunca se somam, se
/// comparam ou se copiam de um para o outro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rhythm {
    Standard,
    Rapid,
    Blitz,
}

impl Rhythm {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Standard => "fide-rhythm-standard",
            Self::Rapid => "fide-rhythm-rapid",
            Self::Blitz => "fide-rhythm-blitz",
        }
    }
}

/// Os três ratings na ordem fixa standard, rápido, blitz; um ritmo sem rating fica `None`.
pub(crate) fn ratings(snapshot: &FideSnapshot) -> [(Rhythm, Option<u16>); 3] {
    [
        (Rhythm::Standard, snapshot.standard),
        (Rhythm::Rapid, snapshot.rapid),
        (Rhythm::Blitz, snapshot.blitz),
    ]
}

/// O que guardar de uma resposta do Lichess: a data é a da consulta que o Provedor confirmou.
pub(crate) fn snapshot_of(answer: &Answer<FideRating>) -> FideSnapshot {
    let fide = &answer.value;
    FideSnapshot {
        fide_id: fide.id,
        name: fide.name.clone(),
        federation: fide.federation.clone(),
        title: fide.title.clone(),
        birth_year: fide.birth_year,
        standard: fide.standard,
        rapid: fide.rapid,
        blitz: fide.blitz,
        inactive: fide.inactive,
        queried_at_ms: answer.provenance.queried_at.as_millis(),
        source: answer.provenance.provider.key().to_owned(),
    }
}

/// O ID FIDE da Conexão padrão do Perfil local (a única que a tela consulta).
pub(crate) fn linked_id(connections: &[Connection]) -> Option<u64> {
    connections
        .iter()
        .filter(|c| c.provider == ProviderId::Fide.key())
        .filter(|c| c.role == ConnectionRole::Player && !c.is_pending())
        .filter(|c| c.kind == ConnectionKind::PublicId)
        .find(|c| c.is_default)
        .or_else(|| {
            connections.iter().find(|c| {
                c.provider == ProviderId::Fide.key()
                    && c.role == ConnectionRole::Player
                    && !c.is_pending()
                    && c.kind == ConnectionKind::PublicId
            })
        })
        .and_then(|c| c.identifier.trim().parse().ok())
}

/// Quando a consulta foi feita, em palavras. O app não lê o fuso do sistema, então a data só
/// aparece a partir de um dia de idade, quando um dia a mais ou a menos não engana.
pub(crate) fn when_text(i18n: &I18n, queried_at_ms: i64, now: Timestamp) -> String {
    let then = Timestamp::from_millis(queried_at_ms);
    match model::age(now, then) {
        Age::JustNow => i18n.t("fide-when-now"),
        Age::Minutes(n) => i18n.t_with("fide-when-minutes", &[("count", &n.to_string())]),
        Age::Hours(n) => i18n.t_with("fide-when-hours", &[("count", &n.to_string())]),
        Age::Days(n) => i18n.t_with(
            "fide-when-days",
            &[
                ("date", &date_text(queried_at_ms)),
                ("count", &n.to_string()),
            ],
        ),
    }
}

// --- Estado e execução -------------------------------------------------------------------

/// O que a tela pede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FideAction {
    /// Consultar (ou atualizar) o rating do ID vinculado. `feature` é o nome do recurso na
    /// frase do diálogo "Ligar Lichess?".
    Refresh { feature: String },
    /// Buscar jogadores da FIDE por nome.
    Search { text: String, feature: String },
    /// Abrir ou fechar a busca por nome.
    ToggleSearch,
    /// Vincular o ID de um jogador achado na busca (a Central cria a Conexão).
    Link { id: u64, name: String },
    /// Ir à Central de Conexões (a página Perfil local não tem busca nem vínculo).
    OpenCentral,
}

/// O resultado da última ação, na linha de baixo do bloco.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub key: &'static str,
    pub tone: StatusTone,
}

impl Notice {
    fn new(key: &'static str, tone: StatusTone) -> Self {
        Self { key, tone }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    Refresh(u64),
    Search(String),
}

enum Running {
    Refresh(Query<FideRating>),
    Search(Query<Vec<FideRating>>),
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Refresh(_) => "Running::Refresh",
            Self::Search(_) => "Running::Search",
        })
    }
}

#[derive(Debug, Default)]
enum Task {
    #[default]
    Idle,
    /// O diálogo "Ligar Lichess?" está aberto para este pedido.
    AwaitingConsent(Job),
    /// `Query` não é `Sync`: o `Mutex` deixa o estado viver num recurso do Bevy.
    Running(Box<Mutex<Running>>),
}

/// O rating FIDE do Perfil local ativo e a busca por nome.
#[derive(Debug, Default)]
pub struct FideProfile {
    /// Último valor guardado do ID vinculado.
    pub(crate) saved: Option<FideSnapshot>,
    loaded_for: Option<(ProfileId, Option<u64>)>,
    task: Task,
    pub(crate) notice: Option<Notice>,
    pub(crate) search_open: bool,
    pub(crate) search_text: String,
    pub(crate) hits: Vec<FideRating>,
    /// A busca terminou (mesmo sem resultados).
    pub(crate) searched: bool,
    /// Banco em memória quando não há persistência (capturas, benchmark).
    local: Option<Mutex<Store>>,
}

/// O que a execução precisa do app.
pub(crate) struct Env<'a> {
    pub service: &'a NetService,
    pub dialog: &'a mut ConsentDialog,
    pub handle: Option<&'a PersistenceHandle>,
    pub connections: &'a [Connection],
}

impl FideProfile {
    /// Valor de exemplo para as capturas (`--connections-demo`): jogador fictício.
    pub(crate) fn demo() -> Self {
        let mut fide = Self::default();
        let now = NetService::now();
        let snapshot = FideSnapshot {
            fide_id: 9_999_991,
            name: "Exemplo, Ana".into(),
            federation: Some("BRA".into()),
            title: Some("FM".into()),
            birth_year: Some(1999),
            standard: Some(2301),
            rapid: Some(2250),
            blitz: None,
            inactive: false,
            queried_at_ms: now.as_millis() - 2 * 86_400_000,
            source: ProviderId::Lichess.key().to_owned(),
        };
        let _ = with_store(&mut fide.local, None, |store, profile| {
            store.save_fide_rating(profile, &snapshot)
        });
        fide
    }

    /// Busca aberta com resultados fictícios, para as capturas (`--connections-fide-search`).
    pub(crate) fn demo_search(&mut self) {
        let hit = |id, name: &str, title: Option<&str>, standard, rapid, blitz| FideRating {
            id,
            name: name.to_owned(),
            federation: Some("BRA".to_owned()),
            birth_year: None,
            title: title.map(str::to_owned),
            standard,
            rapid,
            blitz,
            inactive: false,
        };
        self.search_open = true;
        self.search_text = "Exemplo".to_owned();
        self.searched = true;
        self.hits = vec![
            hit(
                9_999_991,
                "Exemplo, Ana",
                Some("FM"),
                Some(2301),
                Some(2250),
                None,
            ),
            hit(
                9_999_992,
                "Exemplo, Ana Paula",
                None,
                Some(1850),
                None,
                None,
            ),
            FideRating {
                inactive: true,
                ..hit(
                    9_999_993,
                    "Exemplo, Ana Clara",
                    None,
                    Some(1620),
                    Some(1580),
                    Some(1544),
                )
            },
        ];
    }

    /// Há uma consulta ou busca em andamento (ou esperando o diálogo).
    pub(crate) fn is_busy(&self) -> bool {
        !matches!(self.task, Task::Idle)
    }

    pub(crate) fn is_searching(&self) -> bool {
        match &self.task {
            Task::AwaitingConsent(Job::Search(_)) => true,
            Task::Running(running) => running
                .lock()
                .is_ok_and(|r| matches!(&*r, Running::Search(_))),
            _ => false,
        }
    }

    /// Mantém o valor guardado em dia com o Perfil local ativo e o ID vinculado.
    pub(crate) fn reload(&mut self, handle: Option<&PersistenceHandle>, id: Option<u64>) {
        let loaded_for = self.loaded_for;
        let loaded = with_store(&mut self.local, handle, |store, profile| {
            if loaded_for == Some((profile, id)) {
                return (profile, None);
            }
            let saved = id.and_then(|id| store.fide_rating(profile, id).ok().flatten());
            (profile, Some(saved))
        });
        let Some((profile, saved)) = loaded else {
            return;
        };
        if let Some(saved) = saved {
            if self
                .loaded_for
                .is_some_and(|(p, i)| p != profile || (i.is_some() && i != id))
            {
                // Outro Perfil local ou outro ID: nada da tela anterior vale.
                self.notice = None;
                self.hits.clear();
                self.searched = false;
                self.search_open = false;
                self.task = Task::Idle;
            }
            self.saved = saved;
            self.loaded_for = Some((profile, id));
        }
    }

    /// Trabalho de cada quadro: recarrega o valor guardado, responde ao diálogo e colhe a
    /// consulta que terminou. Nunca bloqueia.
    pub(crate) fn drive(&mut self, env: &mut Env, answers: &[ProviderConsentAnswered]) {
        self.reload(env.handle, linked_id(env.connections));
        match std::mem::take(&mut self.task) {
            Task::Idle => {}
            Task::AwaitingConsent(job) => {
                match answers.iter().find(|a| a.provider == ProviderId::Lichess) {
                    None => self.task = Task::AwaitingConsent(job),
                    Some(a) if a.accepted => self.start(job, env.service),
                    Some(_) => {
                        self.notice =
                            Some(Notice::new("fide-note-not-turned-on", StatusTone::Neutral));
                    }
                }
            }
            Task::Running(running) => {
                let mut running = running;
                let done = running.get_mut().ok().and_then(Running::poll);
                match done {
                    Some(outcome) => self.finish(outcome, env.handle),
                    None => self.task = Task::Running(running),
                }
            }
        }
    }

    /// Executa um pedido da tela.
    pub(crate) fn perform(&mut self, action: FideAction, env: &mut Env) {
        if self.is_busy() {
            return;
        }
        self.notice = None;
        let (job, feature) = match action {
            FideAction::ToggleSearch => {
                self.search_open = !self.search_open;
                return;
            }
            // Quem desenha trata: criar a Conexão e trocar de página não são desta tela.
            FideAction::Link { .. } | FideAction::OpenCentral => return,
            FideAction::Refresh { feature } => {
                let Some(id) = linked_id(env.connections) else {
                    return;
                };
                (Job::Refresh(id), feature)
            }
            FideAction::Search { text, feature } => (Job::Search(text), feature),
        };
        if env.service.is_enabled(ProviderId::Lichess) {
            self.start(job, env.service);
            return;
        }
        // Lichess desligado: o diálogo pergunta antes de qualquer envio (CX-09).
        env.dialog.ask(ProviderId::Lichess, Some(feature));
        if env.dialog.current() == Some(ProviderId::Lichess) {
            self.task = Task::AwaitingConsent(job);
        }
    }

    fn start(&mut self, job: Job, service: &NetService) {
        let running = match job {
            Job::Refresh(id) => Running::Refresh(service.lichess().fide_player_refreshed(id)),
            Job::Search(text) => Running::Search(service.lichess().fide_search(&text)),
        };
        self.task = Task::Running(Box::new(Mutex::new(running)));
    }

    fn finish(&mut self, outcome: Outcome, handle: Option<&PersistenceHandle>) {
        match outcome {
            Outcome::Rating(Ok(answer)) => {
                let fresh = snapshot_of(&answer);
                let same = self
                    .saved
                    .as_ref()
                    .is_some_and(|old| ratings(old) == ratings(&fresh) && old.title == fresh.title);
                let saved = with_store(&mut self.local, handle, |store, profile| {
                    store.save_fide_rating(profile, &fresh)
                });
                if matches!(saved, Some(Err(_))) {
                    self.notice = Some(Notice::new("fide-note-save-failed", StatusTone::Warning));
                } else {
                    self.notice = Some(if same {
                        Notice::new("fide-note-same", StatusTone::Success)
                    } else {
                        Notice::new("fide-note-updated", StatusTone::Success)
                    });
                }
                self.saved = Some(fresh);
            }
            Outcome::Rating(Err(error)) => {
                self.notice = Some(failure(&error));
            }
            Outcome::Search(Ok(answer)) => {
                self.hits = answer.value;
                self.searched = true;
            }
            Outcome::Search(Err(error)) => {
                self.hits.clear();
                self.searched = false;
                self.notice = Some(failure(&error));
            }
        }
    }
}

/// A frase curta de uma falha, com o tom certo.
fn failure(error: &ProviderError) -> Notice {
    match error {
        ProviderError::NoData | ProviderError::UnknownName { .. } => {
            Notice::new("fide-note-not-found", StatusTone::Warning)
        }
        ProviderError::RateLimited { .. } => Notice::new("fide-note-limited", StatusTone::Warning),
        ProviderError::Disabled { .. } => {
            Notice::new("fide-note-not-turned-on", StatusTone::Neutral)
        }
        ProviderError::InvalidInput { .. } => {
            Notice::new("fide-note-search-short", StatusTone::Warning)
        }
        ProviderError::Net(e) => Notice::new(model::error_key(e), StatusTone::Danger),
        ProviderError::Malformed { .. } => {
            Notice::new("connections-err-malformed", StatusTone::Danger)
        }
        _ => Notice::new("connections-err-other", StatusTone::Danger),
    }
}

enum Outcome {
    Rating(Result<Answer<FideRating>, ProviderError>),
    Search(Result<Answer<Vec<FideRating>>, ProviderError>),
}

impl Running {
    fn poll(&mut self) -> Option<Outcome> {
        match self {
            Self::Refresh(q) => q.try_take().map(Outcome::Rating),
            Self::Search(q) => q.try_take().map(Outcome::Search),
        }
    }
}

// --- Tela --------------------------------------------------------------------------------

/// O contexto de desenho: `on_card` diz se o fundo é o do cartão (`surface-2`) ou o da página
/// dos bastidores (`surface-1`), para os blocos de rating contrastarem com ele; `in_central`,
/// se a busca por nome e o vínculo do ID cabem aqui (só na Central).
pub(crate) struct Look<'a> {
    pub ctx: &'a egui::Context,
    pub i18n: &'a I18n,
    pub now: Timestamp,
    pub lichess_on: bool,
    pub on_card: bool,
    pub in_central: bool,
}

/// O bloco do rating FIDE: o que a Central mostra no cartão da FIDE e a página Perfil local
/// mostra abaixo do perfil. Devolve os pedidos do quadro.
pub(crate) fn rating_block(
    ui: &mut Ui,
    look: &Look,
    fide: &mut FideProfile,
    id: Option<u64>,
) -> Vec<FideAction> {
    let mut actions = Vec::new();
    let i18n = look.i18n;
    let p = crate::ui_theme::palette(ui);
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.label(
        RichText::new(i18n.t("fide-title"))
            .font(strong_font(look.ctx, 14.0))
            .color(p.ink),
    );
    match (id, fide.saved.clone()) {
        (None, _) => {
            let key = if look.in_central {
                "fide-not-linked"
            } else {
                "fide-not-linked-here"
            };
            ui.label(RichText::new(i18n.t(key)).color(p.ink_muted));
            if !look.in_central
                && ui
                    .add(
                        CsButton::new(&i18n.t("fide-open-central"))
                            .icon(icon::PLUGS_CONNECTED)
                            .variant(ButtonVariant::Secondary),
                    )
                    .clicked()
            {
                actions.push(FideAction::OpenCentral);
            }
        }
        (Some(id), None) => {
            ui.label(
                RichText::new(i18n.t_with("fide-never", &[("id", &id.to_string())])).color(p.ink),
            );
        }
        (Some(_), Some(saved)) => {
            player_line(ui, look, &saved, p);
            tiles(ui, look, &saved, p);
            origin_lines(ui, look, &saved, p);
        }
    }
    if id.is_some() {
        refresh_row(ui, look, fide, id, &mut actions, p);
    }
    if let Some(notice) = &fide.notice {
        notice_line(ui, look, notice, p);
    }
    if look.in_central {
        search(ui, look, fide, id, &mut actions, p);
    }
    actions
}

/// O bloco da página Perfil local dos bastidores. Devolve `true` se a pessoa pediu para abrir a
/// Central (o vínculo do ID e a busca por nome vivem lá).
pub fn profile_block(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    central: &mut crate::connections::CentralParams,
) -> bool {
    ui.separator();
    ui.add_space(12.0);
    let look = Look {
        ctx,
        i18n,
        now: NetService::now(),
        lichess_on: central.service.is_enabled(ProviderId::Lichess),
        on_card: false,
        in_central: false,
    };
    let crate::connections::ConnectionsUi { fide, list, .. } = &mut *central.ui;
    let id = linked_id(list);
    let actions = rating_block(ui, &look, fide, id);
    let mut open_central = false;
    for action in actions {
        if action == FideAction::OpenCentral {
            open_central = true;
            continue;
        }
        let mut env = Env {
            service: &central.service,
            dialog: &mut central.dialog,
            handle: central.persistence.as_deref(),
            connections: list,
        };
        fide.perform(action, &mut env);
    }
    open_central
}

fn player_line(ui: &mut Ui, look: &Look, saved: &FideSnapshot, p: &Palette) {
    let i18n = look.i18n;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(
            RichText::new(&saved.name)
                .font(strong_font(look.ctx, 18.0))
                .color(p.ink),
        );
        if let Some(title) = &saved.title {
            cs_status_badge(ui, StatusTone::Info, icon::MEDAL, title)
                .on_hover_text(i18n.t("fide-title-hint"));
        }
        if saved.inactive {
            cs_status_badge(
                ui,
                StatusTone::Neutral,
                icon::MOON,
                &i18n.t("fide-inactive"),
            )
            .on_hover_text(i18n.t("fide-inactive-hint"));
        }
        if let Some(federation) = &saved.federation {
            ui.label(
                RichText::new(i18n.t_with("fide-federation", &[("code", federation)]))
                    .text_style(egui::TextStyle::Small)
                    .color(p.ink_muted),
            );
        }
        ui.label(
            RichText::new(i18n.t_with("fide-id", &[("id", &saved.fide_id.to_string())]))
                .text_style(egui::TextStyle::Small)
                .color(p.ink_muted),
        );
    });
}

/// Os três ratings lado a lado, cada um na sua caixa e com o seu nome: nunca um número solto.
fn tiles(ui: &mut Ui, look: &Look, saved: &FideSnapshot, p: &Palette) {
    let i18n = look.i18n;
    let gap = 8.0;
    let width = ((ui.available_width() - 2.0 * gap) / 3.0).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (rhythm, value) in ratings(saved) {
            let fill = if look.on_card {
                p.surface_1
            } else {
                p.surface_2
            };
            egui::Frame::new()
                .fill(fill)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(width - 24.0 - 2.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(
                            RichText::new(i18n.t(rhythm.key()))
                                .text_style(egui::TextStyle::Small)
                                .color(p.ink_muted),
                        );
                        match value {
                            Some(rating) => {
                                ui.label(
                                    RichText::new(rating.to_string())
                                        .font(strong_font(look.ctx, 28.0))
                                        .color(p.ink),
                                );
                            }
                            None => {
                                ui.label(
                                    RichText::new("—")
                                        .font(strong_font(look.ctx, 28.0))
                                        .color(p.ink_disabled),
                                )
                                .on_hover_text(i18n.t("fide-no-rating"));
                            }
                        }
                    });
                });
        }
    });
}

/// "FIDE, via Lichess, consultado ..." e o aviso de que a data é a da consulta.
fn origin_lines(ui: &mut Ui, look: &Look, saved: &FideSnapshot, p: &Palette) {
    let i18n = look.i18n;
    let when = when_text(i18n, saved.queried_at_ms, look.now);
    ui.label(
        RichText::new(i18n.t_with("fide-origin", &[("when", &when)]))
            .text_style(egui::TextStyle::Small)
            .color(p.ink),
    );
    ui.label(
        RichText::new(i18n.t("fide-explain"))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
    );
    if !look.lichess_on {
        ui.label(
            RichText::new(i18n.t("fide-offline"))
                .text_style(egui::TextStyle::Small)
                .color(p.ink_muted),
        );
    }
}

fn refresh_row(
    ui: &mut Ui,
    look: &Look,
    fide: &FideProfile,
    id: Option<u64>,
    actions: &mut Vec<FideAction>,
    p: &Palette,
) {
    let i18n = look.i18n;
    let busy = fide.is_busy();
    let label = if fide.saved.is_some() {
        i18n.t("fide-refresh")
    } else {
        i18n.t("fide-consult")
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let variant = if fide.saved.is_some() {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Primary
        };
        let button = ui.add(
            CsButton::new(&label)
                .icon(icon::ARROWS_CLOCKWISE)
                .variant(variant)
                .enabled(!busy && id.is_some()),
        );
        if button.clicked() {
            actions.push(FideAction::Refresh {
                feature: i18n.t("fide-feature"),
            });
        }
        if busy && !fide.is_searching() {
            ui.label(RichText::new(i18n.t("fide-working")).color(p.ink_muted));
        }
    });
}

fn notice_line(ui: &mut Ui, look: &Look, notice: &Notice, p: &Palette) {
    let (glyph, color) = match notice.tone {
        StatusTone::Success => (icon::CHECK_CIRCLE, p.success),
        StatusTone::Danger => (icon::WARNING_CIRCLE, p.danger),
        StatusTone::Warning => (icon::WARNING, p.warning),
        _ => (icon::INFO, p.info),
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(glyph)
                .font(crate::ui_theme::icon_font(look.ctx, 14.0))
                .color(color),
        );
        ui.label(
            RichText::new(look.i18n.t(notice.key))
                .text_style(egui::TextStyle::Small)
                .color(p.ink),
        );
    });
}

/// Busca por nome, para quem não sabe o ID. Cada resultado mostra os três ratings separados e
/// o botão "Vincular" (que a Central transforma numa Conexão).
fn search(
    ui: &mut Ui,
    look: &Look,
    fide: &mut FideProfile,
    id: Option<u64>,
    actions: &mut Vec<FideAction>,
    p: &Palette,
) {
    let i18n = look.i18n;
    // Sem ID vinculado a busca já vem aberta: é o caminho para achar o ID.
    let open = fide.search_open || id.is_none();
    if id.is_some() {
        let label = if open {
            i18n.t("fide-search-hide")
        } else {
            i18n.t("fide-search-show")
        };
        if ui
            .add(
                CsButton::new(&label)
                    .icon(icon::MAGNIFYING_GLASS)
                    .variant(ButtonVariant::Subtle),
            )
            .clicked()
        {
            actions.push(FideAction::ToggleSearch);
        }
    }
    if !open {
        return;
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(i18n.t("fide-search-title"))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
    );
    let searching = fide.is_searching();
    let mut submit = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.scope(|ui| {
            ui.spacing_mut().text_edit_width = 240.0;
            let field = cs_text_field(ui, &mut fide.search_text, &i18n.t("fide-search-hint"), None);
            submit |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        let ready = !searching && fide.search_text.trim().chars().count() >= 2;
        let button = ui.add(
            CsButton::new(&i18n.t("fide-search-go"))
                .icon(icon::MAGNIFYING_GLASS)
                .variant(ButtonVariant::Secondary)
                .enabled(ready),
        );
        submit |= button.clicked();
        if searching {
            ui.label(RichText::new(i18n.t("fide-working")).color(p.ink_muted));
        }
    });
    if submit && !searching && !fide.is_busy() && fide.search_text.trim().chars().count() >= 2 {
        actions.push(FideAction::Search {
            text: fide.search_text.trim().to_owned(),
            feature: i18n.t("fide-feature-search"),
        });
    }
    if fide.searched && fide.hits.is_empty() {
        ui.label(RichText::new(i18n.t("fide-search-empty")).color(p.ink_muted));
    }
    for hit in fide.hits.iter().take(MAX_HITS) {
        hit_row(ui, look, hit, id, actions, p);
    }
    if fide.hits.len() > MAX_HITS {
        ui.label(
            RichText::new(i18n.t_with(
                "fide-search-more",
                &[
                    ("shown", &MAX_HITS.to_string()),
                    ("total", &fide.hits.len().to_string()),
                ],
            ))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
        );
    }
}

fn hit_row(
    ui: &mut Ui,
    look: &Look,
    hit: &FideRating,
    linked: Option<u64>,
    actions: &mut Vec<FideAction>,
    p: &Palette,
) {
    let i18n = look.i18n;
    let fill = if look.on_card {
        p.surface_1
    } else {
        p.surface_2
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(
                    RichText::new(&hit.name)
                        .font(strong_font(look.ctx, 14.0))
                        .color(p.ink),
                );
                if let Some(title) = &hit.title {
                    cs_status_badge(ui, StatusTone::Info, icon::MEDAL, title);
                }
                if hit.inactive {
                    cs_status_badge(
                        ui,
                        StatusTone::Neutral,
                        icon::MOON,
                        &i18n.t("fide-inactive"),
                    );
                }
                if let Some(federation) = &hit.federation {
                    ui.label(
                        RichText::new(i18n.t_with("fide-federation", &[("code", federation)]))
                            .text_style(egui::TextStyle::Small)
                            .color(p.ink_muted),
                    );
                }
            });
            let value = |v: Option<u16>| v.map_or_else(|| "—".to_owned(), |v| v.to_string());
            ui.label(
                RichText::new(i18n.t_with(
                    "fide-hit-ratings",
                    &[
                        ("standard", &value(hit.standard)),
                        ("rapid", &value(hit.rapid)),
                        ("blitz", &value(hit.blitz)),
                        ("id", &hit.id.to_string()),
                    ],
                ))
                .text_style(egui::TextStyle::Small)
                .color(p.ink_muted),
            );
            if linked == Some(hit.id) {
                cs_status_badge(
                    ui,
                    StatusTone::Success,
                    icon::LINK_SIMPLE,
                    &i18n.t("fide-hit-linked"),
                );
            } else if ui
                .add(
                    CsButton::new(&i18n.t("fide-hit-link"))
                        .icon(icon::LINK_SIMPLE)
                        .variant(ButtonVariant::Secondary),
                )
                .clicked()
            {
                actions.push(FideAction::Link {
                    id: hit.id,
                    name: hit.name.clone(),
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig, RetryPolicy};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// 2026-11-11T12:00:00Z.
    const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

    const ANA: &str = r#"{"id":9999991,"name":"Exemplo, Ana","federation":"BRA","year":1999,
        "title":"FM","standard":2301,"rapid":2250,"blitz":2199}"#;

    struct Env {
        server: FakeServer,
        clock: Arc<FakeClock>,
        service: NetService,
        dialog: ConsentDialog,
        fide: FideProfile,
        list: Vec<Connection>,
    }

    fn env() -> Env {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let config = NetConfig::in_memory()
            .clock(clock.clone())
            .jitter_seed(7)
            .retry(RetryPolicy {
                max_retries: 0,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(1),
            })
            .route("lichess.org", &server.base_url());
        let net = Net::open(config).expect("abre a rede");
        let mut env = Env {
            server,
            clock: clock.clone(),
            service: NetService::from_net(net, clock, false),
            dialog: ConsentDialog::default(),
            fide: FideProfile::default(),
            list: Vec::new(),
        };
        env.link(9_999_991);
        env
    }

    impl Env {
        fn link(&mut self, id: u64) {
            let mut store = Store::open_in_memory().unwrap();
            let profile = store.default_profile().unwrap();
            store
                .add_connection(
                    profile,
                    &chess_store::NewConnection::new(
                        "fide",
                        ConnectionKind::PublicId,
                        id.to_string(),
                    ),
                )
                .unwrap();
            self.list = store.list_connections(profile, None).unwrap();
        }

        fn run(&mut self, action: FideAction, answers: &[ProviderConsentAnswered]) {
            let mut env = Env_ {
                service: &self.service,
                dialog: &mut self.dialog,
                handle: None,
                connections: &self.list,
            };
            self.fide.perform(action, &mut env);
            let _ = answers;
        }

        fn drive(&mut self, answers: &[ProviderConsentAnswered]) {
            let mut env = Env_ {
                service: &self.service,
                dialog: &mut self.dialog,
                handle: None,
                connections: &self.list,
            };
            self.fide.drive(&mut env, answers);
        }

        /// Quadro a quadro até a consulta terminar (nunca bloqueia: só `try_take`).
        fn settle(&mut self, answers: &[ProviderConsentAnswered]) {
            let start = Instant::now();
            let mut answers = answers.to_vec();
            while start.elapsed() < Duration::from_secs(5) {
                self.drive(&answers);
                answers.clear();
                if !self.fide.is_busy() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("a consulta não terminou");
        }
    }

    type Env_<'a> = super::Env<'a>;

    fn refresh() -> FideAction {
        FideAction::Refresh {
            feature: "O rating FIDE".into(),
        }
    }

    #[test]
    fn standard_rapid_and_blitz_are_kept_apart_and_a_missing_one_stays_missing() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(ANA));
        env.fide.reload(None, Some(9_999_991));
        env.run(refresh(), &[]);
        env.settle(&[]);
        let saved = env.fide.saved.clone().expect("guardou");
        assert_eq!(
            ratings(&saved),
            [
                (Rhythm::Standard, Some(2301)),
                (Rhythm::Rapid, Some(2250)),
                (Rhythm::Blitz, Some(2199)),
            ]
        );
        // Sem blitz na FIDE: continua sem, não herda o rápido nem o standard.
        let json = r#"{"id":9999991,"name":"Exemplo, Ana","standard":2301,"rapid":2250}"#;
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(json));
        env.clock.advance(Duration::from_secs(60));
        env.run(refresh(), &[]);
        env.settle(&[]);
        let saved = env.fide.saved.clone().unwrap();
        assert_eq!(ratings(&saved)[2], (Rhythm::Blitz, None));
    }

    #[test]
    fn the_rating_is_saved_with_the_query_date_and_shown_again_without_any_network() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(ANA));
        env.run(refresh(), &[]);
        env.settle(&[]);
        let saved = env.fide.saved.clone().unwrap();
        assert_eq!(saved.queried_at_ms, T0.as_millis());
        assert_eq!(saved.source, "lichess");
        assert_eq!(env.fide.notice.as_ref().unwrap().key, "fide-note-updated");

        // Uma tela nova sobre o mesmo banco, com o Lichess desligado: o valor aparece e nenhum
        // pedido sai.
        let requests = env.server.requests().len();
        env.service.set_enabled(ProviderId::Lichess, false).unwrap();
        let mut again = FideProfile {
            local: env.fide.local.take(),
            ..FideProfile::default()
        };
        again.reload(None, Some(9_999_991));
        assert_eq!(again.saved, Some(saved));
        assert_eq!(env.server.requests().len(), requests);
    }

    #[test]
    fn an_unchanged_rating_says_so() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(ANA));
        env.run(refresh(), &[]);
        env.settle(&[]);
        env.clock.advance(Duration::from_secs(3600));
        env.run(refresh(), &[]);
        env.settle(&[]);
        assert_eq!(env.fide.notice.as_ref().unwrap().key, "fide-note-same");
        assert_eq!(env.server.requests().len(), 2, "Atualizar vai ao Provedor");
    }

    #[test]
    fn with_the_lichess_off_the_dialog_asks_first_and_nothing_is_sent() {
        let mut env = env();
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(ANA));
        env.run(refresh(), &[]);
        assert_eq!(env.dialog.current(), Some(ProviderId::Lichess));
        env.drive(&[]);
        assert!(
            env.server.requests().is_empty(),
            "nada sai antes da resposta"
        );
        assert!(env.fide.is_busy());

        // "Agora não": nada é consultado e a tela diz isso.
        env.fide.task = Task::Idle;
        env.dialog.dismiss();
        env.run(refresh(), &[]);
        env.drive(&[ProviderConsentAnswered {
            provider: ProviderId::Lichess,
            accepted: false,
        }]);
        assert!(!env.fide.is_busy());
        assert_eq!(
            env.fide.notice.as_ref().unwrap().key,
            "fide-note-not-turned-on"
        );
        assert!(env.server.requests().is_empty());

        // "Ligar": a consulta sai.
        env.dialog.dismiss();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.run(refresh(), &[]);
        env.settle(&[]);
        assert!(env.fide.saved.is_some());
    }

    #[test]
    fn the_fair_play_guard_does_not_block_the_rating() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.service.net().fair_play_guard().engage("Partida online");
        env.server
            .route("/api/fide/player/9999991", FakeResponse::json(ANA));
        env.run(refresh(), &[]);
        env.settle(&[]);
        assert!(env.fide.saved.is_some());
    }

    #[test]
    fn a_missing_player_and_a_pause_are_said_in_plain_words() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/fide/player/9999991", FakeResponse::status(404));
        env.run(refresh(), &[]);
        env.settle(&[]);
        assert_eq!(env.fide.notice.as_ref().unwrap().key, "fide-note-not-found");
        assert!(env.fide.saved.is_none());
        env.server
            .route("/api/fide/player/9999991", FakeResponse::status(429));
        env.run(refresh(), &[]);
        env.settle(&[]);
        assert_eq!(env.fide.notice.as_ref().unwrap().key, "fide-note-limited");
    }

    #[test]
    fn the_name_search_lists_players_and_a_short_text_is_refused() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        let json = format!(
            "[{ANA},{}]",
            r#"{"id":9999992,"name":"Exemplo, Beto","standard":1800}"#
        );
        env.server
            .route("/api/fide/player", FakeResponse::json(&json));
        env.run(
            FideAction::Search {
                text: "Exemplo".into(),
                feature: "A busca".into(),
            },
            &[],
        );
        assert!(env.fide.is_searching());
        env.settle(&[]);
        assert_eq!(env.fide.hits.len(), 2);
        assert!(env.fide.searched);
        assert_eq!(env.fide.hits[1].rapid, None);

        env.run(
            FideAction::Search {
                text: "x".into(),
                feature: "A busca".into(),
            },
            &[],
        );
        env.settle(&[]);
        assert_eq!(
            env.fide.notice.as_ref().unwrap().key,
            "fide-note-search-short"
        );
    }

    #[test]
    fn the_linked_id_is_the_default_fide_connection_only() {
        let mut env = env();
        assert_eq!(linked_id(&env.list), Some(9_999_991));
        env.list.clear();
        assert_eq!(linked_id(&env.list), None);
    }

    #[test]
    fn dates_and_ages_read_in_plain_portuguese() {
        let i18n = I18n::pt_br();
        let at = |secs: u64| T0.plus(Duration::from_secs(secs));
        let ms = T0.as_millis();
        assert_eq!(when_text(&i18n, ms, at(10)), "agora há pouco");
        assert_eq!(when_text(&i18n, ms, at(300)), "há 5 min");
        assert_eq!(when_text(&i18n, ms, at(7200)), "há 2 h");
        assert_eq!(
            when_text(&i18n, ms, at(3 * 86_400)),
            "em 11/11/2026 (há 3 dias)"
        );
    }
}
