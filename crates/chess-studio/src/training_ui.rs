//! Tela de Treino (B13): lista de Exercícios por tipo, Resolução sobre o tabuleiro,
//! revisão com explicação, "Repetir" e métricas.
//!
//! A Partida de um Exercício ocupa o lugar da [`Session`] enquanto o Treino está aberto;
//! a Partida em andamento fica guardada e volta intacta ao sair (ADR-0001: só a `Session`
//! escreve a Partida; o juiz [`ExerciseRun`] apenas lê os Eventos e pede desfazer ou a
//! resposta do adversário). Nada do Treino é gravado como Partida do histórico, então o
//! treino não pode alterar resultados competitivos (TRN-05). Texto vem do i18n (Fluent).

use crate::aids::{AidSettings, ErrorExplanation, NoticeView, notice_view, reason_key};
use crate::capture::CaptureMode;
use crate::i18n::I18n;
use crate::persistence::PersistenceHandle;
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::{Color as Side, Square};
use chess_game::Command;
use chess_store::{ProfileId, RunId, RunQuery, Store};
use chess_training::{
    Aid, AidsRecord, Catalog, ErrorCause, Exercise, ExerciseKind, ExerciseRun, Feedback, Metrics,
    Review, RunError, RunStatus, RunSummary, Tally,
};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Rodadas máximas de "juiz pede, `Session` aplica" por quadro.
const MAX_ROUNDS: usize = 16;

pub struct TrainingPlugin;

impl Plugin for TrainingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainingUi>()
            .add_systems(
                Update,
                (training_demo, training_judge).chain().after(PlaySet),
            )
            .add_systems(EguiPrimaryContextPass, training_window);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Closed,
    List,
    Exercise,
    Review,
}

#[derive(Debug)]
struct ActiveRun {
    run: ExerciseRun,
    run_id: Option<RunId>,
    /// "Por quê?" pedido para o erro atual (feedback "após tentativa").
    revealed: bool,
    shown_errors: usize,
}

#[derive(Debug)]
struct ReviewState {
    review: Review,
    exercise: Exercise,
}

/// Estado da tela de Treino.
#[derive(Resource)]
pub struct TrainingUi {
    view: View,
    catalog: Catalog,
    /// Banco próprio quando o app roda sem persistência (capturas, banco indisponível).
    local: Option<Mutex<Store>>,
    synced: bool,
    /// A Partida em andamento, guardada enquanto a `Session` mostra o Exercício.
    stashed: Option<Session>,
    active: Option<ActiveRun>,
    review: Option<ReviewState>,
    metrics: Metrics,
    /// Identifica a sessão do app nas métricas.
    session_id: u64,
    clock: Instant,
}

impl std::fmt::Debug for TrainingUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrainingUi")
            .field("view", &self.view)
            .field("exercises", &self.catalog.len())
            .finish_non_exhaustive()
    }
}

impl Default for TrainingUi {
    fn default() -> Self {
        let catalog = Catalog::bundled().unwrap_or_else(|err| {
            error!("catálogo de Exercícios inválido: {err}");
            Catalog::default()
        });
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .unwrap_or(0);
        Self {
            view: View::Closed,
            catalog,
            local: None,
            synced: false,
            stashed: None,
            active: None,
            review: None,
            metrics: Metrics::default(),
            session_id: wall,
            clock: Instant::now(),
        }
    }
}

/// Auxílios ligados no Perfil de auxílios do app (TRN-03).
fn aids_record(aids: &AidSettings) -> AidsRecord {
    let mut enabled = Vec::new();
    for (on, aid) in [
        (aids.destinations, Aid::Destinations),
        (aids.ghosts, Aid::Ghosts),
        (aids.selected, Aid::SelectedPiece),
        (aids.last_move, Aid::LastMove),
        (aids.check, Aid::Check),
    ] {
        if on {
            enabled.push(aid);
        }
    }
    let feedback = match aids.explanation {
        ErrorExplanation::Immediate => Feedback::Immediate,
        ErrorExplanation::AfterAttempt => Feedback::AfterAttempt,
        ErrorExplanation::ReviewOnly => Feedback::ReviewOnly,
    };
    AidsRecord::new(&enabled, feedback)
}

