//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Neste marco (A4) há apenas cena provisória, câmera e picking: nenhuma lógica de jogo.

mod board;
mod camera;
mod picking;
mod ui;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Chess Studio".into(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins((
            board::BoardPlugin,
            camera::CameraPlugin,
            picking::PickingPlugin,
            ui::UiPlugin,
        ))
        .run();
}
