//! FICS como visitante no app (#254; decisões FC-01 a FC-12 e FC-13 a FC-18): a conexão de
//! visitante, a busca de adversário, os desafios, a Partida jogada e a observação. Sem Bevy nem
//! egui.
//!
//! - **Só visitante.** O app entra com `FicsLogin::Guest`; não há campo de senha em lugar
//!   nenhum, e a conexão (texto puro, sem criptografia) só abre por ação do jogador, com o FICS
//!   ligado no Perfil local. Desligado: nenhuma conexão (o gate da rede também barra).
//! - **A Partida jogada** passa pela mesma sessão abstrata do Lichess
//!   ([`GameSession`](super::service::GameSession)): o tabuleiro, o Painel e a gravação no banco
//!   são os de sempre, e o banco recebe exatamente os Eventos que o FICS confirmou, com a Origem
//!   "Partida online · FICS (visitante)".
//! - **Trava de fair play.** Quem a segura é a `FicsSession` (da Partida contra pessoa até o fim
//!   confirmado). A sessão só é descartada pelo fim confirmado seguido de "Sair do FICS", ou
//!   por descarte explícito: depois de uma queda, a trava fica até o jogador descartar.
//! - **Queda.** Sem reconexão automática: no FICS, a Partida de visitante que cai é perdida
//!   por desconexão. O app avisa; entrar de novo é outra sessão, outro visitante.
//! - **Observar.** Só na memória (`FicsSession::observed`): nunca vai ao banco e não liga a
//!   trava.

use super::service::{
    Confirmed, GameKey, GameSession, OnlineNotice, Provider, StoreAccess, Unavailable, record,
};
use crate::net_service::NetService;
use crate::session::OnlineIntent;
use chess_core::{Color, Move};
use chess_game::{Command, Game, Timestamp as GameTime};
use chess_net::{ProviderId, Timestamp};
use chess_providers::{
    Fics, FicsChallenge, FicsError, FicsLogin, FicsNotice, FicsOutgoing, FicsSession, FicsStatus,
    LinkStatus, ObservedGame,
};
use chess_store::{GameId, GameQuery, ProfileId, Store};

/// Chave do Provedor no `GameSetup` e no `metadata` das Partidas do FICS.
pub const FICS_KEY: &str = "fics";

/// O que o FICS conta ao jogador (o texto vem do Fluent, na interface).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FicsInfo {
    /// O FICS abriu a sessão de visitante com este nome.
    LoggedIn {
        handle: String,
    },
    /// A busca de adversário foi publicada.
    SeekPosted,
    /// Chegou um desafio.
    Challenge {
        name: String,
    },
    /// O FICS enviou o desafio por nome do jogador.
    ChallengeIssued {
        to: String,
    },
    /// O desafiado recusou.
    ChallengeDeclined {
        by: String,
    },
    /// O desafiado não está no FICS.
    ChallengeFailed {
        name: String,
    },
    /// O FICS recusou o Lance enviado: a peça volta.
    MoveRefused {
        mv: Move,
    },
    /// O Lance ficou sem confirmação (queda ou fim): não é reenviado.
    MoveLost {
        mv: Move,
    },
    /// O FICS registrou outro Lance no lugar do enviado.
    MoveReplaced {
        mv: Move,
    },
    /// Uma Partida que não é xadrez padrão foi recusada; nada foi criado.
    VariantRefused {
        variant: String,
    },
    /// O tabuleiro do FICS não confere com a Partida do app: nada foi reescrito.
    Desynced,
    /// Um tabuleiro coerente desfez a dessincronização.
    Resynced,
    /// O adversário pediu para abortar.
    AbortRequested,
    ObservationStarted {
        game: u32,
    },
    ObservationEnded {
        game: u32,
    },
    /// A sessão parou (queda, entrada recusada, Provedor desligado…).
    Closed(FicsError),
    /// Um pedido foi recusado (antes da rede ou pelo envio).
    Refused(FicsError),
}

/// A busca de adversário enviada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FicsSeek {
    pub minutes: u32,
    pub increment_s: u32,
    pub since: Timestamp,
    /// O FICS confirmou a publicação.
    pub posted: bool,
}

