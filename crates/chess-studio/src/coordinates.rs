//! Coordenadas a–h e 1–8 nas bordas do tabuleiro (C16, issue #144), com opção de
//! mostrar/ocultar. No 3D elas ficam impressas na moldura (uma malha de quadrados com um
//! atlas de glifos gerado em código, sem assets novos); no 2D técnico são os textos das
//! margens (`board2d`). Só visual: não entram no picking (que é matemática de raio contra
//! o plano das casas) nem no enquadramento da câmera (que usa só os pontos do tabuleiro).
//!
//! Orientação: os rótulos são fixos no mundo, com a1 e as colunas/filas sempre nas casas
//! certas. Há duas cópias, uma legível do lado das brancas e outra girada 180° legível do
//! lado das pretas; assim a Vista invertida só gira a câmera e continua legível.

use crate::presentation::Presentation;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use chess_core::Square;
use chess_input::{BOARD_SIZE, square_center};

/// Opção "Coordenadas no tabuleiro" (ligada por padrão; preferência persistida).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardCoordinates(pub bool);

impl Default for BoardCoordinates {
    fn default() -> Self {
        Self(true)
    }
}

/// Rótulos 3D de um lado da moldura: `turned = false` é o conjunto legível do lado das
/// brancas; `turned = true`, o girado 180° legível do lado das pretas. Só o conjunto do
/// lado da câmera aparece (o do lado oposto ficaria de cabeça para baixo e atrás das peças).
#[derive(Component)]
pub struct Labels3d {
    pub turned: bool,
}

/// Qual conjunto de rótulos 3D a câmera lê de pé: `right_x` é o eixo "direita" da câmera
/// no eixo x do mundo (+1 nas Vistas das brancas e na superior; -1 na Vista invertida).
pub fn reads_turned(right_x: f32) -> bool {
    right_x < -1e-3
}

/// Rótulo 2D técnico (texto nas margens).
#[derive(Component)]
pub struct Labels2d;

/// Largura da moldura além das casas: o rótulo fica no meio dessa faixa.
const FRAME_BORDER: f32 = 0.6;
/// Distância do centro do tabuleiro ao centro da faixa de rótulos.
const EDGE: f32 = BOARD_SIZE / 2.0 + FRAME_BORDER / 2.0;
/// Altura e largura do glifo no mundo (cabem na faixa de 0,6 da moldura).
const GLYPH_H: f32 = 0.34;
const GLYPH_W: f32 = GLYPH_H * (CELL_W as f32) / (CELL_H as f32);
/// Face da moldura fica em y = -0,02; o rótulo flutua um pouco acima para não brigar com ela.
const LABEL_Y: f32 = -0.014;

/// Um rótulo na moldura: o glifo, o centro no plano do tabuleiro (x, z) e se está girado 180°
/// (legível do lado das pretas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelSlot {
    pub glyph: char,
    pub center: Vec2,
    pub turned: bool,
}

impl LabelSlot {
    /// Os quatro cantos no mundo, na ordem (esq-cima, dir-cima, dir-baixo, esq-baixo) do ponto
    /// de vista de quem lê o texto.
    pub fn corners(self) -> [Vec3; 4] {
        let sign = if self.turned { -1.0 } else { 1.0 };
        let (hw, hh) = (GLYPH_W / 2.0, GLYPH_H / 2.0);
        // Leitor das brancas: direita = +x, em cima = -z.
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(dx, dz)| {
            Vec3::new(
                self.center.x + sign * dx,
                LABEL_Y,
                self.center.y + sign * dz,
            )
        })
    }
}

