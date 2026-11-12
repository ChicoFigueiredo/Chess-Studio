//! A sessão do FICS como visitante (#237, ADR-0007, CX-18): uma conexão ICS consultável por
//! quadro, o login de visitante, a busca de adversário, os desafios, a Partida jogada e a
//! observada.
//!
//! - Login: responde `guest` ao prompt de login e Enter ao "Press return…"; **nunca** manda
//!   senha. Se o servidor pedir senha, a sessão fecha ([`FicsError::PasswordRequested`]).
//! - Cada `style 12` passa pelo rastreador (o Lance é deduzido do tabuleiro pelo núcleo de
//!   regras) e vira um [`chess_game::ProviderSnapshot`]: o controlador aplica só o que falta.
//!   Repetido, atrasado ou parcial não regride; inalcançável marca a dessincronização.
//! - O Lance do jogador é intenção enviada e sai **uma vez** pelo TCP. Se a conexão cair, a
//!   Partida de visitante termina no FICS (derrota por desconexão): não há reenvio nem
//!   reconexão automática; outra sessão é outro visitante.
//! - Contra pessoa, a sessão segura a Trava de fair play da camada de rede do começo até o
//!   FICS confirmar o fim. Descartar a sessão solta a trava dela.
//! - A Partida observada não é do jogador: fica só na memória, para mostrar
//!   ([`FicsSession::observed`]); nunca vai ao banco.

use super::parse::{self, FicsChallenge, GameHeader, Message, Relation, Style12};
use super::tracker::{Step, Tracker};
use super::{FicsError, PROVIDER_KEY};
use chess_core::{CanonicalPosition, Color, Move, Piece};
use chess_game::{
    Command, Event, Game, GameSetup, OnlineGame, OnlinePlayer, Opponent, ProviderClock,
    ProviderEnd, ProviderSnapshot, Rejection, Snapshot, TimeControl, Timestamp as GameTime,
    Variant,
};
use chess_net::{Clock, FairPlayHold, IcsEvent, IcsLink, Net, ProviderId, Timestamp};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Desafios guardados ao mesmo tempo (um servidor hostil não enche a memória).
const MAX_CHALLENGES: usize = 16;
/// Comandos enviados logo depois do login: tabuleiro em `style 12`, sem quebra de linha, sem
/// campainha e sem anúncios de busca.
const SETUP_COMMANDS: [&str; 5] = [
    "set interface ChessStudio",
    "set style 12",
    "iset nowrap 1",
    "set bell 0",
    "set seek 0",
];

/// Situação da sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FicsStatus {
    /// Abrindo a conexão.
    Connecting,
    /// Conectado, entrando como visitante.
    LoggingIn,
    /// Pronto: o FICS confirmou a sessão de visitante.
    Ready,
    /// Encerrada (o motivo veio em [`FicsNotice::Closed`]).
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginStep {
    AwaitPrompt,
    SentGuest,
    SentReturn,
    Done,
}

/// O que a interface deve contar ao jogador.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FicsNotice {
    /// O FICS abriu a sessão de visitante com este nome.
    LoggedIn {
        handle: String,
    },
    /// A busca de adversário foi publicada.
    SeekPosted,
    /// Um desafio chegou (aceite com [`FicsSession::accept`]).
    Challenge(FicsChallenge),
    /// O FICS enviou o desafio por nome do jogador local (`Issuing: …`).
    ChallengeIssued {
        to: String,
    },
    /// O desafiado recusou o desafio por nome.
    ChallengeDeclined {
        by: String,
    },
    /// O desafiado não está no FICS: o desafio não foi feito.
    ChallengeFailed {
        name: String,
    },
    /// A Partida jogada começou: grave o setup ([`FicsPoll::created`]).
    GameStarted {
        game: u32,
        local_color: Color,
    },
    /// Partida que não é xadrez padrão (ou de tipo desconhecido): nada foi criado.
    VariantRefused {
        game: u32,
        variant: String,
    },
    /// O FICS confirmou o Lance enviado.
    MoveConfirmed {
        mv: Move,
    },
    /// O FICS registrou outro Lance no lugar do enviado.
    MoveReplaced {
        sent: Move,
    },
    /// O FICS recusou o Lance enviado; a intenção foi descartada.
    MoveRefused {
        mv: Move,
    },
    /// A conexão caiu (ou a Partida acabou) com o Lance sem confirmação; nada foi reenviado.
    MoveLost {
        mv: Move,
    },
    /// O adversário propôs empate.
    DrawOffered,
    /// O adversário pediu para abortar (aceite com [`FicsSession::abort`]).
    AbortRequested,
    /// O tabuleiro do FICS não tem caminho único a partir da Partida local: nada foi
    /// reescrito e não há Lance local até um tabuleiro coerente.
    Desynced {
        game: u32,
        ply: u64,
    },
    /// Um tabuleiro coerente desfez a dessincronização.
    Resynced {
        game: u32,
    },
    /// O FICS confirmou o fim da Partida jogada.
    GameEnded {
        game: u32,
    },
    ObservationStarted {
        game: u32,
    },
    ObservationEnded {
        game: u32,
    },
    /// A sessão parou.
    Closed(FicsError),
    /// O controlador recusou um retrato por outro motivo (não deveria acontecer).
    Rejected(Rejection),
}

