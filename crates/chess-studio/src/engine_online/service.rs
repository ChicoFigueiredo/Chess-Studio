//! O serviço da Engine online no app (#252, CX-13), sem Bevy nem egui: uma Engine local
//! instalada responde a desafios do Lichess pela Bot API, com a Conexão de **Engine**
//! (`bot:play`), nunca a do jogador.
//!
//! - **Nada inicia sozinho.** Só [`EngineOnline::start`] (um clique) abre o fluxo de eventos;
//!   até lá o serviço não faz nenhum pedido. Reabrir o app volta a `Parada`.
//! - **Limites do usuário.** A [`ChallengePolicy`] (ritmos, com ou sem rating, quem pode
//!   desafiar, Partidas ao mesmo tempo) e o máximo de Partidas por execução. Desafio fora dos
//!   limites é recusado com o motivo do Lichess.
//! - **Parar.** [`EngineOnline::stop`] deixa de aceitar desafios e espera as Partidas em
//!   andamento terminarem; [`EngineOnline::stop_now`] abandona as que estão em andamento.
//!   Aceitar um desafio e parar antes do `gameStart` não deixa Partida solta: o serviço espera
//!   o início (ou o prazo) antes de fechar.
//! - **Autoridade.** Quem escreve a Partida é a `OnlineGameSession` (o controlador); este
//!   serviço só grava no banco os Eventos que ela devolve. O núcleo de regras valida cada
//!   Lance da Engine (ADR-0003): Lance ilegal nunca é enviado.
//! - **Sem Trava de fair play própria.** É uma Engine declarada (AD-05): a Partida da Engine
//!   online não liga a Trava, e esta tela não consulta a Engine remota. Mas a Engine online
//!   calcula: com a Trava ligada por uma Partida online contra pessoa ela não liga, recusa os
//!   desafios que a Trava não deixa e cada Lance dela pede licença (FP-04, #261).

use crate::fair_play::{Computation, FairPlay, FairPlayLocked, GuardedMoveEngine};
use crate::net_service::NetService;
use crate::online::service::StoreAccess;
use chess_net::{Clock, ProviderId, Timestamp};
use chess_providers::{
    BoardError, Challenge, ChallengeAnswer, ChallengeDecision, ChallengePolicy, ConnectionEvent,
    ConnectionEvents, ConnectionRole, DeclineReason, DrawResponse, EnginePolicy, GameInfo,
    IrreversibleConfirmation, LichessBot, LinkStatus, MoveEngine, OnlineGameSession, ProviderError,
    Query, SessionNotice,
};
use chess_store::{ConnectionRole as StoreRole, GameId, GameMetadata};
use chess_uci::{ControllerEvent, EngineConfig, EngineController, Failure, FailureKind, Strength};
use serde_json::json;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

/// Chave do Provedor nas Partidas da Engine online (`chess_game::ENGINE_GAME_SUFFIX`).
pub const BOT_KEY: &str = "lichess-bot";
/// Quanto tempo um desafio aceito espera pelo `gameStart` antes de ser dado como perdido.
const ACCEPTED_TTL_MS: i64 = 60_000;
/// Quantos avisos a tela guarda.
const ACTIVITY_CAP: usize = 12;

/// A Engine local que vai jogar: uma do gerenciador (já registrada e liberada).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineSpec {
    pub name: String,
    pub path: PathBuf,
    pub options: Vec<(String, String)>,
}

/// Os limites escolhidos pelo usuário.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub policy: ChallengePolicy,
    /// Máximo de Partidas aceitas nesta execução (`None`: sem limite).
    pub max_games: Option<u32>,
    /// Resposta à proposta de empate do adversário (a Engine nunca propõe).
    pub draw: DrawResponse,
    /// Tempo de pensar quando a Partida não tem relógio.
    pub movetime: Duration,
    /// Elo pedido à Engine (a mesma força da Engine adversária local); `None`: força total.
    pub elo: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            policy: ChallengePolicy::default(),
            max_games: Some(5),
            draw: DrawResponse::Decline,
            movetime: Duration::from_millis(1000),
            elo: None,
        }
    }
}

