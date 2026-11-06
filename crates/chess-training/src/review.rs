//! Revisão ao fim da Resolução: explicação, solução e, no afogamento, por que
//! não é xeque-mate (TRN-01, BDD-05).

use crate::aids::AidsRecord;
use crate::exercise::{Exercise, ExerciseKind, Solution};
use crate::run::{RunError, RunResult, RunSummary};
use chess_core::{CanonicalPosition, GameStatus};
use chess_game::{EndReason, Outcome};

/// Fatos da posição de afogamento que a interface traduz na explicação: o rei **não**
/// está em xeque e mesmo assim não há Lance legal (com xeque seria xeque-mate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotCheckmate {
    pub king_in_check: bool,
    pub legal_moves: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub exercise_id: String,
    pub title: String,
    pub objective: String,
    pub result: RunResult,
    /// Explicação do catálogo.
    pub explanation: String,
    /// Solução em SAN (uma resposta possível quando o objetivo é um critério).
    pub solution_san: Vec<String>,
    /// Só em Exercícios de afogamento.
    pub not_checkmate: Option<NotCheckmate>,
    pub errors: Vec<RunError>,
    pub outcome: Option<Outcome>,
    pub aids: AidsRecord,
    pub duration_ms: u64,
}

impl Review {
    pub fn new(exercise: &Exercise, summary: &RunSummary) -> Self {
        // Resolvido por critério: mostra o Lance que a pessoa fez; senão, uma resposta possível.
        let line = if summary.result == RunResult::Correct
            && !matches!(exercise.solution, Solution::Moves { .. })
        {
            summary
                .moves
                .iter()
                .filter_map(|m| m.parse().ok())
                .collect()
        } else {
            exercise.solution_line()
        };
        let mut solution_san = Vec::new();
        let mut position = exercise.position().ok();
        let mut after_solution = position.clone();
        for mv in &line {
            let Some(current) = position.take() else {
                break;
            };
            if let Ok(san) = current.san(*mv) {
                solution_san.push(san);
            }
            position = current.play(*mv).ok();
            after_solution.clone_from(&position);
        }
        let not_checkmate = (exercise.kind == ExerciseKind::Stalemate)
            .then(|| {
                // Prefere a posição que a pessoa de fato alcançou.
                let reached = summary
                    .outcome
                    .filter(|o| o.reason == EndReason::Stalemate)
                    .and_then(|_| CanonicalPosition::from_fen(&summary.final_fen).ok());
                reached.or(after_solution)
            })
            .flatten()
            .filter(|p| matches!(p.status(), GameStatus::Stalemate))
            .map(|p| NotCheckmate {
                king_in_check: p.is_check(),
                legal_moves: p.legal_moves().len(),
            });
        Self {
            exercise_id: exercise.id.clone(),
            title: exercise.title.clone(),
            objective: exercise.objective.clone(),
            result: summary.result,
            explanation: exercise.explanation.clone(),
            solution_san,
            not_checkmate,
            errors: summary.errors.clone(),
            outcome: summary.outcome,
            aids: summary.aids.clone(),
            duration_ms: summary.duration_ms,
        }
    }
}
