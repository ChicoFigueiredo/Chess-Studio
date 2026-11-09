//! Intérprete de gestos (máquina de estados pura que traduz eventos de ponteiro,
//! teclado e foco em `Command`s) e geometria pura de tabuleiro/picking
//! (raio para casa, casa para transform) nas quatro Vistas.
//!
//! Nada aqui depende do Bevy: só `glam` (o mesmo tipo de vetor que o Bevy reexporta).

pub mod arrow;
pub mod board;
pub mod controls;
pub mod gesture;
pub mod settle;
pub mod view;

pub use arrow::{ARROW_TRIANGLES, arrow_outline};
pub use board::{
    BOARD_SIZE, PIECE_PICK_RADIUS, SQUARE_SIZE, all_squares, piece_pick_height, point_to_square,
    ray_to_board_plane, ray_to_square, ray_to_square_with_pieces, square_center,
};
pub use gesture::{
    GestureEvent, GestureInterpreter, GestureState, GestureVisual, Modifiers, MoveMethod,
    PointerButton,
};
pub use settle::{DRAG_HEIGHT, SETTLE_SECS, settle_position};
pub use view::{CameraPose, CameraView, fit_distance, frame_points};
