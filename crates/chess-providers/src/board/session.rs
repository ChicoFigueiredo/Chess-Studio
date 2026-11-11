//! A sessão de uma Partida online pela Board API (ADR-0007): o fluxo da Partida
//! (`/api/board/game/stream/{id}`) traduzido em retratos para o controlador da Partida, o
//! envio único do Lance, a reconexão e a Trava de fair play.
//!
//! - Cada `gameFull`/`gameState` vira um [`chess_game::ProviderSnapshot`]; o controlador
//!   aplica só o que falta. `chatLine` é ignorado; `opponentGone` abre o prazo de reclamar.
//! - O Lance do jogador é intenção enviada ([`chess_game::Game::send_move`]) e sai **uma
//!   vez**. Sem resposta (queda, prazo), a sessão nunca reenvia: reabre o fluxo, cujo
//!   primeiro `gameFull` diz se o Lance entrou; se não entrou, a intenção é descartada e o
//!   jogador decide de novo.
//! - Fluxo caído com a Partida em andamento: reabre com backoff e jitter pelo relógio
//!   injetado, registra o intervalo sem dados e reconcilia pelo `gameFull` (inclusive o fim
//!   que aconteceu enquanto estava sem fluxo).
//! - Contra pessoa, a sessão segura a Trava de fair play da camada de rede do começo até o
//!   Provedor confirmar o fim. Descartar a sessão solta a trava dela.

use super::engine::{EngineSide, Step};
use super::json::{self, Ending, GameFull, OpponentGoneJson, StateJson};
use super::link::{self, Link, LinkItem, LinkStatus};
use super::{BoardError, GameInfo, LichessBoard};
use crate::ProviderError;
use chess_core::{Color, Move};
use chess_game::{
    Command, Desync, Event, Game, GameResult, Opponent, Outcome, ProviderEnd, Rejection,
    Timestamp as GameTime,
};
use chess_net::{FairPlayHold, NetError, Pending, Timestamp};
use std::fmt;

/// Pedido ao Provedor numa Partida online: o efeito só entra quando o Provedor confirma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardAction {
    Resign,
    Abort,
    /// Propõe empate (ou aceita a proposta do adversário: no Lichess é o mesmo pedido).
    OfferDraw,
    AcceptDraw,
    DeclineDraw,
    /// Vitória por abandono do adversário, depois do prazo do Lichess.
    ClaimVictory,
    /// Recusa da volta de Lance pedida pelo adversário (automática: nunca há desfazer).
    DeclineTakeback,
}

impl BoardAction {
    fn path(self, api: &str, game: &str) -> String {
        let tail = match self {
            Self::Resign => "resign",
            Self::Abort => "abort",
            Self::OfferDraw | Self::AcceptDraw => "draw/yes",
            Self::DeclineDraw => "draw/no",
            Self::ClaimVictory => "claim-victory",
            Self::DeclineTakeback => "takeback/no",
        };
        format!("/api/{api}/game/{game}/{tail}")
    }
}

/// Intervalo sem dados do Provedor (fluxo caído), pelo relógio injetado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub from: Timestamp,
    pub to: Timestamp,
}

/// O adversário saiu da Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpponentGone {
    /// A partir de quando o Lichess deixa reclamar a vitória.
    pub claim_from: Option<Timestamp>,
}

/// O que a interface deve contar ao jogador.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionNotice {
    /// O Provedor confirmou o Lance enviado.
    MoveConfirmed { mv: Move },
    /// O Provedor registrou outro Lance no lugar do enviado.
    MoveReplaced { sent: Move },
    /// O Provedor recusou o Lance enviado; a intenção foi descartada.
    MoveRefused { mv: Move, error: ProviderError },
    /// O envio ficou sem resposta e o estado do Provedor não traz o Lance: a intenção foi
    /// descartada (nada foi reenviado).
    MoveLost { mv: Move },
    /// Um pedido (abandonar, empate…) falhou; a Partida não mudou.
    RequestFailed {
        action: BoardAction,
        error: ProviderError,
    },
    /// O retrato do Provedor não confere com o log: nada foi reescrito.
    Desynced(Desync),
    /// O fluxo caiu; reabre em `at`.
    Reconnecting { attempt: u32, at: Timestamp },
    /// O fluxo voltou depois de `gap`.
    Reconnected { gap: Gap },
    /// A sessão parou (variante recusada, Conexão revogada, Partida inexistente).
    Closed(BoardError),
    /// O controlador recusou o retrato por outro motivo (não deveria acontecer).
    Rejected(Rejection),
    /// Partida da Engine online: a Engine propôs um Lance ilegal; o núcleo de regras o
    /// recusou e **nada foi enviado**.
    EngineMoveRejected { uci: String },
    /// Partida da Engine online: a Engine falhou (processo morreu, não achou Lance ou propôs
    /// Lances ilegais em sequência). Com a política padrão a sessão abandona a Partida.
    EngineFailed { detail: String },
}