fn aid_key(aid: Aid) -> &'static str {
    match aid {
        Aid::Destinations => "aid-destinations",
        Aid::Ghosts => "aid-ghosts",
        Aid::SelectedPiece => "aid-selected",
        Aid::LastMove => "aid-last-move",
        Aid::Check => "aid-check",
    }
}

fn kind_key(kind: ExerciseKind) -> String {
    format!("training-kind-{}", kind.as_str())
}

/// Banco do Treino: o do app, ou um em memória quando não há persistência.
fn with_store<R>(
    local: &mut Option<Mutex<Store>>,
    handle: Option<&PersistenceHandle>,
    f: impl FnOnce(&mut Store, ProfileId) -> R,
) -> Option<R> {
    if let Some(handle) = handle
        && let Ok(mut persistence) = handle.0.lock()
    {
        let (store, profile) = persistence.store_and_profile();
        return Some(f(store, profile));
    }
    if local.is_none() {
        *local = Store::open_in_memory().ok().map(Mutex::new);
    }
    let store = local.as_mut()?.get_mut().ok()?;
    let profile = store.default_profile().ok()?;
    Some(f(store, profile))
}

impl TrainingUi {
    /// A `Session` mostra a Partida de um Exercício (a persistência não a grava).
    pub fn session_swapped(&self) -> bool {
        self.stashed.is_some()
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Espelha o catálogo no SQLite uma vez por execução.
    fn sync_catalog(&mut self, handle: Option<&PersistenceHandle>) {
        if self.synced {
            return;
        }
        let all: Vec<Exercise> = self.catalog.iter().cloned().collect();
        let done = with_store(&mut self.local, handle, |store, _| {
            store.upsert_exercises(&all)
        });
        match done {
            Some(Ok(_)) => self.synced = true,
            Some(Err(err)) => warn!("Treino: não foi possível gravar o catálogo: {err}"),
            None => {}
        }
        self.refresh_metrics(handle);
    }

    fn refresh_metrics(&mut self, handle: Option<&PersistenceHandle>) {
        let metrics = with_store(&mut self.local, handle, |store, profile| {
            store.metrics(&RunQuery {
                profile: Some(profile),
                ..RunQuery::default()
            })
        });
        if let Some(Ok(metrics)) = metrics {
            self.metrics = metrics;
        }
    }

    fn open(&mut self, handle: Option<&PersistenceHandle>) {
        self.sync_catalog(handle);
        self.view = if self.active.is_some() {
            View::Exercise
        } else {
            View::List
        };
    }

    /// Sai do Exercício e volta a mostrar a Partida que estava em andamento.
    fn leave(&mut self, session: &mut Session, view: View) {
        self.active = None;
        self.review = None;
        self.view = view;
        if let Some(mut original) = self.stashed.take() {
            original.set_move_method(session.move_method());
            original.supersede(session);
            *session = original;
        }
    }

    /// Começa uma Resolução nova na posição **original** do Exercício (também é o "Repetir").
    fn start(
        &mut self,
        id: &str,
        session: &mut Session,
        aids: &AidSettings,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(exercise) = self.catalog.get(id).cloned() else {
            return;
        };
        let Ok(game) = exercise.new_game() else {
            return;
        };
        let mut fresh = Session::from_game(game);
        fresh.set_move_method(session.move_method());
        fresh.supersede(session);
        let previous = std::mem::replace(session, fresh);
        if self.stashed.is_none() {
            self.stashed = Some(previous);
        }
        let session_id = self.session_id;
        let run_id = with_store(&mut self.local, handle, |store, profile| {
            store.start_run(profile, &exercise.id, session_id)
        })
        .and_then(|r| {
            r.map_err(|e| warn!("Treino: Resolução sem registro: {e}"))
                .ok()
        });
        let run = ExerciseRun::start(&exercise, aids_record(aids), self.now_ms());
        self.active = Some(ActiveRun {
            run,
            run_id,
            revealed: false,
            shown_errors: 0,
        });
        self.review = None;
        self.view = View::Exercise;
    }

    /// Encerra a Resolução (acertou ou desistiu): grava o registro imutável e abre a revisão.
    fn finish(&mut self, session: &mut Session, handle: Option<&PersistenceHandle>) {
        let Some(active) = self.active.take() else {
            return;
        };
        let exercise = active.run.exercise().clone();
        let summary: RunSummary = active
            .run
            .finish(session.game(), self.now_ms())
            .in_session(self.session_id);
        if let Some(run_id) = active.run_id {
            let stored = with_store(&mut self.local, handle, |store, _| {
                store.finish_run(run_id, &summary)
            });
            if let Some(Err(err)) = stored {
                warn!("Treino: não foi possível gravar a Resolução: {err}");
            }
        }
        let review = Review::new(&exercise, &summary);
        self.review = Some(ReviewState { review, exercise });
        // Revisão: a Partida do Exercício fica só para leitura.
        let plies = session.snapshot().history.len();
        session.set_review_ply(Some(plies));
        self.view = View::Review;
        self.refresh_metrics(handle);
    }
}

/// Juiz: lê os Eventos novos da `Session` e aplica nela o que a Resolução pede.
fn training_judge(
    mut state: ResMut<TrainingUi>,
    mut session: ResMut<Session>,
    aids: Res<AidSettings>,
    handle: Option<Res<PersistenceHandle>>,
) {
    if state.active.is_some() {
        state.judge(&mut session, &aids, handle.as_deref());
    }
}

impl TrainingUi {
    fn judge(
        &mut self,
        session: &mut Session,
        aids: &AidSettings,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.run.note_aids(&aids_record(aids));
        for _ in 0..MAX_ROUNDS {
            let followups = active.run.observe(session.game());
            if followups.is_empty() {
                break;
            }
            for followup in followups {
                for command in followup.commands() {
                    session.command(command);
                }
            }
        }
        if active.run.errors().len() != active.shown_errors {
            active.shown_errors = active.run.errors().len();
            active.revealed = false;
        }
        if active.run.status() == RunStatus::Solved {
            self.finish(session, handle);
        }
    }
}

enum Action {
    Open,
    Close,
    Start(String),
    GiveUp,
    BackToList,
    Reveal,
}

fn error_text(i18n: &I18n, error: &RunError) -> String {
    match &error.cause {
        ErrorCause::IllegalMove { from, to, reason } => i18n.t_with(
            "training-error-illegal",
            &[
                ("from", from),
                ("to", to),
                ("reason", &i18n.t(reason_key(*reason))),
            ],
        ),
        ErrorCause::OffObjective { uci } => {
            i18n.t_with("training-error-off-objective", &[("uci", uci)])
        }
    }
}

fn feedback_mode_key(feedback: Feedback) -> &'static str {
    match feedback {
        Feedback::Immediate => "aid-explanation-immediate",
        Feedback::AfterAttempt => "aid-explanation-after-attempt",
        Feedback::ReviewOnly => "aid-explanation-review-only",
    }
}

