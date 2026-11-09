//! Cursor de mão e realce ao passar sobre a peça (C15, #143).
//!
//! Afordância de interface, não Auxílio visual: diz "esta peça é sua e pode ser pegada
//! agora", nunca para onde ela vai (nada de destinos nem de legalidade), então o Perfil
//! "Jogo físico" não a desliga. O usuário desliga o realce na preferência "Realce ao passar
//! o mouse"; o cursor de mão segue valendo. A decisão é função pura ([`decide`]); os
//! sistemas Bevy só leem a Partida e o picking (corpo da peça no 3D, #138) e desenham
//! (ADR-0001: nada aqui escreve a Partida).

use crate::camera::CameraSet;
use crate::controls::Controls;
use crate::picking::{HoveredSquare, PickSet};
use crate::pieces::{BoardOverride, visible_pieces};
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};
use bevy_egui::input::EguiWantsInput;
use chess_core::{Color as Side, Square};

/// Preferência "Realce ao passar o mouse" (ligada por padrão).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoverHighlight(pub bool);

impl Default for HoverHighlight {
    fn default() -> Self {
        Self(true)
    }
}

/// Casa da peça que recebe o realce agora (`None` se desligado ou sem peça movível).
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct HoverTarget(pub Option<Square>);

/// Cursor do sistema que o tabuleiro pede.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Default,
    /// Mão: há uma peça sua para pegar sob o ponteiro.
    Pointer,
    /// Mão fechada: peça em arrasto.
    Grabbing,
}

impl CursorKind {
    fn icon(self) -> SystemCursorIcon {
        match self {
            CursorKind::Default => SystemCursorIcon::Default,
            CursorKind::Pointer => SystemCursorIcon::Pointer,
            CursorKind::Grabbing => SystemCursorIcon::Grabbing,
        }
    }
}

/// Fatos do quadro que a decisão consulta.
#[derive(Clone, Copy, Debug)]
pub struct HoverFacts<'a> {
    /// O ponteiro está sobre um painel/janela do egui (ou capturando um atalho).
    pub over_ui: bool,
    /// Editor de posição ativo: o tabuleiro não joga.
    pub editor_active: bool,
    /// Revisão ou variante em exibição: somente leitura.
    pub reviewing: bool,
    pub game_over: bool,
    pub paused: bool,
    /// A Engine joga neste momento.
    pub engine_turn: bool,
    pub side_to_move: Side,
    /// Obrigação de peça tocada: só estas casas podem sair (vazio = sem obrigação).
    pub must_move_from: &'a [Square],
    /// Peça sob o ponteiro: casa e dono.
    pub hovered: Option<(Square, Side)>,
    /// Uma peça está sendo arrastada.
    pub dragging: bool,
    /// Preferência "Realce ao passar o mouse".
    pub highlight_enabled: bool,
}

/// Resultado: cursor e peça a realçar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoverDecision {
    pub cursor: CursorKind,
    pub highlight: Option<Square>,
}

/// Decide cursor e realce. Só a peça que o usuário pode mover agora conta; peças do
/// adversário ficam sem resposta, para não revelar legalidade.
pub fn decide(f: &HoverFacts) -> HoverDecision {
    let idle = HoverDecision {
        cursor: CursorKind::Default,
        highlight: None,
    };
    if f.over_ui || f.editor_active || f.reviewing || f.game_over || f.paused || f.engine_turn {
        return idle;
    }
    if f.dragging {
        return HoverDecision {
            cursor: CursorKind::Grabbing,
            highlight: None,
        };
    }
    let Some((square, owner)) = f.hovered else {
        return idle;
    };
    let movable = owner == f.side_to_move
        && (f.must_move_from.is_empty() || f.must_move_from.contains(&square));
    if !movable {
        return idle;
    }
    HoverDecision {
        cursor: CursorKind::Pointer,
        highlight: f.highlight_enabled.then_some(square),
    }
}

/// Elevação da peça realçada (mundo 3D; no 2D vira escala via `rest_transform_2d`).
pub const HOVER_LIFT: f32 = 0.08;

#[derive(Component)]
struct HoverRing3d;
#[derive(Component)]
struct HoverRing2d;

pub struct HoverPlugin;

