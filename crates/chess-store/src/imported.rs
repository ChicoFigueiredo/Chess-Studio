//! Partidas importadas de PGN (#48): a árvore de variações (com comentários e
//! NAGs) fica em JSON na tabela `imported_games`, com colunas de listagem. É a
//! base da biblioteca (C7). Não toca `games`/`game_events`: o log de Eventos
//! continua sendo a verdade das Partidas jogadas aqui (ADR-0002).

use crate::error::{Result, StoreError};
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use chess_pgn::{PgnError, PgnGame};
use rusqlite::{OptionalExtension, params};

/// Identificador de uma partida importada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImportedId(pub i64);

/// Resultado de [`Store::import_pgn`]: as partidas gravadas e os erros das que
/// foram puladas (cada erro tem partida, linha, coluna e o Lance problemático).
#[derive(Debug)]
pub struct ImportReport {
    pub imported: Vec<ImportedId>,
    pub errors: Vec<PgnError>,
}

/// Filtro de [`Store::list_imported`]; o padrão lista tudo, da mais recente à mais antiga.
/// `profile: None` lista de todos os perfis (testes/administração): a interface sempre passa o ativo.
#[derive(Debug, Clone, Default)]
pub struct ImportedQuery {
    /// Trecho (sem distinguir maiúsculas) de evento, brancas ou pretas.
    pub text: Option<String>,
    pub profile: Option<ProfileId>,
    pub limit: Option<u32>,
}

/// Linha de listagem de uma partida importada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSummary {
    pub id: ImportedId,
    pub profile: ProfileId,
    /// Origem informada na importação (por exemplo, o nome do arquivo).
    pub source: Option<String>,
    pub event: Option<String>,
    pub white: Option<String>,
    pub black: Option<String>,
    pub date: Option<String>,
    /// `1-0`, `0-1`, `1/2-1/2` ou `*`.
    pub result: String,
    pub start_fen: String,
    /// Meios-lances em todas as linhas (principal e variações).
    pub ply_count: u32,
    pub variation_count: u32,
    pub imported_at_ms: i64,
}

/// Versão do formato JSON da árvore (para migrações futuras do modelo).
const TREE_VERSION: i64 = 1;

impl Store {
    /// Importa as partidas do PGN no perfil padrão. Partidas com erro são puladas e
    /// relatadas; as válidas são gravadas numa única transação.
    pub fn import_pgn(&mut self, pgn: &str, source: Option<&str>) -> Result<ImportReport> {
        let profile = self.default_profile()?;
        self.import_pgn_for(profile, pgn, source)
    }

    pub fn import_pgn_for(
        &mut self,
        profile: ProfileId,
        pgn: &str,
        source: Option<&str>,
    ) -> Result<ImportReport> {
        let parsed = chess_pgn::parse_pgn_lenient(pgn);
        let now = now_ms();
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        let mut imported = Vec::with_capacity(parsed.games.len());
        {
            let mut insert = tx.prepare(
                "INSERT INTO imported_games (profile_id, source, event, white, black, date, result,
                                             start_fen, ply_count, variation_count, tree_version,
                                             tree_json, imported_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;
            for game in &parsed.games {
                let known = |name: &str| game.tag(name).filter(|v| *v != "?").map(str::to_owned);
                insert.execute(params![
                    profile.0,
                    source,
                    known("Event"),
                    known("White"),
                    known("Black"),
                    game.tag("Date").filter(|v| !v.starts_with('?')),
                    game.result.token(),
                    game.start_fen,
                    i64::try_from(game.total_plies()).unwrap_or(i64::MAX),
                    i64::try_from(game.variation_count()).unwrap_or(i64::MAX),
                    TREE_VERSION,
                    serde_json::to_string(game)?,
                    now,
                ])?;
                imported.push(ImportedId(tx.last_insert_rowid()));
            }
        }
        tx.commit()?;
        Ok(ImportReport {
            imported,
            errors: parsed.errors,
        })
    }

    /// Partidas importadas, da mais recente à mais antiga.
    pub fn list_imported(&self, query: &ImportedQuery) -> Result<Vec<ImportedSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile_id, source, event, white, black, date, result, start_fen,
                    ply_count, variation_count, imported_at_ms
             FROM imported_games
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND (?2 IS NULL OR instr(lower(coalesce(event, '') || char(9) || coalesce(white, '')
                                              || char(9) || coalesce(black, '')), lower(?2)) > 0)
             ORDER BY imported_at_ms DESC, id DESC
             LIMIT ?3",
        )?;
        let limit = query.limit.map_or(-1, i64::from);
        let rows = stmt.query_map(
            params![query.profile.map(|p| p.0), query.text, limit],
            |r| {
                Ok(ImportedSummary {
                    id: ImportedId(r.get(0)?),
                    profile: ProfileId(r.get(1)?),
                    source: r.get(2)?,
                    event: r.get(3)?,
                    white: r.get(4)?,
                    black: r.get(5)?,
                    date: r.get(6)?,
                    result: r.get(7)?,
                    start_fen: r.get(8)?,
                    ply_count: r.get(9)?,
                    variation_count: r.get(10)?,
                    imported_at_ms: r.get(11)?,
                })
            },
        )?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// A árvore completa (linha principal, variações, comentários e NAGs) de uma partida
    /// do `profile`; `WrongProfile` se ela é de outro perfil.
    pub fn load_imported(&self, profile: ProfileId, id: ImportedId) -> Result<PgnGame> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT profile_id, tree_json FROM imported_games WHERE id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (owner, json) = row.ok_or(StoreError::ImportedGameNotFound(id))?;
        if owner != profile.0 {
            return Err(StoreError::WrongProfile { profile });
        }
        Ok(serde_json::from_str(&json)?)
    }

    /// Leitura sem filtro de perfil, só para o índice interno da Biblioteca.
    pub(crate) fn load_imported_any(&self, id: ImportedId) -> Result<PgnGame> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT tree_json FROM imported_games WHERE id = ?1",
                [id.0],
                |r| r.get(0),
            )
            .optional()?;
        let json = json.ok_or(StoreError::ImportedGameNotFound(id))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn delete_imported(&self, profile: ProfileId, id: ImportedId) -> Result<()> {
        self.load_imported(profile, id)?;
        self.conn.execute(
            "DELETE FROM imported_games WHERE id = ?1 AND profile_id = ?2",
            params![id.0, profile.0],
        )?;
        Ok(())
    }
}
