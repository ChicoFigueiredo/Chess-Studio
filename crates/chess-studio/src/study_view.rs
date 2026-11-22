//! Sinalização do Modo de estudo (#324): a **tonalidade do Tema** sobre as casas e a moldura do
//! tabuleiro principal e a **faixa "Estudo"** com o botão "Voltar à Partida".
//!
//! # Render
//!
//! Só malhas e sprites translúcidos (como os Auxílios de `highlight` e o HUD) e desenho egui:
//! **nenhuma câmera nova e nenhum `Msaa`/`Hdr` próprio** (regra de câmeras, #82/#142). As peças
//! não mudam de cor: a tinta fica sobre as casas, abaixo das peças e das marcações. Os
//! marcadores não têm colisor nem entram no picking, então não atrapalham o Método de movimento.
//!
//! # Autoridade (ADR-0001)
//!
//! Aqui só se lê a `Session`. O único efeito é a intenção `Session::leave_study` ("Voltar à
//! Partida"); nada do estudo vai ao log.

use crate::board2d::UNIT;
use crate::i18n::I18n;
use crate::presentation::{Presentation, VisibleIn};
use crate::session::Session;
use crate::ui_theme::{Appearance, ButtonVariant, CsButton, Palette, Tokens, scene_study_tint};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_input::BOARD_SIZE;

/// Altura (3D) da tinta das casas: acima do plano das casas (y = 0), abaixo do HUD (0.004+) e
/// dos Auxílios (0.010+).
const SQUARES_Y: f32 = 0.002;
/// A tinta da moldura fica logo acima da face dela, que é a de `board::FRAME_TOP`.
const FRAME_Y: f32 = crate::board::FRAME_TOP + 0.004;
/// Profundidade (2D técnico) da tinta das casas: acima das casas (0) e abaixo dos Auxílios.
const SQUARES_Z: f32 = 0.1;
/// Profundidade da moldura do 2D técnico: atrás das casas.
const FRAME_Z: f32 = -0.1;
/// Largura da moldura tingida do 2D técnico, em casas, de cada lado.
const FRAME_2D_BORDER: f32 = 0.12;

/// Marca as quatro entidades da tinta (casas e moldura, 3D e 2D técnico).
#[derive(Component)]
struct StudyTint;

/// A tinta é só do Modo de estudo e só da Apresentação em que a entidade existe.
pub(crate) fn tint_visible(studying: bool, rule: VisibleIn, presentation: Presentation) -> bool {
    studying && rule.shows(presentation)
}

#[derive(Resource)]
struct StudyTintMaterial(Handle<StandardMaterial>);

fn flat_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

fn spawn_tint(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(flat_material(Color::NONE));
    commands.insert_resource(StudyTintMaterial(material.clone()));
    // 3D: um plano sobre as 64 casas e outro sobre a moldura (que as casas escondem por cima).
    for (side, y, name) in [
        (BOARD_SIZE, SQUARES_Y, "study_tint_squares"),
        (crate::board::FRAME_WIDTH, FRAME_Y, "study_tint_frame"),
    ] {
        commands.spawn((
            Name::new(name),
            StudyTint,
            Mesh3d(meshes.add(Plane3d::default().mesh().size(side, side))),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, y, 0.0),
            Visibility::Hidden,
            VisibleIn::BOTH_3D,
        ));
    }
    // 2D técnico: sprites; a moldura é uma faixa fina além das casas.
    for (side, z, name) in [
        (BOARD_SIZE * UNIT, SQUARES_Z, "study_tint_squares_2d"),
        (
            (BOARD_SIZE + 2.0 * FRAME_2D_BORDER) * UNIT,
            FRAME_Z,
            "study_tint_frame_2d",
        ),
    ] {
        commands.spawn((
            Name::new(name),
            StudyTint,
            Sprite::from_color(Color::NONE, Vec2::splat(side)),
            Transform::from_xyz(0.0, 0.0, z),
            Visibility::Hidden,
            VisibleIn::TECHNICAL,
        ));
    }
}