/// Resultado de uma consulta da sessão.
#[derive(Debug, Default)]
pub struct FicsPoll {
    /// A Partida jogada acabou de nascer: grave o setup antes dos Eventos.
    pub created: bool,
    /// Eventos novos da Partida jogada (só o que o FICS confirmou), na ordem: grave-os.
    pub events: Vec<Event>,
    pub notices: Vec<FicsNotice>,
}

/// O desafio por nome que o jogador local fez (`match`), até a resposta. Só xadrez padrão,
/// sem rating (visitante).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicsOutgoing {
    pub opponent: String,
    pub minutes: u32,
    pub increment_s: u32,
    /// A cor pedida (`None`: o FICS sorteia).
    pub color: Option<Color>,
    /// O FICS confirmou o envio (`Issuing: …`).
    pub issued: bool,
}

/// Uma Partida observada, para mostrar. Não é Partida do jogador: não vai ao banco.
#[derive(Debug, Clone)]
pub struct ObservedGame {
    pub game: u32,
    pub white: String,
    pub black: String,
    /// Tabuleiro, Lances vistos desde o começo da observação, relógios e resultado.
    pub snapshot: Snapshot,
}

/// Uma Partida acompanhada (jogada ou observada).
struct Tracked {
    number: u32,
    game: Game,
    tracker: Tracker,
    /// `None` quando só observa.
    local: Option<Color>,
    white: String,
    black: String,
    opened_at: Timestamp,
    draw_offer: Option<Color>,
    clock: Option<ProviderClock>,
    desynced: Option<u64>,
}

impl Tracked {
    fn over(&self) -> bool {
        self.game.snapshot().is_over()
    }

    fn in_progress(&self) -> bool {
        !self.over()
    }

    fn opponent(&self) -> Option<Color> {
        self.local.map(Color::opposite)
    }

    fn name_of(&self, color: Color) -> &str {
        match color {
            Color::White => &self.white,
            Color::Black => &self.black,
        }
    }
}

/// Sessão do FICS, consultada por quadro ([`FicsSession::poll`] nunca bloqueia). Descartar
/// fecha a conexão e solta a Trava de fair play dela.
pub struct FicsSession {
    net: Net,
    clock: Arc<dyn Clock>,
    link: IcsLink,
    status: FicsStatus,
    login: LoginStep,
    login_deadline: Timestamp,
    handle: Option<String>,
    challenges: Vec<FicsChallenge>,
    /// O desafio por nome feito pelo jogador local, até a resposta (um por vez).
    outgoing: Option<FicsOutgoing>,
    /// A última Partida que o FICS anunciou criar (`{Game N … Creating …}`).
    creation: Option<GameHeader>,
    /// Partidas recusadas (variante): os tabuleiros delas são ignorados.
    refused: Vec<u32>,
    played: Option<Tracked>,
    /// O FICS confirmou a observação desta Partida; o cabeçalho chega logo depois.
    observing: Option<(u32, Option<GameHeader>)>,
    observed: Option<Tracked>,
    hold: Option<FairPlayHold>,
}

impl fmt::Debug for FicsSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FicsSession")
            .field("status", &self.status)
            .field("handle", &self.handle)
            .field("playing", &self.played.as_ref().map(|t| t.number))
            .field("observing", &self.observed.as_ref().map(|t| t.number))
            .finish_non_exhaustive()
    }
}

impl FicsSession {
    pub(crate) fn new(
        net: Net,
        clock: Arc<dyn Clock>,
        link: IcsLink,
        login_deadline: Duration,
    ) -> Self {
        let deadline = clock.now().plus(login_deadline);
        Self {
            net,
            clock,
            link,
            status: FicsStatus::Connecting,
            login: LoginStep::AwaitPrompt,
            login_deadline: deadline,
            handle: None,
            challenges: Vec::new(),
            outgoing: None,
            creation: None,
            refused: Vec::new(),
            played: None,
            observing: None,
            observed: None,
            hold: None,
        }
    }

    pub fn status(&self) -> FicsStatus {
        self.status
    }

    /// O nome de visitante que o FICS deu (`GuestABCD`).
    pub fn handle(&self) -> Option<&str> {
        self.handle.as_deref()
    }

    /// Desafios recebidos e ainda sem resposta.
    pub fn challenges(&self) -> &[FicsChallenge] {
        &self.challenges
    }

