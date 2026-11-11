//! Banco próprio da rede (SQLite): consentimento e cache por Perfil local. Fica num arquivo
//! separado do banco das Partidas: o cache é descartável (apagar o arquivo não perde nada
//! além do consentimento) e as gravações das threads de rede não disputam o log de Eventos.

use crate::LocalProfileId;
use crate::catalog::{CATALOG_VERSION, ProviderId};
use crate::clock::Timestamp;
use crate::error::NetError;
use crate::response::ImportMode;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::time::Duration;

const SCHEMA_VERSION: i64 = 1;

const V1: &str = "
CREATE TABLE consents (
    profile_id      INTEGER NOT NULL,
    provider        TEXT NOT NULL,
    enabled         INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    changed_at_ms   INTEGER NOT NULL,
    catalog_version INTEGER NOT NULL,
    PRIMARY KEY (profile_id, provider)
) WITHOUT ROWID;

CREATE TABLE cache (
    id             INTEGER PRIMARY KEY,
    profile_id     INTEGER NOT NULL,
    cache_key      TEXT NOT NULL,
    provider       TEXT NOT NULL,
    source_url     TEXT NOT NULL,
    status         INTEGER NOT NULL,
    content_type   TEXT,
    body           BLOB NOT NULL,
    queried_at_ms  INTEGER NOT NULL,
    expires_at_ms  INTEGER NOT NULL,
    etag           TEXT,
    last_modified  TEXT,
    license        TEXT NOT NULL,
    mode           TEXT NOT NULL,
    UNIQUE (profile_id, cache_key)
);
CREATE INDEX cache_by_provider ON cache (profile_id, provider);
";

/// Entrada do cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheEntry {
    pub provider: ProviderId,
    pub source_url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub queried_at: Timestamp,
    pub expires_at: Timestamp,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub license: String,
    pub mode: ImportMode,
}

impl CacheEntry {
    pub fn is_fresh(&self, now: Timestamp) -> bool {
        now < self.expires_at
    }
}

#[derive(Debug)]
pub(crate) struct NetStore {
    conn: Connection,
}

fn db(err: rusqlite::Error) -> NetError {
    NetError::storage(err)
}

fn mode_key(mode: ImportMode) -> &'static str {
    match mode {
        ImportMode::Api => "api",
        ImportMode::Download => "download",
        ImportMode::ManualImport => "manual_import",
    }
}

fn mode_from(key: &str) -> ImportMode {
    match key {
        "download" => ImportMode::Download,
        "manual_import" => ImportMode::ManualImport,
        _ => ImportMode::Api,
    }
}

impl NetStore {
    pub fn open(path: &Path) -> Result<Self, NetError> {
        Self::init(Connection::open(path).map_err(db)?)
    }

    pub fn open_in_memory() -> Result<Self, NetError> {
        Self::init(Connection::open_in_memory().map_err(db)?)
    }

