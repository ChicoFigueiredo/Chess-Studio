//! Memória de posição na tela de Treino (C11, US-25): estudo cronometrado, bandeja de
//! peças e conferência, nas três Apresentações.
//!
//! O tabuleiro mostra a Posição a memorizar e depois o que a pessoa montou por meio de
//! [`BoardOverride`]: a `Session` guarda só uma Partida de enfeite (dois reis) e os gestos
//! do tabuleiro não chegam a ela enquanto o editor está aberto (ADR-0001). Arrastar da
//! bandeja usa a geometria de picking existente; clicar numa peça da bandeja a "segura" e
//! o próximo clique numa casa a coloca. O julgamento é o de `chess_training::judge`.

use super::{Action, TrainingUi, View, with_store};
use crate::aids::AidSettings;
use crate::camera::MainCamera;
use crate::capture::CaptureMode;
use crate::dock::Dock;
use crate::i18n::I18n;
use crate::persistence::PersistenceHandle;
use crate::picking::{HoveredSquare, board_point_under_cursor};
use crate::pieces::BoardOverride;
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, egui};
use chess_core::{Color as Side, Piece, Square};
use chess_game::{Game, GameSetup, PolicyKind};
use chess_input::point_to_square;
use chess_store::RunId;
use chess_training::{
    Exercise, PiecePlacement, PositionJudgment, PositionMemoryRun, PositionPhase, RunResult,
    RunSummary, SquareVerdict,
};
use std::collections::HashMap;

/// Partida de enfeite que ocupa a `Session` enquanto o editor está aberto: só os dois reis,
/// para que nada da Posição a memorizar vaze pelos realces do tabuleiro.
const PLACEHOLDER_FEN: &str = "4k3/8/8/8/8/8/8/4K3 w - - 0 1";

const PIECES: [Piece; 6] = [
    Piece::King,
    Piece::Queen,
    Piece::Rook,
    Piece::Bishop,
    Piece::Knight,
    Piece::Pawn,
];

/// A peça que a pessoa está segurando (vinda da bandeja ou levantada do tabuleiro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hand {
    pub side: Side,
    pub piece: Piece,
    /// Casa de onde foi levantada; `None` se veio da bandeja.
    pub origin: Option<Square>,
}

/// Resultado de "Conferir" (ou de desistir): o que a revisão mostra.
#[derive(Debug)]
pub(super) struct PositionDone {
    pub judgment: PositionJudgment,
    pub target: PiecePlacement,
    pub placed: PiecePlacement,
    pub summary: RunSummary,
}

/// Estado do Exercício de memória de posição em andamento (ou já conferido).
#[derive(Debug)]
pub(super) struct PositionState {
    pub exercise: Exercise,
    run: Option<PositionMemoryRun>,
    run_id: Option<RunId>,
    hand: Option<Hand>,
    /// A mão veio de um "pressionar" no tabuleiro (arrasto): soltar decide o destino.
    lifted_by_press: bool,
    pub done: Option<PositionDone>,
    /// Na revisão: mostrar a Posição certa em vez da montada.
    pub show_answer: bool,
}

/// Comandos da interface do editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Cmd {
    BeginRecall,
    Clear,
    Check,
    GiveUp,
    ToggleAnswer,
}

impl PositionState {
    pub(super) fn new(exercise: Exercise, run: PositionMemoryRun, run_id: Option<RunId>) -> Self {
        Self {
            exercise,
            run: Some(run),
            run_id,
            hand: None,
            lifted_by_press: false,
            done: None,
            show_answer: false,
        }
    }

    pub(super) fn phase(&self) -> PositionPhase {
        self.run.as_ref().map_or(PositionPhase::Done, |r| r.phase())
    }

    pub(super) fn hand(&self) -> Option<Hand> {
        self.hand
    }

