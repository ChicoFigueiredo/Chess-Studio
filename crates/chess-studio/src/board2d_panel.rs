//! Painel "Tabuleiro 2D" do dock (C17): a posição que o tabuleiro principal mostra, em 2D
//! técnico, ao lado dele.
//!
//! # Render
//!
//! Tudo é desenhado com o `painter` do egui (quadrados, glifos Unicode de xadrez e
//! marcadores). **Não há câmera nova nem textura offscreen**: a regra de câmeras (#82/#142,
//! mesmo `Msaa` e mesmo `Hdr`) não é tocada e o custo é o de ~100 formas de egui por quadro.
//!
//! # Autoridade (ADR-0001)
//!
//! O painel só lê. A posição vem de [`visible_pieces`], a mesma função do tabuleiro
//! principal (posição ao vivo, revisão, variante da Análise/Aberturas, posição provisória), e
//! as marcações vêm de [`Marks`], já filtradas pelos Auxílios do Perfil e pela Política.
//! Com "Jogar por aqui" ligado, o painel traduz o ponteiro em `GestureEvent` e os entrega a
//! `Session::gesture`, o mesmo caminho de `play`: Política, Método de movimento, Engine,
//! revisão e peça tocada são decididos pelo intérprete e pela Partida, nunca aqui.
//!
//! Nada do painel mostra notação (a Súmula não é revelada): só casas, peças e marcações.

use crate::aids::Marks;
use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::pieces::{BoardOverride, glyph_colors, visible_pieces};
use crate::play::DRAG_THRESHOLD_PX;
use crate::session::Session;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2 as EVec2};
use chess_core::{Color as Side, Piece, Square};
use chess_input::{GestureEvent, Modifiers, PointerButton, all_squares};

/// Lado de baixo do painel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    WhiteBottom,
    BlackBottom,
}

impl Orientation {
    pub fn flipped(self) -> Self {
        match self {
            Orientation::WhiteBottom => Orientation::BlackBottom,
            Orientation::BlackBottom => Orientation::WhiteBottom,
        }
    }
}

/// Orientação que o tabuleiro principal exibe: no 2D técnico a câmera é fixa (brancas
/// embaixo); nas Apresentações 3D, o lado do `yaw` da câmera (`cos(yaw) < 0` = Vista das
/// pretas, como a Vista invertida).
pub fn orientation_for(yaw: f32, three_d: bool) -> Orientation {
    if three_d && yaw.cos() < -1e-3 {
        Orientation::BlackBottom
    } else {
        Orientation::WhiteBottom
    }
}

/// Coordenadas nas bordas do painel: seguem a opção "Coordenadas no tabuleiro" (#144).
pub fn show_coordinates(world: &World) -> bool {
    world
        .get_resource::<crate::coordinates::BoardCoordinates>()
        .is_none_or(|c| c.0)
}

/// Retângulo da casa dentro do tabuleiro `board` (quadrado).
pub fn square_rect(square: Square, board: Rect, orientation: Orientation) -> Rect {
    let cell = board.width() / 8.0;
    let (col, row) = grid_of(square, orientation);
    let min = board.min + EVec2::new(f32::from(col) * cell, f32::from(row) * cell);
    Rect::from_min_size(min, EVec2::splat(cell))
}

/// (coluna, linha) na tela, a partir da esquerda e do alto.
fn grid_of(square: Square, orientation: Orientation) -> (u8, u8) {
    let (file, rank) = (square.file(), square.rank());
    match orientation {
        Orientation::WhiteBottom => (file, 7 - rank),
        Orientation::BlackBottom => (7 - file, rank),
    }
}

/// Casa sob `pos`; `None` fora do tabuleiro.
pub fn square_at(pos: Pos2, board: Rect, orientation: Orientation) -> Option<Square> {
    if !board.contains(pos) || board.width() <= 0.0 {
        return None;
    }
    let cell = board.width() / 8.0;
    let col = (((pos.x - board.min.x) / cell).floor() as u8).min(7);
    let row = (((pos.y - board.min.y) / cell).floor() as u8).min(7);
    match orientation {
        Orientation::WhiteBottom => Square::new(col, 7 - row),
        Orientation::BlackBottom => Square::new(7 - col, row),
    }
}

