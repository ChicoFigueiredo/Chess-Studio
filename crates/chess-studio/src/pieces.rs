//! Peças da cena, reconstruídas **a partir do `snapshot()`** da Partida (ADR-0001): as
//! Apresentações 3D e o 2D técnico mostram sempre a Posição canônica (ou, na promoção
//! pendente, a Posição visual provisória do próprio snapshot). Nada aqui decide lance.
//!
//! Os modelos vêm do glTF "Chess Set": ao carregar, a primeira peça de cada tipo e cor
//! vira um molde de malhas/materiais e a cena original é descartada. Moldes permitem
//! quantas damas a promoção exigir.
//!
//! Animação (INT-05): o destino de um encaixe é sempre a casa canônica; o ponto de partida
//! vem do gesto (cursor ao soltar) ou da casa de onde a peça veio (deslize).

use crate::board::PIECE_SCALE;
use crate::board2d::UNIT;
use crate::layout::{glyph, parse_node_name};
use crate::play::PlaySet;
use crate::presentation::VisibleIn;
use crate::session::Session;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use chess_core::{Color as Side, Piece, Square};
use chess_input::{DRAG_HEIGHT, SETTLE_SECS, settle_position, square_center};
use std::collections::HashMap;

/// Altura da peça apenas selecionada (por clique), sem arrasto.
const SELECT_HEIGHT: f32 = 0.3;

/// Parte de um molde: malha e material com a posição relativa à peça.
#[derive(Debug, Clone)]
struct TemplatePart {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
}

type PartQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static Mesh3d>,
        Option<&'static MeshMaterial3d<StandardMaterial>>,
    ),
>;

#[derive(Resource, Default, Debug)]
pub struct PieceTemplates {
    by_kind: HashMap<(Side, Piece), Vec<TemplatePart>>,
}

impl PieceTemplates {
    /// Malhas e posições relativas da peça (para as peças fantasmas dos Auxílios).
    pub fn ghost_parts(&self, side: Side, piece: Piece) -> Vec<(Handle<Mesh>, Transform)> {
        self.by_kind
            .get(&(side, piece))
            .map(|parts| {
                parts
                    .iter()
                    .map(|p| (p.mesh.clone(), p.transform))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn complete(&self) -> bool {
        self.by_kind.len() == 12
    }
}

#[derive(Resource, Debug)]
pub struct PieceFont(pub Handle<Font>);

/// Peças já desenhadas (uma entidade 3D e uma 2D por casa ocupada).
#[derive(Resource, Default, Debug)]
struct BoardPieces {
    by_square: HashMap<Square, Spawned>,
    synced: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct Spawned {
    side: Side,
    piece: Piece,
    e3d: Entity,
    e2d: Entity,
}

/// Casa canônica onde a peça repousa.
#[derive(Component, Debug, Clone, Copy)]
struct PieceAt(Square);

#[derive(Component)]
struct Piece3d;
#[derive(Component)]
struct Piece2d;

/// Encaixe em andamento: a peça vai de `from` ao centro de `target`.
#[derive(Component, Debug, Clone, Copy)]
struct Settle {
    from: Vec3,
    elapsed: f32,
}

pub struct PiecesPlugin;

impl Plugin for PiecesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PieceTemplates>()
            .init_resource::<BoardPieces>()
            .add_systems(Startup, load_piece_assets)
            .add_systems(
                Update,
                (sync_pieces, place_resting, animate_settle)
                    .chain()
                    .after(PlaySet),
            );
    }
}

fn load_piece_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(PieceFont(assets.load("fonts/NotoSansSymbols2-chess.ttf")));
    commands
        .spawn((
            Name::new("pieces_source"),
            WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset("models/chess_pieces.glb")),
            ),
            Visibility::Hidden,
        ))
        .observe(capture_templates);
}

/// Malhas da peça `entity` (e descendentes), com transformações relativas a ela.
fn collect_parts(
    entity: Entity,
    acc: Transform,
    is_root: bool,
    children: &Query<&Children>,
    parts: &PartQuery,
    out: &mut Vec<TemplatePart>,
) {
    let Ok((transform, mesh, material)) = parts.get(entity) else {
        return;
    };
    let local = if is_root {
        Transform::IDENTITY
    } else {
        acc.mul_transform(*transform)
    };
    if let (Some(mesh), Some(material)) = (mesh, material) {
        out.push(TemplatePart {
            mesh: mesh.0.clone(),
            material: material.0.clone(),
            transform: local,
        });
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_parts(child, local, false, children, parts, out);
        }
    }
}

fn capture_templates(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    names: Query<&Name>,
    parts: PartQuery,
    mut templates: ResMut<PieceTemplates>,
    mut commands: Commands,
) {
    for entity in children.iter_descendants(ready.entity) {
        let Some((side, piece)) = names.get(entity).ok().and_then(|n| parse_node_name(n)) else {
            continue;
        };
        if templates.by_kind.contains_key(&(side, piece)) {
            continue;
        }
        let mut out = Vec::new();
        collect_parts(
            entity,
            Transform::IDENTITY,
            true,
            &children,
            &parts,
            &mut out,
        );
        templates.by_kind.insert((side, piece), out);
    }
    if !templates.complete() {
        warn!(
            "moldes de peças incompletos: {}/12",
            templates.by_kind.len()
        );
    }
    commands.entity(ready.entity).despawn();
}

