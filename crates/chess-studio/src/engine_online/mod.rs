//! Engine online no Lichess (#252, CX-13): uma Engine local instalada responde a desafios pela
//! Bot API, com a Conexão de **Engine** da Central. O serviço ([`service`]), a ligação com o
//! Bevy (este módulo) e a tela ([`view`]).
//!
//! Nada inicia sozinho: a tela abre pela Central (ou pelo passeio das capturas) e a Engine só
//! liga no clique de "Ligar a Engine online". Reabrir o app volta a `Parada`.
//!
//! A cada quadro ([`drive_engine_online`]) o serviço consulta o fluxo de eventos, responde aos
//! desafios pela política do usuário, avança as sessões das Partidas e grava no banco só o que
//! o Lichess confirmou. Nada aqui escreve a Partida (ADR-0001): a sessão do Provedor é o
//! controlador.

pub mod demo;
pub mod exit;
pub mod service;
pub mod view;

#[cfg(test)]
mod tests;

use crate::connections::ConnectionsUi;
use crate::engine_manager::{DbLocation, EngineManager};
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::online::service::StoreAccess;
use crate::persistence::PersistenceHandle;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_net::ProviderId;
use chess_providers::IrreversibleConfirmation;
use service::{EngineOnline, EngineSpec, Unavailable, uci_factory};
use std::sync::Mutex;
use view::{ConversionView, Form, Intent, ViewModel};

/// O serviço da Engine online como recurso do Bevy (o `Mutex` só torna o recurso `Sync`: as
/// consultas da rede não são; nunca há disputa).
#[derive(Resource, Default)]
pub struct EngineOnlineService(pub Mutex<EngineOnline>);

impl EngineOnlineService {
    pub fn with<R>(&self, f: impl FnOnce(&mut EngineOnline) -> R) -> R {
        let mut service = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut service)
    }
}

/// Estado da tela.
#[derive(Resource, Default)]
pub struct EngineOnlineUi {
    pub open: bool,
    pub form: Form,
    /// O último pedido da tela que não pôde nem começar.
    pub error: Option<Unavailable>,
    /// Cena de demonstração das capturas (dados fictícios; nenhum pedido sai).
    pub demo: Option<demo::Scene>,
}

pub struct EngineOnlinePlugin;

impl Plugin for EngineOnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineOnlineService>()
            .init_resource::<exit::ExitPrompt>()
            .init_resource::<EngineOnlineUi>()
            .add_systems(
                Update,
                (
                    demo::start_tour,
                    demo::run_tour,
                    follow_central,
                    drive_engine_online,
                )
                    .chain(),
            )
            .add_systems(Update, exit::guard_close)
            .add_systems(
                EguiPrimaryContextPass,
                (engine_online_window, exit::exit_prompt_window),
            );
    }
}

/// O pedido da Central ("Abrir a tela da Engine online") abre a tela e fecha os bastidores.
fn follow_central(
    central: Option<ResMut<ConnectionsUi>>,
    mut ui: ResMut<EngineOnlineUi>,
    ribbon: Option<ResMut<crate::ribbon::RibbonState>>,
) {
    let Some(mut central) = central else {
        return;
    };
    if std::mem::take(&mut central.auth.open_engine_online) {
        ui.open = true;
        ui.error = None;
        if let Some(mut ribbon) = ribbon {
            ribbon.backstage = None;
        }
    }
}

/// O quadro da Engine online. Nunca bloqueia.
fn drive_engine_online(
    service: Res<EngineOnlineService>,
    persistence: Option<Res<PersistenceHandle>>,
    ui: Res<EngineOnlineUi>,
) {
    if ui.demo.is_some() {
        return;
    }
    service.with(|run| {
        // Parada e sem nada pendente: nem toca o banco.
        if !run.is_running() && run.conversion().is_none() {
            return;
        }
        match persistence.as_deref().and_then(|h| h.0.lock().ok()) {
            Some(mut guard) => {
                let (store, profile) = guard.store_and_profile();
                run.frame(Some(StoreAccess { store, profile }));
            }
            None => run.frame(None),
        }
    });
}

