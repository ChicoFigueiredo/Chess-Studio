//! Cena 3D: tabuleiro de madeira (casas claras/escuras e borda), mesa discreta da Mesa
//! imersiva e iluminação de estúdio. Só geometria visual: nenhuma regra de jogo (as peças
//! vêm da Partida, ver `pieces`). A geometria das 64 casas vem de `chess_input` (a mesma do picking).
//!
//! Assets (todos CC0, ver `assets/LICENSES.md`): peças do "Chess Set" da Poly Haven e
//! texturas PBR de madeira (maple, nogueira negra e carvalho) da Poly Haven.

use crate::presentation::{LightRole, VisibleIn};
use bevy::asset::RenderAssetUsages;
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use chess_core::Square;
use chess_input::{BOARD_SIZE, SQUARE_SIZE, all_squares, square_center};

/// Escala do modelo glTF (metros) para o mundo (1 casa = 1 unidade): o "Chess Set" tem
/// casas de 0,057888 m.
pub const PIECE_SCALE: f32 = 1.0 / 0.057_888;
/// Espessura da moldura do tabuleiro.
const FRAME_THICKNESS: f32 = 0.4;
/// A face da moldura fica um pouco abaixo do plano das casas (y = 0).
const FRAME_TOP: f32 = -0.02;
/// Largura total da moldura (tabuleiro 8 + borda).
const FRAME_WIDTH: f32 = BOARD_SIZE + 1.2;
/// Topo da mesa (fica logo abaixo da moldura).
const TABLE_TOP: f32 = FRAME_TOP - FRAME_THICKNESS;

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 420.0,
            ..default()
        })
        .add_systems(Startup, (spawn_board, spawn_table, spawn_lights));
    }
}

/// Textura de madeira com repetição e filtro anisotrópico.
fn load_texture(assets: &AssetServer, path: &'static str, srgb: bool) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.is_srgb = srgb;
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                anisotropy_clamp: 8,
                ..default()
            });
        })
        .load(path)
}

struct WoodPaths {
    diffuse: &'static str,
    normal: &'static str,
    arm: &'static str,
}

fn wood_material(
    assets: &AssetServer,
    paths: &WoodPaths,
    tint: Color,
    uv_scale: f32,
) -> StandardMaterial {
    StandardMaterial {
        base_color: tint,
        base_color_texture: Some(load_texture(assets, paths.diffuse, true)),
        normal_map_texture: Some(load_texture(assets, paths.normal, false)),
        metallic_roughness_texture: Some(load_texture(assets, paths.arm, false)),
        occlusion_texture: Some(load_texture(assets, paths.arm, false)),
        perceptual_roughness: 1.0,
        metallic: 1.0,
        uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(uv_scale)),
        ..default()
    }
}

const MAPLE: WoodPaths = WoodPaths {
    diffuse: "textures/maple_diff_1k.jpg",
    normal: "textures/maple_nor_gl_1k.jpg",
    arm: "textures/maple_arm_1k.jpg",
};
const WALNUT: WoodPaths = WoodPaths {
    diffuse: "textures/walnut_diff_1k.jpg",
    normal: "textures/walnut_nor_gl_1k.jpg",
    arm: "textures/walnut_arm_1k.jpg",
};
const OAK: WoodPaths = WoodPaths {
    diffuse: "textures/oak_diff_1k.jpg",
    normal: "textures/oak_nor_gl_1k.jpg",
    arm: "textures/oak_arm_1k.jpg",
};

/// Madeira das peças (B19): buxo nas brancas e ébano nas pretas, materiais PBR próprios
/// aplicados sobre o mesmo modelo Staunton CC0 no lugar dos materiais originais do glTF.
#[derive(Resource, Clone, Debug)]
pub struct PieceWood {
    pub white: Handle<StandardMaterial>,
    pub black: Handle<StandardMaterial>,
}

impl PieceWood {
    pub fn for_side(&self, side: chess_core::Color) -> Handle<StandardMaterial> {
        match side {
            chess_core::Color::White => self.white.clone(),
            chess_core::Color::Black => self.black.clone(),
        }
    }
}

fn piece_wood(assets: &AssetServer, materials: &mut Assets<StandardMaterial>) -> PieceWood {
    // Buxo: madeira clara e amarelada, acabamento acetinado.
    let mut boxwood = wood_material(assets, &MAPLE, Color::srgb(1.0, 0.92, 0.74), 1.6);
    boxwood.perceptual_roughness = 0.75;
    // Ébano: quase preto, com o veio apenas sugerido, mais polido.
    let mut ebony = wood_material(assets, &WALNUT, Color::srgb(0.17, 0.14, 0.13), 1.6);
    ebony.perceptual_roughness = 0.6;
    PieceWood {
        white: materials.add(boxwood),
        black: materials.add(ebony),
    }
}

