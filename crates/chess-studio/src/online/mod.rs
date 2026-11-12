//! Partida online no Lichess (#233, ADR-0007, CX-15) e no FICS como visitante (#254): o
//! serviço ([`service`], com a parte do FICS em [`fics`]), a ligação com o Bevy e a interface
//! ([`view`], com a tela do FICS em [`fics_view`]). O tabuleiro e o Painel são os mesmos para
//! os dois Provedores: falam com a Partida pela sessão abstrata (`service::GameSession`).
//!
//! A cada quadro, depois dos gestos ([`drive_online`]):
//!
//! 1. acompanha a Conexão de jogador autorizada (`NetService::authorized_connection`);
//! 2. envia as intenções do tabuleiro (Lance, empate, abandono…) à sessão da Partida;
//! 3. recolhe o que o Provedor mandou e grava no banco só o que ele confirmou;
//! 4. põe no tabuleiro a Partida que começou e espelha a que está nele;
//! 5. informa a Trava de fair play do app das Partidas em andamento.
//!
//! Nada aqui escreve a Partida (ADR-0001): a sessão do Provedor é o controlador.

pub mod demo;
pub mod fics;
pub mod fics_view;
pub mod service;
pub mod view;

#[cfg(test)]
mod fics_tests;
#[cfg(test)]
mod tests;

use crate::camera::CameraRig;
use crate::dock::{PanelAccess, PanelDef};
use crate::fair_play::FairPlay;
use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use chess_providers::{BoardError, LinkStatus, ProviderError};
use service::{GameKey, OnlineNotice, OnlinePlay, Provider, StoreAccess};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Id do Painel Partida online no dock.
pub const PANEL_ID: &str = crate::dock::ids::ONLINE;

/// O serviço da Partida online como recurso do Bevy (o `Mutex` só torna o recurso `Sync`:
/// as consultas da rede não são; nunca há disputa).
#[derive(Resource, Default)]
pub struct OnlineService(pub Mutex<OnlinePlay>);

impl OnlineService {
    pub fn new() -> Self {
        Self(Mutex::new(OnlinePlay::new()))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut OnlinePlay) -> R) -> R {
        let mut play = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut play)
    }
}

/// Aba da tela da Partida online.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Seek,
    Challenge,
    Engine,
    Playing,
}

/// Um aviso na tela, com hora para sumir.
#[derive(Clone, Debug)]
pub struct Shown {
    pub notice: OnlineNotice,
    pub at: Instant,
}

/// Quanto tempo um aviso fica no Painel.
pub const NOTICE_TTL: Duration = Duration::from_secs(8);

/// Estado da interface da Partida online.
#[derive(Resource, Debug, Default)]
pub struct OnlineUi {
    pub open: bool,
    pub tab: Tab,
    pub form: view::Form,
    /// Confirmação de abandono aberta.
    pub confirm_resign: bool,
    /// Avisos recentes (o mais novo por último).
    pub notices: Vec<Shown>,
    /// Desafios recebidos já vistos (o aviso discreto não volta para eles).
    pub seen_challenges: Vec<String>,
    /// A oferta de atualizar as Partidas em andamento ao abrir o app já foi respondida.
    pub offer_answered: bool,
    /// Último erro de um pedido iniciado pela tela (aparece nela).
    pub error: Option<service::Unavailable>,
    /// Cena de demonstração das capturas (dados fictícios; nenhum pedido sai).
    pub demo: Option<demo::Scene>,
    /// O Provedor escolhido na tela (Lichess ou FICS como visitante).
    pub provider: Provider,
    /// Aba da tela do FICS.
    pub fics_tab: fics_view::FicsTab,
    /// O que o jogador preenche na tela do FICS.
    pub fics_form: fics_view::FicsForm,
    /// "Sair do FICS" com a Partida em andamento pede confirmação (o FICS a dá como perdida).
    pub fics_confirm_leave: bool,
    /// Último erro de um pedido ao FICS iniciado pela tela.
    pub fics_error: Option<service::Unavailable>,
    /// "Encerrar sem resultado" pedido para esta Partida do banco (FC-22); o quadro da Partida
    /// online o faz (precisa do banco e do tabuleiro).
    pub close_request: Option<chess_store::GameId>,
    /// Desafios do FICS já vistos (o aviso discreto não volta para eles).
    pub seen_fics_challenges: Vec<String>,
}

impl OnlineUi {
    /// Abre a tela numa aba.
    pub fn open_at(&mut self, tab: Tab) {
        self.open = true;
        self.tab = tab;
        self.error = None;
    }

    /// Abre a tela do FICS numa aba.
    pub fn open_fics(&mut self, tab: fics_view::FicsTab) {
        self.open = true;
        self.provider = Provider::Fics;
        self.fics_tab = tab;
        self.fics_error = None;
    }