/// Ponto da tela -> plano do tabuleiro do `chess_input` (x para a direita, z para o lado das
/// brancas), mesmo fora das 64 casas: serve ao arrasto da peça.
pub fn to_plane(pos: Pos2, board: Rect, orientation: Orientation) -> Vec3 {
    let cell = board.width() / 8.0;
    let c = board.center();
    let (dx, dy) = ((pos.x - c.x) / cell, (pos.y - c.y) / cell);
    match orientation {
        Orientation::WhiteBottom => Vec3::new(dx, 0.0, dy),
        Orientation::BlackBottom => Vec3::new(-dx, 0.0, -dy),
    }
}

/// Inverso de [`to_plane`].
pub fn from_plane(plane: Vec3, board: Rect, orientation: Orientation) -> Pos2 {
    let cell = board.width() / 8.0;
    let c = board.center();
    match orientation {
        Orientation::WhiteBottom => c + EVec2::new(plane.x, plane.z) * cell,
        Orientation::BlackBottom => c - EVec2::new(plane.x, plane.z) * cell,
    }
}

/// Maior quadrado que cabe em `available`, com um piso para o painel nunca sumir.
pub fn board_side(available: EVec2) -> f32 {
    available.x.min(available.y).max(64.0)
}

/// O que o ponteiro fez neste quadro, já em termos do tabuleiro do painel.
#[derive(Clone, Copy, Debug, Default)]
pub struct PointerFrame {
    /// Casa sob o cursor.
    pub square: Option<Square>,
    /// Ponto no plano do tabuleiro.
    pub world: Vec3,
    /// O botão primário desceu agora, sobre o tabuleiro do painel.
    pub pressed: bool,
    /// Primário segurado.
    pub down: bool,
    /// Primário solto agora.
    pub released: bool,
    /// Distância do cursor ao ponto do press, em pontos lógicos.
    pub moved_px: f32,
    /// Tecla do Ajuste (j'adoube).
    pub adjust: bool,
}

/// Estado do gesto em curso no painel (clique ou arrasto).
#[derive(Clone, Copy, Debug, Default)]
pub struct Drive {
    held: bool,
    dragging: bool,
}

impl Drive {
    /// Traduz o quadro do ponteiro nos eventos do intérprete de gestos do `chess_input`.
    /// Mesmo protocolo de `play::gesture_input`: o arrasto só começa depois do limiar.
    pub fn events(&mut self, f: PointerFrame) -> Vec<GestureEvent> {
        let mut out = Vec::new();
        if f.pressed {
            self.held = true;
            self.dragging = false;
            out.push(GestureEvent::PointerDown {
                button: PointerButton::Primary,
                square: f.square,
                modifiers: Modifiers {
                    alt: f.adjust,
                    ..Modifiers::default()
                },
            });
        }
        if self.held && f.down {
            if !self.dragging {
                self.dragging = f.moved_px > DRAG_THRESHOLD_PX;
            }
            if self.dragging {
                out.push(GestureEvent::PointerMove {
                    square: f.square,
                    world: f.world,
                });
            }
        }
        if self.held && f.released {
            self.held = false;
            self.dragging = false;
            out.push(GestureEvent::PointerUp { square: f.square });
        }
        out
    }

    /// Há um gesto do painel em curso?
    pub fn active(&self) -> bool {
        self.held
    }
}

/// Estado do painel (só Apresentação; nada disso vai para a Partida).
#[derive(Resource, Default, Debug)]
pub struct Board2dPanel {
    /// "Jogar por aqui": desligado por padrão (somente leitura).
    pub play_here: bool,
    /// Girar o painel em relação à Vista (só neste painel).
    pub flipped: bool,
    drive: Drive,
    press_pos: Option<Pos2>,
    /// Último quadro em que o ponteiro esteve sobre o tabuleiro do painel com "Jogar por
    /// aqui" ligado: `play::gesture_input` não deve tratá-lo como clique em painel genérico
    /// (isso cancelaria a seleção).
    over_board_frame: Option<u32>,
}

