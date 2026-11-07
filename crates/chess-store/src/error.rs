//! Erros tipados do repositório.

use crate::{EngineId, FavoriteId, GameId, ImportedId, ProfileId, RunId};
use chess_game::ReplayError;
use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// Falha do SQLite (E/S, disco cheio, banco travado além do `busy_timeout`...).
    Sqlite(rusqlite::Error),
    /// JSON de payload, Configuração ou preferência ilegível.
    Json(serde_json::Error),
    GameNotFound(GameId),
    ProfileNotFound(ProfileId),
    /// O banco foi criado por uma versão mais nova do app.
    SchemaTooNew {
        found: u32,
        supported: u32,
    },
    /// Evento com versão de payload que esta build não conhece.
    UnsupportedEventVersion {
        game: GameId,
        seq: u64,
        version: u16,
    },
    /// O log existe mas não reaplica sobre a Configuração (banco corrompido ou editado).
    Replay {
        game: GameId,
        source: ReplayError,
    },
    /// Buraco ou desordem na sequência do log de uma Partida.
    CorruptLog {
        game: GameId,
        detail: String,
    },
    /// Exercício que não está no catálogo gravado.
    ExerciseNotFound(String),
    RunNotFound(RunId),
    /// Engine que não está registrada.
    EngineNotFound(EngineId),
    /// A Resolução já foi encerrada: o registro é imutável.
    RunAlreadyFinished(RunId),
    /// O registro encerrado não é do Exercício da Resolução iniciada.
    RunMismatch {
        run: RunId,
        expected: String,
        got: String,
    },
    /// Partida importada de PGN que não existe (ou foi excluída).
    ImportedGameNotFound(ImportedId),
    /// FEN que não descreve uma Posição válida (busca por Posição, favoritas).
    InvalidFen(String),
    /// Posição favorita que não existe (ou foi removida).
    FavoriteNotFound(FavoriteId),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "erro do SQLite: {e}"),
            Self::Json(e) => write!(f, "JSON inválido no banco: {e}"),
            Self::GameNotFound(id) => write!(f, "Partida {} não existe", id.0),
            Self::ProfileNotFound(id) => write!(f, "perfil {} não existe", id.0),
            Self::SchemaTooNew { found, supported } => write!(
                f,
                "banco na versão de esquema {found}, mas este app só entende até {supported}; atualize o Chess Studio"
            ),
            Self::UnsupportedEventVersion { game, seq, version } => write!(
                f,
                "evento {seq} da Partida {} tem payload na versão {version}, desconhecida desta build (atual: {})",
                game.0,
                chess_game::PAYLOAD_VERSION
            ),
            Self::Replay { game, source } => {
                write!(f, "o log da Partida {} não reaplica: {source}", game.0)
            }
            Self::CorruptLog { game, detail } => {
                write!(f, "log da Partida {} corrompido: {detail}", game.0)
            }
            Self::ExerciseNotFound(id) => write!(f, "Exercício `{id}` não está no catálogo"),
            Self::RunNotFound(id) => write!(f, "Resolução {} não existe", id.0),
            Self::EngineNotFound(id) => write!(f, "Engine {} não está registrada", id.0),
            Self::RunAlreadyFinished(id) => {
                write!(
                    f,
                    "Resolução {} já foi encerrada e não pode ser reescrita",
                    id.0
                )
            }
            Self::RunMismatch { run, expected, got } => write!(
                f,
                "Resolução {} é do Exercício `{expected}`, não de `{got}`",
                run.0
            ),
            Self::ImportedGameNotFound(id) => write!(f, "partida importada {} não existe", id.0),
            Self::InvalidFen(detail) => write!(f, "FEN inválido: {detail}"),
            Self::FavoriteNotFound(id) => write!(f, "posição favorita {} não existe", id.0),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Replay { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;