    /// O que o tabuleiro deve mostrar agora.
    pub(super) fn board(&self) -> HashMap<Square, (Side, Piece)> {
        if let Some(done) = &self.done {
            return if self.show_answer {
                done.target.to_map()
            } else {
                done.placed.to_map()
            };
        }
        match &self.run {
            Some(run) if run.phase() == PositionPhase::Study => run.target().to_map(),
            Some(run) => run.placed().to_map(),
            None => HashMap::new(),
        }
    }

    // --- Editor (sem Bevy: testável) --------------------------------------------------

    /// Pegar uma peça da bandeja (clique ou início de arrasto).
    pub(super) fn pick_from_tray(&mut self, side: Side, piece: Piece) {
        if self.phase() == PositionPhase::Recall {
            self.hand = Some(Hand {
                side,
                piece,
                origin: None,
            });
            self.lifted_by_press = false;
        }
    }

    /// Soltar a peça da bandeja numa casa (ou fora do tabuleiro: descarta).
    pub(super) fn drop_from_tray(&mut self, square: Option<Square>) {
        if let (Some(hand), Some(square), Some(run)) = (self.hand, square, self.run.as_mut()) {
            run.place(square, hand.side, hand.piece);
        }
        self.hand = None;
        self.lifted_by_press = false;
    }

    /// Botão esquerdo pressionado sobre o tabuleiro (`square` = casa sob o cursor).
    pub(super) fn press(&mut self, square: Option<Square>) {
        let (Some(square), Some(run)) = (square, self.run.as_mut()) else {
            return;
        };
        if let Some(hand) = self.hand {
            run.place(square, hand.side, hand.piece);
            self.hand = None;
            self.lifted_by_press = false;
        } else if let Some((side, piece)) = run.placed().get(square) {
            run.remove(square);
            self.hand = Some(Hand {
                side,
                piece,
                origin: Some(square),
            });
            self.lifted_by_press = true;
        }
    }

    /// Botão esquerdo solto. Depois de levantar uma peça: soltar noutra casa a move; soltar
    /// na mesma casa a mantém na mão (clique-clique); soltar fora do tabuleiro ou sobre um
    /// painel a remove.
    pub(super) fn release(&mut self, square: Option<Square>) {
        if !self.lifted_by_press {
            return;
        }
        self.lifted_by_press = false;
        let Some(hand) = self.hand else { return };
        match square {
            Some(target) if Some(target) == hand.origin => {}
            Some(target) => {
                if let Some(run) = self.run.as_mut() {
                    run.place(target, hand.side, hand.piece);
                }
                self.hand = None;
            }
            None => self.hand = None,
        }
    }

    /// Botão direito: larga o que está na mão (volta à origem) ou remove a peça da casa.
    pub(super) fn secondary(&mut self, square: Option<Square>) {
        let Some(run) = self.run.as_mut() else { return };
        if let Some(hand) = self.hand.take() {
            if let Some(origin) = hand.origin {
                run.place(origin, hand.side, hand.piece);
            }
            self.lifted_by_press = false;
        } else if let Some(square) = square {
            run.remove(square);
        }
    }

    pub(super) fn tick(&mut self, now_ms: u64) {
        if let Some(run) = self.run.as_mut()
            && run.study_finished(now_ms)
        {
            run.begin_recall(now_ms);
        }
    }

    pub(super) fn begin_recall(&mut self, now_ms: u64) {
        if let Some(run) = self.run.as_mut() {
            run.begin_recall(now_ms);
        }
    }

    pub(super) fn clear(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.clear();
        }
        self.hand = None;
    }

    /// Encerra a Resolução: confere (`check`) ou desiste, e guarda o resultado.
    fn conclude(&mut self, now_ms: u64, check: bool) -> Option<&PositionDone> {
        let mut run = self.run.take()?;
        let target = run.target().clone();
        let placed = run.placed().clone();
        let judgment = if check {
            run.check(now_ms).clone()
        } else {
            chess_training::judge(&target, &placed)
        };
        let summary = run.finish(now_ms);
        self.hand = None;
        self.lifted_by_press = false;
        self.done = Some(PositionDone {
            judgment,
            target,
            placed,
            summary,
        });
        self.done.as_ref()
    }
}