/// Peças a desenhar: Posição visual provisória (promoção pendente) ou canônica.
fn visible_pieces(session: &Session) -> HashMap<Square, (Side, Piece)> {
    // Revisão Lance a Lance: mostra a Posição revisada (somente leitura).
    if let Some(review) = session.review() {
        return chess_input::all_squares()
            .filter_map(|s| review.position.piece_at(s).map(|cp| (s, cp)))
            .collect();
    }
    let snapshot = session.snapshot();
    if let Some(view) = &snapshot.provisional {
        return view.pieces.iter().map(|&(s, c, p)| (s, (c, p))).collect();
    }
    chess_input::all_squares()
        .filter_map(|s| snapshot.position.piece_at(s).map(|cp| (s, cp)))
        .collect()
}

fn spawn_piece_3d(
    commands: &mut Commands,
    templates: &PieceTemplates,
    square: Square,
    side: Side,
    piece: Piece,
    at: Vec3,
) -> Entity {
    let facing = if side == Side::White {
        std::f32::consts::PI
    } else {
        0.0
    };
    let parts = templates
        .by_kind
        .get(&(side, piece))
        .cloned()
        .unwrap_or_default();
    commands
        .spawn((
            Name::new(format!("piece3d_{square}")),
            Piece3d,
            PieceAt(square),
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_y(facing))
                .with_scale(Vec3::splat(PIECE_SCALE)),
            Visibility::default(),
            VisibleIn::BOTH_3D,
        ))
        .with_children(|parent| {
            for part in parts {
                parent.spawn((
                    Mesh3d(part.mesh),
                    MeshMaterial3d(part.material),
                    part.transform,
                ));
            }
        })
        .id()
}

/// Peça do 2D técnico: glifo preenchido com contorno de cor oposta (legível em casas
/// claras e escuras).
fn spawn_piece_2d(
    commands: &mut Commands,
    font: &Handle<Font>,
    square: Square,
    side: Side,
    piece: Piece,
) -> Entity {
    let (fill, outline) = if side == Side::White {
        (Color::srgb(0.98, 0.97, 0.92), Color::srgb(0.08, 0.08, 0.09))
    } else {
        (Color::srgb(0.06, 0.06, 0.07), Color::srgb(0.95, 0.95, 0.90))
    };
    let glyph = |color: Color, x: f32, y: f32, z: f32| {
        (
            Text2d::new(glyph(piece).to_string()),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(84.0),
                ..default()
            },
            TextColor(color),
            Transform::from_xyz(x, y, z),
        )
    };
    commands
        .spawn((
            Name::new(format!("piece2d_{square}")),
            Piece2d,
            PieceAt(square),
            rest_transform_2d(square_center(square)),
            Visibility::default(),
            VisibleIn::TECHNICAL,
        ))
        .with_children(|parent| {
            for (dx, dy) in [(-2.5, 0.0), (2.5, 0.0), (0.0, -2.5), (0.0, 2.5)] {
                parent.spawn(glyph(outline, dx, dy, 0.0));
            }
            parent.spawn(glyph(fill, 0.0, 0.0, 0.5));
        })
        .id()
}

/// Transformação 2D para uma posição do mundo 3D (a altura vira escala e profundidade).
fn rest_transform_2d(pos: Vec3) -> Transform {
    let plane = square_pos_of(pos);
    Transform::from_xyz(plane.x, plane.y - 4.0, 2.0 + pos.y)
        .with_scale(Vec3::splat(1.0 + pos.y * 0.25))
}

fn square_pos_of(pos: Vec3) -> Vec2 {
    Vec2::new(pos.x * UNIT, -pos.z * UNIT)
}

