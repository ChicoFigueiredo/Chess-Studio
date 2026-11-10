//! Relógio de xadrez da mesa (DEC-33): objeto 3D feito por código (3D profissional e Mesa
//! imersiva), widget no 2D técnico e a tecla Espaço. Os três acionam o **mesmo** Comando
//! (`Session::press_clock`), então a Política é idêntica em todas as Apresentações (AC-02).
//!
//! O objeto é uma cunha com dois mostradores de sete segmentos (tempo legível de longe) e
//! dois botões no degrau da frente. O usuário nunca tem o relógio acionado sozinho: só o
//! botão do lado da vez responde ao clique, e a Engine (B9) aciona o dela por Comando.
//! Nada aqui decide regra: o tempo vem do `Snapshot` (`Session::clock_reading`).

use crate::i18n::I18n;
use crate::play::PlaySet;
use crate::presentation::{Presentation, VisibleIn};
use crate::session::Session;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::input::EguiWantsInput;
use chess_core::Color as Side;

/// Altura do tampo da mesa (a mesma do `board`): o relógio apoia nele.
const TABLE_TOP: f32 = -0.42;
/// Posição do relógio ao lado do tabuleiro, do ponto de vista do usuário.
const CLOCK_X: f32 = 6.45;
/// O relógio é um pouco menor que o desenho em unidades: cabe ao lado do tabuleiro e da vista.
const CLOCK_SCALE: f32 = 0.82;
const CLOCK_Z: f32 = 0.6;

/// Perfil da cunha no plano (z, y): o degrau da frente vai até z = 1,15 e a rampa dos
/// mostradores sobe até y = 1,05 no fundo.
const RAMP_FRONT: (f32, f32) = (0.35, 0.32);
const RAMP_BACK: (f32, f32) = (-0.6, 1.05);
const BASE_HALF_WIDTH: f32 = 2.1;

const DIGIT_W: f32 = 0.21;
const DIGIT_H: f32 = 0.52;
const SEG_T: f32 = 0.05;
const DIGIT_ADVANCE: f32 = 0.29;
const COLON_ADVANCE: f32 = 0.13;
const DISPLAY_X: f32 = 1.05;

const BUTTON_X: f32 = 1.05;
const BUTTON_Z: f32 = 0.78;
const BUTTON_TOP: f32 = 0.5;
const BUTTON_HALF: Vec3 = Vec3::new(0.42, 0.16, 0.42);

// ---------------------------------------------------------------------------------------
// Lógica pura (testada): texto do tempo, segmentos e acerto do clique.

/// Segundos mostrados: arredonda para cima, então "00:00" só aparece com a bandeira caída.
pub fn display_seconds(ms: u64) -> u64 {
    ms.div_ceil(1000)
}

