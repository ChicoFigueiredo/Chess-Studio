//! O motor do Painel Explorador, sem Bevy nem egui: dado o que o Painel quer (aba, fonte,
//! Posição) e o que o app permite (Provedor ligado, Trava de fair play, autorização), decide
//! se pede, quando pede (a Posição precisa parar ~300 ms), cancela o pedido antigo e
//! entrega, por quadro, o que a interface deve mostrar. É aqui que a regra de domínio vive;
//! a interface só desenha o resultado.
//!
//! Nada aqui fala com a Partida (ADR-0001): a Posição entra como texto (FEN).

use super::lookup::{Lookup, LookupState, Settle};
use super::model::{
    Authorization, CloudView, EndgameTable, Lock, Notice, OpeningsTable, Source, Tab,
    covered_by_tablebase, notice_for,
};
use crate::net_service::NetService;
use chess_core::{CanonicalPosition, Color};
use chess_net::{ProviderId, Timestamp};
use chess_providers::{
    CloudEval, ExplorerPosition, LichessFilter, MastersFilter, PlayerFilter, ProviderError,
    Tablebase,
};
use chess_store::ConnectionRole;
use std::time::Instant;

/// O que muda a cada quadro.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    /// A Posição mostrada no tabuleiro, em FEN.
    pub fen: &'a str,
    /// Relógio da interface (debounce).
    pub now: Instant,
    /// Relógio de parede (fim de uma pausa pedida pelo Provedor).
    pub at: Timestamp,
    /// Por que o Painel não pode consultar nada, se for o caso.
    pub lock: Option<Lock>,
    pub authorization: Authorization,
}

/// O que a aba Aberturas mostra para uma fonte online.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpeningsView {
    Notice(Notice),
    Table(OpeningsTable),
}

/// O que a aba Finais mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndgamesView {
    Notice(Notice),
    Table(EndgameTable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
    Openings,
    Endgames,
}

/// O Painel Explorador: escolhas do usuário e as consultas em andamento.
pub struct Explorer {
    pub tab: Tab,
    pub source: Source,
    /// Nome público no Lichess, para a fonte "Um jogador".
    pub player: String,
    pub player_color: Color,
    settle: Settle<String>,
    openings: Lookup<ExplorerPosition>,
    tablebase: Lookup<Tablebase>,
}

impl Default for Explorer {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            source: Source::default(),
            player: String::new(),
            player_color: Color::White,
            settle: Settle::default(),
            openings: Lookup::default(),
            tablebase: Lookup::default(),
        }
    }
}

impl Explorer {
    /// Pedidos que o Painel já iniciou (testes e diagnóstico).
    #[cfg(test)]
    pub fn requests_started(&self) -> usize {
        self.openings.sent() + self.tablebase.sent()
    }

    /// Falta esperar a Posição parar ou o pedido chegar: a interface pede novo quadro.
    pub fn is_busy(&self) -> bool {
        self.settle.is_settling()
            || matches!(self.openings.state(), LookupState::Loading { .. })
            || matches!(self.tablebase.state(), LookupState::Loading { .. })
    }

    pub fn settle_remaining(&self, now: Instant) -> Option<std::time::Duration> {
        self.settle.remaining(now)
    }

    /// Recomeça a consulta atual (o botão "Tentar de novo").
    pub fn retry(&mut self) {
        self.stand_down();
    }

    fn stand_down(&mut self) {
        self.openings.clear();
        self.tablebase.clear();
        self.settle.reset();
    }

    fn wanted(&self, input: &Input<'_>, service: &NetService) -> Option<(Wanted, String)> {
        let lichess_on = service.is_enabled(ProviderId::Lichess);
        match self.tab {
            Tab::Openings => {
                if !self.source.is_online() || !lichess_on {
                    return None;
                }
                if input.authorization != Authorization::Granted {
                    return None;
                }
                if self.source == Source::Player && self.player.trim().is_empty() {
                    return None;
                }
                let who = match self.source {
                    Source::Player => format!("{}:{:?}", self.player.trim(), self.player_color),
                    other => other.key().to_owned(),
                };
                Some((Wanted::Openings, format!("aberturas|{who}|{}", input.fen)))
            }
            Tab::Endgames => (lichess_on && covered_by_tablebase(input.fen))
                .then(|| (Wanted::Endgames, format!("finais|{}", input.fen))),
        }
    }

