//! Partida local do app: a [`Game`] (Assistido, dois jogadores no mesmo computador), o
//! intérprete de gestos e o relógio monotônico (ADR-0001/0002).
//!
//! Só esta estrutura escreve a Partida. O Bevy lê [`Session::snapshot`] para desenhar e
//! manda intenções (`gesture`, `command`). O instante dos Comandos vem do relógio do app
//! (`Instant`), nunca do domínio. Sem dependência do Bevy além do atributo `Resource`:
//! o fluxo inteiro (ponteiro -> Comando -> Partida) é testável sem janela.

use bevy::prelude::{Resource, Vec3, warn};
use chess_core::{Color, Piece, Square};
use chess_game::{
    Command, EngineSeat, EventKind, Game, GameSetup, PolicyKind, Snapshot, Timestamp,
};
use chess_input::{DRAG_HEIGHT, GestureEvent, GestureInterpreter, GestureVisual, MoveMethod};
use chess_uci::{EngineConfig, EnginePlayer};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Quanto tempo a mensagem de Lance ilegal fica no painel.
const ILLEGAL_NOTICE: Duration = Duration::from_secs(3);

/// Onde a peça deve ser encaixada após um gesto e de onde a animação parte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleHint {
    /// Casa canônica da peça depois do Comando (destino do Lance, ou origem se ilegal).
    pub square: Square,
    /// Ponto do mundo onde a peça estava na tela ao soltar.
    pub from: Vec3,
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
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        let game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        Self {
            snapshot: game.snapshot(),
            game,
            revision: 0,
            started: Instant::now(),
            interpreter: GestureInterpreter::new(MoveMethod::default()),
            always_queen: false,
            hint: None,
            illegal_at: None,
            engine: None,
        }
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
        let mono_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok());
        Timestamp { mono_ms, wall_ms }
    }

    /// Evento de gesto -> Comandos do intérprete -> Partida.
    pub fn gesture(&mut self, event: GestureEvent) {
        // Na vez da Engine o ponteiro não move nada (ela joga as próprias peças).
        if self.engine_turn() {
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
    pub fn new_game(&mut self) {
        let method = self.interpreter.method();
        let always_queen = self.always_queen;
        *self = Self::new();
        self.interpreter.set_method(method);
        self.always_queen = always_queen;
        self.revision = self.revision.wrapping_add(1);
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
        let method = self.interpreter.method();
        let always_queen = self.always_queen;
        let game = Game::new(GameSetup::standard(PolicyKind::Assisted).with_engine(seat));
        *self = Self::new();
        self.interpreter.set_method(method);
        self.always_queen = always_queen;
        self.snapshot = game.snapshot();
        self.game = game;
        self.engine = Some(EnginePlayer::new(seat, config));
        self.revision = self.revision.wrapping_add(1);
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
                self.snapshot = self.game.snapshot();
                self.revision = self.revision.wrapping_add(1);
            }
            Err(rejection) => warn!("Comando {:?} recusado: {rejection}", command.kind()),
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
        } else if has(|k| matches!(k, EventKind::PromotionPending { .. })) {
            self.hint = Some(SettleHint {
                square: to,
                from: start,
            });
            if self.always_queen {
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
}