/// Mostra a tinta só no Modo de estudo e a pinta com o `study-tint` do Tema efetivo.
#[allow(clippy::type_complexity)]
fn update_tint(
    session: Res<Session>,
    presentation: Option<Res<Presentation>>,
    appearance: Option<Res<Appearance>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    material: Option<Res<StudyTintMaterial>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut items: Query<(&VisibleIn, &mut Visibility, Option<&mut Sprite>), With<StudyTint>>,
) {
    let presentation = presentation.map_or_else(Default::default, |p| *p);
    let studying = session.studying();
    let mode = appearance.map_or_else(Default::default, |a| a.theme);
    let system_dark = windows.single().ok().and_then(|w| match w.window_theme? {
        bevy::window::WindowTheme::Dark => Some(true),
        bevy::window::WindowTheme::Light => Some(false),
    });
    let tint = scene_study_tint(mode, system_dark);
    if let (Some(material), Some(mut materials)) = (material, materials)
        && materials
            .get(&material.0)
            .is_some_and(|m| m.base_color != tint)
        && let Some(mut m) = materials.get_mut(&material.0)
    {
        m.base_color = tint;
    }
    for (rule, mut visibility, sprite) in &mut items {
        let want = if tint_visible(studying, *rule, presentation) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if let Some(mut sprite) = sprite
            && sprite.color != tint
        {
            sprite.color = tint;
        }
    }
}

// --- Faixa "Estudo" ----------------------------------------------------------------------

/// Largura máxima da faixa, em pontos.
const BANNER_MAX_WIDTH: f32 = 560.0;
/// Margem da faixa até as bordas da área do tabuleiro.
const BANNER_MARGIN: f32 = 8.0;
/// Altura da faixa (uma linha de controles de 32 pt, mais o respiro).
const BANNER_HEIGHT: f32 = 44.0;
/// Abaixo desta largura a faixa abre mão da frase de apoio (cabe em 800x600).
const BANNER_HINT_MIN_WIDTH: f32 = 520.0;

/// Onde a faixa fica: centrada no topo da área do tabuleiro, nunca mais larga que ela.
pub(crate) fn banner_rect(area: egui::Rect) -> egui::Rect {
    let width = (area.width() - 2.0 * BANNER_MARGIN).clamp(0.0, BANNER_MAX_WIDTH);
    let min = egui::pos2(area.center().x - width / 2.0, area.min.y + BANNER_MARGIN);
    egui::Rect::from_min_size(min, egui::vec2(width, BANNER_HEIGHT))
}

/// O que a faixa diz além de "Estudo": o aviso de Lance novo na Partida (a Engine adversária
/// jogou durante o estudo, decisão 12 da spec #320).
pub(crate) fn banner_notice(game_advanced: bool) -> Option<&'static str> {
    game_advanced.then_some("study-banner-game-advanced")
}

pub struct StudyViewPlugin;

impl Plugin for StudyViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_tint)
            .add_systems(
                Update,
                update_tint.after(crate::presentation::apply_visibility),
            )
            .add_systems(
                EguiPrimaryContextPass,
                study_banner.after(crate::dock::dock_ui),
            );
    }
}