fn aid_list(i18n: &I18n, aids: &[Aid]) -> String {
    aids.iter()
        .map(|a| i18n.t(aid_key(*a)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tally_line(i18n: &I18n, name: &str, t: &Tally) -> String {
    i18n.t_with(
        "training-metrics-line",
        &[
            ("name", name),
            ("runs", &t.runs.to_string()),
            ("correct", &t.correct.to_string()),
            ("errors", &t.errors.to_string()),
            ("noaids", &t.correct_without_aids.to_string()),
        ],
    )
}

fn metrics_section(ui: &mut egui::Ui, i18n: &I18n, metrics: &Metrics, session_id: u64) {
    ui.collapsing(i18n.t("training-metrics-heading"), |ui| {
        if metrics.total.runs == 0 {
            ui.label(i18n.t("training-metrics-empty"));
            return;
        }
        let t = &metrics.total;
        ui.label(i18n.t_with(
            "training-metrics-total",
            &[
                ("runs", &t.runs.to_string()),
                ("correct", &t.correct.to_string()),
                ("errors", &t.errors.to_string()),
                ("noaids", &t.correct_without_aids.to_string()),
            ],
        ));
        ui.strong(i18n.t("training-metrics-by-kind"));
        for (kind, tally) in &metrics.by_kind {
            ui.label(tally_line(i18n, &i18n.t(&kind_key(*kind)), tally));
        }
        ui.strong(i18n.t("training-metrics-by-session"));
        for (n, (id, tally)) in metrics.by_session.iter().rev().enumerate() {
            let name = if *id == session_id {
                i18n.t("training-metrics-session-current")
            } else {
                i18n.t_with(
                    "training-metrics-session-previous",
                    &[("n", &(n + 1).to_string())],
                )
            };
            ui.label(tally_line(i18n, &name, tally));
        }
    });
}

fn list_view(ui: &mut egui::Ui, i18n: &I18n, state: &TrainingUi) -> Option<Action> {
    let mut action = None;
    ui.strong(i18n.t("training-list-heading"));
    egui::ScrollArea::vertical()
        .max_height(260.0)
        .show(ui, |ui| {
            for kind in ExerciseKind::ALL {
                let exercises: Vec<&Exercise> =
                    state.catalog.iter().filter(|e| e.kind == kind).collect();
                if exercises.is_empty() {
                    continue;
                }
                let kind_tally = state
                    .metrics
                    .by_kind
                    .get(&kind)
                    .copied()
                    .unwrap_or_default();
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        i18n.t(&kind_key(kind)),
                        kind_tally.correct
                    ))
                    .strong(),
                );
                for exercise in exercises {
                    let tally = state
                        .metrics
                        .by_exercise
                        .get(&exercise.id)
                        .copied()
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        if ui.button(i18n.t("training-start")).clicked() {
                            action = Some(Action::Start(exercise.id.clone()));
                        }
                        ui.label(format!(
                            "{} — {}",
                            exercise.title,
                            i18n.t_with(
                                "training-exercise-count",
                                &[
                                    ("correct", &tally.correct.to_string()),
                                    ("runs", &tally.runs.to_string()),
                                ],
                            )
                        ));
                    });
                }
            }
        });
    action
}

