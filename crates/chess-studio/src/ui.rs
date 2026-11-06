//! Painel egui: casa sob o cursor, casa clicada e Vista atual.

use crate::camera::CameraRig;
use crate::capture::CaptureMode;
use crate::picking::{ClickedSquare, HoveredSquare};
use crate::presentation::{GraphicsQuality, Presentation};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPrimaryContextPass, PrimaryEguiContext, egui,
};
use chess_input::{BoardSquare, CameraView};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera)
            .add_systems(EguiPrimaryContextPass, info_panel);
    }
}

/// Câmera exclusiva do egui: o painel aparece em qualquer Apresentação (3D ou 2D), mesmo
/// quando uma das câmeras de cena está inativa. A camada vazia impede que ela desenhe a cena.
fn spawn_ui_camera(mut commands: Commands, mut settings: ResMut<EguiGlobalSettings>) {
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
        Msaa::Off,
    ));
}

fn square_name(square: Option<BoardSquare>) -> String {
    square.map_or_else(|| "—".to_owned(), |s| s.to_string())
}

fn info_panel(
    mut contexts: EguiContexts,
    hovered: Res<HoveredSquare>,
    clicked: Res<ClickedSquare>,
    rig: Res<CameraRig>,
    mut presentation: ResMut<Presentation>,
    mut quality: ResMut<GraphicsQuality>,
    capture: Option<Res<CaptureMode>>,
) -> Result {
    if capture.is_some() {
        return Ok(());
    }
    egui::Window::new("Chess Studio").show(contexts.ctx_mut()?, |ui| {
        ui.horizontal(|ui| {
            for p in Presentation::ALL {
                ui.selectable_value(&mut *presentation, p, p.label());
            }
        });
        egui::ComboBox::from_label("Qualidade")
            .selected_text(quality.label())
            .show_ui(ui, |ui| {
                for q in GraphicsQuality::ALL {
                    ui.selectable_value(&mut *quality, q, q.label());
                }
            });
        ui.separator();
        ui.label(format!("Casa sob o cursor: {}", square_name(hovered.0)));
        ui.label(format!("Última casa clicada: {}", square_name(clicked.0)));
        ui.label(format!(
            "Vista: {}",
            rig.view.map_or("Livre", CameraView::label)
        ));
        ui.separator();
        ui.small("Meio+mover: órbita | Shift+meio ou direito+mover: pan");
        ui.small("Roda: zoom | 1/2/3/4: Vistas | 0: reset");
        ui.small("Tab: alterna 3D profissional, Mesa imersiva e 2D técnico");
    });
    Ok(())
}