/// A conexão de visitante aberta (uma por vez).
pub struct FicsConn {
    session: FicsSession,
    /// A Partida jogada no banco.
    store_id: Option<GameId>,
    seek: Option<FicsSeek>,
    /// Observação pedida, até o FICS responder.
    observe_requested: Option<u32>,
    observing: bool,
    /// Por que a sessão parou.
    closed: Option<FicsError>,
}

impl std::fmt::Debug for FicsConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FicsConn")
            .field("session", &self.session)
            .field("store_id", &self.store_id)
            .finish_non_exhaustive()
    }
}

impl FicsConn {
    fn in_game(&self) -> bool {
        self.session.game().is_some_and(|g| !g.snapshot().is_over())
    }
}

impl GameSession for FicsConn {
    fn provider(&self) -> Provider {
        Provider::Fics
    }

    fn remote_id(&self) -> Option<&str> {
        self.session
            .game()
            .and_then(|g| g.setup().online_game())
            .map(|o| o.remote_id.as_str())
    }

    fn game(&self) -> Option<&Game> {
        self.session.game()
    }

    fn store_id(&self) -> Option<GameId> {
        self.store_id
    }

    fn link(&self) -> LinkStatus {
        match self.session.status() {
            FicsStatus::Connecting | FicsStatus::LoggingIn => LinkStatus::Connecting,
            FicsStatus::Ready => LinkStatus::Live,
            FicsStatus::Closed => LinkStatus::Closed,
        }
    }

    fn desynced(&self) -> bool {
        self.session.is_desynced()
    }

    fn request(&mut self, intent: OnlineIntent, _now: Timestamp) -> Result<(), OnlineNotice> {
        use OnlineIntent as I;
        let result = match intent {
            I::Play(mv) => self.session.play(mv),
            I::Resign => self.session.resign(),
            I::Abort => self.session.abort(),
            I::OfferDraw => self.session.offer_draw(),
            I::AcceptDraw => self.session.accept_draw(),
            I::DeclineDraw => self.session.decline_draw(),
            I::ClaimVictory => Err(FicsError::NotAllowed(
                "o FICS não tem reclamar vitória por ausência",
            )),
        };
        result.map_err(|error| {
            OnlineNotice::Fics(match (intent, error) {
                (_, FicsError::Desynced) => FicsInfo::Desynced,
                (I::Play(mv), _) => FicsInfo::MoveRefused { mv },
                (_, error) => FicsInfo::Refused(error),
            })
        })
    }
}

/// A parte do FICS no serviço online: a conexão de visitante e o que se faz com ela.
#[derive(Debug, Default)]
pub struct FicsPlay {
    conn: Option<FicsConn>,
}

fn refused(error: FicsError) -> Unavailable {
    Unavailable::Fics(error)
}

impl FicsPlay {
    // --- Conexão --------------------------------------------------------------------------

    /// Entra no FICS como visitante (só por ação do jogador). Com o FICS desligado, nada sai:
    /// nenhuma conexão é aberta. Depois de uma queda com Partida em andamento, a sessão antiga
    /// precisa ser descartada antes (é ela que segura a Trava de fair play).
    pub fn connect(&mut self, net: &NetService) -> Result<(), Unavailable> {
        if !net.is_enabled(ProviderId::Fics) {
            return Err(Unavailable::ProviderOff);
        }
        if let Some(conn) = &self.conn {
            if conn.session.status() != FicsStatus::Closed {
                return Ok(());
            }
            if conn.session.holds_fair_play() {
                return Err(refused(FicsError::NotAllowed(
                    "descarte antes a sessão da Partida que caiu",
                )));
            }
        }
        // Só visitante: `FicsLogin::Account` nem existe na interface (FC-01).
        let session = Fics::new(net.net().clone(), net.clock())
            .connect(FicsLogin::Guest)
            .map_err(refused)?;
        self.conn = Some(FicsConn {
            session,
            store_id: None,
            seek: None,
            observe_requested: None,
            observing: false,
            closed: None,
        });
        Ok(())
    }