/// Tempo em texto: `H:MM:SS` a partir de uma hora, senão `MM:SS`.
pub fn clock_text(ms: u64) -> String {
    let total = display_seconds(ms);
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Casas do mostrador: `[H, M, M, S, S]`; a hora fica em branco abaixo de uma hora.
pub fn display_cells(ms: u64) -> [Option<u8>; 5] {
    let total = display_seconds(ms);
    let digit = |v: u64| u8::try_from(v % 10).ok();
    let hours = (total / 3600).min(9);
    let (m, s) = (total / 60 % 60, total % 60);
    [
        (hours > 0).then_some(u8::try_from(hours).unwrap_or(9)),
        digit(m / 10),
        digit(m),
        digit(s / 10),
        digit(s),
    ]
}

/// Segmentos acesos de um dígito, na ordem a, b, c, d, e, f, g.
pub fn segments_for(digit: u8) -> [bool; 7] {
    const A: usize = 0;
    const B: usize = 1;
    const C: usize = 2;
    const D: usize = 3;
    const E: usize = 4;
    const F: usize = 5;
    const G: usize = 6;
    let lit: &[usize] = match digit {
        0 => &[A, B, C, D, E, F],
        1 => &[B, C],
        2 => &[A, B, D, E, G],
        3 => &[A, B, C, D, G],
        4 => &[B, C, F, G],
        5 => &[A, C, D, F, G],
        6 => &[A, C, D, E, F, G],
        7 => &[A, B, C],
        8 => &[A, B, C, D, E, F, G],
        _ => &[A, B, C, D, F, G],
    };
    let mut out = [false; 7];
    for &i in lit {
        out[i] = true;
    }
    out
}

/// O raio acerta a caixa centrada na origem com meias-medidas `half`? (método das placas;
/// o raio já está no espaço local do objeto.)
pub fn ray_hits_box(origin: Vec3, dir: Vec3, half: Vec3) -> bool {
    let mut t_min = 0.0_f32;
    let mut t_max = f32::MAX;
    for axis in 0..3 {
        let (o, d, h) = (origin[axis], dir[axis], half[axis]);
        if d.abs() < 1e-8 {
            if o.abs() > h {
                return false;
            }
            continue;
        }
        let (mut t0, mut t1) = ((-h - o) / d, (h - o) / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        t_min = t_min.max(t0);
        t_max = t_max.min(t1);
        if t_min > t_max {
            return false;
        }
    }
    true
}

/// Lado de cada mostrador: o do usuário fica sempre à esquerda dele (slot 0).
pub fn side_of_slot(user_side: Side, slot: usize) -> Side {
    if slot == 0 {
        user_side
    } else {
        user_side.opposite()
    }
}

/// Transformação do relógio na mesa para o lado do usuário.
fn root_transform(user_side: Side) -> Transform {
    let (position, yaw) = clock_placement(user_side);
    Transform::from_translation(position)
        .with_rotation(Quat::from_rotation_y(yaw))
        .with_scale(Vec3::splat(CLOCK_SCALE))
}

/// Posição do relógio e giro em Y: ao lado do tabuleiro, à direita do usuário e de frente
/// para ele (as pretas veem o mesmo arranjo girado meia-volta).
pub fn clock_placement(user_side: Side) -> (Vec3, f32) {
    match user_side {
        Side::White => (Vec3::new(CLOCK_X, TABLE_TOP, CLOCK_Z), 0.0),
        Side::Black => (
            Vec3::new(-CLOCK_X, TABLE_TOP, -CLOCK_Z),
            std::f32::consts::PI,
        ),
    }
}

// ---------------------------------------------------------------------------------------
// Cena 3D.

#[derive(Component)]
pub(crate) struct ClockRoot;
/// Conteúdo do relógio; escondido quando a Partida não tem controle de tempo.
#[derive(Component)]
struct ClockContent;
#[derive(Component)]
struct Segment {
    slot: usize,
    cell: usize,
    seg: usize,
}
#[derive(Component)]
struct Colon {
    slot: usize,
    which: usize,
}
#[derive(Component)]
struct ClockButton {
    slot: usize,
}
#[derive(Component)]
struct ButtonRing {
    slot: usize,
}
#[derive(Component)]
struct SideTag {
    slot: usize,
}

#[derive(Resource)]
struct ClockMaterials {
    off: Handle<StandardMaterial>,
    on_active: Handle<StandardMaterial>,
    on_idle: Handle<StandardMaterial>,
    on_flag: Handle<StandardMaterial>,
    ivory: Handle<StandardMaterial>,
    charcoal: Handle<StandardMaterial>,
    hover: Handle<StandardMaterial>,
}

impl ClockMaterials {
    fn side(&self, side: Side) -> &Handle<StandardMaterial> {
        match side {
            Side::White => &self.ivory,
            Side::Black => &self.charcoal,
        }
    }
}

/// Estado de interação do relógio 3D.
#[derive(Resource, Default, Debug)]
struct ClockUi {
    /// Slot do botão sob o cursor.
    hover: Option<usize>,
    /// Botão apertado e o tempo desde o aperto, para a animação.
    pressed: Option<(usize, f32)>,
}

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClockUi>()
            .add_systems(Startup, spawn_clock)
            .add_systems(
                Update,
                (clock_input, update_clock)
                    .chain()
                    .after(PlaySet)
                    .after(crate::picking::PickSet),
            );
        crate::dock::register_panel(
            app,
            crate::dock::PanelDef {
                id: crate::dock::ids::CLOCK,
                label_key: "dock-panel-clock",
                access: crate::dock::PanelAccess::Always,
                ui: clock_panel_ui,
            },
        );
    }
}

