//! Apresentações (GLOSSARY): 3D profissional, Mesa imersiva e 2D técnico. As três mostram a
//! mesma posição (ver `layout`); aqui só se alterna o que é visível, as luzes, o fundo e a
//! câmera ativa. Tab alterna ciclicamente.
//!
//! Qualidade gráfica (MSAA, sombras e efeitos opcionais) fica em `GraphicsQuality`,
//! independente da Apresentação.
//!
//! # Ultra (US-30)
//!
//! MSAA 8x, mapa de sombras 4096 com filtro gaussiano e, opcionalmente, **bloom suave** com
//! o mapeamento de tons (`Tonemapping::TonyMcMapface`) já no pipeline HDR. Tudo isso é
//! escolhido contra o que a GPU declara suportar ([`GpuSupport`]): sem 8x o Ultra cai para
//! o maior MSAA que existe, sem HDR com esse MSAA o bloom simplesmente não liga. O picking
//! é matemática pura de `chess_input` e não é tocado.
//!
//! **SSAO não entra**: no Bevy ele exige `Msaa::Off`, o que contradiz o MSAA 8x do Ultra
//! e a regra de MSAA único entre as câmeras (issue #82). Idem o filtro de sombras temporal
//! (precisa de TAA).

use crate::camera::MainCamera;
use bevy::camera::Hdr;
use bevy::light::{DirectionalLightShadowMap, ShadowFilteringMethod};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapter;
use bevy::render::view::{Msaa, Tonemapping};
use wgpu_types::TextureFormat;

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

/// Efeitos opcionais do Ultra (bloom suave). Só valem com a qualidade Ultra e com suporte
/// da GPU; fora disso este recurso não faz nada.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UltraEffects(pub bool);

impl Default for UltraEffects {
    fn default() -> Self {
        Self(true)
    }
}

/// O que a GPU declara suportar: máscara de contagens de amostras de MSAA por formato
/// (bit 0 = 1x, bit 1 = 2x, bit 2 = 4x, bit 3 = 8x).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuSupport {
    /// Formato da textura principal em LDR (o padrão).
    ldr: u8,
    /// Formato HDR intermediário (`Rgba16Float`), exigido pelo bloom.
    hdr: u8,
}

fn sample_bit(samples: u32) -> u8 {
    match samples {
        1 => 1,
        2 => 2,
        4 => 4,
        8 => 8,
        _ => 0,
    }
}

impl GpuSupport {
    /// Tudo suportado: o valor antes da detecção e nos testes sem GPU.
    pub const FULL: GpuSupport = GpuSupport {
        ldr: 0b1111,
        hdr: 0b1111,
    };

    /// Só o que cada contagem de `ldr` e `hdr` permitir (testes e detecção).
    pub fn from_counts(ldr: &[u32], hdr: &[u32]) -> Self {
        let mask = |counts: &[u32]| counts.iter().fold(1, |m, c| m | sample_bit(*c));
        Self {
            ldr: mask(ldr),
            hdr: mask(hdr),
        }
    }

    /// Consulta o adaptador real.
    pub fn detect(adapter: &RenderAdapter) -> Self {
        let counts = |format: TextureFormat| {
            let flags = adapter.get_texture_format_features(format).flags;
            [1, 2, 4, 8]
                .into_iter()
                .filter(|n| flags.sample_count_supported(*n))
                .collect::<Vec<u32>>()
        };
        Self::from_counts(
            &counts(TextureFormat::Rgba8UnormSrgb),
            &counts(TextureFormat::Rgba16Float),
        )
    }

    /// A maior contagem de amostras `<= wanted` que a máscara aceita (1 = MSAA desligado).
    fn best(mask: u8, wanted: u32) -> u32 {
        [8, 4, 2]
            .into_iter()
            .find(|n| *n <= wanted && mask & sample_bit(*n) != 0)
            .unwrap_or(1)
    }
}

impl Default for GpuSupport {
    fn default() -> Self {
        Self::FULL
    }
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

