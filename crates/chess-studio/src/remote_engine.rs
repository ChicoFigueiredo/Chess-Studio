//! A Engine remota na interface (#252, CX-11): o Stockfish do Chess-API.com como uma escolha
//! **explícita** na lista de Engines da análise, nunca a padrão e nunca por troca silenciosa.
//!
//! - **Escolher** a Engine remota abre um aviso claro: a Posição analisada (só o FEN, sem nomes
//!   nem histórico) sai desta máquina para o Provedor. Só depois do "Continuar" vem o
//!   consentimento por Provedor (o diálogo "Ligar Chess-API.com?", que já existe), se ele
//!   ainda estiver desligado. A escolha vale para a sessão; ao reabrir o app volta a Engine local.
//! - **Trava de fair play.** Quem consulta é a `GuardedAnalysis`, que recusa sob a trava antes
//!   de qualquer pedido; a análise some e o painel explica em linguagem de usuário.
//! - **Problemas** (Provedor desligado, limite, resposta inválida) chegam tipados e viram texto
//!   de usuário com a saída certa (ligar o Provedor, esperar o prazo, tentar de novo).
//! - **Validação.** O adaptador devolve só Lances aceitos pelo núcleo de regras (ADR-0003); o
//!   que o serviço mandar ilegal nunca chega à tela.

use crate::analysis::Analysis;
use crate::i18n::I18n;
use crate::net_service::{NetService, Readiness};
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::ui_theme::{
    ButtonVariant, StatusTone, cs_button, cs_status_badge, palette, strong_font,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_net::{ProviderId, Timestamp};
use chess_providers::{ChessApi, ProviderError};

/// O Provedor da Engine remota.
pub const PROVIDER: ProviderId = ProviderId::ChessApi;

/// Estado da Engine remota dentro da análise.
#[derive(Debug, Default)]
pub struct RemoteState {
    selected: bool,
    api: Option<ChessApi>,
    problem: Option<ProviderError>,
}

impl RemoteState {
    /// A Engine remota é a escolhida (aviso lido e Provedor ligado)?
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn select(&mut self) {
        self.selected = true;
        self.problem = None;
    }

    pub fn deselect(&mut self) {
        self.selected = false;
        self.problem = None;
    }

    /// O cliente do serviço, sobre a rede do app.
    pub fn api(&self) -> Option<&ChessApi> {
        self.api.as_ref()
    }

    pub fn set_api(&mut self, api: ChessApi) {
        self.api = Some(api);
    }

    pub fn set_problem(&mut self, problem: Option<ProviderError>) {
        self.problem = problem;
    }

    pub fn problem(&self) -> Option<&ProviderError> {
        self.problem.as_ref()
    }
}

/// O que a pessoa pode fazer a respeito de um problema da Engine remota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    /// Ligar o Provedor (reabre o consentimento).
    TurnOn,
    /// Esperar: o prazo está no texto.
    Wait,
    /// A Trava de fair play: nada a fazer até a Partida acabar.
    Locked,
    /// Tentar de novo.
    Retry,
}

/// Texto de usuário para um problema, e a saída.
pub fn problem_text(i18n: &I18n, error: &ProviderError, now: Timestamp) -> (String, Remedy) {
    match error {
        ProviderError::Disabled { .. } => (i18n.t("remote-problem-disabled"), Remedy::TurnOn),
        ProviderError::FairPlayLocked { .. } => (i18n.t("remote-problem-fairplay"), Remedy::Locked),
        ProviderError::RateLimited { until, .. } => {
            let millis = until.as_millis().saturating_sub(now.as_millis());
            let seconds = ((millis + 999) / 1000).max(1);
            (
                i18n.t_with("remote-problem-limit", &[("seconds", &seconds.to_string())]),
                Remedy::Wait,
            )
        }
        ProviderError::Malformed { .. } => (i18n.t("remote-problem-invalid"), Remedy::Retry),
        ProviderError::Net(_) => (i18n.t("remote-problem-offline"), Remedy::Retry),
        _ => (i18n.t("remote-problem-generic"), Remedy::Retry),
    }
}

// --- O fluxo: aviso, consentimento, escolha ----------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    /// O aviso "a Posição sai da máquina" está na tela.
    Warning,
    /// O diálogo de consentimento do Provedor está na tela.
    Consent,
}

/// O fluxo de escolher a Engine remota.
#[derive(Resource, Debug, Default)]
pub struct RemoteFlow {
    stage: Stage,
}

impl RemoteFlow {
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// A pessoa escolheu a Engine remota na lista: mostra o aviso. Nada é enviado nem ligado.
    pub fn request(&mut self) {
        self.stage = Stage::Warning;
    }

    pub fn cancel(&mut self) {
        self.stage = Stage::Idle;
    }

    /// "Continuar" no aviso: se o Provedor já está ligado, a Engine é escolhida; se não,
    /// abre o consentimento do Provedor.
    pub fn confirm(
        &mut self,
        net: &NetService,
        dialog: &mut ConsentDialog,
        analysis: &mut Analysis,
    ) {
        match net.ensure_enabled(PROVIDER, Some("A Engine remota"), dialog) {
            Readiness::Ready => {
                analysis.choose_remote();
                self.stage = Stage::Idle;
            }
            Readiness::Asked => self.stage = Stage::Consent,
            Readiness::NoNetworkService => self.stage = Stage::Idle,
        }
    }