fn unlit(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn plastic(color: Color, roughness: f32, metallic: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        metallic,
        ..default()
    }
}

/// Prisma triangular (perfil no plano z-y extrudado em x), sombreamento chapado.
fn wedge_mesh(profile: [(f32, f32); 3], half_width: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let centroid = Vec3::new(
        0.0,
        (profile[0].1 + profile[1].1 + profile[2].1) / 3.0,
        (profile[0].0 + profile[1].0 + profile[2].0) / 3.0,
    );
    let point = |i: usize, x: f32| Vec3::new(x, profile[i].1, profile[i].0);
    let mut push_tri = |a: Vec3, b: Vec3, c: Vec3, outward: Vec3| {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        let (b, c) = if n.dot(outward) < 0.0 {
            n = -n;
            (c, b)
        } else {
            (b, c)
        };
        let base = u32::try_from(positions.len()).unwrap_or(0);
        for p in [a, b, c] {
            positions.push(p.to_array());
            normals.push(n.to_array());
        }
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    };
    for sign in [-1.0_f32, 1.0] {
        push_tri(
            point(0, sign * half_width),
            point(1, sign * half_width),
            point(2, sign * half_width),
            Vec3::X * sign,
        );
    }
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        let (a, b) = (point(i, -half_width), point(j, -half_width));
        let (c, d) = (point(j, half_width), point(i, half_width));
        let mid = (a + b + c + d) / 4.0;
        let outward = mid - centroid;
        push_tri(a, b, c, outward);
        push_tri(a, c, d, outward);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Centro de cada casa do mostrador no plano da rampa (x, z) e se é dígito (senão, dois pontos).
fn display_layout() -> [(f32, bool); 7] {
    let total = 5.0 * DIGIT_ADVANCE + 2.0 * COLON_ADVANCE;
    let mut x = -total / 2.0;
    let mut out = [(0.0, true); 7];
    for (i, is_digit) in [true, false, true, true, false, true, true]
        .into_iter()
        .enumerate()
    {
        let advance = if is_digit {
            DIGIT_ADVANCE
        } else {
            COLON_ADVANCE
        };
        out[i] = (x + advance / 2.0, is_digit);
        x += advance;
    }
    out
}

#[allow(clippy::too_many_lines)]
fn spawn_clock(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mats = ClockMaterials {
        off: materials.add(unlit(Color::srgb(0.17, 0.18, 0.20))),
        on_active: materials.add(unlit(Color::srgb(1.0, 0.74, 0.14))),
        on_idle: materials.add(unlit(Color::srgb(0.62, 0.46, 0.12))),
        on_flag: materials.add(unlit(Color::srgb(1.0, 0.18, 0.12))),
        ivory: materials.add(plastic(Color::srgb(0.93, 0.91, 0.85), 0.35, 0.0)),
        charcoal: materials.add(plastic(Color::srgb(0.10, 0.10, 0.12), 0.35, 0.1)),
        hover: materials.add(plastic(Color::srgb(0.98, 0.72, 0.25), 0.3, 0.0)),
    };
    let body = materials.add(plastic(Color::srgb(0.13, 0.135, 0.15), 0.45, 0.35));
    let glass = materials.add(plastic(Color::srgb(0.02, 0.025, 0.03), 0.15, 0.0));
    let ring_mat = materials.add(unlit(Color::srgb(0.25, 0.95, 0.45)));
    let rubber = materials.add(plastic(Color::srgb(0.04, 0.04, 0.05), 0.8, 0.0));

    let base_mesh = meshes.add(Cuboid::new(BASE_HALF_WIDTH * 2.0, RAMP_FRONT.1, 1.75));
    let wedge = meshes.add(wedge_mesh(
        [RAMP_FRONT, RAMP_BACK, (RAMP_BACK.0, RAMP_FRONT.1)],
        BASE_HALF_WIDTH,
    ));
    let h_seg = meshes.add(Cuboid::new(DIGIT_W, 0.012, SEG_T));
    let v_seg = meshes.add(Cuboid::new(SEG_T, 0.012, DIGIT_H / 2.0));
    let dot = meshes.add(Cuboid::new(SEG_T, 0.012, SEG_T));
    let bezel = meshes.add(Cuboid::new(1.9, 0.01, 0.92));
    let tag = meshes.add(Cuboid::new(0.11, 0.02, 0.11));
    let button_mesh = meshes.add(Cylinder::new(0.33, 0.18));
    let skirt_mesh = meshes.add(Cylinder::new(0.4, 0.05));
    let ring_mesh = meshes.add(Annulus::new(0.36, 0.43));

    let (ramp_dz, ramp_dy) = (RAMP_FRONT.0 - RAMP_BACK.0, RAMP_BACK.1 - RAMP_FRONT.1);
    let slope = ramp_dy.atan2(ramp_dz);
    let slope_center = Vec3::new(
        0.0,
        (RAMP_FRONT.1 + RAMP_BACK.1) / 2.0,
        (RAMP_FRONT.0 + RAMP_BACK.0) / 2.0,
    );
    let layout = display_layout();
    commands
        .spawn((
            Name::new("clock"),
            ClockRoot,
            root_transform(Side::White),
            Visibility::default(),
            VisibleIn::BOTH_3D,
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("clock_content"),
                ClockContent,
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|content| {
                // Corpo e rampa também somem sem controle de tempo: sem mostradores era só um
                // calço cinza ao lado do tabuleiro (#135).
                content.spawn((
                    Name::new("clock_body"),
                    Mesh3d(base_mesh),
                    MeshMaterial3d(body.clone()),
                    Transform::from_xyz(0.0, RAMP_FRONT.1 / 2.0, 0.275),
                ));
                content.spawn((
                    Name::new("clock_wedge"),
                    Mesh3d(wedge),
                    MeshMaterial3d(body.clone()),
                    Transform::default(),
                ));
                // Mostradores: rampa inclinada com vidro escuro, etiqueta do lado e dígitos.
                content
                    .spawn((
                        Name::new("clock_displays"),
                        Transform::from_translation(slope_center)
                            .with_rotation(Quat::from_rotation_x(slope)),
                        Visibility::default(),
                    ))
                    .with_children(|ramp| {
                        for slot in 0..2 {
                            let cx = if slot == 0 { -DISPLAY_X } else { DISPLAY_X };
                            ramp.spawn((
                                Mesh3d(bezel.clone()),
                                MeshMaterial3d(glass.clone()),
                                Transform::from_xyz(cx, 0.006, 0.0),
                            ));
                            ramp.spawn((
                                SideTag { slot },
                                Mesh3d(tag.clone()),
                                MeshMaterial3d(mats.ivory.clone()),
                                Transform::from_xyz(cx - 0.85, 0.012, -0.395),
                            ));
                            let mut cell = 0;
                            for (x, is_digit) in layout {
                                let x = cx + x;
                                if is_digit {
                                    spawn_digit(ramp, slot, cell, x, &h_seg, &v_seg, &mats);
                                    cell += 1;
                                } else {
                                    for z in [-0.09, 0.09] {
                                        ramp.spawn((
                                            Colon {
                                                slot,
                                                which: cell.min(2) / 2,
                                            },
                                            Mesh3d(dot.clone()),
                                            MeshMaterial3d(mats.off.clone()),
                                            Transform::from_xyz(x, 0.012, z),
                                        ));
                                    }
                                }
                            }
                        }
                    });
                // Botões no degrau da frente.
                for slot in 0..2 {
                    let x = if slot == 0 { -BUTTON_X } else { BUTTON_X };
                    content.spawn((
                        Mesh3d(skirt_mesh.clone()),
                        MeshMaterial3d(rubber.clone()),
                        Transform::from_xyz(x, RAMP_FRONT.1 + 0.025, BUTTON_Z),
                    ));
                    content
                        .spawn((
                            Name::new(format!("clock_button_{slot}")),
                            ClockButton { slot },
                            Mesh3d(button_mesh.clone()),
                            MeshMaterial3d(mats.ivory.clone()),
                            Transform::from_xyz(x, BUTTON_TOP - 0.09, BUTTON_Z),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                ButtonRing { slot },
                                Mesh3d(ring_mesh.clone()),
                                MeshMaterial3d(ring_mat.clone()),
                                Transform::from_xyz(0.0, 0.095, 0.0).with_rotation(
                                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                                ),
                                Visibility::Hidden,
                            ));
                        });
                }
            });
        });
    commands.insert_resource(mats);
}

