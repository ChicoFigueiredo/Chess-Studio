//! Intérprete de gestos (máquina de estados pura que traduz eventos de ponteiro,
//! teclado e foco em `Command`s) e geometria pura de tabuleiro/picking
//! (raio para casa, casa para transform) nas quatro Vistas.
//!
//! Nada aqui depende do Bevy: só `glam` (o mesmo tipo de vetor que o Bevy reexporta).

pub mod board;
pub mod gesture;
pub mod settle;
pub mod view;

pub use board::{
    BOARD_SIZE, SQUARE_SIZE, all_squares, point_to_square, ray_to_board_plane, ray_to_square,
    square_center,
};
pub use gesture::{
    GestureEvent, GestureInterpreter, GestureState, GestureVisual, Modifiers, MoveMethod,
    PointerButton,
};
pub use settle::{DRAG_HEIGHT, SETTLE_SECS, settle_position};
pub use view::{CameraPose, CameraView, frame_points};
