//! Painel egui: Apresentação, qualidade, Vista, estado da Partida (lado a jogar, último
//! Lance em SAN, resultado), ações (Desfazer/Refazer/Abandonar/Nova Partida), método de
//! movimento e seletor de promoção. Todo texto vem do i18n (Fluent, pt-BR).

use crate::camera::CameraRig;
use crate::capture::CaptureMode;
use crate::engine::{EngineUi, engine_section};
use crate::i18n::I18n;
use crate::new_game::NewGameDialog;
use crate::picking::HoveredSquare;
use crate::presentation::{GpuSupport, GraphicsQuality, Presentation, UltraEffects};
use crate::session::Session;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPrimaryContextPass, PrimaryEguiContext, egui,
};
use chess_core::{Color as Side, Piece, Square};
use chess_game::{Command, EndReason, GameResult, Outcome};
use chess_input::{CameraView, MoveMethod};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera).add_systems(
            EguiPrimaryContextPass,
            (install_fonts, info_panel, promotion_selector).chain(),
        );
    }
}

/// Fonte da interface: Noto Sans (SIL OFL 1.1), com os acentos do pt-BR. As fontes
/// embutidas do egui usam a Ubuntu Font Licence, fora da allowlist (ADR-0004).
fn install_fonts(mut contexts: EguiContexts, mut done: Local<bool>) -> Result {
    if *done {
        return Ok(());
    }
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "noto-sans".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSans-Regular.ttf"
        ))),
    );
    // Glifos de xadrez (U+265A..265F) para a bandeja do editor de posição (C11).
    fonts.font_data.insert(
        "noto-chess".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols2-chess.ttf"
        ))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.push("noto-sans".to_owned());
        list.push("noto-chess".to_owned());
    }
    contexts.ctx_mut()?.set_fonts(fonts);
    *done = true;
    Ok(())
}

/// Câmera exclusiva do egui: o painel aparece em qualquer Apresentação (3D ou 2D), mesmo
/// quando uma das câmeras de cena está inativa. A camada vazia impede que ela desenhe a cena.
pub(crate) fn spawn_ui_camera(mut commands: Commands, mut settings: ResMut<EguiGlobalSettings>) {
    settings.auto_create_primary_context = false;
    commands.spawn((
        Camera2d,
        PrimaryEguiContext,
        RenderLayers::layer(31),
        Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // MSAA igual ao das outras câmeras da janela: `presentation::apply_quality`.
    ));
}

fn square_name(i18n: &I18n, square: Option<Square>) -> String {
    square.map_or_else(|| i18n.t("square-none"), |s| s.to_string())
}

fn view_key(view: CameraView) -> &'static str {
    match view {
        CameraView::Top => "view-top",
        CameraView::Oblique => "view-oblique",
        CameraView::Standard => "view-standard",
        CameraView::Inverted => "view-inverted",
    }
}

fn move_method_key(method: MoveMethod) -> &'static str {
    match method {
        MoveMethod::TwoTaps => "move-method-two-taps",
        MoveMethod::Drag => "move-method-drag",
        MoveMethod::Both => "move-method-both",
    }
}

fn result_key(result: GameResult) -> &'static str {
    match result {
        GameResult::WhiteWins => "game-result-white",
        GameResult::BlackWins => "game-result-black",
        GameResult::Draw => "game-result-draw",
    }
}

fn reason_key(reason: EndReason) -> &'static str {
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

/// Estado das telas de Controles e de perfis locais, e o que o Ultra precisa saber.
#[derive(bevy::ecs::system::SystemParam)]
struct Extras<'w> {
    controls_ui: ResMut<'w, crate::controls::ControlsUi>,
    controls: Res<'w, crate::controls::Controls>,
    local: ResMut<'w, crate::local_profiles::LocalProfiles>,
    effects: ResMut<'w, UltraEffects>,
    support: Res<'w, GpuSupport>,
    training: Res<'w, crate::training_ui::TrainingUi>,
    scoresheet: Option<Res<'w, crate::scoresheet::ScoresheetUi>>,
    coordinates: ResMut<'w, crate::coordinates::BoardCoordinates>,
}

