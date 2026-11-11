//! Engine online (CX-13): uma Engine UCI local joga Partidas online pela Bot API
//! (`/api/bot/...`), sempre por uma **Conexão de Engine** (`bot:play`, conta BOT separada),
//! nunca pela Conexão do jogador. O contrário também vale: [`crate::LichessBoard`] recusa a
//! Conexão de Engine. Uma Partida da Engine online não conta como Partida de pessoa (chave
//! `lichess-bot` no lugar de `lichess`) e não liga a Trava de fair play: é uma Engine
//! declarada.
//!
//! - [`LichessBot::events`]: o fluxo de eventos da Conexão (desafios, começo e fim de
//!   Partidas), com reconexão, igual ao da Board API.
//! - [`ChallengePolicy`]: quais desafios a Engine aceita (ritmos, com ou sem rating, só
//!   pessoas ou também outras Engines, Partidas simultâneas). Só xadrez padrão: a variante é
//!   sempre conferida no catálogo. [`LichessBot::answer_challenge`] decide e responde.
//! - [`LichessBot::open_game`]: a sessão da Partida ([`crate::OnlineGameSession`]); a cada
//!   `poll` a Engine pensa na vez dela, o núcleo de regras valida o Lance e ele sai **uma
//!   vez**. Lance ilegal da Engine: nada é enviado.
//! - [`LichessBot::upgrade_to_bot`]: a conversão da conta em BOT é **irreversível** e nunca
//!   acontece sozinha; só com uma confirmação explícita ([`IrreversibleConfirmation`]).
//!
//! ```no_run
//! use chess_providers::{ChallengeDecision, EnginePolicy, LichessBot};
//! # fn demo(bot: LichessBot, engine: Box<dyn chess_providers::MoveEngine>) {
//! let mut events = bot.events();
//! // A cada quadro:
//! for event in events.poll() {
//!     if let chess_providers::ConnectionEvent::Challenge(challenge) = event {
//!         let answer = bot.answer_challenge(&challenge, 0);
//!         println!("{:?}", answer.decision);
//!     }
//! }
//! # let _ = engine;
//! # }
//! ```

use crate::answer::Query;
use crate::auth::ConnectionRole;
use crate::board::{
    Backoff, BoardError, Challenge, ConnectionEvents, DeclineReason, EnginePolicy, EngineSide,
    GameInfo, LichessBoard, MoveEngine, OnlineGameSession, OnlineTime,
};
use crate::error::Missing;
use crate::family::ProviderProfile;
use chess_net::{Clock, ConnectionId, Net};
use std::fmt;
use std::sync::Arc;

/// Ritmo de um desafio, como o Lichess o classifica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChallengeSpeed {
    UltraBullet,
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Correspondence,
}

impl ChallengeSpeed {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "ultraBullet" => Some(Self::UltraBullet),
            "bullet" => Some(Self::Bullet),
            "blitz" => Some(Self::Blitz),
            "rapid" => Some(Self::Rapid),
            "classical" => Some(Self::Classical),
            "correspondence" => Some(Self::Correspondence),
            _ => None,
        }
    }
}

/// Com ou sem rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatedPolicy {
    /// Só casuais.
    CasualOnly,
    /// Só com rating.
    RatedOnly,
    Both,
}

/// Contra quem a Engine aceita jogar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpponentPolicy {
    /// Só pessoas.
    PeopleOnly,
    /// Pessoas e outras Engines (contas BOT).
    PeopleAndEngines,
    /// Só outras Engines.
    EnginesOnly,
}

/// Política de aceitação de desafios. O padrão é conservador: ritmos de blitz a clássico,
/// casuais e com rating, só pessoas, uma Partida por vez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengePolicy {
    pub speeds: Vec<ChallengeSpeed>,
    pub rated: RatedPolicy,
    pub opponents: OpponentPolicy,
    /// Partidas ao mesmo tempo (1 por padrão).
    pub max_concurrent: usize,
}