fn exercise_view(
    ui: &mut egui::Ui,
    i18n: &I18n,
    active: &ActiveRun,
    session: &Session,
    aids: &AidSettings,
) -> Option<Action> {
    let mut action = None;
    let exercise = active.run.exercise();
    ui.strong(&exercise.title);
    ui.label(i18n.t_with("training-objective", &[("text", &exercise.objective)]));
    ui.label(i18n.t(match session.snapshot().side_to_move {
        Side::White => "training-side-white",
        Side::Black => "training-side-black",
    }));
    ui.small(i18n.t("training-hint-undo"));
    let errors = active.run.errors();
    if let Some(last) = errors.last() {
        // O feedback segue a preferência de explicação (imediata, após tentativa, só revisão).
        match notice_view(aids.explanation, active.revealed) {
            NoticeView::Hidden => {
                ui.small(i18n.t("training-feedback-hidden"));
            }
            NoticeView::Brief => {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, i18n.t("training-error-brief"));
                    if ui.small_button(i18n.t("aid-why")).clicked() {
                        action = Some(Action::Reveal);
                    }
                });
            }
            NoticeView::WithReason => {
                ui.colored_label(egui::Color32::YELLOW, error_text(i18n, last));
            }
        }
        if aids.explanation != ErrorExplanation::ReviewOnly {
            ui.label(i18n.t_with(
                "training-errors-count",
                &[("count", &errors.len().to_string())],
            ));
        }
    }
    ui.horizontal(|ui| {
        if ui.button(i18n.t("training-give-up")).clicked() {
            action = Some(Action::GiveUp);
        }
        if ui.button(i18n.t("training-back")).clicked() {
            action = Some(Action::BackToList);
        }
    });
    action
}

