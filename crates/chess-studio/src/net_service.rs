//! Serviço de rede do app (CX-3, #214): o recurso Bevy **único** que possui o
//! [`chess_net::Net`]. Os outros tickets (Explorador, importação, Engines remotas, Partida
//! online) consomem a rede por aqui: `Res<NetService>` e `net()`.
//!
//! O serviço
//!
//! - abre o `rede.db` ao lado do banco das Partidas (ou um banco em memória sem banco em disco:
//!   capturas e testes);
//! - acompanha o Perfil local ativo (`Net::set_active_profile`): consentimento e cache são do
//!   Perfil local;
//! - liga a Trava de fair play ([`FairPlayGuard`]) enquanto uma Partida competitiva está em
//!   andamento; as Partidas online contra pessoa seguram a mesma trava pela sessão delas;
//! - cria o ponto único da trava no app ([`FairPlay`]), sobre a mesma `FairPlayGuard`: todo
//!   serviço que calcula (rede ou não) o consulta;
//! - resume o estado da rede para a barra de status ([`NetSummary`]);
//! - oferece [`NetService::ensure_enabled`], a porta de entrada dos recursos que precisam de um
//!   Provedor desligado: abre o diálogo "Ligar <Provedor>?" (`provider_consent`).
//!
//! Só Apresentação e serviço (ADR-0001): nada aqui toca a Partida.

use crate::fair_play::FairPlay;
use crate::local_profiles::LocalProfiles;
use crate::provider_consent::ConsentDialog;
use crate::secrets::{self, OpenedVault};
use crate::session::Session;
use bevy::prelude::*;
use chess_net::{
    Clock, ConnectionId, FairPlayHold, LocalProfileId, Net, NetConfig, NetError, NetworkStatus,
    ProviderId, SystemClock, Timestamp,
};
use chess_providers::{ChessCom, Lichess, LichessAuth};
use chess_store::ConnectionRole;
use chess_vault::{SecretVault, VaultError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Nome do arquivo do banco da rede, ao lado do banco das Partidas.
pub const NET_DB_FILE: &str = "rede.db";

/// Motivo (diagnóstico) da Trava de fair play ligada pela Partida competitiva local.
const COMPETITIVE_REASON: &str = "Partida competitiva em andamento";

/// O que a barra de status precisa saber da rede, sem Bevy nem egui.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetSummary {
    /// Provedores ligados para o Perfil local ativo.
    pub enabled: usize,
    /// Dos ligados, os que pediram pausa.
    pub limited: usize,
    /// Dos ligados, os que falharam na última tentativa.
    pub failing: usize,
    /// Pedidos em andamento ou na fila.
    pub active_requests: usize,
    /// A Trava de fair play está ligada.
    pub fair_play: bool,
}

impl NetSummary {
    pub fn from_status(status: &NetworkStatus, fair_play: bool) -> Self {
        Self {
            enabled: status.enabled_count(),
            limited: status.limited_count(),
            failing: status.failing_count(),
            active_requests: status.active_requests,
            fair_play,
        }
    }

    /// Nenhum Provedor ligado: o app está offline (o padrão, ADR-0006).
    pub fn is_offline(&self) -> bool {
        self.enabled == 0
    }

    /// Algum Provedor ligado pede atenção (limitado ou com falha).
    pub fn needs_attention(&self) -> bool {
        self.limited > 0 || self.failing > 0
    }
}

/// Resposta de [`NetService::ensure_enabled`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // API para os próximos tickets (Explorador, importação...)
pub enum Readiness {
    /// O Provedor está ligado: pode pedir.
    Ready,
    /// Estava desligado: o diálogo "Ligar <Provedor>?" foi aberto. O recurso espera a
    /// `ProviderConsentAnswered` e tenta de novo.
    Asked,
    /// O Provedor não tem serviço de rede (importação manual, informativo): não há o que ligar.
    NoNetworkService,
}

/// A leitura de fundo dos segredos do Cofre do sistema (`SecretVault::preload`): o cofre do
/// sistema pode demorar ou pedir desbloqueio ao usuário, então ela roda fora da interface e o
/// gate de rede só confia em segredos já lidos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preload {
    /// Ainda lendo.
    Loading,
    /// Terminou: os segredos estão em memória.
    Ready,
    /// O cofre do sistema não respondeu ou recusou: as autorizações não podem ser usadas agora.
    Failed(VaultError),
}