/// Cria a Engine de uma Partida (os testes trocam por uma roteirizada).
pub type EngineFactory = Box<dyn FnMut(&EngineSpec) -> Result<Box<dyn MoveEngine>, String> + Send>;

/// A força pedida: Elo simples como na Engine adversária local (`UCI_LimitStrength`/`UCI_Elo`,
/// ou `Skill Level`), ou força total.
pub fn strength_of(elo: Option<u32>) -> Strength {
    elo.map_or(Strength::Full, |elo| {
        Strength::Elo(elo.clamp(crate::engine::MIN_ELO, crate::engine::MAX_ELO))
    })
}

/// A fábrica real: um processo UCI da Engine escolhida, na força do usuário (total ou Elo).
pub fn uci_factory(elo: Option<u32>) -> EngineFactory {
    let strength = strength_of(elo);
    Box::new(move |spec| {
        let config = spec
            .options
            .iter()
            .fold(EngineConfig::new(&spec.path), |c, (name, value)| {
                c.option(name, value)
            });
        Ok(Box::new(EngineController::start(config, strength)))
    })
}

/// Por que o serviço não pôde ligar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// Não há Conexão de Engine autorizada: a Central de Conexões resolve.
    NoConnection,
    /// O Lichess está desligado para o Perfil local.
    ProviderOff,
    /// Já está ligada.
    AlreadyRunning,
    Bot(BoardError),
    /// A Trava de fair play está ligada: a Engine online calcula (FP-04).
    FairPlay(FairPlayLocked),
}

/// Em que pé está a Engine online.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Nada em execução; nada é pedido ao Lichess.
    Stopped,
    /// Ligada, ouvindo desafios, sem Partida.
    Waiting,
    /// Jogando ao menos uma Partida.
    Playing,
    /// Não aceita mais desafios; termina as Partidas em andamento.
    Stopping,
}

/// Por que parou (ou está parando).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// O usuário pediu e as Partidas terminam sozinhas.
    User,
    /// O usuário pediu para abandonar as Partidas em andamento.
    UserNow,
    /// O limite de Partidas desta execução foi atingido.
    LimitReached,
    /// O Lichess parou o fluxo (autorização revogada, Provedor desligado).
    ConnectionLost(ProviderError),
}

/// O que a tela conta (histórico curto, o mais novo por último).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    Accepted {
        name: String,
    },
    Declined {
        name: String,
        reason: DeclineReason,
    },
    /// Uma Partida que já estava no banco continua de onde parou (app religado).
    GameResumed {
        opponent: String,
    },
    GameStarted {
        opponent: String,
    },
    GameEnded {
        opponent: String,
        result: Option<String>,
    },
    /// O Lichess recusou o aceite do desafio (a conta pode não ser BOT).
    AcceptFailed {
        name: String,
        error: ProviderError,
    },
    /// A Engine propôs um Lance ilegal; o núcleo de regras o recusou e nada foi enviado.
    EngineMoveRejected,
    /// A Engine falhou; a Partida é abandonada (política padrão).
    EngineFailed {
        detail: String,
    },
    /// A Engine nem subiu para uma Partida que o Lichess abriu; ela é abandonada.
    EngineStartFailed {
        detail: String,
    },
    RequestFailed(ProviderError),
    SessionClosed(BoardError),
    StoreFailed(String),
    /// Trava de fair play (FP-04): o desafio foi recusado ("mais tarde").
    FairPlayDeclined {
        name: String,
    },
    /// Trava de fair play (FP-04): a Partida começou numa Posição que a Trava não deixa; a
    /// Engine não recebe a Posição e a Partida é abandonada.
    FairPlayResigned {
        opponent: String,
    },
}

/// Uma Partida da Engine em andamento, para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRow {
    pub id: String,
    pub opponent: String,
    pub rating: Option<u32>,
    pub white: bool,
    pub plies: usize,
    pub rated: bool,
    pub speed: String,
    pub engine_thinking: bool,
    pub link_lost: bool,
    pub over: bool,
}

/// O resultado da conversão da conta em BOT (irreversível; só com confirmação).
#[derive(Debug)]
pub enum Conversion {
    Sending(Query<()>),
    Done,
    Failed,
}