    /// O desafio por nome feito pelo jogador local, até a resposta.
    pub fn outgoing(&self) -> Option<&FicsOutgoing> {
        self.outgoing.as_ref()
    }

    /// A Partida jogada, depois do primeiro tabuleiro dela.
    pub fn game(&self) -> Option<&Game> {
        self.played.as_ref().map(|t| &t.game)
    }

    /// Número da Partida jogada no FICS.
    pub fn game_number(&self) -> Option<u32> {
        self.played.as_ref().map(|t| t.number)
    }

    /// A Partida observada (a atual ou a última), para mostrar.
    pub fn observed(&self) -> Option<ObservedGame> {
        self.observed.as_ref().map(|t| ObservedGame {
            game: t.number,
            white: t.white.clone(),
            black: t.black.clone(),
            snapshot: t.game.snapshot(),
        })
    }

    /// A Partida jogada está dessincronizada (nenhum Lance local até um tabuleiro coerente).
    pub fn is_desynced(&self) -> bool {
        self.played.as_ref().is_some_and(|t| t.desynced.is_some())
    }

    /// Esta sessão segura a Trava de fair play.
    pub fn holds_fair_play(&self) -> bool {
        self.hold.is_some()
    }

    // ---------------- Consulta por quadro ----------------

    /// Nunca bloqueia: aplica o que chegou do FICS e devolve os Eventos novos.
    pub fn poll(&mut self) -> FicsPoll {
        let mut out = FicsPoll::default();
        while self.status != FicsStatus::Closed {
            let Some(event) = self.link.try_next() else {
                break;
            };
            match event {
                IcsEvent::Connected => self.status = FicsStatus::LoggingIn,
                IcsEvent::Line(line) => self.on_message(parse::parse_line(&line), &mut out),
                IcsEvent::Prompt(text) => self.on_prompt(parse::parse_prompt(&text), &mut out),
                // Linha gigante: descartada pelo transporte; nada a fazer.
                IcsEvent::LineTooLong { .. } => {}
                IcsEvent::Ended => {
                    let in_game = self.played.as_ref().is_some_and(Tracked::in_progress);
                    self.close(
                        FicsError::ConnectionClosed {
                            game_in_progress: in_game,
                        },
                        &mut out,
                    );
                }
                IcsEvent::Failed(error) => self.close(FicsError::Provider(error.into()), &mut out),
            }
        }
        if self.status != FicsStatus::Ready
            && self.status != FicsStatus::Closed
            && self.clock.now() >= self.login_deadline
        {
            self.close(FicsError::LoginTimeout, &mut out);
        }
        self.sync_hold();
        out
    }

    fn close(&mut self, error: FicsError, out: &mut FicsPoll) {
        self.link.close();
        self.status = FicsStatus::Closed;
        self.outgoing = None;
        if let Some(tracked) = &mut self.played
            && let Some(sent) = tracked.game.drop_sent_move()
        {
            out.notices.push(FicsNotice::MoveLost { mv: sent.mv });
        }
        out.notices.push(FicsNotice::Closed(error));
    }

    fn send(&mut self, line: &str) -> Result<(), FicsError> {
        self.link
            .send_line(line)
            .map_err(|e| FicsError::Provider(e.into()))
    }

    // ---------------- Login de visitante ----------------

    fn on_prompt(&mut self, message: Message, out: &mut FicsPoll) {
        match message {
            Message::LoginPrompt | Message::PasswordPrompt | Message::GuestName(_) => {
                self.on_message(message, out);
            }
            _ => {}
        }
    }

    fn on_login(&mut self, message: &Message, out: &mut FicsPoll) {
        match message {
            // Nenhuma senha sai deste app sem canal cifrado (CX-18): fecha sem responder. Depois
            // do login um "password:" é só texto (um recado de alguém), não um pedido.
            Message::PasswordPrompt if self.login != LoginStep::Done => {
                self.close(FicsError::PasswordRequested, out);
            }
            Message::LoginPrompt if self.login == LoginStep::AwaitPrompt => {
                self.login = LoginStep::SentGuest;
                if let Err(e) = self.send("guest") {
                    self.close(e, out);
                }
            }
            Message::GuestName(name) if self.login == LoginStep::SentGuest => {
                self.login = LoginStep::SentReturn;
                self.handle = Some(name.clone());
                if let Err(e) = self.send("") {
                    self.close(e, out);
                }
            }
            Message::SessionStarted(name)
                if matches!(self.login, LoginStep::SentGuest | LoginStep::SentReturn) =>
            {
                self.login = LoginStep::Done;
                self.status = FicsStatus::Ready;
                self.handle = Some(name.clone());
                for command in SETUP_COMMANDS {
                    if let Err(e) = self.send(command) {
                        return self.close(e, out);
                    }
                }
                out.notices.push(FicsNotice::LoggedIn {
                    handle: name.clone(),
                });
            }
            _ => {}
        }
    }

