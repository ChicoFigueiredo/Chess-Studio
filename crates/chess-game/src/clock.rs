//! Relógio de xadrez derivado dos Eventos (ADR-0002): base + incremento por Lance
//! concluído, quem corre, Pausa e bandeira. Nada aqui lê o relógio do sistema; o
//! tempo vem dos `Timestamp`s injetados nos Comandos e gravados nos Eventos.
//!
//! Modelo: o relógio das brancas começa a correr em `mono_ms = 0` (início da
//! Partida). Cada Evento "liquida" o tempo decorrido contra quem corre. O tempo
//! restante em um instante qualquer é consultável sem Evento ([`ClockView::at`]),
//! e a bandeira é detectada quando um Comando chega depois do tempo esgotado
//! ([`flag_events`]).

use crate::event::EventKind;
use crate::outcome::{EndReason, GameResult, Outcome};
use crate::time::{TimeControl, Timestamp};
use chess_core::{CanonicalPosition, Color};

/// Penalidade de 2 minutos (arts. 7.5.5 e 9.5.3), em ms.
pub const TWO_MINUTES_MS: u64 = 120_000;

fn idx(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

/// Estado interno do relógio, atualizado só por `apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClockState {
    pub remaining: [u64; 2],
    pub increment_ms: u64,
    pub running: Option<Color>,
    /// Instante (mono) da última liquidação.
    pub since_ms: u64,
}

impl ClockState {
    pub fn new(tc: TimeControl, first: Color) -> Self {
        Self {
            remaining: [tc.base_ms; 2],
            increment_ms: tc.increment_ms,
            running: Some(first),
            since_ms: 0,
        }
    }

    pub fn remaining_of(&self, color: Color) -> u64 {
        self.remaining[idx(color)]
    }

    /// Cobra o tempo decorrido até `at` de quem corre (sem pausa).
    pub fn settle(&mut self, at_ms: u64, paused: bool) {
        if !paused && let Some(r) = self.running {
            let elapsed = at_ms.saturating_sub(self.since_ms);
            let slot = &mut self.remaining[idx(r)];
            *slot = slot.saturating_sub(elapsed);
        }
        self.since_ms = self.since_ms.max(at_ms);
    }

    pub fn add(&mut self, color: Color, ms: u64) {
        let slot = &mut self.remaining[idx(color)];
        *slot = slot.saturating_add(ms);
    }

    /// Lance concluído por `mover`: incremento (opcional) e a vez passa.
    pub fn move_completed(&mut self, mover: Color, with_increment: bool) {
        if with_increment {
            self.add(mover, self.increment_ms);
        }
        self.running = Some(mover.opposite());
    }
}

/// Bandeira caída (ou que cairá) segundo o relógio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagFall {
    pub side: Color,
    /// Instante (mono) em que o tempo zerou.
    pub at_ms: u64,
    /// Derrota, ou empate se o adversário não tem como dar mate (art. 6.9).
    pub result: GameResult,
}

/// Relógio em um instante consultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockReading {
    pub white_ms: u64,
    pub black_ms: u64,
    pub running: Option<Color>,
    pub paused: bool,
    /// Presente quando o tempo de quem corre já se esgotou naquele instante.
    pub flag: Option<FlagFall>,
}

impl ClockReading {
    pub fn remaining(&self, color: Color) -> u64 {
        match color {
            Color::White => self.white_ms,
            Color::Black => self.black_ms,
        }
    }
}

/// Cópia imutável do relógio no último Evento, para a UI consultar por instante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockView {
    pub white_ms: u64,
    pub black_ms: u64,
    pub increment_ms: u64,
    pub running: Option<Color>,
    pub paused: bool,
    /// Instante (mono) a que `white_ms`/`black_ms` se referem.
    pub since_ms: u64,
    /// Resultado se as brancas / as pretas perderem por tempo (art. 6.9).
    result_if_white_flags: GameResult,
    result_if_black_flags: GameResult,
}

impl ClockView {
    pub(crate) fn from_state(
        clock: &ClockState,
        paused: bool,
        ended: bool,
        position: &CanonicalPosition,
    ) -> Self {
        Self {
            white_ms: clock.remaining[0],
            black_ms: clock.remaining[1],
            increment_ms: clock.increment_ms,
            running: if ended { None } else { clock.running },
            paused,
            since_ms: clock.since_ms,
            result_if_white_flags: flag_result(position, Color::White),
            result_if_black_flags: flag_result(position, Color::Black),
        }
    }

    /// Tempo restante de cada lado em `ts` (o relógio corre para quem está ativo,
    /// exceto em Pausa). Instantes anteriores ao último Evento valem como ele.
    pub fn at(&self, ts: Timestamp) -> ClockReading {
        let elapsed = if self.paused {
            0
        } else {
            ts.mono_ms.saturating_sub(self.since_ms)
        };
        let mut rem = [self.white_ms, self.black_ms];
        let mut flag = None;
        if let Some(r) = self.running {
            let i = idx(r);
            if elapsed >= rem[i] {
                flag = Some(FlagFall {
                    side: r,
                    at_ms: self.since_ms + rem[i],
                    result: match r {
                        Color::White => self.result_if_white_flags,
                        Color::Black => self.result_if_black_flags,
                    },
                });
                rem[i] = 0;
            } else {
                rem[i] -= elapsed;
            }
        }
        ClockReading {
            white_ms: rem[0],
            black_ms: rem[1],
            running: self.running,
            paused: self.paused,
            flag,
        }
    }
}

/// Art. 6.9: quem perde por tempo perde, salvo se o adversário não pode dar mate
/// por nenhuma sequência de Lances legais, quando é empate.
fn flag_result(position: &CanonicalPosition, flagged: Color) -> GameResult {
    let winner = flagged.opposite();
    if position.can_checkmate(winner) {
        match winner {
            Color::White => GameResult::WhiteWins,
            Color::Black => GameResult::BlackWins,
        }
    } else {
        GameResult::Draw
    }
}

/// Se um Comando em `at_ms` encontra a bandeira caída, devolve os Eventos que a
/// registram (o Comando em si é descartado).
pub(crate) fn flag_events(
    clock: &ClockState,
    paused: bool,
    position: &CanonicalPosition,
    at_ms: u64,
) -> Option<Vec<EventKind>> {
    if paused {
        return None;
    }
    let side = clock.running?;
    let rem = clock.remaining_of(side);
    let elapsed = at_ms.saturating_sub(clock.since_ms);
    if elapsed < rem {
        return None;
    }
    let result = flag_result(position, side);
    let reason = if result == GameResult::Draw {
        EndReason::TimeoutDraw
    } else {
        EndReason::Timeout
    };
    Some(vec![
        EventKind::FlagFell {
            side,
            flag_at_ms: clock.since_ms + rem,
        },
        EventKind::GameEnded {
            outcome: Outcome { result, reason },
        },
    ])
}
