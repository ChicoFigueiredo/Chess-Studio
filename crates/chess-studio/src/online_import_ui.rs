//! Formulário "Importar do Lichess ou Chess.com" da Biblioteca (CX-9, #226): nome público,
//! período, ritmo, cor e máximo; andamento com Cancelar; resumo igual ao da importação de
//! arquivo; botão "Atualizar" que traz só as Partidas novas daquele nome.
//!
//! O trabalho (thread, `GamesFeed`, `import_pgn_stream`) vive em [`crate::online_import`];
//! aqui há só o estado do formulário e o desenho. Nada sincroniza sozinho: tudo começa num
//! clique.

use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::online_import::{
    Clients, Failure, Job, Period, Progress, Request, Site, Summary, last_imported_day,
};
use crate::panel_chrome::radius;
use crate::training_ui::with_store;
use crate::ui_theme::{
    ButtonVariant, CsButton, Tabular, cs_dropdown, cs_segmented, cs_text_field, icon_font, palette,
    strong_font,
};
use bevy_egui::egui;
use chess_core::Color;
use chess_net::Timestamp;
use chess_providers::{DEFAULT_MAX_GAMES, Day, Pace};
use chess_store::{ConnectionRole, ProfileId, Store};
use egui_phosphor::regular as icon;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// Máximos oferecidos (Partidas mais recentes primeiro).
const MAX_CHOICES: [u32; 4] = [50, 100, DEFAULT_MAX_GAMES, 1000];
/// "Atualizar" pede até isto (o período real é o que veio depois da última Partida).
const UPDATE_MAX: u32 = 1000;

/// O que o formulário sabe do resto do app, atualizado a cada quadro.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub lichess_on: bool,
    pub chesscom_on: bool,
    /// Partida competitiva em andamento (Trava de fair play).
    pub locked: bool,
    pub now: Option<Timestamp>,
    db: Option<PathBuf>,
    profile: Option<ProfileId>,
}

impl Env {
    fn enabled(&self, site: Site) -> bool {
        match site {
            Site::Lichess => self.lichess_on,
            Site::ChessCom => self.chesscom_on,
        }
    }
}

/// O que a última importação terminou fazendo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub site: Site,
    pub name: String,
    pub result: Result<Summary, Failure>,
}

/// Estado do formulário.
pub struct OnlineState {
    pub site: Site,
    pub name: String,
    period: Period,
    pace: Option<Pace>,
    color: Option<Color>,
    max: u32,
    job: Option<Job>,
    pub finished: Option<Finished>,
    /// Último dia já importado deste nome (habilita "Atualizar").
    last_day: Option<Day>,
    last_key: Option<(Site, String)>,
    suggestion: Option<String>,
    suggested_for: Option<Site>,
    clients: Option<Clients>,
    env: Env,
    /// Pedido de ligar o Provedor (o diálogo abre fora do desenho).
    pub ask_to_turn_on: Option<Site>,
}

impl Default for OnlineState {
    fn default() -> Self {
        Self {
            site: Site::Lichess,
            name: String::new(),
            period: Period::All,
            pace: None,
            color: None,
            max: DEFAULT_MAX_GAMES,
            job: None,
            finished: None,
            last_day: None,
            last_key: None,
            suggestion: None,
            suggested_for: None,
            clients: None,
            env: Env::default(),
            ask_to_turn_on: None,
        }
    }
}

impl OnlineState {
    /// Atualiza o que vem de fora do formulário. Chame uma vez por quadro, antes de desenhar.
    pub fn sync(
        &mut self,
        net: Option<&NetService>,
        locked: bool,
        handle: Option<&crate::persistence::PersistenceHandle>,
        local: &mut Option<Mutex<Store>>,
    ) {
        if let Some(net) = net {
            if self.clients.is_none() {
                self.clients = Some(Clients::of(net));
            }
            self.env.lichess_on = net.is_enabled(Site::Lichess.provider());
            self.env.chesscom_on = net.is_enabled(Site::ChessCom.provider());
        }
        self.env.locked = locked;
        self.env.now = Some(NetService::now());
        let key = (self.site, self.name.trim().to_lowercase());
        let wants_suggestion = self.suggested_for != Some(self.site);
        if self.last_key.as_ref() != Some(&key) || wants_suggestion {
            let (site, name) = (self.site, self.name.clone());
            let data = with_store(local, handle, |store, profile| {
                let day = last_imported_day(store, profile, site, &name);
                let suggestion = default_name(store, profile, site);
                (store.database_path(), profile, day, suggestion)
            });
            if let Some((db, profile, day, suggestion)) = data {
                self.env.db = db;
                self.env.profile = Some(profile);
                self.last_day = day;
                self.last_key = Some(key);
                self.suggestion = suggestion;
                self.suggested_for = Some(self.site);
            }
        }
    }