    // ---------------- Mensagens ----------------

    fn on_message(&mut self, message: Message, out: &mut FicsPoll) {
        match &message {
            Message::LoginPrompt
            | Message::PasswordPrompt
            | Message::GuestName(_)
            | Message::SessionStarted(_) => return self.on_login(&message, out),
            _ => {}
        }
        if self.status != FicsStatus::Ready {
            return;
        }
        match message {
            Message::Board(board) => self.on_board(&board, out),
            Message::GameCreated(header) => {
                if self.is_me(&header.white) || self.is_me(&header.black) {
                    // O FICS reaproveita números: a Partida nova não herda a recusa da antiga.
                    self.refused.retain(|g| *g != header.game);
                    self.creation = Some(header);
                }
            }
            Message::GameEnded { game, end } => self.on_end(game, end, out),
            Message::ObservationStarted(game) => {
                self.refused.retain(|g| *g != game);
                self.observing = Some((game, None));
                self.observed = None;
            }
            Message::ObservedHeader(header) => {
                if let Some((game, info)) = &mut self.observing
                    && *game == header.game
                {
                    *info = Some(header);
                }
            }
            Message::ObservationRemoved(game) => {
                if self.observing.as_ref().is_some_and(|(g, _)| *g == game) {
                    self.observing = None;
                    out.notices.push(FicsNotice::ObservationEnded { game });
                }
            }
            Message::Challenge(challenge) => {
                if self.is_me(&challenge.receiver) && !self.is_me(&challenge.challenger) {
                    self.challenges
                        .retain(|c| !c.challenger.eq_ignore_ascii_case(&challenge.challenger));
                    if self.challenges.len() >= MAX_CHALLENGES {
                        self.challenges.remove(0);
                    }
                    self.challenges.push(challenge.clone());
                    out.notices.push(FicsNotice::Challenge(challenge));
                }
            }
            Message::MatchIssued(issued) => {
                if self.is_me(&issued.challenger)
                    && let Some(sent) = self.outgoing.as_mut()
                    && sent.opponent.eq_ignore_ascii_case(&issued.receiver)
                {
                    sent.issued = true;
                    out.notices.push(FicsNotice::ChallengeIssued {
                        to: sent.opponent.clone(),
                    });
                }
            }
            Message::MatchDeclined { by } => {
                if self
                    .outgoing
                    .as_ref()
                    .is_some_and(|o| o.opponent.eq_ignore_ascii_case(&by))
                    && let Some(sent) = self.outgoing.take()
                {
                    out.notices
                        .push(FicsNotice::ChallengeDeclined { by: sent.opponent });
                }
            }
            Message::MatchWithdrawn { by } => {
                self.challenges
                    .retain(|c| !c.challenger.eq_ignore_ascii_case(&by));
            }
            Message::NotLoggedIn { name } => {
                if self
                    .outgoing
                    .as_ref()
                    .is_some_and(|o| o.opponent.eq_ignore_ascii_case(&name))
                    && let Some(sent) = self.outgoing.take()
                {
                    out.notices.push(FicsNotice::ChallengeFailed {
                        name: sent.opponent,
                    });
                }
            }
            Message::DrawOffered { by } => self.on_draw_offer(&by, out),
            Message::DrawCancelled { by } => {
                // Só a recusa do jogador local ou a retirada de quem propôs.
                let offered_by = self
                    .played
                    .as_ref()
                    .and_then(|t| t.draw_offer.map(|c| t.name_of(c).to_owned()));
                let cancels =
                    by == "You" || offered_by.is_some_and(|n| n.eq_ignore_ascii_case(&by));
                if cancels
                    && let Some(tracked) = self.played.as_mut()
                    && tracked.draw_offer.take().is_some()
                {
                    let at = at(self.clock.as_ref(), tracked);
                    apply(tracked, None, at, out, true);
                }
            }
            Message::AbortRequested { by } => {
                if self
                    .played
                    .as_ref()
                    .is_some_and(|t| t.in_progress() && !self.is_me(&by))
                {
                    out.notices.push(FicsNotice::AbortRequested);
                }
            }
            Message::SeekPosted => out.notices.push(FicsNotice::SeekPosted),
            Message::MoveRefused => {
                if let Some(tracked) = self.played.as_mut()
                    && let Some(sent) = tracked.game.drop_sent_move()
                {
                    out.notices.push(FicsNotice::MoveRefused { mv: sent.mv });
                }
            }
            _ => {}
        }
    }

    fn is_me(&self, name: &str) -> bool {
        self.handle
            .as_deref()
            .is_some_and(|h| h.eq_ignore_ascii_case(name))
    }

