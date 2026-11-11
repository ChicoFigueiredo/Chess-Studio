//! Chess Studio: binário Bevy. Só lê o estado da Partida e envia intenções (ADR-0001).
//!
//! Partida a dois no mesmo computador (Assistido): o intérprete de gestos (`chess_input`)
//! traduz ponteiro em Comandos, a `Session` os envia à Partida e a cena é reconstruída do
//! `snapshot()`. `--demo-data` abre um Perfil local de exemplo num banco temporário, sem rede (#275). `--screenshot <dir>` gera as capturas de evidência e encerra (com
//! `--moves`, `--select`, `--panel`, `--aids physical|training|ghosts` e `--review <n>` opcionais); `--benchmark` liga o modo benchmark.

mod action_icons;
mod actions;
mod aids;
mod aids_panel;
mod analysis;
mod asset_dir;
mod benchmark;
mod board;
mod board2d;
mod board2d_panel;
mod camera;
mod capture;
mod captured;
mod clock;
mod connections;
mod controls;
mod coordinates;
mod demo_data;
mod detached;
mod dock;
mod end_game;
mod engine;
mod engine_consent;
mod engine_manager;
mod engine_online;
mod engine_wizard;
mod eval_bar;
mod explorer;
mod fair_play;
mod fen_pgn;
mod fide_panel;
mod fide_profile;
mod file_picker;
mod focus;
mod framing;
mod highlight;
mod hover;
mod hud;
mod hud_draw;
mod hud_policy;
mod i18n;
mod layout;
mod library_bulk;
mod library_ui;
mod load_control;
mod local_profiles;
mod local_time;
mod net_service;
mod new_game;
mod notation;
mod online;
mod online_import;
mod online_import_ui;
mod openings;
mod options;
mod palette;
mod panel_chrome;
mod panel_stripe;
mod paths;
mod persistence;
mod picking;
mod pieces;
mod play;
mod presentation;
mod profiles;
mod provider_consent;
mod ready;
mod remote_engine;
mod responsive;
mod ribbon;
mod scoresheet;
mod screen_layouts;
mod secrets;
mod session;
mod short_date;
mod status_bar;
mod study;
mod table;
mod training_ui;
mod ui;
mod ui_theme;

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