/// A faixa sobre o tabuleiro principal, enquanto o Modo de estudo está ligado.
fn study_banner(
    mut contexts: EguiContexts,
    mut session: ResMut<Session>,
    i18n: Res<I18n>,
    dock: Res<crate::dock::Dock>,
) -> Result {
    if !session.studying() {
        return Ok(());
    }
    let Some(area) = dock.board_area() else {
        return Ok(());
    };
    let notice = banner_notice(session.study_game_advanced());
    let ctx = contexts.ctx_mut()?;
    let p = Palette::from_visuals(&ctx.global_style().visuals);
    let rect = banner_rect(area);
    let radius = Tokens::embedded().length("radius-l").unwrap_or(6.0);
    let mut back = false;
    egui::Area::new(egui::Id::new("chess-studio-study-banner"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.surface_2)
                .stroke(egui::Stroke::new(1.0, p.border_strong))
                .corner_radius(radius)
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - 22.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                egui_phosphor::regular::FLASK,
                                i18n.t("study-banner-title")
                            ))
                            .strong()
                            .color(p.accent_ink),
                        );
                        if rect.width() >= BANNER_HINT_MIN_WIDTH && notice.is_none() {
                            ui.label(
                                egui::RichText::new(i18n.t("study-banner-hint"))
                                    .color(p.ink_muted),
                            );
                        }
                        if let Some(key) = notice {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} {}",
                                    egui_phosphor::regular::WARNING,
                                    i18n.t(key)
                                ))
                                .color(p.warning),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            back = ui
                                .add(
                                    CsButton::new(&i18n.t("study-banner-back"))
                                        .icon(egui_phosphor::regular::ARROW_U_UP_LEFT)
                                        .variant(ButtonVariant::Primary),
                                )
                                .on_hover_text(i18n.t("study-banner-back-hover"))
                                .clicked();
                        });
                    });
                });
        });
    if back {
        // Só a intenção: a `Session` sai do estudo e restaura a posição de jogo (ADR-0001).
        session.leave_study();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::ThemeMode;

    #[test]
    fn the_tint_shows_only_while_studying_and_only_in_the_presentation_it_belongs_to() {
        for p in Presentation::ALL {
            assert!(!tint_visible(false, VisibleIn::BOTH_3D, p), "{p:?}");
            assert!(!tint_visible(false, VisibleIn::TECHNICAL, p), "{p:?}");
            assert_eq!(tint_visible(true, VisibleIn::BOTH_3D, p), p.is_3d());
            assert_eq!(tint_visible(true, VisibleIn::TECHNICAL, p), !p.is_3d());
        }
    }

    fn tint_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<Session>()
            .init_resource::<Presentation>()
            .add_systems(Startup, spawn_tint)
            .add_systems(Update, update_tint);
        app.update();
        app
    }

    fn visible_count(app: &mut App) -> usize {
        let mut q = app.world_mut().query::<(&StudyTint, &Visibility)>();
        q.iter(app.world())
            .filter(|(_, v)| **v == Visibility::Inherited)
            .count()
    }

    #[test]
    fn the_tint_follows_the_study_mode_and_the_presentation_and_adds_no_camera() {
        let mut app = tint_app();
        app.update();
        assert_eq!(visible_count(&mut app), 0, "fora do estudo, nada aparece");

        app.world_mut().resource_mut::<Session>().study_here().unwrap();
        app.update();
        assert_eq!(visible_count(&mut app), 2, "3D profissional: casas e moldura");
        *app.world_mut().resource_mut::<Presentation>() = Presentation::ImmersiveTable;
        app.update();
        assert_eq!(visible_count(&mut app), 2, "Mesa imersiva: casas e moldura");
        *app.world_mut().resource_mut::<Presentation>() = Presentation::Technical2d;
        app.update();
        assert_eq!(visible_count(&mut app), 2, "2D técnico: casas e moldura");

        app.world_mut().resource_mut::<Session>().leave_study();
        app.update();
        assert_eq!(visible_count(&mut app), 0, "sumiu ao voltar à Partida");

        // Regra de câmeras (#82/#142): a tinta não cria câmera nem define Msaa/Hdr.
        let world = app.world_mut();
        assert_eq!(world.query::<&Camera>().iter(world).count(), 0);
        assert_eq!(world.query::<&Msaa>().iter(world).count(), 0);
    }

    #[test]
    fn the_tint_takes_the_color_of_the_effective_theme() {
        let mut app = tint_app();
        let feltro = crate::ui_theme::ThemeId::from_key("feltro").expect("tema");
        app.insert_resource(Appearance {
            theme: ThemeMode::Fixed(feltro),
            ..Default::default()
        });
        app.world_mut().resource_mut::<Session>().study_here().unwrap();
        *app.world_mut().resource_mut::<Presentation>() = Presentation::Technical2d;
        app.update();
        app.update();
        let want = scene_study_tint(ThemeMode::Fixed(feltro), None);
        let mut q = app.world_mut().query::<(&StudyTint, &Sprite)>();
        let colors: Vec<Color> = q.iter(app.world()).map(|(_, s)| s.color).collect();
        assert_eq!(colors, vec![want; 2]);
    }

    #[test]
    fn the_banner_sits_at_the_top_of_the_board_area_and_fits_a_small_window() {
        // Janela 800x600 com o dock aberto: a área do tabuleiro é a que sobra.
        for (w, h) in [(800.0, 600.0), (480.0, 360.0), (1600.0, 900.0)] {
            let area = egui::Rect::from_min_size(egui::pos2(0.0, 90.0), egui::vec2(w, h - 90.0));
            let r = banner_rect(area);
            assert!(area.contains_rect(r), "{w}x{h}: {r:?} cabe em {area:?}");
            assert!((r.center().x - area.center().x).abs() < 0.5, "centrada");
            assert!(r.min.y < area.min.y + 2.0 * BANNER_MARGIN, "no topo");
            assert!(r.width() <= BANNER_MAX_WIDTH);
        }
    }

    #[test]
    fn the_banner_warns_of_a_new_move_only_when_the_game_advanced() {
        assert_eq!(banner_notice(false), None);
        assert_eq!(banner_notice(true), Some("study-banner-game-advanced"));
    }
}