    /// Recolhe o resultado da thread, se terminou. `true` quando a Biblioteca mudou.
    pub fn poll(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let Some(result) = job.take_result() else {
            return false;
        };
        let (site, name) = (job.site, job.name.clone());
        self.job = None;
        self.finished = Some(Finished { site, name, result });
        // O "Atualizar" passa a valer para o que acabou de entrar.
        self.last_key = None;
        true
    }

    fn request(&self, update: bool) -> Request {
        Request {
            site: self.site,
            name: self.name.trim().to_owned(),
            period: self.period,
            pace: self.pace,
            color: self.color,
            max: if update { UPDATE_MAX } else { self.max },
            since: if update { self.last_day } else { None },
        }
    }

    /// Começa a importação (ou a atualização). Não faz nada sem Provedor ligado, sem nome ou
    /// com uma importação em andamento.
    pub fn start(&mut self, update: bool) -> bool {
        let request = self.request(update);
        if self.job.is_some()
            || !request.is_valid()
            || !self.env.enabled(self.site)
            || self.env.locked
        {
            return false;
        }
        let (Some(clients), Some(profile), Some(now)) =
            (self.clients.clone(), self.env.profile, self.env.now)
        else {
            return false;
        };
        self.finished = None;
        self.job = Some(Job::spawn(
            clients,
            self.env.db.clone(),
            profile,
            request,
            now,
        ));
        true
    }

    /// Um andamento de mentira para a captura de evidência.
    pub fn demo_running(&mut self, site: Site, name: &str, progress: Progress) {
        self.site = site;
        self.name = name.to_owned();
        self.job = Some(Job::demo_running(site, name, progress));
    }

    /// Um resumo de mentira para a captura de evidência.
    pub fn demo_finished(&mut self, site: Site, name: &str, summary: Summary) {
        self.site = site;
        self.name = name.to_owned();
        self.finished = Some(Finished {
            site,
            name: name.to_owned(),
            result: Ok(summary),
        });
    }
}

/// A Conexão padrão (papel de jogador) do Provedor: o nome que o formulário sugere.
fn default_name(store: &Store, profile: ProfileId, site: Site) -> Option<String> {
    store
        .list_connections_by_role(profile, Some(site.key()), ConnectionRole::Player)
        .ok()?
        .into_iter()
        .find(|c| c.is_default)
        .map(|c| c.identifier)
}

// --- Desenho --------------------------------------------------------------------------

fn label(ui: &mut egui::Ui, text: String) {
    let p = palette(ui);
    ui.label(egui::RichText::new(text).small().color(p.ink_muted));
}

/// Três números lado a lado, com o nome embaixo: recebidas, novas e repetidas.
fn stat_tiles(ui: &mut egui::Ui, tiles: [(String, usize); 3]) {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 64.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        radius("radius-m"),
        p.surface_2,
        egui::Stroke::new(1.0, p.border),
        egui::StrokeKind::Inside,
    );
    let cell = rect.width() / 3.0;
    let value_font = strong_font(ui.ctx(), 20.0);
    let caption = egui::TextStyle::Small.resolve(ui.style());
    for (i, (name, value)) in tiles.into_iter().enumerate() {
        let center_x = rect.left() + cell * (i as f32 + 0.5);
        let text = Tabular::new(
            &painter,
            &crate::explorer::fmt_count(value as u64),
            &value_font,
            p.ink,
        );
        text.paint(
            &painter,
            egui::pos2(center_x - text.size.x / 2.0, rect.top() + 10.0),
            p.ink,
        );
        painter.text(
            egui::pos2(center_x, rect.bottom() - 14.0),
            egui::Align2::CENTER_CENTER,
            name,
            caption.clone(),
            p.ink_muted,
        );
        if i > 0 {
            painter.vline(
                rect.left() + cell * i as f32,
                rect.y_range().shrink(12.0),
                egui::Stroke::new(1.0, p.border),
            );
        }
    }
}

fn pace_label(i18n: &I18n, pace: Option<Pace>) -> String {
    i18n.t(match pace {
        None => "library-online-pace-any",
        Some(Pace::Bullet) => "library-online-pace-bullet",
        Some(Pace::Blitz) => "library-online-pace-blitz",
        Some(Pace::Rapid) => "library-online-pace-rapid",
        Some(Pace::Classical) => "library-online-pace-classical",
        Some(Pace::Correspondence) => "library-online-pace-correspondence",
    })
}

