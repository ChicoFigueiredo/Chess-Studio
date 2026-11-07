//! Perfis de treino (US-27) e Resoluções datadas (US-26). O catálogo de Perfis vem de
//! arquivo versionado (`chess-training`); aqui só ficam o **avanço** de cada jogador
//! (um registro por passo cumprido) e qual Perfil está ativo. Nada aqui lê ou escreve
//! `games`: o treino nunca altera resultados de Partidas competitivas (TRN-05).

use crate::error::Result;
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use crate::training::{RunQuery, StoredRun};
use chess_training::{DatedRun, ProfileSet, RunResult, RunSummary};
use rusqlite::{OptionalExtension, params};
use std::collections::BTreeSet;

impl StoredRun {
    /// A Resolução com o instante e o jogador, no formato das métricas de evolução.
    pub fn dated(&self) -> DatedRun {
        DatedRun {
            at_ms: self.created_at_ms,
            player: self.profile.0,
            summary: self.summary.clone(),
        }
    }
}

impl Store {
    /// Resoluções encerradas com instante e jogador, da mais recente à mais antiga.
    pub fn dated_runs(&self, query: &RunQuery) -> Result<Vec<DatedRun>> {
        Ok(self
            .list_runs(query)?
            .iter()
            .map(StoredRun::dated)
            .collect())
    }

    /// Registra um passo cumprido do Perfil de treino. `true` se é novo (o registro é
    /// idempotente: cumprir de novo não duplica nem muda a data).
    pub fn record_profile_step(
        &mut self,
        player: ProfileId,
        training_profile: &str,
        exercise_id: &str,
    ) -> Result<bool> {
        ensure_profile(&self.conn, player)?;
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO training_profile_progress
                 (profile_id, training_profile, exercise, completed_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![player.0, training_profile, exercise_id, now_ms()],
        )?;
        Ok(changed > 0)
    }

    /// Exercícios já cumpridos no Perfil de treino.
    pub fn profile_progress(
        &self,
        player: ProfileId,
        training_profile: &str,
    ) -> Result<BTreeSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT exercise FROM training_profile_progress
             WHERE profile_id = ?1 AND training_profile = ?2",
        )?;
        let rows = stmt.query_map(params![player.0, training_profile], |r| {
            r.get::<_, String>(0)
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Uma Resolução acertada cumpre o passo em todos os Perfis de treino que incluem o
    /// Exercício. Devolve os Perfis em que o passo era novo.
    pub fn advance_profiles(
        &mut self,
        player: ProfileId,
        profiles: &ProfileSet,
        summary: &RunSummary,
    ) -> Result<Vec<String>> {
        let mut advanced = Vec::new();
        if summary.result != RunResult::Correct {
            return Ok(advanced);
        }
        for profile in profiles.iter().filter(|p| p.contains(&summary.exercise_id)) {
            if self.record_profile_step(player, &profile.id, &summary.exercise_id)? {
                advanced.push(profile.id.clone());
            }
        }
        Ok(advanced)
    }

    /// Perfil de treino que o jogador está seguindo, se houver.
    pub fn active_training_profile(&self, player: ProfileId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT training_profile FROM training_profile_active WHERE profile_id = ?1",
                [player.0],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Escolhe (ou, com `None`, larga) o Perfil de treino ativo.
    pub fn set_active_training_profile(
        &mut self,
        player: ProfileId,
        training_profile: Option<&str>,
    ) -> Result<()> {
        ensure_profile(&self.conn, player)?;
        match training_profile {
            Some(id) => self.conn.execute(
                "INSERT INTO training_profile_active (profile_id, training_profile)
                 VALUES (?1, ?2)
                 ON CONFLICT(profile_id) DO UPDATE SET training_profile = excluded.training_profile",
                params![player.0, id],
            )?,
            None => self.conn.execute(
                "DELETE FROM training_profile_active WHERE profile_id = ?1",
                [player.0],
            )?,
        };
        Ok(())
    }
}
