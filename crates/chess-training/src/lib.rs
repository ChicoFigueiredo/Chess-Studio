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
mod evolution;
mod exercise;
mod memory;
mod metrics;
mod position_memory;
mod profiles;
mod review;
mod run;
mod scoresheet;
mod scoresheet_check;
mod scoresheet_history;
mod scoresheet_pdf;

pub use aids::{Aid, AidsRecord, Feedback};
pub use catalog::{Catalog, CatalogError};
pub use evolution::{
    DatedRun, Endgames, Evolution, Period, PeriodStats, by_period, by_player, by_training_profile,
    civil_date, day_of, report,
};
pub use exercise::{
    Exercise, ExerciseError, ExerciseKind, MEMORY_LINE_RANGE, POSITION_STUDY_SECONDS, Solution,
};
pub use memory::{MemoryPhase, MemoryRun, MemorySummary, PlyResult};
pub use metrics::{Metrics, Tally};
pub use position_memory::{
    PiecePlacement, PositionJudgment, PositionMemoryRun, PositionMemorySummary, PositionPhase,
    SquareVerdict, judge, squares_in_reading_order,
};
pub use profiles::{Difficulty, ProfileError, ProfileSet, ProfileStep, TrainingProfile};
pub use review::{MemoryReview, NotCheckmate, PlyReview, Review};
pub use run::{ErrorCause, ExerciseRun, Followup, RunError, RunResult, RunStatus, RunSummary};
pub use scoresheet::{MAX_ENTRY_CHARS, MAX_PLY, ManualScoresheet, ScoresheetError};
pub use scoresheet_check::{
    CheckError, Counts, EntryCheck, ScoresheetCheck, Verdict, check_against_game, check_scoresheet,
};
pub use scoresheet_history::{HistoryPeriod, HistoryPoint, ScoresheetHistory};
pub use scoresheet_pdf::{
    A4_HEIGHT_PT, A4_WIDTH_PT, MAX_BLANK_PAGES, PdfError, ROWS_PER_PAGE, SheetContent, SheetHeader,
    SheetLabels, SheetSpec, page_count, render_scoresheet_pdf, rows_for,
};