    /// O MSAA que a GPU realmente aceita: o desta qualidade ou o maior abaixo dele.
    pub fn msaa_for(self, support: GpuSupport) -> Msaa {
        Msaa::from_samples(GpuSupport::best(support.ldr, self.msaa().samples()))
    }

    /// O bloom suave liga? Só no Ultra, com a preferência ligada e com a GPU renderizando
    /// o formato HDR com o MSAA efetivo desta qualidade.
    pub fn bloom_for(self, enabled: bool, support: GpuSupport) -> bool {
        let samples = self.msaa_for(support).samples();
        self == GraphicsQuality::Ultra
            && enabled
            && (samples == 1 || support.hdr & sample_bit(samples) != 0)
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

    pub(crate) fn shows(self, p: Presentation) -> bool {
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
            .init_resource::<UltraEffects>()
            .init_resource::<GpuSupport>()
            .add_systems(
                Update,
                (
                    apply_visibility,
                    apply_presentation,
                    apply_scene_background,
                    detect_gpu_support,
                    apply_quality,
                    show_new_entities,
                )
                    .chain(),
            );
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

/// O fundo da cena (3D e 2D técnico) é o `surface-0` do Tema efetivo (issue #206). Só o
/// `ClearColor`: tabuleiro, peças e iluminação não mudam com o Tema, e as câmeras seguem
/// idênticas em `Msaa` e `Hdr`.
fn apply_scene_background(
    appearance: Option<Res<crate::ui_theme::Appearance>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut clear: ResMut<ClearColor>,
    floor: Query<&MeshMaterial3d<StandardMaterial>, With<crate::board::StudioFloor>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut labels: Query<&mut TextColor, With<crate::coordinates::Labels2d>>,
) {
    let mode = appearance.map_or_else(Default::default, |a| a.theme);
    let system_dark = windows.single().ok().and_then(|w| match w.window_theme? {
        bevy::window::WindowTheme::Dark => Some(true),
        bevy::window::WindowTheme::Light => Some(false),
    });
    let want = crate::ui_theme::scene_background(mode, system_dark);
    if clear.0 != want {
        clear.0 = want;
    }
    // As coordenadas do 2D técnico ficam sobre o fundo: `ink-muted` do Tema.
    let label = crate::ui_theme::scene_label_color(mode, system_dark);
    for mut color in &mut labels {
        if color.0 != label {
            color.0 = label;
        }
    }
    // O piso do 3D profissional cobre o fundo: sem luz, na cor do token, para o que aparece
    // na tela ficar no `surface-0` (o mapeamento de tons da câmera 3D é compensado).
    let floor_color = compensate_tonemapping(want);
    if let Some(materials) = materials.as_deref_mut() {
        for handle in &floor {
            if materials
                .get(&handle.0)
                .is_some_and(|m| m.base_color != floor_color)
                && let Some(mut m) = materials.get_mut(&handle.0)
            {
                m.base_color = floor_color;
            }
        }
    }
}

/// Resposta medida do mapeamento de tons da câmera 3D (`TonyMcMapface`, sem HDR) a um piso sem
/// luz de cinza neutro: (entrada sRGB, saída sRGB) em capturas de 1280x720. Os claros saem
/// comprimidos: a entrada 1,0 aparece como 0,77 (197/255).
const TONEMAP_RESPONSE: [(f32, f32); 12] = [
    (0.0, 0.004),
    (0.1, 0.110),
    (0.2, 0.212),
    (0.3, 0.310),
    (0.4, 0.400),
    (0.5, 0.482),
    (0.6, 0.557),
    (0.7, 0.624),
    (0.8, 0.682),
    (0.9, 0.729),
    (0.95, 0.753),
    (1.0, 0.773),
];

/// Entrada sRGB que o mapeamento de tons devolve como `want` (limitada ao que ele alcança).
fn invert_tonemapping(want: f32) -> f32 {
    for w in TONEMAP_RESPONSE.windows(2) {
        let ((i0, o0), (i1, o1)) = (w[0], w[1]);
        if want <= o1 {
            let t = ((want - o0) / (o1 - o0)).clamp(0.0, 1.0);
            return i0 + (i1 - i0) * t;
        }
    }
    1.0
}

/// Cor a entregar à câmera 3D para que, depois do mapeamento de tons, apareça `target`. Os
/// claros acima de ~0,77 não se alcançam: ficam no máximo da câmera.
fn compensate_tonemapping(target: Color) -> Color {
    let c = target.to_srgba();
    Color::srgb(
        invert_tonemapping(c.red),
        invert_tonemapping(c.green),
        invert_tonemapping(c.blue),
    )
}

#[allow(clippy::type_complexity)]
fn apply_presentation(
    presentation: Res<Presentation>,
    mut cameras: ParamSet<(
        Single<&mut Camera, With<MainCamera>>,
        Single<&mut Camera, With<TechnicalCamera>>,
    )>,
    mut lights: Query<(&LightRole, &mut DirectionalLight, &mut Transform)>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !presentation.is_changed() {
        return;
    }
    let p = *presentation;
    cameras.p0().is_active = p.is_3d();
    cameras.p1().is_active = !p.is_3d();

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

/// Lê o adaptador uma vez, quando o renderizador já existe.
fn detect_gpu_support(
    adapter: Option<Res<RenderAdapter>>,
    mut support: ResMut<GpuSupport>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(adapter) = adapter else { return };
    *done = true;
    let detected = GpuSupport::detect(&adapter);
    info!("GPU: MSAA suportado {detected:?}");
    *support = detected;
}

#[allow(clippy::too_many_arguments)]
fn apply_quality(
    quality: Res<GraphicsQuality>,
    effects: Res<UltraEffects>,
    support: Res<GpuSupport>,
    mut commands: Commands,
    camera: Single<Entity, With<MainCamera>>,
    cameras: Query<Entity, With<Camera>>,
    new_cameras: Query<(), Added<Camera>>,
    mut lights: Query<(&LightRole, &mut DirectionalLight)>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    // Câmera nova (a de uma janela destacada, #168) recebe o mesmo MSAA/HDR das demais.
    if !(quality.is_changed()
        || effects.is_changed()
        || support.is_changed()
        || !new_cameras.is_empty())
    {
        return;
    }
    let msaa = quality.msaa_for(*support);
    // Todas as câmeras da janela (3D, 2D técnico e egui) precisam do mesmo MSAA: câmeras com
    // MSAA diferente não compartilham a textura principal e, conforme a ordem das entidades,
    // a Apresentação 3D sai preta (issue #82).
    for entity in &cameras {
        commands.entity(entity).insert(msaa);
    }
    commands.entity(*camera).insert(quality.shadow_filter());
    // Efeitos opcionais do Ultra. O bloom e o tonemapping ficam só na câmera 3D, mas o `Hdr`
    // vale para TODAS as câmeras da janela: câmeras com HDR diferente usam texturas principais
    // diferentes e a câmera do egui (sem limpeza de cor) compunha a interface sobre uma textura
    // de quadros antigos ao trocar de Apresentação: janela e tooltip duplicados (issue #142).
    let bloom = quality.bloom_for(effects.0, *support);
    for entity in &cameras {
        if bloom {
            commands.entity(entity).insert(Hdr);
        } else {
            commands.entity(entity).remove::<Hdr>();
        }
    }
    if bloom {
        commands.entity(*camera).insert((
            Tonemapping::TonyMcMapface,
            Bloom {
                intensity: 0.06,
                ..Bloom::NATURAL
            },
        ));
    } else {
        commands.entity(*camera).remove::<Bloom>();
    }
    shadow_map.size = quality.shadow_map_size();
    for (role, mut light) in &mut lights {
        // Só a luz principal projeta sombra.
        light.shadow_maps_enabled = *role == LightRole::Key;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compensação inverte a resposta medida do mapeamento de tons: escuros quase idênticos,
    /// claros no máximo que a câmera alcança, sempre monótona.
    #[test]
    fn tonemapping_compensation_is_monotonic_and_clamped() {
        let mut last = -1.0;
        for i in 0..=100 {
            let v = invert_tonemapping(i as f32 / 100.0);
            assert!((0.0..=1.0).contains(&v) && v >= last);
            last = v;
        }
        assert!((invert_tonemapping(0.110) - 0.1).abs() < 1e-3);
        assert_eq!(invert_tonemapping(0.92), 1.0);
    }
    use crate::camera::MainCamera;
    use bevy_egui::EguiGlobalSettings;
    use bevy_egui::input::EguiWantsInput;

    fn spawn_main_camera(mut commands: Commands) {
        commands.spawn((Camera3d::default(), MainCamera));
    }

    /// Câmera de UI de uma janela destacada (#168), criada pelo mesmo código do app.
    fn spawn_detached_camera(mut commands: Commands) {
        let mut layout = crate::dock::DockLayout::default();
        layout
            .detach_group_to_window(crate::dock::ids::NOTATION, [400.0, 300.0])
            .unwrap();
        crate::detached::spawn_detached_window(
            &mut commands,
            0,
            &layout.windows[0],
            "teste".into(),
            &[],
        );
    }

    /// US-30: o preset desce ao que a GPU suporta, sem nunca pedir uma contagem inexistente.
    #[test]
    fn quality_falls_back_to_what_the_gpu_supports() {
        let full = GpuSupport::FULL;
        assert_eq!(GraphicsQuality::Ultra.msaa_for(full), Msaa::Sample8);
        assert_eq!(GraphicsQuality::Balanced.msaa_for(full), Msaa::Sample4);
        assert_eq!(GraphicsQuality::Economic.msaa_for(full), Msaa::Sample2);
        // Sem 8x: o Ultra usa 4x.
        let no_8x = GpuSupport::from_counts(&[1, 2, 4], &[1, 2, 4]);
        assert_eq!(GraphicsQuality::Ultra.msaa_for(no_8x), Msaa::Sample4);
        // Só 2x: Ultra e Equilibrado caem para 2x.
        let only_2x = GpuSupport::from_counts(&[1, 2], &[1, 2]);
        assert_eq!(GraphicsQuality::Ultra.msaa_for(only_2x), Msaa::Sample2);
        assert_eq!(GraphicsQuality::Balanced.msaa_for(only_2x), Msaa::Sample2);
        // Sem MSAA nenhum: desligado.
        let none = GpuSupport::from_counts(&[], &[]);
        for q in GraphicsQuality::ALL {
            assert_eq!(q.msaa_for(none), Msaa::Off);
        }
        // Lacuna (4x sem 8x e sem 2x): ainda acha o 4x.
        let gap = GpuSupport::from_counts(&[4], &[4]);
        assert_eq!(GraphicsQuality::Ultra.msaa_for(gap), Msaa::Sample4);
        assert_eq!(GraphicsQuality::Economic.msaa_for(gap), Msaa::Off);
    }

    #[test]
    fn bloom_only_in_ultra_when_enabled_and_hdr_is_supported_at_the_effective_msaa() {
        let full = GpuSupport::FULL;
        assert!(GraphicsQuality::Ultra.bloom_for(true, full));
        assert!(
            !GraphicsQuality::Ultra.bloom_for(false, full),
            "preferência desligada"
        );
        assert!(!GraphicsQuality::Balanced.bloom_for(true, full));
        assert!(!GraphicsQuality::Economic.bloom_for(true, full));
        // HDR sem 8x, mas o Ultra efetivo é 4x e o HDR rende 4x: liga.
        let ldr8_hdr4 = GpuSupport::from_counts(&[1, 2, 4, 8], &[1, 2, 4]);
        assert_eq!(GraphicsQuality::Ultra.msaa_for(ldr8_hdr4), Msaa::Sample8);
        assert!(
            !GraphicsQuality::Ultra.bloom_for(true, ldr8_hdr4),
            "HDR sem 8x"
        );
        // HDR indisponível: sem bloom, a qualidade segue.
        let no_hdr = GpuSupport::from_counts(&[1, 2, 4, 8], &[]);
        assert!(!GraphicsQuality::Ultra.bloom_for(true, no_hdr));
    }

    /// Regressão da issue #82: câmeras da mesma janela com MSAA diferente não compartilham a
    /// textura principal; conforme a ordem das entidades (que muda ao registrar qualquer
    /// `Resource` extra), a Apresentação 3D saía toda preta nas capturas.
    #[test]
    fn all_window_cameras_share_one_msaa_setting() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::controls::Controls>()
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
                    spawn_detached_camera,
                ),
            );
        for quality in GraphicsQuality::ALL {
            app.insert_resource(quality);
            app.update();
            app.update();
            let mut cameras = app.world_mut().query::<(&Camera, &Msaa)>();
            let found: Vec<Msaa> = cameras.iter(app.world()).map(|(_, m)| *m).collect();
            assert_eq!(
                found.len(),
                4,
                "câmeras 3D, 2D técnico, egui e a da janela destacada"
            );
            assert!(
                found
                    .iter()
                    .all(|m| *m == quality.msaa_for(GpuSupport::FULL)),
                "{quality:?}: MSAA divergente entre câmeras: {found:?}"
            );
        }
    }

    /// Issue #142: a interface é desenhada por exatamente uma câmera (a do contexto egui
    /// primário) em qualquer Apresentação, e todas as câmeras da janela têm o mesmo MSAA e o
    /// mesmo HDR depois de trocar de Apresentação várias vezes com o bloom ligado.
    #[test]
    fn one_ui_camera_and_uniform_camera_setup_after_cycling_presentations() {
        use bevy_egui::PrimaryEguiContext;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::controls::Controls>()
            .init_resource::<EguiWantsInput>()
            .init_resource::<ClearColor>()
            .init_resource::<GlobalAmbientLight>()
            .init_resource::<DirectionalLightShadowMap>()
            .init_resource::<EguiGlobalSettings>()
            .insert_resource(GraphicsQuality::Ultra)
            .add_plugins(PresentationPlugin)
            .add_systems(
                Startup,
                (
                    spawn_main_camera,
                    crate::board2d::spawn_camera,
                    crate::ui::spawn_ui_camera,
                    spawn_detached_camera,
                ),
            );
        for step in 0..9 {
            if step == 4 {
                app.insert_resource(UltraEffects(false));
            }
            if step == 6 {
                app.insert_resource(UltraEffects(true));
            }
            let next = app.world().resource::<Presentation>().next();
            app.insert_resource(next);
            app.update();
            app.update();
            let world = app.world_mut();
            let ui_cameras = world
                .query_filtered::<&Camera, With<PrimaryEguiContext>>()
                .iter(world)
                .count();
            assert_eq!(ui_cameras, 1, "passo {step}: uma única câmera de UI");
            let ui_active = world
                .query_filtered::<&Camera, With<PrimaryEguiContext>>()
                .single(world)
                .unwrap()
                .is_active;
            assert!(ui_active, "passo {step}: a câmera de UI fica sempre ativa");
            let mut q = world.query::<(&Camera, &Msaa, Has<Hdr>)>();
            let setups: Vec<(Msaa, bool)> = q.iter(world).map(|(_, m, h)| (*m, h)).collect();
            assert_eq!(setups.len(), 4, "inclui a janela destacada");
            assert!(
                setups.windows(2).all(|w| w[0] == w[1]),
                "passo {step}: MSAA/HDR divergente entre câmeras: {setups:?}"
            );
            // Issue #156: câmeras 2D sem tonemapping (senão, com HDR, a passada de tonemapping
            // concorre com o `egui_pass` e a interface some em alguns quadros).
            let mut tm = world.query_filtered::<&Tonemapping, (With<Camera2d>, With<Camera>)>();
            let tonemaps: Vec<_> = tm.iter(world).copied().collect();
            assert_eq!(
                tonemaps.len(),
                3,
                "passo {step}: as câmeras 2D e a da janela"
            );
            assert!(
                tonemaps.iter().all(|t| *t == Tonemapping::None),
                "passo {step}: câmeras 2D sem tonemapping: {tonemaps:?}"
            );
            let scene_active = q.iter(world).filter(|(c, ..)| c.is_active).count();
            // UI + a câmera da janela destacada (outro alvo) + exatamente uma câmera de cena.
            assert_eq!(
                scene_active, 3,
                "UI + janela destacada + uma câmera de cena"
            );
        }
    }
}
