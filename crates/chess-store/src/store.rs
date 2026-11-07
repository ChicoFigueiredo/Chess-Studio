//! O repositório: módulo profundo sobre uma conexão SQLite.

use crate::error::{Result, StoreError};
use crate::migrations;
use chess_game::{Event, EventKind, Game, GameSetup, Outcome, PAYLOAD_VERSION};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Identificador estável de uma Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameId(pub i64);

/// Identificador de um perfil local (DEC-50: um no P0; o esquema suporta vários).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileId(pub i64);

/// Tamanho máximo do nome de um perfil local, em caracteres.
pub const MAX_PROFILE_NAME_LEN: usize = 40;

/// Um perfil local (US-29): nome e quando foi criado/usado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    pub id: ProfileId,
    pub name: String,
    pub created_at_ms: i64,
    pub last_used_at_ms: i64,
}

fn clean_profile_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_PROFILE_NAME_LEN {
        return Err(StoreError::InvalidProfileName);
    }
    Ok(name.to_owned())
}

fn is_unique_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Metadados livres de uma Partida (listagem e exibição; não afetam as regras).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameMetadata {
    pub title: Option<String>,
    /// JSON extra (nomes dos jogadores, Engine usada, etc.). `null` vale `{}`.
    pub extra: Value,
}

/// Filtro de [`Store::list_games`]; o padrão lista tudo, da mais recente à mais antiga.
#[derive(Debug, Clone, Default)]
pub struct GameQuery {
    pub only_unfinished: bool,
    pub profile: Option<ProfileId>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSummary {
    pub id: GameId,
    pub profile: ProfileId,
    pub title: Option<String>,
    pub start_fen: String,
    pub metadata: Value,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub event_count: u64,
    pub finished: bool,
    /// Resultado e motivo, quando a Partida terminou (AC-07).
    pub outcome: Option<Outcome>,
    /// `true` se a última sessão não foi fechada de forma limpa.
    pub abrupt_close: bool,
    pub paused: bool,
}

/// Partida reaberta para retomada.
#[derive(Debug)]
pub struct OpenedGame {
    pub id: GameId,
    pub game: Game,
    /// O processo anterior morreu sem [`Store::close_game`].
    pub abrupt_close: bool,
    /// Sempre `true`: a Partida reabre pausada (DEC-42); veja [`Store::resume_game`].
    pub paused: bool,
}

/// Valores efetivos dos PRAGMAs (diagnóstico e testes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pragmas {
    pub journal_mode: String,
    pub foreign_keys: bool,
    /// 0 OFF, 1 NORMAL, 2 FULL, 3 EXTRA.
    pub synchronous: i64,
    pub busy_timeout_ms: i64,
}

#[derive(Debug)]
pub struct Store {
    pub(crate) conn: Connection,
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

pub(crate) fn ensure_profile(conn: &Connection, profile: ProfileId) -> Result<()> {
    let ok = conn
        .query_row("SELECT 1 FROM profiles WHERE id = ?1", [profile.0], |_| {
            Ok(())
        })
        .optional()?
        .is_some();
    ok.then_some(()).ok_or(StoreError::ProfileNotFound(profile))
}

impl Store {
    /// Abre (criando, se preciso) o banco em `path`, aplica os PRAGMAs e as migrações.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Banco em memória (testes e modo sem persistência).
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        // WAL: leitores não bloqueiam o escritor e a queda nunca corrompe o arquivo.
        // (Em memória o SQLite mantém o modo `memory`; a linha devolvida é ignorada.)
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        // FULL: o commit só retorna após o fsync do WAL. Com NORMAL uma queda de energia
        // poderia perder o último Lance já confirmado ao jogador; o volume de escrita
        // aqui é de poucos bytes por Lance, então o custo de FULL é irrelevante.
        conn.pragma_update(None, "synchronous", "FULL")?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<u32> {
        migrations::current_version(&self.conn)
    }

    pub fn pragmas(&self) -> Result<Pragmas> {
        let q = |name: &str| -> Result<rusqlite::types::Value> {
            Ok(self
                .conn
                .query_row(&format!("PRAGMA {name}"), [], |r| r.get(0))?)
        };
        let int = |v: rusqlite::types::Value| match v {
            rusqlite::types::Value::Integer(i) => i,
            _ => -1,
        };
        let journal_mode = match q("journal_mode")? {
            rusqlite::types::Value::Text(t) => t,
            _ => String::new(),
        };
        Ok(Pragmas {
            journal_mode,
            foreign_keys: int(q("foreign_keys")?) == 1,
            synchronous: int(q("synchronous")?),
            busy_timeout_ms: int(q("busy_timeout")?),
        })
    }