fn spawn_digit(
    ramp: &mut ChildSpawnerCommands<'_>,
    slot: usize,
    cell: usize,
    x: f32,
    h_seg: &Handle<Mesh>,
    v_seg: &Handle<Mesh>,
    mats: &ClockMaterials,
) {
    let (hw, hz) = (DIGIT_W / 2.0, DIGIT_H / 2.0);
    // a, b, c, d, e, f, g
    let parts = [
        (h_seg, 0.0, -hz),
        (v_seg, hw, -hz / 2.0),
        (v_seg, hw, hz / 2.0),
        (h_seg, 0.0, hz),
        (v_seg, -hw, hz / 2.0),
        (v_seg, -hw, -hz / 2.0),
        (h_seg, 0.0, 0.0),
    ];
    for (seg, (mesh, dx, dz)) in parts.into_iter().enumerate() {
        ramp.spawn((
            Segment { slot, cell, seg },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mats.off.clone()),
            Transform::from_xyz(x + dx, 0.012, dz),
        ));
    }
}

/// Raio (origem, direção) do cursor no mundo, para a câmera 3D principal.
fn cursor_ray(window: &Window, camera: (&Camera, &Transform)) -> Option<(Vec3, Vec3)> {
    let cursor = window.cursor_position()?;
    let ray = camera
        .0
        .viewport_to_world(&GlobalTransform::from(*camera.1), cursor)
        .ok()?;
    Some((ray.origin, *ray.direction))
}