// --- Integração com a tela de Treino -----------------------------------------------

impl TrainingUi {
    /// Começa a memória de posição: a `Session` passa a ser a Partida de enfeite (a
    /// persistência não a grava) e o tabuleiro mostra a Posição por alguns segundos.
    pub(super) fn start_position(
        &mut self,
        id: &str,
        session: &mut Session,
        aids: &AidSettings,
        handle: Option<&PersistenceHandle>,
    ) {
        let Some(exercise) = self.catalog.get(id).cloned() else {
            return;
        };
        let Ok(setup) = GameSetup::from_fen(PolicyKind::Assisted, PLACEHOLDER_FEN) else {
            return;
        };
        let mut fresh = Session::from_game(Game::new(setup));
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
        let run = PositionMemoryRun::start(&exercise, super::aids_record(aids), self.now_ms());
        self.active = None;
        self.review = None;
        self.position = Some(PositionState::new(exercise, run, run_id));
        self.view = View::Position;
    }

    /// Conferir ou desistir: grava o registro imutável e abre a revisão.
    pub(super) fn conclude_position(&mut self, check: bool, handle: Option<&PersistenceHandle>) {
        let now = self.now_ms();
        let session_id = self.session_id;
        let Some(state) = self.position.as_mut() else {
            return;
        };
        let run_id = state.run_id;
        if state.conclude(now, check).is_none() {
            return;
        }
        let Some(done) = state.done.as_mut() else {
            return;
        };
        done.summary = done.summary.clone().in_session(session_id);
        let summary = done.summary.clone();
        self.after_run(run_id, &summary, handle);
    }

    fn after_run(
        &mut self,
        run_id: Option<RunId>,
        summary: &RunSummary,
        handle: Option<&PersistenceHandle>,
    ) {
        if let Some(run_id) = run_id {
            let stored = with_store(&mut self.local, handle, |store, _| {
                store.finish_run(run_id, summary)
            });
            if let Some(Err(err)) = stored {
                warn!("Treino: não foi possível gravar a Resolução: {err}");
            }
        }
        self.advance_profiles(summary, handle);
        self.refresh_metrics(handle);
    }
}

// --- Sistemas ------------------------------------------------------------------------

/// Espelha o estado do editor no desenho do tabuleiro (as três Apresentações).
pub(super) fn sync_board(state: Res<TrainingUi>, mut over: ResMut<BoardOverride>) {
    let desired = state.position.as_ref().map(PositionState::board);
    over.set(desired);
}

/// Faz o cronômetro de estudo andar e passa à reconstrução quando o tempo acaba.
pub(super) fn tick(mut state: ResMut<TrainingUi>) {
    let now = state.now_ms();
    if let Some(position) = state.position.as_mut() {
        position.tick(now);
    }
}

fn paint_piece(painter: &egui::Painter, center: egui::Pos2, size: f32, side: Side, piece: Piece) {
    let glyph = crate::layout::glyph(piece).to_string();
    let (fill, outline) = if side == Side::White {
        (
            egui::Color32::from_rgb(250, 248, 235),
            egui::Color32::from_rgb(20, 20, 24),
        )
    } else {
        (
            egui::Color32::from_rgb(24, 24, 28),
            egui::Color32::from_rgb(240, 240, 230),
        )
    };
    let font = egui::FontId::proportional(size);
    for (dx, dy) in [(-1.2, 0.0), (1.2, 0.0), (0.0, -1.2), (0.0, 1.2)] {
        painter.text(
            center + egui::vec2(dx, dy),
            egui::Align2::CENTER_CENTER,
            &glyph,
            font.clone(),
            outline,
        );
    }
    painter.text(center, egui::Align2::CENTER_CENTER, glyph, font, fill);
}