fn review_view(ui: &mut egui::Ui, i18n: &I18n, review: &ReviewState) -> Option<Action> {
    let mut action = None;
    let r = &review.review;
    ui.strong(i18n.t("training-review-heading"));
    ui.strong(&r.title);
    let (color, key) = match r.result {
        chess_training::RunResult::Correct => {
            (egui::Color32::LIGHT_GREEN, "training-result-correct")
        }
        chess_training::RunResult::Incorrect => {
            (egui::Color32::LIGHT_RED, "training-result-incorrect")
        }
    };
    ui.colored_label(color, i18n.t(key));
    if let Some(outcome) = r.outcome {
        ui.label(i18n.t_with(
            "training-review-outcome",
            &[("outcome", &crate::ui::outcome_text(i18n, outcome))],
        ));
    }
    ui.label(i18n.t_with("training-review-explanation", &[("text", &r.explanation)]));
    if !r.solution_san.is_empty() {
        ui.label(i18n.t_with(
            "training-review-solution",
            &[("san", &r.solution_san.join(" "))],
        ));
    }
    if let Some(why) = r.not_checkmate {
        ui.separator();
        ui.strong(i18n.t("training-not-mate-heading"));
        if !why.king_in_check && why.legal_moves == 0 {
            ui.label(i18n.t("training-not-mate-no-check"));
        }
        ui.label(i18n.t("training-not-mate-rule"));
    }
    ui.separator();
    if r.errors.is_empty() {
        ui.label(i18n.t("training-review-no-errors"));
    } else {
        ui.label(i18n.t_with(
            "training-review-errors",
            &[("count", &r.errors.len().to_string())],
        ));
        for (i, error) in r.errors.iter().enumerate() {
            ui.label(i18n.t_with(
                "training-review-error-line",
                &[
                    ("n", &(i + 1).to_string()),
                    ("text", &error_text(i18n, error)),
                ],
            ));
        }
    }
    if r.aids.without_aids() {
        ui.label(i18n.t("training-review-aids-none"));
    } else {
        ui.label(i18n.t_with(
            "training-review-aids-used",
            &[("list", &aid_list(i18n, &r.aids.used))],
        ));
    }
    if !r.aids.off.is_empty() {
        ui.label(i18n.t_with(
            "training-review-aids-off",
            &[("list", &aid_list(i18n, &r.aids.off))],
        ));
    }
    ui.label(i18n.t_with(
        "training-review-feedback",
        &[("mode", &i18n.t(feedback_mode_key(r.aids.feedback)))],
    ));
    ui.label(i18n.t_with(
        "training-review-duration",
        &[("seconds", &(r.duration_ms / 1000).to_string())],
    ));
    ui.horizontal(|ui| {
        if ui.button(i18n.t("training-repeat")).clicked() {
            action = Some(Action::Start(review.exercise.id.clone()));
        }
        if ui.button(i18n.t("training-back")).clicked() {
            action = Some(Action::BackToList);
        }
    });
    action
}