/// Clique no botão 3D (lado da vez) e Espaço. Ambos são o mesmo Comando.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clock_input(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<crate::controls::Controls>,
    egui: Res<EguiWantsInput>,
    presentation: Res<Presentation>,
    table: Res<crate::table::TableClock>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Single<(&Camera, &Transform), With<crate::camera::MainCamera>>,
    button_query: Query<(&ClockButton, &GlobalTransform)>,
    mut ui: ResMut<ClockUi>,
    mut session: ResMut<Session>,
    mut requests: MessageReader<crate::actions::ActionRequested>,
) {
    session.tick();
    // O atalho chega como `ActionRequested` (`actions`), já filtrado pelo foco do egui.
    if requests
        .read()
        .any(|r| r.0 == chess_input::controls::Action::ClockPress)
    {
        let slot = usize::from(session.snapshot().side_to_move != session.user_side());
        session.press_clock();
        ui.pressed = Some((slot, 0.0));
    }
    if let Some((_, t)) = ui.pressed.as_mut() {
        *t += time.delta_secs();
        if *t > 0.25 {
            ui.pressed = None;
        }
    }

    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();
    let mut hover = None;
    if presentation.is_3d()
        && table.0
        && !over_ui
        && session.snapshot().clock.is_some()
        && let Some((origin, dir)) = cursor_ray(&window, *camera)
    {
        for (button, global) in &button_query {
            let to_local = global.affine().inverse();
            if ray_hits_box(
                to_local.transform_point3(origin),
                to_local.transform_vector3(dir),
                BUTTON_HALF,
            ) {
                hover = Some(button.slot);
            }
        }
    }
    if ui.hover != hover {
        ui.hover = hover;
    }
    if controls.select_just_pressed(&buttons, &keys)
        && let Some(slot) = hover
        && session.can_press_clock(side_of_slot(session.user_side(), slot))
    {
        session.press_clock();
        ui.pressed = Some((slot, 0.0));
    }
}

