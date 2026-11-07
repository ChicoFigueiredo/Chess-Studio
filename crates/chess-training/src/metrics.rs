//! Métricas por tipo, por sessão e por Exercício (TRN-05). Só leem Resoluções:
//! nenhuma Partida competitiva entra nem sai daqui.

use crate::exercise::ExerciseKind;
use crate::run::{RunResult, RunSummary};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    pub runs: u32,
    pub correct: u32,
    pub incorrect: u32,
    /// Erros (Lance ilegal ou fora do objetivo) somados.
    pub errors: u32,
    /// Acertos sem nenhum Auxílio ligado.
    pub correct_without_aids: u32,
    /// Acertos sem nenhum erro.
    pub first_try_correct: u32,
    /// Memória de sequência: Lances julgados e acertados na primeira tentativa.
    pub memory_plies: u32,
    pub memory_hits: u32,
}

impl Tally {
    pub fn add(&mut self, run: &RunSummary) {
        self.runs += 1;
        self.errors += u32::try_from(run.errors.len()).unwrap_or(u32::MAX);
        if let Some(memory) = &run.memory {
            self.memory_plies += u32::try_from(memory.plies.len()).unwrap_or(u32::MAX);
            self.memory_hits += u32::try_from(memory.hits()).unwrap_or(u32::MAX);
        }
        match run.result {
            RunResult::Correct => {
                self.correct += 1;
                if run.aids.without_aids() {
                    self.correct_without_aids += 1;
                }
                if run.first_try() {
                    self.first_try_correct += 1;
                }
            }
            RunResult::Incorrect => self.incorrect += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metrics {
    pub total: Tally,
    pub by_kind: BTreeMap<ExerciseKind, Tally>,
    pub by_session: BTreeMap<u64, Tally>,
    pub by_exercise: BTreeMap<String, Tally>,
}

impl Metrics {
    pub fn from_runs(runs: &[RunSummary]) -> Self {
        let mut metrics = Self::default();
        for run in runs {
            metrics.total.add(run);
            metrics.by_kind.entry(run.kind).or_default().add(run);
            metrics.by_session.entry(run.session).or_default().add(run);
            metrics
                .by_exercise
                .entry(run.exercise_id.clone())
                .or_default()
                .add(run);
        }
        metrics
    }
}