struct BotGame {
    session: OnlineGameSession,
    store_id: Option<GameId>,
    opponent: String,
    rating: Option<u32>,
    color_white: bool,
    rated: bool,
    speed: String,
    /// "Parar já" pedido: o abandono sai assim que a Partida existir.
    resign_wanted: bool,
}

struct Answering {
    id: String,
    name: String,
    decision: ChallengeDecision,
    request: Query<()>,
}

/// A Engine que não subiu: falha na primeira busca, e a política abandona a Partida em vez de
/// deixar o relógio correr.
struct DeadEngine {
    message: String,
    queue: VecDeque<ControllerEvent>,
    next: u64,
}

impl MoveEngine for DeadEngine {
    fn search(&mut self, _request: chess_uci::SearchRequest) -> u64 {
        let id = self.next;
        self.next += 1;
        self.queue.push_back(ControllerEvent::Failed {
            id: Some(id),
            failure: Failure::new(FailureKind::Spawn, self.message.clone()),
        });
        id
    }

    fn cancel(&mut self) {
        self.queue.clear();
    }

    fn poll(&mut self) -> Option<ControllerEvent> {
        self.queue.pop_front()
    }
}

/// O serviço da Engine online.
#[derive(Default)]
pub struct EngineOnline {
    bot: Option<LichessBot>,
    events: Option<ConnectionEvents>,
    active: bool,
    stopping: Option<StopReason>,
    spec: Option<EngineSpec>,
    settings: Settings,
    factory: Option<EngineFactory>,
    games: Vec<BotGame>,
    /// Desafios aceitos que esperam o `gameStart`: `(id, instante do aceite)`.
    accepted: Vec<(String, Timestamp)>,
    answering: Vec<Answering>,
    seen: HashSet<String>,
    accepted_total: u32,
    finished_total: u32,
    declined_total: u32,
    activity: VecDeque<Activity>,
    conversion: Option<Conversion>,
    clock: Option<std::sync::Arc<dyn Clock>>,
    /// O ponto único da Trava de fair play (o da rede da Conexão), desde que ligou.
    fair: Option<FairPlay>,
}

impl std::fmt::Debug for EngineOnline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineOnline")
            .field("state", &self.state())
            .field("games", &self.games.len())
            .finish_non_exhaustive()
    }
}

/// A Partida da Engine online ainda sem fim no banco para o `remote_id`, com o log carregado.
fn stored_game(
    access: &mut StoreAccess<'_>,
    remote_id: &str,
) -> Option<(GameId, chess_game::Game)> {
    let query = chess_store::GameQuery {
        only_unfinished: true,
        profile: Some(access.profile),
        limit: None,
    };
    let found = access
        .store
        .list_games(&query)
        .ok()?
        .into_iter()
        .find(|g| {
            let online = &g.metadata["online"];
            online["provider"] == BOT_KEY && online["remote_id"] == remote_id
        })?;
    let game = access.store.load_game(found.id).ok()?;
    Some((found.id, game))
}

fn metadata(remote_id: &str, at: Timestamp) -> GameMetadata {
    GameMetadata {
        title: Some("Partida da Engine online no Lichess".to_owned()),
        extra: json!({
            "online": {
                "provider": BOT_KEY,
                "remote_id": remote_id,
                "url": format!("https://lichess.org/{remote_id}"),
                "license": "CC0-1.0",
                "mode": "bot-api",
                "queried_at_ms": at.as_millis(),
            }
        }),
    }
}

impl EngineOnline {
    // --- Estado ---------------------------------------------------------------------------

    pub fn state(&self) -> RunState {
        if !self.active {
            RunState::Stopped
        } else if self.stopping.is_some() {
            RunState::Stopping
        } else if self.games.is_empty() {
            RunState::Waiting
        } else {
            RunState::Playing
        }
    }

    pub fn is_running(&self) -> bool {
        self.active
    }

    pub fn stop_reason(&self) -> Option<&StopReason> {
        self.stopping.as_ref()
    }

    pub fn engine(&self) -> Option<&EngineSpec> {
        self.spec.as_ref()
    }