#[allow(clippy::too_many_arguments)]
fn info_panel(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    hovered: Res<HoveredSquare>,
    rig: Res<CameraRig>,
    mut session: ResMut<Session>,
    mut dialog: ResMut<NewGameDialog>,
    mut presentation: ResMut<Presentation>,
    mut quality: ResMut<GraphicsQuality>,
    capture: Option<Res<CaptureMode>>,
    mut engine_ui: ResMut<EngineUi>,
    aids: Res<crate::aids::AidSettings>,
    mut dock: ResMut<crate::dock::Dock>,
    registry: Res<crate::dock::PanelRegistry>,
    mut view_state: ResMut<crate::focus::ViewState>,
    mut profiles: ResMut<crate::profiles::InterfaceProfiles>,
    mut extras: Extras,
) -> Result {
    // No modo Foco a janela principal some (as janelas da Partida, como a promoção, ficam).
    if view_state.focus || capture.as_ref().is_some_and(|c| !c.show_panel) {
        return Ok(());
    }
    // Edita cópias: tomar `&mut` dos recursos todo quadro marcaria `is_changed` e faria os
    // sistemas de Apresentação e qualidade reaplicarem tudo a cada quadro.
    let mut chosen_presentation = *presentation;
    let mut chosen_quality = *quality;
    let mut chosen_view = *view_state;
    let mut chosen_effects = extras.effects.0;
    let mut chosen_coordinates = extras.coordinates.0;
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let responsive = crate::responsive::responsive_layout(
        Vec2::new(screen.width(), screen.height()),
        dock.layout.width,
    );
    let compact = responsive.compact;
    // Janela pequena: a janela principal começa recolhida, rola por dentro e nunca passa da
    // tela; Nova Partida e o relógio vivem na barra de essenciais (`responsive`).
    egui::Window::new(i18n.t("app-title"))
        .default_open(!compact)
        .max_height(responsive.main_window_max_height)
        .show(ctx, |ui| {
            let mut body = |ui: &mut egui::Ui| {
                ui.horizontal_wrapped(|ui| {
                    for p in Presentation::ALL {
                        ui.selectable_value(&mut chosen_presentation, p, i18n.t(p.label_key()));
                    }
                });
                egui::ComboBox::from_label(i18n.t("quality-label"))
                    .selected_text(i18n.t(chosen_quality.label_key()))
                    .show_ui(ui, |ui| {
                        for q in GraphicsQuality::ALL {
                            ui.selectable_value(&mut chosen_quality, q, i18n.t(q.label_key()));
                        }
                    });
                if chosen_quality == GraphicsQuality::Ultra {
                    let available = chosen_quality.bloom_for(true, *extras.support);
                    ui.add_enabled(
                        available,
                        egui::Checkbox::new(&mut chosen_effects, i18n.t("quality-effects")),
                    )
                    .on_hover_text(i18n.t("quality-effects-hint"));
                    if !available {
                        ui.small(i18n.t("quality-effects-unavailable"));
                    }
                }
                ui.separator();
                let hide_notation = extras
                    .scoresheet
                    .as_deref()
                    .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
                game_section(ui, &i18n, &mut session, &mut dialog, &aids, hide_notation);
                engine_section(ui, &i18n, &mut session, &mut engine_ui);
                ui.separator();
                crate::dock::panels_section(ui, &i18n, &mut dock, &registry, &session);
                ui.separator();
                crate::focus::view_section(ui, &i18n, &mut chosen_view);
                ui.checkbox(&mut chosen_coordinates, i18n.t("board-coordinates"))
                    .on_hover_text(i18n.t("board-coordinates-hint"));
                ui.separator();
                crate::profiles::profiles_section(ui, &i18n, &mut profiles);
                ui.separator();
                crate::local_profiles::section(
                    ui,
                    &i18n,
                    &mut extras.local,
                    extras.training.session_swapped(),
                );
                ui.separator();
                crate::controls::controls_section(ui, &i18n, &mut extras.controls_ui);
                if compact {
                    return;
                }
                ui.separator();
                ui.label(i18n.t_with(
                    "square-hover",
                    &[("square", &square_name(&i18n, hovered.0))],
                ));
                let view = rig
                    .view
                    .map_or_else(|| i18n.t("view-free"), |v| i18n.t(view_key(v)));
                ui.label(i18n.t_with("view-label", &[("name", &view)]));
                ui.separator();
                for line in crate::controls::help_lines(&i18n, extras.controls.map()) {
                    ui.small(line);
                }
            };
            if compact {
                egui::ScrollArea::vertical()
                    .max_height(responsive.main_window_max_height - 32.0)
                    .show(ui, |ui| body(ui));
            } else {
                body(ui);
            }
        });
    if chosen_presentation != *presentation {
        *presentation = chosen_presentation;
    }
    if chosen_view != *view_state {
        *view_state = chosen_view;
    }
    if chosen_quality != *quality {
        *quality = chosen_quality;
    }
    if chosen_coordinates != extras.coordinates.0 {
        extras.coordinates.0 = chosen_coordinates;
    }
    if chosen_effects != extras.effects.0 {
        extras.effects.0 = chosen_effects;
    }
    Ok(())
}

