//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Neste marco (A5) há cena com assets CC0, três Apresentações, câmera e picking: nenhuma
//! lógica de jogo. `--screenshot <dir>` gera as capturas de evidência e encerra.

mod board;
mod board2d;
mod camera;
mod capture;
mod layout;
mod picking;
mod presentation;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_egui::EguiPlugin;
use std::path::PathBuf;

/// `--screenshot <dir>`: diretório de saída das capturas.
fn screenshot_dir() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn main() {
    let capture = screenshot_dir();
    let (resolution, present_mode) = if capture.is_some() {
        // Evidência em 1080p, medindo sem o limite do vsync.
        (WindowResolution::new(1920, 1080), PresentMode::AutoNoVsync)
    } else {
        (WindowResolution::new(1280, 720), PresentMode::AutoVsync)
    };
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Chess Studio".into(),
            resolution,
            present_mode,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(EguiPlugin::default())
    .add_plugins((
        presentation::PresentationPlugin,
        board::BoardPlugin,
        board2d::Board2dPlugin,
        camera::CameraPlugin,
        picking::PickingPlugin,
        ui::UiPlugin,
        capture::CapturePlugin,
    ));
    if let Some(dir) = capture {
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("não foi possível criar {}: {err}", dir.display());
        }
        app.insert_resource(capture::CaptureMode::new(dir));
    }
    app.run();
}