impl Default for ChallengePolicy {
    fn default() -> Self {
        Self {
            speeds: vec![
                ChallengeSpeed::Blitz,
                ChallengeSpeed::Rapid,
                ChallengeSpeed::Classical,
            ],
            rated: RatedPolicy::Both,
            opponents: OpponentPolicy::PeopleOnly,
            max_concurrent: 1,
        }
    }
}

/// O que a política decidiu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeDecision {
    Accept,
    Decline(DeclineReason),
}

impl ChallengePolicy {
    /// Decide sobre `challenge` com `active_games` Partidas da Engine em andamento. A variante
    /// é conferida primeiro e nunca é configurável: só xadrez padrão.
    pub fn decide(
        &self,
        profile: &ProviderProfile,
        challenge: &Challenge,
        active_games: usize,
    ) -> ChallengeDecision {
        use ChallengeDecision::Decline;
        if !profile.accepts_variant(&challenge.variant) {
            return Decline(DeclineReason::Standard);
        }
        let is_bot = challenge.challenger.title.as_deref() == Some("BOT");
        match (self.opponents, is_bot) {
            (OpponentPolicy::PeopleOnly, true) => return Decline(DeclineReason::NoBot),
            (OpponentPolicy::EnginesOnly, false) => return Decline(DeclineReason::OnlyBot),
            _ => {}
        }
        match (self.rated, challenge.rated) {
            (RatedPolicy::CasualOnly, true) => return Decline(DeclineReason::Casual),
            (RatedPolicy::RatedOnly, false) => return Decline(DeclineReason::Rated),
            _ => {}
        }
        if matches!(challenge.time, OnlineTime::Unlimited) {
            return Decline(DeclineReason::TimeControl);
        }
        match ChallengeSpeed::parse(&challenge.speed) {
            Some(speed) if self.speeds.contains(&speed) => {}
            _ => return Decline(DeclineReason::TimeControl),
        }
        if active_games >= self.max_concurrent {
            return Decline(DeclineReason::Later);
        }
        ChallengeDecision::Accept
    }
}

/// A resposta dada a um desafio: a decisão e o pedido ao Provedor.
pub struct ChallengeAnswer {
    pub decision: ChallengeDecision,
    /// O pedido (aceitar ou recusar) a caminho; consulte com `try_take`.
    pub request: Query<()>,
}

impl fmt::Debug for ChallengeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChallengeAnswer")
            .field("decision", &self.decision)
            .finish_non_exhaustive()
    }
}

/// Confirmação explícita de que o usuário leu o aviso da conversão em BOT. Só a interface a
/// cria, depois de mostrar [`IrreversibleConfirmation::WARNING`] e o usuário confirmar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrreversibleConfirmation(());

impl IrreversibleConfirmation {
    /// Texto para a interface mostrar antes de pedir a confirmação.
    pub const WARNING: &'static str = "Converter a conta em conta BOT não tem volta. Uma conta \
        BOT joga só pela interface de programação (uma Engine), não pelo site, e não pode voltar \
        a ser uma conta comum. A conta precisa não ter nenhuma Partida jogada. Use uma conta \
        criada só para a Engine, nunca a sua conta pessoal.";

    /// O usuário viu o aviso e confirmou.
    pub fn user_confirmed() -> Self {
        Self(())
    }
}

/// Cliente da Bot API de um Provedor da família do Lichess, por uma Conexão de Engine.
#[derive(Clone)]
pub struct LichessBot {
    client: LichessBoard,
    policy: ChallengePolicy,
}

impl fmt::Debug for LichessBot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LichessBot")
            .field("connection", &self.client.connection)
            .finish_non_exhaustive()
    }
}

impl LichessBot {
    /// A Bot API do Lichess. `role` é o papel da Conexão na Central: a do jogador é recusada
    /// antes de qualquer pedido ([`BoardError::PlayerConnection`]).
    pub fn new(
        net: Net,
        clock: Arc<dyn Clock>,
        connection: ConnectionId,
        role: ConnectionRole,
    ) -> Result<Self, BoardError> {
        Self::with_profile(ProviderProfile::LICHESS, net, clock, connection, role)
    }

