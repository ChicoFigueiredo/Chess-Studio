//! A Resolução: o juiz de uma tentativa de Exercício sobre uma Partida Assistido.

use crate::aids::AidsRecord;
use crate::exercise::{Exercise, ExerciseKind, Solution, satisfied};
use chess_core::{Color, Move};
use chess_game::{Command, EventKind, Game, IllegalReason, Outcome, PolicyKind, Timestamp};
use serde::{Deserialize, Serialize};

/// Quantas rodadas juiz-controlador `drive` aceita por chamada (proteção contra laço).
const MAX_DRIVE_ROUNDS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorCause {
    /// Lance ilegal (a Partida o recusou; o motivo vem das Leis, AC-03).
    IllegalMove {
        from: String,
        to: String,
        reason: IllegalReason,
    },
    /// Lance legal que não cumpre o objetivo; é desfeito.
    OffObjective { uci: String },
}

/// Um erro da Resolução: a causa e a Posição (FEN) em que aconteceu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunError {
    pub cause: ErrorCause,
    pub fen: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    InProgress,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    Correct,
    Incorrect,
}

/// O que o controlador da Partida deve fazer em seguida a pedido da Resolução.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Followup {
    /// Desfazer um Lance (fora do objetivo, ou a resposta do adversário ao desfazer).
    Undo,
    /// Resposta do adversário prevista na sequência.
    Play(Move),
}

impl Followup {
    pub fn commands(self) -> Vec<Command> {
        match self {
            Followup::Undo => vec![Command::Undo],
            Followup::Play(mv) => {
                let mut commands = vec![Command::Release {
                    from: mv.from(),
                    to: mv.to(),
                }];
                if let Some(piece) = mv.promotion() {
                    commands.push(Command::ChoosePromotion { piece });
                }
                commands
            }
        }
    }
}

/// Registro imutável de uma Resolução encerrada. Repetir o Exercício cria **outra**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub exercise_id: String,
    pub exercise_version: u32,
    pub kind: ExerciseKind,
    pub policy: PolicyKind,
    pub result: RunResult,
    pub errors: Vec<RunError>,
    /// Lances (UCI) que ficaram na Partida ao encerrar.
    pub moves: Vec<String>,
    pub aids: AidsRecord,
    pub duration_ms: u64,
    /// Resultado da Partida do Exercício (ex.: empate por afogamento), se terminou.
    pub outcome: Option<Outcome>,
    pub final_fen: String,
    pub started_at_ms: u64,
    /// Sessão do app em que a Resolução aconteceu (agrupa as métricas).
    pub session: u64,
}

impl RunSummary {
    pub fn in_session(mut self, session: u64) -> Self {
        self.session = session;
        self
    }