    /// A resposta do consentimento: ligado, a Engine é escolhida; recusado, nada muda.
    pub fn answered(&mut self, accepted: bool, analysis: &mut Analysis) {
        if self.stage == Stage::Consent {
            if accepted {
                analysis.choose_remote();
            }
            self.stage = Stage::Idle;
        }
    }
}

pub struct RemoteEnginePlugin;

impl Plugin for RemoteEnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemoteFlow>()
            .add_systems(Update, (provide_api, take_requests, follow_consent).chain())
            .add_systems(EguiPrimaryContextPass, warning_window);
    }
}

/// Entrega à análise o cliente do serviço, sobre a rede do app. Não envia nada.
fn provide_api(net: Option<Res<NetService>>, mut analysis: ResMut<Analysis>) {
    if analysis.remote().api().is_none()
        && let Some(net) = net
    {
        analysis
            .remote_mut()
            .set_api(ChessApi::new(net.net().clone(), net.clock()));
    }
}

/// Os pedidos do painel: escolher a Engine remota na lista ou ligar o Provedor depois de um
/// problema. Os dois passam pelo aviso.
fn take_requests(mut analysis: ResMut<Analysis>, mut flow: ResMut<RemoteFlow>) {
    let ask = analysis.take_remote_ask();
    let turn_on = analysis.take_remote_turn_on();
    if (ask || turn_on) && flow.stage() == Stage::Idle {
        flow.request();
    }
}

fn follow_consent(
    mut answers: MessageReader<ProviderConsentAnswered>,
    mut flow: ResMut<RemoteFlow>,
    mut analysis: ResMut<Analysis>,
) {
    for answer in answers.read() {
        if answer.provider == PROVIDER {
            flow.answered(answer.accepted, &mut analysis);
        }
    }
}

fn warning_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut flow: ResMut<RemoteFlow>,
    net: Option<Res<NetService>>,
    mut dialog: ResMut<ConsentDialog>,
    mut analysis: ResMut<Analysis>,
) -> Result {
    if flow.stage() != Stage::Warning {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut choice = None;
    egui::Area::new(egui::Id::new("remote-engine-dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(&ctx, |ui| {
            let rect = ctx.viewport_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
        });
    egui::Window::new(i18n.t("remote-warning-title"))
        .id(egui::Id::new("remote-engine-warning"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            choice = warning_body(ui, &ctx, &i18n);
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(false);
    }
    match (choice, net) {
        (Some(true), Some(net)) => flow.confirm(&net, &mut dialog, &mut analysis),
        (Some(_), _) => flow.cancel(),
        (None, _) => {}
    }
    Ok(())
}

/// O corpo do aviso. `Some(true)` = continuar, `Some(false)` = cancelar.
pub fn warning_body(ui: &mut egui::Ui, ctx: &egui::Context, i18n: &I18n) -> Option<bool> {
    let p = palette(ui);
    ui.set_max_width(460.0);
    ui.spacing_mut().item_spacing.y = 8.0;
    cs_status_badge(
        ui,
        StatusTone::Warning,
        egui_phosphor::regular::CLOUD_ARROW_UP,
        &i18n.t("remote-badge"),
    );
    ui.label(i18n.t_with("remote-warning-lead", &[("provider", PROVIDER.info().name)]));
    ui.label(
        egui::RichText::new(i18n.t("remote-warning-sends-label"))
            .font(strong_font(ctx, 14.0))
            .color(p.ink),
    );
    ui.label(i18n.t("remote-warning-sends"));
    ui.label(
        egui::RichText::new(i18n.t("remote-warning-footer"))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
    );
    ui.add_space(8.0);
    let mut choice = None;
    ui.horizontal(|ui| {
        if cs_button(
            ui,
            &i18n.t("remote-warning-continue"),
            ButtonVariant::Primary,
        )
        .clicked()
        {
            choice = Some(true);
        }
        if cs_button(
            ui,
            &i18n.t("remote-warning-cancel"),
            ButtonVariant::Secondary,
        )
        .clicked()
        {
            choice = Some(false);
        }
    });
    choice
}

/// O selo e o lembrete permanente enquanto a Engine remota está escolhida.
pub fn active_note(ui: &mut egui::Ui, i18n: &I18n) {
    let p = palette(ui);
    cs_status_badge(
        ui,
        StatusTone::Warning,
        egui_phosphor::regular::CLOUD_ARROW_UP,
        &i18n.t("remote-badge"),
    );
    ui.label(
        egui::RichText::new(
            i18n.t_with("remote-active-note", &[("provider", PROVIDER.info().name)]),
        )
        .text_style(egui::TextStyle::Small)
        .color(p.ink_muted),
    );
}

/// Linha de problema com a saída: ligar o Provedor, esperar ou tentar de novo. Devolve o que
/// a pessoa pediu.
pub fn problem_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    error: &ProviderError,
    now: Timestamp,
) -> Option<Remedy> {
    let p = palette(ui);
    let (text, remedy) = problem_text(i18n, error, now);
    ui.colored_label(p.warning, text);
    let mut asked = None;
    match remedy {
        Remedy::TurnOn => {
            if cs_button(ui, &i18n.t("remote-act-turn-on"), ButtonVariant::Primary).clicked() {
                asked = Some(Remedy::TurnOn);
            }
        }
        Remedy::Retry | Remedy::Wait => {
            if cs_button(ui, &i18n.t("analysis-retry"), ButtonVariant::Secondary).clicked() {
                asked = Some(Remedy::Retry);
            }
        }
        Remedy::Locked => {}
    }
    asked
}

#[cfg(test)]
#[path = "remote_engine_tests.rs"]
mod tests;
