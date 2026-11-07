//! Partida local do app: a [`Game`] (Assistido, dois jogadores no mesmo computador), o
//! intérprete de gestos e o relógio monotônico (ADR-0001/0002).
//!
//! Só esta estrutura escreve a Partida. O Bevy lê [`Session::snapshot`] para desenhar e
//! manda intenções (`gesture`, `command`). O instante dos Comandos vem do relógio do app
//! (`Instant`), nunca do domínio. Sem dependência do Bevy além do atributo `Resource`:
//! o fluxo inteiro (ponteiro -> Comando -> Partida) é testável sem janela.

use crate::new_game::{NewGameConfig, Opponent};
use bevy::prelude::{Resource, Vec3, warn};
use chess_core::{Color, FenError, Move, Piece, Square};
use chess_game::{
    ClockReading, Command, EngineSeat, EventKind, Game, GameSetup, IllegalReason, PgnOptions,
    PolicyKind, Rejection, ReviewView, Snapshot, Timestamp,
};
use chess_input::{DRAG_HEIGHT, GestureEvent, GestureInterpreter, GestureVisual, MoveMethod};
use chess_uci::{EngineConfig, EnginePlayer};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Quanto tempo a mensagem de Lance ilegal fica no painel.
const ILLEGAL_NOTICE: Duration = Duration::from_secs(6);
/// Quanto tempo um aviso da arbitragem fica no painel.
const NOTICE_SECS: Duration = Duration::from_secs(6);

/// Aviso do árbitro (a Política) ao usuário nas Políticas competitivas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Infração registrada; `ended` se ela encerrou a Partida.
    Infraction {
        count: u8,
        ended: bool,
    },
    ClaimAccepted,
    ClaimRejected,
    /// Retratar recusado: o Lance legal já foi feito (art. 4.7).
    CannotRetract,
    /// Lance recusado pela Obrigação de peça tocada (art. 4.3).
    TouchObligation(Vec<Square>),
    ClaimRightLost,
    ClaimedMoveIllegal,
}

/// Onde a peça deve ser encaixada após um gesto e de onde a animação parte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleHint {
    /// Casa canônica da peça depois do Comando (destino do Lance, ou origem se ilegal).
    pub square: Square,
    /// Ponto do mundo onde a peça estava na tela ao soltar.
    pub from: Vec3,
}

/// Último Lance ilegal recusado (para a explicação de erro, UX-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IllegalNotice {
    pub from: Square,
    pub to: Square,
    pub reason: IllegalReason,
}

/// Ocorrência ilegal do log, para a revisão (AC-08: fora do histórico legal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IllegalIncident {
    /// Número de meios-lances legais já jogados quando ocorreu.
    pub after_ply: usize,
    pub side: Color,
    pub notice: IllegalNotice,
}

