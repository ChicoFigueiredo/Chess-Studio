//! Evolução do estudante (US-26): métricas por período (dia/semana), por jogador e por
//! Perfil de treino; % de Exercícios concluídos sem Auxílios; acerto médio de memória
//! (sequência e posição); desempenho em finais, com os erros de afogamento à parte.
//!
//! Funções puras sobre Resoluções já gravadas: nenhuma Partida competitiva entra (TRN-05)
//! e o relógio do sistema nunca é lido (o fuso entra por parâmetro).

use crate::exercise::ExerciseKind;
use crate::metrics::Tally;
use crate::profiles::TrainingProfile;
use crate::run::{RunResult, RunSummary};
use chess_game::EndReason;
use std::collections::BTreeMap;

/// Uma Resolução encerrada com o instante em que foi gravada.
#[derive(Debug, Clone, PartialEq)]
pub struct DatedRun {
    /// Milissegundos desde 1970-01-01 UTC.
    pub at_ms: i64,
    /// Perfil do jogador (id do banco).
    pub player: i64,
    pub summary: RunSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    /// Semana que começa na segunda-feira.
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodStats {
    /// Primeiro dia do período, em dias desde 1970-01-01 (no fuso pedido).
    pub start_day: i64,
    pub tally: Tally,
}

/// Desempenho em finais: Exercícios de afogamento e de mate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Endgames {
    pub runs: u32,
    pub correct: u32,
    /// Erros (Lances fora do objetivo ou ilegais) somados.
    pub errors: u32,
    /// Erros de afogamento: Exercício de afogamento resolvido com erro ou desistido, e
    /// Exercício de mate que terminou afogado sem querer.
    pub stalemate_errors: u32,
}

impl Endgames {
    /// Acerto em %, `None` sem Resoluções.
    pub fn accuracy_pct(&self) -> Option<u32> {
        pct(self.correct, self.runs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Evolution {
    pub total: Tally,
    /// Dos Exercícios concluídos (acertados), quantos % sem nenhum Auxílio. `None` se
    /// nenhum foi concluído.
    pub completed_without_aids_pct: Option<u32>,
    /// Acerto médio (%) da memória de sequência.
    pub sequence_memory_pct: Option<u32>,
    /// Acerto médio (%) da memória de posição.
    pub position_memory_pct: Option<u32>,
    pub endgames: Endgames,
}

/// Acerto de uma Resolução de memória de sequência, em %: Lances certos na primeira
/// tentativa sobre o total. Sem o detalhe por Lance (registros antigos), vale 100 ou 0.
fn sequence_score(run: &RunSummary) -> u32 {
    if let Some(memory) = run.memory.as_ref().filter(|m| !m.plies.is_empty()) {
        let total = u32::try_from(memory.plies.len()).unwrap_or(u32::MAX);
        let hits = u32::try_from(memory.hits()).unwrap_or(u32::MAX);
        return (hits * 100 + total / 2) / total;
    }
    if run.result == RunResult::Correct {
        100
    } else {
        0
    }
}

/// Média inteira arredondada; `None` sem amostras.
fn average(values: &[u32]) -> Option<u32> {
    let n = u32::try_from(values.len()).ok().filter(|n| *n > 0)?;
    Some((values.iter().sum::<u32>() + n / 2) / n)
}

fn pct(part: u32, whole: u32) -> Option<u32> {
    (whole > 0).then(|| (part * 100 + whole / 2) / whole)
}

/// Dia (desde 1970-01-01) de um instante, no fuso `utc_offset_min` (minutos a leste de UTC).
pub fn day_of(at_ms: i64, utc_offset_min: i32) -> i64 {
    (at_ms + i64::from(utc_offset_min) * 60_000).div_euclid(86_400_000)
}

/// Data civil (ano, mês, dia) de um dia desde 1970-01-01.
pub fn civil_date(day: i64) -> (i32, u32, u32) {
    // Algoritmo de Howard Hinnant (datas civis proléticas do calendário gregoriano).
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        i32::try_from(y).unwrap_or(i32::MAX),
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// Resoluções agrupadas por dia ou semana, em ordem cronológica. Períodos sem
/// Resoluções não aparecem.
pub fn by_period(runs: &[DatedRun], period: Period, utc_offset_min: i32) -> Vec<PeriodStats> {
    let mut groups: BTreeMap<i64, Tally> = BTreeMap::new();
    for run in runs {
        let day = day_of(run.at_ms, utc_offset_min);
        let start = match period {
            Period::Day => day,
            // 1970-01-01 foi quinta-feira: (dia + 3) mod 7 conta a partir da segunda (0).
            Period::Week => day - (day + 3).rem_euclid(7),
        };
        groups.entry(start).or_default().add(&run.summary);
    }
    groups
        .into_iter()
        .map(|(start_day, tally)| PeriodStats { start_day, tally })
        .collect()
}

/// Totais por jogador (Perfil do banco).
pub fn by_player(runs: &[DatedRun]) -> BTreeMap<i64, Tally> {
    let mut by: BTreeMap<i64, Tally> = BTreeMap::new();
    for run in runs {
        by.entry(run.player).or_default().add(&run.summary);
    }
    by
}

/// Totais por Perfil de treino: só contam as Resoluções de Exercícios que o Perfil
/// inclui (um Exercício pode estar em vários Perfis).
pub fn by_training_profile(
    runs: &[DatedRun],
    profiles: &[TrainingProfile],
) -> Vec<(String, Tally)> {
    profiles
        .iter()
        .map(|profile| {
            let mut tally = Tally::default();
            for run in runs {
                if profile.contains(&run.summary.exercise_id) {
                    tally.add(&run.summary);
                }
            }
            (profile.id.clone(), tally)
        })
        .collect()
}

/// Indicadores gerais do conjunto de Resoluções.
pub fn report(runs: &[DatedRun]) -> Evolution {
    let mut total = Tally::default();
    let mut endgames = Endgames::default();
    let (mut sequence, mut position) = (Vec::new(), Vec::new());
    for run in runs {
        let summary = &run.summary;
        total.add(summary);
        match summary.kind {
            ExerciseKind::Memory => sequence.push(sequence_score(summary)),
            ExerciseKind::MemoryPosition => {
                if let Some(memory) = &summary.position_memory {
                    position.push(memory.percent);
                }
            }
            ExerciseKind::Stalemate | ExerciseKind::Mate => {
                endgames.runs += 1;
                endgames.errors += u32::try_from(summary.errors.len()).unwrap_or(u32::MAX);
                if summary.result == RunResult::Correct {
                    endgames.correct += 1;
                }
                let missed_stalemate =
                    summary.kind == ExerciseKind::Stalemate && !summary.first_try();
                let stalemated_by_accident = summary.kind == ExerciseKind::Mate
                    && summary
                        .outcome
                        .is_some_and(|o| o.reason == EndReason::Stalemate);
                if missed_stalemate || stalemated_by_accident {
                    endgames.stalemate_errors += 1;
                }
            }
            _ => {}
        }
    }
    Evolution {
        completed_without_aids_pct: pct(total.correct_without_aids, total.correct),
        sequence_memory_pct: average(&sequence),
        position_memory_pct: average(&position),
        total,
        endgames,
    }
}