/// Malha com um quadrado horizontal (normal +Y) por casa, no plano y = 0. As UVs seguem as
/// coordenadas de mundo, com um deslocamento por casa: cada casa mostra um trecho
/// diferente do veio da madeira, como em peças de marchetaria.
fn squares_mesh(squares: &[Square]) -> Mesh {
    const GRAIN: f32 = 0.22;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for sq in squares {
        let c = square_center(*sq);
        let h = SQUARE_SIZE / 2.0;
        let base = u32::try_from(positions.len()).unwrap_or(0);
        let n = f32::from(sq.file() + 8 * sq.rank());
        let (ou, ov) = ((n * 0.381_966).fract(), (n * 0.618_034).fract());
        // Ordem: (-x,-z), (+x,-z), (+x,+z), (-x,+z), vista de cima (+Y).
        for (dx, dz) in [(-h, -h), (h, -h), (h, h), (-h, h)] {
            positions.push([c.x + dx, 0.0, c.z + dz]);
            uvs.push([(dx + h) * GRAIN + ou, (dz + h) * GRAIN + ov]);
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    if let Err(err) = mesh.generate_tangents() {
        warn!("sem tangentes nas casas: {err}");
    }
    mesh
}

fn spawn_board(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(piece_wood(&assets, &mut materials));
    // a1 é escura: coluna + fila par = escura.
    let (dark, light): (Vec<_>, Vec<_>) =
        all_squares().partition(|sq| (sq.file() + sq.rank()) % 2 == 0);
    let light_mat = materials.add(wood_material(
        &assets,
        &MAPLE,
        Color::srgb(1.0, 0.96, 0.88),
        1.0,
    ));
    let dark_mat = materials.add(wood_material(
        &assets,
        &WALNUT,
        Color::srgb(0.62, 0.52, 0.46),
        1.0,
    ));
    for (squares, mat) in [(light, light_mat), (dark, dark_mat)] {
        commands.spawn((
            Name::new("board_squares"),
            Mesh3d(meshes.add(squares_mesh(&squares))),
            MeshMaterial3d(mat),
            Transform::default(),
            VisibleIn::BOTH_3D,
        ));
    }

    // Moldura (borda) de nogueira, um pouco abaixo do plano das casas.
    let frame_mat = materials.add(wood_material(
        &assets,
        &WALNUT,
        Color::srgb(0.50, 0.40, 0.36),
        0.9,
    ));
    commands.spawn((
        Name::new("board_frame"),
        Mesh3d(meshes.add(Cuboid::new(FRAME_WIDTH, FRAME_THICKNESS, FRAME_WIDTH))),
        MeshMaterial3d(frame_mat),
        Transform::from_xyz(0.0, FRAME_TOP - FRAME_THICKNESS / 2.0, 0.0),
        VisibleIn::BOTH_3D,
    ));
}

/// Dimensões da mesa: bem maior que o enquadramento de qualquer Vista, para que nenhuma
/// borda ou faixa de fundo apareça (DEC-04).
const TABLE_SIZE: Vec3 = Vec3::new(90.0, 0.7, 70.0);

/// Mesa discreta (Mesa imersiva) e fundo neutro com sombra suave (3D profissional).
fn spawn_table(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Carvalho dessaturado e fosco: a mesa fica em segundo plano, sem brilho especular forte.
    let mut table = wood_material(&assets, &OAK, Color::srgb(0.40, 0.39, 0.38), 9.0);
    table.reflectance = 0.2;
    let table_mat = materials.add(table);
    commands.spawn((
        Name::new("table"),
        Mesh3d(meshes.add(Cuboid::new(TABLE_SIZE.x, TABLE_SIZE.y, TABLE_SIZE.z))),
        MeshMaterial3d(table_mat),
        Transform::from_xyz(0.0, TABLE_TOP - TABLE_SIZE.y / 2.0, 0.0),
        VisibleIn::IMMERSIVE,
    ));

    // Estúdio: piso neutro fosco que só recebe a sombra do tabuleiro.
    // A cor (e o `unlit`) vêm do Tema: `presentation::apply_scene_background` (issue #206).
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.14, 0.16),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("studio_floor"),
        StudioFloor,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(120.0, 120.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, TABLE_TOP - 0.7, 0.0),
        VisibleIn::PROFESSIONAL,
    ));
}

/// O piso do estúdio da Apresentação 3D profissional (o fundo visível atrás do tabuleiro).
#[derive(Component)]
pub struct StudioFloor;

fn spawn_lights(mut commands: Commands) {
    for role in [LightRole::Key, LightRole::Fill, LightRole::Rim] {
        commands.spawn((
            role,
            DirectionalLight {
                shadow_maps_enabled: role == LightRole::Key,
                ..default()
            },
            // Posição, cor e intensidade vêm da Apresentação (`presentation`).
            Transform::from_xyz(-5.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
    }
}
