//! Súmula manual (V2-SUM-01): um registro **separado**, por perfil local e por Partida.
//! Nada aqui escreve em `games` nem em `game_events`: a Súmula nunca altera a Partida,
//! o log de eventos nem o PGN (ADR-0001/0002). Só *lê* `games` para conferir o dono.

use crate::error::{Result, StoreError};
use crate::store::{GameId, ProfileId, Store, ensure_profile, now_ms};
use chess_training::{
    Counts, EntryCheck, HistoryPeriod, ManualScoresheet, ScoresheetCheck, ScoresheetHistory,
    Verdict,
};
use rusqlite::{OptionalExtension, params};

impl Store {
    /// A Súmula do `profile` para a Partida, se existir. Partida de outro perfil é
    /// `WrongProfile`; Partida inexistente, `GameNotFound`.
    pub fn load_scoresheet(
        &self,
        profile: ProfileId,
        game: GameId,
    ) -> Result<Option<ManualScoresheet>> {
        self.ensure_game_owner(profile, game)?;
        let head: Option<(i64, Option<i64>)> = self
            .conn
            .query_row(
                "SELECT id, delivered_at_ms FROM manual_scoresheets
                 WHERE profile_id = ?1 AND game_id = ?2",
                params![profile.0, game.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, delivered_at_ms)) = head else {
            return Ok(None);
        };
        let mut stmt = self.conn.prepare(
            "SELECT ply, text FROM manual_scoresheet_entries
             WHERE scoresheet_id = ?1 ORDER BY ply",
        )?;
        let rows = stmt.query_map([id], |r| Ok((r.get::<_, u16>(0)?, r.get::<_, String>(1)?)))?;
        let entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        ManualScoresheet::from_parts(entries, delivered_at_ms)
            .map(Some)
            .map_err(StoreError::InvalidScoresheet)
    }