    /// Partidas aceitas, recusadas e terminadas nesta execução.
    pub fn counts(&self) -> (u32, u32, u32) {
        (
            self.accepted_total,
            self.declined_total,
            self.finished_total,
        )
    }

    pub fn activity(&self) -> impl Iterator<Item = &Activity> {
        self.activity.iter()
    }

    pub fn conversion(&self) -> Option<&Conversion> {
        self.conversion.as_ref()
    }

    /// Quantas Partidas ainda não terminaram (as que o app teria de abandonar ao sair).
    pub fn games_in_progress(&self) -> usize {
        self.games
            .iter()
            .filter(|g| !g.session.game().is_some_and(|x| x.snapshot().is_over()))
            .count()
    }

    /// As Partidas em andamento, para a tela.
    pub fn rows(&self) -> Vec<GameRow> {
        self.games
            .iter()
            .map(|g| GameRow {
                id: g.session.game_id().to_owned(),
                opponent: g.opponent.clone(),
                rating: g.rating,
                white: g.color_white,
                plies: g.session.game().map_or(0, |x| x.snapshot().history.len()),
                rated: g.rated,
                speed: g.speed.clone(),
                engine_thinking: g.session.engine_thinking(),
                link_lost: matches!(g.session.link(), LinkStatus::Retrying { .. }),
                over: g.session.game().is_some_and(|x| x.snapshot().is_over()),
            })
            .collect()
    }

    fn now(&self) -> Timestamp {
        self.clock
            .as_ref()
            .map_or_else(NetService::now, |c| c.now())
    }

    fn note(&mut self, activity: Activity) {
        self.activity.push_back(activity);
        while self.activity.len() > ACTIVITY_CAP {
            self.activity.pop_front();
        }
    }

    // --- Ligar e parar --------------------------------------------------------------------

    /// Liga a Engine online: abre o fluxo de eventos da Conexão de Engine. Só por ação do
    /// usuário. Antes de qualquer pedido confere a Conexão e o Provedor.
    pub fn start(
        &mut self,
        net: &NetService,
        spec: EngineSpec,
        settings: Settings,
        factory: EngineFactory,
    ) -> Result<(), Unavailable> {
        if self.active {
            return Err(Unavailable::AlreadyRunning);
        }
        // FP-04: a Engine online calcula; com tudo travado não liga, antes de qualquer pedido.
        let fair = net.fair_play().clone();
        fair.check_anywhere(Computation::OnlineEngine)
            .map_err(Unavailable::FairPlay)?;
        let bot = self.bot_for(net)?.challenge_policy(settings.policy.clone());
        self.events = Some(bot.events());
        self.bot = Some(bot);
        self.clock = Some(net.clock());
        self.fair = Some(fair);
        self.spec = Some(spec);
        self.settings = settings;
        self.factory = Some(factory);
        self.active = true;
        self.stopping = None;
        self.accepted_total = 0;
        self.finished_total = 0;
        self.declined_total = 0;
        self.seen.clear();
        Ok(())
    }

    fn bot_for(&self, net: &NetService) -> Result<LichessBot, Unavailable> {
        let id = net
            .authorized_connection(ProviderId::Lichess, StoreRole::Engine)
            .ok_or(Unavailable::NoConnection)?;
        if !net.is_enabled(ProviderId::Lichess) {
            return Err(Unavailable::ProviderOff);
        }
        LichessBot::new(net.net().clone(), net.clock(), id, ConnectionRole::Engine)
            .map_err(Unavailable::Bot)
    }

    /// Para: não aceita mais desafios e espera as Partidas em andamento terminarem.
    pub fn stop(&mut self) {
        self.begin_stop(StopReason::User);
    }

    /// Para já: abandona as Partidas em andamento (o usuário confirmou).
    pub fn stop_now(&mut self) {
        self.begin_stop(StopReason::UserNow);
        for game in &mut self.games {
            // O pedido sai no próximo quadro (e só quando a Partida já existe).
            game.resign_wanted = true;
        }
    }

    fn begin_stop(&mut self, reason: StopReason) {
        if self.active && self.stopping.is_none() {
            self.stopping = Some(reason);
        }
    }