fn game_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &mut Session,
    dialog: &mut NewGameDialog,
    aids: &crate::aids::AidSettings,
    hide_notation: bool,
) {
    let snapshot = session.snapshot().clone();
    ui.strong(i18n.t("game-heading"));
    match snapshot.outcome {
        Some(outcome) => {
            ui.strong(i18n.t("game-over"));
            ui.label(outcome_text(i18n, outcome));
        }
        None => {
            ui.label(i18n.t(match snapshot.side_to_move {
                Side::White => "game-turn-white",
                Side::Black => "game-turn-black",
            }));
            // O alerta de xeque é um Auxílio visual: vive em `aids_panel`.
        }
    }
    match snapshot.history.last() {
        // Súmula manual aberta: o último lance em SAN é notação pronta (V2-SUM-01).
        Some(_) if hide_notation => ui.label(i18n.t("scoresheet-last-move-hidden")),
        Some(last) => ui.label(i18n.t_with("game-last-move", &[("san", &last.san)])),
        None => ui.label(i18n.t("game-no-moves")),
    };
    // O aviso de Lance ilegal segue a política de explicação (`aids_panel`); relógio e
    // arbitragem vivem nos painéis do dock.
    ui.horizontal(|ui| {
        if ui
            .add_enabled(snapshot.can_undo, egui::Button::new(i18n.t("button-undo")))
            .clicked()
        {
            session.undo();
        }
        if ui
            .add_enabled(snapshot.can_redo, egui::Button::new(i18n.t("button-redo")))
            .clicked()
        {
            session.command(Command::Redo);
        }
        let can_resign = snapshot.outcome.is_none() && snapshot.provisional.is_none();
        if ui
            .add_enabled(can_resign, egui::Button::new(i18n.t("button-resign")))
            .clicked()
        {
            session.resign_side_to_move();
        }
        if ui.button(i18n.t("button-new-game")).clicked() {
            // A Nova Partida abre a tela de configuração (AC-01); a atual segue até "Iniciar".
            dialog.open_from(session.config(), aids);
        }
    });
    let mut method = session.move_method();
    egui::ComboBox::from_label(i18n.t("move-method-label"))
        .selected_text(i18n.t(move_method_key(method)))
        .show_ui(ui, |ui| {
            for m in [MoveMethod::Both, MoveMethod::TwoTaps, MoveMethod::Drag] {
                ui.selectable_value(&mut method, m, i18n.t(move_method_key(m)));
            }
        });
    session.set_move_method(method);
}

/// Seletor de promoção (D/T/B/C), aberto enquanto a Partida tem promoção pendente.
/// Esc é tratado pelo intérprete de gestos (retrata o Lance, DEC-40).
fn promotion_selector(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
) -> Result {
    if session.snapshot().provisional.is_none() {
        return Ok(());
    }
    let mut choice = None;
    let competitive = session.is_competitive();
    let mut always_queen = session.always_queen();
    egui::Window::new(i18n.t("promotion-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
            ui.label(i18n.t("promotion-prompt"));
            ui.horizontal(|ui| {
                for (piece, key) in [
                    (Piece::Queen, "promotion-queen"),
                    (Piece::Rook, "promotion-rook"),
                    (Piece::Bishop, "promotion-bishop"),
                    (Piece::Knight, "promotion-knight"),
                ] {
                    if ui.button(i18n.t(key)).clicked() {
                        choice = Some(piece);
                    }
                }
            });
            if competitive {
                // Sem dama automática nas Políticas competitivas (DEC-40).
                ui.small(i18n.t("promotion-competitive-hint"));
            } else {
                ui.checkbox(&mut always_queen, i18n.t("promotion-always-queen"));
                ui.small(i18n.t("promotion-cancel-hint"));
            }
        });
    if always_queen != session.always_queen() {
        session.set_always_queen(always_queen);
    } else if let Some(piece) = choice {
        session.command(Command::ChoosePromotion { piece });
    }
    Ok(())
}
