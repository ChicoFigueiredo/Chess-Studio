//! Treinamento: catálogo de Exercícios, Resolução sobre uma Partida em modo de
//! exercício, revisão e métricas por tipo e sessão (memória de sequência é do B15).
//!
//! Nada aqui depende de Bevy, de SQLite ou do relógio do sistema (ADR-0001/0002):
//! os instantes entram por parâmetro e a Partida do Exercício é uma [`chess_game::Game`]
//! na Política Assistido (desfazer permitido, DEC-39). Uma Resolução ([`ExerciseRun`])
//! é um **juiz** que observa essa Partida: quem escreve nela é sempre o controlador
//! (o app, via `Session`, ou os testes); a Resolução só lê os Eventos novos e devolve
//! [`Followup`]s (desfazer um Lance fora do objetivo, jogar a resposta do adversário).
//!
//! Resultados de Partidas competitivas nunca passam por aqui: uma Resolução não é uma
//! Partida do histórico (TRN-05).
//!
//! ```
//! use chess_training::{AidsRecord, Catalog, ExerciseRun, Feedback, RunStatus};
//!
//! let catalog = Catalog::bundled().unwrap();
//! let exercise = catalog.get("mate-em-1-001").unwrap();
//! let mut game = exercise.new_game().unwrap();
//! let mut run = ExerciseRun::start(exercise, AidsRecord::new(&[], Feedback::Immediate), 0);
//! assert_eq!(run.status(), RunStatus::InProgress);
//! assert_eq!(game.snapshot().position.to_fen(), exercise.fen);
//! ```

mod aids;
mod catalog;
mod exercise;
mod metrics;
mod review;
mod run;

pub use aids::{Aid, AidsRecord, Feedback};
pub use catalog::{Catalog, CatalogError};
pub use exercise::{Exercise, ExerciseError, ExerciseKind, Solution};
pub use metrics::{Metrics, Tally};
pub use review::{NotCheckmate, Review};
pub use run::{ErrorCause, ExerciseRun, Followup, RunError, RunResult, RunStatus, RunSummary};