/// Resultado de uma consulta da sessão.
#[derive(Debug, Default)]
pub struct SessionPoll {
    /// A Partida acabou de nascer (primeiro `gameFull`): grave o setup antes dos Eventos.
    pub created: bool,
    /// Eventos novos do log (só o que o Provedor confirmou), na ordem: grave-os.
    pub events: Vec<Event>,
    pub notices: Vec<SessionNotice>,
}

/// Sessão de uma Partida online, consultada por quadro ([`OnlineGameSession::poll`] nunca
/// bloqueia). Descartar fecha os pedidos e o fluxo e solta a Trava de fair play dela.
pub struct OnlineGameSession {
    board: LichessBoard,
    game_id: String,
    local_color: Color,
    expects_human: bool,
    /// `board` (pessoa) ou `bot` (Engine online): o prefixo dos caminhos da Partida.
    api: &'static str,
    engine: Option<EngineSide>,
    game: Option<Game>,
    link: Link,
    /// Instante (mono) e relógio no começo desta sessão: o tempo do domínio continua o do log.
    base_mono: u64,
    opened_at: Timestamp,
    sends: Vec<(Move, Pending)>,
    /// Envio sem resposta: o próximo `gameFull` decide.
    unconfirmed: Option<Move>,
    requests: Vec<(BoardAction, Pending)>,
    gaps: Vec<Gap>,
    opponent_gone: Option<OpponentGone>,
    takeback_declined: Option<(Color, usize)>,
    hold: Option<FairPlayHold>,
    closed: bool,
}

impl fmt::Debug for OnlineGameSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnlineGameSession")
            .field("game_id", &self.game_id)
            .field("link", &self.link.status())
            .field("started", &self.game.is_some())
            .field("sends", &self.sends.len())
            .finish_non_exhaustive()
    }
}

/// O Provedor certamente não aplicou o pedido (recusa, limite, sem autorização).
fn refused(error: &NetError) -> bool {
    !matches!(
        error,
        NetError::Timeout { .. }
            | NetError::Unreachable { .. }
            | NetError::Cancelled
            | NetError::BodyTooLarge { .. }
    ) && !matches!(error, NetError::Http { status, .. } if *status >= 500)
}

impl OnlineGameSession {
    fn new(
        board: LichessBoard,
        game_id: String,
        local_color: Color,
        expects_human: bool,
        engine: Option<EngineSide>,
    ) -> Self {
        let api = if engine.is_some() { "bot" } else { "board" };
        let link = board.link(board.get(&format!("/api/{api}/game/stream/{game_id}")));
        let opened_at = board.clock.now();
        let mut session = Self {
            board,
            game_id,
            local_color,
            expects_human,
            api,
            engine,
            game: None,
            link,
            base_mono: 0,
            opened_at,
            sends: Vec::new(),
            unconfirmed: None,
            requests: Vec::new(),
            gaps: Vec::new(),
            opponent_gone: None,
            takeback_declined: None,
            hold: None,
            closed: false,
        };
        session.sync_hold();
        session
    }

    pub(crate) fn open(board: LichessBoard, info: &GameInfo) -> Self {
        Self::new(
            board,
            info.game_id.clone(),
            info.color,
            info.opponent.ai_level.is_none(),
            None,
        )
    }

    pub(crate) fn open_engine(board: LichessBoard, info: &GameInfo, engine: EngineSide) -> Self {
        Self::new(board, info.game_id.clone(), info.color, false, Some(engine))
    }