/// Dono do [`Net`] do app. Um só por aplicativo.
#[derive(Resource)]
pub struct NetService {
    net: Net,
    /// Adaptadores tipados de dados públicos (#221) sobre a mesma rede.
    lichess: Lichess,
    chesscom: ChessCom,
    /// O mesmo cliente da família do Lichess com o perfil do PlayStrategy (#253).
    playstrategy: Lichess,
    /// Em disco (`rede.db`) ou em memória.
    persistent: bool,
    /// O Cofre dos segredos das Conexões (o mesmo que o gate de rede consulta).
    vault: SecretVault,
    /// Autorizar, conferir e revogar Conexões do Lichess (usa o Cofre e a rede).
    auth: LichessAuth,
    /// Por que o Cofre real não abriu (a Central explica); `None` se abriu.
    vault_error: Option<VaultError>,
    /// A leitura de fundo do Cofre.
    preload: Arc<Mutex<Preload>>,
    /// Mantém vivo o diretório temporário do Cofre falso (capturas, testes).
    _scratch: Option<secrets::ScratchDir>,
    /// Conexões autorizadas (com segredo no Cofre) por Provedor e papel, para o Perfil local
    /// ativo. Ver [`NetService::authorized_connection`].
    authorized:
        std::sync::Mutex<std::collections::HashMap<(ProviderId, ConnectionRole), ConnectionId>>,
    /// A Trava de fair play do app, sobre a `FairPlayGuard` desta rede.
    fair_play: FairPlay,
    /// O relógio da rede (o do sistema; o falso nos testes).
    clock: Arc<dyn Clock>,
    /// A Partida competitiva local segura a trava como as Partidas online: uma
    /// [`FairPlayHold`] própria, que nunca solta a de outra Partida (e vice-versa).
    competitive_hold: Mutex<Option<FairPlayHold>>,
}

impl std::fmt::Debug for NetService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetService")
            .field("persistent", &self.persistent)
            .finish_non_exhaustive()
    }
}

impl NetService {
    /// Abre a rede com o banco em `db` (`None` ou falha ao abrir: em memória, sem persistir).
    pub fn open(db: Option<&Path>) -> Self {
        if let Some(path) = db {
            if let Some(parent) = path.parent() {
                // Falha aqui reaparece como erro do SQLite ao abrir o arquivo.
                let _ = std::fs::create_dir_all(parent);
            }
            // O Cofre real mora ao lado do banco. A leitura dos segredos roda em thread de
            // fundo: o cofre do sistema pode pedir desbloqueio e a interface nunca espera.
            let opened = secrets::open_vault(&secrets::vault_dir_beside(path));
            let config = NetConfig::at_path(path).vault(Arc::new(opened.vault.clone()));
            match Net::open(config) {
                Ok(net) => {
                    let service = Self::with_vault(net, Arc::new(SystemClock), true, opened);
                    service.start_preload();
                    return service;
                }
                Err(err) => warn!(
                    "banco da rede {} indisponível, a rede segue sem persistência: {err}",
                    path.display()
                ),
            }
        }
        Self::in_memory()
    }

    /// Rede sem banco em disco (capturas, testes): nada persiste e o Cofre é **falso** (cofre do
    /// sistema em memória, diretório temporário): nunca o cofre real do usuário.
    pub fn in_memory() -> Self {
        Self::in_memory_with(secrets::volatile_vault(
            secrets::VolatileKeyring::new(),
            "cofre-falso",
        ))
    }

    /// Como [`NetService::in_memory`], com o Cofre falso escolhido (sem cofre do sistema, com
    /// outro custo de Argon2id...).
    pub fn in_memory_with(opened: OpenedVault) -> Self {
        let config = NetConfig::in_memory().vault(Arc::new(opened.vault.clone()));
        // `in_memory` não depende de disco: só falharia com o SQLite embarcado quebrado.
        let net = Net::open(config).expect("SQLite em memória indisponível");
        Self::with_vault(net, Arc::new(SystemClock), false, opened)
    }

    /// Serviço sobre uma rede já aberta (testes com servidor falso e relógio falso). O Cofre é
    /// falso e novo; para escolher o Cofre, use [`NetService::with_vault`].
    #[cfg(test)]
    pub fn from_net(net: Net, clock: Arc<dyn Clock>, persistent: bool) -> Self {
        let opened = secrets::volatile_vault(secrets::VolatileKeyring::new(), "cofre-falso");
        Self::with_vault(net, clock, persistent, opened)
    }

