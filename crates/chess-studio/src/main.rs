//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Neste marco (Gate A) há cena com assets CC0, três Apresentações, câmera e picking: nenhuma
//! lógica de jogo. `--screenshot <dir>` gera as capturas de evidência e encerra;
//! `--benchmark` liga o modo benchmark (ver `benchmark`).

mod benchmark;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bench = match benchmark::parse_args(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Erro: {e}");
            std::process::exit(2);
        }
    };
    let capture = screenshot_dir();
    let no_vsync = bench.is_some() && args.iter().any(|a| a == "--no-vsync");

    let (resolution, present_mode) = if let Some(cfg) = bench.as_ref() {
        // Fator de escala 1.0 no benchmark: a resolução física é exatamente a pedida.
        let res = WindowResolution::new(cfg.width, cfg.height).with_scale_factor_override(1.0);
        let mode = if no_vsync {
            PresentMode::AutoNoVsync
        } else {
            PresentMode::AutoVsync
        };
        (res, mode)
    } else if capture.is_some() {
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
    if let Some(config) = bench {
        app.add_plugins(benchmark::BenchmarkPlugin {
            config,
            vsync: !no_vsync,
        });
    }
    app.run();
}