/// As Engines que podem jogar: as do gerenciador, já registradas e liberadas.
pub fn engines_from(manager: &EngineManager) -> Vec<EngineSpec> {
    crate::analysis::choices_from(manager)
        .into_iter()
        .map(|c| EngineSpec {
            name: c.name,
            path: c.path,
            options: c.options,
        })
        .collect()
}

impl ViewModel {
    /// O que a tela mostra, a partir do serviço e da Central.
    pub fn of(run: &EngineOnline, net: &NetService, engines: Vec<EngineSpec>) -> Self {
        Self {
            connected: net
                .authorized_connection(ProviderId::Lichess, chess_store::ConnectionRole::Engine)
                .is_some(),
            provider_on: net.is_enabled(ProviderId::Lichess),
            engines,
            state: run.state(),
            stop_reason: run.stop_reason().cloned(),
            running_engine: run.engine().map(|e| e.name.clone()),
            rows: run.rows(),
            activity: run.activity().cloned().collect(),
            counts: run.counts(),
            conversion: ConversionView::of(run.conversion()),
        }
    }
}

/// Aplica uma intenção da tela. Separado do sistema para os testes o chamarem sem Bevy.
pub fn apply_intent(
    intent: Intent,
    ui: &mut EngineOnlineUi,
    run: &mut EngineOnline,
    net: &NetService,
    engines: &[EngineSpec],
    open_center: &mut dyn FnMut(),
) {
    match intent {
        Intent::Close => ui.open = false,
        Intent::OpenCenter => {
            ui.open = false;
            open_center();
        }
        Intent::Start => {
            let spec = ui
                .form
                .engine
                .as_ref()
                .and_then(|path| engines.iter().find(|e| &e.path == path))
                .cloned();
            ui.error = match spec {
                None => None,
                Some(spec) => run
                    .start(
                        net,
                        spec,
                        ui.form.settings.clone(),
                        uci_factory(ui.form.settings.elo),
                    )
                    .err(),
            };
        }
        Intent::Stop => run.stop(),
        Intent::StopNow => run.stop_now(),
        Intent::Upgrade => {
            // A confirmação só nasce aqui, depois do aviso e do "Entendi" marcado na tela.
            ui.error = run
                .upgrade_to_bot(net, IrreversibleConfirmation::user_confirmed())
                .err();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn engine_online_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<EngineOnlineUi>,
    service: Res<EngineOnlineService>,
    net: Option<Res<NetService>>,
    manager: Option<ResMut<EngineManager>>,
    db: Option<Res<DbLocation>>,
    ribbon: Option<ResMut<crate::ribbon::RibbonState>>,
) -> Result {
    if !state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut engines = Vec::new();
    if let Some(mut manager) = manager {
        manager.ensure_loaded(db.map(|d| d.0.clone()));
        engines = engines_from(&manager);
    }
    let model = match state.demo.as_ref() {
        Some(scene) => scene.model.clone(),
        None => {
            let Some(net) = net.as_deref() else {
                return Ok(());
            };
            service.with(|run| ViewModel::of(run, net, engines.clone()))
        }
    };
    // Sem escolha ainda, a primeira Engine da lista (nunca liga sozinho).
    if state.form.engine.is_none() {
        state.form.engine = model.engines.first().map(|e| e.path.clone());
    }
    let mut intents = Vec::new();
    let mut form = state.form.clone();
    let error = state.error.clone();
    egui::Window::new(i18n.t("engine-online-title"))
        .id(egui::Id::new("engine-online-window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            let height = (ctx.viewport_rect().height() - 400.0).max(240.0);
            egui::ScrollArea::vertical()
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    view::body(
                        ui,
                        &ctx,
                        &i18n,
                        &mut form,
                        &model,
                        error.as_ref(),
                        &mut intents,
                    );
                });
        });
    state.form = form;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        intents.push(Intent::Close);
    }
    if state.demo.is_some() {
        return Ok(());
    }
    let (Some(net), mut ribbon) = (net, ribbon) else {
        return Ok(());
    };
    for intent in intents {
        service.with(|run| {
            apply_intent(intent, &mut state, run, &net, &engines, &mut || {
                if let Some(ribbon) = ribbon.as_mut() {
                    ribbon.backstage = Some(crate::ribbon::BackstagePage::Connections);
                }
            });
        });
    }
    Ok(())
}
