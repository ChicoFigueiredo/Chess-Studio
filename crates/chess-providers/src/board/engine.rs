//! O lado Engine de uma Partida pela Bot API (CX-13): uma Engine UCI local propõe o Lance da
//! vez; o núcleo de regras o valida; só então a sessão o envia ao Provedor, uma vez.
//!
//! A Engine é uma [`MoveEngine`] (o `EngineController` do `chess-uci` a implementa). A Partida
//! online não aceita `Command::EngineMove` (só o retrato do Provedor muda o log), por isso
//! aqui não se usa o `EnginePlayer`: o pedido de busca é montado com o mesmo cuidado
//! (`position` desde o início com todos os Lances, relógio do Provedor em `wtime/btime`) e a
//! resposta vira intenção enviada.

use chess_core::{Color, Move};
use chess_game::{Game, Timestamp as GameTime};
use chess_uci::{
    Clock as UciClock, ControllerEvent, EngineController, GoParams, PositionSpec, SearchRequest,
};
use std::time::Duration;

/// A Engine que propõe os Lances. Nunca bloqueia.
pub trait MoveEngine: Send {
    /// Pede a busca e devolve o `id` do pedido.
    fn search(&mut self, request: SearchRequest) -> u64;
    /// Cancela a busca em curso (a resposta, se vier, é descartada).
    fn cancel(&mut self);
    /// Próximo evento da Engine, se já chegou.
    fn poll(&mut self) -> Option<ControllerEvent>;
}

impl MoveEngine for EngineController {
    fn search(&mut self, request: SearchRequest) -> u64 {
        EngineController::search(self, request)
    }

    fn cancel(&mut self) {
        EngineController::cancel(self);
    }

    fn poll(&mut self) -> Option<ControllerEvent> {
        EngineController::poll(self)
    }
}

/// Como a Engine responde a uma proposta de empate do adversário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawResponse {
    /// Recusa (o padrão).
    #[default]
    Decline,
    /// Aceita sempre.
    Accept,
}

/// Política simples da Engine numa Partida: empate e o que fazer se ela falhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnginePolicy {
    /// Resposta à proposta de empate do adversário. A Engine nunca propõe empate.
    pub draw: DrawResponse,
    /// Se a Engine falhar (processo morreu, dois Lances ilegais seguidos), abandona a Partida
    /// em vez de deixar o relógio correr.
    pub resign_if_engine_fails: bool,
    /// Tempo de pensar quando a Partida não tem relógio (correspondência).
    pub movetime: Duration,
}

impl Default for EnginePolicy {
    fn default() -> Self {
        Self {
            draw: DrawResponse::Decline,
            resign_if_engine_fails: true,
            movetime: Duration::from_millis(1000),
        }
    }
}

/// Piso do tempo de pensar com relógio.
const MIN_THINK: Duration = Duration::from_millis(200);
/// Lances ilegais seguidos que a Engine pode propor antes de ser dada como falha.
const MAX_ILLEGAL_STREAK: u8 = 2;

/// O que a sessão deve fazer depois de um passo da Engine.
#[derive(Debug)]
pub(crate) enum Step {
    /// Lance legal proposto: vira intenção enviada e sai uma vez.
    Play(Move),
    /// Lance ilegal proposto: nada foi enviado.
    Rejected { uci: String },
    /// A Engine não serve mais (ou falhou demais).
    Failed { detail: String },
    /// Responder à proposta de empate do adversário.
    AnswerDraw { accept: bool },
}

struct Search {
    id: u64,
    plies: usize,
}

pub(crate) struct EngineSide {
    engine: Box<dyn MoveEngine>,
    policy: EnginePolicy,
    searching: Option<Search>,
    illegal_streak: u8,
    failed: bool,
    answered_offer_at: Option<usize>,
}

