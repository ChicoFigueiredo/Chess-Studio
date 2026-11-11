//! O serviço da Partida online no app (#233, ADR-0007), sem Bevy nem egui: a Conexão de
//! jogador, o fluxo de eventos (desafios, começo e fim), buscar adversário, desafiar, a Engine
//! do Lichess, as Partidas em andamento e as sessões de cada Partida.
//!
//! - **Autoridade.** Quem escreve a Partida é a `OnlineGameSession` do `chess-providers`
//!   (o controlador, só com o que o Provedor confirmou). Este serviço grava no banco
//!   exatamente os Eventos que ela devolve e a `Session` do tabuleiro só os espelha.
//! - **Nunca bloqueia.** Tudo é consulta por quadro ([`OnlinePlay::frame`]); nenhum pedido
//!   espera a rede.
//! - **Envio único.** O Lance sai uma vez pela sessão; nada aqui reenvia.
//! - **Trava de fair play.** A sessão de uma Partida contra pessoa segura a trava da rede;
//!   [`OnlinePlay::locked_games`] dá ao ponto único do app (`FairPlay`) as Partidas em
//!   andamento, inclusive as do banco sem sessão (correspondência, app reaberto).
//! - **Correspondência.** A sessão de uma Partida por correspondência só fica aberta enquanto
//!   ela está no tabuleiro; fora dele, a trava vale para ela e as Posições dela (PO-13).
//! - **Conexão.** Só a Conexão de **jogador** autorizada (`NetService::authorized_connection`)
//!   joga; a de Engine nunca.

use crate::net_service::NetService;
use chess_core::{Color, Move};
use chess_game::Game;
use chess_net::{Clock, ConnectionId, ProviderId, Timestamp};
use chess_providers::{
    BoardAction, BoardError, Challenge, ChallengeSpec, ConnectionEvent, ConnectionEvents,
    ConnectionRole, DeclineReason, GameInfo, LichessBoard, LinkStatus, OnlineGameSession,
    OnlineTime, ProviderError, Query, Seek, SeekState, Seeking, SessionNotice,
};
use chess_store::{ConnectionRole as StoreRole, GameId, GameMetadata, GameQuery, ProfileId, Store};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Chave do Provedor no `GameSetup` das Partidas do Lichess.
pub const LICHESS_KEY: &str = "lichess";

/// Por que um pedido online não pôde nem começar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// Não há Conexão de jogador autorizada: a Central de Conexões resolve.
    NoConnection,
    /// O Lichess está desligado para o Perfil local.
    ProviderOff,
    /// A Board API recusou antes da rede (ritmo, busca em andamento…).
    Board(BoardError),
}

/// O que a interface conta ao jogador (avisos de vida curta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnlineNotice {
    /// Uma Partida online começou.
    GameStarted { remote_id: String },
    /// Chegou um desafio.
    ChallengeReceived { id: String },
    /// O desafiado recusou.
    ChallengeDeclined { name: String },
    /// Um pedido (desafio, busca, Engine do Lichess, lista) falhou.
    Failed(ProviderError),
    /// O Lichess recusou o Lance enviado: a peça volta.
    MoveRefused { mv: Move },
    /// O envio ficou sem resposta e o Lichess não registrou o Lance: jogue de novo.
    MoveLost { mv: Move },
    /// O Lichess registrou outro Lance no lugar do enviado.
    MoveReplaced { mv: Move },
    /// Um pedido da Partida (empate, abandono…) falhou; nada mudou.
    RequestFailed { action: BoardAction },
    /// A Partida local e a do Lichess não conferem; nada foi reescrito.
    Desynced,
    /// A sessão de uma Partida parou (variante, autorização revogada, Partida inexistente).
    SessionClosed(BoardError),
    /// Uma Partida em variante começou no Lichess e foi ignorada: o app só joga xadrez padrão.
    VariantIgnored { variant: String },
    /// A busca de adversário terminou sem Partida.
    SeekEnded,
    /// O fluxo de eventos parou de vez (autorização revogada, Provedor desligado).
    EventsClosed(ProviderError),
    /// Falha ao gravar no banco.
    StoreFailed(String),
}

/// Busca de adversário em andamento.
#[derive(Debug)]
pub struct SeekRun {
    seeking: Seeking,
    pub seek: Seek,
    pub since: Timestamp,
    /// Correspondência publicada (fica no Lichess até alguém aceitar).
    pub posted: bool,
}