    /// Sai do FICS: fecha a conexão e descarta a sessão. Com Partida em andamento, é o
    /// descarte explícito (o FICS a dá como perdida); a Trava de fair play dela cai junto.
    pub fn leave(&mut self) {
        self.conn = None;
    }

    pub fn status(&self) -> Option<FicsStatus> {
        self.conn.as_ref().map(|c| c.session.status())
    }

    /// O nome de visitante que o FICS deu.
    pub fn handle(&self) -> Option<&str> {
        self.conn.as_ref().and_then(|c| c.session.handle())
    }

    /// Por que a sessão parou (se parou).
    pub fn closed_reason(&self) -> Option<&FicsError> {
        self.conn.as_ref().and_then(|c| c.closed.as_ref())
    }

    /// A conexão caiu com a Partida em andamento: o FICS a dá como perdida, o resultado não
    /// foi confirmado e a sessão segura a trava até o descarte.
    pub fn dropped_mid_game(&self) -> bool {
        self.status() == Some(FicsStatus::Closed)
            && self.game_in_progress()
            && self.holds_fair_play()
    }

    /// A sessão segura a Trava de fair play.
    pub fn holds_fair_play(&self) -> bool {
        self.conn
            .as_ref()
            .is_some_and(|c| c.session.holds_fair_play())
    }

    // --- Antes da Partida -----------------------------------------------------------------

    fn ready(&mut self) -> Result<&mut FicsConn, Unavailable> {
        self.conn.as_mut().ok_or(refused(FicsError::NotAllowed(
            "entre no FICS como visitante antes",
        )))
    }

