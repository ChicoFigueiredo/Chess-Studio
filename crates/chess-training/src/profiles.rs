//! Perfis de treino (US-27): sequências nomeadas de Exercícios, cada passo com uma
//! dificuldade. O avanço (quais passos já foram cumpridos) é calculado a partir do
//! conjunto de Exercícios resolvidos; quem o guarda é o store.
//!
//! Formato (`crates/chess-training/profiles/*.json`):
//!
//! ```json
//! { "format": 1,
//!   "profiles": [
//!     { "id": "regras-basicas", "name": "Regras básicas", "description": "...",
//!       "steps": [ { "exercise": "roque-001", "difficulty": "easy" } ] } ] }
//! ```

use crate::catalog::Catalog;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

const FORMAT_VERSION: u32 = 1;

const BUNDLED: &str = include_str!("../profiles/perfis.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Medium => "medium",
            Self::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileStep {
    /// Identificador do Exercício no catálogo.
    pub exercise: String,
    pub difficulty: Difficulty,
}

/// Um Perfil de treino: sequência nomeada de Exercícios (não confundir com o Perfil de
/// auxílios nem com o Perfil do jogador no banco).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<ProfileStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    Parse(String),
    UnsupportedFormat(u32),
    DuplicateId(String),
    EmptyField {
        profile: String,
        field: &'static str,
    },
    NoSteps(String),
    UnknownExercise {
        profile: String,
        exercise: String,
    },
    RepeatedExercise {
        profile: String,
        exercise: String,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "perfis ilegíveis: {e}"),
            Self::UnsupportedFormat(v) => write!(f, "formato de perfis {v} desconhecido"),
            Self::DuplicateId(id) => write!(f, "Perfil de treino `{id}` repetido"),
            Self::EmptyField { profile, field } => {
                write!(f, "Perfil `{profile}`: campo `{field}` vazio")
            }
            Self::NoSteps(id) => write!(f, "Perfil `{id}` sem passos"),
            Self::UnknownExercise { profile, exercise } => {
                write!(f, "Perfil `{profile}`: Exercício `{exercise}` não existe")
            }
            Self::RepeatedExercise { profile, exercise } => {
                write!(f, "Perfil `{profile}`: Exercício `{exercise}` repetido")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

impl TrainingProfile {
    pub fn exercise_ids(&self) -> impl Iterator<Item = &str> {
        self.steps.iter().map(|s| s.exercise.as_str())
    }

    /// Quantos passos já foram cumpridos (`done` = Exercícios resolvidos).
    pub fn completed(&self, done: &BTreeSet<String>) -> usize {
        self.steps
            .iter()
            .filter(|s| done.contains(&s.exercise))
            .count()
    }

    /// O primeiro passo ainda não cumprido, na ordem do Perfil.
    pub fn next_step(&self, done: &BTreeSet<String>) -> Option<&ProfileStep> {
        self.steps.iter().find(|s| !done.contains(&s.exercise))
    }

    pub fn is_complete(&self, done: &BTreeSet<String>) -> bool {
        self.completed(done) == self.steps.len()
    }

    /// Avanço em %, arredondado.
    pub fn percent(&self, done: &BTreeSet<String>) -> u32 {
        let total = self.steps.len();
        if total == 0 {
            return 0;
        }
        u32::try_from((self.completed(done) * 100 + total / 2) / total).unwrap_or(100)
    }

    /// O Exercício `exercise_id` faz parte deste Perfil?
    pub fn contains(&self, exercise_id: &str) -> bool {
        self.exercise_ids().any(|e| e == exercise_id)
    }
}

#[derive(Deserialize)]
struct File {
    format: u32,
    profiles: Vec<TrainingProfile>,
}

/// Os Perfis de treino carregados (validados contra o catálogo).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileSet {
    profiles: Vec<TrainingProfile>,
}

impl ProfileSet {
    pub fn from_json(text: &str, catalog: &Catalog) -> Result<Self, ProfileError> {
        let file: File =
            serde_json::from_str(text).map_err(|e| ProfileError::Parse(e.to_string()))?;
        if file.format != FORMAT_VERSION {
            return Err(ProfileError::UnsupportedFormat(file.format));
        }
        let mut seen = BTreeSet::new();
        for profile in &file.profiles {
            for (field, value) in [
                ("id", &profile.id),
                ("name", &profile.name),
                ("description", &profile.description),
            ] {
                if value.trim().is_empty() {
                    return Err(ProfileError::EmptyField {
                        profile: profile.id.clone(),
                        field,
                    });
                }
            }
            if !seen.insert(profile.id.clone()) {
                return Err(ProfileError::DuplicateId(profile.id.clone()));
            }
            if profile.steps.is_empty() {
                return Err(ProfileError::NoSteps(profile.id.clone()));
            }
            let mut exercises = BTreeSet::new();
            for step in &profile.steps {
                if catalog.get(&step.exercise).is_none() {
                    return Err(ProfileError::UnknownExercise {
                        profile: profile.id.clone(),
                        exercise: step.exercise.clone(),
                    });
                }
                if !exercises.insert(step.exercise.as_str()) {
                    return Err(ProfileError::RepeatedExercise {
                        profile: profile.id.clone(),
                        exercise: step.exercise.clone(),
                    });
                }
            }
        }
        Ok(Self {
            profiles: file.profiles,
        })
    }

    /// Os Perfis que acompanham o app (embutidos no binário).
    pub fn bundled(catalog: &Catalog) -> Result<Self, ProfileError> {
        Self::from_json(BUNDLED, catalog)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TrainingProfile> {
        self.profiles.iter()
    }

    pub fn get(&self, id: &str) -> Option<&TrainingProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}