    fn on_draw_offer(&mut self, by: &str, out: &mut FicsPoll) {
        let Some(tracked) = self.played.as_mut() else {
            return;
        };
        let Some(opponent) = tracked.opponent() else {
            return;
        };
        if !tracked.in_progress() || !tracked.name_of(opponent).eq_ignore_ascii_case(by) {
            return;
        }
        tracked.draw_offer = Some(opponent);
        let at = at(self.clock.as_ref(), tracked);
        apply(tracked, None, at, out, true);
        out.notices.push(FicsNotice::DrawOffered);
    }

    fn on_board(&mut self, board: &Style12, out: &mut FicsPoll) {
        if self.refused.contains(&board.game) {
            return;
        }
        if board.is_playing() {
            self.on_played_board(board, out);
        } else if board.relation == Relation::Observing {
            self.on_observed_board(board, out);
        }
        // Exame e posição isolada: fora do escopo (só Partidas em andamento).
    }

    fn refuse(&mut self, game: u32, variant: String, out: &mut FicsPoll) {
        if self.refused.len() >= MAX_CHALLENGES {
            self.refused.remove(0);
        }
        self.refused.push(game);
        out.notices
            .push(FicsNotice::VariantRefused { game, variant });
    }

    fn on_played_board(&mut self, board: &Style12, out: &mut FicsPoll) {
        let current = self.played.as_ref().map(|t| (t.number, t.in_progress()));
        // O FICS anunciou uma Partida nova com este número (pode ser o de uma que acabou).
        let fresh = matches!(&self.creation, Some(h) if h.game == board.game);
        match current {
            Some((number, true)) if number == board.game => {}
            // Outra Partida enquanto esta segue: ignorada.
            Some((_, true)) => return,
            // Tabuleiro atrasado de uma Partida que acabou: o controlador o ignora.
            Some((number, false)) if number == board.game && !fresh => {}
            _ => {
                let header = match &self.creation {
                    Some(h) if h.game == board.game => self.creation.take(),
                    _ => None,
                };
                let Some(header) = header else {
                    return self.refuse(board.game, "desconhecida".to_owned(), out);
                };
                if !accepts(&header.kind) {
                    return self.refuse(board.game, header.kind, out);
                }
                let local = if self.is_me(&board.white) {
                    Color::White
                } else if self.is_me(&board.black) {
                    Color::Black
                } else if board.relation == Relation::MyMove {
                    board.side
                } else {
                    board.side.opposite()
                };
                let setup = setup(board, &header, Some(local), None);
                let Ok(setup) = setup else {
                    return self.refuse(board.game, header.kind, out);
                };
                self.played =
                    Some(self.track(board, setup, Some(local), CanonicalPosition::start()));
                // Quem esperava resposta já tem a Partida (a busca o FICS retira sozinho).
                self.outgoing = None;
                out.created = true;
                out.notices.push(FicsNotice::GameStarted {
                    game: board.game,
                    local_color: local,
                });
            }
        }
        let clock = self.clock.clone();
        if let Some(tracked) = self.played.as_mut() {
            update(tracked, board, clock.as_ref(), out, true);
        }
    }

    fn on_observed_board(&mut self, board: &Style12, out: &mut FicsPoll) {
        let Some((game, header)) = self.observing.clone() else {
            return;
        };
        if game != board.game {
            return;
        }
        if self.observed.as_ref().is_none_or(|t| t.number != game) {
            let Some(header) = header else {
                // Sem o cabeçalho não se sabe o tipo: não dá para garantir xadrez padrão.
                let _ = self.send(&format!("unobserve {game}"));
                self.observing = None;
                return self.refuse(game, "desconhecida".to_owned(), out);
            };
            if !accepts(&header.kind) {
                let _ = self.send(&format!("unobserve {game}"));
                self.observing = None;
                return self.refuse(game, header.kind, out);
            }
            let fen = board.fen();
            let Ok(setup) = setup(board, &header, None, Some(&fen)) else {
                let _ = self.send(&format!("unobserve {game}"));
                self.observing = None;
                return self.refuse(game, "posição inválida".to_owned(), out);
            };
            let Ok(position) = CanonicalPosition::from_fen(&fen) else {
                return;
            };
            self.observed = Some(self.track(board, setup, None, position));
            out.notices.push(FicsNotice::ObservationStarted { game });
        }
        let clock = self.clock.clone();
        if let Some(tracked) = self.observed.as_mut() {
            update(tracked, board, clock.as_ref(), out, false);
        }
    }

    /// Começa a acompanhar a Partida do tabuleiro: a jogada desde a Posição inicial padrão, a
    /// observada desde a Posição em que a observação começou.
    fn track(
        &self,
        board: &Style12,
        setup: GameSetup,
        local: Option<Color>,
        start: CanonicalPosition,
    ) -> Tracked {
        let start_ply = if local.is_some() { 0 } else { board.ply() };
        Tracked {
            number: board.game,
            game: Game::new(setup),
            tracker: Tracker::new(start, start_ply),
            local,
            white: board.white.clone(),
            black: board.black.clone(),
            opened_at: self.clock.now(),
            draw_offer: None,
            clock: None,
            desynced: None,
        }
    }

