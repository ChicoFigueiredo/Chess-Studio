//! Opções da mesa 3D (aba contextual Tabuleiro, #194): Relógio e Peças tomadas. São
//! preferências do Perfil local (`persistence`), lidas e escritas só pelas Ações; nada aqui
//! toca a Partida (ADR-0001). As Peças tomadas são derivadas do `Snapshot`, reaproveitam as
//! malhas e os materiais dos moldes das peças e só são recriadas quando a contagem muda
//! (captura, Desfazer, revisão) ou a opção alterna, nunca por quadro.

use crate::board::PieceWood;
use crate::captured::{Counts, KINDS, captured};
use crate::pieces::{PieceTemplates, spawn_display_piece};
use crate::presentation::VisibleIn;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::{CanonicalPosition, Color as Side, Piece};

/// Relógio 3D sobre a mesa (ligado por padrão: o Jogo físico o mostra).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableClock(pub bool);

impl Default for TableClock {
    fn default() -> Self {
        Self(true)
    }
}

/// Peças tomadas em 3D ao lado do tabuleiro (ligadas por padrão, como no Jogo físico).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCaptured(pub bool);

impl Default for TableCaptured {
    fn default() -> Self {
        Self(true)
    }
}

/// Escala das peças tomadas em relação às do tabuleiro.
const SCALE: f32 = 0.5;
/// Distância entre peças de uma fileira (ao longo do lado direito do jogador).
const STEP: f32 = 0.42;
/// Afastamento do centro do tabuleiro até a primeira peça, rumo à direita do jogador: fora
/// da moldura (4,6) e do relógio (z diferente).
const RIGHT: f32 = 5.3;
/// Profundidade (rumo ao jogador) da fileira dos peões; a das peças fica uma linha atrás.
const DEPTH: f32 = 3.9;
const ROW_GAP: f32 = 0.6;
/// Altura do tampo da mesa.
const TABLE_TOP: f32 = -0.42;

/// Uma peça tomada e onde ela fica, em coordenadas do mundo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Tipo da peça tomada (a cor é a do adversário de `player`).
    pub piece: Piece,
    pub pos: Vec3,
}

/// Posições das peças que `player` tomou (`lost` = o que o adversário perdeu, por `KINDS`).
/// Ficam à **direita do jogador** (as brancas em +x, as pretas em -x, de frente para o
/// tabuleiro), numa fileira de peões e outra com as demais, da maior para a menor.
pub fn layout(player: Side, lost: &[u8; 5]) -> Vec<Slot> {
    let sign = if player == Side::White { 1.0 } else { -1.0 };
    let mut slots = Vec::new();
    let mut put = |row: usize, col: usize, piece: Piece| {
        let u = RIGHT + col as f32 * STEP;
        let v = DEPTH - row as f32 * ROW_GAP;
        slots.push(Slot {
            piece,
            pos: Vec3::new(sign * u, TABLE_TOP, sign * v),
        });
    };
    for col in 0..usize::from(lost[0]) {
        put(0, col, Piece::Pawn);
    }
    let mut col = 0;
    for (i, kind) in KINDS.iter().enumerate().skip(1).rev() {
        for _ in 0..lost[i] {
            put(1, col, *kind);
            col += 1;
        }
    }
    slots
}

/// Raiz das peças tomadas; as peças são filhas e recriadas só quando algo muda.
#[derive(Component)]
struct CapturedRoot;

#[derive(Resource, Default)]
struct Shown {
    key: Option<(u64, Option<usize>, bool, bool)>,
    counts: Option<(Counts, bool)>,
}

pub struct TablePlugin;

impl Plugin for TablePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TableClock>()
            .init_resource::<TableCaptured>()
            .init_resource::<Shown>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((
                    Name::new("captured_pieces"),
                    CapturedRoot,
                    Transform::default(),
                    Visibility::default(),
                    VisibleIn::BOTH_3D,
                ));
            })
            .add_systems(Update, sync_captured.after(crate::play::PlaySet));
    }
}

/// O que cada lado tomou na Posição mostrada (a da revisão, quando há).
fn shown_counts(session: &Session) -> Counts {
    let (now, plies) = match session.review() {
        Some(r) => (r.position.clone(), r.ply),
        None => (
            session.snapshot().position.clone(),
            session.snapshot().history.len(),
        ),
    };
    let start = session
        .game()
        .review_at_ply(0)
        .map_or_else(CanonicalPosition::start, |v| v.position);
    let history = &session.snapshot().history[..plies.min(session.snapshot().history.len())];
    captured(&start, &now, history)
}