impl Board2dPanel {
    /// O ponteiro está (ou acabou de estar) sobre o tabuleiro jogável do painel?
    pub fn pointer_over_playable(&self, frame: u32) -> bool {
        self.over_board_frame
            .is_some_and(|f| frame.wrapping_sub(f) <= 1)
            || self.drive.active()
    }
}

fn to_c32(color: Color) -> Color32 {
    let s = color.to_srgba();
    Color32::from_rgba_unmultiplied(
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        (s.alpha * 255.0).round() as u8,
    )
}

fn glyph_char(piece: Piece) -> char {
    crate::layout::glyph(piece)
}

pub(crate) fn paint_piece(
    painter: &egui::Painter,
    rect: Rect,
    side: Side,
    piece: Piece,
    alpha: f32,
    center: Option<Pos2>,
) {
    let (fill, outline) = glyph_colors(side);
    let at = center.unwrap_or_else(|| rect.center());
    let font = FontId::proportional(rect.width() * 0.84);
    let text = glyph_char(piece).to_string();
    let tint = |c: Color| to_c32(c.with_alpha(alpha));
    let o = (rect.width() * 0.025).max(0.8);
    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
        painter.text(
            at + EVec2::new(dx, dy),
            egui::Align2::CENTER_CENTER,
            &text,
            font.clone(),
            tint(outline),
        );
    }
    painter.text(at, egui::Align2::CENTER_CENTER, &text, font, tint(fill));
}

/// Desenha o tabuleiro do painel. Puro egui: sem Bevy.
pub fn paint_board(
    painter: &egui::Painter,
    board: Rect,
    orientation: Orientation,
    pieces: &std::collections::HashMap<Square, (Side, Piece)>,
    marks: &Marks,
    dragged: Option<(Square, Pos2)>,
    coordinates: bool,
) {
    let light = to_c32(crate::board2d::LIGHT);
    let dark = to_c32(crate::board2d::DARK);
    for sq in all_squares() {
        let rect = square_rect(sq, board, orientation);
        let is_dark = (sq.file() + sq.rank()) % 2 == 0;
        painter.rect_filled(rect, 0.0, if is_dark { dark } else { light });
    }
    if let Some((from, to)) = marks.last_move {
        for sq in [from, to] {
            painter.rect_filled(
                square_rect(sq, board, orientation),
                0.0,
                Color32::from_rgba_unmultiplied(242, 204, 38, 90),
            );
        }
    }
    if let Some(sq) = marks.check {
        painter.rect_filled(
            square_rect(sq, board, orientation),
            0.0,
            Color32::from_rgba_unmultiplied(230, 26, 26, 140),
        );
    }
    if let Some(sq) = marks.selected {
        painter.rect_filled(
            square_rect(sq, board, orientation),
            0.0,
            Color32::from_rgba_unmultiplied(255, 204, 26, 128),
        );
    }
    if coordinates {
        let size = (board.width() / 8.0 * 0.22).clamp(8.0, 14.0);
        let font = FontId::proportional(size);
        for sq in all_squares() {
            let (col, row) = grid_of(sq, orientation);
            let rect = square_rect(sq, board, orientation);
            let is_dark = (sq.file() + sq.rank()) % 2 == 0;
            let ink = if is_dark { light } else { dark };
            if row == 7 {
                painter.text(
                    rect.right_bottom() - EVec2::new(2.0, 1.0),
                    egui::Align2::RIGHT_BOTTOM,
                    char::from(b'a' + sq.file()),
                    font.clone(),
                    ink,
                );
            }
            if col == 0 {
                painter.text(
                    rect.left_top() + EVec2::new(2.0, 1.0),
                    egui::Align2::LEFT_TOP,
                    char::from(b'1' + sq.rank()),
                    font.clone(),
                    ink,
                );
            }
        }
    }
    for (sq, (side, piece)) in pieces {
        if dragged.is_some_and(|(from, _)| from == *sq) {
            continue;
        }
        paint_piece(
            painter,
            square_rect(*sq, board, orientation),
            *side,
            *piece,
            1.0,
            None,
        );
    }
    for ghost in &marks.ghosts {
        paint_piece(
            painter,
            square_rect(ghost.square, board, orientation),
            ghost.side,
            ghost.piece,
            0.4,
            None,
        );
    }
    for sq in &marks.destinations {
        let rect = square_rect(*sq, board, orientation);
        painter.circle_filled(
            rect.center(),
            rect.width() * 0.15,
            Color32::from_rgba_unmultiplied(26, 140, 64, 190),
        );
    }
    if let Some((from, at)) = dragged
        && let Some((side, piece)) = pieces.get(&from)
    {
        let rect = Rect::from_center_size(at, EVec2::splat(board.width() / 8.0));
        paint_piece(painter, rect, *side, *piece, 1.0, Some(at));
    }
    painter.rect_stroke(
        board,
        0.0,
        Stroke::new(1.0, Color32::from_gray(40)),
        egui::StrokeKind::Outside,
    );
}

