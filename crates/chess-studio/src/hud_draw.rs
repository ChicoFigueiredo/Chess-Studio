//! Desenho do HUD (`hud`) no tabuleiro: quadrados translúcidos para casas atacadas, peças
//! ameaçadas e indefesas, e setas. Tudo é posicionado a partir das casas (geometria pura de
//! `chess-input`), então acompanha qualquer órbita, inversão ou zoom (UI-06); nada usa
//! câmera própria nem `Msaa` próprio (CLAUDE.md, issue #82). O 3D usa malhas planas sobre o
//! tabuleiro (profissional e mesa imersiva) e o 2D técnico, sprites e malhas 2D.
//!
//! Os itens são recriados só quando [`HudMarks`] ou os ajustes visuais mudam.

use crate::board2d::UNIT;
use crate::hud::{HudElement, HudMarks, HudSet, HudSettings};
use crate::presentation::VisibleIn;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use chess_core::Square;
use chess_input::{ARROW_TRIANGLES, arrow_outline, square_center};

/// Marca todas as entidades do HUD (recriadas a cada mudança).
#[derive(Component)]
struct HudItem;

/// Lado de cada quadrado do HUD, em casas: o maior fica por baixo, os menores por cima.
fn square_extent(element: HudElement) -> f32 {
    match element {
        HudElement::Attacked => 0.96,
        HudElement::Threatened => 0.74,
        _ => 0.50,
    }
}

/// Altura (3D) do quadrado: acima das casas, abaixo dos destinos e do xeque (0.010-0.015).
fn square_height(element: HudElement) -> f32 {
    match element {
        HudElement::Attacked => 0.004,
        HudElement::Threatened => 0.006,
        _ => 0.008,
    }
}

fn square_depth_2d(element: HudElement) -> f32 {
    match element {
        HudElement::Attacked => 0.20,
        HudElement::Threatened => 0.22,
        _ => 0.24,
    }
}

pub struct HudDrawPlugin;

impl Plugin for HudDrawPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rebuild_hud.after(HudSet));
    }
}

fn color(settings: &HudSettings, element: HudElement) -> Color {
    let [r, g, b] = settings.palette.rgb(element);
    // Setas um pouco mais fortes que as manchas de casa: a seta é fina.
    let boost = match element {
        HudElement::LastMoveArrow | HudElement::EngineArrow | HudElement::HintArrow => 1.5,
        _ => 1.0,
    };
    Color::srgba(r, g, b, (settings.opacity() * boost).min(0.95))
}

