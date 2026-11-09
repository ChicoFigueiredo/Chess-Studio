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

use crate::board::{PIECE_SCALE, PieceWood};
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

    pub fn is_complete(&self) -> bool {
        self.by_kind.len() == 12
    }
}

#[derive(Resource, Debug)]
pub struct PieceFont(pub Handle<Font>);

/// Peças desenhadas **no lugar** das da Partida: o editor de posição (C11) mostra a
/// Posição a memorizar e depois o que a pessoa montou, nas três Apresentações. Não é
/// Partida nem Posição canônica: nada aqui decide Lance (ADR-0001).
#[derive(Resource, Default, Debug)]
pub struct BoardOverride {
    pieces: Option<HashMap<Square, (Side, Piece)>>,
    version: u64,
}

impl BoardOverride {
    /// Troca o desenho; só conta como mudança (e redesenha a cena) se for diferente.
    pub fn set(&mut self, pieces: Option<HashMap<Square, (Side, Piece)>>) {
        if self.pieces != pieces {
            self.pieces = pieces;
            self.version = self.version.wrapping_add(1);
        }
    }

    pub fn is_active(&self) -> bool {
        self.pieces.is_some()
    }
}

/// Peças já desenhadas (uma entidade 3D e uma 2D por casa ocupada).
#[derive(Resource, Default, Debug)]
pub(crate) struct BoardPieces {
    by_square: HashMap<Square, Spawned>,
    /// (revisão da Partida, versão do desenho alternativo) da última sincronização.
    synced: Option<(u64, u64)>,
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
            .init_resource::<BoardOverride>()
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
    if !templates.is_complete() {
        warn!(
            "moldes de peças incompletos: {}/12",
            templates.by_kind.len()
        );
    }
    commands.entity(ready.entity).despawn();
}

/// A cena já desenha a Posição atual: sincronizada com a revisão da Partida e com uma
/// entidade por peça esperada (usado pelo estado "cena pronta", B22).
pub(crate) fn pieces_synced(board: &BoardPieces, session: &Session, over: &BoardOverride) -> bool {
    board.synced == Some((session.revision(), over.version))
        && board.by_square.len() == visible_pieces(session, over).len()
}

/// Peças a desenhar: Posição visual provisória (promoção pendente, peça solta em casa
/// ilegal, DEC-32), Lance feito à espera do relógio (art. 4.7) ou a canônica.
pub(crate) fn visible_pieces(
    session: &Session,
    over: &BoardOverride,
) -> HashMap<Square, (Side, Piece)> {
    if let Some(pieces) = &over.pieces {
        return pieces.clone();
    }
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
    if let Some(release) = &snapshot.provisional_release {
        return release
            .pieces
            .iter()
            .map(|&(s, c, p)| (s, (c, p)))
            .collect();
    }
    if let Some(made) = &snapshot.made_move {
        return chess_input::all_squares()
            .filter_map(|s| made.position_after.piece_at(s).map(|cp| (s, cp)))
            .collect();
    }
    chess_input::all_squares()
        .filter_map(|s| snapshot.position.piece_at(s).map(|cp| (s, cp)))
        .collect()
}