    fn on_end(&mut self, game: u32, end: ProviderEnd, out: &mut FicsPoll) {
        let clock = self.clock.clone();
        if let Some(tracked) = self.played.as_mut()
            && tracked.number == game
            && tracked.in_progress()
        {
            let at = at(clock.as_ref(), tracked);
            apply(tracked, Some(end), at, out, true);
            if let Some(sent) = tracked.game.drop_sent_move() {
                out.notices.push(FicsNotice::MoveLost { mv: sent.mv });
            }
            if tracked.over() {
                out.notices.push(FicsNotice::GameEnded { game });
            }
        }
        if let Some(tracked) = self.observed.as_mut()
            && tracked.number == game
            && tracked.in_progress()
        {
            let at = at(clock.as_ref(), tracked);
            apply(tracked, Some(end), at, out, false);
        }
    }

    /// Contra pessoa e em andamento: segura a trava; fim confirmado: solta.
    fn sync_hold(&mut self) {
        let required = self
            .played
            .as_ref()
            .is_some_and(|t| t.game.online_human_in_progress());
        if !required {
            self.hold = None;
        } else if self.hold.is_none() {
            let number = self.played.as_ref().map_or(0, |t| t.number);
            self.hold = Some(self.net.fair_play_guard().hold(format!(
                "Partida online contra pessoa no FICS (Partida {number})"
            )));
        }
    }

    // ---------------- Comandos ----------------

    fn ready(&self) -> Result<(), FicsError> {
        match self.status {
            FicsStatus::Ready => Ok(()),
            FicsStatus::Closed => Err(FicsError::NotAllowed("a sessão com o FICS foi encerrada")),
            _ => Err(FicsError::NotAllowed(
                "a entrada como visitante no FICS ainda não terminou",
            )),
        }
    }

    fn playing(&self) -> Result<&Tracked, FicsError> {
        self.ready()?;
        self.played
            .as_ref()
            .filter(|t| t.in_progress())
            .ok_or(FicsError::NotAllowed("não há Partida em andamento"))
    }

    fn idle(&self) -> Result<(), FicsError> {
        self.ready()?;
        if self.played.as_ref().is_some_and(Tracked::in_progress) {
            return Err(FicsError::NotAllowed(
                "termine a Partida em andamento antes",
            ));
        }
        Ok(())
    }

    /// Busca adversário: xadrez padrão, sem rating (visitante só joga sem rating), início
    /// automático quando alguém aceitar. `base_minutes` e `increment_s` como no FICS.
    pub fn seek(&mut self, base_minutes: u32, increment_s: u32) -> Result<(), FicsError> {
        self.idle()?;
        pace(base_minutes, increment_s)?;
        self.send(&format!("seek {base_minutes} {increment_s} unrated"))
    }

    /// Desafia `opponent` pelo nome (`match`): xadrez padrão, sem rating, no ritmo dado e,
    /// se pedida, com a cor. Um por vez. Tudo é conferido **antes da rede**: o nome só pode ter
    /// letras (nada de espaço, número ou palavra a mais, como `wild`), então nenhuma variante
    /// chega ao FICS. A Partida começa pelo tabuleiro, como na busca.
    pub fn challenge(
        &mut self,
        opponent: &str,
        minutes: u32,
        increment_s: u32,
        color: Option<Color>,
    ) -> Result<(), FicsError> {
        self.idle()?;
        if !super::Fics::valid_handle(opponent) {
            return Err(FicsError::NotAllowed(
                "o nome no FICS tem de 3 a 17 letras, sem espaço, número nem símbolo",
            ));
        }
        if self.is_me(opponent) {
            return Err(FicsError::NotAllowed("não dá para desafiar a si mesmo"));
        }
        if self.outgoing.is_some() {
            return Err(FicsError::NotAllowed(
                "já há um desafio enviado; retire-o ou espere a resposta",
            ));
        }
        pace(minutes, increment_s)?;
        let side = match color {
            Some(Color::White) => " white",
            Some(Color::Black) => " black",
            None => "",
        };
        self.send(&format!(
            "match {opponent} {minutes} {increment_s} unrated{side}"
        ))?;
        self.outgoing = Some(FicsOutgoing {
            opponent: opponent.to_owned(),
            minutes,
            increment_s,
            color,
            issued: false,
        });
        Ok(())
    }

    /// Retira o desafio por nome enviado.
    pub fn withdraw_challenge(&mut self) -> Result<(), FicsError> {
        self.ready()?;
        let Some(sent) = self.outgoing.take() else {
            return Ok(());
        };
        self.send(&format!("withdraw {}", sent.opponent))
    }

    /// Retira a busca de adversário.
    pub fn unseek(&mut self) -> Result<(), FicsError> {
        self.ready()?;
        self.send("unseek")
    }