/// O desafio por nome que o jogador fez.
#[derive(Debug)]
pub struct Outgoing {
    pub name: String,
    pub spec: ChallengeSpec,
    query: Option<Query<Challenge>>,
    pub id: Option<String>,
    pub state: OutgoingState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutgoingState {
    Sending,
    Waiting,
    Declined,
    Canceled,
    Failed(ProviderError),
}

/// Pedido sem resposta útil (aceitar, recusar, cancelar desafio): fica vivo até a resposta,
/// porque descartar a consulta cancelaria o pedido.
#[derive(Debug)]
struct Request {
    what: RequestKind,
    query: Query<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Accept,
    Decline,
    Cancel,
}

/// Pedido de Partida contra a Engine do Lichess.
#[derive(Debug)]
struct AiRun {
    query: Option<Query<String>>,
    game_id: Option<String>,
}

/// Uma Partida online com sessão aberta.
pub struct Live {
    session: OnlineGameSession,
    pub store_id: Option<GameId>,
    pub realtime: bool,
    pub info: Option<GameInfo>,
    /// Aberta só para buscar o fim (terminou enquanto o app não olhava): fica até o retrato.
    fetching_end: bool,
}

impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Live")
            .field("game", &self.session.game_id())
            .field("store_id", &self.store_id)
            .finish_non_exhaustive()
    }
}

impl Live {
    pub fn remote_id(&self) -> &str {
        self.session.game_id()
    }

    pub fn game(&self) -> Option<&Game> {
        self.session.game()
    }

    pub fn link(&self) -> LinkStatus {
        self.session.link()
    }

    /// O adversário saiu e o prazo do Lichess para reclamar a vitória já passou.
    pub fn can_claim_victory(&self, now: Timestamp) -> bool {
        self.session
            .opponent_gone()
            .and_then(|g| g.claim_from)
            .is_some_and(|from| now >= from)
    }

    /// O adversário saiu (o Lichess avisou).
    pub fn opponent_gone(&self) -> bool {
        self.session.opponent_gone().is_some()
    }

    fn over(&self) -> bool {
        self.game().is_some_and(|g| g.snapshot().is_over())
    }
}

/// Uma linha de "Partidas em andamento".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayingRow {
    pub remote_id: String,
    pub opponent: String,
    pub rating: Option<u32>,
    pub color: Color,
    /// É a vez do jogador local.
    pub my_turn: bool,
    /// Segundos para o prazo do Lance (do Lichess), quando conhecido.
    pub seconds_left: Option<u64>,
    /// Dias por Lance (correspondência).
    pub days: Option<u32>,
    /// Já está no banco local.
    pub stored: bool,
}

/// O banco visto pelo serviço: as Partidas online em andamento e o id de cada uma.
#[derive(Debug, Default)]
struct DbIndex {
    by_remote: BTreeMap<String, GameId>,
    in_progress: Vec<(GameId, Game)>,
    dirty: bool,
}

/// Onde o serviço grava (o banco do Perfil local; nada nas capturas sem banco).
pub struct StoreAccess<'a> {
    pub store: &'a mut Store,
    pub profile: ProfileId,
}

/// O que um quadro mudou.
#[derive(Debug, Default)]
pub struct Frame {
    /// A Partida a pôr no tabuleiro agora (começou, ou o jogador pediu para abrir).
    pub show: Option<String>,
}

/// O serviço inteiro. Um por app.
#[derive(Default)]
pub struct OnlinePlay {
    board: Option<LichessBoard>,
    connection: Option<ConnectionId>,
    clock: Option<Arc<dyn Clock>>,
    events: Option<ConnectionEvents>,
    seek: Option<SeekRun>,
    ai: Option<AiRun>,
    pub outgoing: Vec<Outgoing>,
    pub incoming: Vec<Challenge>,
    playing: Option<Query<Vec<GameInfo>>>,
    /// A última lista de Partidas em andamento do Lichess (Atualizar).
    remote: Vec<GameInfo>,
    remote_at: Option<Timestamp>,
    games: Vec<Live>,
    /// A Partida que deve ir ao tabuleiro assim que existir (começou ou foi aberta).
    to_show: Option<String>,
    notices: Vec<OnlineNotice>,
    db: DbIndex,
    /// Quem espera um `gameStart`: a busca, a Engine do Lichess ou um desafio feito.
    expecting_start: bool,
    requests: Vec<Request>,
}

impl std::fmt::Debug for OnlinePlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnlinePlay")
            .field("connection", &self.connection)
            .field("games", &self.games)
            .finish_non_exhaustive()
    }
}