/// Mostradores, botões, anel de "sua vez" e posição do relógio.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_clock(
    session: Res<Session>,
    ui: Res<ClockUi>,
    table: Res<crate::table::TableClock>,
    mats: Res<ClockMaterials>,
    mut root: Single<&mut Transform, (With<ClockRoot>, Without<ClockButton>)>,
    mut content: Single<&mut Visibility, With<ClockContent>>,
    mut segments: Query<(&Segment, &mut MeshMaterial3d<StandardMaterial>), Without<ClockButton>>,
    mut colons: Query<
        (&Colon, &mut MeshMaterial3d<StandardMaterial>),
        (Without<Segment>, Without<ClockButton>, Without<SideTag>),
    >,
    mut tags: Query<
        (&SideTag, &mut MeshMaterial3d<StandardMaterial>),
        (Without<Segment>, Without<ClockButton>, Without<Colon>),
    >,
    mut buttons: Query<
        (
            &ClockButton,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Transform,
        ),
        (
            Without<Segment>,
            Without<Colon>,
            Without<SideTag>,
            Without<ClockRoot>,
        ),
    >,
    mut rings: Query<(&ButtonRing, &mut Visibility), Without<ClockContent>>,
) {
    let user = session.user_side();
    let want = root_transform(user);
    if **root != want {
        **root = want;
    }
    let reading = session.clock_reading();
    // Escondido pela aba Tabuleiro: nada do relógio aparece (o enquadramento já o ignora).
    let shown = if reading.is_some() && table.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **content != shown {
        **content = shown;
    }
    let Some(reading) = reading else { return };

    let remaining = |slot: usize| reading.remaining(side_of_slot(user, slot));
    let material_for = |slot: usize, lit: bool| {
        let side = side_of_slot(user, slot);
        if !lit {
            return &mats.off;
        }
        if remaining(slot) == 0 || reading.flag.is_some_and(|f| f.side == side) {
            &mats.on_flag
        } else if reading.running == Some(side) && !reading.paused {
            &mats.on_active
        } else {
            &mats.on_idle
        }
    };
    for (segment, mut material) in &mut segments {
        let cells = display_cells(remaining(segment.slot));
        let lit = cells[segment.cell].is_some_and(|d| segments_for(d)[segment.seg]);
        let want = material_for(segment.slot, lit);
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
    for (colon, mut material) in &mut colons {
        let cells = display_cells(remaining(colon.slot));
        // O primeiro par de pontos só aparece com a hora; o segundo, sempre.
        let lit = colon.which == 1 || cells[0].is_some();
        let want = material_for(colon.slot, lit);
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
    for (tag, mut material) in &mut tags {
        let want = mats.side(side_of_slot(user, tag.slot));
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
    for (button, mut material, mut transform) in &mut buttons {
        let side = side_of_slot(user, button.slot);
        let clickable = session.can_press_clock(side);
        let want = if clickable && ui.hover == Some(button.slot) {
            &mats.hover
        } else {
            mats.side(side)
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
        let depth = match ui.pressed {
            Some((slot, t)) if slot == button.slot => {
                0.09 * (1.0 - (t / 0.25 - 0.5).abs() * 2.0).max(0.0)
            }
            _ => 0.0,
        };
        let y = BUTTON_TOP - 0.09 - depth;
        if (transform.translation.y - y).abs() > 1e-4 {
            transform.translation.y = y;
        }
    }
    for (ring, mut visibility) in &mut rings {
        let on = session.can_press_clock(side_of_slot(user, ring.slot));
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Painel Relógio do dock (egui): o widget que era do 2D técnico, agora em todas as
// Apresentações. O relógio 3D continua na mesa; ambos acionam o mesmo Comando.

/// Texto do controle de tempo: `15 min + 10 s` (ou só a base, sem incremento).
pub fn control_text(base_ms: u64, increment_ms: u64) -> String {
    let base = if base_ms.is_multiple_of(60_000) {
        format!("{} min", base_ms / 60_000)
    } else {
        format!("{} s", base_ms / 1_000)
    };
    if increment_ms == 0 {
        base
    } else {
        format!("{base} + {} s", increment_ms / 1_000)
    }
}

/// Tempo restante em que o relógio passa a avisar (ChessClock: tempo baixo em `warning`).
pub const LOW_TIME_MS: u64 = 30_000;

/// Como o valor do relógio é pintado. O valor fica sempre visível: a cor só reforça.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockTone {
    Normal,
    /// Tempo baixo (30 s ou menos): `warning`.
    Low,
    /// Bandeira caída: `danger`.
    Flag,
}

pub fn clock_tone(ms: u64) -> ClockTone {
    match ms {
        0 => ClockTone::Flag,
        1..=LOW_TIME_MS => ClockTone::Low,
        _ => ClockTone::Normal,
    }
}

/// Um lado do relógio (ChessClock): nome e tempo em 32px com números tabulares (fonte
/// monoespaçada). O lado que joga ganha o contorno `accent` e um traço à esquerda.
fn clock_card(
    ui: &mut egui::Ui,
    p: &crate::ui_theme::Palette,
    name: &str,
    time: &str,
    running: bool,
    tone: ClockTone,
) {
    let stroke = if running {
        egui::Stroke::new(2.0, p.accent)
    } else {
        egui::Stroke::new(1.0, p.border)
    };
    let digits = match tone {
        ClockTone::Normal if running => p.ink,
        ClockTone::Normal => p.ink_muted,
        ClockTone::Low => p.warning,
        ClockTone::Flag => p.danger,
    };
    let corner = crate::panel_chrome::radius("radius-m");
    let response = egui::Frame::NONE
        .fill(p.surface_2)
        .stroke(stroke)
        .corner_radius(corner)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let label = egui::RichText::new(name).size(14.0).color(if running {
                    p.ink
                } else {
                    p.ink_muted
                });
                ui.label(label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(time)
                            .font(egui::FontId::new(32.0, egui::FontFamily::Monospace))
                            .color(digits),
                    );
                });
            });
        })
        .response;
    if running {
        let rect = response.rect;
        let strip = egui::Rect::from_min_size(rect.min, egui::vec2(4.0, rect.height()));
        ui.painter().rect_filled(
            strip,
            egui::CornerRadius {
                nw: corner.nw,
                sw: corner.sw,
                ne: 0,
                se: 0,
            },
            p.accent,
        );
    }
}