    /// Chame a cada quadro. Não bloqueia: recolhe o que chegou, cancela o que ficou velho e
    /// só pede quando a Posição parou.
    pub fn drive(&mut self, input: &Input<'_>, service: &NetService) {
        self.openings.poll();
        self.tablebase.poll();
        // O Explorador do Lichess só responde com uma Conexão autorizada: o pedido a nomeia.
        service.lichess().use_connection(
            service.authorized_connection(ProviderId::Lichess, ConnectionRole::Player),
        );
        self.retry_after_pause(input);
        // A Trava vale antes de tudo: nem um pedido sai, e o que estava a caminho é cancelado.
        if input.lock.is_some() {
            self.stand_down();
            return;
        }
        let Some((kind, key)) = self.wanted(input, service) else {
            self.stand_down();
            return;
        };
        let current = match kind {
            Wanted::Openings => self.openings.key(),
            Wanted::Endgames => self.tablebase.key(),
        };
        if current != Some(key.as_str()) {
            // Mudou a Posição (ou a fonte): o que estava na tela era de outra pergunta.
            self.openings.clear();
            self.tablebase.clear();
        }
        if !self.settle.poll(&key, input.now) {
            return;
        }
        match kind {
            Wanted::Openings => self.start_openings(key, input.fen, service),
            Wanted::Endgames => {
                let query = service.lichess().tablebase(input.fen);
                self.tablebase.start(key, query);
            }
        }
    }

    fn start_openings(&mut self, key: String, fen: &str, service: &NetService) {
        let lichess = service.lichess();
        match self.source {
            Source::Masters => {
                let query = lichess.masters(fen, &MastersFilter::default());
                self.openings.start(key, query);
            }
            Source::Lichess => {
                let query = lichess.explorer(fen, &LichessFilter::default());
                self.openings.start(key, query);
            }
            Source::Player => {
                let stream = lichess.player_explorer(
                    fen,
                    self.player.trim(),
                    self.player_color,
                    &PlayerFilter::default(),
                );
                self.openings.start(key, stream);
            }
            Source::Library => {}
        }
    }

    /// Passada a pausa que o Provedor pediu, tenta de novo uma vez (sem martelar o serviço).
    fn retry_after_pause(&mut self, input: &Input<'_>) {
        fn paused<T>(state: &LookupState<T>) -> Option<Timestamp> {
            match state {
                LookupState::Failed(ProviderError::RateLimited { until, .. }) => Some(*until),
                _ => None,
            }
        }
        let over = paused(self.openings.state())
            .into_iter()
            .chain(paused(self.tablebase.state()))
            .any(|until| input.at >= until);
        if over {
            self.stand_down();
        }
    }

    /// A aba Aberturas para uma fonte online.
    pub fn openings_view(&self, input: &Input<'_>, service: &NetService) -> OpeningsView {
        let provider = self.source.provider().unwrap_or(ProviderId::Lichess);
        if let Some(lock) = input.lock {
            return OpeningsView::Notice(lock_notice(lock));
        }
        if !service.is_enabled(provider) {
            return OpeningsView::Notice(Notice::ProviderOff(provider));
        }
        if input.authorization != Authorization::Granted {
            return OpeningsView::Notice(Notice::SignInRequired(provider));
        }
        if self.source == Source::Player && self.player.trim().is_empty() {
            return OpeningsView::Notice(Notice::NeedsPlayerName);
        }
        match self.openings.state() {
            LookupState::Idle => OpeningsView::Notice(Notice::Loading),
            LookupState::Loading { partial: None } => OpeningsView::Notice(Notice::Loading),
            LookupState::Loading {
                partial: Some(answer),
            } => OpeningsView::Table(OpeningsTable::from_explorer(answer, true)),
            LookupState::Done(answer) => {
                let empty = answer.value.moves.is_empty() && answer.value.outcomes.total() == 0;
                if empty {
                    OpeningsView::Notice(Notice::NoData)
                } else {
                    OpeningsView::Table(OpeningsTable::from_explorer(answer, false))
                }
            }
            LookupState::Failed(error) => OpeningsView::Notice(notice_for(error)),
        }
    }