fn training_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<TrainingUi>,
    mut session: ResMut<Session>,
    aids: Res<AidSettings>,
    handle: Option<Res<PersistenceHandle>>,
    capture: Option<Res<CaptureMode>>,
) -> Result {
    if capture
        .as_ref()
        .is_some_and(|c| !c.show_panel && demo_script().is_none())
    {
        return Ok(());
    }
    let mut action = None;
    egui::Window::new(i18n.t("training-window-title"))
        .anchor(egui::Align2::LEFT_BOTTOM, [8.0, -8.0])
        .default_width(330.0)
        .show(contexts.ctx_mut()?, |ui| match state.view {
            View::Closed => {
                if ui.button(i18n.t("training-open")).clicked() {
                    action = Some(Action::Open);
                }
            }
            view => {
                let inner = match view {
                    View::List => list_view(ui, &i18n, &state),
                    View::Exercise => state
                        .active
                        .as_ref()
                        .and_then(|a| exercise_view(ui, &i18n, a, &session, &aids)),
                    _ => state
                        .review
                        .as_ref()
                        .and_then(|r| review_view(ui, &i18n, r)),
                };
                if inner.is_some() {
                    action = inner;
                }
                ui.separator();
                metrics_section(ui, &i18n, &state.metrics, state.session_id);
                ui.small(i18n.t("training-note-no-effect"));
                if ui.button(i18n.t("training-close")).clicked() {
                    action = Some(Action::Close);
                }
            }
        });
    let handle = handle.as_deref();
    match action {
        None => {}
        Some(Action::Open) => state.open(handle),
        Some(Action::Close) => state.leave(&mut session, View::Closed),
        Some(Action::Start(id)) => state.start(&id, &mut session, &aids, handle),
        Some(Action::GiveUp) => state.finish(&mut session, handle),
        // Sair sem resolver nem desistir descarta a Resolução (não vira erro).
        Some(Action::BackToList) => state.leave(&mut session, View::List),
        Some(Action::Reveal) => {
            if let Some(active) = state.active.as_mut() {
                active.revealed = true;
            }
        }
    }
    Ok(())
}

// --- Evidências (`--screenshot <dir> --training list|exercise|review`) ----------------

fn demo_script() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--training" {
            return args.next();
        }
    }
    None
}

fn square(name: &str) -> Option<Square> {
    name.parse().ok()
}

fn demo_move(session: &mut Session, uci: &str) {
    if let (Some(from), Some(to)) = (square(&uci[0..2]), square(&uci[2..4])) {
        session.command(Command::Release { from, to });
    }
}

/// Roteiro de captura: só existe em `--screenshot`, com `--training` (nunca em uso normal).
fn training_demo(
    mut state: ResMut<TrainingUi>,
    mut session: ResMut<Session>,
    aids: Res<AidSettings>,
    capture: Option<Res<CaptureMode>>,
    mut frame: Local<u32>,
) {
    if capture.is_none() {
        return;
    }
    let Some(script) = demo_script() else { return };
    *frame += 1;
    match *frame {
        3 => {
            state.sync_catalog(None);
            // Histórico de demonstração para a lista e as métricas.
            seed_demo_runs(&mut state);
            state.refresh_metrics(None);
            state.view = View::List;
            if script != "list" {
                state.start("afogamento-001", &mut session, &aids, None);
            }
        }
        6 if script != "list" => demo_move(&mut session, "e6e8"), // mate: erro
        14 if script == "review" => demo_move(&mut session, "e6f7"), // afogamento
        _ => {}
    }
}