/// Bandeja de peças + entrada do tabuleiro (clique/arrasto) durante a reconstrução.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn editor(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<TrainingUi>,
    hovered: Res<HoveredSquare>,
    buttons: Res<ButtonInput<MouseButton>>,
    egui_input: Res<EguiWantsInput>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &Transform), With<MainCamera>>,
    presentation: Res<Presentation>,
    dock: Res<Dock>,
    capture: Option<Res<CaptureMode>>,
) -> Result {
    let Some(position) = state.position.as_mut() else {
        return Ok(());
    };
    if position.phase() != PositionPhase::Recall {
        return Ok(());
    }
    let capturing = capture.is_some();
    let ctx = contexts.ctx_mut()?.clone();
    let over_ui = egui_input.wants_pointer_input() || egui_input.is_pointer_over_area();

    // Entrada no tabuleiro (fora de qualquer painel).
    if !capturing {
        if buttons.just_pressed(MouseButton::Left) && !over_ui {
            position.press(hovered.0);
        }
        if buttons.just_released(MouseButton::Left) {
            position.release(hovered.0);
        }
        if buttons.just_pressed(MouseButton::Right) && !over_ui {
            position.secondary(hovered.0);
        }
    }

    // Bandeja.
    let mut dropped = None;
    egui::Window::new(i18n.t("position-tray-title"))
        .anchor(egui::Align2::LEFT_CENTER, [8.0, 0.0])
        .resizable(false)
        .show(&ctx, |ui| {
            ui.small(i18n.t("position-tray-hint"));
            for side in [Side::White, Side::Black] {
                ui.horizontal(|ui| {
                    for piece in PIECES {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(34.0, 38.0),
                            egui::Sense::click_and_drag(),
                        );
                        let held = position.hand().is_some_and(|h| {
                            h.origin.is_none() && h.side == side && h.piece == piece
                        });
                        let bg = if held {
                            egui::Color32::from_rgb(220, 170, 40)
                        } else if side == Side::White {
                            egui::Color32::from_rgb(120, 122, 130)
                        } else {
                            egui::Color32::from_rgb(190, 192, 200)
                        };
                        ui.painter().rect_filled(rect, 4.0, bg);
                        paint_piece(ui.painter(), rect.center(), 30.0, side, piece);
                        if response.drag_started() || response.clicked() {
                            position.pick_from_tray(side, piece);
                        }
                        if response.drag_stopped() {
                            dropped = Some(());
                        }
                    }
                });
            }
        });

    // Soltar da bandeja: a casa vem da geometria de picking, mesmo com o egui no ponteiro.
    if dropped.is_some() {
        let (camera, transform) = *camera;
        let square = board_point_under_cursor(&window, camera, transform, *presentation, &dock)
            .and_then(point_to_square);
        position.drop_from_tray(square);
    }

    // A peça na mão acompanha o cursor.
    if let (Some(hand), Some(pos)) = (position.hand(), ctx.pointer_latest_pos()) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("position-hand"),
        ));
        paint_piece(&painter, pos, 46.0, hand.side, hand.piece);
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    Ok(())
}

// --- Painel de Treino ------------------------------------------------------------------

fn side_key(side: Side) -> &'static str {
    match side {
        Side::White => "position-side-white",
        Side::Black => "position-side-black",
    }
}

fn piece_key(piece: Piece) -> &'static str {
    match piece {
        Piece::King => "position-piece-king",
        Piece::Queen => "position-piece-queen",
        Piece::Rook => "position-piece-rook",
        Piece::Bishop => "position-piece-bishop",
        Piece::Knight => "position-piece-knight",
        Piece::Pawn => "position-piece-pawn",
    }
}