fn flat_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// Malha da seta no mundo 3D (plano do tabuleiro, `y = height`) ou no 2D técnico
/// (`x * UNIT`, `-z * UNIT`, profundidade `z`).
fn arrow_mesh(from: Square, to: Square, space: Space) -> Option<Mesh> {
    let outline = arrow_outline(from, to);
    if outline.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = outline
        .iter()
        .map(|p| match space {
            Space::World3d { y } => [p.x, y, p.z],
            Space::Plane2d => [p.x * UNIT, -p.z * UNIT, 0.0],
        })
        .collect();
    let mut indices = Vec::new();
    for [a, b, c] in ARROW_TRIANGLES {
        let (pa, pb, pc) = (
            Vec3::from(positions[a as usize]),
            Vec3::from(positions[b as usize]),
            Vec3::from(positions[c as usize]),
        );
        let normal = (pb - pa).cross(pc - pa);
        // Frente voltada para quem olha: +Y no 3D, +Z no 2D.
        let faces_viewer = match space {
            Space::World3d { .. } => normal.y > 0.0,
            Space::Plane2d => normal.z > 0.0,
        };
        if faces_viewer {
            indices.extend_from_slice(&[a, b, c]);
        } else {
            indices.extend_from_slice(&[a, c, b]);
        }
    }
    let normal = match space {
        Space::World3d { .. } => [0.0, 1.0, 0.0],
        Space::Plane2d => [0.0, 0.0, 1.0],
    };
    let normals = vec![normal; positions.len()];
    // O material 2D lê UVs; a cor é lisa, então todas valem zero.
    let uvs = vec![[0.0_f32, 0.0]; positions.len()];
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

#[derive(Clone, Copy)]
enum Space {
    World3d { y: f32 },
    Plane2d,
}

#[allow(clippy::too_many_arguments)]
fn rebuild_hud(
    marks: Res<HudMarks>,
    settings: Res<HudSettings>,
    items: Query<Entity, With<HudItem>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials_2d: ResMut<Assets<ColorMaterial>>,
) {
    if !(marks.is_changed() || settings.is_changed()) {
        return;
    }
    for entity in &items {
        commands.entity(entity).despawn();
    }
    let squares = [
        (HudElement::Attacked, &marks.attacked),
        (HudElement::Threatened, &marks.threatened),
        (HudElement::Undefended, &marks.undefended),
    ];
    for (element, list) in squares {
        if list.is_empty() {
            continue;
        }
        let extent = square_extent(element);
        let tint = color(&settings, element);
        let mesh = meshes.add(Cuboid::new(extent, 0.004, extent));
        let material = materials.add(flat_material(tint));
        for &square in list {
            let c = square_center(square);
            commands.spawn((
                HudItem,
                Name::new(format!("hud_square3d_{square}")),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(c.x, square_height(element), c.z),
                Visibility::default(),
                VisibleIn::BOTH_3D,
            ));
            commands.spawn((
                HudItem,
                Name::new(format!("hud_square2d_{square}")),
                Sprite::from_color(tint, Vec2::splat(UNIT * extent)),
                Transform::from_xyz(c.x * UNIT, -c.z * UNIT, square_depth_2d(element)),
                Visibility::default(),
                VisibleIn::TECHNICAL,
            ));
        }
    }
    for arrow in &marks.arrows {
        let tint = color(&settings, arrow.element);
        if let Some(mesh) = arrow_mesh(arrow.from, arrow.to, Space::World3d { y: 0.02 }) {
            commands.spawn((
                HudItem,
                Name::new(format!("hud_arrow3d_{}{}", arrow.from, arrow.to)),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(flat_material(tint))),
                Transform::default(),
                Visibility::default(),
                VisibleIn::BOTH_3D,
            ));
        }
        if let Some(mesh) = arrow_mesh(arrow.from, arrow.to, Space::Plane2d) {
            commands.spawn((
                HudItem,
                Name::new(format!("hud_arrow2d_{}{}", arrow.from, arrow.to)),
                Mesh2d(meshes.add(mesh)),
                MeshMaterial2d(materials_2d.add(ColorMaterial::from_color(tint))),
                // A ordem de desenho 2D usa o `z` do Transform, não o dos vértices.
                Transform::from_xyz(0.0, 0.0, 1.6),
                Visibility::default(),
                VisibleIn::TECHNICAL,
            ));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hud::{HudArrow, HudPalette};

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<ColorMaterial>>()
            .init_resource::<HudSettings>()
            .init_resource::<HudMarks>()
            .add_systems(Update, rebuild_hud);
        app
    }

    fn count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&HudItem>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn nothing_is_drawn_without_marks() {
        let mut app = app();
        app.update();
        assert_eq!(count(&mut app), 0);
    }

    #[test]
    fn items_follow_the_marks_in_3d_and_2d() {
        let mut app = app();
        *app.world_mut().resource_mut::<HudMarks>() = HudMarks {
            arrows: vec![HudArrow {
                from: sq("e2"),
                to: sq("e4"),
                element: HudElement::LastMoveArrow,
            }],
            attacked: vec![sq("a3"), sq("b3")],
            threatened: vec![sq("c3")],
            undefended: vec![],
        };
        app.update();
        // 3 casas x (3D + 2D) + 1 seta x (3D + 2D).
        assert_eq!(count(&mut app), 3 * 2 + 2);
        *app.world_mut().resource_mut::<HudMarks>() = HudMarks::default();
        app.update();
        app.update();
        assert_eq!(count(&mut app), 0, "o HUD some quando as marcas somem");
    }

    #[test]
    fn visual_settings_rebuild_without_new_marks() {
        let mut app = app();
        app.world_mut().resource_mut::<HudMarks>().attacked = vec![sq("d4")];
        app.update();
        let before = count(&mut app);
        app.world_mut().resource_mut::<HudSettings>().palette = HudPalette::Contrast;
        app.update();
        assert_eq!(count(&mut app), before, "recriado, sem duplicar");
    }

    #[test]
    fn arrow_meshes_have_the_expected_triangles_in_both_spaces() {
        for space in [Space::World3d { y: 0.02 }, Space::Plane2d] {
            let mesh = arrow_mesh(sq("a1"), sq("h8"), space).unwrap();
            assert_eq!(mesh.indices().unwrap().len(), 9);
        }
        assert!(arrow_mesh(sq("a1"), sq("a1"), Space::World3d { y: 0.0 }).is_none());
    }

    #[test]
    fn arrow_3d_faces_up_in_every_direction() {
        for (a, b) in [
            ("a1", "h8"),
            ("h8", "a1"),
            ("a8", "h1"),
            ("d4", "d8"),
            ("d4", "a4"),
        ] {
            let mesh = arrow_mesh(sq(a), sq(b), Space::World3d { y: 0.0 }).unwrap();
            let positions: Vec<Vec3> = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap()
                .iter()
                .map(|p| Vec3::from(*p))
                .collect();
            let idx: Vec<u32> = mesh.indices().unwrap().iter().map(|i| i as u32).collect();
            for t in idx.chunks(3) {
                let [p, q, r] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
                assert!((q - p).cross(r - p).y > 0.0, "{a}{b}");
            }
        }
    }
}
