//! Intérprete de gestos (máquina de estados pura que traduz eventos de ponteiro,
//! teclado e foco em `Command`s) e geometria pura de tabuleiro/picking
//! (raio para casa, casa para transform) nas quatro Vistas.
//!
//! Nada aqui depende do Bevy: só `glam` (o mesmo tipo de vetor que o Bevy reexporta).

pub mod board;
pub mod square;
pub mod view;

pub use board::{BOARD_SIZE, SQUARE_SIZE, ray_to_square, square_center};
pub use square::BoardSquare;
pub use view::{CameraPose, CameraView};