fn piece_name(i18n: &I18n, (side, piece): (Side, Piece)) -> String {
    i18n.t_with(
        "position-piece-name",
        &[
            ("piece", &i18n.t(piece_key(piece))),
            ("side", &i18n.t(side_key(side))),
        ],
    )
}

/// Texto de uma casa julgada, para a lista da revisão.
pub(super) fn verdict_text(i18n: &I18n, square: Square, verdict: SquareVerdict) -> Option<String> {
    let square = square.to_string();
    Some(match verdict {
        SquareVerdict::Correct(_) => return None,
        SquareVerdict::WrongPiece { expected, placed } => i18n.t_with(
            "position-verdict-wrong",
            &[
                ("square", &square),
                ("placed", &piece_name(i18n, placed)),
                ("expected", &piece_name(i18n, expected)),
            ],
        ),
        SquareVerdict::Missing(expected) => i18n.t_with(
            "position-verdict-missing",
            &[
                ("square", &square),
                ("expected", &piece_name(i18n, expected)),
            ],
        ),
        SquareVerdict::Extra(placed) => i18n.t_with(
            "position-verdict-extra",
            &[("square", &square), ("placed", &piece_name(i18n, placed))],
        ),
    })
}

/// Painel do Exercício: estudo (contagem regressiva), reconstrução e revisão.
pub(super) fn panel(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &PositionState,
    now_ms: u64,
) -> Option<Action> {
    let mut action = None;
    ui.strong(&state.exercise.title);
    ui.label(i18n.t_with("training-objective", &[("text", &state.exercise.objective)]));
    if let Some(done) = &state.done {
        done_view(ui, i18n, state, done, &mut action);
    } else if let Some(run) = &state.run {
        match run.phase() {
            PositionPhase::Study => {
                let seconds = run.study_remaining_ms(now_ms).div_ceil(1000);
                ui.heading(i18n.t_with("position-study-countdown", &[("s", &seconds.to_string())]));
                ui.small(i18n.t("position-study-hint"));
                if ui.button(i18n.t("position-memorized")).clicked() {
                    action = Some(Action::Position(Cmd::BeginRecall));
                }
            }
            _ => {
                ui.label(i18n.t_with(
                    "position-recall-count",
                    &[("n", &run.placed().len().to_string())],
                ));
                ui.small(i18n.t("position-recall-hint"));
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("position-check")).clicked() {
                        action = Some(Action::Position(Cmd::Check));
                    }
                    if ui.button(i18n.t("position-clear")).clicked() {
                        action = Some(Action::Position(Cmd::Clear));
                    }
                });
            }
        }
    }
    ui.horizontal(|ui| {
        if state.done.is_none() && ui.button(i18n.t("training-give-up")).clicked() {
            action = Some(Action::Position(Cmd::GiveUp));
        }
        if ui.button(i18n.t("training-back")).clicked() {
            action = Some(Action::BackToList);
        }
    });
    action
}