fn failure_text(i18n: &I18n, failure: &Failure, now: Timestamp) -> String {
    match failure {
        Failure::Off(site) => i18n.t_with("library-online-fail-off", &[("provider", site.name())]),
        Failure::Limited { until } => i18n.t_with(
            "library-online-fail-limited",
            &[(
                "minutes",
                &crate::explorer::minutes_until(now, *until).to_string(),
            )],
        ),
        Failure::UnknownName(name) => i18n.t_with("library-online-fail-unknown", &[("name", name)]),
        Failure::Offline => i18n.t("library-online-fail-offline"),
        Failure::FairPlay => i18n.t("library-online-fail-fairplay"),
        Failure::NoDatabase => i18n.t("library-online-fail-nodb"),
        Failure::Other(detail) => i18n.t_with("library-online-fail-other", &[("detail", detail)]),
    }
}

/// O formulário completo. Devolve `true` se algo pediu uma nova leitura da Biblioteca.
pub fn draw(ui: &mut egui::Ui, i18n: &I18n, state: &mut OnlineState) {
    let p = palette(ui);
    let site = state.site;
    let name = site.name();
    ui.add(
        egui::Label::new(
            egui::RichText::new(i18n.t_with("library-online-help", &[("provider", name)]))
                .small()
                .color(p.ink_muted),
        )
        .wrap(),
    );
    ui.add_space(8.0);

    if let Some(job) = &state.job {
        running(ui, i18n, job);
        return;
    }

    // Nome público (sugere a Conexão padrão vinculada).
    label(ui, i18n.t("library-online-name-label"));
    let unknown = match &state.finished {
        Some(Finished {
            result: Err(Failure::UnknownName(_)),
            name: asked,
            ..
        }) if asked.eq_ignore_ascii_case(state.name.trim()) => {
            Some(i18n.t_with("library-online-fail-unknown", &[("name", asked)]))
        }
        _ => None,
    };
    cs_text_field(
        ui,
        &mut state.name,
        &i18n.t_with("library-online-name-hint", &[("provider", name)]),
        unknown.as_deref(),
    );
    if let Some(suggestion) = state.suggestion.clone()
        && state.name.trim().is_empty()
    {
        ui.add_space(4.0);
        if ui
            .add(
                CsButton::new(&i18n.t_with("library-online-use-linked", &[("name", &suggestion)]))
                    .icon(icon::USER)
                    .variant(ButtonVariant::Subtle),
            )
            .clicked()
        {
            state.name = suggestion;
        }
    }
    ui.add_space(8.0);

    egui::Grid::new("library-online-filters")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            label(ui, i18n.t("library-online-period"));
            let periods: Vec<(Period, String)> = Period::ALL
                .into_iter()
                .map(|p| (p, i18n.t(p.key())))
                .collect();
            cs_dropdown(ui, "library-online-period", &mut state.period, &periods);
            ui.end_row();

            label(ui, i18n.t("library-online-pace"));
            let paces: Vec<(Option<Pace>, String)> = std::iter::once(None)
                .chain(Pace::ALL.into_iter().map(Some))
                .map(|pace| (pace, pace_label(i18n, pace)))
                .collect();
            cs_dropdown(ui, "library-online-pace", &mut state.pace, &paces);
            ui.end_row();

            label(ui, i18n.t("library-online-max"));
            let maxes: Vec<(u32, String)> = MAX_CHOICES
                .into_iter()
                .map(|n| {
                    (
                        n,
                        i18n.t_with(
                            "library-online-max-n",
                            &[("count", &crate::explorer::fmt_count(u64::from(n)))],
                        ),
                    )
                })
                .collect();
            cs_dropdown(ui, "library-online-max", &mut state.max, &maxes);
            ui.end_row();
        });
    ui.add_space(8.0);
    label(ui, i18n.t("library-online-color"));
    let colors = [
        (None, i18n.t("library-online-color-any")),
        (Some(Color::White), i18n.t("explorer-legend-white")),
        (Some(Color::Black), i18n.t("explorer-legend-black")),
    ];
    cs_segmented(ui, &mut state.color, &colors);
    ui.add_space(12.0);

    actions_row(ui, i18n, state);
    if let Some(finished) = state.finished.clone() {
        ui.add_space(12.0);
        outcome(
            ui,
            i18n,
            &finished,
            state.env.now.unwrap_or(Timestamp::from_millis(0)),
        );
    }
}

