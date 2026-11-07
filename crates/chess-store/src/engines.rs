//! Engines conhecidas (UCI-05): instaladas pelo app (`catalog`), detectadas no computador
//! (`detected`) ou escolhidas à mão (`manual`), com a configuração de cada uma. Nunca o
//! binário no SQLite. Só registros `catalog` com `install_dir` podem levar à remoção de
//! arquivos, e mesmo assim o gerenciador confere o marcador de instalação (PEND-08).

use crate::error::{Result, StoreError};
use crate::store::{Store, now_ms};
use rusqlite::{OptionalExtension, Row, params};
use std::collections::BTreeMap;

/// Identificador estável de uma Engine registrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EngineId(pub i64);

/// De onde veio a Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineOrigin {
    /// Baixada do catálogo pelo app.
    Catalog,
    /// Encontrada no `PATH` ou em locais comuns (não é do app: nunca apagada).
    Detected,
    /// Executável escolhido pelo usuário (não é do app: nunca apagado).
    Manual,
}

impl EngineOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Detected => "detected",
            Self::Manual => "manual",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "catalog" => Self::Catalog,
            "detected" => Self::Detected,
            _ => Self::Manual,
        }
    }
}

/// Uma Engine registrada e sua configuração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRecord {
    pub name: String,
    pub version: Option<String>,
    /// Caminho do executável (único por registro).
    pub path: String,
    pub protocol: String,
    /// SHA-256 verificado do download, quando o app a instalou.
    pub checksum: Option<String>,
    /// `ready`, `missing`, `unknown`…
    pub status: String,
    pub origin: EngineOrigin,
    /// Identificador no catálogo (`stockfish`, `lc0`).
    pub catalog_id: Option<String>,
    /// Variante instalada (`cpu-openblas`…; vazio quando só há uma).
    pub variant: Option<String>,
    /// Pasta instalada pelo app em `<dados>/engines/`; `None` para Engines do usuário.
    pub install_dir: Option<String>,
    /// Opções UCI escolhidas (`Threads`, `Hash`, `UCI_Elo`…).
    pub settings: BTreeMap<String, String>,
    pub installed_at_ms: i64,
}

const COLUMNS: &str = "id, name, version, path, protocol, checksum, status, origin, catalog_id, \
                       variant, install_dir, settings_json, installed_at_ms";

fn read_row(row: &Row<'_>) -> rusqlite::Result<(EngineId, EngineRecord, String)> {
    let origin: String = row.get(7)?;
    let settings_json: String = row.get(11)?;
    Ok((
        EngineId(row.get(0)?),
        EngineRecord {
            name: row.get(1)?,
            version: row.get(2)?,
            path: row.get(3)?,
            protocol: row.get(4)?,
            checksum: row.get(5)?,
            status: row.get(6)?,
            origin: EngineOrigin::parse(&origin),
            catalog_id: row.get(8)?,
            variant: row.get(9)?,
            install_dir: row.get(10)?,
            settings: BTreeMap::new(),
            installed_at_ms: row.get(12)?,
        },
        settings_json,
    ))
}

fn finish(parts: (EngineId, EngineRecord, String)) -> Result<(EngineId, EngineRecord)> {
    let (id, mut record, json) = parts;
    record.settings = serde_json::from_str(&json)?;
    Ok((id, record))
}

impl Store {
    /// Registra a Engine ou, se o caminho já existe, atualiza o registro (mantendo o id e,
    /// se `record.settings` estiver vazio, a configuração já salva).
    pub fn upsert_engine(&self, record: &EngineRecord) -> Result<EngineId> {
        let settings = serde_json::to_string(&record.settings)?;
        let now = if record.installed_at_ms == 0 {
            now_ms()
        } else {
            record.installed_at_ms
        };
        self.conn.execute(
            "INSERT INTO engine_installations
                (name, version, path, protocol, checksum, status, origin, catalog_id, variant,
                 install_dir, settings_json, installed_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(path) DO UPDATE SET
                name = excluded.name, version = excluded.version, protocol = excluded.protocol,
                checksum = excluded.checksum, status = excluded.status, origin = excluded.origin,
                catalog_id = excluded.catalog_id, variant = excluded.variant,
                install_dir = excluded.install_dir, installed_at_ms = excluded.installed_at_ms,
                settings_json = CASE WHEN excluded.settings_json = '{}'
                                     THEN engine_installations.settings_json
                                     ELSE excluded.settings_json END",
            params![
                record.name,
                record.version,
                record.path,
                record.protocol,
                record.checksum,
                record.status,
                record.origin.as_str(),
                record.catalog_id,
                record.variant,
                record.install_dir,
                settings,
                now,
            ],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM engine_installations WHERE path = ?1",
            [&record.path],
            |r| r.get(0),
        )?;
        Ok(EngineId(id))
    }

    /// Todas as Engines registradas, da mais antiga à mais nova.
    pub fn list_engines(&self) -> Result<Vec<(EngineId, EngineRecord)>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM engine_installations ORDER BY id"))?;
        let rows = stmt
            .query_map([], read_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().map(finish).collect()
    }

    pub fn get_engine(&self, id: EngineId) -> Result<Option<EngineRecord>> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM engine_installations WHERE id = ?1"),
                [id.0],
                read_row,
            )
            .optional()?;
        row.map(|r| finish(r).map(|(_, record)| record)).transpose()
    }

    /// Engine registrada para um executável, se houver.
    pub fn find_engine_by_path(&self, path: &str) -> Result<Option<(EngineId, EngineRecord)>> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM engine_installations WHERE path = ?1"),
                [path],
                read_row,
            )
            .optional()?;
        row.map(finish).transpose()
    }

    /// Grava a configuração (substitui a anterior).
    pub fn set_engine_settings(
        &self,
        id: EngineId,
        settings: &BTreeMap<String, String>,
    ) -> Result<()> {
        let json = serde_json::to_string(settings)?;
        let changed = self.conn.execute(
            "UPDATE engine_installations SET settings_json = ?2 WHERE id = ?1",
            params![id.0, json],
        )?;
        if changed == 0 {
            return Err(StoreError::EngineNotFound(id));
        }
        Ok(())
    }

    pub fn set_engine_status(&self, id: EngineId, status: &str) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE engine_installations SET status = ?2 WHERE id = ?1",
            params![id.0, status],
        )?;
        if changed == 0 {
            return Err(StoreError::EngineNotFound(id));
        }
        Ok(())
    }

    /// Remove o **registro**; apagar arquivos é do gerenciador (e só do que o app instalou).
    pub fn remove_engine(&self, id: EngineId) -> Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM engine_installations WHERE id = ?1", [id.0])?;
        if changed == 0 {
            return Err(StoreError::EngineNotFound(id));
        }
        Ok(())
    }
}