fn done_view(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &PositionState,
    done: &PositionDone,
    action: &mut Option<Action>,
) {
    ui.separator();
    ui.strong(i18n.t("training-review-heading"));
    let (color, key) = match done.summary.result {
        RunResult::Correct => (egui::Color32::LIGHT_GREEN, "position-result-perfect"),
        RunResult::Incorrect => (egui::Color32::LIGHT_RED, "position-result-imperfect"),
    };
    ui.colored_label(color, i18n.t(key));
    let j = &done.judgment;
    ui.label(i18n.t_with(
        "position-score",
        &[
            ("percent", &j.percent().to_string()),
            ("correct", &j.correct.to_string()),
            ("total", &j.target_total.to_string()),
        ],
    ));
    ui.label(i18n.t_with(
        "position-score-detail",
        &[
            ("wrong", &j.wrong_piece.to_string()),
            ("missing", &j.missing.to_string()),
            ("extra", &j.extra.to_string()),
        ],
    ));
    let mistakes: Vec<String> = j
        .squares
        .iter()
        .filter_map(|(s, v)| verdict_text(i18n, *s, *v))
        .collect();
    if !mistakes.is_empty() {
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for line in &mistakes {
                    ui.label(line);
                }
            });
    }
    if let Some(memory) = &done.summary.position_memory {
        ui.small(i18n.t_with(
            "position-times",
            &[
                ("study", &(memory.study_ms / 1000).to_string()),
                ("recall", &(memory.recall_ms / 1000).to_string()),
            ],
        ));
    }
    ui.small(i18n.t_with(
        "training-review-explanation",
        &[("text", &state.exercise.explanation)],
    ));
    ui.horizontal(|ui| {
        let label = if state.show_answer {
            "position-show-mine"
        } else {
            "position-show-answer"
        };
        if ui.button(i18n.t(label)).clicked() {
            *action = Some(Action::Position(Cmd::ToggleAnswer));
        }
        if ui.button(i18n.t("training-repeat")).clicked() {
            *action = Some(Action::Start(state.exercise.id.clone()));
        }
    });
}