fn actions_row(ui: &mut egui::Ui, i18n: &I18n, state: &mut OnlineState) {
    let site = state.site;
    let name = site.name();
    if state.env.locked {
        ui.add(
            egui::Label::new(
                egui::RichText::new(i18n.t("library-online-locked"))
                    .small()
                    .color(palette(ui).ink_muted),
            )
            .wrap(),
        );
        return;
    }
    if !state.env.enabled(site) {
        if ui
            .add(
                CsButton::new(&i18n.t_with("explorer-off-button", &[("provider", name)]))
                    .variant(ButtonVariant::Primary),
            )
            .clicked()
        {
            state.ask_to_turn_on = Some(site);
        }
        ui.add_space(4.0);
        label(
            ui,
            i18n.t_with("library-online-off-note", &[("provider", name)]),
        );
        return;
    }
    let ready = !state.name.trim().is_empty();
    ui.horizontal(|ui| {
        let import = ui.add_enabled(
            ready,
            CsButton::new(&i18n.t("library-online-import")).variant(ButtonVariant::Primary),
        );
        if import.clicked() {
            state.start(false);
        }
        if let Some(day) = state.last_day {
            let update = ui
                .add_enabled(
                    ready,
                    CsButton::new(&i18n.t("library-online-update")).icon(icon::ARROWS_CLOCKWISE),
                )
                .on_hover_text(i18n.t_with(
                    "library-online-update-tip",
                    &[(
                        "date",
                        &format!("{:02}/{:02}/{}", day.day, day.month, day.year),
                    )],
                ));
            if update.clicked() {
                state.start(true);
            }
        }
    });
    ui.add_space(4.0);
    label(ui, i18n.t("library-online-no-auto"));
}

/// Importação em andamento: contagens e Cancelar. A interface segue livre.
fn running(ui: &mut egui::Ui, i18n: &I18n, job: &Job) {
    let p = palette(ui);
    let progress = job.progress();
    ui.ctx().request_repaint_after(Duration::from_millis(100));
    ui.horizontal(|ui| {
        ui.spinner();
        ui.add(
            egui::Label::new(
                egui::RichText::new(i18n.t_with(
                    "library-online-running",
                    &[("name", &job.name), ("provider", job.site.name())],
                ))
                .font(strong_font(ui.ctx(), 14.0)),
            )
            .wrap(),
        );
    });
    ui.add_space(8.0);
    stat_tiles(
        ui,
        [
            (i18n.t("library-online-received"), progress.received),
            (i18n.t("library-online-new"), progress.new),
            (i18n.t("library-online-duplicates"), progress.duplicates),
        ],
    );
    if progress.errors > 0 {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(i18n.t_with(
                "library-online-errors",
                &[("count", &progress.errors.to_string())],
            ))
            .small()
            .color(p.warning),
        );
    }
    ui.add_space(8.0);
    if job.is_cancelling() {
        ui.label(
            egui::RichText::new(i18n.t("library-online-cancelling"))
                .small()
                .color(p.ink_muted),
        );
    } else if ui
        .add(CsButton::new(&i18n.t("library-import-cancel")).variant(ButtonVariant::Secondary))
        .clicked()
    {
        job.cancel();
    }
}

/// O resumo: o mesmo da importação de arquivo (novas, repetidas, com erro) ou o motivo da falha.
fn outcome(ui: &mut egui::Ui, i18n: &I18n, finished: &Finished, now: Timestamp) {
    let p = palette(ui);
    match &finished.result {
        Ok(summary) => {
            let (glyph, color, key) = if summary.cancelled {
                (icon::PROHIBIT, p.warning, "library-online-cancelled")
            } else {
                (icon::CHECK_CIRCLE, p.success, "library-online-done")
            };
            ui.horizontal_top(|ui| {
                ui.label(
                    egui::RichText::new(glyph)
                        .font(icon_font(ui.ctx(), 16.0))
                        .color(color),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(i18n.t_with(
                            key,
                            &[("name", &finished.name), ("provider", finished.site.name())],
                        ))
                        .font(strong_font(ui.ctx(), 14.0)),
                    )
                    .wrap(),
                );
            });
            ui.add_space(8.0);
            stat_tiles(
                ui,
                [
                    (i18n.t("library-online-received"), summary.received),
                    (i18n.t("library-online-new"), summary.new),
                    (i18n.t("library-online-duplicates"), summary.duplicates),
                ],
            );
            if summary.errors > 0 {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(i18n.t_with(
                        "library-online-errors",
                        &[("count", &summary.errors.to_string())],
                    ))
                    .small()
                    .color(p.warning),
                );
            }
            if summary.received == 0 && !summary.cancelled {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(i18n.t("library-online-none"))
                            .small()
                            .color(p.ink_muted),
                    )
                    .wrap(),
                );
            }
        }
        Err(Failure::UnknownName(_)) => {}
        Err(failure) => {
            ui.horizontal_top(|ui| {
                ui.label(
                    egui::RichText::new(icon::WARNING)
                        .font(icon_font(ui.ctx(), 16.0))
                        .color(p.warning),
                );
                ui.add(egui::Label::new(failure_text(i18n, failure, now)).wrap());
            });
        }
    }
}