    /// A Bot API de outro Provedor da família (o PlayStrategy).
    pub fn with_profile(
        profile: ProviderProfile,
        net: Net,
        clock: Arc<dyn Clock>,
        connection: ConnectionId,
        role: ConnectionRole,
    ) -> Result<Self, BoardError> {
        if role != ConnectionRole::Engine {
            return Err(BoardError::PlayerConnection);
        }
        Ok(Self {
            client: LichessBoard::build(profile, net, clock, connection),
            policy: ChallengePolicy::default(),
        })
    }

    pub fn provider_profile(&self) -> &ProviderProfile {
        &self.client.profile
    }

    /// Política de aceitação de desafios.
    pub fn challenge_policy(mut self, policy: ChallengePolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn policy(&self) -> &ChallengePolicy {
        &self.policy
    }

    /// Espera entre reconexões dos fluxos.
    pub fn backoff(mut self, backoff: Backoff) -> Self {
        self.client = self.client.backoff(backoff);
        self
    }

    /// Semente do jitter (testes reprodutíveis).
    pub fn jitter_seed(mut self, seed: u64) -> Self {
        self.client = self.client.jitter_seed(seed);
        self
    }

    /// Abre o fluxo de eventos da Conexão de Engine (desafios, `gameStart`, `gameFinish`).
    pub fn events(&self) -> ConnectionEvents {
        self.client.events()
    }

    /// Partidas da Conexão em andamento (para retomar depois de reabrir o app).
    pub fn playing(&self) -> Query<Vec<GameInfo>> {
        self.client.playing()
    }

    /// O que a política diz sobre o desafio, sem responder ao Provedor.
    pub fn decide(&self, challenge: &Challenge, active_games: usize) -> ChallengeDecision {
        self.policy
            .decide(&self.client.profile, challenge, active_games)
    }

    /// Decide pela política e responde ao Provedor (aceita ou recusa com o motivo). Quem
    /// chama conta as Partidas em andamento (`active_games`).
    pub fn answer_challenge(&self, challenge: &Challenge, active_games: usize) -> ChallengeAnswer {
        let decision = self.decide(challenge, active_games);
        let request = match decision {
            ChallengeDecision::Accept => self.client.accept_challenge(challenge),
            ChallengeDecision::Decline(reason) => {
                self.client.decline_challenge(&challenge.id, reason)
            }
        };
        ChallengeAnswer { decision, request }
    }

    /// Sessão de uma Partida nova, jogada por `engine` (avisada por `gameStart` ou listada em
    /// `playing`). A Partida nasce do primeiro `gameFull`.
    pub fn open_game(
        &self,
        info: &GameInfo,
        engine: Box<dyn MoveEngine>,
        policy: EnginePolicy,
    ) -> OnlineGameSession {
        OnlineGameSession::open_engine(self.client.clone(), info, EngineSide::new(engine, policy))
    }

    /// Sessão de uma Partida da Engine que já está no log (reabrir o app). Uma Partida de
    /// pessoa é recusada ([`BoardError::WrongGame`]).
    pub fn resume_game(
        &self,
        game: chess_game::Game,
        engine: Box<dyn MoveEngine>,
        policy: EnginePolicy,
    ) -> Result<OnlineGameSession, BoardError> {
        OnlineGameSession::resume_with(
            self.client.clone(),
            game,
            Some(EngineSide::new(engine, policy)),
        )
    }

    /// Converte a conta da Conexão em conta BOT (`POST /api/bot/account/upgrade`).
    /// **Irreversível**: o adaptador nunca chama isto sozinho; a interface mostra
    /// [`IrreversibleConfirmation::WARNING`] e só então cria a confirmação.
    pub fn upgrade_to_bot(&self, _confirmed: IrreversibleConfirmation) -> Query<()> {
        crate::answer::fetch(
            &self.client.net,
            self.client.post("/api/bot/account/upgrade", None),
            Missing::NoData,
            |_| Ok(()),
        )
    }
}
