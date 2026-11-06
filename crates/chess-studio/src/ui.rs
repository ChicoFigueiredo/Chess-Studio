//! Painel egui: casa sob o cursor, casa clicada e Vista atual.

use crate::camera::CameraRig;
use crate::picking::{ClickedSquare, HoveredSquare};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_input::{BoardSquare, CameraView};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, info_panel);
    }
}

fn square_name(square: Option<BoardSquare>) -> String {
    square.map_or_else(|| "—".to_owned(), |s| s.to_string())
}

fn info_panel(
    mut contexts: EguiContexts,
    hovered: Res<HoveredSquare>,
    clicked: Res<ClickedSquare>,
    rig: Res<CameraRig>,
) -> Result {
    egui::Window::new("Chess Studio").show(contexts.ctx_mut()?, |ui| {
        ui.label(format!("Casa sob o cursor: {}", square_name(hovered.0)));
        ui.label(format!("Última casa clicada: {}", square_name(clicked.0)));
        ui.label(format!(
            "Vista: {}",
            rig.view.map_or("Livre", CameraView::label)
        ));
        ui.separator();
        ui.small("Meio+mover: órbita | Shift+meio ou direito+mover: pan");
        ui.small("Roda: zoom | 1/2/3/4: Vistas | 0: reset");
    });
    Ok(())
}