fn panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let mut panel = world.remove_resource::<Board2dPanel>().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.checkbox(&mut panel.play_here, i18n.t("board2d-play-here"))
            .on_hover_text(i18n.t("board2d-play-here-hint"));
        if ui
            .button(i18n.t("board2d-flip"))
            .on_hover_text(i18n.t("board2d-flip-hint"))
            .clicked()
        {
            panel.flipped = !panel.flipped;
        }
    });

    let three_d = world
        .get_resource::<crate::presentation::Presentation>()
        .is_none_or(|p| p.is_3d());
    let yaw = world
        .get_resource::<crate::camera::CameraRig>()
        .map_or(0.0, |r| r.pose.yaw);
    let mut orientation = orientation_for(yaw, three_d);
    if panel.flipped {
        orientation = orientation.flipped();
    }

    let side = board_side(ui.available_size());
    let (response, painter) = ui.allocate_painter(EVec2::splat(side), Sense::click_and_drag());
    let board = response.rect;

    let (pieces, drag_world, editor_active) = {
        let session = world.resource::<Session>();
        let over = world.get_resource::<BoardOverride>();
        let default_over = BoardOverride::default();
        let over = over.unwrap_or(&default_over);
        (
            visible_pieces(session, over),
            session.gesture_visual().drag_world,
            over.is_active(),
        )
    };
    let marks = world.get_resource::<Marks>().cloned().unwrap_or_default();
    let origin = world.resource::<Session>().gesture_visual().origin;
    let dragged = drag_world
        .zip(origin)
        .map(|(w, from)| (from, from_plane(w, board, orientation)));
    paint_board(
        &painter,
        board,
        orientation,
        &pieces,
        &marks,
        dragged,
        show_coordinates(world),
    );

    let frame = world.get_resource::<FrameCount>().map_or(0, |f| f.0);
    let can_play = panel.play_here && !editor_active;
    if can_play {
        let (pos, pressed, down, released, alt) = ui.input(|i| {
            (
                i.pointer.hover_pos().or_else(|| i.pointer.interact_pos()),
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.primary_down(),
                i.pointer.button_released(egui::PointerButton::Primary),
                i.modifiers.alt,
            )
        });
        let over_board = response.hovered() || panel.drive.active();
        if response.hovered() {
            panel.over_board_frame = Some(frame);
        }
        let pressed = pressed && response.hovered();
        if pressed {
            panel.press_pos = pos;
        }
        if let Some(pos) = pos.filter(|_| over_board) {
            let moved = panel.press_pos.map_or(0.0, |p| p.distance(pos));
            let events = panel.drive.events(PointerFrame {
                square: square_at(pos, board, orientation),
                world: to_plane(pos, board, orientation),
                pressed,
                down,
                released,
                moved_px: moved,
                adjust: alt,
            });
            if !events.is_empty() {
                let mut session = world.resource_mut::<Session>();
                for event in events {
                    session.gesture(event);
                }
            }
        }
    } else if panel.drive.active() {
        world
            .resource_mut::<Session>()
            .gesture(GestureEvent::PanelInteraction);
        panel.drive = Drive::default();
    }
    world.insert_resource(panel);
}

