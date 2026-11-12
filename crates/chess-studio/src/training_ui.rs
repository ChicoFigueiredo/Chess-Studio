//! Tela de Treino (B13): lista de Exercícios por tipo, Resolução sobre o tabuleiro,
//! revisão com explicação, "Repetir" e métricas.
//!
//! A Partida de um Exercício ocupa o lugar da [`Session`] enquanto o Treino está aberto;
//! a Partida em andamento fica guardada e volta intacta ao sair (ADR-0001: só a `Session`
//! escreve a Partida; o juiz [`ExerciseRun`] apenas lê os Eventos e pede desfazer ou a
//! resposta do adversário). Nada do Treino é gravado como Partida do histórico, então o
//! treino não pode alterar resultados competitivos (TRN-05). Texto vem do i18n (Fluent).
//!
//! Memória de sequência (B15, TRN-04): a **apresentação** anima a sequência numa
//! `Session` só de leitura (a revisão Lance a Lance avança no ritmo do Exercício); depois
//! a **reprodução** troca por uma `Session` nova na posição inicial, onde a pessoa joga os
//! dois lados com a movimentação normal e o [`MemoryRun`] julga cada Lance.

use crate::aids::{AidSettings, ErrorExplanation, NoticeView, notice_view, reason_key};
use crate::capture::CaptureMode;
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::play::PlaySet;
use crate::session::Session;
use crate::ui_theme::{ButtonVariant, cs_button};
use bevy::prelude::*;
use bevy_egui::{EguiPrimaryContextPass, egui};
use chess_core::{Color as Side, Square};
use chess_game::Command;
use chess_store::{ProfileId, RunId, RunQuery, Store};
use chess_training::{
    Aid, AidsRecord, Catalog, ErrorCause, Exercise, ExerciseKind, ExerciseRun, Feedback,
    MemoryPhase, MemoryRun, Metrics, PlyResult, ProfileSet, Review, RunError, RunStatus,
    RunSummary, Solution, Tally,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

mod download;
mod evolution;
mod packs;
mod position;
mod profiles;
mod themes;

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
            .add_systems(Update, (position::tick, position::sync_board).chain())
            .add_systems(Update, (packs::poll, download::poll, download::on_consent))
            .add_systems(Update, download::demo_system)
            .add_systems(EguiPrimaryContextPass, position::editor);
        crate::dock::register_panel(
            app,
            crate::dock::PanelDef {
                id: crate::dock::ids::TRAINING,
                label_key: "dock-panel-training",
                access: crate::dock::PanelAccess::Always,
                ui: training_panel,
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Closed,
    List,
    Exercise,
    Review,
    /// Memória de posição: estudo, reconstrução e revisão (C11).
    Position,
    /// Perfis de treino (C11).
    Profiles,
    /// Aba Evolução (C11).
    Evolution,
    /// Pacotes de Exercícios: instalar do Lichess, listar e remover (#217).
    Packs,
}

#[derive(Debug)]
struct ActiveRun {
    run: ExerciseRun,
    run_id: Option<RunId>,
    /// "Por quê?" pedido para o erro atual (feedback "após tentativa").
    revealed: bool,
    shown_errors: usize,
}

/// Resolução de memória em curso (apresentação ou reprodução).
#[derive(Debug)]
struct MemoryActive {
    run: MemoryRun,
    run_id: Option<RunId>,
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
    memory: Option<MemoryActive>,
    review: Option<ReviewState>,
    /// Memória de posição em andamento (C11).
    position: Option<position::PositionState>,
    /// Perfis de treino, avanço por Perfil e o Perfil seguido (C11).
    profiles: ProfileSet,
    progress: BTreeMap<String, BTreeSet<String>>,
    active_profile: Option<String>,
    evolution_data: evolution::EvolutionData,
    evolution_view: evolution::EvolutionView,
    /// Pacotes de Exercícios instalados, filtros da lista e leitura em andamento.
    packs: packs::PacksUi,
    metrics: Metrics,
    /// Identifica a sessão do app nas métricas.
    session_id: u64,
    clock: Instant,
    /// Avanço artificial do relógio (só nas capturas e nos testes).
    clock_skew_ms: u64,
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
        let profiles = ProfileSet::bundled(&catalog).unwrap_or_else(|err| {
            error!("Perfis de treino inválidos: {err}");
            ProfileSet::default()
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
            memory: None,
            review: None,
            position: None,
            profiles,
            progress: BTreeMap::new(),
            active_profile: None,
            evolution_data: evolution::EvolutionData::default(),
            evolution_view: evolution::EvolutionView::default(),
            packs: packs::PacksUi::default(),
            metrics: Metrics::default(),
            session_id: wall,
            clock: Instant::now(),
            clock_skew_ms: 0,
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
pub(crate) fn with_store<R>(
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
        u64::try_from(self.clock.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .saturating_add(self.clock_skew_ms)
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
        self.refresh_profiles(handle);
        self.refresh_evolution(handle);
    }

    fn open(&mut self, handle: Option<&PersistenceHandle>) {
        self.reload_packs(handle);
        self.sync_catalog(handle);
        self.view = if self.active.is_some() || self.memory.is_some() {
            View::Exercise
        } else {
            View::List
        };
    }

    /// Troca de tela dentro do Treino (lista, Perfis, Evolução).
    fn show(&mut self, view: View, session: &mut Session, handle: Option<&PersistenceHandle>) {
        if view == View::List {
            self.leave(session, View::List);
        } else {
            self.refresh_metrics(handle);
            self.view = view;
        }
    }

    /// Sai do Exercício e volta a mostrar a Partida que estava em andamento.
    fn leave(&mut self, session: &mut Session, view: View) {
        self.active = None;
        self.memory = None;
        self.review = None;
        self.position = None;
        self.view = view;
        if let Some(mut original) = self.stashed.take() {
            original.set_move_method(session.move_method());
            original.supersede(session);
            *session = original;
        }
    }

    /// Coloca `fresh` no lugar da `Session` do app, guardando a Partida em andamento.
    fn install(&mut self, session: &mut Session, mut fresh: Session) {
        fresh.set_move_method(session.move_method());
        fresh.supersede(session);
        let previous = std::mem::replace(session, fresh);
        if self.stashed.is_none() {
            self.stashed = Some(previous);
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
        if exercise.kind == ExerciseKind::MemoryPosition {
            return self.start_position(id, session, aids, handle);
        }
        self.position = None;
        let is_memory = matches!(exercise.solution, Solution::Memorize { .. });
        let fresh = if is_memory {
            presentation_session(&exercise)
        } else {
            exercise.new_game().ok().map(Session::from_game)
        };
        let Some(fresh) = fresh else {
            return;
        };
        self.install(session, fresh);
        let session_id = self.session_id;
        let run_id = with_store(&mut self.local, handle, |store, profile| {
            store.start_run(profile, &exercise.id, session_id)
        })
        .and_then(|r| {
            r.map_err(|e| warn!("Treino: Resolução sem registro: {e}"))
                .ok()
        });
        self.review = None;
        if is_memory {
            let run = MemoryRun::start(&exercise, aids_record(aids), self.now_ms());
            self.memory = Some(MemoryActive { run, run_id });
            self.active = None;
            self.view = View::Exercise;
            return;
        }
        let run = ExerciseRun::start(&exercise, aids_record(aids), self.now_ms());
        self.memory = None;
        self.active = Some(ActiveRun {
            run,
            run_id,
            revealed: false,
            shown_errors: 0,
        });
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
        self.advance_profiles(&summary, handle);
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
    if state.memory.is_some() {
        state.memory_step(&mut session, &aids, handle.as_deref());
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
    ShowAgain,
    Open,
    Close,
    Start(String),
    GiveUp,
    BackToList,
    Reveal,
    /// Comando do editor de memória de posição.
    Position(position::Cmd),
    /// Vai para outra tela do Treino (Perfis, Evolução, lista).
    Show(View),
    /// Segue o Perfil de treino e resolve o próximo Exercício dele.
    FollowAndStart(String, String),
    Unfollow,
    /// Comando da tela de Pacotes.
    Packs(packs::Cmd),
}

fn promotion_piece(uci: &str) -> Option<chess_core::Piece> {
    match uci.get(4..5)? {
        "q" => Some(chess_core::Piece::Queen),
        "r" => Some(chess_core::Piece::Rook),
        "b" => Some(chess_core::Piece::Bishop),
        "n" => Some(chess_core::Piece::Knight),
        _ => None,
    }
}

/// `Session` da apresentação: a sequência inteira já jogada, mostrada Lance a Lance pela
/// revisão (somente leitura, sem gestos). Começa na posição inicial.
fn presentation_session(exercise: &Exercise) -> Option<Session> {
    let Solution::Memorize { line, .. } = &exercise.solution else {
        return None;
    };
    let mut session = Session::from_game(exercise.new_game().ok()?);
    for uci in line {
        demo_move(&mut session, uci);
        if let Some(piece) = promotion_piece(uci) {
            session.command(Command::ChoosePromotion { piece });
        }
    }
    session.set_review_ply(Some(0));
    Some(session)
}

/// `Session` da reprodução: Partida nova na posição inicial do Exercício.
fn recall_session(exercise: &Exercise) -> Option<Session> {
    exercise.new_game().ok().map(Session::from_game)
}

impl TrainingUi {
    /// Avança a Resolução de memória: anima a apresentação, passa à reprodução e julga.
    fn memory_step(
        &mut self,
        session: &mut Session,
        aids: &AidSettings,
        handle: Option<&PersistenceHandle>,
    ) {
        let now = self.now_ms();
        let Some(memory) = self.memory.as_mut() else {
            return;
        };
        memory.run.note_aids(&aids_record(aids));
        match memory.run.phase() {
            MemoryPhase::Presentation => {
                session.set_review_ply(Some(memory.run.presentation_ply(now)));
                if memory.run.presentation_finished(now) {
                    self.begin_recall(session);
                }
            }
            MemoryPhase::Recall => {
                for _ in 0..MAX_ROUNDS {
                    let followups = memory.run.observe(session.game());
                    if followups.is_empty() {
                        break;
                    }
                    for followup in followups {
                        for command in followup.commands() {
                            session.command(command);
                        }
                    }
                }
                if memory.run.status() == RunStatus::Solved {
                    self.finish_memory(session, handle);
                }
            }
            MemoryPhase::Done => {}
        }
    }

    /// Volta ao início: Partida nova na posição inicial, onde a pessoa reproduz.
    fn begin_recall(&mut self, session: &mut Session) {
        let Some(memory) = self.memory.as_mut() else {
            return;
        };
        memory.run.begin_recall();
        if let Some(fresh) = recall_session(memory.run.exercise()) {
            self.install(session, fresh);
        }
    }

    /// "Mostrar de novo": a sequência é apresentada outra vez; o já julgado fica.
    fn show_again(&mut self, session: &mut Session) {
        let now = self.now_ms();
        let Some(memory) = self.memory.as_mut() else {
            return;
        };
        memory.run.show_again(now);
        if let Some(fresh) = presentation_session(memory.run.exercise()) {
            self.install(session, fresh);
        }
    }

    /// Encerra a Resolução de memória (sequência completa ou desistência).
    fn finish_memory(&mut self, session: &mut Session, handle: Option<&PersistenceHandle>) {
        if self
            .memory
            .as_ref()
            .is_some_and(|m| m.run.phase() == MemoryPhase::Presentation)
        {
            self.begin_recall(session);
        }
        let Some(memory) = self.memory.take() else {
            return;
        };
        let exercise = memory.run.exercise().clone();
        let summary: RunSummary = memory
            .run
            .finish(session.game(), self.now_ms())
            .in_session(self.session_id);
        if let Some(run_id) = memory.run_id {
            let stored = with_store(&mut self.local, handle, |store, _| {
                store.finish_run(run_id, &summary)
            });
            if let Some(Err(err)) = stored {
                warn!("Treino: não foi possível gravar a Resolução: {err}");
            }
        }
        let review = Review::new(&exercise, &summary);
        self.review = Some(ReviewState { review, exercise });
        let plies = session.snapshot().history.len();
        session.set_review_ply(Some(plies));
        self.view = View::Review;
        self.refresh_metrics(handle);
    }

    /// Salta o tempo (capturas e testes): termina a apresentação em curso.
    fn skip_presentation(&mut self) {
        self.clock_skew_ms += 600_000;
    }
}

fn ply_marks(ui: &mut egui::Ui, plies: &[PlyResult]) {
    ui.horizontal_wrapped(|ui| {
        for (i, ply) in plies.iter().enumerate() {
            let color = match ply {
                PlyResult::Hit => egui::Color32::LIGHT_GREEN,
                PlyResult::Miss => egui::Color32::LIGHT_RED,
                PlyResult::Pending => egui::Color32::GRAY,
            };
            ui.colored_label(color, egui::RichText::new((i + 1).to_string()).strong());
        }
    });
}

fn memory_view(
    ui: &mut egui::Ui,
    i18n: &I18n,
    memory: &MemoryActive,
    session: &Session,
    now_ms: u64,
) -> Option<Action> {
    let mut action = None;
    let run = &memory.run;
    let total = run.line().len();
    ui.strong(&run.exercise().title);
    ui.label(i18n.t_with("training-objective", &[("text", &run.exercise().objective)]));
    match run.phase() {
        MemoryPhase::Presentation => {
            let shown = run.presentation_ply(now_ms);
            if shown == 0 {
                ui.label(i18n.t("training-memory-present-start"));
            } else {
                ui.label(i18n.t_with(
                    "training-memory-present",
                    &[("n", &shown.to_string()), ("total", &total.to_string())],
                ));
            }
            ui.small(i18n.t("training-memory-present-hint"));
        }
        _ => {
            let n = (session.snapshot().history.len() + 1).min(total);
            ui.label(i18n.t_with(
                "training-memory-recall",
                &[("n", &n.to_string()), ("total", &total.to_string())],
            ));
            ui.label(i18n.t(match session.snapshot().side_to_move {
                Side::White => "training-memory-turn-white",
                Side::Black => "training-memory-turn-black",
            }));
            ply_marks(ui, run.plies());
            if !run.errors().is_empty() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    i18n.t_with(
                        "training-memory-errors",
                        &[("count", &run.errors().len().to_string())],
                    ),
                );
            }
            if ui.button(i18n.t("training-memory-show-again")).clicked() {
                action = Some(Action::ShowAgain);
            }
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

fn memory_review(ui: &mut egui::Ui, i18n: &I18n, memory: &chess_training::MemoryReview) {
    ui.separator();
    ui.strong(i18n.t("training-memory-review-heading"));
    let hits = memory
        .plies
        .iter()
        .filter(|p| p.result == PlyResult::Hit)
        .count();
    ui.label(i18n.t_with(
        "training-memory-review-hits",
        &[
            ("hits", &hits.to_string()),
            ("total", &memory.plies.len().to_string()),
        ],
    ));
    match memory.first_error {
        Some(index) => {
            ui.label(i18n.t_with(
                "training-memory-review-first-error",
                &[("n", &(index + 1).to_string())],
            ));
        }
        None => {
            ui.label(i18n.t("training-memory-review-no-error"));
        }
    }
    if memory.replays > 0 {
        ui.label(i18n.t_with(
            "training-memory-review-replays",
            &[("count", &memory.replays.to_string())],
        ));
    }
    for (i, ply) in memory.plies.iter().enumerate() {
        let (color, key) = match ply.result {
            PlyResult::Hit => (egui::Color32::LIGHT_GREEN, "training-memory-ply-hit"),
            PlyResult::Miss => (egui::Color32::LIGHT_RED, "training-memory-ply-miss"),
            PlyResult::Pending => (egui::Color32::GRAY, "training-memory-ply-pending"),
        };
        ui.colored_label(
            color,
            i18n.t_with(
                "training-memory-ply-line",
                &[
                    ("n", &(i + 1).to_string()),
                    ("san", &ply.san),
                    ("result", &i18n.t(key)),
                ],
            ),
        );
    }
}

/// Roteiro de memória (`--training memory` / `memory-review`): `memory` pára a apresentação
/// no 3º Lance; `memory-review` reproduz a abertura italiana com um erro e um "mostrar de
/// novo" e abre a revisão.
fn memory_demo(
    state: &mut TrainingUi,
    session: &mut Session,
    aids: &AidSettings,
    script: &str,
    frame: u32,
) {
    const PLAY: [&str; 7] = ["e2e4", "e7e5", "b1c3", "g1f3", "b8c6", "f1c4", "f8c5"];
    match frame {
        3 => {
            state.sync_catalog(None);
            state.view = View::List;
            state.start("memoria-italiana-001", session, aids, None);
            if script == "memory" {
                // 3 Lances e meio de 1400 ms: mostra o 3º Lance e segura.
                state.clock_skew_ms = 3 * 1400 + 200;
            }
        }
        6 if script == "memory-review" => state.skip_presentation(),
        8 if script == "memory-review" => state.show_again(session),
        10 if script == "memory-review" => state.skip_presentation(),
        _ => {}
    }
    if script == "memory-review"
        && frame >= 14
        && frame.is_multiple_of(2)
        && let Some(uci) = PLAY.get(((frame - 14) / 2) as usize)
    {
        demo_move(session, uci);
    }
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
        if t.memory_plies > 0 {
            ui.label(i18n.t_with(
                "training-metrics-memory",
                &[
                    ("hits", &t.memory_hits.to_string()),
                    ("plies", &t.memory_plies.to_string()),
                ],
            ));
        }
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

fn exercise_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &TrainingUi,
    exercise: &Exercise,
) -> Option<Action> {
    let mut action = None;
    let tally = state
        .metrics
        .by_exercise
        .get(&exercise.id)
        .copied()
        .unwrap_or_default();
    // Linha do botão com o título; os números e os temas vão numa linha pequena embaixo, para
    // o texto não quebrar ao lado do botão.
    ui.horizontal(|ui| {
        if cs_button(ui, &i18n.t("training-start"), ButtonVariant::Secondary).clicked() {
            action = Some(Action::Start(exercise.id.clone()));
        }
        ui.add(egui::Label::new(egui::RichText::new(&exercise.title).strong()).truncate());
    });
    let mut details = i18n.t_with(
        "training-exercise-count",
        &[
            ("correct", &tally.correct.to_string()),
            ("runs", &tally.runs.to_string()),
        ],
    );
    if let Some(rating) = exercise.rating {
        details += &format!(
            " — {}",
            i18n.t_with(
                "training-exercise-rating",
                &[("rating", &rating.to_string())]
            )
        );
    }
    if !exercise.themes.is_empty() {
        details += &format!(" — {}", themes::theme_names(i18n, &exercise.themes, 3));
    }
    ui.small(details);
    ui.add_space(4.0);
    action
}

/// Seção de um pacote instalado: nome, Origem visível e os Exercícios que passam no filtro.
fn pack_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &TrainingUi,
    id: &str,
    matching: &[&Exercise],
) -> Option<Action> {
    let mut action = None;
    ui.label(
        egui::RichText::new(i18n.t_with(
            "training-section-pack",
            &[("name", packs::pack_name(&state.packs, id))],
        ))
        .strong(),
    );
    if let Some(origin) = packs::pack_origin(i18n, &state.packs, id) {
        ui.small(origin);
    }
    for exercise in matching.iter().take(packs::ROWS_PER_SECTION) {
        if let Some(chosen) = exercise_row(ui, i18n, state, exercise) {
            action = Some(chosen);
        }
    }
    if matching.len() > packs::ROWS_PER_SECTION {
        ui.small(i18n.t_with(
            "training-list-shown",
            &[
                ("shown", &packs::ROWS_PER_SECTION.to_string()),
                ("total", &matching.len().to_string()),
            ],
        ));
    }
    action
}

fn list_view(ui: &mut egui::Ui, i18n: &I18n, state: &mut TrainingUi) -> Option<Action> {
    let mut action = None;
    ui.strong(i18n.t("training-list-heading"));
    packs::filter_bar(ui, i18n, &mut state.packs);
    let filter = state.packs.filter();
    let state = &*state;
    let matching: Vec<&Exercise> = state.catalog.iter().filter(|e| filter.matches(e)).collect();
    if matching.is_empty() {
        ui.label(i18n.t("training-list-none"));
    }
    // A lista ocupa a altura disponível do Painel (antes: 260 px fixos).
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            // De fábrica, por tipo.
            let factory: Vec<&Exercise> = matching
                .iter()
                .copied()
                .filter(|e| e.pack.is_none())
                .collect();
            if !factory.is_empty() && !state.packs.installed.is_empty() {
                ui.label(egui::RichText::new(i18n.t("training-section-factory")).strong());
            }
            for kind in ExerciseKind::ALL {
                let exercises: Vec<&Exercise> =
                    factory.iter().copied().filter(|e| e.kind == kind).collect();
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
                    if let Some(chosen) = exercise_row(ui, i18n, state, exercise) {
                        action = Some(chosen);
                    }
                }
            }
            // Pacotes instalados, separados dos de fábrica.
            for info in &state.packs.installed {
                let of_pack: Vec<&Exercise> = matching
                    .iter()
                    .copied()
                    .filter(|e| e.pack.as_deref() == Some(info.id.as_str()))
                    .collect();
                if of_pack.is_empty() {
                    continue;
                }
                ui.separator();
                if let Some(chosen) = pack_section(ui, i18n, state, &info.id, &of_pack) {
                    action = Some(chosen);
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
    if let Some(memory) = &r.memory {
        memory_review(ui, i18n, memory);
    }
    ui.separator();
    if r.memory.is_some() {
        // Os erros de memória aparecem Lance a Lance acima.
    } else if r.errors.is_empty() {
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

/// Painel Treino do dock (C2): o que era a janela flutuante. Estado em [`TrainingUi`]; a
/// `Session` só recebe as intenções do juiz (ADR-0001).
pub fn training_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    world.resource_scope(|world, mut state: Mut<TrainingUi>| {
        world.resource_scope(|world, mut session: Mut<Session>| {
            let aids = world
                .get_resource::<AidSettings>()
                .copied()
                .unwrap_or_default();
            let handle = world.get_resource::<PersistenceHandle>();
            let mut action = None;
            match state.view {
                View::Closed => {
                    if ui.button(i18n.t("training-open")).clicked() {
                        action = Some(Action::Open);
                    }
                }
                view => {
                    let inner = match view {
                        View::List => {
                            let go = ui
                                .horizontal(|ui| {
                                    let mut go = None;
                                    if ui.button(i18n.t("profiles-open")).clicked() {
                                        go = Some(Action::Show(View::Profiles));
                                    }
                                    if ui.button(i18n.t("evolution-open")).clicked() {
                                        go = Some(Action::Show(View::Evolution));
                                    }
                                    if cs_button(
                                        ui,
                                        &i18n.t("training-packs-open"),
                                        ButtonVariant::Secondary,
                                    )
                                    .clicked()
                                    {
                                        go = Some(Action::Show(View::Packs));
                                    }
                                    go
                                })
                                .inner;
                            let resume = profiles::continue_bar(ui, i18n, &state);
                            let chosen = list_view(ui, i18n, &mut state);
                            go.or(resume).or(chosen)
                        }
                        View::Exercise => {
                            if let Some(memory) = state.memory.as_ref() {
                                memory_view(ui, i18n, memory, &session, state.now_ms())
                            } else {
                                state
                                    .active
                                    .as_ref()
                                    .and_then(|a| exercise_view(ui, i18n, a, &session, &aids))
                            }
                        }
                        View::Position => {
                            let now = state.now_ms();
                            state
                                .position
                                .as_ref()
                                .and_then(|p| position::panel(ui, i18n, p, now))
                        }
                        View::Profiles => profiles::view(ui, i18n, &state),
                        View::Packs => {
                            state.packs.download.provider_on = world
                                .get_resource::<NetService>()
                                .map(|net| net.is_enabled(chess_net::ProviderId::Lichess));
                            packs::view(ui, i18n, &mut state.packs)
                        }
                        View::Evolution => evolution::view(ui, i18n, &mut state)
                            .then_some(Action::Show(View::List)),
                        _ => state.review.as_ref().and_then(|r| review_view(ui, i18n, r)),
                    };
                    if inner.is_some() {
                        action = inner;
                    }
                    ui.separator();
                    metrics_section(ui, i18n, &state.metrics, state.session_id);
                    ui.small(i18n.t("training-note-no-effect"));
                    if ui.button(i18n.t("training-close")).clicked() {
                        action = Some(Action::Close);
                    }
                }
            }
            match action {
                None => {}
                Some(Action::Open) => state.open(handle),
                Some(Action::Close) => state.leave(&mut session, View::Closed),
                Some(Action::Start(id)) => state.start(&id, &mut session, &aids, handle),
                Some(Action::GiveUp) => {
                    if state.memory.is_some() {
                        state.finish_memory(&mut session, handle);
                    } else {
                        state.finish(&mut session, handle);
                    }
                }
                Some(Action::ShowAgain) => state.show_again(&mut session),
                Some(Action::Position(cmd)) => state.apply_position(cmd, handle),
                Some(Action::Show(view)) => state.show(view, &mut session, handle),
                Some(Action::FollowAndStart(profile, exercise)) => {
                    state.follow_profile(Some(profile), handle);
                    state.start(&exercise, &mut session, &aids, handle);
                }
                Some(Action::Unfollow) => state.follow_profile(None, handle),
                Some(Action::Packs(packs::Cmd::StartDownload)) => {
                    download::begin(&mut state, world, i18n);
                }
                Some(Action::Packs(cmd)) => state.apply_packs(cmd, handle),
                // Sair sem resolver nem desistir descarta a Resolução (não vira erro).
                Some(Action::BackToList) => state.leave(&mut session, View::List),
                Some(Action::Reveal) => {
                    if let Some(active) = state.active.as_mut() {
                        active.revealed = true;
                    }
                }
            }
        });
    });
}

// --- Evidências (`--screenshot <dir> --training list|exercise|review`) ----------------

/// Roteiro de captura de Treino ativo (`--training`): o dock aparece mesmo sem `--panel`.
pub(crate) fn demo_active() -> bool {
    demo_script().is_some()
}

pub(super) fn demo_script() -> Option<String> {
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
    if script.starts_with("download") {
        // Cenas do download de problemas: ver `download::demo_system`.
        return;
    }
    if script.starts_with("memory") {
        memory_demo(&mut state, &mut session, &aids, &script, *frame);
        return;
    }
    if matches!(
        script.as_str(),
        "position" | "position-review" | "evolution" | "profiles"
    ) {
        position_demo(&mut state, &mut session, &aids, &script, *frame);
        return;
    }
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

/// Roteiros do C11 (`--training position|position-review|evolution|profiles`).
fn position_demo(
    state: &mut TrainingUi,
    session: &mut Session,
    aids: &AidSettings,
    script: &str,
    frame: u32,
) {
    match frame {
        3 => {
            state.sync_catalog(None);
            match script {
                "evolution" => {
                    // Resoluções sintéticas (só na captura): 24 dias de treino.
                    state.evolution_data =
                        evolution::EvolutionData::compute(&evolution::demo_runs(), &state.profiles);
                    state.view = View::Evolution;
                }
                "profiles" => {
                    state.refresh_profiles(None);
                    for id in ["xeque-001", "xeque-002", "mate-em-1-001", "roque-001"] {
                        let _ = with_store(&mut state.local, None, |store, player| {
                            store.record_profile_step(player, "regras-basicas", id)
                        });
                    }
                    state.follow_profile(Some("regras-basicas".into()), None);
                    state.refresh_profiles(None);
                    state.view = View::Profiles;
                }
                _ => state.start("memoria-posicao-003", session, aids, None),
            }
        }
        6 if script.starts_with("position") => {
            state.apply_position(position::Cmd::BeginRecall, None)
        }
        8 if script.starts_with("position") => position::demo_place(state, true),
        10 if script == "position-review" => state.apply_position(position::Cmd::Check, None),
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

    /// Memória de sequência no app: apresentação só leitura, volta ao início, erro por
    /// Lance, "mostrar de novo" e revisão com as métricas por Lance (TRN-04, DEC-45).
    #[test]
    fn memory_flow_presents_then_recalls_from_the_start_and_reviews_each_ply() {
        let mut state = TrainingUi::default();
        let aids = AidSettings::physical();
        let mut session = Session::new();
        play(&mut session, "e2e4");
        let before = session.snapshot().clone();

        state.open(None);
        state.start("memoria-italiana-001", &mut session, &aids, None);
        assert!(state.memory.is_some() && state.active.is_none());
        // Apresentação: posição inicial em revisão (sem gestos).
        state.memory_step(&mut session, &aids, None);
        assert!(session.review().is_some(), "apresentação é só leitura");
        assert_eq!(
            session.game().snapshot().history.len(),
            6,
            "sequência pronta"
        );

        // Termina a apresentação: volta ao início.
        state.skip_presentation();
        state.memory_step(&mut session, &aids, None);
        let fresh = state
            .catalog
            .get("memoria-italiana-001")
            .unwrap()
            .fen
            .clone();
        assert_eq!(
            session.snapshot().position.to_fen(),
            fresh,
            "volta ao início"
        );
        assert!(session.review().is_none(), "agora os gestos valem");
        assert_eq!(
            state.memory.as_ref().unwrap().run.phase(),
            MemoryPhase::Recall
        );

        play(&mut session, "e2e4");
        play(&mut session, "e7e6"); // erro no Lance 2
        state.memory_step(&mut session, &aids, None);
        assert_eq!(state.memory.as_ref().unwrap().run.errors().len(), 1);
        assert_eq!(session.snapshot().history.len(), 1, "Lance errado desfeito");

        state.show_again(&mut session);
        assert_eq!(session.game().snapshot().history.len(), 6);
        state.skip_presentation();
        state.memory_step(&mut session, &aids, None);
        for uci in ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5"] {
            play(&mut session, uci);
            state.memory_step(&mut session, &aids, None);
        }
        assert_eq!(state.view, View::Review);
        let memory = state
            .review
            .as_ref()
            .unwrap()
            .review
            .memory
            .clone()
            .unwrap();
        assert_eq!(memory.first_error, Some(1));
        assert_eq!(memory.replays, 1);
        assert_eq!(state.metrics.total.correct, 1, "Exercício completo");
        assert_eq!(state.metrics.total.memory_plies, 6);
        assert_eq!(state.metrics.total.memory_hits, 5);

        state.leave(&mut session, View::Closed);
        assert_eq!(session.snapshot(), &before);
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
