//! Memória de sequência (TRN-04, DEC-45): o app **apresenta** uma sequência de 3 a 8
//! Lances a partir da posição do Exercício e o usuário a **reproduz**, jogando os dois
//! lados com a movimentação normal. Cada Lance é julgado contra o esperado.
//!
//! Como [`crate::ExerciseRun`], o juiz só lê os Eventos da Partida (ADR-0001) e devolve
//! [`Followup`]s; a apresentação é só tempo: [`MemoryRun::presentation_ply`] diz qual
//! Lance a interface deve estar mostrando num instante injetado.

use crate::aids::AidsRecord;
use crate::exercise::{Exercise, ExerciseKind, Solution};
use crate::run::{ErrorCause, Followup, RunError, RunResult, RunStatus, RunSummary};
use chess_game::{EventKind, Game, PolicyKind, Timestamp};
use serde::{Deserialize, Serialize};

const MAX_DRIVE_ROUNDS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPhase {
    /// O app anima a sequência; a Partida não aceita Lances.
    Presentation,
    /// A pessoa reproduz a sequência a partir do início.
    Recall,
    /// Sequência completa.
    Done,
}

/// Resultado de um Lance da sequência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlyResult {
    /// Ainda não jogado (ou a Resolução acabou antes dele).
    Pending,
    /// Certo na primeira tentativa.
    Hit,
    /// Houve pelo menos uma tentativa errada neste Lance.
    Miss,
}

/// Resultado de memória guardado na Resolução encerrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySummary {
    pub plies: Vec<PlyResult>,
    /// Índice (0 = primeiro Lance) do primeiro Lance errado.
    pub first_error: Option<usize>,
    /// Quantas vezes a pessoa pediu "mostrar de novo".
    pub replays: u32,
}

impl MemorySummary {
    pub fn hits(&self) -> usize {
        self.plies.iter().filter(|p| **p == PlyResult::Hit).count()
    }
}

#[derive(Debug, Clone)]
pub struct MemoryRun {
    exercise: Exercise,
    line: Vec<String>,
    speed_ms: u32,
    phase: MemoryPhase,
    aids: AidsRecord,
    errors: Vec<RunError>,
    plies: Vec<PlyResult>,
    cursor: usize,
    presentation_start_ms: u64,
    replays: u32,
    started_at_ms: u64,
}

impl MemoryRun {
    /// Começa pela apresentação; `at_ms` é o instante injetado. O Exercício precisa ter
    /// passado pela validação do catálogo (tipo `memory`).
    pub fn start(exercise: &Exercise, aids: AidsRecord, at_ms: u64) -> Self {
        let (line, speed_ms) = match &exercise.solution {
            Solution::Memorize { line, speed_ms } => (line.clone(), *speed_ms),
            _ => (Vec::new(), 1200),
        };
        Self {
            plies: vec![PlyResult::Pending; line.len()],
            exercise: exercise.clone(),
            line,
            speed_ms: speed_ms.max(1),
            phase: MemoryPhase::Presentation,
            aids,
            errors: Vec::new(),
            cursor: 0,
            presentation_start_ms: at_ms,
            replays: 0,
            started_at_ms: at_ms,
        }
    }

    pub fn exercise(&self) -> &Exercise {
        &self.exercise
    }

    pub fn line(&self) -> &[String] {
        &self.line
    }

    pub fn speed_ms(&self) -> u32 {
        self.speed_ms
    }

    pub fn phase(&self) -> MemoryPhase {
        self.phase
    }

    pub fn status(&self) -> RunStatus {
        if self.phase == MemoryPhase::Done {
            RunStatus::Solved
        } else {
            RunStatus::InProgress
        }
    }

    pub fn plies(&self) -> &[PlyResult] {
        &self.plies
    }

    pub fn errors(&self) -> &[RunError] {
        &self.errors
    }

    pub fn aids(&self) -> &AidsRecord {
        &self.aids
    }

    pub fn first_error(&self) -> Option<usize> {
        self.plies.iter().position(|p| *p == PlyResult::Miss)
    }

    pub fn note_aids(&mut self, current: &AidsRecord) {
        self.aids.merge(current);
    }

    /// Quantos Lances da sequência já estão no tabuleiro no instante `now_ms` da
    /// apresentação (0 = posição inicial). Fica no último Lance até a fase acabar.
    pub fn presentation_ply(&self, now_ms: u64) -> usize {
        let elapsed = now_ms.saturating_sub(self.presentation_start_ms);
        let step = elapsed / u64::from(self.speed_ms);
        usize::try_from(step).map_or(self.line.len(), |s| s.min(self.line.len()))
    }

    /// A apresentação terminou: mostrou o último Lance e o segurou por um intervalo.
    pub fn presentation_finished(&self, now_ms: u64) -> bool {
        let elapsed = now_ms.saturating_sub(self.presentation_start_ms);
        elapsed >= (self.line.len() as u64 + 1) * u64::from(self.speed_ms)
    }