pub struct Board2dPanelPlugin;

impl Plugin for Board2dPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Board2dPanel>();
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::BOARD2D,
                label_key: "dock-panel-board2d",
                access: PanelAccess::Always,
                ui: panel_ui,
            },
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn board() -> Rect {
        Rect::from_min_size(Pos2::new(10.0, 20.0), EVec2::splat(320.0))
    }

    #[test]
    fn white_bottom_puts_a1_at_the_lower_left_and_h8_at_the_upper_right() {
        let b = board();
        let a1 = square_rect(sq("a1"), b, Orientation::WhiteBottom);
        assert_eq!(a1.left_bottom(), b.left_bottom());
        let h8 = square_rect(sq("h8"), b, Orientation::WhiteBottom);
        assert_eq!(h8.right_top(), b.right_top());
    }

    #[test]
    fn black_bottom_mirrors_the_board() {
        let b = board();
        let a1 = square_rect(sq("a1"), b, Orientation::BlackBottom);
        assert_eq!(a1.right_top(), b.right_top());
        let h8 = square_rect(sq("h8"), b, Orientation::BlackBottom);
        assert_eq!(h8.left_bottom(), b.left_bottom());
    }

    #[test]
    fn picking_inverts_the_drawing_in_both_orientations() {
        let b = board();
        for o in [Orientation::WhiteBottom, Orientation::BlackBottom] {
            for s in all_squares() {
                assert_eq!(square_at(square_rect(s, b, o).center(), b, o), Some(s));
            }
        }
        assert_eq!(
            square_at(Pos2::new(0.0, 0.0), b, Orientation::WhiteBottom),
            None
        );
    }

    #[test]
    fn plane_coordinates_match_chess_input_in_both_orientations() {
        let b = board();
        for o in [Orientation::WhiteBottom, Orientation::BlackBottom] {
            for s in all_squares() {
                let p = to_plane(square_rect(s, b, o).center(), b, o);
                let c = chess_input::square_center(s);
                assert!((p.x - c.x).abs() < 1e-4 && (p.z - c.z).abs() < 1e-4, "{s}");
                assert_eq!(chess_input::point_to_square(p), Some(s));
                let back = from_plane(p, b, o);
                assert!(back.distance(square_rect(s, b, o).center()) < 1e-3);
            }
        }
    }

    #[test]
    fn orientation_follows_the_view_side() {
        assert_eq!(orientation_for(0.0, true), Orientation::WhiteBottom);
        assert_eq!(
            orientation_for(180f32.to_radians(), true),
            Orientation::BlackBottom
        );
        assert_eq!(
            orientation_for(35f32.to_radians(), true),
            Orientation::WhiteBottom
        );
        // O 2D técnico tem câmera fixa: as brancas ficam embaixo mesmo com a Vista invertida.
        assert_eq!(
            orientation_for(180f32.to_radians(), false),
            Orientation::WhiteBottom
        );
    }

    #[test]
    fn the_board_resizes_with_the_panel_and_never_vanishes() {
        assert_eq!(board_side(EVec2::new(300.0, 500.0)), 300.0);
        assert_eq!(board_side(EVec2::new(500.0, 200.0)), 200.0);
        assert_eq!(board_side(EVec2::new(10.0, 10.0)), 64.0);
    }

    // --- jogar por aqui: os eventos entram no mesmo intérprete/Partida do tabuleiro principal ---

    fn frame(s: Option<Square>) -> PointerFrame {
        PointerFrame {
            square: s,
            ..PointerFrame::default()
        }
    }

    fn feed(session: &mut Session, drive: &mut Drive, f: PointerFrame) {
        for e in drive.events(f) {
            session.gesture(e);
        }
    }

    fn click(session: &mut Session, drive: &mut Drive, at: &str) {
        let s = Some(sq(at));
        feed(
            session,
            drive,
            PointerFrame {
                pressed: true,
                down: true,
                ..frame(s)
            },
        );
        feed(
            session,
            drive,
            PointerFrame {
                released: true,
                ..frame(s)
            },
        );
    }

    #[test]
    fn two_clicks_play_a_move_through_the_session() {
        let mut session = Session::new();
        let mut drive = Drive::default();
        click(&mut session, &mut drive, "e2");
        assert_eq!(session.snapshot().history.len(), 0);
        click(&mut session, &mut drive, "e4");
        assert_eq!(session.snapshot().history.len(), 1);
    }

    #[test]
    fn a_drag_past_the_threshold_plays_the_move() {
        let mut session = Session::new();
        let mut drive = Drive::default();
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                pressed: true,
                down: true,
                ..frame(Some(sq("e2")))
            },
        );
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                down: true,
                moved_px: 40.0,
                world: chess_input::square_center(sq("e4")),
                ..frame(Some(sq("e4")))
            },
        );
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                released: true,
                ..frame(Some(sq("e4")))
            },
        );
        assert_eq!(session.snapshot().history.len(), 1);
    }

    #[test]
    fn a_small_wobble_is_still_a_tap() {
        let mut session = Session::new();
        let mut drive = Drive::default();
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                pressed: true,
                down: true,
                ..frame(Some(sq("e2")))
            },
        );
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                down: true,
                moved_px: 3.0,
                ..frame(Some(sq("e3")))
            },
        );
        feed(
            &mut session,
            &mut drive,
            PointerFrame {
                released: true,
                ..frame(Some(sq("e3")))
            },
        );
        assert_eq!(session.snapshot().history.len(), 0);
        assert_eq!(session.gesture_visual().origin, Some(sq("e2")));
    }

    #[test]
    fn the_session_still_decides_in_review_and_on_the_engines_turn() {
        let mut session = Session::new();
        let mut drive = Drive::default();
        click(&mut session, &mut drive, "e2");
        click(&mut session, &mut drive, "e4");
        session.set_review_ply(Some(0));
        click(&mut session, &mut drive, "e7");
        click(&mut session, &mut drive, "e5");
        assert_eq!(session.snapshot().history.len(), 1, "revisão é só leitura");
    }

    #[test]
    fn move_method_is_respected() {
        let mut session = Session::new();
        session.set_move_method(chess_input::MoveMethod::Drag);
        let mut drive = Drive::default();
        click(&mut session, &mut drive, "e2");
        click(&mut session, &mut drive, "e4");
        assert_eq!(session.snapshot().history.len(), 0, "só arrasto");
    }

    #[test]
    fn an_illegal_move_is_not_played() {
        let mut session = Session::new();
        let mut drive = Drive::default();
        click(&mut session, &mut drive, "e2");
        click(&mut session, &mut drive, "e5");
        assert_eq!(session.snapshot().history.len(), 0);
    }

    #[test]
    fn the_panel_can_be_detached_to_a_floating_window_and_docked_back() {
        let mut layout = crate::dock::DockLayout::default();
        assert!(layout.detach(ids::BOARD2D, [100.0, 100.0], [320.0, 360.0]));
        assert!(layout.is_floating(ids::BOARD2D));
        assert!(layout.dock_back(ids::BOARD2D));
        assert!(!layout.is_floating(ids::BOARD2D));
        assert!(layout.is_visible(ids::BOARD2D));
    }

    #[test]
    fn drawing_the_read_only_panel_never_mutates_the_game() {
        use crate::aids::AidSettings;
        let mut app = App::new();
        app.add_plugins(Board2dPanelPlugin)
            .insert_resource(I18n::pt_br())
            .insert_resource(Session::new())
            .insert_resource(AidSettings::physical())
            .init_resource::<Marks>();
        let before = app.world().resource::<Session>().snapshot().clone();
        let ctx = egui::Context::default();
        let world = app.world_mut();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
            egui::Area::new(egui::Id::new("t")).show(&ctx, |ui| {
                panel_ui(ui, &I18n::pt_br(), world);
            });
        });
        output.textures_delta.clear();
        assert_eq!(app.world().resource::<Session>().snapshot(), &before);
        assert!(!app.world().resource::<Board2dPanel>().play_here);
    }
}