fn metadata(game: &Game, at: Timestamp) -> GameMetadata {
    let online = game.setup().online_game();
    let remote = online.map(|o| o.remote_id.clone()).unwrap_or_default();
    GameMetadata {
        title: Some("Partida online no Lichess".to_owned()),
        extra: json!({
            "online": {
                "provider": LICHESS_KEY,
                "remote_id": remote,
                "url": format!("https://lichess.org/{remote}"),
                "license": "CC0-1.0",
                "mode": "api",
                "queried_at_ms": at.as_millis(),
            }
        }),
    }
}

/// A Partida do banco é online (gravada por este serviço)?
fn stored_remote_id(metadata: &serde_json::Value) -> Option<String> {
    let online = metadata.get("online")?;
    (online.get("provider")?.as_str()? == LICHESS_KEY)
        .then(|| online.get("remote_id")?.as_str().map(str::to_owned))
        .flatten()
}

impl OnlinePlay {
    pub fn new() -> Self {
        Self {
            db: DbIndex {
                dirty: true,
                ..DbIndex::default()
            },
            ..Self::default()
        }
    }

    // --- Conexão --------------------------------------------------------------------------

    /// Acompanha a Conexão de jogador autorizada do Perfil local. Sem ela (ou outra), o
    /// cliente, o fluxo de eventos e as buscas caem; as sessões de Partidas já abertas
    /// seguem até o fim (a Partida não some por trocar a Central).
    pub fn follow_connection(&mut self, net: &NetService) {
        let now = net.authorized_connection(ProviderId::Lichess, StoreRole::Player);
        if now == self.connection && (now.is_none() || self.board.is_some()) {
            return;
        }
        self.connection = now;
        self.events = None;
        self.seek = None;
        self.ai = None;
        self.outgoing.clear();
        self.incoming.clear();
        self.clock = Some(net.clock());
        self.board = now.and_then(|id| {
            LichessBoard::new(net.net().clone(), net.clock(), id, ConnectionRole::Player).ok()
        });
    }

    /// Há Conexão de jogador autorizada?
    pub fn connected(&self) -> bool {
        self.board.is_some()
    }

    /// O fluxo de eventos (desafios recebidos, começo e fim) está aberto.
    pub fn listening(&self) -> bool {
        self.events.is_some()
    }

    fn now(&self) -> Timestamp {
        self.clock
            .as_ref()
            .map_or_else(NetService::now, |c| c.now())
    }

    fn ready(&self, net: &NetService) -> Result<LichessBoard, Unavailable> {
        let board = self.board.clone().ok_or(Unavailable::NoConnection)?;
        if !net.is_enabled(ProviderId::Lichess) {
            return Err(Unavailable::ProviderOff);
        }
        Ok(board)
    }