    /// Converte a conta da Conexão de Engine em conta BOT. **Irreversível**: só com a
    /// confirmação, que a tela cria depois de mostrar o aviso e o usuário confirmar.
    pub fn upgrade_to_bot(
        &mut self,
        net: &NetService,
        confirmed: IrreversibleConfirmation,
    ) -> Result<(), Unavailable> {
        if matches!(self.conversion, Some(Conversion::Sending(_))) {
            return Ok(());
        }
        let bot = self.bot_for(net)?;
        self.conversion = Some(Conversion::Sending(bot.upgrade_to_bot(confirmed)));
        Ok(())
    }

    // --- Um quadro ------------------------------------------------------------------------

    /// Um quadro: eventos, respostas, Partidas, banco. Nunca bloqueia.
    pub fn frame(&mut self, mut store: Option<StoreAccess<'_>>) {
        self.poll_conversion();
        if !self.active {
            return;
        }
        self.poll_events(&mut store);
        self.poll_answers();
        self.poll_games(&mut store);
        self.expire_accepted();
        self.settle();
    }

    fn poll_conversion(&mut self) {
        if let Some(Conversion::Sending(query)) = &mut self.conversion
            && let Some(result) = query.try_take()
        {
            self.conversion = Some(match result {
                Ok(_) => Conversion::Done,
                Err(_) => Conversion::Failed,
            });
        }
    }

    fn limit_reached(&self) -> bool {
        self.settings
            .max_games
            .is_some_and(|max| self.accepted_total >= max)
    }

    fn poll_events(&mut self, store: &mut Option<StoreAccess<'_>>) {
        let Some(events) = &mut self.events else {
            return;
        };
        let polled = events.poll();
        for event in polled {
            match event {
                ConnectionEvent::Challenge(challenge) => self.on_challenge(&challenge),
                ConnectionEvent::GameStarted(info) => self.on_game_started(info, store),
                ConnectionEvent::ChallengeCanceled { id } => {
                    self.accepted.retain(|(accepted, _)| *accepted != id);
                }
                ConnectionEvent::GameFinished(_) | ConnectionEvent::ChallengeDeclined { .. } => {}
                ConnectionEvent::Closed(error) => {
                    self.events = None;
                    self.begin_stop(StopReason::ConnectionLost(error));
                    break;
                }
            }
        }
    }

    fn on_challenge(&mut self, challenge: &Challenge) {
        let Some(bot) = self.bot.clone() else {
            return;
        };
        if !self.seen.insert(challenge.id.clone()) {
            return;
        }
        // FP-04: a Partida começaria numa Posição que a Trava de fair play não deixa (o desafio
        // "a partir de uma Posição" não diz qual: com qualquer trava ligada, fica de fora).
        let start =
            (challenge.variant != "fromPosition").then(chess_core::CanonicalPosition::start);
        let locked = self.fair.as_ref().is_none_or(|fair| {
            fair.check_start(Computation::OnlineEngine, start.as_ref())
                .is_err()
        });
        // Parando, no limite ou sob a Trava: nenhum desafio é aceito. `max_concurrent = 0` faz
        // a política recusar com "mais tarde", sem inventar outro caminho de recusa.
        let blocked = self.stopping.is_some() || self.limit_reached() || locked;
        let bot = if blocked {
            let mut closed = self.settings.policy.clone();
            closed.max_concurrent = 0;
            bot.challenge_policy(closed)
        } else {
            bot
        };
        let active = self.games.len() + self.accepted.len();
        let ChallengeAnswer { decision, request } = bot.answer_challenge(challenge, active);
        let name = challenge.challenger.name.clone();
        match decision {
            ChallengeDecision::Accept => {
                self.accepted_total += 1;
                self.accepted.push((challenge.id.clone(), self.now()));
                self.note(Activity::Accepted { name: name.clone() });
                if self.limit_reached() {
                    self.begin_stop(StopReason::LimitReached);
                }
            }
            ChallengeDecision::Decline(_) if locked => {
                self.declined_total += 1;
                self.note(Activity::FairPlayDeclined { name: name.clone() });
            }
            ChallengeDecision::Decline(reason) => {
                self.declined_total += 1;
                self.note(Activity::Declined {
                    name: name.clone(),
                    reason,
                });
            }
        }
        // O pedido fica vivo até a resposta: descartar a consulta o cancelaria.
        self.answering.push(Answering {
            id: challenge.id.clone(),
            name,
            decision,
            request,
        });
    }