#[derive(Resource, Debug)]
pub struct Session {
    game: Game,
    snapshot: Snapshot,
    revision: u64,
    started: Instant,
    interpreter: GestureInterpreter,
    always_queen: bool,
    hint: Option<SettleHint>,
    illegal_at: Option<Instant>,
    /// Engine adversária (B9); `None` em Partida entre dois jogadores locais.
    engine: Option<EnginePlayer>,
    illegal: Option<IllegalNotice>,
    /// A explicação "após tentativa" foi pedida para o aviso atual.
    illegal_revealed: bool,
    /// Instante monotônico acumulado de sessões anteriores (Partida retomada): o tempo
    /// dos Eventos nunca anda para trás.
    mono_offset: u64,
    /// Muda quando a Partida é trocada (Nova Partida, FEN); a persistência observa.
    epoch: u64,
    /// Revisão Lance a Lance em curso (somente leitura, DEC-39).
    review: Option<ReviewView>,
    /// Configuração da Nova Partida (derivada do `GameSetup` ao reabrir).
    config: NewGameConfig,
    notice: Option<(Notice, Instant)>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self::from_game(Game::new(GameSetup::standard(PolicyKind::Assisted)))
    }

    /// Sessão sobre uma Partida existente (nova, de FEN ou reaberta do log).
    pub fn from_game(game: Game) -> Self {
        // Partida contra a Engine reaberta (ou nova): a Engine volta a jogar o lado gravado
        // no `GameSetup`; o executável vem do ambiente (falha vira "Tentar de novo").
        let engine = game.setup().engine().map(|seat| {
            let path = crate::engine::resolve_engine().unwrap_or_else(|| "stockfish".into());
            EnginePlayer::new(seat, EngineConfig::new(path))
        });
        Self::with_engine(game, engine)
    }

    fn with_engine(game: Game, engine: Option<EnginePlayer>) -> Self {
        let mono_offset = game.events().last().map_or(0, |e| e.at.mono_ms);
        let config = NewGameConfig::from_setup(game.setup());
        Self {
            snapshot: game.snapshot(),
            game,
            revision: 0,
            started: Instant::now(),
            interpreter: GestureInterpreter::new(MoveMethod::default()),
            always_queen: false,
            hint: None,
            illegal_at: None,
            engine,
            illegal: None,
            illegal_revealed: false,
            mono_offset,
            epoch: 0,
            review: None,
            config,
            notice: None,
        }
    }

    /// Faz a cena redesenhar quando esta Sessão ocupa o lugar de `previous` no recurso
    /// (Treino troca a Partida por uma de Exercício e depois a devolve): a revisão passa a
    /// ser diferente da última desenhada.
    pub fn supersede(&mut self, previous: &Session) {
        self.revision = previous.revision.wrapping_add(1);
    }

    /// A Partida em si (somente leitura: exportação e persistência).
    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Revisão em curso, se houver. Durante a revisão a Partida não aceita gestos.
    pub fn review(&self) -> Option<&ReviewView> {
        self.review.as_ref()
    }

    /// Entra, navega ou sai (`None`) da revisão. `ply` além do histórico é ignorado.
    pub fn set_review_ply(&mut self, ply: Option<usize>) {
        let next = match ply {
            Some(p) => match self.game.review_at_ply(p) {
                Some(view) => Some(view),
                None => return,
            },
            None => None,
        };
        if next != self.review {
            self.review = next;
            self.interpreter
                .handle(GestureEvent::PanelInteraction, &self.snapshot);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Aviso do último Lance ilegal, enquanto recente.
    /// Nas Políticas competitivas não há aviso nem explicação durante a Partida (DEC-32).
    pub fn illegal_notice(&self) -> Option<IllegalNotice> {
        self.illegal
            .filter(|_| self.illegal_recently() && !self.is_competitive())
    }

    pub fn illegal_revealed(&self) -> bool {
        self.illegal_revealed
    }

    /// "Por quê?": mostra o motivo no modo "após tentativa".
    pub fn reveal_illegal(&mut self) {
        self.illegal_revealed = true;
        self.illegal_at = Some(Instant::now());
    }

    /// Ocorrências ilegais do log, em ordem (a Partida não as conta como Lances).
    /// Nas Políticas competitivas a explicação só vem depois do resultado (§12).
    pub fn illegal_incidents(&self) -> Vec<IllegalIncident> {
        if self.is_competitive() && self.snapshot.outcome.is_none() {
            return Vec::new();
        }
        let mut plies = 0usize;
        let mut out = Vec::new();
        for event in self.game.events() {
            match &event.kind {
                EventKind::MoveCompleted { .. } | EventKind::MoveRedone { .. } => plies += 1,
                EventKind::MoveUndone { .. } => plies = plies.saturating_sub(1),
                EventKind::IllegalMoveRejected {
                    side,
                    from,
                    to,
                    reason,
                } => out.push(IllegalIncident {
                    after_ply: plies,
                    side: *side,
                    notice: IllegalNotice {
                        from: *from,
                        to: *to,
                        reason: *reason,
                    },
                }),
                _ => {}
            }
        }
        out
    }

    /// PGN da Partida (só Lances legais; comentários opcionais, DEC-44). `notes` são as
    /// anotações de estudo `(meio-lance, texto)`, exportadas como comentários.
    pub fn export_pgn(&self, include_comments: bool, notes: &[(usize, String)]) -> String {
        let wall = self.now().wall_ms;
        self.game.to_pgn(&PgnOptions {
            event: "Partida casual".into(),
            site: "Chess Studio".into(),
            date: wall.map(chess_game::pgn_date_from_unix_ms),
            white: "Brancas".into(),
            black: "Pretas".into(),
            include_comments,
            study_notes: notes.to_vec(),
            ..PgnOptions::default()
        })
    }

    /// Carrega uma posição por FEN como nova Partida (mesma Política). FEN inválido
    /// devolve o erro e **não altera nada** (DB-02).
    pub fn load_fen(&mut self, fen: &str) -> Result<(), FenError> {
        let setup = GameSetup::from_fen(self.snapshot.policy, fen.trim())?;
        self.replace_game(Game::new(setup));
        Ok(())
    }

    /// Troca a Partida preservando as preferências de entrada.
    fn replace_game(&mut self, game: Game) {
        self.replace_with(Self::from_game(game));
    }

    fn replace_with(&mut self, fresh: Session) {
        let method = self.interpreter.method();
        let always_queen = self.always_queen;
        let epoch = self.epoch.wrapping_add(1);
        let revision = self.revision.wrapping_add(1);
        *self = fresh;
        self.interpreter.set_method(method);
        // Sem dama automática nas Políticas competitivas (DEC-40).
        self.always_queen = always_queen && !self.is_competitive();
        self.epoch = epoch;
        self.revision = revision;
    }

    /// Estado atual da Partida (única fonte para desenhar).
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Muda a cada alteração da Partida; a cena só se reconstrói quando ele muda.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn gesture_visual(&self) -> GestureVisual {
        self.interpreter.visual()
    }

    pub fn move_method(&self) -> MoveMethod {
        self.interpreter.method()
    }

    /// Muda o método de movimento (em memória; a persistência é do chess-store).
    pub fn set_move_method(&mut self, method: MoveMethod) {
        if self.interpreter.method() != method {
            self.interpreter.set_method(method);
        }
    }

    pub fn always_queen(&self) -> bool {
        self.always_queen
    }

    /// Opção "sempre dama" (só no Assistido, DEC-40). Ligada com o seletor aberto,
    /// promove a dama na hora.
    pub fn set_always_queen(&mut self, on: bool) {
        if self.is_competitive() {
            return;
        }
        self.always_queen = on;
        if on && self.snapshot.provisional.is_some() {
            self.command(Command::ChoosePromotion {
                piece: Piece::Queen,
            });
        }
    }

    pub fn illegal_recently(&self) -> bool {
        self.illegal_at
            .is_some_and(|at| at.elapsed() < ILLEGAL_NOTICE)
    }

    /// Dica de encaixe do último gesto; é consumida por quem anima a cena.
    pub fn take_hint(&mut self) -> Option<SettleHint> {
        self.hint.take()
    }

    fn now(&self) -> Timestamp {
        let mono_ms = u64::try_from(self.started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .saturating_add(self.mono_offset);
        let wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok());
        Timestamp { mono_ms, wall_ms }
    }

    /// Evento de gesto -> Comandos do intérprete -> Partida.
    pub fn gesture(&mut self, event: GestureEvent) {
        // Na vez da Engine o ponteiro não move nada (ela joga as próprias peças).
        if self.engine_turn() || self.review.is_some() {
            return;
        }
        let visual = self.interpreter.visual();
        let commands = self.interpreter.handle(event, &self.snapshot);
        for command in commands {
            let start = match command {
                Command::Release { from, .. } => Some(
                    visual
                        .drag_world
                        .map_or_else(|| lifted(chess_input::square_center(from)), lifted),
                ),
                _ => None,
            };
            let released = matches!(command, Command::Release { .. });
            self.apply(command, start);
            if released {
                self.interpreter
                    .handle(GestureEvent::Resolved, &self.snapshot);
            }
        }
        // Mudanças por fora do gesto (desfazer, promoção) invalidam a seleção.
        self.interpreter
            .handle(GestureEvent::Resync, &self.snapshot);
    }

    /// Comando vindo da interface (painel e seletor de promoção).
    pub fn command(&mut self, command: Command) {
        if self.review.is_some() {
            return;
        }
        self.apply(command, None);
        self.interpreter
            .handle(GestureEvent::Resync, &self.snapshot);
    }

    /// Desfazer. Contra a Engine desfaz também a resposta dela, senão ela jogaria de novo
    /// na hora e o Lance do usuário nunca poderia ser desfeito (DEC-39).
    pub fn undo(&mut self) {
        self.command(Command::Undo);
        if self.engine_turn() && self.snapshot.can_undo {
            self.command(Command::Undo);
        }
    }

    /// Descarta a Partida e começa outra, com a mesma preferência de movimento.
    #[cfg(test)]
    pub fn new_game(&mut self) {
        self.replace_game(Game::new(GameSetup::standard(PolicyKind::Assisted)));
    }

    /// Começa a Partida da tela de Nova Partida. Passa por `replace_with` (época nova), então
    /// a persistência a grava e a torna retomável, em qualquer Política. Com
    /// `Opponent::Engine` e `engine_config`, a Engine joga o lado oposto ao do usuário; sem
    /// `engine_config` a Partida é entre dois jogadores locais.
    pub fn start(&mut self, config: &NewGameConfig, engine_config: Option<EngineConfig>) {
        let engine_config = engine_config.filter(|_| config.opponent == Opponent::Engine);
        let mut config = config.clone();
        if engine_config.is_none() {
            config.opponent = Opponent::TwoLocal;
        }
        let game = Game::new(config.to_setup());
        let engine = engine_config
            .zip(game.setup().engine())
            .map(|(c, seat)| EnginePlayer::new(seat, c));
        let mut fresh = Self::with_engine(game, engine);
        fresh.config = config;
        self.replace_with(fresh);
    }

    pub fn config(&self) -> &NewGameConfig {
        &self.config
    }

    pub fn user_side(&self) -> Color {
        self.config.user_side
    }

    pub fn is_competitive(&self) -> bool {
        self.snapshot.policy != PolicyKind::Assisted
    }

    /// Aviso da arbitragem ainda visível.
    pub fn notice(&self) -> Option<&Notice> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_SECS)
            .map(|(n, _)| n)
    }

    /// Relógio agora (tempo restante, quem corre, bandeira); `None` sem controle de tempo.
    pub fn clock_reading(&self) -> Option<ClockReading> {
        self.snapshot.clock_at(self.now())
    }

    /// O usuário pode acionar o relógio de `side` agora? Só nas Políticas competitivas,
    /// com a Partida em curso e fora de Pausa; o relógio é o de quem tem a vez e, contra
    /// a Engine, só na vez do usuário (a Engine aciona o dela via `EngineMove`, DEC-33).
    pub fn can_press_clock(&self, side: Color) -> bool {
        self.is_competitive()
            && self.snapshot.outcome.is_none()
            && !self.snapshot.paused
            && self.review.is_none()
            && self.snapshot.side_to_move == side
            && (self.config.opponent == Opponent::TwoLocal || side == self.config.user_side)
    }

    /// Aciona o relógio do lado da vez (botão 3D, widget 2D, painel e Espaço). O gesto em
    /// curso é cancelado antes: a peça não se solta sozinha.
    pub fn press_clock(&mut self) {
        if !self.can_press_clock(self.snapshot.side_to_move) {
            return;
        }
        self.gesture(GestureEvent::Escape);
        self.command(Command::PressClock);
    }

    /// Registra a bandeira caída: um Comando que chega com o tempo esgotado só grava
    /// `FlagFell` e o resultado (o Comando em si é descartado).
    pub fn tick(&mut self) {
        if self.snapshot.outcome.is_none() && self.clock_reading().is_some_and(|r| r.flag.is_some())
        {
            self.command(Command::PressClock);
        }
    }

    pub fn toggle_pause(&mut self) {
        let command = if self.snapshot.paused {
            Command::Resume
        } else {
            Command::Pause
        };
        self.gesture(GestureEvent::Escape);
        self.command(command);
    }

    /// Reclamação de empate pelo lado da vez, com Lance anunciado opcional.
    pub fn claim_draw(&mut self, announced: Option<Move>) {
        self.command(match announced {
            Some(mv) => Command::ClaimDrawWithMove { mv },
            None => Command::ClaimDraw,
        });
    }

    /// Retratar a peça solta (ou a promoção pendente). Lance legal feito não se retrata.
    pub fn retract(&mut self) {
        self.command(Command::RetractMove);
    }

    /// Abandona: o lado da vez (dois jogadores no mesmo computador) ou, contra a Engine,
    /// sempre o lado do usuário.
    pub fn resign_side_to_move(&mut self) {
        let side: Color = match &self.engine {
            Some(engine) => engine.seat().color.opposite(),
            None => self.snapshot.side_to_move,
        };
        self.command(Command::Resign { side });
    }

    // --- Engine adversária (B9) ---------------------------------------------

    /// Começa uma Partida Assistido contra a Engine em `config` (ponto de extensão da
    /// tela de Nova Partida: ela só precisa chamar isto com o lado, o Elo e a Engine).
    pub fn start_engine_game(&mut self, seat: EngineSeat, config: EngineConfig) {
        let new = NewGameConfig {
            user_side: seat.color.opposite(),
            opponent: Opponent::Engine,
            engine_elo: seat.elo,
            ..NewGameConfig::default()
        };
        self.start(&new, Some(config));
    }

    /// A Engine joga neste momento (a interface ignora o ponteiro)?
    pub fn engine_turn(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| {
            self.snapshot.outcome.is_none() && self.snapshot.side_to_move == e.seat().color
        })
    }

    /// Há Engine adversária nesta Partida?
    pub fn engine_active(&self) -> bool {
        self.engine.is_some()
    }

    pub fn engine(&self) -> Option<&EnginePlayer> {
        self.engine.as_ref()
    }

    pub fn engine_retry(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.retry();
        }
    }

    pub fn engine_replace(&mut self, config: EngineConfig) {
        if let Some(engine) = &mut self.engine {
            engine.replace_engine(config);
        }
    }

    /// Chamado a cada quadro: não bloqueia (a Engine vive em thread própria).
    pub fn tick_engine(&mut self) {
        let at = self.now();
        let Some(engine) = &mut self.engine else {
            return;
        };
        if engine.drive(&mut self.game, at) {
            self.snapshot = self.game.snapshot();
            self.revision = self.revision.wrapping_add(1);
            self.interpreter
                .handle(GestureEvent::Resync, &self.snapshot);
        }
    }

    fn apply(&mut self, command: Command, start: Option<Vec3>) {
        match self.game.handle(command.clone(), self.now()) {
            Ok(events) => {
                if events.is_empty() {
                    return;
                }
                if let (Command::Release { from, to }, Some(start)) = (&command, start) {
                    self.note_release(*from, *to, start, &events);
                }
                self.note_arbitration(&events);
                self.snapshot = self.game.snapshot();
                self.revision = self.revision.wrapping_add(1);
            }
            Err(rejection) => self.note_rejection(&command, start, &rejection),
        }
    }

    /// Comando recusado pela Partida: a peça arrastada volta à origem e o usuário é avisado
    /// quando a arbitragem avisaria (obrigação de peça tocada, retratação, reclamação).
    fn note_rejection(&mut self, command: &Command, start: Option<Vec3>, rejection: &Rejection) {
        let notice = match (rejection, command) {
            (Rejection::TouchedPieceObligation { must_move_from }, _) => {
                Some(Notice::TouchObligation(must_move_from.clone()))
            }
            (Rejection::MoveAlreadyMade, Command::RetractMove) => Some(Notice::CannotRetract),
            (Rejection::DrawClaimRightLost, _) => Some(Notice::ClaimRightLost),
            (Rejection::ClaimedMoveIllegal, _) => Some(Notice::ClaimedMoveIllegal),
            _ => None,
        };
        match notice {
            Some(notice) => self.notice = Some((notice, Instant::now())),
            None => warn!("Comando {:?} recusado: {rejection}", command.kind()),
        }
        if let (Command::Release { from, .. }, Some(start)) = (command, start) {
            self.hint = Some(SettleHint {
                square: *from,
                from: start,
            });
            // A cena só se reconstrói quando a revisão muda: a peça precisa voltar.
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Avisos da arbitragem competitiva, a partir dos Eventos.
    fn note_arbitration(&mut self, events: &[chess_game::Event]) {
        let ended = events
            .iter()
            .any(|e| matches!(e.kind, EventKind::GameEnded { .. }));
        for event in events {
            let notice = match &event.kind {
                EventKind::InfractionRecorded { count, .. } => Some(Notice::Infraction {
                    count: *count,
                    ended,
                }),
                EventKind::DrawClaimAccepted { .. } => Some(Notice::ClaimAccepted),
                EventKind::DrawClaimRejected { .. } => Some(Notice::ClaimRejected),
                _ => None,
            };
            if let Some(notice) = notice {
                self.notice = Some((notice, Instant::now()));
            }
        }
    }

    fn note_release(
        &mut self,
        from: Square,
        to: Square,
        start: Vec3,
        events: &[chess_game::Event],
    ) {
        let has = |f: fn(&EventKind) -> bool| events.iter().any(|e| f(&e.kind));
        if has(|k| matches!(k, EventKind::IllegalMoveRejected { .. })) {
            // Lance ilegal no Assistido: a peça volta para a origem (DEC-34).
            self.hint = Some(SettleHint {
                square: from,
                from: start,
            });
            self.illegal_at = Some(Instant::now());
            self.illegal_revealed = false;
            self.illegal = events.iter().find_map(|e| match &e.kind {
                EventKind::IllegalMoveRejected {
                    from, to, reason, ..
                } => Some(IllegalNotice {
                    from: *from,
                    to: *to,
                    reason: *reason,
                }),
                _ => None,
            });
        } else if has(|k| {
            matches!(
                k,
                EventKind::IllegalMoveReleased { .. } | EventKind::MoveMade { .. }
            )
        }) {
            // Simulação FIDE: a peça fica onde foi solta (DEC-32) ou no Lance feito (art. 4.7).
            self.hint = Some(SettleHint {
                square: to,
                from: start,
            });
        } else if has(|k| matches!(k, EventKind::PromotionPending { .. })) {
            self.hint = Some(SettleHint {
                square: to,
                from: start,
            });
            if self.always_queen && self.snapshot.policy == PolicyKind::Assisted {
                self.apply(
                    Command::ChoosePromotion {
                        piece: Piece::Queen,
                    },
                    None,
                );
            }
        } else if has(|k| matches!(k, EventKind::MoveCompleted { .. })) {
            self.hint = Some(SettleHint {
                square: to,
                from: start,
            });
        }
    }
}

fn lifted(p: Vec3) -> Vec3 {
    Vec3::new(p.x, DRAG_HEIGHT, p.z)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{EndReason, GameResult};
    use chess_input::{Modifiers, PointerButton};

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn down(at: &str) -> GestureEvent {
        GestureEvent::PointerDown {
            button: PointerButton::Primary,
            square: Some(sq(at)),
            modifiers: Modifiers::default(),
        }
    }

    fn mv(at: &str) -> GestureEvent {
        GestureEvent::PointerMove {
            square: Some(sq(at)),
            world: chess_input::square_center(sq(at)),
        }
    }

    fn up(at: &str) -> GestureEvent {
        GestureEvent::PointerUp {
            square: Some(sq(at)),
        }
    }

    fn feed(session: &mut Session, events: &[GestureEvent]) {
        for e in events {
            session.gesture(*e);
        }
    }

    fn tap(s: &mut Session, from: &str, to: &str) {
        feed(s, &[down(from), up(from), down(to), up(to)]);
    }

    fn drag(s: &mut Session, from: &str, to: &str) {
        feed(s, &[down(from), mv(from), mv(to), up(to)]);
    }

    /// Mate do pastor, alternando dois toques e arrasto.
    #[test]
    fn scholars_mate_is_playable_by_gestures() {
        let mut s = Session::new();
        tap(&mut s, "e2", "e4");
        drag(&mut s, "e7", "e5");
        tap(&mut s, "f1", "c4");
        drag(&mut s, "b8", "c6");
        tap(&mut s, "d1", "h5");
        drag(&mut s, "g8", "f6");
        assert!(s.snapshot().outcome.is_none());
        tap(&mut s, "h5", "f7");
        let snap = s.snapshot();
        let sans: Vec<&str> = snap.history.iter().map(|h| h.san.as_str()).collect();
        assert_eq!(sans, ["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"]);
        let outcome = snap.outcome.unwrap();
        assert_eq!(outcome.result, GameResult::WhiteWins);
        assert_eq!(outcome.reason, EndReason::Checkmate);
    }

    #[test]
    fn illegal_drop_returns_to_origin_and_leaves_the_game_unchanged() {
        let mut s = Session::new();
        let before = s.snapshot().clone();
        drag(&mut s, "e2", "e5");
        assert_eq!(s.snapshot().position, before.position);
        assert!(s.snapshot().history.is_empty());
        let hint = s.take_hint().unwrap();
        assert_eq!(hint.square, sq("e2"), "volta à origem");
        assert!(s.illegal_recently());
        // O destino ilegal não virou outro Lance (e2-e5 nunca vira e2-e4).
        assert_eq!(s.snapshot().side_to_move, Color::White);
    }

    #[test]
    fn legal_drop_hint_targets_the_canonical_destination() {
        let mut s = Session::new();
        let cursor = chess_input::square_center(sq("e4")) + Vec3::new(0.3, 0.0, 0.3);
        feed(
            &mut s,
            &[
                down("e2"),
                GestureEvent::PointerMove {
                    square: Some(sq("e4")),
                    world: cursor,
                },
                up("e4"),
            ],
        );
        let hint = s.take_hint().unwrap();
        assert_eq!(hint.square, sq("e4"));
        assert_eq!((hint.from.x, hint.from.z), (cursor.x, cursor.z));
        assert!(hint.from.y > 0.0, "parte da peça levantada");
    }

    #[test]
    fn dragging_does_not_touch_the_game_until_release() {
        let mut s = Session::new();
        let rev = s.revision();
        feed(&mut s, &[down("e2"), mv("e3"), mv("e4")]);
        assert_eq!(s.revision(), rev);
        assert!(s.snapshot().history.is_empty());
        assert!(s.gesture_visual().lifted);
        s.gesture(GestureEvent::FocusLost);
        s.gesture(up("e4"));
        assert_eq!(s.revision(), rev, "cancelado: nenhum Lance fantasma");
        assert!(!s.gesture_visual().lifted);
    }

    const PROMOTION_FEN: &str = "1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1";

    fn promotion_session() -> Session {
        let mut s = Session::new();
        s.game = Game::new(GameSetup::from_fen(PolicyKind::Assisted, PROMOTION_FEN).unwrap());
        s.snapshot = s.game.snapshot();
        s
    }

    #[test]
    fn promotion_with_capture_asks_for_the_piece_and_esc_cancels() {
        let mut s = promotion_session();
        tap(&mut s, "a7", "b8");
        assert!(s.snapshot().provisional.is_some());
        s.gesture(GestureEvent::Escape);
        assert!(s.snapshot().provisional.is_none());
        assert!(s.snapshot().history.is_empty());
        tap(&mut s, "a7", "b8");
        s.command(Command::ChoosePromotion {
            piece: Piece::Knight,
        });
        assert_eq!(s.snapshot().history[0].san, "axb8=N");
    }

    #[test]
    fn always_queen_promotes_without_the_selector() {
        let mut s = promotion_session();
        s.set_always_queen(true);
        drag(&mut s, "a7", "a8");
        assert!(s.snapshot().provisional.is_none());
        assert_eq!(s.snapshot().history[0].san, "a8=Q");
    }

    #[test]
    fn enabling_always_queen_with_selector_open_promotes_now() {
        let mut s = promotion_session();
        tap(&mut s, "a7", "a8");
        assert!(s.snapshot().provisional.is_some());
        s.set_always_queen(true);
        assert_eq!(s.snapshot().history.len(), 1);
    }

    #[test]
    fn undo_redo_and_new_game_work_through_commands() {
        let mut s = Session::new();
        tap(&mut s, "e2", "e4");
        s.command(Command::Undo);
        assert!(s.snapshot().history.is_empty());
        s.command(Command::Redo);
        assert_eq!(s.snapshot().history.len(), 1);
        s.resign_side_to_move();
        assert_eq!(
            s.snapshot().outcome.unwrap().result,
            GameResult::WhiteWins // as pretas jogam e abandonam
        );
        s.new_game();
        assert!(s.snapshot().history.is_empty());
        assert!(s.snapshot().outcome.is_none());
    }

    #[test]
    fn engine_turn_ignores_the_pointer_and_resign_is_the_users() {
        let mut s = Session::new();
        s.start_engine_game(
            EngineSeat {
                color: Color::Black,
                elo: 1500,
            },
            EngineConfig::new("/nao/existe/engine"),
        );
        assert!(s.engine_active());
        tap(&mut s, "e2", "e4");
        assert!(s.engine_turn());
        // Na vez da Engine, mexer nas peças pretas não vale.
        drag(&mut s, "e7", "e5");
        assert_eq!(s.snapshot().history.len(), 1);
        s.resign_side_to_move();
        let outcome = s.snapshot().outcome.unwrap();
        assert_eq!(outcome.result, GameResult::BlackWins);
        assert_eq!(outcome.reason, EndReason::Resignation);
        // "Nova Partida" volta a dois jogadores.
        s.new_game();
        assert!(!s.engine_active());
    }

    #[test]
    fn review_is_read_only_and_does_not_create_events() {
        let mut s = Session::new();
        tap(&mut s, "e2", "e4");
        drag(&mut s, "e7", "e5");
        let events_before = s.game().events().len();
        let snapshot = s.snapshot().clone();
        s.set_review_ply(Some(1));
        assert_eq!(s.review().unwrap().ply, 1);
        assert_eq!(s.review().unwrap().last_move.as_ref().unwrap().san, "e4");
        // Gestos e Comandos são ignorados enquanto revisa.
        tap(&mut s, "g1", "f3");
        s.command(Command::Undo);
        assert_eq!(s.game().events().len(), events_before);
        assert_eq!(s.snapshot(), &snapshot);
        s.set_review_ply(Some(99));
        assert_eq!(s.review().unwrap().ply, 1, "ply inválido é ignorado");
        s.set_review_ply(None);
        assert!(s.review().is_none());
        tap(&mut s, "g1", "f3");
        assert_eq!(s.snapshot().history.len(), 3);
    }

    #[test]
    fn review_works_on_a_finished_game_without_changing_the_result() {
        let mut s = Session::new();
        tap(&mut s, "e2", "e4");
        s.command(Command::Resign { side: Color::Black });
        let outcome = s.snapshot().outcome;
        s.set_review_ply(Some(0));
        assert!(s.review().unwrap().is_start);
        assert_eq!(s.snapshot().outcome, outcome);
    }

    #[test]
    fn illegal_drop_is_recorded_with_its_reason_outside_the_legal_history() {
        let mut s = Session::new();
        tap(&mut s, "e2", "e4");
        drag(&mut s, "e7", "e2"); // e7 -> e2 não é Lance de peão
        let notice = s.illegal_notice().unwrap();
        assert_eq!(notice.from, sq("e7"));
        let incidents = s.illegal_incidents();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].after_ply, 1);
        assert_eq!(s.snapshot().history.len(), 1);
        assert!(
            !s.export_pgn(true, &[]).contains("e2"),
            "ilegal fora do PGN"
        );
    }

    fn competitive(policy: PolicyKind) -> Session {
        let mut s = Session::new();
        s.start(&NewGameConfig::for_policy(policy), None);
        s
    }

    #[test]
    fn fide_illegal_drop_stays_where_it_was_dropped_until_the_clock() {
        let mut s = competitive(PolicyKind::FideSimulation);
        drag(&mut s, "e2", "e5");
        let snap = s.snapshot();
        let release = snap.provisional_release.as_ref().unwrap();
        assert_eq!((release.from, release.to), (sq("e2"), sq("e5")));
        assert_eq!(
            s.take_hint().unwrap().square,
            sq("e5"),
            "fica onde foi solta"
        );
        assert!(!s.illegal_recently(), "sem aviso de Lance ilegal (DEC-32)");
        // Espaço / botão: a Infração é concluída e a posição regular volta.
        s.press_clock();
        let snap = s.snapshot();
        assert!(snap.provisional_release.is_none());
        assert_eq!(snap.infractions_white, 1);
        assert!(matches!(
            s.notice(),
            Some(Notice::Infraction {
                count: 1,
                ended: false
            })
        ));
    }

    #[test]
    fn fide_retract_fixes_an_illegal_drop_but_not_a_legal_one() {
        let mut s = competitive(PolicyKind::FideSimulation);
        drag(&mut s, "e2", "e5");
        s.retract();
        assert!(s.snapshot().provisional_release.is_none());
        assert_eq!(
            s.snapshot().must_move_from,
            vec![sq("e2")],
            "obrigação mantida"
        );
        drag(&mut s, "e2", "e4");
        s.retract();
        assert!(
            s.snapshot().made_move.is_some(),
            "Lance legal não volta (art. 4.7)"
        );
        assert_eq!(s.notice(), Some(&Notice::CannotRetract));
        s.press_clock();
        assert_eq!(s.snapshot().history[0].san, "e4");
        assert_eq!(s.snapshot().side_to_move, Color::Black);
    }

    #[test]
    fn touch_obligation_rejects_the_drop_of_another_piece_and_the_piece_returns() {
        let mut s = competitive(PolicyKind::FideSimulation);
        feed(&mut s, &[down("e2"), up("e2")]);
        feed(&mut s, &[GestureEvent::Escape]);
        let rev = s.revision();
        drag(&mut s, "d2", "d4");
        assert!(s.snapshot().made_move.is_none());
        assert!(matches!(s.notice(), Some(Notice::TouchObligation(sqs)) if sqs == &[sq("e2")]));
        assert_eq!(s.take_hint().unwrap().square, sq("d2"), "volta à origem");
        assert_ne!(s.revision(), rev, "a cena é reanimada");
    }

    #[test]
    fn the_clock_is_pressed_by_the_side_to_move_only_and_never_by_itself() {
        let mut s = competitive(PolicyKind::FideSimulation);
        assert!(s.can_press_clock(Color::White));
        assert!(
            !s.can_press_clock(Color::Black),
            "o relógio é de quem tem a vez"
        );
        s.tick();
        assert!(s.snapshot().history.is_empty(), "nada acontece sozinho");
        drag(&mut s, "e2", "e4");
        s.press_clock();
        assert!(!s.can_press_clock(Color::White));
        assert!(s.can_press_clock(Color::Black));
        // No Assistido o relógio não é acionado pelo usuário.
        assert!(!Session::new().can_press_clock(Color::White));
    }

    #[test]
    fn pause_blocks_the_clock_and_resume_restores_it() {
        let mut s = competitive(PolicyKind::FideSimulation);
        s.toggle_pause();
        assert!(s.snapshot().paused && !s.can_press_clock(Color::White));
        s.toggle_pause();
        assert!(!s.snapshot().paused && s.can_press_clock(Color::White));
    }

    #[test]
    fn claim_without_a_basis_is_rejected_with_the_penalty() {
        let mut s = competitive(PolicyKind::FideSimulation);
        s.claim_draw(None);
        assert_eq!(s.notice(), Some(&Notice::ClaimRejected));
        assert!(s.snapshot().outcome.is_none());
        let mv: Move = "e2e4".parse().unwrap();
        s.claim_draw(Some(mv));
        assert!(s.snapshot().outcome.is_none());
    }

    #[test]
    fn a_new_game_starts_with_the_configuration_and_competitive_has_no_auto_queen() {
        let mut s = Session::new();
        s.set_always_queen(true);
        s.start(&NewGameConfig::for_policy(PolicyKind::FideSimulation), None);
        assert_eq!(s.snapshot().policy, PolicyKind::FideSimulation);
        assert!(!s.always_queen(), "sem dama automática nas competitivas");
        s.set_always_queen(true);
        assert!(!s.always_queen());
        assert!(s.snapshot().clock.is_some(), "controle clássico padrão");
    }

    #[test]
    fn same_policy_through_the_same_commands_regardless_of_presentation() {
        // AC-02: a Partida só conhece Comandos; 2D e 3D enviam os mesmos.
        let mut a = competitive(PolicyKind::FideSimulation);
        let mut b = competitive(PolicyKind::FideSimulation);
        drag(&mut a, "g1", "f3");
        a.press_clock();
        b.command(Command::Touch { square: sq("g1") });
        b.command(Command::Release {
            from: sq("g1"),
            to: sq("f3"),
        });
        b.command(Command::PressClock);
        assert_eq!(a.snapshot().position, b.snapshot().position);
        assert_eq!(a.snapshot().history, b.snapshot().history);
    }
}
