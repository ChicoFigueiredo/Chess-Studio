//! Fechar o app com Partida da Engine online em andamento (#271): em vez de sair e deixar o
//! adversário esperando, o app pergunta. As saídas são "Parar e esperar terminarem" (a Engine
//! deixa de aceitar desafios e o app segue aberto até as Partidas acabarem), "Sair mesmo
//! assim" e "Cancelar". Sem Partida em andamento, fecha direto, como sempre.
//!
//! Como a confirmação precisa vetar o fechamento, a janela não fecha mais sozinha
//! (`close_when_requested: false`): [`guard_close`] fecha as janelas ou abre a pergunta.

use super::EngineOnlineService;
use super::EngineOnlineUi;
use super::service::EngineOnline;
use crate::i18n::I18n;
use crate::ui_theme::{ButtonVariant, CsButton};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use bevy_egui::{EguiContexts, egui};
use egui_phosphor::regular as icon;

/// A pergunta de saída.
#[derive(Resource, Default, Debug)]
pub struct ExitPrompt {
    pub open: bool,
}

/// O que fazer com um pedido de fechar a janela principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// Nada em andamento: fecha.
    Close,
    /// Há Partidas da Engine online em andamento: pergunta antes.
    Ask { games: usize },
}

/// Decide pelo estado do serviço (sem Bevy, para os testes).
pub fn decide(run: &EngineOnline) -> CloseDecision {
    match run.games_in_progress() {
        0 => CloseDecision::Close,
        games => CloseDecision::Ask { games },
    }
}

/// A escolha do usuário na pergunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitChoice {
    Cancel,
    /// Para de aceitar desafios e espera as Partidas terminarem; o app segue aberto.
    StopAndWait,
    /// Sai já; as Partidas ficam para o Lichess (o relógio corre).
    LeaveNow,
}

/// Aplica a escolha; devolve `true` quando o app deve fechar agora.
pub fn apply_choice(
    choice: ExitChoice,
    prompt: &mut ExitPrompt,
    ui: &mut EngineOnlineUi,
    run: &mut EngineOnline,
) -> bool {
    prompt.open = false;
    match choice {
        ExitChoice::Cancel => false,
        ExitChoice::StopAndWait => {
            run.stop();
            // A tela da Engine mostra "Parando" e as Partidas que faltam terminar.
            ui.open = true;
            false
        }
        ExitChoice::LeaveNow => true,
    }
}

/// Pedidos de fechar janela: a principal pergunta se preciso; as outras fecham.
pub fn guard_close(
    mut requests: MessageReader<WindowCloseRequested>,
    service: Res<EngineOnlineService>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut prompt: ResMut<ExitPrompt>,
    mut commands: Commands,
) {
    for request in requests.read() {
        let is_primary = primary.get(request.window).is_ok();
        if is_primary && matches!(service.with(|run| decide(run)), CloseDecision::Ask { .. }) {
            prompt.open = true;
        } else if let Ok(mut window) = commands.get_entity(request.window) {
            window.despawn();
        }
    }
}

/// A pergunta na tela.
pub fn exit_prompt_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut prompt: ResMut<ExitPrompt>,
    mut state: ResMut<EngineOnlineUi>,
    service: Res<EngineOnlineService>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
) -> Result {
    let demo = state.demo.as_ref().filter(|s| s.exit);
    if !prompt.open && demo.is_none() {
        return Ok(());
    }
    let games = demo.map_or_else(
        || service.with(|run| run.games_in_progress()),
        |s| s.model.rows.len(),
    );
    if games == 0 {
        // As Partidas terminaram com a pergunta aberta: nada mais a perguntar.
        prompt.open = false;
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut choice = None;
    egui::Window::new(i18n.t("engine-online-exit-title"))
        .id(egui::Id::new("engine-online-exit"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.set_width(super::view::WINDOW_W * 0.8);
            ui.label(i18n.t_with("engine-online-exit-body", &[("count", &games.to_string())]));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-online-exit-wait"))
                            .icon(icon::HOURGLASS)
                            .variant(ButtonVariant::Primary),
                    )
                    .clicked()
                {
                    choice = Some(ExitChoice::StopAndWait);
                }
                if ui
                    .add(CsButton::new(&i18n.t("engine-online-cancel")))
                    .clicked()
                {
                    choice = Some(ExitChoice::Cancel);
                }
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-online-exit-leave"))
                            .icon(icon::WARNING)
                            .variant(ButtonVariant::Danger),
                    )
                    .clicked()
                {
                    choice = Some(ExitChoice::LeaveNow);
                }
            });
        });
    if choice.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(ExitChoice::Cancel);
    }
    if let Some(choice) = choice.filter(|_| state.demo.is_none()) {
        let leave = service.with(|run| apply_choice(choice, &mut prompt, &mut state, run));
        if leave && let Ok(window) = primary.single() {
            commands.entity(window).despawn();
        }
    }
    Ok(())
}
