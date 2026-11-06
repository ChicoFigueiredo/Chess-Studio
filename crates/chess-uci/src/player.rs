//! A Engine como adversária da Partida: liga o [`EngineController`] ao `chess-game`.
//!
//! [`EnginePlayer::drive`] é chamado a cada quadro pela interface (sem bloquear):
//! quando é a vez da Engine ele dispara a busca no worker com o relógio da Partida
//! (`wtime/btime/winc/binc`) ou `movetime` sem relógio, envia `position` desde o início
//! com todos os Lances (a Engine enxerga repetições) e devolve o `bestmove` como
//! [`Command::EngineMove`]. Quem valida é a Partida (CORE-08, UCI-03): Lance ilegal vira
//! [`FailureKind::IllegalMove`], recuperável, e a posição nunca muda. A Engine reclama
//! empate (`claimable_draw`) nas Políticas competitivas (DEC-38) e aciona o próprio
//! relógio (o `EngineMove` conclui o Lance).

use std::time::Duration;

use chess_core::CanonicalPosition;
use chess_game::{Command, EngineSeat, EventKind, Game, Snapshot, Timestamp};

use crate::controller::{
    ControllerEvent, EngineController, Failure, FailureKind, SearchRequest, Strength,
    StrengthApplied,
};
use crate::{Clock, EngineConfig, GoParams, PositionSpec};

/// Tempo de pensar quando a Partida não tem relógio.
pub const DEFAULT_MOVETIME: Duration = Duration::from_millis(1000);

/// Piso do tempo de pensar com relógio (a Engine ainda recebe `stop` no prazo).
const MIN_THINK: Duration = Duration::from_millis(200);

/// Estado visível à interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerStatus {
    /// Subindo o processo (handshake) ou aguardando a vez.
    Waiting,
    /// "Engine pensando…".
    Thinking,
    /// Falhou; a Partida está intacta. Ações: [`EnginePlayer::retry`] (mesma Engine) ou
    /// [`EnginePlayer::replace_engine`] (outra Engine).
    Failed(Failure),
}

#[derive(Debug)]
enum State {
    Idle,
    Thinking { id: u64, events_len: usize },
    Failed(Failure),
}

#[derive(Debug)]
pub struct EnginePlayer {
    seat: EngineSeat,
    config: EngineConfig,
    movetime: Duration,
    controller: EngineController,
    state: State,
    /// Tamanho do log de Eventos quando a situação foi avaliada pela última vez.
    evaluated_len: Option<usize>,
    engine_name: Option<String>,
    strength: Option<StrengthApplied>,
}

impl EnginePlayer {
    /// Sobe a Engine em segundo plano (retorna na hora).
    pub fn new(seat: EngineSeat, config: EngineConfig) -> Self {
        let controller = EngineController::start(config.clone(), Strength::Elo(seat.elo));
        Self {
            seat,
            config,
            movetime: DEFAULT_MOVETIME,
            controller,
            state: State::Idle,
            evaluated_len: None,
            engine_name: None,
            strength: None,
        }
    }

    /// Tempo de pensar quando a Partida não tem relógio.
    pub fn with_movetime(mut self, movetime: Duration) -> Self {
        self.movetime = movetime;
        self
    }

    pub fn seat(&self) -> EngineSeat {
        self.seat
    }

    pub fn status(&self) -> PlayerStatus {
        match &self.state {
            State::Idle => PlayerStatus::Waiting,
            State::Thinking { .. } => PlayerStatus::Thinking,
            State::Failed(f) => PlayerStatus::Failed(f.clone()),
        }
    }

    pub fn is_thinking(&self) -> bool {
        matches!(self.state, State::Thinking { .. })
    }

    /// Nome anunciado pela Engine no handshake (depois do `Ready`).
    pub fn engine_name(&self) -> Option<&str> {
        self.engine_name.as_deref()
    }

    /// Como a força foi configurada (depois do `Ready`).
    pub fn strength(&self) -> Option<StrengthApplied> {
        self.strength
    }

    /// "Tentar de novo": sobe a mesma Engine de novo (se morreu) e refaz a busca.
    pub fn retry(&mut self) {
        if self.controller.is_dead()
            || matches!(&self.state, State::Failed(f) if f.kind != FailureKind::IllegalMove)
        {
            self.respawn();
        } else {
            self.controller.cancel();
        }
        self.state = State::Idle;
        self.evaluated_len = None;
    }

    /// "Trocar Engine": outro executável; a Partida continua de onde está.
    pub fn replace_engine(&mut self, config: EngineConfig) {
        self.config = config;
        self.respawn();
        self.state = State::Idle;
        self.evaluated_len = None;
    }

    fn respawn(&mut self) {
        self.controller =
            EngineController::start(self.config.clone(), Strength::Elo(self.seat.elo));
        self.engine_name = None;
        self.strength = None;
    }