    pub fn push(&mut self, notice: OnlineNotice) {
        self.notices.push(Shown {
            notice,
            at: Instant::now(),
        });
        if self.notices.len() > 4 {
            self.notices.remove(0);
        }
    }

    /// Os avisos ainda na tela.
    pub fn recent(&self) -> impl Iterator<Item = &OnlineNotice> {
        self.notices
            .iter()
            .filter(|s| s.at.elapsed() < NOTICE_TTL)
            .map(|s| &s.notice)
    }
}

pub struct OnlinePlugin;

impl Plugin for OnlinePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OnlineService::new())
            .init_resource::<OnlineUi>()
            .add_systems(
                Update,
                (
                    demo::start_tour,
                    demo::run_tour,
                    demo::apply_demo,
                    demo::show_demo_panel,
                    drive_online,
                )
                    .chain()
                    .after(crate::play::PlaySet),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (view::online_dialog, view::resign_dialog, view::toasts).chain(),
            );
        crate::dock::register_panel(
            app,
            PanelDef {
                id: PANEL_ID,
                label_key: "dock-panel-online",
                access: PanelAccess::Always,
                ui: view::online_panel,
            },
        );
    }
}

/// Mostra o Painel Partida online. Fechado, ou ainda no lugar em que o dock põe um Painel
/// novo (o último grupo, junto das Engines), ele vai para o grupo do Relógio, o mais alto do
/// dock: é onde a vez, os relógios (o Painel os mostra) e as Ações cabem sem rolar a 900 e a
/// 720 px (#272). Num lugar escolhido pelo usuário, só vai à frente.
pub fn reveal_panel(layout: &mut crate::dock::DockLayout) {
    use crate::dock::ids;
    let closed = !layout.is_visible(PANEL_ID);
    if closed {
        layout.set_visible(PANEL_ID, true);
    }
    let default_spot = layout.same_group(PANEL_ID, "Engines");
    if (closed || default_spot)
        && layout.is_visible(ids::CLOCK)
        && !layout.same_group(PANEL_ID, ids::CLOCK)
    {
        layout.move_into_group_of(PANEL_ID, ids::CLOCK);
    }
    layout.activate(PANEL_ID);
}

/// "Encerrar sem resultado" (FC-22): encerra no app a Partida `id` do FICS cujo fim o FICS não
/// confirmou, grava o encerramento e põe a Partida encerrada no tabuleiro, se é a dele.
pub fn close_unconfirmed(
    play: &mut OnlinePlay,
    session: &mut Session,
    store: &mut chess_store::Store,
    id: chess_store::GameId,
    now: chess_net::Timestamp,
) -> Result<(), service::Unavailable> {
    let game = play.fics.close_without_result(store, id, now)?;
    session.mirror_online(&game, false);
    Ok(())
}

/// O quadro da Partida online. Nunca bloqueia.
#[allow(clippy::too_many_arguments)]
pub fn drive_online(
    service: Res<OnlineService>,
    net: Option<Res<NetService>>,
    fair: Option<Res<FairPlay>>,
    persistence: Option<Res<PersistenceHandle>>,
    mut session: ResMut<Session>,
    mut ui: ResMut<OnlineUi>,
    rig: Option<ResMut<CameraRig>>,
    profiles: Option<Res<crate::local_profiles::LocalProfiles>>,
    dock: Option<ResMut<crate::dock::Dock>>,
    mut profile: Local<Option<chess_store::ProfileId>>,
) {
    if ui.demo.is_some() {
        return;
    }
    let Some(net) = net else {
        return;
    };
    let mut rig = rig;
    let active = profiles
        .as_deref()
        .and_then(crate::local_profiles::LocalProfiles::active);
    let switched = *profile != active;
    *profile = active;
    service.with(|play| {
        // Outro Perfil local: outras Partidas no banco (a trava as relê).
        if switched {
            play.mark_db_dirty();
        }
        play.follow_connection(&net);
        if let Some(id) = ui.close_request.take() {
            let now = net.clock().now();
            let closed = match persistence.as_deref().and_then(|h| h.0.lock().ok()) {
                Some(mut guard) => {
                    let (store, _) = guard.store_and_profile();
                    close_unconfirmed(play, &mut session, store, id, now)
                }
                None => Err(service::Unavailable::Store(String::new())),
            };
            if let Err(error) = closed {
                // O Painel mostra o aviso; a tela do FICS, o erro.
                match &error {
                    service::Unavailable::Fics(e) => {
                        ui.push(OnlineNotice::Fics(fics::FicsInfo::Refused(e.clone())));
                    }
                    service::Unavailable::Store(e) => ui.push(OnlineNotice::StoreFailed(e.clone())),
                    _ => {}
                }
                ui.fics_error = Some(error);
            }
        }
        let shown = step(
            play,
            &mut session,
            persistence.as_deref(),
            rig.as_deref_mut(),
        );
        // A Partida online que vai ao tabuleiro traz o Painel dela; a tela de busca fecha (a
        // Partida é o que importa agora).
        if shown {
            ui.open = false;
            if let Some(mut dock) = dock {
                reveal_panel(&mut dock.layout);
            }
        }
        for notice in play.take_notices() {
            ui.push(notice);
        }
        if let Some(fair) = fair.as_deref() {
            fair.follow_games(play.locked_games());
        }
    });
}

