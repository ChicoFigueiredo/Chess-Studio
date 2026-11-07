//! Exercícios e Resoluções (B13). O catálogo vem de arquivo versionado
//! (`chess-training`) e é espelhado nas tabelas `exercises`/`exercise_runs`; cada
//! Resolução encerrada é um registro imutável. Nada aqui lê ou escreve `games`:
//! o treino nunca altera resultados de Partidas competitivas (TRN-05).

use crate::error::{Result, StoreError};
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use chess_training::{Exercise, ExerciseKind, Metrics, RunSummary};
use rusqlite::{OptionalExtension, params};

/// Identificador de uma Resolução gravada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunId(pub i64);

/// O que `upsert_exercises` fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpsertReport {
    pub inserted: usize,
    pub updated: usize,
    /// Já estavam na mesma versão (ou numa mais nova) e foram mantidos.
    pub kept: usize,
}

/// Filtro de [`Store::list_runs`] e [`Store::metrics`]; o padrão lista tudo.
#[derive(Debug, Clone, Default)]
pub struct RunQuery {
    pub profile: Option<ProfileId>,
    pub exercise: Option<String>,
    pub kind: Option<ExerciseKind>,
    pub session: Option<u64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredRun {
    pub id: RunId,
    pub profile: ProfileId,
    pub created_at_ms: i64,
    pub summary: RunSummary,
}

impl Store {
    /// Grava o catálogo: insere o que é novo e substitui só quando a versão do
    /// arquivo é **maior** que a do banco. Idempotente; preserva as Resoluções.
    pub fn upsert_exercises(&mut self, exercises: &[Exercise]) -> Result<UpsertReport> {
        let now = now_ms();
        let tx = self.conn.transaction()?;
        let mut report = UpsertReport::default();
        for exercise in exercises {
            let stored: Option<i64> = tx
                .query_row(
                    "SELECT version FROM exercises WHERE slug = ?1",
                    [&exercise.id],
                    |r| r.get(0),
                )
                .optional()?;
            let definition = serde_json::to_string(exercise)?;
            let solution = serde_json::to_string(&exercise.solution)?;
            match stored {
                None => {
                    tx.execute(
                        "INSERT INTO exercises (kind, initial_fen, solution_json, version,
                                                created_at_ms, slug, definition_json)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            exercise.kind.as_str(),
                            exercise.fen,
                            solution,
                            exercise.version,
                            now,
                            exercise.id,
                            definition
                        ],
                    )?;
                    report.inserted += 1;
                }
                Some(version) if i64::from(exercise.version) > version => {
                    tx.execute(
                        "UPDATE exercises SET kind = ?2, initial_fen = ?3, solution_json = ?4,
                                version = ?5, definition_json = ?6
                         WHERE slug = ?1",
                        params![
                            exercise.id,
                            exercise.kind.as_str(),
                            exercise.fen,
                            solution,
                            exercise.version,
                            definition
                        ],
                    )?;
                    report.updated += 1;
                }
                Some(_) => report.kept += 1,
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Exercícios do catálogo gravado, na ordem de inserção (linhas antigas sem
    /// definição ficam de fora).
    pub fn list_exercises(&self) -> Result<Vec<Exercise>> {
        let mut stmt = self.conn.prepare(
            "SELECT definition_json FROM exercises WHERE definition_json != '{}' ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(exercise) = serde_json::from_str(&row?) {
                out.push(exercise);
            }
        }
        Ok(out)
    }

    /// Abre uma Resolução (ainda não encerrada: fica fora das listagens e métricas).
    pub fn start_run(
        &mut self,
        profile: ProfileId,
        exercise_id: &str,
        session: u64,
    ) -> Result<RunId> {
        ensure_profile(&self.conn, profile)?;
        let exercise: i64 = self
            .conn
            .query_row(
                "SELECT id FROM exercises WHERE slug = ?1",
                [exercise_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::ExerciseNotFound(exercise_id.to_owned()))?;
        self.conn.execute(
            "INSERT INTO exercise_runs (exercise_id, profile_id, correct, hints_used, duration_ms,
                                        created_at_ms, finished, session)
             VALUES (?1, ?2, 0, 0, 0, ?3, 0, ?4)",
            params![
                exercise,
                profile.0,
                now_ms(),
                i64::try_from(session).unwrap_or(i64::MAX)
            ],
        )?;
        Ok(RunId(self.conn.last_insert_rowid()))
    }

    /// Encerra a Resolução com o registro imutável. Encerrar duas vezes é erro: repetir
    /// o Exercício é outra Resolução (`start_run`), nunca uma reescrita.
    pub fn finish_run(&mut self, run: RunId, summary: &RunSummary) -> Result<()> {
        let tx = self.conn.transaction()?;
        let row: Option<(bool, String)> = tx
            .query_row(
                "SELECT r.finished, e.slug FROM exercise_runs r
                 JOIN exercises e ON e.id = r.exercise_id WHERE r.id = ?1",
                [run.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (finished, slug) = row.ok_or(StoreError::RunNotFound(run))?;
        if finished {
            return Err(StoreError::RunAlreadyFinished(run));
        }
        if slug != summary.exercise_id {
            return Err(StoreError::RunMismatch {
                run,
                expected: slug,
                got: summary.exercise_id.clone(),
            });
        }
        tx.execute(
            "UPDATE exercise_runs SET finished = 1, correct = ?2, hints_used = ?3,
                    duration_ms = ?4, errors = ?5, session = ?6, summary_json = ?7
             WHERE id = ?1",
            params![
                run.0,
                summary.result == chess_training::RunResult::Correct,
                summary.aids.used.len() as i64,
                i64::try_from(summary.duration_ms).unwrap_or(i64::MAX),
                summary.errors.len() as i64,
                i64::try_from(summary.session).unwrap_or(i64::MAX),
                serde_json::to_string(summary)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Resoluções encerradas, da mais recente à mais antiga.
    pub fn list_runs(&self, query: &RunQuery) -> Result<Vec<StoredRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.profile_id, r.created_at_ms, r.summary_json
             FROM exercise_runs r JOIN exercises e ON e.id = r.exercise_id
             WHERE r.finished = 1 AND r.summary_json != '{}'
               AND (?1 IS NULL OR r.profile_id = ?1)
               AND (?2 IS NULL OR e.slug = ?2)
               AND (?3 IS NULL OR e.kind = ?3)
               AND (?4 IS NULL OR r.session = ?4)
             ORDER BY r.created_at_ms DESC, r.id DESC
             LIMIT ?5",
        )?;
        let rows = stmt.query_map(
            params![
                query.profile.map(|p| p.0),
                query.exercise,
                query.kind.map(ExerciseKind::as_str),
                query.session.map(|s| i64::try_from(s).unwrap_or(i64::MAX)),
                query.limit.map_or(-1, i64::from),
            ],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            let (id, profile, created_at_ms, json) = row?;
            out.push(StoredRun {
                id: RunId(id),
                profile: ProfileId(profile),
                created_at_ms,
                summary: serde_json::from_str(&json)?,
            });
        }
        Ok(out)
    }

    /// Acertos, erros e Exercícios sem Auxílios por tipo, sessão e Exercício (TRN-05).
    pub fn metrics(&self, query: &RunQuery) -> Result<Metrics> {
        let summaries: Vec<RunSummary> = self
            .list_runs(query)?
            .into_iter()
            .map(|r| r.summary)
            .collect();
        Ok(Metrics::from_runs(&summaries))
    }
}
