//! Aba "Evolução" (C11, US-26): indicadores pedagógicos, evolução por dia ou semana,
//! por Perfil de treino, e um gráfico de barras simples. Só lê Resoluções já gravadas
//! (`chess_training::evolution`, funções puras): nunca toca em Partidas competitivas
//! (TRN-05). Datas em UTC.

use super::TrainingUi;
use crate::i18n::I18n;
use bevy_egui::egui;
use chess_training::{
    DatedRun, Evolution, Period, PeriodStats, ProfileSet, by_period, by_training_profile,
    civil_date,
};

/// Quantos períodos o gráfico mostra (os mais recentes).
const CHART_PERIODS: usize = 14;

/// Estado da aba: o período escolhido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EvolutionView {
    pub period: Period,
}

impl Default for EvolutionView {
    fn default() -> Self {
        Self {
            period: Period::Day,
        }
    }
}

/// Dados prontos para desenhar (derivados das Resoluções; recalculados quando mudam).
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct EvolutionData {
    pub report: Evolution,
    pub by_day: Vec<PeriodStats>,
    pub by_week: Vec<PeriodStats>,
    pub by_profile: Vec<(String, chess_training::Tally)>,
}

impl EvolutionData {
    pub(super) fn compute(runs: &[DatedRun], profiles: &ProfileSet) -> Self {
        let list: Vec<_> = profiles.iter().cloned().collect();
        Self {
            report: chess_training::report(runs),
            by_day: by_period(runs, Period::Day, 0),
            by_week: by_period(runs, Period::Week, 0),
            by_profile: by_training_profile(runs, &list),
        }
    }

    fn periods(&self, period: Period) -> &[PeriodStats] {
        match period {
            Period::Day => &self.by_day,
            Period::Week => &self.by_week,
        }
    }
}

fn pct_text(i18n: &I18n, value: Option<u32>) -> String {
    value.map_or_else(|| i18n.t("evolution-none"), |v| format!("{v}%"))
}

fn date_text((year, month, day): (i32, u32, u32)) -> String {
    format!("{day:02}/{month:02}/{year}")
}

fn short_date((_, month, day): (i32, u32, u32)) -> String {
    format!("{day:02}/{month:02}")
}

/// Barras empilhadas: acertos (azul) e não resolvidos (laranja) por período.
fn chart(ui: &mut egui::Ui, i18n: &I18n, periods: &[PeriodStats]) {
    let shown = &periods[periods.len().saturating_sub(CHART_PERIODS)..];
    let max = shown.iter().map(|p| p.tally.runs).max().unwrap_or(1).max(1);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(200.0), 110.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let correct_color = egui::Color32::from_rgb(54, 122, 196);
    let missed_color = egui::Color32::from_rgb(222, 128, 38);
    let axis = ui.visuals().weak_text_color();
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(24.0, 4.0),
        rect.max - egui::vec2(2.0, 18.0),
    );
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], (1.0, axis));
    painter.line_segment([plot.left_top(), plot.left_bottom()], (1.0, axis));
    painter.text(
        plot.left_top() + egui::vec2(-3.0, 0.0),
        egui::Align2::RIGHT_TOP,
        max.to_string(),
        egui::FontId::proportional(10.0),
        axis,
    );
    painter.text(
        plot.left_bottom() + egui::vec2(-3.0, 0.0),
        egui::Align2::RIGHT_BOTTOM,
        "0",
        egui::FontId::proportional(10.0),
        axis,
    );
    if shown.is_empty() {
        return;
    }
    let slot = plot.width() / shown.len() as f32;
    let width = (slot * 0.62).min(28.0);
    for (i, p) in shown.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let scale = plot.height() / max as f32;
        let correct_h = p.tally.correct as f32 * scale;
        let missed_h = p.tally.incorrect as f32 * scale;
        let base = plot.bottom();
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(cx - width / 2.0, base - correct_h),
                egui::pos2(cx + width / 2.0, base),
            ),
            1.0,
            correct_color,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(cx - width / 2.0, base - correct_h - missed_h),
                egui::pos2(cx + width / 2.0, base - correct_h),
            ),
            1.0,
            missed_color,
        );
        if shown.len() <= 8 || i % 2 == 0 {
            painter.text(
                egui::pos2(cx, base + 3.0),
                egui::Align2::CENTER_TOP,
                short_date(civil_date(p.start_day)),
                egui::FontId::proportional(10.0),
                axis,
            );
        }
    }
    ui.horizontal(|ui| {
        for (color, key) in [
            (correct_color, "evolution-legend-correct"),
            (missed_color, "evolution-legend-missed"),
        ] {
            let (swatch, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(swatch, 2.0, color);
            ui.small(i18n.t(key));
        }
    });
}

fn indicators(ui: &mut egui::Ui, i18n: &I18n, report: &Evolution) {
    {
        {
            let t = &report.total;
            let rows = [
                (
                    "evolution-ind-runs",
                    format!("{} ({} {})", t.runs, t.correct, i18n.t("evolution-solved")),
                ),
                (
                    "evolution-ind-no-aids",
                    pct_text(i18n, report.completed_without_aids_pct),
                ),
                (
                    "evolution-ind-memory-seq",
                    pct_text(i18n, report.sequence_memory_pct),
                ),
                (
                    "evolution-ind-memory-pos",
                    pct_text(i18n, report.position_memory_pct),
                ),
                (
                    "evolution-ind-endgames",
                    match report.endgames.accuracy_pct() {
                        Some(p) => i18n.t_with(
                            "evolution-endgames-value",
                            &[
                                ("pct", &p.to_string()),
                                ("runs", &report.endgames.runs.to_string()),
                                ("stalemate", &report.endgames.stalemate_errors.to_string()),
                            ],
                        ),
                        None => i18n.t("evolution-none"),
                    },
                ),
            ];
            for (key, value) in rows {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}:", i18n.t(key)));
                    ui.strong(value);
                });
            }
        }
    }
}