    fn on_game_started(&mut self, info: GameInfo, store: &mut Option<StoreAccess<'_>>) {
        self.accepted.retain(|(id, _)| *id != info.game_id);
        if self
            .games
            .iter()
            .any(|g| g.session.game_id() == info.game_id)
        {
            return;
        }
        if info.variant != "standard" && info.variant != "fromPosition" {
            return;
        }
        let (Some(bot), Some(spec), Some(fair)) =
            (self.bot.clone(), self.spec.clone(), self.fair.clone())
        else {
            return;
        };
        let policy = EnginePolicy {
            draw: self.settings.draw,
            resign_if_engine_fails: true,
            movetime: self.settings.movetime,
        };
        // Partida que já estava no banco (o app fechou e religou no meio dela): continua no
        // mesmo log, nunca uma segunda linha na Biblioteca. O Lichess reenvia `gameStart`
        // das Partidas em andamento ao abrir o fluxo de eventos.
        let stored = store
            .as_mut()
            .and_then(|access| stored_game(access, &info.game_id).map(|(id, g)| (access, id, g)));
        let mut resumed = None;
        if let Some((access, id, game)) = stored {
            // FP-04: a Partida retomada obedece à Trava pela Posição em que está, com o caminho
            // até ela.
            let refused = fair.check_game(Computation::OnlineEngine, &game).err();
            let engine = self.make_engine(&spec, &fair, refused);
            if let Ok(session) = bot.resume_game(game, engine, policy) {
                access.store.resume_game(id).ok();
                resumed = Some((id, session, refused));
            }
        }
        let (session, store_id, refused) = if let Some((id, session, refused)) = resumed {
            self.note(Activity::GameResumed {
                opponent: info.opponent.name.clone(),
            });
            (session, Some(id), refused)
        } else {
            // FP-04: a Partida começa numa Posição que a Trava de fair play não deixa?
            let start = info
                .fen
                .as_deref()
                .and_then(|fen| chess_core::CanonicalPosition::from_fen(fen).ok());
            let refused = fair
                .check_start(Computation::OnlineEngine, start.as_ref())
                .err();
            let engine = self.make_engine(&spec, &fair, refused);
            self.note(Activity::GameStarted {
                opponent: info.opponent.name.clone(),
            });
            (bot.open_game(&info, engine, policy), None, refused)
        };
        if refused.is_some() {
            self.note(Activity::FairPlayResigned {
                opponent: info.opponent.name.clone(),
            });
        }
        self.games.push(BotGame {
            session,
            store_id,
            opponent: info.opponent.name.clone(),
            rating: info.opponent.rating,
            color_white: info.color == chess_core::Color::White,
            rated: info.rated,
            speed: info.speed.clone(),
            resign_wanted: self.stopping == Some(StopReason::UserNow),
        });
    }

    /// A Engine de uma Partida, sempre atrás da Trava de fair play (FP-04): cada busca pede
    /// licença e, recusada, a Engine não recebe a Posição e a política abandona a Partida.
    /// Recusada já ao começar (`refused`), a Engine nem sobe. Se ela não sobe, uma Engine morta
    /// que faz a política abandonar.
    fn make_engine(
        &mut self,
        spec: &EngineSpec,
        fair: &FairPlay,
        refused: Option<FairPlayLocked>,
    ) -> Box<dyn MoveEngine> {
        let engine: Box<dyn MoveEngine> = match refused {
            Some(locked) => Box::new(DeadEngine {
                message: locked.to_string(),
                queue: VecDeque::new(),
                next: 1,
            }),
            None => {
                let built = match self.factory.as_mut() {
                    Some(factory) => factory(spec),
                    None => Err("Nenhuma Engine escolhida.".to_owned()),
                };
                match built {
                    Ok(engine) => engine,
                    Err(detail) => {
                        self.note(Activity::EngineStartFailed {
                            detail: detail.clone(),
                        });
                        Box::new(DeadEngine {
                            message: detail,
                            queue: VecDeque::new(),
                            next: 1,
                        })
                    }
                }
            }
        };
        Box::new(GuardedMoveEngine::new(fair.clone(), engine))
    }

