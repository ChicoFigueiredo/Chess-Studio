//! Apresentações (GLOSSARY): 3D profissional, Mesa imersiva e 2D técnico. As três mostram a
//! mesma posição (ver `layout`); aqui só se alterna o que é visível, as luzes, o fundo e a
//! câmera ativa. Tab alterna ciclicamente.
//!
//! Qualidade gráfica (MSAA e sombras) fica em `GraphicsQuality`, independente da Apresentação.

use crate::camera::MainCamera;
use bevy::light::{DirectionalLightShadowMap, ShadowFilteringMethod};
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy_egui::input::EguiWantsInput;

#[derive(
    Resource, Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Presentation {
    #[default]
    Professional3d,
    ImmersiveTable,
    Technical2d,
}

impl Presentation {
    pub const ALL: [Presentation; 3] = [
        Presentation::Professional3d,
        Presentation::ImmersiveTable,
        Presentation::Technical2d,
    ];

    /// Chave i18n do nome da Apresentação.
    pub fn label_key(self) -> &'static str {
        match self {
            Presentation::Professional3d => "presentation-professional3d",
            Presentation::ImmersiveTable => "presentation-immersive",
            Presentation::Technical2d => "presentation-technical2d",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Presentation::Professional3d => Presentation::ImmersiveTable,
            Presentation::ImmersiveTable => Presentation::Technical2d,
            Presentation::Technical2d => Presentation::Professional3d,
        }
    }

    pub fn is_3d(self) -> bool {
        self != Presentation::Technical2d
    }

    fn background(self) -> Color {
        match self {
            Presentation::Professional3d => Color::srgb(0.07, 0.075, 0.085),
            Presentation::ImmersiveTable => Color::srgb(0.12, 0.115, 0.11),
            Presentation::Technical2d => Color::srgb(0.15, 0.14, 0.13),
        }
    }
}

/// Qualidade gráfica. Nenhum preset toca na geometria de picking (matemática pura de
/// `chess_input`): só MSAA, sombras e filtro.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GraphicsQuality {
    /// MSAA 2x, sombras com mapa pequeno e filtro de hardware.
    Economic,
    /// MSAA 4x, sombras macias (filtro gaussiano) com mapa de 2048.
    #[default]
    Balanced,
    /// MSAA 8x e sombras macias com mapa de 4096.
    Ultra,
}

impl GraphicsQuality {
    pub const ALL: [GraphicsQuality; 3] = [
        GraphicsQuality::Economic,
        GraphicsQuality::Balanced,
        GraphicsQuality::Ultra,
    ];

    /// Chave i18n do nome da qualidade.
    pub fn label_key(self) -> &'static str {
        match self {
            GraphicsQuality::Economic => "quality-economic",
            GraphicsQuality::Balanced => "quality-balanced",
            GraphicsQuality::Ultra => "quality-ultra",
        }
    }

    pub fn msaa(self) -> Msaa {
        match self {
            GraphicsQuality::Economic => Msaa::Sample2,
            GraphicsQuality::Balanced => Msaa::Sample4,
            GraphicsQuality::Ultra => Msaa::Sample8,
        }
    }

    pub fn shadow_map_size(self) -> usize {
        match self {
            GraphicsQuality::Economic => 1024,
            GraphicsQuality::Balanced => 2048,
            GraphicsQuality::Ultra => 4096,
        }
    }

    pub fn shadow_filter(self) -> ShadowFilteringMethod {
        match self {
            GraphicsQuality::Economic => ShadowFilteringMethod::Hardware2x2,
            GraphicsQuality::Balanced | GraphicsQuality::Ultra => ShadowFilteringMethod::Gaussian,
        }
    }
}

/// Em quais Apresentações a entidade aparece.
#[derive(Component, Clone, Copy, Debug)]
pub struct VisibleIn {
    pub professional: bool,
    pub immersive: bool,
    pub technical: bool,
}

impl VisibleIn {
    pub const BOTH_3D: Self = Self {
        professional: true,
        immersive: true,
        technical: false,
    };
    pub const PROFESSIONAL: Self = Self {
        professional: true,
        immersive: false,
        technical: false,
    };
    pub const IMMERSIVE: Self = Self {
        professional: false,
        immersive: true,
        technical: false,
    };
    pub const TECHNICAL: Self = Self {
        professional: false,
        immersive: false,
        technical: true,
    };

    fn shows(self, p: Presentation) -> bool {
        match p {
            Presentation::Professional3d => self.professional,
            Presentation::ImmersiveTable => self.immersive,
            Presentation::Technical2d => self.technical,
        }
    }
}

/// Câmera do 2D técnico (ortográfica, vista de cima).
#[derive(Component)]
pub struct TechnicalCamera;

/// Papel da luz na iluminação de estúdio.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightRole {
    Key,
    Fill,
    Rim,
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Presentation>()
            .init_resource::<GraphicsQuality>()
            .add_systems(
                Update,
                (
                    cycle_with_tab,
                    apply_visibility,
                    apply_presentation,
                    apply_quality,
                    show_new_entities,
                )
                    .chain(),
            );
    }
}

fn cycle_with_tab(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    mut presentation: ResMut<Presentation>,
) {
    if !egui.wants_keyboard_input() && keys.just_pressed(KeyCode::Tab) {
        *presentation = presentation.next();
    }
}