fn periods_table(ui: &mut egui::Ui, i18n: &I18n, periods: &[PeriodStats], period: Period) {
    egui::Grid::new("evolution-periods")
        .striped(true)
        .num_columns(5)
        .spacing([10.0, 2.0])
        .show(ui, |ui| {
            for key in [
                match period {
                    Period::Day => "evolution-col-day",
                    Period::Week => "evolution-col-week",
                },
                "evolution-col-runs",
                "evolution-col-correct",
                "evolution-col-errors",
                "evolution-col-no-aids",
            ] {
                ui.strong(i18n.t(key));
            }
            ui.end_row();
            for p in periods.iter().rev().take(10) {
                ui.label(date_text(civil_date(p.start_day)));
                ui.label(p.tally.runs.to_string());
                ui.label(p.tally.correct.to_string());
                ui.label(p.tally.errors.to_string());
                ui.label(p.tally.correct_without_aids.to_string());
                ui.end_row();
            }
        });
}

/// Aba Evolução. Devolve `true` se a pessoa pediu para voltar.
pub(super) fn view(ui: &mut egui::Ui, i18n: &I18n, state: &mut TrainingUi) -> bool {
    let mut back = false;
    ui.strong(i18n.t("evolution-heading"));
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            if state.evolution_data.report.total.runs == 0 {
                ui.label(i18n.t("evolution-empty"));
            } else {
                ui.horizontal(|ui| {
                    ui.label(i18n.t("evolution-period"));
                    for (period, key) in [
                        (Period::Day, "evolution-by-day"),
                        (Period::Week, "evolution-by-week"),
                    ] {
                        ui.selectable_value(&mut state.evolution_view.period, period, i18n.t(key));
                    }
                });
                let period = state.evolution_view.period;
                let periods = state.evolution_data.periods(period);
                chart(ui, i18n, periods);
                ui.separator();
                indicators(ui, i18n, &state.evolution_data.report);
                ui.separator();
                periods_table(ui, i18n, periods, period);
                ui.separator();
                ui.strong(i18n.t("evolution-by-profile"));
                for (id, tally) in &state.evolution_data.by_profile {
                    let name = state
                        .profiles
                        .get(id)
                        .map_or(id.as_str(), |p| p.name.as_str());
                    ui.label(i18n.t_with(
                        "evolution-profile-line",
                        &[
                            ("name", name),
                            ("runs", &tally.runs.to_string()),
                            ("correct", &tally.correct.to_string()),
                            ("noaids", &tally.correct_without_aids.to_string()),
                        ],
                    ));
                }
                ui.small(i18n.t("evolution-utc-note"));
            }
        });
    if ui.button(i18n.t("training-back")).clicked() {
        back = true;
    }
    back
}

/// Resoluções sintéticas só para a captura de evidência (`--training evolution`):
/// 24 dias de treino com acertos crescentes.
pub(super) fn demo_runs() -> Vec<DatedRun> {
    use chess_game::PolicyKind;
    use chess_training::{
        Aid, AidsRecord, ErrorCause, ExerciseKind, Feedback, PositionMemorySummary, RunError,
        RunResult, RunSummary,
    };
    const DAY_MS: i64 = 86_400_000;
    let today = 20_763; // 2026-11-06
    let kinds = [
        (ExerciseKind::Stalemate, "afogamento-001"),
        (ExerciseKind::Mate, "mate-em-1-001"),
        (ExerciseKind::Check, "xeque-001"),
        (ExerciseKind::Castling, "roque-001"),
        (ExerciseKind::MemoryPosition, "memoria-posicao-001"),
        (ExerciseKind::Memory, "memoria-italiana-001"),
    ];
    let mut out = Vec::new();
    for back in (0..24i64).rev() {
        if back % 5 == 4 {
            continue;
        }
        let day = today - back;
        let n = 2 + (24 - back) / 6;
        for i in 0..n {
            let (kind, id) = kinds[usize::try_from(i + back).unwrap_or(0) % kinds.len()];
            let ok = (i + (24 - back)) % 4 != 0;
            let with_aids = (i + back) % 3 == 0;
            let percent = u32::try_from(40 + (24 - back) * 2 + i * 3)
                .unwrap_or(100)
                .min(100);
            out.push(DatedRun {
                at_ms: day * DAY_MS + (9 + i) * 3_600_000,
                player: 1,
                summary: RunSummary {
                    exercise_id: id.into(),
                    exercise_version: 1,
                    kind,
                    policy: PolicyKind::Assisted,
                    result: if ok {
                        RunResult::Correct
                    } else {
                        RunResult::Incorrect
                    },
                    errors: if ok {
                        Vec::new()
                    } else {
                        vec![RunError {
                            cause: ErrorCause::OffObjective { uci: "e6e8".into() },
                            fen: String::new(),
                        }]
                    },
                    moves: Vec::new(),
                    aids: AidsRecord::new(
                        if with_aids { &[Aid::Destinations] } else { &[] },
                        Feedback::Immediate,
                    ),
                    duration_ms: 30_000,
                    outcome: None,
                    final_fen: String::new(),
                    started_at_ms: 0,
                    session: 0,
                    memory: None,
                    position_memory: (kind == ExerciseKind::MemoryPosition).then_some(
                        PositionMemorySummary {
                            correct: percent,
                            wrong_piece: 0,
                            missing: 100 - percent,
                            extra: 0,
                            total: 100,
                            percent,
                            study_ms: 10_000,
                            recall_ms: 20_000,
                        },
                    ),
                },
            });
        }
    }
    out
}