    /// Serviço sobre uma rede já aberta e o Cofre dado (o mesmo que está em `NetConfig::vault`).
    pub fn with_vault(
        net: Net,
        clock: Arc<dyn Clock>,
        persistent: bool,
        opened: OpenedVault,
    ) -> Self {
        let OpenedVault {
            vault,
            scratch,
            error,
            real,
        } = opened;
        let fair_play = FairPlay::new(net.fair_play_guard().clone());
        Self {
            lichess: Lichess::new(net.clone(), clock.clone()),
            chesscom: ChessCom::new(net.clone(), clock.clone()),
            playstrategy: Lichess::playstrategy(net.clone(), clock.clone()),
            auth: LichessAuth::new(net.clone(), vault.clone(), clock.clone()),
            clock,
            competitive_hold: Mutex::new(None),
            net,
            persistent,
            vault,
            vault_error: error,
            // Só o Cofre real tem o que ler em fundo; o falso já nasce pronto.
            preload: Arc::new(Mutex::new(if real {
                Preload::Loading
            } else {
                Preload::Ready
            })),
            _scratch: scratch,
            authorized: std::sync::Mutex::default(),
            fair_play,
        }
    }

    /// O relógio da rede (o mesmo que os adaptadores usam).
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    /// O ponto único da Trava de fair play no app (o mesmo recurso [`FairPlay`] do Bevy).
    pub fn fair_play(&self) -> &FairPlay {
        &self.fair_play
    }

    /// O Cofre dos segredos das Conexões.
    pub fn vault(&self) -> &SecretVault {
        &self.vault
    }

    /// Autorizar, conferir e revogar Conexões do Lichess.
    pub fn auth(&self) -> &LichessAuth {
        &self.auth
    }

    /// Por que o Cofre real não pôde ser aberto, se não pôde.
    pub fn vault_error(&self) -> Option<&VaultError> {
        self.vault_error.as_ref()
    }

    /// Situação da leitura de fundo dos segredos.
    pub fn preload(&self) -> Preload {
        self.preload
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Lê os segredos do Cofre numa thread de fundo (cofre do sistema: pode pedir desbloqueio).
    /// Enquanto lê, [`NetService::preload`] é [`Preload::Loading`] e nenhuma Conexão vale como
    /// autorizada.
    fn start_preload(&self) {
        let (vault, state) = (self.vault.clone(), self.preload.clone());
        let work = move || {
            let outcome = match vault.preload() {
                Ok(_) => Preload::Ready,
                Err(error) => Preload::Failed(error),
            };
            *state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = outcome;
        };
        // Sem a thread, a leitura nunca termina e nada fica autorizado: falha para o lado seguro.
        let _ = std::thread::Builder::new()
            .name("chess-studio-cofre".to_owned())
            .spawn(work);
    }

    /// Os segredos já foram lidos: o gate de rede pode consultar o Cofre sem esperar o sistema.
    pub fn vault_loaded(&self) -> bool {
        self.preload() == Preload::Ready
    }

    /// Consultas públicas do Lichess (perfil, FIDE, Explorador...).
    pub fn lichess(&self) -> &Lichess {
        &self.lichess
    }

    /// Consultas públicas do Chess.com.
    pub fn chesscom(&self) -> &ChessCom {
        &self.chesscom
    }

    /// Consultas públicas do PlayStrategy (perfil e Partidas): o cliente do Lichess com o perfil
    /// do PlayStrategy. Sem Explorador, avaliação em cache nem finais.
    pub fn playstrategy(&self) -> &Lichess {
        &self.playstrategy
    }

    /// A Conexão **autorizada** (com segredo no Cofre) do Provedor, no papel dado, para o Perfil
    /// local ativo; `None` se não há. É o ponto único que os recursos que exigem autorização
    /// (o Explorador online do Lichess) consultam, a cada quadro: é barato (um mapa em
    /// memória). A Central a mantém em dia a partir do banco e do Cofre
    /// ([`NetService::set_authorized_connection`]): ao autorizar, desconectar, trocar de Perfil
    /// local e bloquear o Cofre.
    pub fn authorized_connection(
        &self,
        provider: ProviderId,
        role: ConnectionRole,
    ) -> Option<ConnectionId> {
        self.authorized
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(provider, role))
            .copied()
    }

    /// Registra (ou, com `None`, remove) a Conexão autorizada do Provedor no papel dado.
    pub fn set_authorized_connection(
        &self,
        provider: ProviderId,
        role: ConnectionRole,
        connection: Option<ConnectionId>,
    ) {
        let mut map = self
            .authorized
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match connection {
            Some(id) => map.insert((provider, role), id),
            None => map.remove(&(provider, role)),
        };
    }

    /// O `rede.db` que fica ao lado do banco das Partidas.
    pub fn db_path_beside(games_db: &Path) -> PathBuf {
        games_db.with_file_name(NET_DB_FILE)
    }