/// Todos os rótulos da moldura: 16 glifos × 2 (lado das brancas e lado das pretas).
pub fn label_slots() -> Vec<LabelSlot> {
    let mut slots = Vec::with_capacity(32);
    for file in 0..8u8 {
        let Some(sq) = Square::new(file, 0) else {
            continue;
        };
        let along = square_center(sq).x;
        let glyph = char::from(b'a' + file);
        // Letras abaixo da fila 1 (brancas) e acima da fila 8 (pretas).
        slots.push(LabelSlot {
            glyph,
            center: Vec2::new(along, EDGE),
            turned: false,
        });
        slots.push(LabelSlot {
            glyph,
            center: Vec2::new(along, -EDGE),
            turned: true,
        });
    }
    for rank in 0..8u8 {
        let Some(sq) = Square::new(0, rank) else {
            continue;
        };
        let along = square_center(sq).z;
        let glyph = char::from(b'1' + rank);
        // Números à esquerda da coluna a (brancas) e à direita da coluna h (pretas).
        slots.push(LabelSlot {
            glyph,
            center: Vec2::new(-EDGE, along),
            turned: false,
        });
        slots.push(LabelSlot {
            glyph,
            center: Vec2::new(EDGE, along),
            turned: true,
        });
    }
    slots
}

// ---------------------------------------------------------------------------------------
// Glifos: traços vetoriais num quadrado 0..1 (y para cima), rasterizados num atlas.
// ---------------------------------------------------------------------------------------

const GLYPHS: [char; 16] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', '1', '2', '3', '4', '5', '6', '7', '8',
];
/// Tamanho de uma célula do atlas em pixels.
const CELL_W: usize = 40;
const CELL_H: usize = 64;
/// Meia espessura do traço, em pixels da célula.
const HALF_STROKE: f32 = 3.0;

type Poly = Vec<(f32, f32)>;

fn arc(c: (f32, f32), r: (f32, f32), from: f32, to: f32) -> Poly {
    let n = 28;
    (0..=n)
        .map(|i| {
            let t = from + (to - from) * i as f32 / n as f32;
            let a = t.to_radians();
            (c.0 + r.0 * a.cos(), c.1 + r.1 * a.sin())
        })
        .collect()
}

fn ellipse(c: (f32, f32), r: (f32, f32)) -> Poly {
    arc(c, r, 0.0, 360.0)
}

/// Traços (polilinhas) do glifo; vazio se o caractere não existe.
fn strokes(glyph: char) -> Vec<Poly> {
    match glyph {
        'a' => vec![
            ellipse((0.4, 0.3), (0.3, 0.3)),
            vec![(0.7, 0.0), (0.7, 0.6)],
        ],
        'b' => vec![
            vec![(0.3, 0.0), (0.3, 1.0)],
            ellipse((0.6, 0.3), (0.3, 0.3)),
        ],
        'c' => vec![arc((0.55, 0.3), (0.4, 0.3), 40.0, 320.0)],
        'd' => vec![
            vec![(0.7, 0.0), (0.7, 1.0)],
            ellipse((0.4, 0.3), (0.3, 0.3)),
        ],
        'e' => vec![
            arc((0.5, 0.3), (0.38, 0.3), 0.0, 325.0),
            vec![(0.12, 0.3), (0.88, 0.3)],
        ],
        'f' => vec![
            arc((0.75, 0.75), (0.25, 0.25), 0.0, 180.0)
                .into_iter()
                .chain([(0.5, 0.0)])
                .collect(),
            vec![(0.25, 0.6), (0.8, 0.6)],
        ],
        'g' => vec![
            ellipse((0.4, 0.33), (0.3, 0.3)),
            vec![
                (0.7, 0.63),
                (0.7, -0.02),
                (0.6, -0.12),
                (0.4, -0.15),
                (0.22, -0.08),
            ],
        ],
        'h' => vec![
            vec![(0.3, 0.0), (0.3, 1.0)],
            vec![
                (0.3, 0.45),
                (0.38, 0.6),
                (0.5, 0.68),
                (0.62, 0.66),
                (0.7, 0.55),
                (0.7, 0.0),
            ],
        ],
        '1' => vec![vec![(0.28, 0.78), (0.5, 1.0), (0.5, 0.0)]],
        '2' => vec![vec![
            (0.15, 0.8),
            (0.25, 0.95),
            (0.5, 1.0),
            (0.75, 0.93),
            (0.82, 0.75),
            (0.7, 0.55),
            (0.15, 0.0),
            (0.85, 0.0),
        ]],
        '3' => vec![
            vec![
                (0.15, 0.9),
                (0.4, 1.0),
                (0.7, 0.95),
                (0.8, 0.78),
                (0.65, 0.6),
                (0.4, 0.55),
            ],
            vec![
                (0.4, 0.55),
                (0.7, 0.5),
                (0.85, 0.3),
                (0.75, 0.08),
                (0.45, 0.0),
                (0.15, 0.1),
            ],
        ],
        '4' => vec![vec![(0.65, 0.0), (0.65, 1.0), (0.1, 0.3), (0.9, 0.3)]],
        '5' => vec![vec![
            (0.8, 1.0),
            (0.2, 1.0),
            (0.15, 0.55),
            (0.5, 0.62),
            (0.75, 0.55),
            (0.85, 0.32),
            (0.72, 0.08),
            (0.45, 0.0),
            (0.15, 0.1),
        ]],
        '6' => vec![vec![
            (0.75, 0.95),
            (0.5, 1.0),
            (0.25, 0.85),
            (0.15, 0.5),
            (0.18, 0.2),
            (0.4, 0.0),
            (0.65, 0.05),
            (0.82, 0.25),
            (0.8, 0.45),
            (0.6, 0.58),
            (0.35, 0.55),
            (0.17, 0.4),
        ]],
        '7' => vec![vec![(0.1, 1.0), (0.9, 1.0), (0.4, 0.0)]],
        '8' => vec![
            ellipse((0.5, 0.76), (0.27, 0.24)),
            ellipse((0.5, 0.27), (0.33, 0.27)),
        ],
        _ => Vec::new(),
    }
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0)
    };
    let (dx, dy) = (p.0 - (a.0 + t * abx), p.1 - (a.1 + t * aby));
    (dx * dx + dy * dy).sqrt()
}

