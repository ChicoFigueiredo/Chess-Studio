//! Catálogo de Exercícios carregado de arquivo versionado (JSON).
//!
//! Formato (`crates/chess-training/exercises/*.json`):
//!
//! ```json
//! { "format": 1,
//!   "exercises": [
//!     { "id": "roque-001", "kind": "castling", "title": "...", "fen": "...",
//!       "objective": "...", "solution": { "type": "moves", "line": ["e1g1"] },
//!       "explanation": "...", "version": 1 } ] }
//! ```
//!
//! `solution.type`: `moves` (+ `line`), `mate_in_one`, `force_stalemate`, `give_check`,
//! `refuse_illegal` (+ `uci`), `memorize` (+ `line` de 3 a 8 Lances e `speed_ms`, só no tipo `memory`). Cada Exercício é validado pelo chess-core ao carregar.

use crate::exercise::{Exercise, ExerciseError};
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

/// Versão do formato de arquivo que esta build lê.
pub const FORMAT_VERSION: u32 = 1;

const BUNDLED: &[(&str, &str)] = &[
    ("basicos.json", include_str!("../exercises/basicos.json")),
    ("memoria.json", include_str!("../exercises/memoria.json")),
];

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogError {
    Io(String),
    Parse(String),
    UnsupportedFormat(u32),
    DuplicateId(String),
    Invalid { id: String, source: ExerciseError },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "não foi possível ler o catálogo: {e}"),
            Self::Parse(e) => write!(f, "catálogo ilegível: {e}"),
            Self::UnsupportedFormat(v) => write!(f, "formato de catálogo {v} desconhecido"),
            Self::DuplicateId(id) => write!(f, "Exercício `{id}` repetido"),
            Self::Invalid { id, source } => write!(f, "Exercício `{id}` inválido: {source}"),
        }
    }
}

impl std::error::Error for CatalogError {}

#[derive(Deserialize)]
struct File {
    format: u32,
    exercises: Vec<Exercise>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalog {
    exercises: Vec<Exercise>,
}

impl Catalog {
    /// Lê e valida um arquivo do catálogo.
    pub fn from_json(text: &str) -> Result<Self, CatalogError> {
        let mut catalog = Self::default();
        catalog.add_file(text)?;
        Ok(catalog)
    }

    fn add_file(&mut self, text: &str) -> Result<(), CatalogError> {
        let file: File =
            serde_json::from_str(text).map_err(|e| CatalogError::Parse(e.to_string()))?;
        if file.format != FORMAT_VERSION {
            return Err(CatalogError::UnsupportedFormat(file.format));
        }
        let mut seen: HashSet<String> = self.exercises.iter().map(|e| e.id.clone()).collect();
        for exercise in file.exercises {
            exercise
                .validate()
                .map_err(|source| CatalogError::Invalid {
                    id: exercise.id.clone(),
                    source,
                })?;
            if !seen.insert(exercise.id.clone()) {
                return Err(CatalogError::DuplicateId(exercise.id));
            }
            self.exercises.push(exercise);
        }
        Ok(())
    }

    /// O catálogo versionado que acompanha o app (embutido no binário).
    pub fn bundled() -> Result<Self, CatalogError> {
        let mut catalog = Self::default();
        for (_, text) in BUNDLED {
            catalog.add_file(text)?;
        }
        Ok(catalog)
    }

    /// Todos os `*.json` de um diretório, em ordem de nome (curadoria do B14).
    pub fn from_dir(dir: &Path) -> Result<Self, CatalogError> {
        let io = |e: std::io::Error| CatalogError::Io(e.to_string());
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(io)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let mut catalog = Self::default();
        for path in files {
            catalog.add_file(&std::fs::read_to_string(path).map_err(io)?)?;
        }
        Ok(catalog)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Exercise> {
        self.exercises.iter()
    }

    pub fn get(&self, id: &str) -> Option<&Exercise> {
        self.exercises.iter().find(|e| e.id == id)
    }

    pub fn len(&self) -> usize {
        self.exercises.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exercises.is_empty()
    }
}