impl std::fmt::Debug for EngineSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineSide")
            .field("searching", &self.searching.is_some())
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl EngineSide {
    pub(crate) fn new(engine: Box<dyn MoveEngine>, policy: EnginePolicy) -> Self {
        Self {
            engine,
            policy,
            searching: None,
            illegal_streak: 0,
            failed: false,
            answered_offer_at: None,
        }
    }

    pub(crate) fn policy(&self) -> &EnginePolicy {
        &self.policy
    }

    pub(crate) fn has_failed(&self) -> bool {
        self.failed
    }

    pub(crate) fn thinking(&self) -> bool {
        self.searching.is_some()
    }

    /// Para a busca em curso (a Partida acabou ou a sessão parou).
    pub(crate) fn stop(&mut self) {
        if self.searching.take().is_some() {
            self.engine.cancel();
        }
    }

    /// Avança a Engine: lê as respostas, valida pelo núcleo de regras e, se for a vez dela e
    /// nada estiver a caminho, pede a próxima busca. `idle`: nenhum envio sem resposta.
    pub(crate) fn step(
        &mut self,
        game: &Game,
        local: Color,
        at: GameTime,
        idle: bool,
    ) -> Vec<Step> {
        let snapshot = game.snapshot();
        let mut out = Vec::new();
        // Lance proposto neste passo: a sessão ainda vai registrá-lo como intenção enviada.
        let mut playing = false;
        while let Some(event) = self.engine.poll() {
            match event {
                ControllerEvent::Ready { .. } => {}
                ControllerEvent::Move { id, uci } => {
                    if self.searching.as_ref().is_none_or(|s| s.id != id) {
                        continue;
                    }
                    self.searching = None;
                    match uci.parse::<Move>() {
                        Ok(mv) if snapshot.position.is_legal(mv) => {
                            self.illegal_streak = 0;
                            out.push(Step::Play(mv));
                            playing = true;
                        }
                        _ => {
                            self.illegal_streak += 1;
                            out.push(Step::Rejected { uci });
                            if self.illegal_streak >= MAX_ILLEGAL_STREAK {
                                self.fail("a Engine propôs Lances ilegais em sequência", &mut out);
                            }
                        }
                    }
                }
                ControllerEvent::NoMove { id } => {
                    if self.searching.as_ref().is_some_and(|s| s.id == id) {
                        self.searching = None;
                        self.fail("a Engine não encontrou Lance", &mut out);
                    }
                }
                ControllerEvent::Failed { failure, .. } => {
                    self.searching = None;
                    self.fail(&failure.message, &mut out);
                }
            }
        }
        let plies = snapshot.history.len();
        let desynced = snapshot.online.as_ref().is_some_and(|o| o.desync.is_some());
        if snapshot.is_over() || desynced || self.failed {
            self.stop();
            return out;
        }
        // A Posição mudou durante a busca (retrato novo): a resposta valeria para outra.
        if self.searching.as_ref().is_some_and(|s| s.plies != plies) {
            self.stop();
        }
        if snapshot.pending_draw_offer == Some(local.opposite())
            && self.answered_offer_at != Some(plies)
        {
            self.answered_offer_at = Some(plies);
            out.push(Step::AnswerDraw {
                accept: self.policy.draw == DrawResponse::Accept,
            });
        }
        let my_turn = snapshot.side_to_move == local;
        if !my_turn {
            self.stop();
        } else if idle && !playing && game.sent_move().is_none() && self.searching.is_none() {
            let id = self.engine.search(self.request(game, &snapshot, local, at));
            self.searching = Some(Search { id, plies });
        }
        out
    }

    fn fail(&mut self, detail: &str, out: &mut Vec<Step>) {
        if !self.failed {
            self.failed = true;
            out.push(Step::Failed {
                detail: detail.to_owned(),
            });
        }
    }

    fn request(
        &self,
        game: &Game,
        snapshot: &chess_game::Snapshot,
        local: Color,
        at: GameTime,
    ) -> SearchRequest {
        let start = game.setup().start();
        let spec = if start.to_fen() == chess_core::CanonicalPosition::start().to_fen() {
            PositionSpec::startpos()
        } else {
            PositionSpec::from_fen(start.to_fen())
        }
        .with_moves(snapshot.history.iter().map(|h| h.mv));
        match (snapshot.clock_at(at), &snapshot.clock) {
            (Some(reading), Some(view)) => {
                let inc = Duration::from_millis(view.increment_ms);
                let clock = UciClock {
                    wtime: Duration::from_millis(reading.white_ms),
                    btime: Duration::from_millis(reading.black_ms),
                    winc: inc,
                    binc: inc,
                };
                let own = Duration::from_millis(reading.remaining(local));
                let limit = (own / 3 + inc).clamp(MIN_THINK, own.max(MIN_THINK));
                SearchRequest {
                    position: spec,
                    go: GoParams::clock(clock),
                    limit,
                }
            }
            _ => SearchRequest {
                position: spec,
                go: GoParams::movetime(self.policy.movetime),
                limit: self.policy.movetime,
            },
        }
    }
}