pub(crate) fn set_visibility(
    presentation: Presentation,
    items: &mut Query<(&VisibleIn, &mut Visibility)>,
) {
    for (rule, mut vis) in items {
        let want = if rule.shows(presentation) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Entidades criadas depois da troca (por exemplo, a cena glTF que carrega de forma
/// assíncrona) herdam a regra da Apresentação atual.
fn show_new_entities(
    presentation: Res<Presentation>,
    mut items: Query<(&VisibleIn, &mut Visibility), Added<VisibleIn>>,
) {
    for (rule, mut vis) in &mut items {
        if !rule.shows(*presentation) {
            *vis = Visibility::Hidden;
        }
    }
}

/// Mostra só o que a Apresentação atual pede (a Partida e as peças não mudam: ADR-0001).
pub(crate) fn apply_visibility(
    presentation: Res<Presentation>,
    mut items: Query<(&VisibleIn, &mut Visibility)>,
) {
    if presentation.is_changed() {
        set_visibility(*presentation, &mut items);
    }
}

#[allow(clippy::type_complexity)]
fn apply_presentation(
    presentation: Res<Presentation>,
    mut cameras: ParamSet<(
        Single<&mut Camera, With<MainCamera>>,
        Single<&mut Camera, With<TechnicalCamera>>,
    )>,
    mut clear: ResMut<ClearColor>,
    mut lights: Query<(&LightRole, &mut DirectionalLight, &mut Transform)>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !presentation.is_changed() {
        return;
    }
    let p = *presentation;
    cameras.p0().is_active = p.is_3d();
    cameras.p1().is_active = !p.is_3d();
    clear.0 = p.background();

    // Iluminação: (iluminância, cor, posição) por papel.
    for (role, mut light, mut transform) in &mut lights {
        let (lux, color, pos) = match (p, *role) {
            (Presentation::ImmersiveTable, LightRole::Key) => (
                7_500.0,
                Color::srgb(1.0, 0.90, 0.78),
                Vec3::new(-7.0, 9.0, 5.0),
            ),
            (Presentation::ImmersiveTable, LightRole::Fill) => (
                900.0,
                Color::srgb(0.80, 0.86, 1.0),
                Vec3::new(8.0, 5.0, 6.0),
            ),
            (Presentation::ImmersiveTable, LightRole::Rim) => (
                1_800.0,
                Color::srgb(1.0, 0.92, 0.82),
                Vec3::new(2.0, 6.0, -9.0),
            ),
            (_, LightRole::Key) => (
                8_500.0,
                Color::srgb(1.0, 0.96, 0.90),
                Vec3::new(-5.0, 11.0, 6.0),
            ),
            (_, LightRole::Fill) => (
                2_400.0,
                Color::srgb(0.85, 0.90, 1.0),
                Vec3::new(8.0, 6.0, 5.0),
            ),
            (_, LightRole::Rim) => (
                3_000.0,
                Color::srgb(1.0, 1.0, 1.0),
                Vec3::new(1.0, 6.0, -10.0),
            ),
        };
        light.illuminance = lux;
        light.color = color;
        *transform = Transform::from_translation(pos).looking_at(Vec3::ZERO, Vec3::Y);
    }
    ambient.brightness = match p {
        Presentation::ImmersiveTable => 220.0,
        _ => 300.0,
    };
}

fn apply_quality(
    quality: Res<GraphicsQuality>,
    mut commands: Commands,
    camera: Single<Entity, With<MainCamera>>,
    cameras: Query<Entity, With<Camera>>,
    mut lights: Query<(&LightRole, &mut DirectionalLight)>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    if !quality.is_changed() {
        return;
    }
    // Todas as câmeras da janela (3D, 2D técnico e egui) precisam do mesmo MSAA: câmeras com
    // MSAA diferente não compartilham a textura principal e, conforme a ordem das entidades,
    // a Apresentação 3D sai preta (issue #82).
    for entity in &cameras {
        commands.entity(entity).insert(quality.msaa());
    }
    commands.entity(*camera).insert(quality.shadow_filter());
    shadow_map.size = quality.shadow_map_size();
    for (role, mut light) in &mut lights {
        // Só a luz principal projeta sombra.
        light.shadow_maps_enabled = *role == LightRole::Key;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::MainCamera;
    use bevy_egui::EguiGlobalSettings;

    fn spawn_main_camera(mut commands: Commands) {
        commands.spawn((Camera3d::default(), MainCamera));
    }

    /// Regressão da issue #82: câmeras da mesma janela com MSAA diferente não compartilham a
    /// textura principal; conforme a ordem das entidades (que muda ao registrar qualquer
    /// `Resource` extra), a Apresentação 3D saía toda preta nas capturas.
    #[test]
    fn all_window_cameras_share_one_msaa_setting() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<EguiWantsInput>()
            .init_resource::<ClearColor>()
            .init_resource::<GlobalAmbientLight>()
            .init_resource::<DirectionalLightShadowMap>()
            .init_resource::<EguiGlobalSettings>()
            .add_plugins(PresentationPlugin)
            .add_systems(
                Startup,
                (
                    spawn_main_camera,
                    crate::board2d::spawn_camera,
                    crate::ui::spawn_ui_camera,
                ),
            );
        for quality in GraphicsQuality::ALL {
            app.insert_resource(quality);
            app.update();
            app.update();
            let mut cameras = app.world_mut().query::<(&Camera, &Msaa)>();
            let found: Vec<Msaa> = cameras.iter(app.world()).map(|(_, m)| *m).collect();
            assert_eq!(found.len(), 3, "câmeras 3D, 2D técnico e egui");
            assert!(
                found.iter().all(|m| *m == quality.msaa()),
                "{quality:?}: MSAA divergente entre câmeras: {found:?}"
            );
        }
    }
}