fn sync_captured(
    enabled: Res<TableCaptured>,
    session: Res<Session>,
    templates: Res<PieceTemplates>,
    wood: Option<Res<PieceWood>>,
    mut shown: ResMut<Shown>,
    root: Single<(Entity, Option<&Children>), With<CapturedRoot>>,
    mut commands: Commands,
) {
    let ready = templates.is_complete();
    let key = (
        session.revision(),
        session.review().map(|r| r.ply),
        enabled.0,
        ready,
    );
    if shown.key == Some(key) {
        return;
    }
    shown.key = Some(key);
    let on = enabled.0 && ready;
    let counts = if on {
        shown_counts(&session)
    } else {
        Counts::default()
    };
    if shown.counts == Some((counts, on)) {
        return;
    }
    shown.counts = Some((counts, on));
    let (entity, children) = *root;
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    if !on {
        return;
    }
    commands.entity(entity).with_children(|parent| {
        for player in [Side::White, Side::Black] {
            // O que `player` tomou são as peças do adversário que sumiram do tabuleiro.
            let lost = counts.of(player.opposite());
            for slot in layout(player, lost) {
                spawn_display_piece(
                    parent,
                    &templates,
                    wood.as_deref(),
                    player.opposite(),
                    slot.piece,
                    slot.pos,
                    SCALE,
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_captured_means_no_slots() {
        assert!(layout(Side::White, &[0; 5]).is_empty());
    }

    #[test]
    fn slot_count_matches_the_captured_count() {
        let lost = [8, 2, 2, 2, 1];
        assert_eq!(layout(Side::White, &lost).len(), 15);
        assert_eq!(layout(Side::Black, &[3, 0, 1, 0, 0]).len(), 4);
    }

    #[test]
    fn each_player_keeps_the_pieces_on_their_own_right_hand() {
        let lost = [2, 1, 0, 0, 1];
        // Brancas olham para -z: a direita é +x, do lado delas (z > 0).
        for slot in layout(Side::White, &lost) {
            assert!(slot.pos.x > 4.6 && slot.pos.z > 0.0, "{slot:?}");
        }
        // Pretas, de frente para elas: direita em -x, do lado delas (z < 0).
        for slot in layout(Side::Black, &lost) {
            assert!(slot.pos.x < -4.6 && slot.pos.z < 0.0, "{slot:?}");
        }
    }

    #[test]
    fn black_is_the_white_layout_turned_half_a_turn() {
        let lost = [3, 1, 1, 0, 1];
        let white = layout(Side::White, &lost);
        let black = layout(Side::Black, &lost);
        for (w, b) in white.iter().zip(&black) {
            assert_eq!(w.piece, b.piece);
            assert_eq!(Vec3::new(-w.pos.x, w.pos.y, -w.pos.z), b.pos);
        }
    }

    #[test]
    fn pawns_fill_the_first_row_and_the_rest_the_second_by_value() {
        let slots = layout(Side::White, &[2, 1, 0, 1, 1]);
        let kinds: Vec<_> = slots.iter().map(|s| s.piece).collect();
        assert_eq!(
            kinds,
            [
                Piece::Pawn,
                Piece::Pawn,
                Piece::Queen,
                Piece::Rook,
                Piece::Knight
            ]
        );
        assert_eq!(slots[0].pos.z, slots[1].pos.z);
        assert!(
            slots[2].pos.z < slots[0].pos.z,
            "a segunda fileira fica atrás"
        );
        assert!(
            slots[1].pos.x > slots[0].pos.x,
            "avança para fora do tabuleiro"
        );
    }

    #[test]
    fn pieces_stay_clear_of_the_board_frame_and_the_clock() {
        // Relógio em z = 0,6 ± ~0,9 (clock.rs); as fileiras ficam além disso e fora da moldura.
        for slot in layout(Side::White, &[8, 2, 2, 2, 1]) {
            assert!(slot.pos.x > 4.6);
            assert!(slot.pos.z > 2.0);
        }
    }

    #[test]
    fn both_options_default_to_on_like_the_physical_game() {
        assert!(TableClock::default().0);
        assert!(TableCaptured::default().0);
    }
}