    /// Grava a Súmula (cria ou substitui por inteiro, numa transação). Gravar de novo
    /// não duplica: há uma Súmula por perfil e Partida.
    pub fn save_scoresheet(
        &mut self,
        profile: ProfileId,
        game: GameId,
        sheet: &ManualScoresheet,
    ) -> Result<()> {
        self.ensure_game_owner(profile, game)?;
        let now = now_ms();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO manual_scoresheets
                 (profile_id, game_id, created_at_ms, updated_at_ms, delivered_at_ms)
             VALUES (?1, ?2, ?3, ?3, ?4)
             ON CONFLICT (profile_id, game_id) DO UPDATE SET
                 updated_at_ms = excluded.updated_at_ms,
                 delivered_at_ms = excluded.delivered_at_ms",
            params![profile.0, game.0, now, sheet.delivered_at_ms()],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM manual_scoresheets WHERE profile_id = ?1 AND game_id = ?2",
            params![profile.0, game.0],
            |r| r.get(0),
        )?;
        tx.execute(
            "DELETE FROM manual_scoresheet_entries WHERE scoresheet_id = ?1",
            [id],
        )?;
        for (ply, text) in sheet.entries() {
            tx.execute(
                "INSERT INTO manual_scoresheet_entries (scoresheet_id, ply, text)
                 VALUES (?1, ?2, ?3)",
                params![id, ply, text],
            )?;
        }
        // A conferência vale para a Súmula entregue; reaberta/regravada sem entrega, some.
        if sheet.delivered_at_ms().is_none() {
            tx.execute(
                "DELETE FROM manual_scoresheet_checks WHERE scoresheet_id = ?1",
                [id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Grava (substituindo) o resultado da conferência da Súmula entregue (V2-SUM-02). Não
    /// toca a Partida nem a Súmula. Sem Súmula entregue, `ScoresheetNotDelivered`.
    pub fn save_scoresheet_check(
        &mut self,
        profile: ProfileId,
        game: GameId,
        check: &ScoresheetCheck,
    ) -> Result<()> {
        self.ensure_game_owner(profile, game)?;
        let tx = self.conn.transaction()?;
        let id: Option<i64> = tx
            .query_row(
                "SELECT id FROM manual_scoresheets
                 WHERE profile_id = ?1 AND game_id = ?2 AND delivered_at_ms IS NOT NULL",
                params![profile.0, game.0],
                |r| r.get(0),
            )
            .optional()?;
        let id = id.ok_or(StoreError::ScoresheetNotDelivered(game))?;
        tx.execute(
            "DELETE FROM manual_scoresheet_checks WHERE scoresheet_id = ?1",
            [id],
        )?;
        tx.execute(
            "INSERT INTO manual_scoresheet_checks (scoresheet_id, checked_at_ms) VALUES (?1, ?2)",
            params![id, check.checked_at_ms()],
        )?;
        for e in check.entries() {
            tx.execute(
                "INSERT INTO manual_scoresheet_check_entries
                     (scoresheet_id, ply, written, expected, verdict)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, e.ply, e.written, e.expected, e.verdict.key()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// O resultado da conferência gravado para a Súmula do `profile`, se houver.
    pub fn load_scoresheet_check(
        &self,
        profile: ProfileId,
        game: GameId,
    ) -> Result<Option<ScoresheetCheck>> {
        self.ensure_game_owner(profile, game)?;
        let head: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT c.scoresheet_id, c.checked_at_ms
                 FROM manual_scoresheet_checks c
                 JOIN manual_scoresheets s ON s.id = c.scoresheet_id
                 WHERE s.profile_id = ?1 AND s.game_id = ?2",
                params![profile.0, game.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, checked_at_ms)) = head else {
            return Ok(None);
        };
        let mut stmt = self.conn.prepare(
            "SELECT ply, written, expected, verdict FROM manual_scoresheet_check_entries
             WHERE scoresheet_id = ?1 ORDER BY ply",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get::<_, u16>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (ply, written, expected, verdict) = row?;
            let verdict = Verdict::from_key(&verdict).ok_or_else(|| {
                StoreError::Sqlite(rusqlite::Error::InvalidColumnType(
                    3,
                    "verdict".into(),
                    rusqlite::types::Type::Text,
                ))
            })?;
            entries.push(EntryCheck {
                ply,
                written,
                expected,
                verdict,
            });
        }
        Ok(Some(ScoresheetCheck::from_parts(checked_at_ms, entries)))
    }

    /// Histórico de erros de Súmula do `profile` no `period`, a partir das conferências
    /// gravadas (V2-SUM-02): totais por classe e evolução. Só lê. O recorte usa o instante
    /// da conferência e o `now_ms` injetado (`[now - período, now]`; `All` = até `now`).
    pub fn scoresheet_history(
        &self,
        profile: ProfileId,
        period: HistoryPeriod,
        now_ms: i64,
    ) -> Result<ScoresheetHistory> {
        ensure_profile(&self.conn, profile)?;
        let since = period.since_ms(now_ms).unwrap_or(i64::MIN);
        let mut stmt = self.conn.prepare(
            "SELECT c.checked_at_ms, c.scoresheet_id, e.verdict, COUNT(*)
             FROM manual_scoresheet_checks c
             JOIN manual_scoresheets s ON s.id = c.scoresheet_id
             JOIN manual_scoresheet_check_entries e ON e.scoresheet_id = c.scoresheet_id
             WHERE s.profile_id = ?1 AND c.checked_at_ms BETWEEN ?2 AND ?3
             GROUP BY c.scoresheet_id, e.verdict
             ORDER BY c.checked_at_ms, c.scoresheet_id",
        )?;
        let rows = stmt.query_map(params![profile.0, since, now_ms], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u32>(3)?,
            ))
        })?;
        // Uma conferência = uma Súmula; junta as classes dela numa só contagem.
        let mut checks: Vec<(i64, i64, Counts)> = Vec::new();
        for row in rows {
            let (at, sheet, verdict, n) = row?;
            let n = n as usize;
            if checks.last().is_none_or(|(_, s, _)| *s != sheet) {
                checks.push((at, sheet, Counts::default()));
            }
            let Some((_, _, counts)) = checks.last_mut() else {
                continue;
            };
            match Verdict::from_key(&verdict) {
                Some(Verdict::Correct) => counts.correct += n,
                Some(Verdict::AcceptedWithCaveat) => counts.accepted += n,
                Some(Verdict::WrongNotation) => counts.wrong_notation += n,
                Some(Verdict::Swapped) => counts.swapped += n,
                Some(Verdict::Missing) => counts.missing += n,
                Some(Verdict::Extra) => counts.extra += n,
                None => {}
            }
        }
        Ok(ScoresheetHistory::from_counts(
            period,
            now_ms,
            checks.into_iter().map(|(at, _, c)| (at, c)),
        ))
    }

    /// Apaga a Súmula (não mexe na Partida). Sem Súmula, não faz nada.
    pub fn delete_scoresheet(&mut self, profile: ProfileId, game: GameId) -> Result<()> {
        self.ensure_game_owner(profile, game)?;
        self.conn.execute(
            "DELETE FROM manual_scoresheets WHERE profile_id = ?1 AND game_id = ?2",
            params![profile.0, game.0],
        )?;
        Ok(())
    }

    fn ensure_game_owner(&self, profile: ProfileId, game: GameId) -> Result<()> {
        ensure_profile(&self.conn, profile)?;
        let owner: Option<i64> = self
            .conn
            .query_row(
                "SELECT profile_id FROM games WHERE id = ?1",
                [game.0],
                |r| r.get(0),
            )
            .optional()?;
        match owner {
            None => Err(StoreError::GameNotFound(game)),
            Some(o) if o != profile.0 => Err(StoreError::WrongProfile { profile }),
            Some(_) => Ok(()),
        }
    }
}