    /// A aba Finais.
    pub fn endgames_view(&self, input: &Input<'_>, service: &NetService) -> EndgamesView {
        if let Some(lock) = input.lock {
            return EndgamesView::Notice(lock_notice(lock));
        }
        let pieces = super::model::pieces_in_fen(input.fen);
        if !covered_by_tablebase(input.fen) {
            return EndgamesView::Notice(Notice::TooManyPieces { pieces });
        }
        if !service.is_enabled(ProviderId::Lichess) {
            return EndgamesView::Notice(Notice::ProviderOff(ProviderId::Lichess));
        }
        match self.tablebase.state() {
            LookupState::Idle | LookupState::Loading { .. } => {
                EndgamesView::Notice(Notice::Loading)
            }
            LookupState::Done(answer) => {
                let side = if input.fen.split_whitespace().nth(1) == Some("b") {
                    Color::Black
                } else {
                    Color::White
                };
                EndgamesView::Table(EndgameTable::new(answer, side))
            }
            LookupState::Failed(error) => EndgamesView::Notice(notice_for(error)),
        }
    }
}

fn lock_notice(lock: Lock) -> Notice {
    match lock {
        Lock::Competitive => Notice::FairPlay,
        Lock::Training => Notice::Training,
    }
}

// --- Avaliação em cache (Painel Análise) -------------------------------------------------

/// O que a linha "Em cache (Lichess)" mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudRow {
    Notice(Notice),
    /// O Lichess respondeu que não tem avaliação desta Posição: um estado normal.
    NoEvaluation,
    Ready(CloudView),
}

/// A consulta de avaliação em cache da Posição mostrada.
#[derive(Default)]
pub struct CloudDriver {
    settle: Settle<String>,
    lookup: Lookup<Option<CloudEval>>,
}

impl CloudDriver {
    #[cfg(test)]
    pub fn requests_started(&self) -> usize {
        self.lookup.sent()
    }

    pub fn is_busy(&self) -> bool {
        self.settle.is_settling() || matches!(self.lookup.state(), LookupState::Loading { .. })
    }

    pub fn settle_remaining(&self, now: Instant) -> Option<std::time::Duration> {
        self.settle.remaining(now)
    }

    pub fn retry(&mut self) {
        self.lookup.clear();
        self.settle.reset();
    }