    // ---- Partidas -------------------------------------------------------------

    /// Cria a Partida (sessão aberta, log vazio) no perfil padrão.
    pub fn create_game(&mut self, setup: &GameSetup, metadata: &GameMetadata) -> Result<GameId> {
        let profile = self.default_profile()?;
        self.create_game_for(profile, setup, metadata)
    }

    pub fn create_game_for(
        &mut self,
        profile: ProfileId,
        setup: &GameSetup,
        metadata: &GameMetadata,
    ) -> Result<GameId> {
        let setup_json = serde_json::to_string(setup)?;
        let policy = serde_json::to_value(setup.policy())?
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        let extra = if metadata.extra.is_null() {
            Value::Object(Default::default())
        } else {
            metadata.extra.clone()
        };
        let now = now_ms();
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        tx.execute(
            "INSERT INTO games (profile_id, policy, start_fen, setup_json, title, metadata_json,
                                created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                profile.0,
                policy,
                setup.start().to_fen(),
                setup_json,
                metadata.title,
                extra.to_string(),
                now
            ],
        )?;
        let id = GameId(tx.last_insert_rowid());
        tx.commit()?;
        Ok(id)
    }

    /// Acrescenta `events` ao log da Partida numa **única transação**: ou o lote
    /// inteiro fica gravado, em ordem de sequência contígua, ou nada fica.
    pub fn append_events(&mut self, game: GameId, events: &[Event]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        // Valida e serializa antes de abrir a transação de escrita.
        let mut rows = Vec::with_capacity(events.len());
        let mut outcome = None;
        for (i, event) in events.iter().enumerate() {
            if event.version == 0 || event.version > PAYLOAD_VERSION {
                return Err(StoreError::UnsupportedEventVersion {
                    game,
                    seq: i as u64,
                    version: event.version,
                });
            }
            let value = serde_json::to_value(event)?;
            let kind = value["kind"]["type"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            if let EventKind::GameEnded { outcome: o } = &event.kind {
                outcome = Some(serde_json::to_string(o)?);
            }
            rows.push((event.version, kind, value.to_string()));
        }

        // IMMEDIATE: pega o lock de escrita já no início (sem upgrade de leitura -> deadlock).
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists = tx
            .query_row("SELECT 1 FROM games WHERE id = ?1", [game.0], |_| Ok(()))
            .optional()?
            .is_some();
        if !exists {
            return Err(StoreError::GameNotFound(game));
        }
        let next: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq) + 1, 0) FROM game_events WHERE game_id = ?1",
            [game.0],
            |r| r.get(0),
        )?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO game_events (game_id, seq, version, kind, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (offset, (version, kind, payload)) in rows.iter().enumerate() {
                insert.execute(params![
                    game.0,
                    next + offset as i64,
                    version,
                    kind,
                    payload
                ])?;
            }
        }
        tx.execute(
            "UPDATE games SET updated_at_ms = ?2,
                    finished = CASE WHEN ?3 IS NULL THEN finished ELSE 1 END,
                    outcome_json = COALESCE(?3, outcome_json)
             WHERE id = ?1",
            params![game.0, now_ms(), outcome],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Lê o log e reconstrói a Partida com [`Game::try_replay`].
    pub fn load_game(&self, game: GameId) -> Result<Game> {
        let setup_json: String = self
            .conn
            .query_row(
                "SELECT setup_json FROM games WHERE id = ?1",
                [game.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::GameNotFound(game))?;
        let setup: GameSetup = serde_json::from_str(&setup_json)?;
        let events = self.read_events(game)?;
        Game::try_replay(setup, &events).map_err(|source| StoreError::Replay { game, source })
    }

    fn read_events(&self, game: GameId) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT seq, version, payload_json FROM game_events WHERE game_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map([game.0], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut events = Vec::new();
        for (expected, row) in rows.enumerate() {
            let (seq, version, payload) = row?;
            if seq != expected as i64 {
                return Err(StoreError::CorruptLog {
                    game,
                    detail: format!("esperava a sequência {expected}, achou {seq}"),
                });
            }
            let version = u16::try_from(version).unwrap_or(u16::MAX);
            if version == 0 || version > PAYLOAD_VERSION {
                return Err(StoreError::UnsupportedEventVersion {
                    game,
                    seq: seq as u64,
                    version,
                });
            }
            events.push(serde_json::from_str(&payload)?);
        }
        Ok(events)
    }

    /// Reabre a Partida para retomada: marca a sessão como aberta e devolve a
    /// Partida **pausada**, dizendo se o fechamento anterior foi abrupto.
    pub fn open_game(&self, game: GameId) -> Result<OpenedGame> {
        let loaded = self.load_game(game)?;
        let was_open: bool = self.conn.query_row(
            "SELECT session_open FROM games WHERE id = ?1",
            [game.0],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "UPDATE games SET session_open = 1, paused = 1 WHERE id = ?1",
            [game.0],
        )?;
        Ok(OpenedGame {
            id: game,
            game: loaded,
            abrupt_close: was_open,
            paused: true,
        })
    }

    /// Como [`Self::open_game`], mas só se a Partida for do `profile` (`WrongProfile` senão).
    pub fn open_game_for(&self, profile: ProfileId, game: GameId) -> Result<OpenedGame> {
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
            Some(_) => self.open_game(game),
        }
    }

    /// Fechamento limpo (janela fechada normalmente): a Partida fica pausada.
    pub fn close_game(&self, game: GameId) -> Result<()> {
        self.update_flags(game, "session_open = 0, paused = 1")
    }

    /// O jogador retomou a Partida reaberta (limpa a marca de Pausa).
    pub fn resume_game(&self, game: GameId) -> Result<()> {
        self.update_flags(game, "paused = 0")
    }

    fn update_flags(&self, game: GameId, set: &str) -> Result<()> {
        let n = self.conn.execute(
            &format!("UPDATE games SET {set}, updated_at_ms = ?2 WHERE id = ?1"),
            params![game.0, now_ms()],
        )?;
        if n == 0 {
            return Err(StoreError::GameNotFound(game));
        }
        Ok(())
    }

    pub fn list_games(&self, query: &GameQuery) -> Result<Vec<GameSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.profile_id, g.title, g.start_fen, g.metadata_json, g.created_at_ms,
                    g.updated_at_ms, (SELECT COUNT(*) FROM game_events e WHERE e.game_id = g.id),
                    g.finished, g.outcome_json, g.session_open, g.paused
             FROM games g
             WHERE (?1 = 0 OR g.finished = 0) AND (?2 IS NULL OR g.profile_id = ?2)
             ORDER BY g.updated_at_ms DESC, g.id DESC
             LIMIT ?3",
        )?;
        let limit = query.limit.map_or(-1, i64::from);
        let rows = stmt.query_map(
            params![query.only_unfinished, query.profile.map(|p| p.0), limit],
            |r| {
                Ok((
                    GameSummary {
                        id: GameId(r.get(0)?),
                        profile: ProfileId(r.get(1)?),
                        title: r.get(2)?,
                        start_fen: r.get(3)?,
                        metadata: Value::Null,
                        created_at_ms: r.get(5)?,
                        updated_at_ms: r.get(6)?,
                        event_count: r.get::<_, i64>(7)? as u64,
                        finished: r.get(8)?,
                        outcome: None,
                        abrupt_close: r.get(10)?,
                        paused: r.get(11)?,
                    },
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            let (mut summary, metadata, outcome) = row?;
            summary.metadata = serde_json::from_str(&metadata)?;
            summary.outcome = outcome.map(|o| serde_json::from_str(&o)).transpose()?;
            out.push(summary);
        }
        Ok(out)
    }

    /// A Partida não terminada mais recente (para "continuar de onde parou").
    pub fn latest_unfinished_game_for(&self, profile: ProfileId) -> Result<Option<GameId>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM games WHERE finished = 0 AND profile_id = ?1
                 ORDER BY updated_at_ms DESC, id DESC LIMIT 1",
                [profile.0],
                |r| r.get(0).map(GameId),
            )
            .optional()?)
    }

    /// A Partida não terminada mais recente de qualquer perfil.
    pub fn latest_unfinished_game(&self) -> Result<Option<GameId>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM games WHERE finished = 0 ORDER BY updated_at_ms DESC, id DESC LIMIT 1",
                [],
                |r| r.get(0).map(GameId),
            )
            .optional()?)
    }

    // ---- Perfis e preferências ---------------------------------------------------

    /// O perfil em que as operações sem perfil explícito (nova Partida, importação) gravam:
    /// o ativo.
    pub fn default_profile(&self) -> Result<ProfileId> {
        self.active_profile()
    }

    /// O perfil usado por último (empate: o mais antigo). Sempre existe um.
    pub fn active_profile(&self) -> Result<ProfileId> {
        Ok(self.conn.query_row(
            "SELECT id FROM profiles ORDER BY last_used_at_ms DESC, id ASC LIMIT 1",
            [],
            |r| r.get(0).map(ProfileId),
        )?)
    }

    /// Torna `profile` o ativo; vale também na próxima abertura do app.
    pub fn set_active_profile(&self, profile: ProfileId) -> Result<()> {
        ensure_profile(&self.conn, profile)?;
        // Sempre acima de todos os outros, mesmo dentro do mesmo milissegundo.
        self.conn.execute(
            "UPDATE profiles SET last_used_at_ms =
                 MAX(?2, (SELECT MAX(last_used_at_ms) FROM profiles) + 1)
             WHERE id = ?1",
            params![profile.0, now_ms()],
        )?;
        Ok(())
    }

    fn profile_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileInfo> {
        Ok(ProfileInfo {
            id: ProfileId(r.get(0)?),
            name: r.get(1)?,
            created_at_ms: r.get(2)?,
            last_used_at_ms: r.get(3)?,
        })
    }

    /// Todos os perfis locais, na ordem de criação.
    pub fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at_ms, last_used_at_ms FROM profiles ORDER BY id")?;
        let rows = stmt.query_map([], Self::profile_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn profile(&self, profile: ProfileId) -> Result<ProfileInfo> {
        self.conn
            .query_row(
                "SELECT id, name, created_at_ms, last_used_at_ms FROM profiles WHERE id = ?1",
                [profile.0],
                Self::profile_from_row,
            )
            .optional()?
            .ok_or(StoreError::ProfileNotFound(profile))
    }

    /// Cria um perfil local (nome sem espaços nas pontas, de 1 a 40 caracteres, único).
    pub fn create_profile(&self, name: &str) -> Result<ProfileId> {
        let name = clean_profile_name(name)?;
        match self.conn.execute(
            "INSERT INTO profiles (name, created_at_ms) VALUES (?1, ?2)",
            params![name, now_ms()],
        ) {
            Ok(_) => Ok(ProfileId(self.conn.last_insert_rowid())),
            Err(e) if is_unique_violation(&e) => Err(StoreError::ProfileNameTaken(name)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn rename_profile(&self, profile: ProfileId, name: &str) -> Result<()> {
        ensure_profile(&self.conn, profile)?;
        let name = clean_profile_name(name)?;
        match self.conn.execute(
            "UPDATE profiles SET name = ?2 WHERE id = ?1",
            params![profile.0, name],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => Err(StoreError::ProfileNameTaken(name)),
            Err(e) => Err(e.into()),
        }
    }

    /// Exclui o perfil e tudo que é dele (preferências, Partidas, Resoluções, importadas).
    /// O último perfil não pode ser excluído.
    pub fn delete_profile(&mut self, profile: ProfileId) -> Result<()> {
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?;
        if count <= 1 {
            return Err(StoreError::LastProfile);
        }
        // `games.profile_id` não tem ON DELETE CASCADE (esquema publicado): apaga à mão;
        // o log e as posições caem em cascata com a Partida.
        tx.execute("DELETE FROM games WHERE profile_id = ?1", [profile.0])?;
        tx.execute("DELETE FROM profiles WHERE id = ?1", [profile.0])?;
        tx.commit()?;
        Ok(())
    }

    /// Preferência (chave -> JSON) do perfil; `None` se nunca foi gravada.
    pub fn get_pref(&self, profile: ProfileId, key: &str) -> Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT value_json FROM ui_preferences WHERE profile_id = ?1 AND key = ?2",
                params![profile.0, key],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| serde_json::from_str(&s).map_err(StoreError::from))
            .transpose()
    }

    /// Grava (ou substitui) a preferência; persiste no ato, sem "Salvar".
    pub fn set_pref(&self, profile: ProfileId, key: &str, value: &Value) -> Result<()> {
        ensure_profile(&self.conn, profile)?;
        self.conn.execute(
            "INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (?1, ?2, ?3)
             ON CONFLICT (profile_id, key) DO UPDATE SET value_json = excluded.value_json",
            params![profile.0, key, value.to_string()],
        )?;
        Ok(())
    }
}