/// A Partida online do tabuleiro (Provedor e id), se há uma.
pub fn board_key(session: &Session) -> Option<GameKey> {
    GameKey::of(session.game())
}

/// Envia à sessão da Partida do tabuleiro, de qualquer Provedor, o que o jogador pediu.
pub fn route_intents(play: &mut OnlinePlay, session: &mut Session) {
    if let Some(key) = board_key(session) {
        for intent in session.take_online_intents() {
            play.apply_to(&key, intent);
        }
    }
}

/// Um quadro do serviço com o tabuleiro: intenções, rede, banco, tabuleiro. Separado do
/// sistema para os testes o chamarem sem Bevy.
pub fn step(
    play: &mut OnlinePlay,
    session: &mut Session,
    persistence: Option<&PersistenceHandle>,
    rig: Option<&mut CameraRig>,
) -> bool {
    route_intents(play, session);
    let frame = match persistence.and_then(|h| h.0.lock().ok()) {
        Some(mut guard) => {
            let (store, profile) = guard.store_and_profile();
            play.frame(Some(StoreAccess { store, profile }))
        }
        None => play.frame(None),
    };
    after_frame(play, session, frame, rig)
}

/// Depois do quadro do serviço: a Partida nova vai ao tabuleiro e a do tabuleiro é espelhada.
/// Diz se uma Partida foi ao tabuleiro.
pub fn after_frame(
    play: &mut OnlinePlay,
    session: &mut Session,
    frame: service::Frame,
    rig: Option<&mut CameraRig>,
) -> bool {
    let mut shown = false;
    if let Some(key) = frame.show
        && let Some(live) = play.session_for(&key)
        && let Some(game) = live.game()
    {
        session.show_online(
            game.clone(),
            live.store_id(),
            live.link() != LinkStatus::Closed && !live.desynced(),
        );
        shown = true;
        if let Some(rig) = rig {
            let view = session.config().initial_view();
            rig.pose = view.pose();
            rig.view = Some(view);
        }
    }
    let key = board_key(session);
    if let Some(key) = &key {
        match play
            .session_for(key)
            .and_then(|l| l.game().map(|g| (g, l.link(), l.desynced())))
        {
            Some((game, link, desynced)) => {
                // Dessincronizada, a Partida não aceita Lance local (nada é inventado).
                let live = link != LinkStatus::Closed && !desynced;
                session.mirror_online(game, live);
            }
            None => session.set_online_live(false),
        }
    }
    let lichess = key
        .as_ref()
        .filter(|k| k.provider == Provider::Lichess)
        .map(|k| k.remote_id.as_str());
    play.follow_board(lichess);
    shown
}

/// Um erro do Provedor em linguagem de usuário (nunca código HTTP nem termos técnicos).
pub fn provider_error_text(i18n: &crate::i18n::I18n, error: &ProviderError) -> String {
    match error {
        ProviderError::RateLimited { .. } => i18n.t("online-error-rate-limited"),
        ProviderError::ConnectionRejected { .. } => i18n.t("online-error-rejected"),
        ProviderError::Disabled { .. } => i18n.t("online-error-disabled"),
        ProviderError::SignInRequired { .. } => i18n.t("online-error-sign-in"),
        ProviderError::UnknownName { name, .. } => {
            i18n.t_with("online-error-unknown-name", &[("name", name)])
        }
        ProviderError::VariantRefused { .. } => i18n.t("online-error-variant"),
        ProviderError::InvalidInput { reason } => {
            i18n.t_with("online-error-invalid", &[("why", reason)])
        }
        ProviderError::Net(chess_net::NetError::Unreachable { .. })
        | ProviderError::Net(chess_net::NetError::Timeout { .. }) => i18n.t("online-error-offline"),
        _ => i18n.t("online-error-generic"),
    }
}

/// Uma recusa da Board API em linguagem de usuário.
pub fn board_error_text(i18n: &crate::i18n::I18n, error: &BoardError) -> String {
    match error {
        BoardError::TooFast => i18n.t("online-error-too-fast"),
        BoardError::SeekInProgress => i18n.t("online-error-seek-in-progress"),
        BoardError::EngineConnection => i18n.t("online-error-engine-connection"),
        BoardError::UnsupportedVariant(_) => i18n.t("online-error-variant"),
        BoardError::NotAllowed(why) => i18n.t_with("online-error-not-allowed", &[("why", why)]),
        BoardError::Provider(error) => provider_error_text(i18n, error),
        _ => i18n.t("online-error-generic"),
    }
}