    fn poll_answers(&mut self) {
        let mut failed = Vec::new();
        self.answering.retain_mut(|a| match a.request.try_take() {
            None => true,
            Some(Ok(_)) => false,
            Some(Err(error)) => {
                failed.push((a.id.clone(), a.name.clone(), a.decision, error));
                false
            }
        });
        for (id, name, decision, error) in failed {
            if decision == ChallengeDecision::Accept {
                // O aceite não passou: não há Partida a esperar.
                self.accepted.retain(|(accepted, _)| *accepted != id);
                self.accepted_total = self.accepted_total.saturating_sub(1);
                self.note(Activity::AcceptFailed { name, error });
            } else {
                self.note(Activity::RequestFailed(error));
            }
        }
    }

    fn poll_games(&mut self, store: &mut Option<StoreAccess<'_>>) {
        let now = self.now();
        let mut notes = Vec::new();
        for game in &mut self.games {
            if game.resign_wanted {
                // `Err` com a Partida já criada: o pedido já saiu ou a Partida terminou.
                let sent = game.session.resign();
                if sent.is_ok() || game.session.game().is_some() {
                    game.resign_wanted = false;
                }
            }
            let polled = game.session.poll();
            let remote = game.session.game_id().to_owned();
            if polled.created
                && let (Some(state), Some(access)) = (game.session.game(), store.as_mut())
            {
                match access.store.create_game_for(
                    access.profile,
                    state.setup(),
                    &metadata(&remote, now),
                ) {
                    Ok(id) => game.store_id = Some(id),
                    Err(error) => notes.push(Activity::StoreFailed(error.to_string())),
                }
            }
            // Só o que o Provedor confirmou: exatamente os Eventos do controlador.
            if !polled.events.is_empty()
                && let (Some(id), Some(access)) = (game.store_id, store.as_mut())
                && let Err(error) = access.store.append_events(id, &polled.events)
            {
                notes.push(Activity::StoreFailed(error.to_string()));
            }
            for notice in polled.notices {
                match notice {
                    SessionNotice::EngineMoveRejected { .. } => {
                        notes.push(Activity::EngineMoveRejected);
                    }
                    SessionNotice::EngineFailed { detail } => {
                        notes.push(Activity::EngineFailed { detail });
                    }
                    SessionNotice::Closed(error) => notes.push(Activity::SessionClosed(error)),
                    SessionNotice::RequestFailed { error, .. }
                    | SessionNotice::MoveRefused { error, .. } => {
                        notes.push(Activity::RequestFailed(error));
                    }
                    _ => {}
                }
            }
        }
        for note in notes {
            self.note(note);
        }
        // Fluxo fechado (Partida terminada, autorização revogada, Partida inexistente): a
        // sessão não tem mais o que fazer e sai da lista; só a terminada conta como jogada.
        let mut ended = Vec::new();
        self.games.retain(|g| {
            let closed = g.session.link() == LinkStatus::Closed;
            if closed {
                let snapshot = g.session.game().map(chess_game::Game::snapshot);
                let over = snapshot.as_ref().is_some_and(|s| s.is_over());
                let result = snapshot
                    .and_then(|s| s.outcome)
                    .map(|o| o.result.pgn_token().to_owned());
                if over {
                    ended.push((g.opponent.clone(), result));
                }
            }
            !closed
        });
        for (opponent, result) in ended {
            self.finished_total += 1;
            self.note(Activity::GameEnded { opponent, result });
        }
    }

    fn expire_accepted(&mut self) {
        let now = self.now().as_millis();
        self.accepted
            .retain(|(_, at)| now.saturating_sub(at.as_millis()) < ACCEPTED_TTL_MS);
    }

    /// Fecha quando está parando e não resta nada a esperar.
    fn settle(&mut self) {
        if self.stopping.is_some()
            && self.games.is_empty()
            && self.accepted.is_empty()
            && self.answering.is_empty()
        {
            self.events = None;
            self.bot = None;
            self.factory = None;
            self.active = false;
        }
    }
}
