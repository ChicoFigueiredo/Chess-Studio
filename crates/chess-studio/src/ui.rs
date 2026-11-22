//! Base do egui: a câmera exclusiva da interface, o Tema e o seletor de promoção, além dos
//! textos de resultado e motivo do fim da Partida. O painel esquerdo antigo não existe mais
//! (troca única, #167): a interface é o Ribbon, o dock, a barra de status e as Opções. Todo
//! texto vem do i18n (Fluent, pt-BR).

use crate::i18n::I18n;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy_egui::{EguiGlobalSettings, EguiPrimaryContextPass, PrimaryEguiContext};
use chess_game::{EndReason, GameResult, Outcome};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::promotion_dialog::PromotionUi>()
            .add_systems(Startup, spawn_ui_camera)
            .add_systems(
                EguiPrimaryContextPass,
                (
                    crate::ui_theme::apply_appearance,
                    crate::promotion_dialog::promotion_selector,
                )
                    .chain(),
            );
    }
}

/// Câmera exclusiva do egui: o painel aparece em qualquer Apresentação (3D ou 2D), mesmo
/// quando uma das câmeras de cena está inativa. A camada vazia impede que ela desenhe a cena.
pub(crate) fn spawn_ui_camera(mut commands: Commands, mut settings: ResMut<EguiGlobalSettings>) {
    settings.auto_create_primary_context = false;
    commands.spawn((
        Camera2d,
        PrimaryEguiContext,
        // Sem tonemapping: ver `board2d::spawn_camera` (issue #156).
        Tonemapping::None,
        RenderLayers::layer(31),
        Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // MSAA igual ao das outras câmeras da janela: `presentation::apply_quality`.
    ));
}

pub(crate) fn result_key(result: GameResult) -> &'static str {
    match result {
        GameResult::WhiteWins => "game-result-white",
        GameResult::BlackWins => "game-result-black",
        GameResult::Draw => "game-result-draw",
    }
}

pub(crate) fn reason_key(reason: EndReason) -> &'static str {
    match reason {
        EndReason::Checkmate => "game-reason-checkmate",
        EndReason::Stalemate => "game-reason-stalemate",
        EndReason::InsufficientMaterial => "game-reason-insufficient-material",
        EndReason::DeadPosition => "game-reason-dead-position",
        EndReason::ThreefoldRepetition => "game-reason-threefold",
        EndReason::FivefoldRepetition => "game-reason-fivefold",
        EndReason::FiftyMoveRule => "game-reason-fifty-move",
        EndReason::SeventyFiveMoveRule => "game-reason-seventy-five-move",
        EndReason::Resignation => "game-reason-resignation",
        EndReason::DrawAgreement => "game-reason-draw-agreement",
        EndReason::Timeout => "game-reason-timeout",
        EndReason::TimeoutDraw => "game-reason-timeout-draw",
        EndReason::SecondIllegalMove => "game-reason-second-illegal",
        EndReason::SecondIllegalMoveDraw => "game-reason-second-illegal-draw",
        EndReason::EliminatedByInfraction => "game-reason-eliminated",
        EndReason::EliminatedByInfractionDraw => "game-reason-eliminated-draw",
        EndReason::Disconnection => "game-reason-disconnection",
        EndReason::EndedByProvider => "game-reason-ended-by-provider",
        _ => "game-reason-other",
    }
}

pub(crate) fn outcome_text(i18n: &I18n, outcome: Outcome) -> String {
    format!(
        "{} — {}",
        i18n.t(result_key(outcome.result)),
        i18n.t(reason_key(outcome.reason))
    )
}