    /// Acertou sem nenhum erro.
    pub fn first_try(&self) -> bool {
        self.result == RunResult::Correct && self.errors.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ExerciseRun {
    exercise: Exercise,
    user_side: Color,
    aids: AidsRecord,
    errors: Vec<RunError>,
    cursor: usize,
    solved: bool,
    started_at_ms: u64,
}

impl ExerciseRun {
    /// Começa uma Resolução; `at_ms` é o instante injetado do início. O Exercício
    /// precisa ter passado pela validação do catálogo.
    pub fn start(exercise: &Exercise, aids: AidsRecord, at_ms: u64) -> Self {
        let user_side = exercise
            .position()
            .map_or(Color::White, |p| p.side_to_move());
        Self {
            exercise: exercise.clone(),
            user_side,
            aids,
            errors: Vec::new(),
            cursor: 0,
            solved: false,
            started_at_ms: at_ms,
        }
    }

    pub fn exercise(&self) -> &Exercise {
        &self.exercise
    }

    pub fn status(&self) -> RunStatus {
        if self.solved {
            RunStatus::Solved
        } else {
            RunStatus::InProgress
        }
    }

    pub fn errors(&self) -> &[RunError] {
        &self.errors
    }

    pub fn aids(&self) -> &AidsRecord {
        &self.aids
    }

    /// Auxílios ligados agora: um Auxílio ligado a qualquer momento conta como usado (TRN-03).
    pub fn note_aids(&mut self, current: &AidsRecord) {
        self.aids.merge(current);
    }

    /// Lê os Eventos novos da Partida e devolve o que o controlador deve aplicar.
    pub fn observe(&mut self, game: &Game) -> Vec<Followup> {
        let events = game.events();
        let new = events.get(self.cursor..).unwrap_or(&[]);
        self.cursor = events.len();
        if self.solved {
            return Vec::new();
        }
        let snapshot = game.snapshot();
        let mut last_completed = None;
        let mut undone = false;
        for event in new {
            match &event.kind {
                EventKind::IllegalMoveRejected {
                    side,
                    from,
                    to,
                    reason,
                } if *side == self.user_side => {
                    let (from, to) = (from.to_string(), to.to_string());
                    let refused = matches!(&self.exercise.solution,
                        Solution::RefuseIllegal { uci } if *uci == format!("{from}{to}"));
                    if refused {
                        self.solved = true;
                    } else {
                        self.errors.push(RunError {
                            cause: ErrorCause::IllegalMove {
                                from,
                                to,
                                reason: *reason,
                            },
                            fen: snapshot.position.to_fen(),
                        });
                    }
                }
                EventKind::MoveCompleted { side, mv, .. }
                | EventKind::MoveRedone { side, mv, .. }
                    if *side == self.user_side =>
                {
                    last_completed = Some(*mv);
                }
                EventKind::MoveUndone { .. } => {
                    undone = true;
                }
                _ => {}
            }
        }
        if self.solved {
            return Vec::new();
        }
        if let Some(mv) = last_completed {
            return self.judge(game, mv);
        }
        // Desfazer a resposta do adversário deixaria a vez com ele: desfaz o par.
        if undone && snapshot.side_to_move != self.user_side && snapshot.can_undo {
            return vec![Followup::Undo];
        }
        Vec::new()
    }

    fn judge(&mut self, game: &Game, mv: Move) -> Vec<Followup> {
        let snapshot = game.snapshot();
        let n = snapshot.history.len();
        let on_objective = match &self.exercise.solution {
            Solution::Moves { line } => {
                let on_track = snapshot
                    .history
                    .iter()
                    .zip(line)
                    .all(|(h, expected)| h.mv.to_string() == *expected);
                if on_track && n >= line.len() {
                    self.solved = true;
                    return Vec::new();
                }
                if on_track {
                    // Resposta do adversário prevista (após o Lance do usuário).
                    return match line.get(n).and_then(|u| u.parse::<Move>().ok()) {
                        Some(reply) if snapshot.side_to_move != self.user_side => {
                            vec![Followup::Play(reply)]
                        }
                        _ => Vec::new(),
                    };
                }
                false
            }
            Solution::RefuseIllegal { .. } => false,
            criterion => satisfied(criterion, &snapshot.position),
        };
        if on_objective {
            self.solved = true;
            return Vec::new();
        }
        let before = n
            .checked_sub(1)
            .and_then(|ply| game.review_at_ply(ply))
            .map_or_else(|| self.exercise.fen.clone(), |v| v.position.to_fen());
        self.errors.push(RunError {
            cause: ErrorCause::OffObjective {
                uci: mv.to_string(),
            },
            fen: before,
        });
        vec![Followup::Undo]
    }

    /// Aplica os `Followup`s à Partida até a Resolução não pedir mais nada (testes e
    /// controladores sem fila própria). `at_ms` é o instante dos Comandos.
    pub fn drive(&mut self, game: &mut Game, at_ms: u64) {
        for _ in 0..MAX_DRIVE_ROUNDS {
            let followups = self.observe(game);
            if followups.is_empty() {
                return;
            }
            for followup in followups {
                for command in followup.commands() {
                    // Uma recusa deixa a Partida como estava; o próximo `observe` vê o estado real.
                    let _ = game.handle(command, Timestamp::mono(at_ms));
                }
            }
        }
    }

    /// Encerra a Resolução (acerto, ou desistência/saída) e devolve o registro imutável.
    pub fn finish(self, game: &Game, at_ms: u64) -> RunSummary {
        let snapshot = game.snapshot();
        RunSummary {
            exercise_id: self.exercise.id,
            exercise_version: self.exercise.version,
            kind: self.exercise.kind,
            policy: PolicyKind::Assisted,
            result: if self.solved {
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
        }
    }
}
