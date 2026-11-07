//! Memória de posição (C11, US-25): o app mostra a Posição do Exercício por alguns
//! segundos, esvazia o tabuleiro e a pessoa **recoloca as peças** a partir de uma bandeja.
//! "Conferir" julga casa a casa: peça certa, peça errada, faltando e sobrando.
//!
//! Como o resto do treino, nada aqui depende de Bevy, do banco ou do relógio do sistema:
//! o tempo entra por parâmetro e o editor (arrastar, mover, remover) é só estado.

use crate::aids::AidsRecord;
use crate::exercise::{Exercise, ExerciseKind, Solution};
use crate::run::{RunResult, RunSummary};
use chess_core::{CanonicalPosition, Color, FenError, Piece, Square};
use chess_game::PolicyKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

type Placed = (Color, Piece);

/// Peças sobre as casas, sem regras: o que a pessoa montou (ou a Posição a lembrar).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PiecePlacement {
    pieces: HashMap<Square, Placed>,
}

/// Todas as casas, da fileira 8 para a 1 e da coluna a para a h (ordem de leitura).
pub fn squares_in_reading_order() -> impl Iterator<Item = Square> {
    (0..8u8)
        .rev()
        .flat_map(|rank| (0..8u8).filter_map(move |file| Square::new(file, rank)))
}

impl PiecePlacement {
    pub fn empty() -> Self {
        Self::default()
    }