fn spawn_piece_3d(
    commands: &mut Commands,
    templates: &PieceTemplates,
    wood: Option<&PieceWood>,
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
                    // Madeira (B19) no lugar do material original do glTF.
                    MeshMaterial3d(wood.map_or(part.material, |w| w.for_side(side))),
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
    wood: Option<Res<PieceWood>>,
    font: Option<Res<PieceFont>>,
    over: Res<BoardOverride>,
    mut board: ResMut<BoardPieces>,
    mut commands: Commands,
) {
    let Some(font) = font else { return };
    let stamp = (session.revision(), over.version);
    if !templates.is_complete() || board.synced == Some(stamp) {
        return;
    }
    board.synced = Some(stamp);
    let desired = visible_pieces(&session, &over);
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
        let e3d = spawn_piece_3d(
            &mut commands,
            &templates,
            wood.as_deref(),
            square,
            side,
            piece,
            at,
        );
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

    // ---- Troca de Apresentação (B12, VIS-05, UX-07, BDD-02) ----------------------------

    use crate::camera::{CameraMemory, CameraRig, switch_camera};
    use crate::presentation::{Presentation, apply_visibility};
    use chess_game::Command;
    use chess_input::CameraView;

    /// App sem janela: só Partida, sincronização das peças, visibilidade e memória de câmera.
    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Presentation>()
            .init_resource::<CameraRig>()
            .init_resource::<CameraMemory>()
            .insert_resource(Session::new())
            .insert_resource(PieceFont(Handle::default()))
            .init_resource::<BoardPieces>()
            .init_resource::<BoardOverride>()
            .add_systems(
                Update,
                (sync_pieces, apply_visibility, switch_camera).chain(),
            );
        let mut templates = PieceTemplates::default();
        for side in [Side::White, Side::Black] {
            for piece in [
                Piece::Pawn,
                Piece::Knight,
                Piece::Bishop,
                Piece::Rook,
                Piece::Queen,
                Piece::King,
            ] {
                templates.by_kind.insert((side, piece), Vec::new());
            }
        }
        app.insert_resource(templates);
        app
    }

    /// Peças da cena (casa, cor, tipo) e quantas entidades 3D e 2D existem.
    fn scene(app: &mut App) -> (Vec<(String, Side, Piece)>, usize, usize) {
        let mut pieces: Vec<_> = app
            .world()
            .resource::<BoardPieces>()
            .by_square
            .iter()
            .map(|(sq, sp)| (sq.to_string(), sp.side, sp.piece))
            .collect();
        pieces.sort_by(|a, b| a.0.cmp(&b.0));
        let world = app.world_mut();
        let n3d = world.query::<&Piece3d>().iter(world).count();
        let n2d = world.query::<&Piece2d>().iter(world).count();
        (pieces, n3d, n2d)
    }

    fn visible(app: &mut App, is_3d_piece: bool) -> bool {
        let world = app.world_mut();
        let mut all = true;
        if is_3d_piece {
            for v in world
                .query_filtered::<&Visibility, With<Piece3d>>()
                .iter(world)
            {
                all &= *v != Visibility::Hidden;
            }
        } else {
            for v in world
                .query_filtered::<&Visibility, With<Piece2d>>()
                .iter(world)
            {
                all &= *v != Visibility::Hidden;
            }
        }
        all
    }

    #[test]
    fn switching_presentation_keeps_the_game_and_the_scene_position() {
        let mut app = headless_app();
        {
            let mut session = app.world_mut().resource_mut::<Session>();
            for (from, to) in [("e2", "e4"), ("e7", "e5"), ("g1", "f3")] {
                session.command(Command::Release {
                    from: sq(from),
                    to: sq(to),
                });
            }
        }
        app.update();
        let snapshot = app.world().resource::<Session>().snapshot().clone();
        let (pieces, n3d, n2d) = scene(&mut app);
        assert_eq!(pieces.len(), 32);
        assert_eq!((n3d, n2d), (32, 32));

        // 3D profissional -> Mesa imersiva -> 2D técnico -> 3D profissional.
        for _ in 0..3 {
            let next = app.world().resource::<Presentation>().next();
            *app.world_mut().resource_mut::<Presentation>() = next;
            app.update();
            app.update();

            let session = app.world().resource::<Session>();
            assert_eq!(session.snapshot(), &snapshot, "a Partida não muda");
            assert_eq!(scene(&mut app), (pieces.clone(), 32, 32), "mesma posição");
            assert_eq!(visible(&mut app, true), next.is_3d());
            assert_eq!(visible(&mut app, false), !next.is_3d());
        }
    }

    #[test]
    fn each_presentation_remembers_its_own_camera() {
        let mut app = headless_app();
        let set_camera = |app: &mut App, view: CameraView| {
            let mut rig = app.world_mut().resource_mut::<CameraRig>();
            rig.pose = view.pose();
            rig.view = Some(view);
        };
        let go = |app: &mut App, p: Presentation| {
            *app.world_mut().resource_mut::<Presentation>() = p;
            app.update();
        };
        let view = |app: &App| app.world().resource::<CameraRig>().view;

        app.update();
        set_camera(&mut app, CameraView::Oblique); // 3D profissional
        go(&mut app, Presentation::ImmersiveTable);
        assert_eq!(view(&app), Some(CameraView::Standard), "nunca visitada");
        set_camera(&mut app, CameraView::Top);
        go(&mut app, Presentation::Technical2d);
        go(&mut app, Presentation::Professional3d);
        assert_eq!(view(&app), Some(CameraView::Oblique));
        assert_eq!(
            app.world().resource::<CameraRig>().pose,
            CameraView::Oblique.pose()
        );
        go(&mut app, Presentation::ImmersiveTable);
        assert_eq!(view(&app), Some(CameraView::Top));
    }
}