    pub(crate) fn resume(board: LichessBoard, game: Game) -> Result<Self, BoardError> {
        Self::resume_with(board, game, None)
    }

    pub(crate) fn resume_with(
        board: LichessBoard,
        game: Game,
        engine: Option<EngineSide>,
    ) -> Result<Self, BoardError> {
        let key = board.profile.game_key(engine.is_some());
        let info = game
            .setup()
            .online_game()
            .filter(|o| o.provider == key)
            .cloned()
            .ok_or(BoardError::WrongGame)?;
        let human = info.opponent == Opponent::Human && engine.is_none();
        let mut session = Self::new(board, info.remote_id, info.local_color, human, engine);
        session.base_mono = game.events().last().map_or(0, |e| e.at.mono_ms);
        session.game = Some(game);
        session.sync_hold();
        Ok(session)
    }

    pub fn game_id(&self) -> &str {
        &self.game_id
    }

    /// A Partida, depois do primeiro `gameFull` (ou a reaberta do log).
    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }

    pub fn link(&self) -> LinkStatus {
        self.link.status()
    }

    /// Intervalos sem dados do Provedor nesta sessão.
    pub fn gaps(&self) -> &[Gap] {
        &self.gaps
    }

    pub fn opponent_gone(&self) -> Option<OpponentGone> {
        self.opponent_gone
    }

    /// Esta sessão segura a Trava de fair play.
    pub fn holds_fair_play(&self) -> bool {
        self.hold.is_some()
    }

    /// Instante do domínio: continua o do log (nunca volta) e leva o relógio de parede.
    fn at(&self) -> GameTime {
        let now = self.board.clock.now();
        let elapsed = u64::try_from(now.since(self.opened_at).as_millis()).unwrap_or(u64::MAX);
        let last = self
            .game
            .as_ref()
            .and_then(|g| g.events().last())
            .map_or(0, |e| e.at.mono_ms);
        GameTime::with_wall(
            self.base_mono.saturating_add(elapsed).max(last),
            now.as_millis(),
        )
    }

    fn over(&self) -> bool {
        self.game.as_ref().is_some_and(|g| g.snapshot().is_over())
    }

    /// Contra pessoa e em andamento: segura a trava; fim confirmado (ou sessão sem Partida
    /// que parou): solta.
    fn sync_hold(&mut self) {
        let required = match &self.game {
            Some(game) => game.online_human_in_progress(),
            None => self.expects_human && !self.closed,
        };
        if !required {
            self.hold = None;
        } else if self.hold.is_none() {
            self.hold = Some(self.board.net.fair_play_guard().hold(format!(
                "Partida online contra pessoa no {} ({})",
                self.board.profile.name,
                self.game_id
            )));
        }
    }

    fn close(&mut self, error: BoardError, out: &mut SessionPoll) {
        if let Some(engine) = &mut self.engine {
            engine.stop();
        }
        self.link.close();
        self.closed = true;
        out.notices.push(SessionNotice::Closed(error));
    }

    /// Nunca bloqueia: aplica o que chegou do Provedor e devolve os Eventos novos.
    pub fn poll(&mut self) -> SessionPoll {
        let mut out = SessionPoll::default();
        for item in self.link.poll() {
            if self.closed {
                break;
            }
            match item {
                LinkItem::Restored { from, to } => {
                    let gap = Gap { from, to };
                    self.gaps.push(gap);
                    out.notices.push(SessionNotice::Reconnected { gap });
                }
                LinkItem::Line(line) => self.on_line(&line, &mut out),
                LinkItem::Ended if self.over() => self.link.close(),
                LinkItem::Ended => self.retry(None, &mut out),
                LinkItem::Failed(error) if link::is_fatal(&error) => {
                    self.close(BoardError::Provider(error.into()), &mut out);
                }
                LinkItem::Failed(error) => self.retry(link::not_before(&error), &mut out),
            }
        }
        self.poll_sends(&mut out);
        self.poll_requests(&mut out);
        self.drive_engine(&mut out);
        self.sync_hold();
        out
    }

    /// Partida da Engine online: a Engine local propõe, o núcleo de regras valida e o Lance
    /// sai uma vez.
    fn drive_engine(&mut self, out: &mut SessionPoll) {
        if self.closed || self.engine.is_none() {
            return;
        }
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let at = self.at();
        let idle = self.sends.is_empty() && self.unconfirmed.is_none();
        let local = self.local_color;
        let steps = match self.engine.as_mut() {
            Some(engine) => engine.step(game, local, at, idle),
            None => return,
        };
        for step in steps {
            match step {
                Step::Play(mv) => {
                    if let Err(error) = self.submit(mv) {
                        out.notices.push(SessionNotice::EngineFailed {
                            detail: error.to_string(),
                        });
                    }
                }
                Step::Rejected { uci } => out.notices.push(SessionNotice::EngineMoveRejected { uci }),
                Step::Failed { detail } => {
                    let resign = self
                        .engine
                        .as_ref()
                        .is_some_and(|e| e.policy().resign_if_engine_fails);
                    out.notices.push(SessionNotice::EngineFailed { detail });
                    if resign && !self.over() {
                        self.send_request(BoardAction::Resign);
                    }
                }
                Step::AnswerDraw { accept } => self.send_request(if accept {
                    BoardAction::AcceptDraw
                } else {
                    BoardAction::DeclineDraw
                }),
            }
        }
    }

    /// Esta é uma Partida da Engine online (Bot API): nunca conta como Partida de pessoa.
    pub fn is_engine_game(&self) -> bool {
        self.engine.is_some()
    }

    /// A Engine está pensando num Lance.
    pub fn engine_thinking(&self) -> bool {
        self.engine.as_ref().is_some_and(EngineSide::thinking)
    }

    /// A Engine falhou nesta Partida (ver [`SessionNotice::EngineFailed`]).
    pub fn engine_failed(&self) -> bool {
        self.engine.as_ref().is_some_and(EngineSide::has_failed)
    }

    fn retry(&mut self, not_before: Option<Timestamp>, out: &mut SessionPoll) {
        self.link.retry(not_before);
        if let LinkStatus::Retrying { attempt, at } = self.link.status() {
            out.notices
                .push(SessionNotice::Reconnecting { attempt, at });
        }
    }

    fn on_line(&mut self, line: &str, out: &mut SessionPoll) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        match value.get("type").and_then(|t| t.as_str()) {
            Some("gameFull") => {
                if let Ok(full) = serde_json::from_value::<GameFull>(value) {
                    self.on_full(&full, out);
                }
            }
            Some("gameState") => {
                if let Ok(state) = serde_json::from_value::<StateJson>(value) {
                    self.on_state(&state, out);
                }
            }
            Some("opponentGone") => {
                if let Ok(gone) = serde_json::from_value::<OpponentGoneJson>(value) {
                    self.opponent_gone = gone.gone.then(|| OpponentGone {
                        claim_from: gone.claim_win_in_seconds.map(|s| {
                            self.board
                                .clock
                                .now()
                                .plus(std::time::Duration::from_secs(s))
                        }),
                    });
                }
            }
            // `chatLine` e tipos novos: ignorados.
            _ => {}
        }
    }

    fn on_full(&mut self, full: &GameFull, out: &mut SessionPoll) {
        if full.id != self.game_id {
            return self.close(BoardError::WrongGame, out);
        }
        if self.game.is_none() {
            match json::setup(
                &self.board.profile,
                self.engine.is_some(),
                full,
                self.local_color,
            ) {
                Ok(setup) => {
                    self.game = Some(Game::new(setup));
                    self.base_mono = 0;
                    self.opened_at = self.board.clock.now();
                    out.created = true;
                }
                Err(chess_game::OnlineSetupError::UnsupportedVariant(v)) => {
                    return self.close(BoardError::UnsupportedVariant(v), out);
                }
                Err(e) => {
                    return self.close(
                        BoardError::Provider(ProviderError::malformed(
                            self.board.profile.id,
                            e.to_string(),
                        )),
                        out,
                    );
                }
            }
        }
        self.on_state(&full.state, out);
        // O envio sem resposta: este é o estado completo depois dele.
        if let Some(mv) = self.unconfirmed.take()
            && let Some(game) = &mut self.game
            && game.sent_move().is_some_and(|s| s.mv == mv)
        {
            game.drop_sent_move();
            out.notices.push(SessionNotice::MoveLost { mv });
        }
    }

    fn on_state(&mut self, state: &StateJson, out: &mut SessionPoll) {
        let at = self.at();
        let Some(game) = &mut self.game else {
            return;
        };
        let has_clock = game.setup().time_control().is_some();
        let sent = game.sent_move().cloned();
        // Cada Comando aceito já mudou o log: os Eventos dele saem na hora, para a interface
        // gravar exatamente o que o controlador registrou.
        let apply = |game: &mut Game, end: Option<ProviderEnd>, out: &mut SessionPoll| {
            let snapshot = json::snapshot(state, has_clock, end);
            match game.handle(Command::Provider(snapshot), at) {
                Ok(events) => {
                    out.events.extend(events);
                    true
                }
                Err(Rejection::Desynced(desync)) => {
                    out.notices.push(SessionNotice::Desynced(desync));
                    false
                }
                Err(other) => {
                    out.notices.push(SessionNotice::Rejected(other));
                    false
                }
            }
        };
        match json::ending(state) {
            Ending::Playing => {
                apply(game, None, out);
            }
            Ending::End(end) => {
                apply(game, Some(end), out);
            }
            // O motivo do empate vem da Posição: primeiro os Lances, depois o fim.
            Ending::Draw => {
                if apply(game, None, out) {
                    let history: Vec<Move> = game.snapshot().history.iter().map(|h| h.mv).collect();
                    let reason = json::draw_reason(game.setup().start(), &history);
                    let end = ProviderEnd::Decided(Outcome {
                        result: GameResult::Draw,
                        reason,
                    });
                    apply(game, Some(end), out);
                }
            }
        }
        if let Some(sent) = sent
            && game.sent_move().is_none()
        {
            let history = game.snapshot().history;
            let notice = match history.get(sent.ply - 1) {
                Some(h) if h.mv == sent.mv => SessionNotice::MoveConfirmed { mv: sent.mv },
                Some(_) => SessionNotice::MoveReplaced { sent: sent.mv },
                None => SessionNotice::MoveLost { mv: sent.mv },
            };
            if matches!(notice, SessionNotice::MoveConfirmed { .. })
                && self.unconfirmed == Some(sent.mv)
            {
                self.unconfirmed = None;
            }
            out.notices.push(notice);
        }
        self.decline_takeback(state);
    }

    /// Nunca há desfazer na Partida online: a volta de Lance do adversário é recusada, uma
    /// vez por pedido.
    fn decline_takeback(&mut self, state: &StateJson) {
        let Some(by) = json::takeback_by(state) else {
            return;
        };
        let plies = self.game.as_ref().map_or(0, |g| g.snapshot().history.len());
        if by == self.local_color || self.takeback_declined == Some((by, plies)) || self.over() {
            return;
        }
        self.takeback_declined = Some((by, plies));
        self.send_request(BoardAction::DeclineTakeback);
    }

    fn send_request(&mut self, action: BoardAction) {
        let pending = self
            .board
            .net
            .fetch(self.board.post(&action.path(self.api, &self.game_id), None));
        self.requests.push((action, pending));
    }

    fn poll_sends(&mut self, out: &mut SessionPoll) {
        let mut still = Vec::new();
        for (mv, mut pending) in std::mem::take(&mut self.sends) {
            match pending.try_take() {
                None => still.push((mv, pending)),
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    let current = self
                        .game
                        .as_ref()
                        .and_then(|g| g.sent_move())
                        .is_some_and(|s| s.mv == mv);
                    if !current {
                        continue;
                    }
                    if refused(&error) {
                        if let Some(game) = &mut self.game {
                            game.drop_sent_move();
                        }
                        out.notices.push(SessionNotice::MoveRefused {
                            mv,
                            error: error.into(),
                        });
                    } else {
                        // Sem saber se entrou: consulta o estado completo, nunca reenvia.
                        self.unconfirmed = Some(mv);
                        self.link.reconnect_now();
                    }
                }
            }
        }
        self.sends.extend(still);
    }

    fn poll_requests(&mut self, out: &mut SessionPoll) {
        let mut still = Vec::new();
        for (action, mut pending) in std::mem::take(&mut self.requests) {
            match pending.try_take() {
                None => still.push((action, pending)),
                Some(Ok(_)) => {}
                Some(Err(error)) => out.notices.push(SessionNotice::RequestFailed {
                    action,
                    error: error.into(),
                }),
            }
        }
        self.requests.extend(still);
    }

    fn playing(&self) -> Result<&Game, BoardError> {
        let game = self
            .game
            .as_ref()
            .ok_or(BoardError::NotAllowed("a Partida ainda não começou"))?;
        if game.snapshot().is_over() {
            return Err(BoardError::NotAllowed("a Partida já terminou"));
        }
        Ok(game)
    }

    /// O jogador soltou a peça: intenção enviada, validada pelo núcleo de regras e enviada
    /// ao Lichess **uma vez**. Só vira Lance quando o Provedor confirma.
    pub fn play(&mut self, mv: Move) -> Result<(), BoardError> {
        if self.engine.is_some() {
            return Err(BoardError::NotAllowed(
                "numa Partida da Engine online quem joga é a Engine",
            ));
        }
        self.submit(mv)
    }

    /// Intenção enviada + o único `POST` do Lance (de pessoa ou da Engine).
    fn submit(&mut self, mv: Move) -> Result<(), BoardError> {
        if self.closed {
            return Err(BoardError::NotAllowed("a sessão parou"));
        }
        let game = self
            .game
            .as_mut()
            .ok_or(BoardError::NotAllowed("a Partida ainda não começou"))?;
        game.send_move(mv).map_err(BoardError::Rejected)?;
        let path = format!("/api/{}/game/{}/move/{mv}", self.api, self.game_id);
        let pending = self.board.net.fetch(self.board.post(&path, None));
        self.sends.push((mv, pending));
        Ok(())
    }

    fn request(&mut self, action: BoardAction) -> Result<(), BoardError> {
        self.playing()?;
        if self.requests.iter().any(|(a, _)| *a == action) {
            return Err(BoardError::NotAllowed("o pedido já foi enviado"));
        }
        self.send_request(action);
        Ok(())
    }

    /// Pede o abandono. A Partida (e a Trava de fair play) só muda quando o Provedor
    /// confirmar.
    pub fn resign(&mut self) -> Result<(), BoardError> {
        self.request(BoardAction::Resign)
    }

    /// Pede para abortar (o Lichess só deixa no começo).
    pub fn abort(&mut self) -> Result<(), BoardError> {
        self.request(BoardAction::Abort)
    }

    /// Propõe empate (ou aceita a do adversário).
    pub fn offer_draw(&mut self) -> Result<(), BoardError> {
        self.request(BoardAction::OfferDraw)
    }

    fn opponent_offer(&self) -> Result<(), BoardError> {
        let offer = self.playing()?.snapshot().pending_draw_offer;
        if offer == Some(self.local_color.opposite()) {
            Ok(())
        } else {
            Err(BoardError::NotAllowed("o adversário não propôs empate"))
        }
    }

    pub fn accept_draw(&mut self) -> Result<(), BoardError> {
        self.opponent_offer()?;
        self.request(BoardAction::AcceptDraw)
    }

    pub fn decline_draw(&mut self) -> Result<(), BoardError> {
        self.opponent_offer()?;
        self.request(BoardAction::DeclineDraw)
    }

    /// Reclama a vitória por abandono do adversário, só depois do prazo do Lichess.
    pub fn claim_victory(&mut self) -> Result<(), BoardError> {
        if self.engine.is_some() {
            return Err(BoardError::NotAllowed(
                "a Engine online não reclama vitória por abandono",
            ));
        }
        let allowed = self
            .opponent_gone
            .and_then(|g| g.claim_from)
            .is_some_and(|from| self.board.clock.now() >= from);
        if !allowed {
            return Err(BoardError::NotAllowed(
                "o adversário não saiu, ou o prazo do Lichess para reclamar não chegou",
            ));
        }
        self.request(BoardAction::ClaimVictory)
    }
}