    /// Busca adversário (sem rating: visitante só joga sem rating).
    pub fn seek(
        &mut self,
        minutes: u32,
        increment_s: u32,
        now: Timestamp,
    ) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session.seek(minutes, increment_s).map_err(refused)?;
        conn.seek = Some(FicsSeek {
            minutes,
            increment_s,
            since: now,
            posted: false,
        });
        Ok(())
    }

    /// Retira a busca.
    pub fn unseek(&mut self) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.seek = None;
        conn.session.unseek().map_err(refused)
    }

    pub fn seek_state(&self) -> Option<FicsSeek> {
        self.conn.as_ref().and_then(|c| c.seek)
    }

    /// Desafios recebidos e ainda sem resposta.
    pub fn challenges(&self) -> &[FicsChallenge] {
        self.conn
            .as_ref()
            .map_or(&[] as &[FicsChallenge], |c| c.session.challenges())
    }

    /// Aceita o desafio de `challenger`. Variante é recusada antes da rede.
    pub fn accept(&mut self, challenger: &str) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session.accept(challenger).map_err(refused)
    }

    /// Recusa o desafio de `challenger`.
    pub fn decline(&mut self, challenger: &str) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session.decline(challenger).map_err(refused)
    }

    /// Desafia `name` pelo nome (`match`): xadrez padrão, sem rating, no ritmo dado. Nome
    /// inválido (e com ele qualquer variante) é recusado antes da rede.
    pub fn challenge(
        &mut self,
        name: &str,
        minutes: u32,
        increment_s: u32,
        color: Option<Color>,
    ) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session
            .challenge(name.trim(), minutes, increment_s, color)
            .map_err(refused)
    }

    /// Retira o desafio por nome enviado.
    pub fn withdraw_challenge(&mut self) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session.withdraw_challenge().map_err(refused)
    }

    /// O desafio por nome enviado, até a resposta.
    pub fn outgoing(&self) -> Option<&FicsOutgoing> {
        self.conn.as_ref().and_then(|c| c.session.outgoing())
    }

    /// Observa a Partida `game` do FICS: só na memória, sem trava.
    pub fn observe(&mut self, game: u32) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.session.observe(game).map_err(refused)?;
        conn.observe_requested = Some(game);
        Ok(())
    }

    pub fn unobserve(&mut self) -> Result<(), Unavailable> {
        let conn = self.ready()?;
        conn.observe_requested = None;
        conn.session.unobserve().map_err(refused)
    }

    /// A observação pedida, até o FICS responder.
    pub fn observe_requested(&self) -> Option<u32> {
        self.conn.as_ref().and_then(|c| c.observe_requested)
    }

    /// A observação está em andamento.
    pub fn observing(&self) -> bool {
        self.conn.as_ref().is_some_and(|c| c.observing)
    }

    /// A Partida observada (a atual ou a última), para mostrar. Nunca vai ao banco.
    pub fn observed(&self) -> Option<ObservedGame> {
        self.conn.as_ref().and_then(|c| c.session.observed())
    }

    // --- A Partida jogada -----------------------------------------------------------------

    /// A Partida jogada (a atual ou a última desta sessão).
    pub fn game(&self) -> Option<&Game> {
        self.conn.as_ref().and_then(|c| c.session.game())
    }

    /// Há Partida jogada em andamento (inclusive a que caiu sem fim confirmado)?
    pub fn game_in_progress(&self) -> bool {
        self.conn.as_ref().is_some_and(FicsConn::in_game)
    }

    /// A linha no banco da Partida da sessão que caiu durante a Partida (FC-16).
    pub fn dropped_store_id(&self) -> Option<GameId> {
        if !self.dropped_mid_game() {
            return None;
        }
        self.conn.as_ref().and_then(|c| c.store_id)
    }

    /// A linha no banco da Partida da conexão **viva** (a que o FICS ainda pode confirmar).
    pub fn live_store_id(&self) -> Option<GameId> {
        self.conn
            .as_ref()
            .filter(|c| c.session.status() != FicsStatus::Closed)
            .and_then(|c| c.store_id)
    }

    /// Encerra no app, **sem resultado**, a Partida `id` do FICS cujo fim o FICS nunca vai
    /// confirmar (a conexão de visitante caiu, ou o jogador saiu no meio, FC-22). Só por pedido
    /// explícito do jogador; nada é inventado: o log ganha o encerramento, sem vencedor nem
    /// motivo, e só então a Trava de fair play desta Partida cai. A sessão que caiu, se é a
    /// desta Partida, é descartada junto. Com a conexão viva, recusa: o FICS ainda decide.
    pub fn close_without_result(
        &mut self,
        store: &mut Store,
        id: GameId,
        now: Timestamp,
    ) -> Result<Game, Unavailable> {
        if self.live_store_id() == Some(id) {
            return Err(refused(FicsError::NotAllowed(
                "a Partida ainda está em andamento no FICS",
            )));
        }
        let mut game = store
            .load_game(id)
            .map_err(|e| Unavailable::Store(e.to_string()))?;
        let fics = game
            .setup()
            .online_game()
            .is_some_and(|o| o.provider == FICS_KEY);
        if !fics {
            return Err(refused(FicsError::NotAllowed(
                "só a Partida do FICS sem fim confirmado se encerra assim",
            )));
        }
        // O instante do domínio nunca volta: o do último Evento, com a hora de agora.
        let last = game.events().last().map_or(0, |e| e.at.mono_ms);
        let events = game
            .handle(
                Command::CloseWithoutResult,
                GameTime::with_wall(last, now.as_millis()),
            )
            .map_err(|r| refused(FicsError::Rejected(r)))?;
        store
            .append_events(id, &events)
            .map_err(|e| Unavailable::Store(e.to_string()))?;
        if self.conn.as_ref().is_some_and(|c| c.store_id == Some(id)) {
            // A sessão que caiu era desta Partida: ela some (e a trava que segurava, junto).
            self.conn = None;
        }
        Ok(game)
    }

    pub fn session_for(&self, remote_id: &str) -> Option<&dyn GameSession> {
        self.conn
            .as_ref()
            .map(|c| c as &dyn GameSession)
            .filter(|c| c.remote_id() == Some(remote_id))
    }

    pub fn session_for_mut(&mut self, remote_id: &str) -> Option<&mut dyn GameSession> {
        self.conn
            .as_mut()
            .map(|c| c as &mut dyn GameSession)
            .filter(|c| c.remote_id() == Some(remote_id))
    }

    // --- O quadro ------------------------------------------------------------------------

    /// Recolhe o que o FICS mandou, grava no banco o que ele confirmou e diz que Partida pôr
    /// no tabuleiro (a que começou). Nunca bloqueia.
    pub fn poll(
        &mut self,
        store: &mut Option<StoreAccess<'_>>,
        now: Timestamp,
        notices: &mut Vec<OnlineNotice>,
    ) -> Option<GameKey> {
        let conn = self.conn.as_mut()?;
        let polled = conn.session.poll();
        if polled.created {
            // Outra Partida nesta sessão: outra linha no banco.
            conn.store_id = None;
        }
        record(
            store,
            &mut conn.store_id,
            &Confirmed {
                game: conn.session.game(),
                created: polled.created,
                events: &polled.events,
                connection: None,
            },
            now,
            notices,
        );
        let mut show = None;
        for notice in polled.notices {
            let info = match notice {
                FicsNotice::LoggedIn { handle } => FicsInfo::LoggedIn { handle },
                FicsNotice::SeekPosted => {
                    if let Some(seek) = &mut conn.seek {
                        seek.posted = true;
                    }
                    FicsInfo::SeekPosted
                }
                FicsNotice::Challenge(challenge) => FicsInfo::Challenge {
                    name: challenge.challenger,
                },
                FicsNotice::ChallengeIssued { to } => FicsInfo::ChallengeIssued { to },
                FicsNotice::ChallengeDeclined { by } => FicsInfo::ChallengeDeclined { by },
                FicsNotice::ChallengeFailed { name } => FicsInfo::ChallengeFailed { name },
                FicsNotice::GameStarted { game, .. } => {
                    // O FICS retira as buscas de quem começou a jogar.
                    conn.seek = None;
                    let key = GameKey::new(Provider::Fics, game.to_string());
                    notices.push(OnlineNotice::GameStarted {
                        remote_id: key.remote_id.clone(),
                    });
                    show = Some(key);
                    continue;
                }
                FicsNotice::VariantRefused { variant, .. } => FicsInfo::VariantRefused { variant },
                FicsNotice::MoveRefused { mv } => FicsInfo::MoveRefused { mv },
                FicsNotice::MoveLost { mv } => FicsInfo::MoveLost { mv },
                FicsNotice::MoveReplaced { sent } => FicsInfo::MoveReplaced { mv: sent },
                FicsNotice::Desynced { .. } => FicsInfo::Desynced,
                FicsNotice::Resynced { .. } => FicsInfo::Resynced,
                FicsNotice::AbortRequested => FicsInfo::AbortRequested,
                FicsNotice::ObservationStarted { game } => {
                    conn.observe_requested = None;
                    conn.observing = true;
                    FicsInfo::ObservationStarted { game }
                }
                FicsNotice::ObservationEnded { game } => {
                    conn.observing = false;
                    FicsInfo::ObservationEnded { game }
                }
                FicsNotice::Closed(error) => {
                    conn.seek = None;
                    conn.observe_requested = None;
                    conn.observing = false;
                    conn.closed = Some(error.clone());
                    FicsInfo::Closed(error)
                }
                // O Lance confirmado, a proposta de empate e o fim aparecem pela Partida.
                _ => continue,
            };
            notices.push(OnlineNotice::Fics(info));
        }
        show
    }
}

/// As Partidas do FICS no banco sem fim confirmado (FC-22): a conexão de visitante caiu (ou o
/// jogador saiu no meio) e o FICS nunca vai confirmar o resultado. A Biblioteca as mostra como
/// "sem resultado confirmado". Quem chama tira a da conexão viva ([`FicsPlay::live_store_id`]).
pub fn unconfirmed_games(store: &Store, profile: ProfileId) -> Vec<GameId> {
    let query = GameQuery {
        only_unfinished: true,
        profile: Some(profile),
        limit: None,
    };
    store
        .list_games(&query)
        .map(|games| {
            games
                .into_iter()
                .filter(|g| {
                    g.metadata
                        .get("online")
                        .and_then(|o| o.get("provider"))
                        .and_then(serde_json::Value::as_str)
                        == Some(FICS_KEY)
                })
                .map(|g| g.id)
                .collect()
        })
        .unwrap_or_default()
}