impl TrainingUi {
    pub(super) fn apply_position(&mut self, cmd: Cmd, handle: Option<&PersistenceHandle>) {
        let now = self.now_ms();
        match cmd {
            Cmd::BeginRecall => {
                if let Some(p) = self.position.as_mut() {
                    p.begin_recall(now);
                }
            }
            Cmd::Clear => {
                if let Some(p) = self.position.as_mut() {
                    p.clear();
                }
            }
            Cmd::Check => self.conclude_position(true, handle),
            Cmd::GiveUp => self.conclude_position(false, handle),
            Cmd::ToggleAnswer => {
                if let Some(p) = self.position.as_mut() {
                    p.show_answer = !p.show_answer;
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_training::{AidsRecord, Catalog, Feedback};

    fn sq(s: &str) -> Square {
        s.parse().unwrap()
    }

    fn state() -> PositionState {
        let exercise = Catalog::bundled()
            .unwrap()
            .get("memoria-posicao-001")
            .unwrap()
            .clone();
        let run = PositionMemoryRun::start(&exercise, AidsRecord::new(&[], Feedback::Immediate), 0);
        PositionState::new(exercise, run, None)
    }

    fn recalling() -> PositionState {
        let mut s = state();
        s.begin_recall(10_000);
        s
    }

    #[test]
    fn the_board_shows_the_position_while_studying_then_what_was_placed() {
        let mut s = state();
        assert_eq!(s.board().len(), 9, "estudo: a Posição inteira");
        s.begin_recall(10_000);
        assert!(s.board().is_empty(), "reconstrução: tabuleiro vazio");
        s.pick_from_tray(Side::White, Piece::Rook);
        s.press(Some(sq("a1")));
        assert_eq!(s.board().get(&sq("a1")), Some(&(Side::White, Piece::Rook)));
    }

    #[test]
    fn the_study_timer_hands_over_to_the_recall_phase() {
        let mut s = state();
        s.tick(9_999);
        assert_eq!(s.phase(), PositionPhase::Study);
        s.tick(10_000);
        assert_eq!(s.phase(), PositionPhase::Recall);
    }

    #[test]
    fn click_a_tray_piece_then_click_a_square_places_it() {
        let mut s = recalling();
        s.pick_from_tray(Side::Black, Piece::King);
        assert!(s.hand().is_some());
        s.press(Some(sq("g8")));
        assert!(s.hand().is_none());
        assert_eq!(s.board().get(&sq("g8")), Some(&(Side::Black, Piece::King)));
    }

    #[test]
    fn dragging_from_the_tray_onto_a_square_places_it_and_off_board_discards() {
        let mut s = recalling();
        s.pick_from_tray(Side::White, Piece::Pawn);
        s.drop_from_tray(Some(sq("f2")));
        assert_eq!(s.board().len(), 1);
        s.pick_from_tray(Side::White, Piece::Pawn);
        s.drop_from_tray(None);
        assert_eq!(s.board().len(), 1, "soltar fora não coloca");
        assert!(s.hand().is_none());
    }

    #[test]
    fn dragging_a_placed_piece_moves_it() {
        let mut s = recalling();
        s.pick_from_tray(Side::White, Piece::Rook);
        s.press(Some(sq("a1")));
        s.press(Some(sq("a1"))); // levanta
        assert!(s.board().is_empty(), "levantada: sai da casa");
        s.release(Some(sq("a4")));
        assert_eq!(s.board().get(&sq("a4")), Some(&(Side::White, Piece::Rook)));
        assert!(s.hand().is_none());
    }

    #[test]
    fn releasing_on_the_origin_keeps_the_piece_in_hand_for_a_second_click() {
        let mut s = recalling();
        s.drop_for_test(Side::White, Piece::Rook, "a1");
        s.press(Some(sq("a1")));
        s.release(Some(sq("a1")));
        assert!(s.hand().is_some(), "clique-clique");
        s.press(Some(sq("h1")));
        assert_eq!(s.board().get(&sq("h1")), Some(&(Side::White, Piece::Rook)));
    }

    #[test]
    fn releasing_off_the_board_removes_the_piece_and_right_click_removes_or_restores() {
        let mut s = recalling();
        s.drop_for_test(Side::White, Piece::Rook, "a1");
        s.press(Some(sq("a1")));
        s.release(None);
        assert!(s.board().is_empty(), "arrastar para fora remove");

        s.drop_for_test(Side::Black, Piece::Queen, "d8");
        s.secondary(Some(sq("d8")));
        assert!(s.board().is_empty(), "botão direito remove");

        s.drop_for_test(Side::Black, Piece::Queen, "d8");
        s.press(Some(sq("d8")));
        s.release(Some(sq("d8"))); // fica na mão
        s.secondary(None);
        assert_eq!(
            s.board().get(&sq("d8")),
            Some(&(Side::Black, Piece::Queen)),
            "botão direito com a peça na mão devolve à origem"
        );
    }

    #[test]
    fn nothing_is_editable_outside_the_recall_phase() {
        let mut s = state(); // estudando
        s.pick_from_tray(Side::White, Piece::Rook);
        assert!(s.hand().is_none());
        s.press(Some(sq("a1")));
        assert_eq!(s.board().len(), 9, "a Posição segue intacta");
    }

    #[test]
    fn checking_a_perfect_reconstruction_records_a_correct_run() {
        let mut s = recalling();
        let target = s.run.as_ref().unwrap().target().clone();
        for (square, side, piece) in target.iter() {
            s.drop_for_test_sq(side, piece, square);
        }
        s.conclude(20_000, true);
        let done = s.done.as_ref().unwrap();
        assert!(done.judgment.is_perfect());
        assert_eq!(done.summary.result, RunResult::Correct);
        assert_eq!(s.board().len(), 9, "revisão mostra o que foi montado");
        s.show_answer = true;
        assert_eq!(s.board(), done.target.to_map());
    }

    #[test]
    fn giving_up_judges_the_partial_board() {
        let mut s = recalling();
        s.drop_for_test(Side::White, Piece::Rook, "a1");
        s.conclude(12_000, false);
        let done = s.done.as_ref().unwrap();
        assert_eq!(done.judgment.correct, 1);
        assert_eq!(done.summary.result, RunResult::Incorrect);
    }

    impl PositionState {
        fn drop_for_test(&mut self, side: Side, piece: Piece, square: &str) {
            self.drop_for_test_sq(side, piece, square.parse().unwrap());
        }

        fn drop_for_test_sq(&mut self, side: Side, piece: Piece, square: Square) {
            self.pick_from_tray(side, piece);
            self.drop_from_tray(Some(square));
        }
    }
}