    /// As peças de um FEN (só a parte das casas importa).
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let position = CanonicalPosition::from_fen(fen)?;
        Ok(Self::from_position(&position))
    }

    pub fn from_position(position: &CanonicalPosition) -> Self {
        Self {
            pieces: squares_in_reading_order()
                .filter_map(|s| position.piece_at(s).map(|cp| (s, cp)))
                .collect(),
        }
    }

    pub fn get(&self, square: Square) -> Option<(Color, Piece)> {
        self.pieces.get(&square).copied()
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// Coloca a peça (substitui a que estiver na casa).
    pub fn place(&mut self, square: Square, color: Color, piece: Piece) {
        self.pieces.insert(square, (color, piece));
    }

    pub fn remove(&mut self, square: Square) -> Option<(Color, Piece)> {
        self.pieces.remove(&square)
    }

    /// Move a peça de `from` para `to` (a que estiver em `to` é substituída). `false` se
    /// `from` está vazia.
    pub fn move_piece(&mut self, from: Square, to: Square) -> bool {
        match self.pieces.remove(&from) {
            Some(p) => {
                self.pieces.insert(to, p);
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.pieces.clear();
    }

    /// Peças em ordem de leitura.
    pub fn iter(&self) -> impl Iterator<Item = (Square, Color, Piece)> + '_ {
        squares_in_reading_order().filter_map(|s| self.pieces.get(&s).map(|&(c, p)| (s, c, p)))
    }

    pub fn to_map(&self) -> HashMap<Square, Placed> {
        self.pieces.clone()
    }
}

/// O que há de errado (ou certo) numa casa depois de "Conferir".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquareVerdict {
    /// A peça certa na casa certa.
    Correct(Placed),
    /// Há peça, mas não a esperada.
    WrongPiece { expected: Placed, placed: Placed },
    /// Devia haver uma peça e a casa está vazia.
    Missing(Placed),
    /// Há uma peça onde a casa devia estar vazia.
    Extra(Placed),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionJudgment {
    /// Casas com algo a dizer (certas, erradas, faltando, sobrando), em ordem de leitura.
    pub squares: Vec<(Square, SquareVerdict)>,
    pub correct: u32,
    pub wrong_piece: u32,
    pub missing: u32,
    pub extra: u32,
    /// Peças da Posição a lembrar.
    pub target_total: u32,
}

impl PositionJudgment {
    /// Acerto em %: peças certas sobre (peças da Posição + peças sobrando), arredondado.
    /// Só dá 100 sem nenhum erro; colocar peça a mais também custa.
    pub fn percent(&self) -> u32 {
        let denominator = self.target_total + self.extra;
        if denominator == 0 {
            return 100;
        }
        (self.correct * 100 + denominator / 2) / denominator
    }

    pub fn is_perfect(&self) -> bool {
        self.wrong_piece == 0 && self.missing == 0 && self.extra == 0
    }

    pub fn verdict_at(&self, square: Square) -> Option<SquareVerdict> {
        self.squares
            .iter()
            .find(|(s, _)| *s == square)
            .map(|(_, v)| *v)
    }
}

/// Compara o que a pessoa montou (`placed`) com a Posição a lembrar (`target`).
pub fn judge(target: &PiecePlacement, placed: &PiecePlacement) -> PositionJudgment {
    let mut judgment = PositionJudgment {
        squares: Vec::new(),
        correct: 0,
        wrong_piece: 0,
        missing: 0,
        extra: 0,
        target_total: u32::try_from(target.len()).unwrap_or(u32::MAX),
    };
    for square in squares_in_reading_order() {
        let verdict = match (target.get(square), placed.get(square)) {
            (None, None) => continue,
            (Some(expected), Some(got)) if expected == got => {
                judgment.correct += 1;
                SquareVerdict::Correct(got)
            }
            (Some(expected), Some(got)) => {
                judgment.wrong_piece += 1;
                SquareVerdict::WrongPiece {
                    expected,
                    placed: got,
                }
            }
            (Some(expected), None) => {
                judgment.missing += 1;
                SquareVerdict::Missing(expected)
            }
            (None, Some(got)) => {
                judgment.extra += 1;
                SquareVerdict::Extra(got)
            }
        };
        judgment.squares.push((square, verdict));
    }
    judgment
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionPhase {
    /// A Posição está à vista; o tempo corre.
    Study,
    /// Tabuleiro vazio: a pessoa recoloca as peças.
    Recall,
    /// Conferido.
    Done,
}

/// Resultado de memória de posição guardado na Resolução encerrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionMemorySummary {
    pub correct: u32,
    pub wrong_piece: u32,
    pub missing: u32,
    pub extra: u32,
    pub total: u32,
    pub percent: u32,
    /// Tempo que a Posição ficou à vista.
    pub study_ms: u64,
    /// Tempo para recolocar as peças (até conferir ou desistir).
    pub recall_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PositionMemoryRun {
    exercise: Exercise,
    target: PiecePlacement,
    study_ms: u64,
    phase: PositionPhase,
    placed: PiecePlacement,
    aids: AidsRecord,
    started_at_ms: u64,
    recall_started_ms: Option<u64>,
    studied_ms: u64,
    judgment: Option<PositionJudgment>,
    finished_at_ms: Option<u64>,
}

impl PositionMemoryRun {
    /// Começa pelo estudo. O Exercício precisa ser do tipo `memory_position` (validado
    /// pelo catálogo); `at_ms` é o instante injetado.
    pub fn start(exercise: &Exercise, aids: AidsRecord, at_ms: u64) -> Self {
        let seconds = match exercise.solution {
            Solution::MemorizePosition { seconds } => seconds,
            _ => 10,
        };
        Self {
            target: PiecePlacement::from_fen(&exercise.fen).unwrap_or_default(),
            exercise: exercise.clone(),
            study_ms: u64::from(seconds) * 1000,
            phase: PositionPhase::Study,
            placed: PiecePlacement::empty(),
            aids,
            started_at_ms: at_ms,
            recall_started_ms: None,
            studied_ms: 0,
            judgment: None,
            finished_at_ms: None,
        }
    }

    pub fn exercise(&self) -> &Exercise {
        &self.exercise
    }

    pub fn phase(&self) -> PositionPhase {
        self.phase
    }

    /// A Posição a lembrar (só deve ser mostrada na fase de estudo e na revisão).
    pub fn target(&self) -> &PiecePlacement {
        &self.target
    }

    /// O que a pessoa montou até agora.
    pub fn placed(&self) -> &PiecePlacement {
        &self.placed
    }

    pub fn judgment(&self) -> Option<&PositionJudgment> {
        self.judgment.as_ref()
    }

    pub fn aids(&self) -> &AidsRecord {
        &self.aids
    }

    pub fn note_aids(&mut self, current: &AidsRecord) {
        self.aids.merge(current);
    }

    pub fn study_ms(&self) -> u64 {
        self.study_ms
    }

    /// Quanto falta de estudo no instante `now_ms` (0 quando acabou).
    pub fn study_remaining_ms(&self, now_ms: u64) -> u64 {
        let elapsed = now_ms.saturating_sub(self.started_at_ms);
        self.study_ms.saturating_sub(elapsed)
    }

    /// O tempo de estudo acabou e a fase ainda é de estudo?
    pub fn study_finished(&self, now_ms: u64) -> bool {
        self.phase == PositionPhase::Study && self.study_remaining_ms(now_ms) == 0
    }

    /// Passa à reconstrução (no fim do tempo ou quando a pessoa diz "já memorizei").
    pub fn begin_recall(&mut self, at_ms: u64) {
        if self.phase != PositionPhase::Study {
            return;
        }
        self.studied_ms = at_ms.saturating_sub(self.started_at_ms).min(self.study_ms);
        self.recall_started_ms = Some(at_ms);
        self.placed.clear();
        self.phase = PositionPhase::Recall;
    }

    /// Coloca uma peça da bandeja numa casa. Só vale na reconstrução.
    pub fn place(&mut self, square: Square, color: Color, piece: Piece) -> bool {
        if self.phase != PositionPhase::Recall {
            return false;
        }
        self.placed.place(square, color, piece);
        true
    }

    /// Move uma peça já colocada. Só vale na reconstrução.
    pub fn move_piece(&mut self, from: Square, to: Square) -> bool {
        self.phase == PositionPhase::Recall && self.placed.move_piece(from, to)
    }

    /// Tira uma peça colocada. Só vale na reconstrução.
    pub fn remove(&mut self, square: Square) -> bool {
        self.phase == PositionPhase::Recall && self.placed.remove(square).is_some()
    }

    /// Esvazia o que foi montado.
    pub fn clear(&mut self) {
        if self.phase == PositionPhase::Recall {
            self.placed.clear();
        }
    }

    /// "Conferir": julga e encerra a reconstrução.
    pub fn check(&mut self, at_ms: u64) -> &PositionJudgment {
        if self.phase == PositionPhase::Recall {
            self.recall_started_ms.get_or_insert(at_ms);
            self.phase = PositionPhase::Done;
            self.judgment = Some(judge(&self.target, &self.placed));
            self.finished_at_ms = Some(at_ms);
        }
        self.judgment
            .get_or_insert_with(|| judge(&self.target, &self.placed))
    }

    /// Encerra (conferido, ou desistência/saída) e devolve o registro imutável.
    pub fn finish(self, at_ms: u64) -> RunSummary {
        let recall_ms = self.recall_started_ms.map_or(0, |start| {
            self.finished_at_ms.unwrap_or(at_ms).saturating_sub(start)
        });
        // Desistir durante o estudo: só conta o tempo que a Posição ficou à vista.
        let study_ms = if self.recall_started_ms.is_some() {
            self.studied_ms
        } else {
            at_ms.saturating_sub(self.started_at_ms).min(self.study_ms)
        };
        let judgment = self
            .judgment
            .unwrap_or_else(|| judge(&self.target, &self.placed));
        let summary = PositionMemorySummary {
            correct: judgment.correct,
            wrong_piece: judgment.wrong_piece,
            missing: judgment.missing,
            extra: judgment.extra,
            total: judgment.target_total,
            percent: if self.recall_started_ms.is_some() {
                judgment.percent()
            } else {
                0
            },
            study_ms,
            recall_ms,
        };
        RunSummary {
            exercise_id: self.exercise.id,
            exercise_version: self.exercise.version,
            kind: ExerciseKind::MemoryPosition,
            policy: PolicyKind::Assisted,
            result: if self.recall_started_ms.is_some() && judgment.is_perfect() {
                RunResult::Correct
            } else {
                RunResult::Incorrect
            },
            errors: Vec::new(),
            moves: Vec::new(),
            aids: self.aids,
            duration_ms: at_ms.saturating_sub(self.started_at_ms),
            outcome: None,
            final_fen: self.exercise.fen,
            started_at_ms: self.started_at_ms,
            session: 0,
            memory: None,
            position_memory: Some(summary),
        }
    }
}