    fn received(&self, challenger: &str) -> Result<FicsChallenge, FicsError> {
        self.challenges
            .iter()
            .find(|c| c.challenger.eq_ignore_ascii_case(challenger))
            .cloned()
            .ok_or(FicsError::NotAllowed("não há desafio desse jogador"))
    }

    /// Aceita o desafio de `challenger`. Só xadrez padrão: variante é recusada aqui, antes da
    /// rede.
    pub fn accept(&mut self, challenger: &str) -> Result<(), FicsError> {
        self.idle()?;
        let challenge = self.received(challenger)?;
        if !accepts(&challenge.kind) {
            return Err(FicsError::UnsupportedVariant(challenge.kind));
        }
        self.send(&format!("accept {}", challenge.challenger))?;
        self.challenges
            .retain(|c| c.challenger != challenge.challenger);
        Ok(())
    }

    /// Recusa o desafio de `challenger`.
    pub fn decline(&mut self, challenger: &str) -> Result<(), FicsError> {
        self.ready()?;
        let challenge = self.received(challenger)?;
        self.send(&format!("decline {}", challenge.challenger))?;
        self.challenges
            .retain(|c| c.challenger != challenge.challenger);
        Ok(())
    }

    /// Observa a Partida `game` do FICS (só xadrez padrão; a observação começa na Posição
    /// atual dela).
    pub fn observe(&mut self, game: u32) -> Result<(), FicsError> {
        self.idle()?;
        if game == 0 {
            return Err(FicsError::NotAllowed("número de Partida inválido"));
        }
        self.send(&format!("observe {game}"))
    }

    /// Para de observar.
    pub fn unobserve(&mut self) -> Result<(), FicsError> {
        self.ready()?;
        self.send("unobserve")
    }

    /// O jogador soltou a peça: intenção enviada, validada pelo núcleo de regras e enviada ao
    /// FICS **uma vez**. Só vira Lance quando o tabuleiro do FICS o confirma.
    pub fn play(&mut self, mv: Move) -> Result<(), FicsError> {
        let tracked = self.playing()?;
        if tracked.desynced.is_some() {
            return Err(FicsError::Desynced);
        }
        let command = move_command(tracked.tracker.position(), mv);
        let Some(tracked) = self.played.as_mut() else {
            return Err(FicsError::NotAllowed("não há Partida em andamento"));
        };
        tracked.game.send_move(mv).map_err(FicsError::Rejected)?;
        if let Err(e) = self.send(&command) {
            if let Some(tracked) = self.played.as_mut() {
                tracked.game.drop_sent_move();
            }
            return Err(e);
        }
        Ok(())
    }

    /// Pede o abandono. A Partida (e a Trava de fair play) só muda quando o FICS confirmar.
    pub fn resign(&mut self) -> Result<(), FicsError> {
        self.playing()?;
        self.send("resign")
    }

    /// Pede para abortar (ou aceita o pedido do adversário).
    pub fn abort(&mut self) -> Result<(), FicsError> {
        self.playing()?;
        self.send("abort")
    }

    /// Propõe empate.
    pub fn offer_draw(&mut self) -> Result<(), FicsError> {
        self.playing()?;
        self.send("draw")
    }

    fn opponent_offer(&self) -> Result<(), FicsError> {
        let tracked = self.playing()?;
        if tracked.draw_offer.is_some() && tracked.draw_offer == tracked.opponent() {
            Ok(())
        } else {
            Err(FicsError::NotAllowed("o adversário não propôs empate"))
        }
    }

    /// Aceita a proposta de empate do adversário (no FICS, `draw` aceita a proposta).
    pub fn accept_draw(&mut self) -> Result<(), FicsError> {
        self.opponent_offer()?;
        self.send("draw")
    }

    pub fn decline_draw(&mut self) -> Result<(), FicsError> {
        self.opponent_offer()?;
        self.send("decline")
    }

    /// Fecha a conexão (a Trava de fair play só cai com o fim confirmado ou ao descartar a
    /// sessão).
    pub fn close_connection(&mut self) {
        if self.status != FicsStatus::Closed {
            self.link.close();
            self.status = FicsStatus::Closed;
        }
    }
}

/// O ritmo que o FICS aceita (minutos de base e segundos de incremento).
fn pace(minutes: u32, increment_s: u32) -> Result<(), FicsError> {
    if minutes > 600 || increment_s > 300 {
        return Err(FicsError::NotAllowed("ritmo fora do que o FICS aceita"));
    }
    Ok(())
}

/// O FICS aceita só os tipos de xadrez padrão do catálogo (`blitz`, `lightning`,
/// `standard`, `untimed`).
fn accepts(kind: &str) -> bool {
    ProviderId::Fics.info().accepts_variant(kind)
}