impl Plugin for HoverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoverHighlight>()
            .init_resource::<HoverTarget>()
            .add_systems(Startup, spawn_rings)
            .add_systems(
                Update,
                (update_hover, update_rings)
                    .chain()
                    .after(PickSet)
                    .after(CameraSet)
                    .before(PlaySet),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_hover(
    session: Res<Session>,
    editor: Res<BoardOverride>,
    egui: Res<EguiWantsInput>,
    controls: Res<Controls>,
    hovered: Res<HoveredSquare>,
    enabled: Res<HoverHighlight>,
    mut target: ResMut<HoverTarget>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut commands: Commands,
    window: Single<(Entity, Option<&CursorIcon>), With<PrimaryWindow>>,
) {
    // Modo de evidência: sem cursor real, o roteiro fixa a casa realçada.
    if let Some(sq) = capture.as_ref().and_then(|c| c.forced_hover()) {
        if enabled.0 && target.0 != Some(sq) {
            target.0 = Some(sq);
        }
        return;
    }
    let snapshot = session.snapshot();
    let hovered_piece = hovered.0.and_then(|sq| {
        visible_pieces(&session, &editor)
            .get(&sq)
            .map(|(c, _)| (sq, *c))
    });
    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();
    let facts = HoverFacts {
        over_ui: over_ui || controls.is_capturing(),
        editor_active: editor.is_active(),
        reviewing: session.review().is_some(),
        game_over: snapshot.outcome.is_some(),
        paused: snapshot.paused,
        engine_turn: session.engine_turn(),
        side_to_move: snapshot.side_to_move,
        must_move_from: &snapshot.must_move_from,
        hovered: hovered_piece,
        dragging: session.gesture_visual().drag_world.is_some(),
        highlight_enabled: enabled.0,
    };
    let decision = decide(&facts);
    if target.0 != decision.highlight {
        target.0 = decision.highlight;
    }
    // Sobre um painel o cursor é do egui; não brigamos com ele.
    if over_ui {
        return;
    }
    let (entity, current) = *window;
    let wanted = CursorIcon::System(decision.cursor.icon());
    if current != Some(&wanted) {
        commands.entity(entity).insert(wanted);
    }
}

fn spawn_rings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        HoverRing3d,
        Mesh3d(meshes.add(Cylinder::new(0.43, 0.012))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.92, 0.55, 0.55),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        HoverRing2d,
        Sprite::from_color(
            Color::srgba(1.0, 0.92, 0.55, 0.38),
            Vec2::splat(crate::board2d::UNIT * 0.9),
        ),
        Transform::from_xyz(0.0, 0.0, 0.45),
        Visibility::Hidden,
        crate::presentation::VisibleIn::TECHNICAL,
    ));
}

#[allow(clippy::type_complexity)]
fn update_rings(
    target: Res<HoverTarget>,
    mut rings: ParamSet<(
        Single<(&mut Transform, &mut Visibility), With<HoverRing3d>>,
        Single<(&mut Transform, &mut Visibility), With<HoverRing2d>>,
    )>,
) {
    let (mut t, mut v) = rings.p0().into_inner();
    match target.0 {
        Some(sq) => {
            let c = chess_input::square_center(sq);
            t.translation = Vec3::new(c.x, 0.013, c.z);
            *v = Visibility::Visible;
        }
        None => *v = Visibility::Hidden,
    }
    let (mut t, mut v) = rings.p1().into_inner();
    match target.0 {
        Some(sq) => {
            let p = crate::board2d::square_pos(sq);
            t.translation = Vec3::new(p.x, p.y, 0.45);
            *v = Visibility::Visible;
        }
        None => *v = Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn base() -> HoverFacts<'static> {
        HoverFacts {
            over_ui: false,
            editor_active: false,
            reviewing: false,
            game_over: false,
            paused: false,
            engine_turn: false,
            side_to_move: Side::White,
            must_move_from: &[],
            hovered: Some((sq("e2"), Side::White)),
            dragging: false,
            highlight_enabled: true,
        }
    }

    #[test]
    fn own_piece_on_turn_shows_hand_and_highlight() {
        let d = decide(&base());
        assert_eq!(d.cursor, CursorKind::Pointer);
        assert_eq!(d.highlight, Some(sq("e2")));
    }

    #[test]
    fn opponent_piece_empty_square_and_no_hover_stay_default() {
        let mut f = base();
        f.hovered = Some((sq("e7"), Side::Black));
        assert_eq!(decide(&f).cursor, CursorKind::Default);
        assert_eq!(decide(&f).highlight, None);
        f.hovered = None;
        assert_eq!(decide(&f).cursor, CursorKind::Default);
    }

    #[test]
    fn no_response_when_the_board_is_not_playable() {
        let tweaks: [fn(&mut HoverFacts); 6] = [
            |f| f.over_ui = true,
            |f| f.editor_active = true,
            |f| f.reviewing = true,
            |f| f.game_over = true,
            |f| f.paused = true,
            |f| f.engine_turn = true,
        ];
        for tweak in tweaks {
            let mut f = base();
            tweak(&mut f);
            assert_eq!(
                decide(&f),
                HoverDecision {
                    cursor: CursorKind::Default,
                    highlight: None
                }
            );
        }
    }

    #[test]
    fn dragging_shows_grabbing_without_hover_highlight() {
        let mut f = base();
        f.dragging = true;
        f.hovered = Some((sq("e4"), Side::White));
        let d = decide(&f);
        assert_eq!(d.cursor, CursorKind::Grabbing);
        assert_eq!(d.highlight, None);
    }

    #[test]
    fn disabling_the_preference_keeps_the_hand_but_drops_the_highlight() {
        let mut f = base();
        f.highlight_enabled = false;
        let d = decide(&f);
        assert_eq!(d.cursor, CursorKind::Pointer);
        assert_eq!(d.highlight, None);
    }

    #[test]
    fn touched_piece_obligation_limits_which_piece_answers() {
        let mut f = base();
        let only = [sq("g1")];
        f.must_move_from = &only;
        assert_eq!(decide(&f).cursor, CursorKind::Default);
        f.hovered = Some((sq("g1"), Side::White));
        assert_eq!(decide(&f).cursor, CursorKind::Pointer);
    }
}