fn clock_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let Some(mut session) = world.get_resource_mut::<Session>() else {
        return;
    };
    let Some(reading) = session.clock_reading() else {
        ui.weak(i18n.t("clock-none"));
        return;
    };
    let user = session.user_side();
    let snapshot = session.snapshot().clone();
    let mut press = false;
    let p = crate::panel_chrome::palette_of(ui.visuals());
    for slot in 0..2 {
        let side = side_of_slot(user, slot);
        let ms = reading.remaining(side);
        let running = reading.running == Some(side) && !reading.paused;
        let name = i18n.t(match side {
            Side::White => "clock-white",
            Side::Black => "clock-black",
        });
        clock_card(ui, p, &name, &clock_text(ms), running, clock_tone(ms));
        ui.add_space(crate::panel_chrome::token("space-2", 8.0));
    }
    if let Some(time) = session.config().time {
        ui.small(i18n.t_with(
            "clock-control",
            &[("control", &control_text(time.base_ms, time.increment_ms))],
        ));
    }
    if snapshot.outcome.is_none() {
        ui.label(i18n.t(match snapshot.side_to_move {
            Side::White => "game-turn-white",
            Side::Black => "game-turn-black",
        }));
    }
    if reading.paused {
        ui.colored_label(p.warning, i18n.t("clock-paused"));
    }
    if session.is_competitive() {
        ui.small(i18n.t_with(
            "clock-penalties",
            &[
                ("white", &snapshot.infractions_white.to_string()),
                ("black", &snapshot.infractions_black.to_string()),
            ],
        ));
    }
    let enabled = session.can_press_clock(snapshot.side_to_move);
    if ui
        .add(
            crate::ui_theme::CsButton::new(&i18n.t("clock-press"))
                .variant(crate::ui_theme::ButtonVariant::Primary)
                .enabled(enabled),
        )
        .clicked()
    {
        press = true;
    }
    if press {
        session.press_clock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rounds_up_and_drops_the_hour_below_one_hour() {
        assert_eq!(clock_text(90 * 60_000), "1:30:00");
        assert_eq!(clock_text(59 * 60_000 + 59_001), "1:00:00");
        assert_eq!(clock_text(5 * 60_000), "05:00");
        assert_eq!(clock_text(61_000), "01:01");
        assert_eq!(clock_text(1), "00:01", "1 ms ainda é o último segundo");
        assert_eq!(clock_text(0), "00:00");
        assert_eq!(clock_tone(5 * 60_000), ClockTone::Normal);
        assert_eq!(clock_tone(LOW_TIME_MS + 1), ClockTone::Normal);
        assert_eq!(clock_tone(LOW_TIME_MS), ClockTone::Low);
        assert_eq!(clock_tone(1), ClockTone::Low);
        assert_eq!(clock_tone(0), ClockTone::Flag);
    }

    #[test]
    fn cells_hide_the_hour_below_one_hour() {
        assert_eq!(
            display_cells(90 * 60_000),
            [Some(1), Some(3), Some(0), Some(0), Some(0)]
        );
        assert_eq!(
            display_cells(9 * 60_000 + 7_000),
            [None, Some(0), Some(9), Some(0), Some(7)]
        );
    }

    #[test]
    fn seven_segment_patterns_are_the_standard_ones() {
        let count = |d| segments_for(d).iter().filter(|&&on| on).count();
        let expected = [6, 2, 5, 5, 4, 5, 6, 3, 7, 6];
        for (digit, want) in expected.into_iter().enumerate() {
            assert_eq!(
                count(u8::try_from(digit).unwrap_or(0)),
                want,
                "dígito {digit}"
            );
        }
        // Só o 8 acende tudo; 1 e 7 não acendem o segmento de baixo.
        assert!(segments_for(8).iter().all(|&on| on));
        assert!(!segments_for(1)[3] && !segments_for(7)[3]);
    }

    #[test]
    fn ray_hits_only_when_it_crosses_the_box() {
        let half = Vec3::new(0.5, 0.2, 0.5);
        assert!(ray_hits_box(Vec3::new(0.0, 3.0, 0.0), Vec3::NEG_Y, half));
        assert!(ray_hits_box(
            Vec3::new(2.0, 3.0, 2.0),
            Vec3::new(-2.0, -3.0, -2.0).normalize(),
            half
        ));
        assert!(!ray_hits_box(Vec3::new(1.0, 3.0, 0.0), Vec3::NEG_Y, half));
        assert!(
            !ray_hits_box(Vec3::new(0.0, 3.0, 0.0), Vec3::Y, half),
            "atrás da origem"
        );
    }

    #[test]
    fn the_user_display_is_always_slot_zero() {
        assert_eq!(side_of_slot(Side::White, 0), Side::White);
        assert_eq!(side_of_slot(Side::White, 1), Side::Black);
        assert_eq!(side_of_slot(Side::Black, 0), Side::Black);
        assert_eq!(side_of_slot(Side::Black, 1), Side::White);
    }

    #[test]
    fn the_clock_sits_on_the_users_right_for_both_sides() {
        let (white, white_yaw) = clock_placement(Side::White);
        let (black, _) = clock_placement(Side::Black);
        assert!(white.x > 0.0 && white_yaw == 0.0);
        assert!(black.x < 0.0, "as pretas veem o mesmo arranjo girado");
        assert_eq!((white.x, white.z), (-black.x, -black.z));
    }

    #[test]
    fn display_layout_has_five_digits_and_two_colons() {
        let layout = display_layout();
        assert_eq!(layout.iter().filter(|(_, d)| *d).count(), 5);
        assert!(
            layout.windows(2).all(|w| w[0].0 < w[1].0),
            "da esquerda para a direita"
        );
    }

    #[test]
    fn clicking_the_button_in_world_space_hits_for_both_user_sides() {
        for user in [Side::White, Side::Black] {
            let root = GlobalTransform::from(root_transform(user));
            for slot in 0..2 {
                let x = if slot == 0 { -BUTTON_X } else { BUTTON_X };
                let local = Transform::from_xyz(x, BUTTON_TOP - 0.09, BUTTON_Z);
                let button = root.mul_transform(local);
                let to_local = button.affine().inverse();
                let target = button.translation();
                let hit = |aim: Vec3| {
                    let origin = aim + Vec3::new(0.0, 6.0, 4.0);
                    ray_hits_box(
                        to_local.transform_point3(origin),
                        to_local.transform_vector3((aim - origin).normalize()),
                        BUTTON_HALF,
                    )
                };
                assert!(hit(target), "{user:?} botão {slot}");
                assert!(
                    !hit(target + Vec3::new(1.2, 0.0, 0.0)),
                    "{user:?} fora do botão"
                );
            }
        }
    }
}