fn setup(
    board: &Style12,
    header: &GameHeader,
    local: Option<Color>,
    start_fen: Option<&str>,
) -> Result<GameSetup, chess_game::OnlineSetupError> {
    let game = OnlineGame {
        provider: PROVIDER_KEY.to_owned(),
        remote_id: board.game.to_string(),
        // Observando não há jogador local: a convenção é brancas, e a Partida observada
        // nunca sai da sessão (nem para o banco).
        local_color: local.unwrap_or(Color::White),
        opponent: Opponent::Human,
        white: OnlinePlayer {
            name: board.white.clone(),
            rating: None,
        },
        black: OnlinePlayer {
            name: board.black.clone(),
            rating: None,
        },
        rated: header.rated,
        days_per_move: None,
    };
    let setup = GameSetup::online(game, Variant::Standard, start_fen)?;
    Ok(if board.initial_minutes == 0 && board.increment_s == 0 {
        setup
    } else {
        setup.with_time_control(TimeControl {
            base_ms: u64::from(board.initial_minutes) * 60_000,
            increment_ms: u64::from(board.increment_s) * 1000,
        })
    })
}

/// Instante do domínio: o relógio injetado desde a criação, nunca para trás.
fn at(clock: &dyn Clock, tracked: &Tracked) -> GameTime {
    let now = clock.now();
    let elapsed = u64::try_from(now.since(tracked.opened_at).as_millis()).unwrap_or(u64::MAX);
    let last = tracked.game.events().last().map_or(0, |e| e.at.mono_ms);
    GameTime::with_wall(elapsed.max(last), now.as_millis())
}

fn update(
    tracked: &mut Tracked,
    board: &Style12,
    clock: &dyn Clock,
    out: &mut FicsPoll,
    played: bool,
) {
    match tracked.tracker.apply(board) {
        Step::Stale => {}
        Step::Unreachable { ply } => {
            if tracked.desynced != Some(ply) {
                tracked.desynced = Some(ply);
                out.notices.push(FicsNotice::Desynced {
                    game: tracked.number,
                    ply,
                });
            }
        }
        step @ (Step::Same | Step::Advanced) => {
            if step == Step::Advanced {
                // Um Lance encerra a proposta de empate anterior.
                tracked.draw_offer = None;
            }
            if tracked.desynced.take().is_some() {
                out.notices.push(FicsNotice::Resynced {
                    game: tracked.number,
                });
            }
            let (white_ms, black_ms) = board.clock_ms();
            tracked.clock = Some(ProviderClock {
                white_ms,
                black_ms,
                running: board.ticking.unwrap_or(true),
            });
            let at = at(clock, tracked);
            apply(tracked, None, at, out, played);
        }
    }
}

/// Aplica o retrato atual (com o fim, se houver) e resolve a intenção enviada.
fn apply(
    tracked: &mut Tracked,
    end: Option<ProviderEnd>,
    at: GameTime,
    out: &mut FicsPoll,
    played: bool,
) {
    let has_clock = tracked.game.setup().time_control().is_some();
    let snapshot = ProviderSnapshot {
        moves: tracked.tracker.uci(),
        clock: tracked.clock.filter(|_| has_clock),
        draw_offer: tracked.draw_offer,
        end,
    };
    let sent = tracked.game.sent_move().cloned();
    match tracked.game.handle(Command::Provider(snapshot), at) {
        Ok(events) => {
            if played {
                out.events.extend(events);
            }
        }
        Err(rejection) => out.notices.push(FicsNotice::Rejected(rejection)),
    }
    if let Some(sent) = sent
        && tracked.game.sent_move().is_none()
    {
        let history = tracked.game.snapshot().history;
        out.notices.push(match history.get(sent.ply - 1) {
            Some(h) if h.mv == sent.mv => FicsNotice::MoveConfirmed { mv: sent.mv },
            Some(_) => FicsNotice::MoveReplaced { sent: sent.mv },
            None => FicsNotice::MoveLost { mv: sent.mv },
        });
    }
}

/// O Lance como o FICS o lê: coordenadas (`e2e4`), roque em notação `o-o`/`o-o-o` e
/// promoção com `=` (`e7e8=q`).
pub(crate) fn move_command(position: &CanonicalPosition, mv: Move) -> String {
    let king = matches!(position.piece_at(mv.from()), Some((_, Piece::King)));
    if king && mv.from().file().abs_diff(mv.to().file()) == 2 {
        return if mv.to().file() > mv.from().file() {
            "o-o".to_owned()
        } else {
            "o-o-o".to_owned()
        };
    }
    match mv.promotion() {
        Some(piece) => {
            let letter = match piece {
                Piece::Rook => 'r',
                Piece::Bishop => 'b',
                Piece::Knight => 'n',
                _ => 'q',
            };
            format!("{}{}={letter}", mv.from(), mv.to())
        }
        None => format!("{}{}", mv.from(), mv.to()),
    }
}
