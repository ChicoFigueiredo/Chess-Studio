//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Partida a dois no mesmo computador (Assistido): o intérprete de gestos (`chess_input`)
//! traduz ponteiro em Comandos, a `Session` os envia à Partida e a cena é reconstruída do
//! `snapshot()`. `--screenshot <dir>` gera as capturas de evidência e encerra (com
//! `--moves`, `--select`, `--panel`, `--aids physical|training|ghosts` e `--review <n>` opcionais); `--benchmark` liga o modo benchmark.

mod aids;
mod aids_panel;
mod analysis;
mod asset_dir;
mod benchmark;
mod board;
mod board2d;
mod camera;
mod capture;
mod captured;
mod clock;
mod controls;
mod dock;
mod engine;
mod engine_consent;
mod engine_manager;
mod engine_wizard;
mod fen_pgn;
mod fide_panel;
mod focus;
mod framing;
mod highlight;
mod hud;
mod hud_draw;
mod hud_policy;
mod i18n;
mod layout;
mod library_ui;
mod load_control;
mod local_profiles;
mod new_game;
mod notation;
mod paths;
mod persistence;
mod picking;
mod pieces;
mod play;
mod presentation;
mod profiles;
mod ready;
mod responsive;
mod scoresheet;
mod session;
mod study;
mod training_ui;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_egui::EguiPlugin;
use std::path::PathBuf;

/// Valor da opção `--<nome> <valor>`.
fn option_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

/// `--screenshot <dir>`: diretório de saída das capturas.
fn screenshot_dir() -> Option<PathBuf> {
    option_value("--screenshot").map(PathBuf::from)
}

/// Roteiro da captura: `--moves e2e4,e7e5`, `--select e2` e `--panel`.
fn capture_script() -> capture::CaptureScript {
    capture::CaptureScript {
        moves: option_value("--moves")
            .map(|m| m.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
        select: option_value("--select").and_then(|s| s.parse().ok()),
        show_panel: std::env::args().any(|a| a == "--panel") || option_value("--profile").is_some(),
        profile: option_value("--profile"),
        // Vazamento único na partida do processo de captura: o nome vive até o fim.
        single: option_value("--single").map(|s| &*Box::leak(s.into_boxed_str())),
        aids: option_value("--aids"),
        hud: option_value("--hud"),
        inverted: std::env::args().any(|a| a == "--inverted"),
        review: option_value("--review").and_then(|s| s.parse().ok()),
        config: capture::parse_config(
            option_value("--policy").as_deref(),
            option_value("--time").as_deref(),
            option_value("--side").as_deref(),
        ),
        show_new_game: std::env::args().any(|a| a == "--new-game"),
    }
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
        // `--size 800x600` captura numa janela pequena (C4, UI-10).
        let (w, h) = option_value("--size")
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?))
            })
            .unwrap_or((1920, 1080));
        (WindowResolution::new(w, h), PresentMode::AutoNoVsync)
    } else {
        (WindowResolution::new(1280, 720), PresentMode::AutoVsync)
    };

    // Banco do usuário (`--db <arquivo>` troca o caminho). Capturas e benchmark não gravam.
    let db_path = if capture.is_some() || bench.is_some() {
        None
    } else {
        persistence::database_location(option_value("--db").map(PathBuf::from))
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: i18n::I18n::pt_br().t("app-title"),
                    resolution,
                    present_mode,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_dir::current(),
                ..default()
            }),
    )
    .add_plugins(EguiPlugin::default())
    .add_plugins((
        i18n::I18nPlugin,
        persistence::PersistencePlugin { db_path },
        aids::AidsPlugin,
        presentation::PresentationPlugin,
        board::BoardPlugin,
        pieces::PiecesPlugin,
        play::PlayPlugin,
        highlight::HighlightPlugin,
        board2d::Board2dPlugin,
        camera::CameraPlugin,
        picking::PickingPlugin,
        ui::UiPlugin,
        dock::DockPlugin,
        focus::FocusPlugin,
    ))
    .add_plugins((
        aids_panel::AidsPanelPlugin,
        clock::ClockPlugin,
        new_game::NewGamePlugin,
        capture::CapturePlugin,
        ready::ScenePlugin,
        engine::EnginePlugin,
        notation::NotationPlugin,
        captured::CapturedPlugin,
        study::StudyPlugin,
        fide_panel::ArbitrationPlugin,
        analysis::AnalysisPlugin,
        profiles::ProfilesPlugin,
        responsive::ResponsivePlugin,
        controls::ControlsPlugin,
        local_profiles::LocalProfilesPlugin,
    ));
    app.add_plugins((hud::HudPlugin, hud_draw::HudDrawPlugin));
    app.add_plugins(training_ui::TrainingPlugin);
    app.add_plugins(library_ui::LibraryPlugin);
    app.add_plugins(scoresheet::ScoresheetPlugin);
    if let Some(dir) = capture {
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("não foi possível criar {}: {err}", dir.display());
        }
        app.insert_resource(capture::CaptureMode::new(dir, capture_script()));
        // C12: telas e qualidades para as evidências.
        if std::env::args().any(|a| a == "--controls") {
            app.insert_resource(controls::ControlsUi::opened());
        }
        let picker = std::env::args().any(|a| a == "--profile-picker");
        let manager = std::env::args().any(|a| a == "--profile-manager");
        if picker || manager {
            app.insert_resource(local_profiles::LocalProfiles::sample(picker, manager));
        }
        match option_value("--quality").as_deref() {
            Some("economic") => app.insert_resource(presentation::GraphicsQuality::Economic),
            Some("balanced") => app.insert_resource(presentation::GraphicsQuality::Balanced),
            Some("ultra") => app.insert_resource(presentation::GraphicsQuality::Ultra),
            _ => &mut app,
        };
        // `--focus`: a evidência do modo Foco (sem banco, o estado não vem das preferências).
        if std::env::args().any(|a| a == "--focus") {
            app.insert_resource(focus::ViewState {
                focus: true,
                fullscreen: false,
            });
        }
    }
    if let Some(config) = bench {
        // `--quality` do benchmark: o recurso que `apply_quality` aplica a todas as câmeras.
        if let Some(quality) = config.quality {
            app.insert_resource(quality);
        }
        app.add_plugins(benchmark::BenchmarkPlugin {
            config,
            vsync: !no_vsync,
        });
    }
    // Falha de prontidão (B22) e demais erros viram código de saída diferente de zero.
    if let AppExit::Error(code) = app.run() {
        std::process::exit(i32::from(code.get()));
    }
}