fn seed_demo_runs(state: &mut TrainingUi) {
    let session_id = state.session_id.saturating_sub(1);
    let plan: [(&str, &[&str]); 3] = [
        ("roque-001", &["e1g1"]),
        ("mate-em-1-001", &["a1a7", "a1a8"]),
        ("afogamento-001", &["e6e8"]),
    ];
    for (id, moves) in plan {
        let Some(exercise) = state.catalog.get(id).cloned() else {
            continue;
        };
        let Ok(mut game) = exercise.new_game() else {
            continue;
        };
        let mut run = ExerciseRun::start(&exercise, AidsRecord::new(&[], Feedback::Immediate), 0);
        for (i, uci) in moves.iter().enumerate() {
            let at = 100 * (i as u64 + 1);
            if let (Some(from), Some(to)) = (square(&uci[0..2]), square(&uci[2..4])) {
                let _ = game.handle(
                    Command::Release { from, to },
                    chess_game::Timestamp::mono(at),
                );
                run.drive(&mut game, at);
            }
        }
        let summary = run.finish(&game, 4_000).in_session(session_id);
        let _ = with_store(&mut state.local, None, |store, profile| {
            store
                .start_run(profile, id, session_id)
                .and_then(|run_id| store.finish_run(run_id, &summary))
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn play(session: &mut Session, uci: &str) {
        demo_move(session, uci);
    }

    /// Fluxo do Treino sobre a `Session`: erro desfeito, acerto, revisão, repetir e sair
    /// devolvendo a Partida em andamento intacta (TRN-01/02, TRN-05).
    #[test]
    fn exercise_flow_keeps_the_ongoing_game_and_never_persists_it_as_a_game() {
        let mut state = TrainingUi::default();
        let aids = AidSettings::physical();
        let mut session = Session::new();
        play(&mut session, "e2e4");
        let before = session.snapshot().clone();
        let events_before = session.game().events().len();
        let epoch = session.epoch();

        state.open(None);
        assert_eq!(state.view, View::List);
        assert!(!state.session_swapped());
        state.start("afogamento-001", &mut session, &aids, None);
        assert!(
            state.session_swapped(),
            "a persistência não grava o Exercício"
        );
        let original_fen = state.catalog.get("afogamento-001").unwrap().fen.clone();
        assert_eq!(session.snapshot().position.to_fen(), original_fen);

        // Dá mate em vez de afogar: erro, posição restaurada.
        play(&mut session, "e6e8");
        state.judge(&mut session, &aids, None);
        assert_eq!(state.view, View::Exercise);
        assert_eq!(state.active.as_ref().unwrap().run.errors().len(), 1);
        assert_eq!(session.snapshot().position.to_fen(), original_fen);

        // Desistir abre a revisão com a solução; a Resolução fica gravada como erro.
        state.finish(&mut session, None);
        assert_eq!(state.view, View::Review);
        assert_eq!(state.metrics.total.incorrect, 1);
        assert_eq!(state.metrics.total.errors, 1);
        assert!(session.review().is_some(), "revisão é só leitura");

        // Repetir: posição original, outra Resolução; afogamento resolve.
        state.start("afogamento-001", &mut session, &aids, None);
        assert_eq!(session.snapshot().position.to_fen(), original_fen);
        play(&mut session, "e6f7");
        state.judge(&mut session, &aids, None);
        assert_eq!(state.view, View::Review);
        let review = &state.review.as_ref().unwrap().review;
        assert!(review.not_checkmate.is_some());
        assert_eq!(state.metrics.total.runs, 2);
        assert_eq!(state.metrics.total.correct, 1);
        assert_eq!(state.metrics.by_kind[&ExerciseKind::Stalemate].runs, 2);
        // Auxílios do Perfil "Jogo físico": todos desligados e registrados.
        assert!(review.aids.without_aids());
        assert_eq!(review.aids.off.len(), 5);

        // Sair devolve a Partida de antes, sem nenhum Evento a mais.
        state.leave(&mut session, View::Closed);
        assert!(!state.session_swapped());
        assert_eq!(session.snapshot(), &before);
        assert_eq!(session.game().events().len(), events_before);
        assert_eq!(session.epoch(), epoch);
    }

    #[test]
    fn feedback_follows_the_explanation_preference() {
        let training = AidSettings::training();
        let mut after = AidSettings::training();
        after.explanation = ErrorExplanation::AfterAttempt;
        assert_eq!(aids_record(&training).feedback, Feedback::Immediate);
        assert_eq!(aids_record(&after).feedback, Feedback::AfterAttempt);
        assert_eq!(
            aids_record(&AidSettings::physical()).feedback,
            Feedback::ReviewOnly
        );
        assert!(aids_record(&AidSettings::physical()).without_aids());
        assert_eq!(
            aids_record(&training).used,
            [
                Aid::Destinations,
                Aid::SelectedPiece,
                Aid::LastMove,
                Aid::Check
            ]
        );
        assert_eq!(
            notice_view(ErrorExplanation::ReviewOnly, false),
            NoticeView::Hidden
        );
    }
}