    /// Abre o fluxo de eventos da Conexão (só por ação do jogador: buscar, desafiar,
    /// atualizar). Antes de buscar ou desafiar, é por ele que vem o começo da Partida.
    pub fn listen(&mut self, net: &NetService) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        if self.events.is_none() {
            self.events = Some(board.events());
        }
        Ok(())
    }

    // --- Começar ------------------------------------------------------------------------

    /// Busca adversário (tempo real ou correspondência). Uma por vez.
    pub fn seek(&mut self, net: &NetService, seek: Seek) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        // A Board API recusa ritmo rápido demais antes da rede; só então o fluxo de eventos
        // abre (é por ele que o começo da Partida chega).
        let seeking = board.seek(&seek).map_err(Unavailable::Board)?;
        self.listen(net)?;
        self.seek = Some(SeekRun {
            seeking,
            posted: matches!(seek.time, OnlineTime::Correspondence { .. }),
            seek,
            since: self.now(),
        });
        self.expecting_start = true;
        Ok(())
    }

    /// Cancela a busca em tempo real (o Lichess a fecha com a conexão).
    pub fn cancel_seek(&mut self) {
        if let Some(run) = self.seek.take() {
            run.seeking.cancel();
        }
        self.expecting_start = self.ai.is_some() || self.outgoing_waiting();
    }

    pub fn seek_run(&self) -> Option<&SeekRun> {
        self.seek.as_ref()
    }

    /// Partida contra a Engine do Lichess (nível 1 a 8): começa na hora; sem Trava.
    pub fn play_engine(
        &mut self,
        net: &NetService,
        level: u8,
        spec: ChallengeSpec,
    ) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        self.listen(net)?;
        self.ai = Some(AiRun {
            query: Some(board.challenge_engine(level, &spec)),
            game_id: None,
        });
        self.expecting_start = true;
        Ok(())
    }

    /// Esperando a Engine do Lichess aceitar.
    pub fn engine_pending(&self) -> bool {
        self.ai.is_some()
    }

    /// Desafia um jogador pelo nome (inclusive blitz). O começo vem pelo fluxo de eventos.
    pub fn challenge(
        &mut self,
        net: &NetService,
        name: &str,
        spec: ChallengeSpec,
    ) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        self.listen(net)?;
        let name = name.trim().to_owned();
        self.outgoing.retain(|o| {
            !matches!(
                o.state,
                OutgoingState::Declined | OutgoingState::Canceled | OutgoingState::Failed(_)
            )
        });
        self.outgoing.push(Outgoing {
            query: Some(board.challenge(&name, &spec)),
            name,
            spec,
            id: None,
            state: OutgoingState::Sending,
        });
        self.expecting_start = true;
        Ok(())
    }

    /// Cancela um desafio feito.
    pub fn cancel_challenge(&mut self, index: usize) {
        let Some(out) = self.outgoing.get_mut(index) else {
            return;
        };
        if let (Some(id), Some(board)) = (&out.id, &self.board) {
            self.requests.push(Request {
                what: RequestKind::Cancel,
                query: board.cancel_challenge(id),
            });
        }
        out.query = None;
        out.state = OutgoingState::Canceled;
        self.expecting_start = self.seek.is_some() || self.ai.is_some() || self.outgoing_waiting();
    }

    fn outgoing_waiting(&self) -> bool {
        self.outgoing
            .iter()
            .any(|o| matches!(o.state, OutgoingState::Sending | OutgoingState::Waiting))
    }

    /// Aceita um desafio recebido. Variante não padrão é recusada antes da rede.
    pub fn accept(&mut self, net: &NetService, id: &str) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        self.listen(net)?;
        let Some(index) = self.incoming.iter().position(|c| c.id == id) else {
            return Ok(());
        };
        let challenge = self.incoming.remove(index);
        // A resposta chega sem bloquear; o começo vem pelo fluxo de eventos. Variante não
        // padrão: a consulta já nasce recusada, antes da rede.
        self.requests.push(Request {
            what: RequestKind::Accept,
            query: board.accept_challenge(&challenge),
        });
        self.expecting_start = true;
        Ok(())
    }

    /// Recusa um desafio recebido, com o motivo.
    pub fn decline(&mut self, id: &str, reason: DeclineReason) {
        let Some(index) = self.incoming.iter().position(|c| c.id == id) else {
            return;
        };
        let challenge = self.incoming.remove(index);
        if let Some(board) = &self.board {
            self.requests.push(Request {
                what: RequestKind::Decline,
                query: board.decline_challenge(&challenge.id, reason),
            });
        }
    }

    // --- Partidas em andamento ------------------------------------------------------------

    /// Pede ao Lichess a lista de Partidas em andamento (botão Atualizar; nunca sozinho).
    pub fn refresh_playing(&mut self, net: &NetService) -> Result<(), Unavailable> {
        let board = self.ready(net)?;
        self.listen(net)?;
        self.playing = Some(board.playing());
        Ok(())
    }

    pub fn refreshing(&self) -> bool {
        self.playing.is_some()
    }

    /// Quando a lista do Lichess foi lida pela última vez nesta sessão do app.
    pub fn refreshed_at(&self) -> Option<Timestamp> {
        self.remote_at
    }

    /// "Partidas em andamento": as do banco local e as que o Lichess listou, sem repetir.
    pub fn playing_rows(&self) -> Vec<PlayingRow> {
        let mut rows: BTreeMap<String, PlayingRow> = BTreeMap::new();
        for (_, game) in &self.db.in_progress {
            let Some(online) = game.setup().online_game() else {
                continue;
            };
            let snapshot = game.snapshot();
            let opponent = match online.local_color {
                Color::White => &online.black,
                Color::Black => &online.white,
            };
            rows.insert(
                online.remote_id.clone(),
                PlayingRow {
                    remote_id: online.remote_id.clone(),
                    opponent: opponent.name.clone(),
                    rating: opponent.rating,
                    color: online.local_color,
                    my_turn: snapshot.side_to_move == online.local_color,
                    seconds_left: None,
                    days: online.days_per_move,
                    stored: true,
                },
            );
        }
        for info in &self.remote {
            let row = rows
                .entry(info.game_id.clone())
                .or_insert_with(|| PlayingRow {
                    remote_id: info.game_id.clone(),
                    opponent: info.opponent.name.clone(),
                    rating: info.opponent.rating,
                    color: info.color,
                    my_turn: info.is_my_turn,
                    seconds_left: None,
                    days: None,
                    stored: false,
                });
            // O Lichess sabe melhor de quem é a vez e quanto falta.
            row.my_turn = info.is_my_turn;
            row.seconds_left = info.seconds_left;
        }
        // Na vez do jogador primeiro; depois quem tem menos tempo.
        let mut rows: Vec<PlayingRow> = rows.into_values().collect();
        rows.sort_by_key(|r| (!r.my_turn, r.seconds_left.unwrap_or(u64::MAX)));
        rows
    }

    /// Partidas online em andamento no banco (correspondência e as que o app não fechou).
    pub fn stored_in_progress(&self) -> usize {
        self.db.in_progress.len()
    }

    /// Abre uma Partida em andamento no tabuleiro: a do banco continua pelo retrato do
    /// Lichess; a que só existe lá nasce do primeiro retrato.
    pub fn open(&mut self, net: &NetService, remote_id: &str) -> Result<(), Unavailable> {
        if self.live(remote_id).is_some() {
            self.to_show = Some(remote_id.to_owned());
            return Ok(());
        }
        let board = self.ready(net)?;
        let live = match self.resume_stored(&board, remote_id) {
            Some(live) => live?,
            None => {
                let info = self
                    .remote
                    .iter()
                    .find(|i| i.game_id == remote_id)
                    .cloned()
                    .ok_or(Unavailable::Board(BoardError::WrongGame))?;
                self.start_session(&board, info)
            }
        };
        self.games.push(live);
        self.to_show = Some(remote_id.to_owned());
        Ok(())
    }

    /// A sessão de uma Partida que já está no banco: continua pelo retrato do Provedor, no
    /// mesmo log (nunca uma segunda linha).
    fn resume_stored(
        &self,
        board: &LichessBoard,
        remote_id: &str,
    ) -> Option<Result<Live, Unavailable>> {
        let (id, game) = self.db.in_progress.iter().find(|(_, g)| {
            g.setup()
                .online_game()
                .is_some_and(|o| o.remote_id == remote_id)
        })?;
        let realtime = game
            .setup()
            .online_game()
            .is_some_and(|o| o.days_per_move.is_none());
        Some(
            board
                .resume_game(game.clone())
                .map(|session| Live {
                    session,
                    store_id: Some(*id),
                    realtime,
                    info: None,
                    fetching_end: false,
                })
                .map_err(Unavailable::Board),
        )
    }

    /// A sessão de uma Partida que começou agora (ou que já estava no banco).
    fn start_session(&self, board: &LichessBoard, info: GameInfo) -> Live {
        if let Some(Ok(mut live)) = self.resume_stored(board, &info.game_id) {
            live.info = Some(info);
            return live;
        }
        Live {
            session: board.open_game(&info),
            store_id: None,
            realtime: info.speed != "correspondence",
            info: Some(info),
            fetching_end: false,
        }
    }

    /// Partida do banco que terminou no Lichess sem o app ver: abre a sessão só para buscar o
    /// retrato do fim (resultado no log, Trava solta). Nada vai ao tabuleiro.
    fn fetch_end(&mut self, remote_id: &str) {
        if self.live(remote_id).is_some() {
            return;
        }
        let Some(board) = self.board.clone() else {
            return;
        };
        if let Some(Ok(mut live)) = self.resume_stored(&board, remote_id) {
            live.fetching_end = true;
            self.games.push(live);
        }
    }

    // --- Durante a Partida ----------------------------------------------------------------

    pub fn live(&self, remote_id: &str) -> Option<&Live> {
        self.games.iter().find(|g| g.remote_id() == remote_id)
    }

    fn live_mut(&mut self, remote_id: &str) -> Option<&mut Live> {
        self.games.iter_mut().find(|g| g.remote_id() == remote_id)
    }

    #[cfg(test)]
    pub fn games(&self) -> &[Live] {
        &self.games
    }

    /// Há Partida online em tempo real contra pessoa em andamento (com sessão)?
    pub fn realtime_in_progress(&self) -> bool {
        self.games
            .iter()
            .any(|g| g.realtime && g.game().is_none_or(Game::online_human_in_progress) && !g.over())
    }

    /// Envia o que o jogador pediu na Partida `remote_id`. Nada muda até o retrato.
    pub fn apply(&mut self, remote_id: &str, intent: crate::session::OnlineIntent) {
        use crate::session::OnlineIntent as I;
        let now = self.now();
        let Some(live) = self.live_mut(remote_id) else {
            return;
        };
        let result = match intent {
            I::Play(mv) => live.session.play(mv),
            I::Resign => live.session.resign(),
            I::Abort => live.session.abort(),
            I::OfferDraw => live.session.offer_draw(),
            I::AcceptDraw => live.session.accept_draw(),
            I::DeclineDraw => live.session.decline_draw(),
            I::ClaimVictory if live.can_claim_victory(now) => live.session.claim_victory(),
            I::ClaimVictory => Err(BoardError::NotAllowed("o prazo do Lichess não chegou")),
        };
        if let Err(error) = result {
            let notice = match (intent, error) {
                (I::Play(mv), _) => OnlineNotice::MoveRefused { mv },
                (_, error) => OnlineNotice::SessionClosed(error),
            };
            self.notices.push(notice);
        }
    }

    /// A Partida no tabuleiro mudou: a sessão de uma correspondência que saiu dele fecha (a
    /// trava continua por ela, pelo banco); as de tempo real seguem até o fim.
    pub fn follow_board(&mut self, shown: Option<&str>) {
        let before = self.games.len();
        self.games.retain(|g| {
            g.realtime || g.fetching_end || Some(g.remote_id()) == shown || g.game().is_none()
        });
        if self.games.len() != before {
            self.db.dirty = true;
        }
    }

    // --- O quadro ------------------------------------------------------------------------

    /// Avisos desde a última consulta.
    pub fn take_notices(&mut self) -> Vec<OnlineNotice> {
        std::mem::take(&mut self.notices)
    }

    /// Recolhe o que chegou (eventos, buscas, desafios, sessões), grava no banco o que o
    /// Provedor confirmou e diz o que pôr no tabuleiro. Nunca bloqueia.
    pub fn frame(&mut self, mut store: Option<StoreAccess<'_>>) -> Frame {
        if self.db.dirty
            && let Some(access) = store.as_mut()
        {
            self.refresh_db(access.store, access.profile);
        }
        self.poll_events();
        self.poll_seek();
        self.poll_ai();
        self.poll_outgoing();
        self.poll_playing();
        self.poll_requests();
        self.poll_games(&mut store);
        let show = self.to_show.take_if(|id| {
            self.games
                .iter()
                .any(|g| g.remote_id() == id && g.game().is_some())
        });
        Frame { show }
    }

    fn refresh_db(&mut self, store: &mut Store, profile: ProfileId) {
        self.db.dirty = false;
        let query = GameQuery {
            only_unfinished: true,
            profile: Some(profile),
            limit: None,
        };
        let Ok(summaries) = store.list_games(&query) else {
            return;
        };
        self.db.in_progress.clear();
        for summary in summaries {
            let Some(remote) = stored_remote_id(&summary.metadata) else {
                continue;
            };
            self.db.by_remote.insert(remote, summary.id);
            if let Ok(game) = store.load_game(summary.id)
                && !game.snapshot().is_over()
            {
                self.db.in_progress.push((summary.id, game));
            }
        }
    }

    fn poll_events(&mut self) {
        let Some(events) = &mut self.events else {
            return;
        };
        let polled = events.poll();
        let mut finished = Vec::new();
        for event in polled {
            match event {
                ConnectionEvent::GameStarted(info) => self.on_game_started(info),
                ConnectionEvent::GameFinished(info) => {
                    // Partida do banco sem sessão: o retrato do fim vem pela sessão reaberta.
                    self.remote.retain(|g| g.game_id != info.game_id);
                    finished.push(info.game_id);
                }
                ConnectionEvent::Challenge(challenge) => {
                    // Desafio feito por este jogador também chega aqui: não é recebido.
                    let mine = self
                        .outgoing
                        .iter()
                        .any(|o| o.id.as_deref() == Some(&challenge.id));
                    if !mine && !self.incoming.iter().any(|c| c.id == challenge.id) {
                        self.notices.push(OnlineNotice::ChallengeReceived {
                            id: challenge.id.clone(),
                        });
                        self.incoming.push(challenge);
                    }
                }
                ConnectionEvent::ChallengeCanceled { id } => {
                    self.incoming.retain(|c| c.id != id);
                    for out in &mut self.outgoing {
                        if out.id.as_deref() == Some(&id) {
                            out.state = OutgoingState::Canceled;
                        }
                    }
                }
                ConnectionEvent::ChallengeDeclined { id, .. } => {
                    for out in &mut self.outgoing {
                        if out.id.as_deref() == Some(&id) {
                            out.state = OutgoingState::Declined;
                            self.notices.push(OnlineNotice::ChallengeDeclined {
                                name: out.name.clone(),
                            });
                        }
                    }
                }
                ConnectionEvent::Closed(error) => {
                    self.notices.push(OnlineNotice::EventsClosed(error));
                    self.events = None;
                    break;
                }
            }
        }
        for remote_id in finished {
            self.fetch_end(&remote_id);
        }
    }

    fn on_game_started(&mut self, info: GameInfo) {
        if self.live(&info.game_id).is_some() {
            return;
        }
        if info.variant != "standard" && info.variant != "fromPosition" {
            self.notices.push(OnlineNotice::VariantIgnored {
                variant: info.variant,
            });
            return;
        }
        let Some(board) = self.board.clone() else {
            return;
        };
        let correspondence = info.speed == "correspondence";
        let expected = self.expecting_start
            || self
                .ai
                .as_ref()
                .is_some_and(|a| a.game_id.as_deref() == Some(&info.game_id));
        // Correspondência que começou sem o jogador esperar (busca publicada há dias), ou que
        // o Lichess relista ao abrir o fluxo: entra na lista; não abre a sessão nem troca o
        // tabuleiro. Só a nova é avisada.
        if correspondence && !expected {
            if !self.db.by_remote.contains_key(&info.game_id) {
                self.notices.push(OnlineNotice::GameStarted {
                    remote_id: info.game_id.clone(),
                });
            }
            self.remote.retain(|g| g.game_id != info.game_id);
            self.remote.push(info);
            return;
        }
        let remote_id = info.game_id.clone();
        let live = self.start_session(&board, info);
        self.games.push(live);
        self.to_show = Some(remote_id.clone());
        self.notices.push(OnlineNotice::GameStarted { remote_id });
        // Quem esperava já tem a sua Partida.
        if let Some(run) = self.seek.take() {
            run.seeking.cancel();
        }
        self.ai = None;
        for out in &mut self.outgoing {
            if matches!(out.state, OutgoingState::Waiting | OutgoingState::Sending) {
                out.state = OutgoingState::Canceled;
            }
        }
        self.outgoing.retain(|o| o.state != OutgoingState::Canceled);
        self.expecting_start = false;
    }

    fn poll_seek(&mut self) {
        let Some(run) = &mut self.seek else {
            return;
        };
        match run.seeking.poll() {
            SeekState::Waiting => {}
            SeekState::Posted { .. } => run.posted = true,
            SeekState::Ended => {
                // O `gameStart` (se houve par) chega pelo fluxo de eventos e já fechou a busca.
                self.seek = None;
                self.notices.push(OnlineNotice::SeekEnded);
            }
            SeekState::Failed(error) => {
                self.seek = None;
                self.notices.push(OnlineNotice::Failed(error));
            }
        }
    }

    fn poll_ai(&mut self) {
        let Some(ai) = &mut self.ai else {
            return;
        };
        let Some(query) = &mut ai.query else {
            return;
        };
        match query.try_take() {
            None => {}
            Some(Ok(answer)) => {
                ai.game_id = Some(answer.value);
                ai.query = None;
            }
            Some(Err(error)) => {
                self.ai = None;
                self.notices.push(OnlineNotice::Failed(error));
            }
        }
    }

    fn poll_outgoing(&mut self) {
        for out in &mut self.outgoing {
            let Some(query) = &mut out.query else {
                continue;
            };
            match query.try_take() {
                None => {}
                Some(Ok(answer)) => {
                    out.id = Some(answer.value.id);
                    out.state = OutgoingState::Waiting;
                    out.query = None;
                }
                Some(Err(error)) => {
                    out.state = OutgoingState::Failed(error.clone());
                    out.query = None;
                    self.notices.push(OnlineNotice::Failed(error));
                }
            }
        }
    }

    fn poll_requests(&mut self) {
        let mut still = Vec::new();
        for mut request in std::mem::take(&mut self.requests) {
            match request.query.try_take() {
                None => still.push(request),
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    if request.what == RequestKind::Accept {
                        self.expecting_start = self.seek.is_some() || self.ai.is_some();
                    }
                    self.notices.push(OnlineNotice::Failed(error));
                }
            }
        }
        self.requests = still;
    }

    fn poll_playing(&mut self) {
        let Some(query) = &mut self.playing else {
            return;
        };
        match query.try_take() {
            None => {}
            Some(Ok(answer)) => {
                self.remote = answer
                    .value
                    .into_iter()
                    .filter(|g| g.variant == "standard" || g.variant == "fromPosition")
                    .collect();
                self.remote_at = Some(self.now());
                self.playing = None;
                // Do banco, em andamento aqui e fora da lista do Lichess: terminou enquanto o
                // app estava fechado. O retrato do fim vem pela sessão.
                let gone: Vec<String> = self
                    .db
                    .in_progress
                    .iter()
                    .filter_map(|(_, g)| g.setup().online_game().map(|o| o.remote_id.clone()))
                    .filter(|id| !self.remote.iter().any(|r| &r.game_id == id))
                    .collect();
                for remote_id in gone {
                    self.fetch_end(&remote_id);
                }
            }
            Some(Err(error)) => {
                self.playing = None;
                self.notices.push(OnlineNotice::Failed(error));
            }
        }
    }

    fn poll_games(&mut self, store: &mut Option<StoreAccess<'_>>) {
        let now = self.now();
        let mut ended = false;
        for live in &mut self.games {
            let polled = live.session.poll();
            if polled.created
                && let (Some(game), Some(access)) = (live.session.game(), store.as_mut())
            {
                match access.store.create_game_for(
                    access.profile,
                    game.setup(),
                    &metadata(game, now),
                ) {
                    Ok(id) => {
                        live.store_id = Some(id);
                        self.db.by_remote.insert(live.remote_id().to_owned(), id);
                    }
                    Err(error) => self
                        .notices
                        .push(OnlineNotice::StoreFailed(error.to_string())),
                }
            }
            if polled.created
                && let Some(game) = live.session.game()
            {
                live.realtime = game
                    .setup()
                    .online_game()
                    .is_some_and(|o| o.days_per_move.is_none());
            }
            // Só o que o Provedor confirmou: exatamente os Eventos do controlador.
            if !polled.events.is_empty()
                && let (Some(id), Some(access)) = (live.store_id, store.as_mut())
                && let Err(error) = access.store.append_events(id, &polled.events)
            {
                self.notices
                    .push(OnlineNotice::StoreFailed(error.to_string()));
            }
            ended |= polled.events.iter().any(|e| {
                matches!(
                    e.kind,
                    chess_game::EventKind::GameEnded { .. } | chess_game::EventKind::GameAborted
                )
            });
            for notice in polled.notices {
                let mapped = match notice {
                    SessionNotice::MoveRefused { mv, .. } => Some(OnlineNotice::MoveRefused { mv }),
                    SessionNotice::MoveLost { mv } => Some(OnlineNotice::MoveLost { mv }),
                    SessionNotice::MoveReplaced { sent } => {
                        Some(OnlineNotice::MoveReplaced { mv: sent })
                    }
                    SessionNotice::RequestFailed { action, .. } => {
                        Some(OnlineNotice::RequestFailed { action })
                    }
                    SessionNotice::Desynced(_) => Some(OnlineNotice::Desynced),
                    SessionNotice::Closed(error) => Some(OnlineNotice::SessionClosed(error)),
                    _ => None,
                };
                self.notices.extend(mapped);
            }
        }
        if ended {
            self.db.dirty = true;
        }
        if self
            .games
            .iter()
            .any(|g| g.store_id.is_some() && g.game().is_some())
        {
            // Novas Partidas no banco entram na lista.
            let unknown = self.games.iter().any(|g| {
                g.store_id.is_some()
                    && !g.over()
                    && !self
                        .db
                        .in_progress
                        .iter()
                        .any(|(id, _)| Some(*id) == g.store_id)
            });
            if unknown {
                self.db.dirty = true;
            }
        }
        // Terminada e com o fluxo fechado: a sessão já soltou a trava; sai da lista.
        self.games
            .retain(|g| !(g.over() && g.link() == LinkStatus::Closed));
    }

    /// As Partidas online que a Trava de fair play acompanha: as das sessões abertas (o
    /// estado mais novo) e as do banco sem sessão.
    pub fn locked_games(&self) -> Vec<&Game> {
        let mut games: Vec<&Game> = self.games.iter().filter_map(Live::game).collect();
        for (_, game) in &self.db.in_progress {
            let remote = game.setup().online_game().map(|o| o.remote_id.as_str());
            if !self.games.iter().any(|g| Some(g.remote_id()) == remote) {
                games.push(game);
            }
        }
        games
    }

    /// O banco mudou por fora (troca de Perfil local, Biblioteca): relê no próximo quadro.
    pub fn mark_db_dirty(&mut self) {
        self.db.dirty = true;
    }
}