    /// A camada de rede: `fetch`, `stream`, `read_cache`, `status`...
    pub fn net(&self) -> &Net {
        &self.net
    }

    #[allow(dead_code)]
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// Resumo do estado da rede (barato: pode ser lido a cada quadro).
    pub fn summary(&self) -> NetSummary {
        NetSummary::from_status(&self.net.status(), self.fair_play.any())
    }

    pub fn is_enabled(&self, provider: ProviderId) -> bool {
        self.net.is_enabled(provider)
    }

    /// Liga ou desliga o Provedor para o Perfil local ativo (gravado).
    pub fn set_enabled(&self, provider: ProviderId, enabled: bool) -> Result<(), NetError> {
        self.net.set_enabled(provider, enabled)
    }

    /// Passa a servir o Perfil local `profile` (consentimento e cache são dele).
    pub fn follow_profile(&self, profile: chess_store::ProfileId) {
        let next = LocalProfileId(profile.0);
        if self.net.active_profile() != next {
            self.net.set_active_profile(next);
        }
    }

    /// Segura a Trava de fair play enquanto `competitive_in_progress` e solta quando acaba.
    /// É uma [`FairPlayHold`] própria, como a de cada Partida online: o fim da Partida
    /// competitiva local nunca solta a trava de uma Partida online em andamento, e o fim de uma
    /// Partida online nunca solta a da competitiva.
    pub fn follow_competitive(&self, competitive_in_progress: bool, was_engaged: &mut bool) {
        if competitive_in_progress == *was_engaged {
            return;
        }
        let mut hold = self
            .competitive_hold
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *hold =
            competitive_in_progress.then(|| self.net.fair_play_guard().hold(COMPETITIVE_REASON));
        *was_engaged = competitive_in_progress;
    }

    /// Porta de entrada dos recursos que precisam de um Provedor: devolve [`Readiness::Ready`] se
    /// ele está ligado; se não, abre o diálogo "Ligar <Provedor>?" com o que será enviado e
    /// devolve [`Readiness::Asked`]. `feature` é o nome do recurso na frase do diálogo
    /// ("O Explorador"); o recurso ouve `ProviderConsentAnswered` para tentar de novo.
    #[allow(dead_code)] // API para os próximos tickets
    pub fn ensure_enabled(
        &self,
        provider: ProviderId,
        feature: Option<&str>,
        dialog: &mut ConsentDialog,
    ) -> Readiness {
        if !provider.info().uses_network() {
            return Readiness::NoNetworkService;
        }
        if self.net.is_enabled(provider) {
            return Readiness::Ready;
        }
        dialog.ask(provider, feature.map(str::to_owned));
        Readiness::Asked
    }

    /// Instante do relógio do sistema, o mesmo que a camada de rede usa.
    pub fn now() -> Timestamp {
        SystemClock.now()
    }
}

/// Liga o [`NetService`] ao app.
pub struct NetServicePlugin;

impl Plugin for NetServicePlugin {
    fn build(&self, app: &mut App) {
        // O `DbLocation` existe quando a persistência abriu o banco; sem ele (capturas,
        // benchmark) a rede fica em memória.
        let games_db = app
            .world()
            .get_resource::<crate::engine_manager::DbLocation>()
            .map(|location| location.0.clone());
        let service = NetService::open(
            games_db
                .as_deref()
                .map(NetService::db_path_beside)
                .as_deref(),
        );
        app.insert_resource(service.fair_play().clone())
            .insert_resource(service)
            .add_systems(Update, follow_app_state);
    }
}