/// Atlas RGBA8 (branco, alfa = cobertura do traço): uma linha com os 16 glifos de `GLYPHS`.
/// Devolve (bytes, largura, altura).
pub fn render_atlas() -> (Vec<u8>, usize, usize) {
    let (w, h) = (CELL_W * GLYPHS.len(), CELL_H);
    let mut data = vec![255u8; w * h * 4];
    for (index, glyph) in GLYPHS.iter().enumerate() {
        // Traços em pixels da célula: o glifo ocupa 80% da célula, centralizado.
        let to_px = |(u, v): (f32, f32)| {
            (
                CELL_W as f32 * (0.1 + 0.8 * u),
                CELL_H as f32 * (0.1 + 0.8 * (1.0 - v)),
            )
        };
        let segments: Vec<((f32, f32), (f32, f32))> = strokes(*glyph)
            .iter()
            .flat_map(|poly| {
                poly.windows(2)
                    .map(|pair| (to_px(pair[0]), to_px(pair[1])))
                    .collect::<Vec<_>>()
            })
            .collect();
        for py in 0..CELL_H {
            for px in 0..CELL_W {
                let p = (px as f32 + 0.5, py as f32 + 0.5);
                let d = segments
                    .iter()
                    .map(|(a, b)| dist_to_segment(p, *a, *b))
                    .fold(f32::INFINITY, f32::min);
                let alpha = (HALF_STROKE + 0.5 - d).clamp(0.0, 1.0);
                let x = index * CELL_W + px;
                data[(py * w + x) * 4 + 3] = (alpha * 255.0).round() as u8;
            }
        }
    }
    (data, w, h)
}

/// Faixa horizontal (u0, u1) do glifo no atlas.
fn uv_range(glyph: char) -> Option<(f32, f32)> {
    let i = GLYPHS.iter().position(|g| *g == glyph)?;
    let n = GLYPHS.len() as f32;
    Some((i as f32 / n, (i as f32 + 1.0) / n))
}

