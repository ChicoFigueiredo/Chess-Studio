//! Partidas importadas em lote, por Origem (CX-19, #242).
//!
//! Só a tabela `imported_games` é tocada: as Partidas jogadas no app (`games`) vivem em outro
//! lugar e **nunca** entram nestas consultas. Uma importação é identificada pela primeira
//! Origem da Partida (Provedor, nome do arquivo ou da consulta e instante); uma Partida que
//! também veio de outra Origem (a mesma Partida baixada de dois lugares) **não** é apagada por
//! uma só delas, e é contada à parte para a confirmação dizer isso.

use crate::error::Result;
use crate::origin::ImportMode;
use crate::store::{ProfileId, Store, ensure_profile};
use rusqlite::{Connection, params};

/// Um conjunto de Partidas importadas de uma só vez (mesmo Provedor, nome e instante).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportGroup {
    /// Provedor da Origem (`lichess`, `chess-com`, `twic`...); `None`: arquivo sem Provedor.
    pub provider: Option<String>,
    /// Nome do arquivo ou da consulta (texto livre da Origem).
    pub source: Option<String>,
    pub mode: ImportMode,
    /// Instante da importação (ms desde a época Unix).
    pub retrieved_at_ms: i64,
    /// Partidas que esta importação trouxe e ainda estão na Biblioteca.
    pub games: usize,
}

impl ImportGroup {
    /// O alvo que apaga exatamente esta importação.
    pub fn scope(&self) -> ImportScope {
        ImportScope::Batch {
            provider: self.provider.clone(),
            source: self.source.clone(),
            retrieved_at_ms: self.retrieved_at_ms,
        }
    }
}

/// O que apagar: tudo o que veio de um Provedor ou uma importação só.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportScope {
    /// Todas as importações do Provedor (`None`: as de arquivos sem Provedor).
    Provider(Option<String>),
    /// Uma importação: Provedor, nome e instante.
    Batch {
        provider: Option<String>,
        source: Option<String>,
        retrieved_at_ms: i64,
    },
}

/// Quantas Partidas um apagamento levaria e quantas ele deixaria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BulkCount {
    /// Seriam apagadas.
    pub to_delete: usize,
    /// Ficariam porque também vieram de outra Origem.
    pub kept_other_origin: usize,
}

/// O que um apagamento fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BulkDeleted {
    pub deleted: usize,
    pub kept_other_origin: usize,
}

/// Condição SQL do alvo, sobre `imported_games` (parâmetros `?2`, `?3`, `?4`; `?1` é o perfil).
/// `IS` compara `NULL` com `NULL`, então "sem Provedor" e "sem nome" funcionam.
fn matches(scope: &ImportScope) -> &'static str {
    match scope {
        ImportScope::Provider(_) => "provider IS ?2 AND ?3 IS NULL AND ?4 IS NULL",
        ImportScope::Batch { .. } => {
            "provider IS ?2 AND source IS ?3 AND coalesce(retrieved_at_ms, imported_at_ms) = ?4"
        }
    }
}

/// Origem somada depois da primeira que **não** é a do alvo.
fn other_origin(scope: &ImportScope) -> &'static str {
    match scope {
        ImportScope::Provider(_) => "o.provider IS NOT ?2",
        ImportScope::Batch { .. } => {
            "NOT (o.provider IS ?2 AND o.source IS ?3 AND o.retrieved_at_ms = ?4)"
        }
    }
}

fn params_of(scope: &ImportScope) -> (Option<&str>, Option<&str>, Option<i64>) {
    match scope {
        ImportScope::Provider(provider) => (provider.as_deref(), None, None),
        ImportScope::Batch {
            provider,
            source,
            retrieved_at_ms,
        } => (
            provider.as_deref(),
            source.as_deref(),
            Some(*retrieved_at_ms),
        ),
    }
}

fn count(conn: &Connection, profile: ProfileId, scope: &ImportScope) -> Result<BulkCount> {
    let (provider, source, at) = params_of(scope);
    let kept: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM imported_games
             WHERE profile_id = ?1 AND {}
               AND EXISTS (SELECT 1 FROM imported_game_origins o
                           WHERE o.game_id = imported_games.id AND ({}))",
            matches(scope),
            other_origin(scope)
        ),
        params![profile.0, provider, source, at],
        |r| r.get(0),
    )?;
    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM imported_games WHERE profile_id = ?1 AND {}",
            matches(scope)
        ),
        params![profile.0, provider, source, at],
        |r| r.get(0),
    )?;
    let kept = usize::try_from(kept).unwrap_or(0);
    Ok(BulkCount {
        to_delete: usize::try_from(total).unwrap_or(0).saturating_sub(kept),
        kept_other_origin: kept,
    })
}

impl Store {
    /// As importações do Perfil local, da mais recente à mais antiga (a lista que a Biblioteca
    /// mostra para escolher o que apagar).
    pub fn import_groups(&self, profile: ProfileId) -> Result<Vec<ImportGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider, source, import_mode,
                    coalesce(retrieved_at_ms, imported_at_ms) AS at, COUNT(*)
             FROM imported_games WHERE profile_id = ?1
             GROUP BY provider, source, import_mode, at
             ORDER BY at DESC, provider, source",
        )?;
        let rows = stmt.query_map([profile.0], |r| {
            let mode: String = r.get(2)?;
            Ok(ImportGroup {
                provider: r.get(0)?,
                source: r.get(1)?,
                mode: ImportMode::parse(&mode).unwrap_or_default(),
                retrieved_at_ms: r.get(3)?,
                games: usize::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Quantas Partidas `scope` apagaria e quantas deixaria (para a confirmação).
    pub fn count_imported_in(&self, profile: ProfileId, scope: &ImportScope) -> Result<BulkCount> {
        ensure_profile(&self.conn, profile)?;
        count(&self.conn, profile, scope)
    }

    /// Apaga as Partidas **importadas** de `scope` do Perfil local, tudo ou nada. Nunca toca
    /// as Partidas jogadas no app, as de outro Perfil local, nem as que também vieram de outra
    /// Origem.
    pub fn delete_imported_in(
        &mut self,
        profile: ProfileId,
        scope: &ImportScope,
    ) -> Result<BulkDeleted> {
        ensure_profile(&self.conn, profile)?;
        let tx = self.conn.transaction()?;
        let counted = count(&tx, profile, scope)?;
        let (provider, source, at) = params_of(scope);
        let deleted = tx.execute(
            &format!(
                "DELETE FROM imported_games
                 WHERE profile_id = ?1 AND {}
                   AND NOT EXISTS (SELECT 1 FROM imported_game_origins o
                                   WHERE o.game_id = imported_games.id AND ({}))",
                matches(scope),
                other_origin(scope)
            ),
            params![profile.0, provider, source, at],
        )?;
        tx.commit()?;
        Ok(BulkDeleted {
            deleted,
            kept_other_origin: counted.kept_other_origin,
        })
    }
}
