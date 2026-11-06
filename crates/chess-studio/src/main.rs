//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Neste marco (A4) há apenas cena provisória, câmera e picking: nenhuma lógica de jogo.
//! `--benchmark` liga o modo benchmark (ver `benchmark`).

mod benchmark;
mod board;
mod camera;
mod picking;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_egui::EguiPlugin;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bench = match benchmark::parse_args(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Erro: {e}");
            std::process::exit(2);
        }
    };
    let no_vsync = bench.is_some() && args.iter().any(|a| a == "--no-vsync");
    let (width, height) = bench.as_ref().map_or((1280, 720), |c| (c.width, c.height));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Chess Studio".into(),
            // Fator de escala 1.0 no benchmark: a resolução física é exatamente a pedida.
            resolution: if bench.is_some() {
                WindowResolution::new(width, height).with_scale_factor_override(1.0)
            } else {
                WindowResolution::new(width, height)
            },
            present_mode: if no_vsync {
                PresentMode::AutoNoVsync
            } else {
                PresentMode::AutoVsync
            },
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
    ));
    if let Some(config) = bench {
        app.add_plugins(benchmark::BenchmarkPlugin {
            config,
            vsync: !no_vsync,
        });
    }
    app.run();
}