/// Malha com um quadrado horizontal (normal +Y) por rótulo, UVs apontando para o atlas.
pub fn labels_mesh(slots: &[LabelSlot]) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for slot in slots {
        let Some((u0, u1)) = uv_range(slot.glyph) else {
            continue;
        };
        let base = u32::try_from(positions.len()).unwrap_or(0);
        // Cantos (esq-cima, dir-cima, dir-baixo, esq-baixo) do ponto de vista do leitor.
        let corner_uvs = [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]];
        for (corner, uv) in slot.corners().into_iter().zip(corner_uvs) {
            positions.push(corner.to_array());
            uvs.push(uv);
        }
        // Vistos de cima (+Y) os cantos giram em sentido anti-horário: (0,2,1) e (0,3,2).
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn atlas_image() -> Image {
    let (data, w, h) = render_atlas();
    Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub struct CoordinatesPlugin;

impl Plugin for CoordinatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardCoordinates>()
            .add_systems(Startup, spawn_labels_3d)
            .add_systems(Update, (coordinates_shortcut, sync_visibility).chain());
    }
}

fn spawn_labels_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Tinta clara sobre a nogueira escura da moldura, discreta e fosca.
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.86, 0.74, 0.55, 0.9),
        base_color_texture: Some(images.add(atlas_image())),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.9,
        reflectance: 0.1,
        depth_bias: 1.0,
        ..default()
    });
    for turned in [false, true] {
        let slots: Vec<LabelSlot> = label_slots()
            .into_iter()
            .filter(|s| s.turned == turned)
            .collect();
        commands.spawn((
            Name::new("board_coordinates"),
            Mesh3d(meshes.add(labels_mesh(&slots))),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            Labels3d { turned },
        ));
    }
}

/// C (remapeável, `controls`) alterna as coordenadas. Não age se o egui quer o teclado.
fn coordinates_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<crate::controls::Controls>,
    egui: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut coordinates: ResMut<BoardCoordinates>,
) {
    let typing = egui.is_some_and(|e| e.wants_keyboard_input());
    if !typing && controls.key_just_pressed(chess_input::controls::Action::ToggleCoordinates, &keys)
    {
        coordinates.0 = !coordinates.0;
    }
}

/// Mostra os rótulos da Apresentação atual quando a opção está ligada; no 3D, só o conjunto
/// do lado da câmera. Roda todo quadro (poucas entidades), escrevendo só quando muda.
#[allow(clippy::type_complexity)]
fn sync_visibility(
    coordinates: Res<BoardCoordinates>,
    presentation: Res<Presentation>,
    camera: Option<Single<&Transform, With<crate::camera::MainCamera>>>,
    mut q3d: Query<(&Labels3d, &mut Visibility), Without<Labels2d>>,
    mut q2d: Query<&mut Visibility, (With<Labels2d>, Without<Labels3d>)>,
) {
    let turned = camera.is_some_and(|t| reads_turned(t.right().x));
    for (labels, mut vis) in &mut q3d {
        let want = visibility_for(
            coordinates.0 && labels.turned == turned,
            presentation.is_3d(),
        );
        if *vis != want {
            *vis = want;
        }
    }
    let want_2d = visibility_for(coordinates.0, !presentation.is_3d());
    for mut vis in &mut q2d {
        if *vis != want_2d {
            *vis = want_2d;
        }
    }
}