/// Acompanha o Perfil local ativo e a Partida competitiva.
fn follow_app_state(
    service: Res<NetService>,
    profiles: Option<Res<LocalProfiles>>,
    session: Res<Session>,
    mut engaged: Local<bool>,
) {
    if let Some(active) = profiles.as_deref().and_then(LocalProfiles::active) {
        service.follow_profile(active);
    }
    service.follow_competitive(session.competitive_in_progress(), &mut engaged);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_store::ProfileId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "chess-studio-net-service-{}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("cria diretório temporário");
            Self(dir)
        }

        fn db(&self) -> PathBuf {
            self.0.join(NET_DB_FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_fresh_service_is_offline_and_every_provider_starts_off() {
        let service = NetService::in_memory();
        let summary = service.summary();
        assert!(summary.is_offline());
        assert_eq!(summary, NetSummary::default());
        assert!(!service.is_enabled(ProviderId::Lichess));
        assert!(!service.is_persistent());
    }

    #[test]
    fn turning_a_provider_on_and_off_persists_per_local_profile() {
        let dir = TempDir::new();
        {
            let service = NetService::open(Some(&dir.db()));
            assert!(service.is_persistent());
            service.follow_profile(ProfileId(1));
            service.set_enabled(ProviderId::Lichess, true).unwrap();
            service.set_enabled(ProviderId::ChessCom, true).unwrap();
            service.set_enabled(ProviderId::ChessCom, false).unwrap();
            // Outro Perfil local não herda a escolha.
            service.follow_profile(ProfileId(2));
            assert!(!service.is_enabled(ProviderId::Lichess));
            service.set_enabled(ProviderId::Fide, false).unwrap();
        }
        let reopened = NetService::open(Some(&dir.db()));
        reopened.follow_profile(ProfileId(1));
        assert!(reopened.is_enabled(ProviderId::Lichess));
        assert!(!reopened.is_enabled(ProviderId::ChessCom));
        assert_eq!(reopened.summary().enabled, 1);
        reopened.follow_profile(ProfileId(2));
        assert!(!reopened.is_enabled(ProviderId::Lichess));
        assert!(reopened.summary().is_offline());
    }

    #[test]
    fn a_provider_without_a_network_service_cannot_be_turned_on() {
        let service = NetService::in_memory();
        assert!(matches!(
            service.set_enabled(ProviderId::Twic, true),
            Err(NetError::NoNetworkService { .. })
        ));
        assert!(!service.is_enabled(ProviderId::Twic));
    }

    #[test]
    fn the_fair_play_guard_follows_the_competitive_game_only_on_transitions() {
        let service = NetService::in_memory();
        let mut engaged = false;
        service.follow_competitive(false, &mut engaged);
        assert!(!service.net().fair_play_guard().is_engaged());
        service.follow_competitive(true, &mut engaged);
        assert!(service.net().fair_play_guard().is_engaged());
        assert!(service.summary().fair_play);
        service.follow_competitive(false, &mut engaged);
        assert!(!service.net().fair_play_guard().is_engaged());
        // Sem transição, a trava não é tocada (um gatilho futuro pode tê-la ligado).
        service.net().fair_play_guard().engage("outro gatilho");
        service.follow_competitive(false, &mut engaged);
        assert!(service.net().fair_play_guard().is_engaged());
    }

    #[test]
    fn the_local_competitive_game_and_an_online_game_never_release_each_others_lock() {
        let service = NetService::in_memory();
        let guard = service.net().fair_play_guard().clone();
        let mut engaged = false;
        // Competitiva local começa; uma Partida online contra pessoa começa por cima.
        service.follow_competitive(true, &mut engaged);
        let online = guard.hold("Partida online contra pessoa");
        // A competitiva termina: a online continua segurando.
        service.follow_competitive(false, &mut engaged);
        assert!(
            guard.is_engaged(),
            "a Partida online ainda está em andamento"
        );
        assert!(service.fair_play().everything_locked());
        // Outra competitiva começa; a online termina: a competitiva continua segurando.
        service.follow_competitive(true, &mut engaged);
        drop(online);
        assert!(
            guard.is_engaged(),
            "a competitiva local ainda está em andamento"
        );
        service.follow_competitive(false, &mut engaged);
        assert!(!guard.is_engaged(), "só cai quando as duas terminaram");
        assert!(!service.fair_play().any());
    }

    #[test]
    fn ensure_enabled_opens_the_dialog_for_an_off_provider_and_is_ready_when_on() {
        let service = NetService::in_memory();
        let mut dialog = ConsentDialog::default();
        assert_eq!(
            service.ensure_enabled(ProviderId::Lichess, Some("O Explorador"), &mut dialog),
            Readiness::Asked
        );
        assert_eq!(dialog.current(), Some(ProviderId::Lichess));
        dialog.dismiss();
        service.set_enabled(ProviderId::Lichess, true).unwrap();
        assert_eq!(
            service.ensure_enabled(ProviderId::Lichess, None, &mut dialog),
            Readiness::Ready
        );
        assert_eq!(dialog.current(), None);
        // Importação manual não tem o que ligar.
        assert_eq!(
            service.ensure_enabled(ProviderId::Twic, None, &mut dialog),
            Readiness::NoNetworkService
        );
        assert_eq!(dialog.current(), None);
    }

    #[test]
    fn the_database_sits_beside_the_games_database() {
        assert_eq!(
            NetService::db_path_beside(Path::new("/dados/ChessStudio/chess-studio.db")),
            PathBuf::from("/dados/ChessStudio/rede.db")
        );
    }
}
