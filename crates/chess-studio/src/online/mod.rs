//! Partida online no Lichess (#233, ADR-0007, CX-15): o serviço ([`service`]), a ligação
//! com o Bevy e a interface ([`view`]).
//!
//! A cada quadro, depois dos gestos ([`drive_online`]):
//!
//! 1. acompanha a Conexão de jogador autorizada (`NetService::authorized_connection`);
//! 2. envia as intenções do tabuleiro (Lance, empate, abandono…) à sessão da Partida;
//! 3. recolhe o que o Lichess mandou e grava no banco só o que ele confirmou;
//! 4. põe no tabuleiro a Partida que começou e espelha a que está nele;
//! 5. informa a Trava de fair play do app das Partidas em andamento.
//!
//! Nada aqui escreve a Partida (ADR-0001): a sessão do Provedor é o controlador.

pub mod demo;
pub mod service;
pub mod view;

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
use service::{OnlineNotice, OnlinePlay, StoreAccess};
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
}

impl OnlineUi {
    /// Abre a tela numa aba.
    pub fn open_at(&mut self, tab: Tab) {
        self.open = true;
        self.tab = tab;
        self.error = None;
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
                (demo::apply_demo, demo::show_demo_panel, drive_online)
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
        let shown = step(
            play,
            &mut session,
            persistence.as_deref(),
            rig.as_deref_mut(),
        );
        // A Partida online que vai ao tabuleiro traz o Painel dela.
        if shown && let Some(mut dock) = dock {
            dock.layout.set_visible(PANEL_ID, true);
            dock.layout.activate(PANEL_ID);
        }
        for notice in play.take_notices() {
            ui.push(notice);
        }
        if let Some(fair) = fair.as_deref() {
            fair.follow_games(play.locked_games());
        }
    });
}

/// Um quadro do serviço com o tabuleiro: intenções, rede, banco, tabuleiro. Separado do
/// sistema para os testes o chamarem sem Bevy.
pub fn step(
    play: &mut OnlinePlay,
    session: &mut Session,
    persistence: Option<&PersistenceHandle>,
    rig: Option<&mut CameraRig>,
) -> bool {
    if let Some(remote) = session.online_remote_id().map(str::to_owned) {
        for intent in session.take_online_intents() {
            play.apply(&remote, intent);
        }
    }
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
    if let Some(remote) = frame.show
        && let Some(live) = play.live(&remote)
        && let Some(game) = live.game()
    {
        session.show_online(
            game.clone(),
            live.store_id,
            live.link() != LinkStatus::Closed,
        );
        shown = true;
        if let Some(rig) = rig {
            let view = session.config().initial_view();
            rig.pose = view.pose();
            rig.view = Some(view);
        }
    }
    if let Some(remote) = session.online_remote_id().map(str::to_owned) {
        match play
            .live(&remote)
            .and_then(|l| l.game().map(|g| (g, l.link())))
        {
            Some((game, link)) => {
                let live = link != LinkStatus::Closed;
                session.mirror_online(game, live);
            }
            None => session.set_online_live(false),
        }
    }
    play.follow_board(session.online_remote_id());
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