/// Roteiro da captura: `--moves e2e4,e7e5`, `--select e2` e `--panel` (mostra o invólucro da
/// interface: Ribbon, dock, barra de status e barra de avaliação; sem ele a captura sai limpa).
fn capture_script() -> capture::CaptureScript {
    capture::CaptureScript {
        moves: option_value("--moves")
            .map(|m| m.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
        select: option_value("--select").and_then(|s| s.parse().ok()),
        hover: option_value("--hover").and_then(|s| s.parse().ok()),
        show_panel: shell_requested(),
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

/// A captura mostra o invólucro da interface: `--panel`, `--profile` ou qualquer opção que só
/// faz sentido com ele (Ribbon, KeyTips, bastidores, Opções, Paleta, barra de avaliação).
fn shell_requested() -> bool {
    const FLAGS: [&str; 13] = [
        "--engine-online-tour",
        "--online-demo",
        "--online-tour",
        "--backstage-page",
        "--panel",
        "--keytips",
        "--backstage",
        "--options",
        "--palette",
        "--ribbon",
        "--ribbon-tab",
        "--eval-demo",
        "--layout-dialog",
    ];
    std::env::args().any(|a| FLAGS.contains(&a.as_str())) || option_value("--profile").is_some()
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
    // `--demo-data`: Perfil local de exemplo num banco temporário, sem rede (#275).
    let demo = if args.iter().any(|a| a == demo_data::FLAG) && capture.is_none() && bench.is_none()
    {
        match demo_data::DemoWorkspace::create() {
            Ok(demo) => {
                eprintln!(
                    "Dados de demonstração em {} (apagados ao sair; o banco real não é tocado)",
                    demo.dir().display()
                );
                Some(demo)
            }
            Err(err) => {
                eprintln!("Erro: não foi possível preparar os dados de demonstração: {err}");
                std::process::exit(2);
            }
        }
    } else {
        None
    };
    let db_path = if capture.is_some() || bench.is_some() {
        None
    } else {
        demo_data::resolve_db(
            demo.as_ref(),
            option_value("--db").map(PathBuf::from),
            persistence::database_location,
        )
    };

    let mut app = App::new();
    if demo.is_some() {
        app.insert_resource(demo_data::DemoMode);
    }
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                // Fechar a janela principal encerra o app mesmo com janelas destacadas abertas.
                exit_condition: bevy::window::ExitCondition::OnPrimaryClosed,
                // Quem fecha as janelas é `engine_online::exit`, que pergunta antes se há Partida
                // da Engine online em andamento.
                close_when_requested: false,
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
    .add_plugins(detached::DetachedPlugin)
    .add_plugins((
        i18n::I18nPlugin,
        persistence::PersistencePlugin { db_path },
        aids::AidsPlugin,
        presentation::PresentationPlugin,
        board::BoardPlugin,
        pieces::PiecesPlugin,
        play::PlayPlugin,
        highlight::HighlightPlugin,
        hover::HoverPlugin,
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
    // Rede por Provedor: o `NetService` (dono do `chess-net`), o diálogo "Ligar <Provedor>?" e a
    // Central de Conexões. Depois da persistência: o `rede.db` fica ao lado do banco.
    app.add_plugins((
        net_service::NetServicePlugin,
        provider_consent::ProviderConsentPlugin,
        remote_engine::RemoteEnginePlugin,
        connections::ConnectionsPlugin,
    ));
    app.add_plugins(actions::ActionsPlugin);
    app.add_plugins((hud::HudPlugin, hud_draw::HudDrawPlugin));
    app.add_plugins(board2d_panel::Board2dPanelPlugin);
    app.add_plugins(coordinates::CoordinatesPlugin);
    app.add_plugins(table::TablePlugin);
    app.add_plugins(eval_bar::EvalBarPlugin);
    app.add_plugins(end_game::EndGamePlugin);
    app.add_plugins(training_ui::TrainingPlugin);
    app.add_plugins(library_ui::LibraryPlugin);
    app.add_plugins(openings::OpeningsPlugin);
    app.add_plugins(explorer::ExplorerPlugin);
    app.add_plugins(online::OnlinePlugin);
    app.add_plugins(engine_online::EngineOnlinePlugin);
    app.add_plugins(scoresheet::ScoresheetPlugin);
    app.add_plugins(ui_theme::UiThemePlugin);
    app.add_plugins(options::OptionsPlugin);
    app.add_plugins(screen_layouts::ScreenLayoutsPlugin);
    // O invólucro da interface: Paleta de Ações (Ctrl+K), barra de status e Ribbon.
    app.add_plugins((
        palette::PalettePlugin,
        status_bar::StatusBarPlugin,
        panel_stripe::PanelStripePlugin,
        ribbon::RibbonPlugin,
    ));
    // `--gallery`: janela com todos os widgets do design system nos dois Temas e Densidades.
    if args.iter().any(|a| a == "--gallery") {
        app.insert_resource(
            option_value("--gallery-scroll")
                .and_then(|v| v.parse::<f32>().ok())
                .map_or_else(
                    ui_theme::GalleryMode::default,
                    ui_theme::GalleryMode::scrolled,
                ),
        );
    }
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
        // `--connections-demo`: Provedores ligados e Conexões de exemplo, para a evidência da
        // Central e do indicador de rede (sem banco, nada disso persiste).
        if std::env::args().any(|a| a == "--connections-demo") {
            let mut demo = connections::ConnectionsUi::demo();
            // `--connections-open lichess,chesscom`: cartões com os Detalhes abertos.
            if let Some(keys) = option_value("--connections-open") {
                demo.open_details(keys.split(','));
            }
            if let Some(pixels) = option_value("--connections-scroll").and_then(|v| v.parse().ok())
            {
                demo.scrolled(pixels);
            }
            // `--connections-fide-search`: a busca por nome com resultados fictícios (#242).
            if std::env::args().any(|a| a == "--connections-fide-search") {
                demo.fide.demo_search();
            }
            // `--connections-clear-confirm`: a confirmação de "Limpar dados guardados" (#242).
            if std::env::args().any(|a| a == "--connections-clear-confirm") {
                demo.stored.confirming = true;
            }
            app.insert_resource(demo);
            let net = app.world().resource::<net_service::NetService>();
            for provider in [
                chess_net::ProviderId::Lichess,
                chess_net::ProviderId::ChessCom,
                chess_net::ProviderId::PlayStrategy,
            ] {
                let _ = net.set_enabled(provider, true);
            }
        }
        // `--connections-auth <cena>`: a Central com a autorização do Lichess em cena (cofre
        // falso e temporário, nunca o real; nenhum pedido sai). Cenas em `connections::demo`.
        if let Some(name) = option_value("--connections-auth") {
            match connections::demo::scene(&name) {
                Some(mut scene) => {
                    if let Some(pixels) =
                        option_value("--connections-scroll").and_then(|v| v.parse().ok())
                    {
                        scene.ui.scrolled(pixels);
                    }
                    app.insert_resource(scene.service.fair_play().clone());
                    app.insert_resource(scene.service);
                    app.insert_resource(scene.ui);
                }
                None => eprintln!("cena desconhecida em --connections-auth: {name}"),
            }
        }
        // `--consent-demo`: o diálogo "Ligar <Provedor>?" aberto, para a evidência.
        if std::env::args().any(|a| a == "--consent-demo") {
            app.world_mut()
                .resource_mut::<provider_consent::ConsentDialog>()
                .ask(
                    chess_net::ProviderId::Lichess,
                    Some("O teste da Conexão".to_owned()),
                );
        }
        // `--captured-text`: o Painel Capturadas no estilo texto (a opção de Tabuleiro).
        if std::env::args().any(|a| a == "--captured-text") {
            app.insert_resource(captured::CapturedStyle::Text);
        }
        if std::env::args().any(|a| a == "--no-coords") {
            app.insert_resource(coordinates::BoardCoordinates(false));
        }
        // `--theme <id>`: o Tema da interface na captura (`dark`, `light`, `feltro`, `pretas`...;
        // o padrão segue o sistema).
        if let Some(theme) = option_value("--theme")
            .as_deref()
            .and_then(ui_theme::ThemeMode::from_storage)
        {
            // `--density compact` troca a Densidade (o padrão é Confortável).
            let density = match option_value("--density").as_deref() {
                Some("compact" | "compacta") => ui_theme::Density::Compact,
                _ => ui_theme::Density::default(),
            };
            app.insert_resource(ui_theme::Appearance {
                theme,
                density,
                ..default()
            });
        }
        // `--palette [--palette-query <texto>]`: a Paleta de Ações aberta, para a evidência.
        if std::env::args().any(|a| a == "--palette") {
            let mut palette = palette::PaletteState::default();
            palette.open();
            palette.query = option_value("--palette-query").unwrap_or_default();
            app.insert_resource(palette);
        }
        match option_value("--quality").as_deref() {
            Some("economic") => app.insert_resource(presentation::GraphicsQuality::Economic),
            Some("balanced") => app.insert_resource(presentation::GraphicsQuality::Balanced),
            Some("ultra") => app.insert_resource(presentation::GraphicsQuality::Ultra),
            _ => &mut app,
        };
        // `--no-end-dialog`: capturas do rei tombado sem o diálogo por cima.
        if std::env::args().any(|a| a == "--no-end-dialog") {
            app.insert_resource(end_game::EndDialog {
                suppressed: true,
                ..default()
            });
        }
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
    let exit = app.run();
    // O banco temporário da demonstração some antes de qualquer `exit`.
    drop(demo);
    if let AppExit::Error(code) = exit {
        std::process::exit(i32::from(code.get()));
    }
}