#[allow(clippy::too_many_arguments)]
fn sync_pieces(
    mut session: ResMut<Session>,
    templates: Res<PieceTemplates>,
    font: Option<Res<PieceFont>>,
    mut board: ResMut<BoardPieces>,
    mut commands: Commands,
) {
    let Some(font) = font else { return };
    if !templates.complete() || board.synced == Some(session.revision()) {
        return;
    }
    board.synced = Some(session.revision());
    let desired = visible_pieces(&session);
    let hint = session.take_hint();

    // Peças que saíram da casa (ou mudaram de tipo): somem; as iguais ficam.
    let mut vanished: Vec<(Square, Side, Piece)> = Vec::new();
    let gone: Vec<Square> = board
        .by_square
        .iter()
        .filter(|(sq, sp)| desired.get(sq) != Some(&(sp.side, sp.piece)))
        .map(|(sq, _)| *sq)
        .collect();
    for square in gone {
        if let Some(sp) = board.by_square.remove(&square) {
            vanished.push((square, sp.side, sp.piece));
            commands.entity(sp.e3d).despawn();
            commands.entity(sp.e2d).despawn();
        }
    }

    // Peças novas: nascem na origem do deslize, do cursor ao soltar ou já no lugar.
    let mut newcomers: Vec<(Square, Side, Piece)> = desired
        .iter()
        .filter(|(sq, _)| !board.by_square.contains_key(sq))
        .map(|(sq, (c, p))| (*sq, *c, *p))
        .collect();
    newcomers.sort_by_key(|(sq, _, _)| (sq.rank(), sq.file()));
    for (square, side, piece) in newcomers {
        let start = if hint.is_some_and(|h| h.square == square) {
            hint.map(|h| h.from)
        } else {
            nearest_source(&mut vanished, square, side, piece)
        };
        let at = start.unwrap_or_else(|| square_center(square));
        let e3d = spawn_piece_3d(&mut commands, &templates, square, side, piece, at);
        let e2d = spawn_piece_2d(&mut commands, &font.0, square, side, piece);
        commands.entity(e2d).insert(rest_transform_2d(at));
        if let Some(from) = start {
            let settle = Settle { from, elapsed: 0.0 };
            commands.entity(e3d).insert(settle);
            commands.entity(e2d).insert(settle);
        }
        board.by_square.insert(
            square,
            Spawned {
                side,
                piece,
                e3d,
                e2d,
            },
        );
    }

    // Lance ilegal: nada nasceu, a peça solta volta à origem.
    if let Some(hint) = hint
        && let Some(sp) = board.by_square.get(&hint.square)
    {
        let settle = Settle {
            from: hint.from,
            elapsed: 0.0,
        };
        commands.entity(sp.e3d).insert(settle);
        commands.entity(sp.e2d).insert(settle);
    }
}

/// Casa de onde veio uma peça nova: a mais próxima, entre as que sumiram, com o mesmo
/// tipo e cor (roque, desfazer). Consome a candidata.
fn nearest_source(
    vanished: &mut Vec<(Square, Side, Piece)>,
    to: Square,
    side: Side,
    piece: Piece,
) -> Option<Vec3> {
    let target = square_center(to);
    let (index, _) = vanished
        .iter()
        .enumerate()
        .filter(|(_, (_, c, p))| *c == side && *p == piece)
        .map(|(i, (sq, _, _))| (i, square_center(*sq).distance(target)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let (square, _, _) = vanished.remove(index);
    Some(square_center(square))
}

#[allow(clippy::type_complexity)]
/// Peças em repouso ficam no centro da casa; a selecionada/arrastada acompanha o gesto.
fn place_resting(
    session: Res<Session>,
    mut pieces: Query<
        (&PieceAt, &mut Transform, Has<Piece3d>),
        (Or<(With<Piece3d>, With<Piece2d>)>, Without<Settle>),
    >,
) {
    let visual = session.gesture_visual();
    for (at, mut transform, is_3d) in &mut pieces {
        let mut pos = square_center(at.0);
        if visual.lifted && visual.origin == Some(at.0) {
            match visual.drag_world {
                Some(world) => pos = Vec3::new(world.x, DRAG_HEIGHT, world.z),
                None => pos.y = SELECT_HEIGHT,
            }
        }
        if is_3d {
            transform.translation = pos;
        } else {
            let t = rest_transform_2d(pos);
            transform.translation = t.translation;
            transform.scale = t.scale;
        }
    }
}

fn animate_settle(
    time: Res<Time>,
    mut commands: Commands,
    mut pieces: Query<(Entity, &PieceAt, &mut Settle, &mut Transform, Has<Piece3d>)>,
) {
    for (entity, at, mut settle, mut transform, is_3d) in &mut pieces {
        settle.elapsed += time.delta_secs();
        let t = settle.elapsed / SETTLE_SECS;
        let pos = settle_position(settle.from, at.0, t);
        if is_3d {
            transform.translation = pos;
        } else {
            let placed = rest_transform_2d(pos);
            transform.translation = placed.translation;
            transform.scale = placed.scale;
        }
        if t >= 1.0 {
            commands.entity(entity).remove::<Settle>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board2d::square_pos;

    fn sq(name: &str) -> Square {
        name.parse().unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn nearest_source_picks_the_closest_vanished_piece_of_the_same_kind() {
        let mut vanished = vec![
            (sq("a1"), Side::White, Piece::Rook),
            (sq("h1"), Side::White, Piece::Rook),
            (sq("e1"), Side::White, Piece::King),
        ];
        let from = nearest_source(&mut vanished, sq("f1"), Side::White, Piece::Rook);
        assert_eq!(from, Some(square_center(sq("h1"))));
        assert_eq!(vanished.len(), 2, "a candidata é consumida");
        assert_eq!(
            nearest_source(&mut vanished, sq("c4"), Side::Black, Piece::Rook),
            None
        );
    }

    #[test]
    fn square_pos_of_matches_the_2d_board_layout() {
        for s in chess_input::all_squares() {
            assert_eq!(square_pos_of(square_center(s)), square_pos(s));
        }
    }
}