    /// Avança a Engine: lê respostas, aplica à Partida e dispara a próxima busca.
    /// Devolve `true` se a Partida mudou (Lance, reclamação ou Evento de recusa).
    pub fn drive(&mut self, game: &mut Game, at: Timestamp) -> bool {
        let mut changed = false;
        while let Some(event) = self.controller.poll() {
            changed |= self.on_event(event, game, at);
        }
        // Algo mudou na Partida (Lance humano, desfazer, pausa) durante a busca: a
        // resposta pendente vale para outra posição.
        if let State::Thinking { events_len, .. } = self.state
            && game.events().len() != events_len
        {
            self.controller.cancel();
            self.state = State::Idle;
            self.evaluated_len = None;
        }
        if matches!(self.state, State::Idle) && self.evaluated_len != Some(game.events().len()) {
            self.evaluated_len = Some(game.events().len());
            changed |= self.start_turn(game, at);
        }
        changed
    }

    fn on_event(&mut self, event: ControllerEvent, game: &mut Game, at: Timestamp) -> bool {
        match event {
            ControllerEvent::Ready { name, strength } => {
                self.engine_name = name;
                self.strength = Some(strength);
                false
            }
            ControllerEvent::Move { id, uci } => {
                if !matches!(self.state, State::Thinking { id: cur, .. } if cur == id) {
                    return false;
                }
                self.apply_move(game, uci, at)
            }
            ControllerEvent::NoMove { .. } => {
                self.state = State::Failed(Failure::new(
                    FailureKind::Protocol,
                    "a Engine não encontrou Lance (bestmove none) numa Partida em andamento",
                ));
                false
            }
            ControllerEvent::Failed { failure, .. } => {
                self.state = State::Failed(failure);
                false
            }
        }
    }

    fn apply_move(&mut self, game: &mut Game, uci: String, at: Timestamp) -> bool {
        match game.handle(Command::EngineMove { uci: uci.clone() }, at) {
            Ok(events) => {
                let rejected = events
                    .iter()
                    .any(|e| matches!(e.kind, EventKind::EngineMoveRejected { .. }));
                self.state = if rejected {
                    State::Failed(Failure::new(
                        FailureKind::IllegalMove,
                        format!("a Engine propôs o Lance ilegal {uci}; a Partida não mudou"),
                    ))
                } else {
                    State::Idle
                };
                !events.is_empty()
            }
            Err(rejection) => {
                self.state = State::Failed(Failure::new(
                    FailureKind::IllegalMove,
                    format!("a Partida recusou o Lance {uci} da Engine: {rejection}"),
                ));
                false
            }
        }
    }

    /// É a vez da Engine? Reclama empate se couber; senão inicia a busca.
    fn start_turn(&mut self, game: &mut Game, at: Timestamp) -> bool {
        let mut snapshot = game.snapshot();
        if !self.is_engine_turn(&snapshot) {
            return false;
        }
        let mut changed = false;
        if snapshot.claimable_draw.is_some()
            && game
                .handle(Command::ClaimDraw, at)
                .is_ok_and(|e| !e.is_empty())
        {
            changed = true;
            snapshot = game.snapshot();
            self.evaluated_len = Some(game.events().len());
            if snapshot.outcome.is_some() {
                return true;
            }
        }
        let request = self.build_request(game, &snapshot, at);
        let id = self.controller.search(request);
        self.state = State::Thinking {
            id,
            events_len: game.events().len(),
        };
        changed
    }

    fn is_engine_turn(&self, s: &Snapshot) -> bool {
        s.outcome.is_none()
            && !s.paused
            && s.side_to_move == self.seat.color
            && s.provisional.is_none()
            && s.made_move.is_none()
            && s.provisional_release.is_none()
    }

    fn build_request(&self, game: &Game, s: &Snapshot, at: Timestamp) -> SearchRequest {
        let position = position_spec(game, s);
        match (s.clock_at(at), &s.clock) {
            (Some(reading), Some(view)) => {
                let inc = Duration::from_millis(view.increment_ms);
                let clock = Clock {
                    wtime: Duration::from_millis(reading.white_ms),
                    btime: Duration::from_millis(reading.black_ms),
                    winc: inc,
                    binc: inc,
                };
                let own = Duration::from_millis(reading.remaining(self.seat.color));
                let limit = (own / 3 + inc).clamp(MIN_THINK, own.max(MIN_THINK));
                SearchRequest {
                    position,
                    go: GoParams::clock(clock),
                    limit,
                }
            }
            _ => SearchRequest {
                position,
                go: GoParams::movetime(self.movetime),
                limit: self.movetime,
            },
        }
    }
}

/// `position` desde o início da Partida com todos os Lances, para a Engine enxergar
/// repetições (parecer do supervisor no PR #64).
fn position_spec(game: &Game, s: &Snapshot) -> PositionSpec {
    let start = game.setup().start();
    let spec = if start.to_fen() == CanonicalPosition::start().to_fen() {
        PositionSpec::startpos()
    } else {
        PositionSpec::from_fen(start.to_fen())
    };
    spec.with_moves(s.history.iter().map(|h| h.mv))
}