    /// Volta ao início e passa à reprodução. O controlador oferece uma Partida **nova**
    /// do Exercício (a anterior, se houver, é descartada).
    pub fn begin_recall(&mut self) {
        if self.phase != MemoryPhase::Done {
            self.phase = MemoryPhase::Recall;
        }
        self.cursor = 0;
    }

    /// "Mostrar de novo": volta à apresentação em `at_ms`. O que já foi julgado fica.
    pub fn show_again(&mut self, at_ms: u64) {
        if self.phase == MemoryPhase::Done {
            return;
        }
        self.phase = MemoryPhase::Presentation;
        self.presentation_start_ms = at_ms;
        self.replays += 1;
    }

    /// Lê os Eventos novos da Partida de reprodução e julga os Lances.
    pub fn observe(&mut self, game: &Game) -> Vec<Followup> {
        let events = game.events();
        let new = events.get(self.cursor..).unwrap_or(&[]);
        self.cursor = events.len();
        if self.phase != MemoryPhase::Recall {
            return Vec::new();
        }
        let snapshot = game.snapshot();
        let mut touched = false;
        for event in new {
            match &event.kind {
                EventKind::IllegalMoveRejected {
                    from, to, reason, ..
                } => {
                    let idx = snapshot.history.len();
                    if let Some(slot) = self.plies.get_mut(idx) {
                        *slot = PlyResult::Miss;
                    }
                    self.errors.push(RunError {
                        cause: ErrorCause::IllegalMove {
                            from: from.to_string(),
                            to: to.to_string(),
                            reason: *reason,
                        },
                        fen: snapshot.position.to_fen(),
                    });
                }
                EventKind::MoveCompleted { .. }
                | EventKind::MoveRedone { .. }
                | EventKind::MoveUndone { .. } => touched = true,
                _ => {}
            }
        }
        if !touched {
            return Vec::new();
        }
        let history = &snapshot.history;
        let mismatch = history
            .iter()
            .zip(&self.line)
            .position(|(h, expected)| h.mv.to_string() != *expected);
        let matched = mismatch.unwrap_or(history.len().min(self.line.len()));
        for slot in self.plies.iter_mut().take(matched) {
            if *slot == PlyResult::Pending {
                *slot = PlyResult::Hit;
            }
        }
        if let Some(idx) = mismatch {
            if self.plies[idx] != PlyResult::Miss || new_completion(new) {
                self.plies[idx] = PlyResult::Miss;
                self.errors.push(RunError {
                    cause: ErrorCause::OffObjective {
                        uci: history[idx].mv.to_string(),
                    },
                    fen: idx
                        .checked_sub(1)
                        .and_then(|ply| game.review_at_ply(ply))
                        .map_or_else(|| self.exercise.fen.clone(), |v| v.position.to_fen()),
                });
            }
            return vec![Followup::Undo];
        }
        if history.len() >= self.line.len() {
            self.phase = MemoryPhase::Done;
        }
        Vec::new()
    }

    /// Aplica os `Followup`s à Partida até o juiz não pedir mais nada (testes).
    pub fn drive(&mut self, game: &mut Game, at_ms: u64) {
        for _ in 0..MAX_DRIVE_ROUNDS {
            let followups = self.observe(game);
            if followups.is_empty() {
                return;
            }
            for followup in followups {
                for command in followup.commands() {
                    let _ = game.handle(command, Timestamp::mono(at_ms));
                }
            }
        }
    }

    /// Encerra (sequência completa ou desistência) e devolve o registro imutável.
    pub fn finish(self, game: &Game, at_ms: u64) -> RunSummary {
        let snapshot = game.snapshot();
        let memory = MemorySummary {
            first_error: self.first_error(),
            plies: self.plies.clone(),
            replays: self.replays,
        };
        RunSummary {
            exercise_id: self.exercise.id,
            exercise_version: self.exercise.version,
            kind: ExerciseKind::Memory,
            policy: PolicyKind::Assisted,
            result: if self.phase == MemoryPhase::Done {
                RunResult::Correct
            } else {
                RunResult::Incorrect
            },
            errors: self.errors,
            moves: snapshot.history.iter().map(|h| h.mv.to_string()).collect(),
            aids: self.aids,
            duration_ms: at_ms.saturating_sub(self.started_at_ms),
            outcome: snapshot.outcome,
            final_fen: snapshot.position.to_fen(),
            started_at_ms: self.started_at_ms,
            session: 0,
            memory: Some(memory),
        }
    }
}

/// Entre os Eventos novos há um Lance jogado (e não só um desfazer)?
fn new_completion(events: &[chess_game::Event]) -> bool {
    events.iter().any(|e| {
        matches!(
            e.kind,
            EventKind::MoveCompleted { .. } | EventKind::MoveRedone { .. }
        )
    })
}