    /// Recolhe, cancela e pede (quando a Posição parou) e devolve o que mostrar.
    pub fn drive(
        &mut self,
        base: &CanonicalPosition,
        now: Instant,
        at: Timestamp,
        lock: Option<Lock>,
        service: &NetService,
    ) -> CloudRow {
        self.lookup.poll();
        if let LookupState::Failed(ProviderError::RateLimited { until, .. }) = self.lookup.state()
            && at >= *until
        {
            self.retry();
        }
        if let Some(lock) = lock {
            self.retry();
            return CloudRow::Notice(lock_notice(lock));
        }
        if !service.is_enabled(ProviderId::Lichess) {
            self.retry();
            return CloudRow::Notice(Notice::ProviderOff(ProviderId::Lichess));
        }
        let fen = base.to_fen();
        if self.lookup.key() != Some(fen.as_str()) {
            self.lookup.clear();
        }
        if self.settle.poll(&fen, now) {
            let query = service.lichess().cloud_eval(&fen, 1);
            self.lookup.start(fen, query);
        }
        match self.lookup.state() {
            LookupState::Idle | LookupState::Loading { .. } => CloudRow::Notice(Notice::Loading),
            LookupState::Done(answer) => match CloudView::new(answer, base) {
                Some(view) => CloudRow::Ready(view),
                None => CloudRow::NoEvaluation,
            },
            LookupState::Failed(ProviderError::NoData) => CloudRow::NoEvaluation,
            LookupState::Failed(error) => CloudRow::Notice(notice_for(error)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::explorer::model::How;
    use chess_net::Clock;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
    const AFTER_E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    /// KPK, o exemplo da documentação dos finais exatos do Lichess.
    const KPK: &str = "4k3/6KP/8/8/8/8/7p/8 w - - 0 1";
    /// Torre e peão contra torre (5 peças).
    const ROOK_ENDGAME: &str = "8/8/4k3/8/8/4KP2/6R1/7r w - - 0 1";
    const FULL_BOARD_PLUS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn fixture(name: &str) -> FakeResponse {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../chess-providers/tests/fixtures")
            .join(name);
        FakeResponse::recorded(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    struct Env {
        clock: Arc<FakeClock>,
        server: FakeServer,
        service: NetService,
        t0: Instant,
    }

    /// Cofre falso: entrega um token para qualquer Conexão registrada.
    #[derive(Debug)]
    struct FakeVault;

    impl chess_net::AuthorizationSource for FakeVault {
        fn authorization(
            &self,
            _profile: chess_net::LocalProfileId,
            provider: ProviderId,
            _connection: chess_net::ConnectionId,
        ) -> Option<chess_net::Authorization> {
            Some(chess_net::Authorization::bearer(
                provider,
                &chess_net::Secret::new("token-de-teste"),
            ))
        }
    }

    fn env(lichess_on: bool) -> Env {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let mut config = NetConfig::in_memory()
            .clock(clock.clone())
            .jitter_seed(7)
            .vault(Arc::new(FakeVault));
        for host in [
            "lichess.org",
            "explorer.lichess.org",
            "tablebase.lichess.ovh",
            "api.chess.com",
        ] {
            config = config.route(host, &server.base_url());
        }
        let net = Net::open(config).unwrap();
        if lichess_on {
            net.set_enabled(ProviderId::Lichess, true).unwrap();
        }
        let service = NetService::from_net(net, clock.clone(), false);
        // Uma Conexão autorizada do Lichess no Cofre falso (a Central ainda não cria nenhuma).
        service.set_authorized_connection(
            ProviderId::Lichess,
            ConnectionRole::Player,
            Some(chess_net::ConnectionId(1)),
        );
        Env {
            clock,
            server,
            service,
            t0: Instant::now(),
        }
    }

    fn input(env: &Env, fen: &'static str, after_ms: u64, _auth: Authorization) -> Input<'static> {
        Input {
            fen,
            now: env.t0 + Duration::from_millis(after_ms),
            at: env.clock.now(),
            lock: None,
            authorization: Authorization::lichess_now(&env.service),
        }
    }

    /// Quadros até o Painel parar de esperar (a rede falsa responde em milissegundos).
    fn settle_down(explorer: &mut Explorer, env: &Env, fen: &'static str, auth: Authorization) {
        let mut ms = 0;
        for _ in 0..2000 {
            explorer.drive(&input(env, fen, ms, auth), &env.service);
            if !explorer.is_busy() && ms > 400 {
                return;
            }
            ms += 10;
            std::thread::sleep(Duration::from_millis(3));
        }
        panic!("o Painel nunca parou de esperar");
    }

    fn masters_explorer() -> Explorer {
        Explorer {
            source: Source::Masters,
            ..Explorer::default()
        }
    }

    #[test]
    fn masters_with_an_authorized_connection_show_the_table_with_its_origin() {
        let env = env(true);
        env.server
            .route("/masters", fixture("lichess-explorer-masters.http"));
        let mut explorer = masters_explorer();
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        let view = explorer.openings_view(
            &input(&env, AFTER_E4, 900, Authorization::Granted),
            &env.service,
        );
        let OpeningsView::Table(table) = view else {
            panic!("esperava a tabela, veio {view:?}");
        };
        assert_eq!(table.total, 3000);
        assert_eq!(table.rows[0].san, "c5");
        assert_eq!(table.rows[0].rating, Some(2500));
        assert!(table.has_ratings());
        assert_eq!(table.opening.as_ref().unwrap().0, "B00");
        assert_eq!(table.references.len(), 2);
        assert_eq!(table.origin.provider, "Lichess");
        assert!(matches!(table.origin.how, How::Queried(_)));
        assert_eq!(env.server.requests().len(), 1);
    }

    #[test]
    fn without_an_authorized_connection_the_online_sources_ask_for_sign_in_and_send_nothing() {
        let env = env(true);
        env.service
            .set_authorized_connection(ProviderId::Lichess, ConnectionRole::Player, None);
        let mut explorer = masters_explorer();
        for source in [Source::Masters, Source::Lichess, Source::Player] {
            explorer.source = source;
            explorer.player = "alguem".into();
            settle_down(&mut explorer, &env, AFTER_E4, Authorization::NotAvailable);
            let view = explorer.openings_view(
                &input(&env, AFTER_E4, 900, Authorization::NotAvailable),
                &env.service,
            );
            assert_eq!(
                view,
                OpeningsView::Notice(Notice::SignInRequired(ProviderId::Lichess)),
                "{source:?}"
            );
        }
        assert!(env.server.requests().is_empty());
        assert_eq!(explorer.requests_started(), 0);
    }

    #[test]
    fn the_real_401_of_the_service_is_a_sign_in_notice_not_an_error() {
        let env = env(true);
        env.server
            .route("/masters", fixture("lichess-explorer-masters-401.http"));
        let mut explorer = masters_explorer();
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        let view = explorer.openings_view(
            &input(&env, AFTER_E4, 900, Authorization::Granted),
            &env.service,
        );
        assert_eq!(
            view,
            OpeningsView::Notice(Notice::SignInRequired(ProviderId::Lichess))
        );
    }

    #[test]
    fn a_provider_that_is_off_offers_to_turn_on_and_sends_nothing() {
        let env = env(false);
        let mut explorer = masters_explorer();
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        assert_eq!(
            explorer.openings_view(
                &input(&env, AFTER_E4, 900, Authorization::Granted),
                &env.service
            ),
            OpeningsView::Notice(Notice::ProviderOff(ProviderId::Lichess))
        );
        explorer.tab = Tab::Endgames;
        settle_down(&mut explorer, &env, KPK, Authorization::Granted);
        assert_eq!(
            explorer.endgames_view(&input(&env, KPK, 900, Authorization::Granted), &env.service),
            EndgamesView::Notice(Notice::ProviderOff(ProviderId::Lichess))
        );
        assert!(env.server.requests().is_empty());
    }

    #[test]
    fn the_local_library_source_never_touches_the_network() {
        let env = env(true);
        let mut explorer = Explorer::default();
        assert_eq!(explorer.source, Source::Library);
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        assert!(env.server.requests().is_empty());
        assert_eq!(explorer.requests_started(), 0);
    }

    #[test]
    fn a_burst_of_position_changes_makes_a_single_request_for_the_last_position() {
        let env = env(true);
        env.server
            .route("/masters", fixture("lichess-explorer-masters.http"));
        let mut explorer = masters_explorer();
        let fens: [&'static str; 4] = [
            START,
            AFTER_E4,
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2",
        ];
        // A seta da Notação mudando a Posição a cada 80 ms: nenhum pedido sai.
        for (i, fen) in fens.iter().enumerate() {
            explorer.drive(
                &input(&env, fen, 80 * i as u64, Authorization::Granted),
                &env.service,
            );
        }
        assert_eq!(explorer.requests_started(), 0);
        assert!(env.server.requests().is_empty());
        // Parou: um único pedido, da última Posição.
        let last = fens[3];
        settle_down(&mut explorer, &env, last, Authorization::Granted);
        let sent = env.server.requests();
        assert_eq!(sent.len(), 1);
        assert_eq!(explorer.requests_started(), 1);
        assert!(
            sent[0]
                .query_value("fen")
                .unwrap()
                .starts_with("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2")
        );
    }

    #[test]
    fn changing_position_clears_the_old_rows_at_once() {
        let env = env(true);
        env.server
            .route("/masters", fixture("lichess-explorer-masters.http"));
        let mut explorer = masters_explorer();
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        assert!(matches!(
            explorer.openings_view(
                &input(&env, AFTER_E4, 900, Authorization::Granted),
                &env.service
            ),
            OpeningsView::Table(_)
        ));
        explorer.drive(
            &input(&env, START, 1000, Authorization::Granted),
            &env.service,
        );
        assert_eq!(
            explorer.openings_view(
                &input(&env, START, 1000, Authorization::Granted),
                &env.service
            ),
            OpeningsView::Notice(Notice::Loading)
        );
    }

    #[test]
    fn the_fair_play_lock_sends_nothing_and_cancels_what_was_in_flight() {
        let env = env(true);
        env.server.route("/masters", FakeResponse::stall());
        let mut explorer = masters_explorer();
        // Pede (a rede falsa não responde) e, no meio, a Partida competitiva começa.
        explorer.drive(
            &input(&env, AFTER_E4, 0, Authorization::Granted),
            &env.service,
        );
        explorer.drive(
            &input(&env, AFTER_E4, 400, Authorization::Granted),
            &env.service,
        );
        assert_eq!(explorer.requests_started(), 1);
        let locked = Input {
            lock: Some(Lock::Competitive),
            ..input(&env, AFTER_E4, 500, Authorization::Granted)
        };
        explorer.drive(&locked, &env.service);
        assert!(!explorer.is_busy(), "o pedido a caminho foi cancelado");
        assert_eq!(
            explorer.openings_view(&locked, &env.service),
            OpeningsView::Notice(Notice::FairPlay)
        );
        // Nenhum pedido novo, em nenhuma aba, enquanto durar a Trava.
        explorer.tab = Tab::Endgames;
        for ms in [600, 1200, 2400] {
            let locked = Input {
                lock: Some(Lock::Competitive),
                ..input(&env, KPK, ms, Authorization::Granted)
            };
            explorer.drive(&locked, &env.service);
        }
        assert_eq!(explorer.requests_started(), 1);
    }

    #[test]
    fn the_service_guard_alone_is_enough_the_panel_just_explains_it() {
        let env = env(true);
        env.server
            .route("/masters", fixture("lichess-explorer-masters.http"));
        env.service.net().fair_play_guard().engage("teste");
        let mut explorer = masters_explorer();
        // A interface não sabia da Partida (lock None): o serviço recusa e o Painel explica.
        settle_down(&mut explorer, &env, AFTER_E4, Authorization::Granted);
        assert_eq!(
            explorer.openings_view(
                &input(&env, AFTER_E4, 900, Authorization::Granted),
                &env.service
            ),
            OpeningsView::Notice(Notice::FairPlay)
        );
        assert!(env.server.requests().is_empty());
    }

    #[test]
    fn endgames_show_the_verdict_distances_and_moves_from_best_to_worst() {
        let env = env(true);
        env.server
            .route("/standard", fixture("lichess-tablebase-kpk.http"));
        let mut explorer = Explorer {
            tab: Tab::Endgames,
            ..Explorer::default()
        };
        settle_down(&mut explorer, &env, KPK, Authorization::NotAvailable);
        let EndgamesView::Table(table) = explorer.endgames_view(
            &input(&env, KPK, 900, Authorization::NotAvailable),
            &env.service,
        ) else {
            panic!("esperava a tabela de finais");
        };
        use crate::explorer::model::Outlook;
        assert_eq!(table.verdict.unwrap().outlook, Outlook::Win);
        assert_eq!(table.side, Color::White);
        assert_eq!((table.dtz, table.dtm), (Some(1), Some(17)));
        // O pior vem por último (a ordem do Provedor é do melhor para o pior).
        assert_eq!(table.moves.first().unwrap().san, "h8=Q+");
        assert_eq!(
            table.moves.first().unwrap().verdict.unwrap().outlook,
            Outlook::Win
        );
        assert_eq!(table.origin.provider, "Lichess");
    }

    #[test]
    fn more_than_seven_pieces_never_make_a_request() {
        let env = env(true);
        let mut explorer = Explorer {
            tab: Tab::Endgames,
            ..Explorer::default()
        };
        settle_down(&mut explorer, &env, FULL_BOARD_PLUS, Authorization::Granted);
        assert_eq!(
            explorer.endgames_view(
                &input(&env, FULL_BOARD_PLUS, 900, Authorization::Granted),
                &env.service
            ),
            EndgamesView::Notice(Notice::TooManyPieces { pieces: 32 })
        );
        assert!(env.server.requests().is_empty());
    }

    #[test]
    fn a_rook_and_pawn_endgame_with_no_data_is_a_calm_notice() {
        let env = env(true);
        env.server.route("/standard", FakeResponse::status(404));
        let mut explorer = Explorer {
            tab: Tab::Endgames,
            ..Explorer::default()
        };
        settle_down(&mut explorer, &env, ROOK_ENDGAME, Authorization::Granted);
        assert_eq!(
            explorer.endgames_view(
                &input(&env, ROOK_ENDGAME, 900, Authorization::Granted),
                &env.service
            ),
            EndgamesView::Notice(Notice::NoData)
        );
    }

    #[test]
    fn a_pause_asked_by_the_provider_is_shown_and_retried_once_it_is_over() {
        let env = env(true);
        env.server.route_seq(
            "/standard",
            vec![
                FakeResponse::status(429).header("retry-after", "120"),
                fixture("lichess-tablebase-kpk.http"),
            ],
        );
        let mut explorer = Explorer {
            tab: Tab::Endgames,
            ..Explorer::default()
        };
        settle_down(&mut explorer, &env, KPK, Authorization::Granted);
        let view =
            explorer.endgames_view(&input(&env, KPK, 900, Authorization::Granted), &env.service);
        let EndgamesView::Notice(Notice::Limited { until }) = view else {
            panic!("esperava a pausa, veio {view:?}");
        };
        assert_eq!(until, T0.plus(Duration::from_secs(120)));
        // Enquanto a pausa dura, o Painel não insiste.
        let sent_during = env.server.requests().len();
        for ms in [1000, 2000, 3000] {
            explorer.drive(&input(&env, KPK, ms, Authorization::Granted), &env.service);
        }
        assert_eq!(env.server.requests().len(), sent_during);
        // Passada a pausa, tenta de novo sozinho e mostra o final.
        env.clock.advance(Duration::from_secs(121));
        settle_down(&mut explorer, &env, KPK, Authorization::Granted);
        assert!(matches!(
            explorer.endgames_view(
                &input(&env, KPK, 9000, Authorization::Granted),
                &env.service
            ),
            EndgamesView::Table(_)
        ));
    }

    #[test]
    fn offline_the_saved_answer_is_shown_with_its_age() {
        let env = env(true);
        env.server
            .route("/standard", fixture("lichess-tablebase-kpk.http"));
        let mut explorer = Explorer {
            tab: Tab::Endgames,
            ..Explorer::default()
        };
        settle_down(&mut explorer, &env, KPK, Authorization::Granted);
        assert!(matches!(
            explorer.endgames_view(&input(&env, KPK, 900, Authorization::Granted), &env.service),
            EndgamesView::Table(_)
        ));
        // O computador fica sem internet e o cache vence (30 dias).
        let Env {
            clock,
            service,
            server,
            t0,
        } = env;
        drop(server);
        clock.advance(Duration::from_secs(40 * 86_400));
        let env = Env {
            clock,
            server: FakeServer::start(),
            service,
            t0,
        };
        explorer.retry();
        settle_down(&mut explorer, &env, KPK, Authorization::Granted);
        let EndgamesView::Table(table) = explorer.endgames_view(
            &input(&env, KPK, 9000, Authorization::Granted),
            &env.service,
        ) else {
            panic!("o cache vencido devia aparecer offline");
        };
        assert!(table.origin.is_stale());
    }

    // --- Avaliação em cache ---

    fn cloud_pump(driver: &mut CloudDriver, env: &Env, base: &CanonicalPosition) -> CloudRow {
        let mut ms = 0;
        let mut row = None;
        for _ in 0..2000 {
            let now = env.t0 + Duration::from_millis(ms);
            row = Some(driver.drive(base, now, env.clock.now(), None, &env.service));
            if !driver.is_busy() && ms > 400 {
                break;
            }
            ms += 10;
            std::thread::sleep(Duration::from_millis(3));
        }
        row.unwrap()
    }

    #[test]
    fn cached_evaluation_shows_depth_and_a_line_checked_by_the_rules_core() {
        let env = env(true);
        env.server
            .route("/api/cloud-eval", fixture("lichess-cloud-eval-start.http"));
        let mut driver = CloudDriver::default();
        let base = CanonicalPosition::start();
        let CloudRow::Ready(view) = cloud_pump(&mut driver, &env, &base) else {
            panic!("esperava a avaliação");
        };
        assert_eq!(view.depth, 75);
        assert_eq!(view.san[0], "e4");
        assert_eq!(view.san[8], "O-O", "e1h1 do Lichess vira roque pelo núcleo");
        assert_eq!(view.origin.provider, "Lichess");
    }

    #[test]
    fn no_cached_evaluation_is_a_normal_state_and_off_or_locked_send_nothing() {
        let env = env(true);
        env.server
            .route("/api/cloud-eval", FakeResponse::status(404));
        let mut driver = CloudDriver::default();
        let base = CanonicalPosition::start();
        assert_eq!(cloud_pump(&mut driver, &env, &base), CloudRow::NoEvaluation);

        let off = self::env(false);
        let mut driver = CloudDriver::default();
        assert_eq!(
            cloud_pump(&mut driver, &off, &base),
            CloudRow::Notice(Notice::ProviderOff(ProviderId::Lichess))
        );
        assert!(off.server.requests().is_empty());

        let mut driver = CloudDriver::default();
        let row = driver.drive(
            &base,
            env.t0 + Duration::from_secs(5),
            T0,
            Some(Lock::Competitive),
            &env.service,
        );
        assert_eq!(row, CloudRow::Notice(Notice::FairPlay));
        assert_eq!(driver.requests_started(), 0);
    }
}