fn visibility_for(on: bool, in_presentation: bool) -> Visibility {
    if on && in_presentation {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(glyph: char, turned: bool) -> LabelSlot {
        label_slots()
            .into_iter()
            .find(|s| s.glyph == glyph && s.turned == turned)
            .expect("rótulo existe")
    }

    fn sq(name: &str) -> Square {
        name.parse().expect("casa válida")
    }

    #[test]
    fn there_are_sixteen_glyphs_on_each_side() {
        let slots = label_slots();
        assert_eq!(slots.len(), 32);
        assert_eq!(slots.iter().filter(|s| s.turned).count(), 16);
    }

    #[test]
    fn files_sit_under_their_columns_on_the_white_side() {
        for (i, name) in ["a1", "b1", "c1", "d1", "e1", "f1", "g1", "h1"]
            .iter()
            .enumerate()
        {
            let glyph = char::from(b'a' + i as u8);
            let s = slot(glyph, false);
            let c = square_center(sq(name));
            assert!((s.center.x - c.x).abs() < 1e-5, "{glyph}");
            // Fora do tabuleiro, do lado da fila 1 (brancas).
            assert!(s.center.y > BOARD_SIZE / 2.0 && c.z > 0.0);
        }
    }

    #[test]
    fn ranks_sit_beside_their_rows_with_a1_in_the_right_corner() {
        for (i, name) in ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8"]
            .iter()
            .enumerate()
        {
            let glyph = char::from(b'1' + i as u8);
            let s = slot(glyph, false);
            let c = square_center(sq(name));
            assert!((s.center.y - c.z).abs() < 1e-5, "{glyph}");
            assert!(s.center.x < -BOARD_SIZE / 2.0);
        }
        // a1: a letra 'a' e o número '1' do lado das brancas ficam junto do canto de a1.
        let a1 = square_center(sq("a1"));
        assert_eq!(slot('a', false).center.x, a1.x);
        assert_eq!(slot('1', false).center.y, a1.z);
    }

    #[test]
    fn black_side_copies_mirror_the_white_side_through_the_center() {
        for g in GLYPHS {
            let (w, b) = (slot(g, false), slot(g, true));
            if g.is_ascii_alphabetic() {
                assert_eq!(w.center.x, b.center.x);
                assert_eq!(w.center.y, -b.center.y);
            } else {
                assert_eq!(w.center.y, b.center.y);
                assert_eq!(w.center.x, -b.center.x);
            }
        }
    }

    #[test]
    fn every_label_fits_inside_the_frame_border() {
        let (inner, outer) = (BOARD_SIZE / 2.0, BOARD_SIZE / 2.0 + FRAME_BORDER);
        for s in label_slots() {
            for c in s.corners() {
                let m = c.x.abs().max(c.z.abs());
                assert!(
                    m > inner + 0.05 && m < outer - 0.05,
                    "{:?} em {c:?}",
                    s.glyph
                );
            }
        }
    }

    #[test]
    fn labels_stay_on_a_plane_just_above_the_frame_face() {
        for s in label_slots() {
            for c in s.corners() {
                assert!(c.y > -0.02 && c.y < 0.0);
            }
        }
    }

    #[test]
    fn white_side_text_reads_left_to_right_and_black_side_is_turned() {
        let w = slot('a', false).corners();
        assert!(w[1].x > w[0].x && w[3].z > w[0].z);
        let b = slot('a', true).corners();
        assert!(b[1].x < b[0].x && b[3].z < b[0].z);
    }

    #[test]
    fn mesh_has_a_quad_per_label_and_atlas_covers_every_glyph() {
        let slots = label_slots();
        let mesh = labels_mesh(&slots);
        assert_eq!(mesh.count_vertices(), slots.len() * 4);
        let (data, w, h) = render_atlas();
        assert_eq!(data.len(), w * h * 4);
        for (i, g) in GLYPHS.iter().enumerate() {
            let ink: usize = (0..CELL_H)
                .flat_map(|y| (0..CELL_W).map(move |x| (x, y)))
                .filter(|(x, y)| data[(y * w + i * CELL_W + x) * 4 + 3] > 128)
                .count();
            assert!(ink > 80, "glifo {g} sem tinta suficiente ({ink})");
            assert!(ink < CELL_W * CELL_H / 2, "glifo {g} borrado ({ink})");
        }
    }

    #[test]
    fn visibility_needs_the_option_and_the_matching_presentation() {
        assert_eq!(visibility_for(true, true), Visibility::Inherited);
        assert_eq!(visibility_for(false, true), Visibility::Hidden);
        assert_eq!(visibility_for(true, false), Visibility::Hidden);
    }

    #[test]
    fn the_camera_side_picks_which_label_set_reads_upright() {
        assert!(!reads_turned(1.0));
        assert!(!reads_turned(0.2));
        assert!(reads_turned(-1.0));
        assert!(reads_turned(-0.5));
    }

    #[test]
    fn coordinates_are_on_by_default() {
        assert!(BoardCoordinates::default().0);
    }
}