    fn init(mut conn: Connection) -> Result<Self, NetError> {
        conn.busy_timeout(Duration::from_secs(5)).map_err(db)?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
            .map_err(db)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db)?;
        if version > SCHEMA_VERSION {
            return Err(NetError::Storage {
                detail: format!("banco da rede na versão {version}, mais nova que o app"),
            });
        }
        if version < 1 {
            let tx = conn.transaction().map_err(db)?;
            tx.execute_batch(V1).map_err(db)?;
            tx.pragma_update(None, "user_version", 1).map_err(db)?;
            tx.commit().map_err(db)?;
        }
        Ok(Self { conn })
    }

    /// Todos os consentimentos ligados, de todos os Perfis locais.
    pub fn enabled_consents(&self) -> Result<Vec<(LocalProfileId, ProviderId)>, NetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT profile_id, provider FROM consents WHERE enabled = 1")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(db)?;
        let mut out = Vec::new();
        for row in rows {
            let (profile, key) = row.map_err(db)?;
            // Provedor de uma versão futura do catálogo: ignorado.
            if let Some(provider) = ProviderId::from_key(&key) {
                out.push((LocalProfileId(profile), provider));
            }
        }
        Ok(out)
    }

    pub fn set_consent(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        enabled: bool,
        at: Timestamp,
    ) -> Result<(), NetError> {
        self.conn
            .execute(
                "INSERT INTO consents (profile_id, provider, enabled, changed_at_ms, catalog_version)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (profile_id, provider) DO UPDATE SET
                    enabled = excluded.enabled,
                    changed_at_ms = excluded.changed_at_ms,
                    catalog_version = excluded.catalog_version",
                params![
                    profile.0,
                    provider.key(),
                    enabled,
                    at.as_millis(),
                    CATALOG_VERSION
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn get(&self, profile: LocalProfileId, key: &str) -> Result<Option<CacheEntry>, NetError> {
        self.conn
            .query_row(
                "SELECT provider, source_url, status, content_type, body, queried_at_ms,
                        expires_at_ms, etag, last_modified, license, mode
                 FROM cache WHERE profile_id = ?1 AND cache_key = ?2",
                params![profile.0, key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        CacheEntry {
                            provider: ProviderId::Lichess,
                            source_url: r.get(1)?,
                            status: r.get(2)?,
                            content_type: r.get(3)?,
                            body: r.get(4)?,
                            queried_at: Timestamp::from_millis(r.get(5)?),
                            expires_at: Timestamp::from_millis(r.get(6)?),
                            etag: r.get(7)?,
                            last_modified: r.get(8)?,
                            license: r.get(9)?,
                            mode: mode_from(&r.get::<_, String>(10)?),
                        },
                    ))
                },
            )
            .optional()
            .map_err(db)
            .map(|row| {
                row.and_then(|(provider, mut entry)| {
                    entry.provider = ProviderId::from_key(&provider)?;
                    Some(entry)
                })
            })
    }

    pub fn put(
        &self,
        profile: LocalProfileId,
        key: &str,
        entry: &CacheEntry,
    ) -> Result<(), NetError> {
        self.conn
            .execute(
                "INSERT INTO cache (profile_id, cache_key, provider, source_url, status,
                    content_type, body, queried_at_ms, expires_at_ms, etag, last_modified,
                    license, mode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT (profile_id, cache_key) DO UPDATE SET
                    provider = excluded.provider, source_url = excluded.source_url,
                    status = excluded.status, content_type = excluded.content_type,
                    body = excluded.body, queried_at_ms = excluded.queried_at_ms,
                    expires_at_ms = excluded.expires_at_ms, etag = excluded.etag,
                    last_modified = excluded.last_modified, license = excluded.license,
                    mode = excluded.mode",
                params![
                    profile.0,
                    key,
                    entry.provider.key(),
                    entry.source_url,
                    entry.status,
                    entry.content_type,
                    entry.body,
                    entry.queried_at.as_millis(),
                    entry.expires_at.as_millis(),
                    entry.etag,
                    entry.last_modified,
                    entry.license,
                    mode_key(entry.mode),
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    /// `304`: o conteúdo segue valendo; renova o instante da consulta e a validade.
    pub fn renew(
        &self,
        profile: LocalProfileId,
        key: &str,
        queried_at: Timestamp,
        expires_at: Timestamp,
        etag: Option<&str>,
    ) -> Result<(), NetError> {
        self.conn
            .execute(
                "UPDATE cache SET queried_at_ms = ?3, expires_at_ms = ?4, etag = COALESCE(?5, etag)
                 WHERE profile_id = ?1 AND cache_key = ?2",
                params![
                    profile.0,
                    key,
                    queried_at.as_millis(),
                    expires_at.as_millis(),
                    etag
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn clear_provider(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
    ) -> Result<usize, NetError> {
        self.conn
            .execute(
                "DELETE FROM cache WHERE profile_id = ?1 AND provider = ?2",
                params![profile.0, provider.key()],
            )
            .map_err(db)
    }

    /// Perfil local excluído: some o consentimento e o cache dele.
    pub fn forget_profile(&self, profile: LocalProfileId) -> Result<(), NetError> {
        self.conn
            .execute("DELETE FROM consents WHERE profile_id = ?1", [profile.0])
            .map_err(db)?;
        self.conn
            .execute("DELETE FROM cache WHERE profile_id = ?1", [profile.0])
            .map_err(db)?;
        Ok(())
    }
}
