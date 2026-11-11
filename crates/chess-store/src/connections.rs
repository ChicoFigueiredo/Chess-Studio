//! Conexões (CX-3, #214): o vínculo de um Perfil local com um Provedor por **nome ou ID
//! público**. Pode haver várias por Provedor, cada uma com apelido, e uma é a padrão (por
//! Provedor e papel). A do jogador e a de Engine nunca se misturam (GLOSSARY).
//!
//! O Provedor é a chave do catálogo de `chess-net` (`"lichess"`, `"chesscom"`...): este crate
//! não depende da camada de rede. Nada aqui guarda segredo; a autorização (etapa 3) acrescenta
//! só uma referência ao Cofre.

use crate::error::{Result, StoreError};
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use rusqlite::{OptionalExtension, params};

/// Tamanho máximo do nome ou ID público (nomes de usuário e IDs FIDE cabem folgados).
pub const MAX_IDENTIFIER_LEN: usize = 64;
/// Tamanho máximo do apelido.
pub const MAX_NICKNAME_LEN: usize = 40;

/// Identificador de uma Conexão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub i64);

/// Como a Conexão se identifica no Provedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionKind {
    /// Nome de usuário público (Lichess, Chess.com).
    PublicName,
    /// ID público (FIDE ID, ID de jogador).
    PublicId,
}

impl ConnectionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicName => "public_name",
            Self::PublicId => "public_id",
        }
    }

    fn from_db(text: &str) -> Self {
        match text {
            "public_id" => Self::PublicId,
            _ => Self::PublicName,
        }
    }
}

/// Para que serve a Conexão: o jogador e a Engine não se misturam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionRole {
    Player,
    Engine,
}

impl ConnectionRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Engine => "engine",
        }
    }

    fn from_db(text: &str) -> Self {
        match text {
            "engine" => Self::Engine,
            _ => Self::Player,
        }
    }
}

/// Uma Conexão pedida (ainda sem id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConnection {
    pub provider: String,
    pub kind: ConnectionKind,
    pub identifier: String,
    pub nickname: String,
    pub role: ConnectionRole,
}

impl NewConnection {
    pub fn new(
        provider: impl Into<String>,
        kind: ConnectionKind,
        identifier: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            kind,
            identifier: identifier.into(),
            nickname: String::new(),
            role: ConnectionRole::Player,
        }
    }

    pub fn nickname(mut self, nickname: impl Into<String>) -> Self {
        self.nickname = nickname.into();
        self
    }

    pub fn role(mut self, role: ConnectionRole) -> Self {
        self.role = role;
        self
    }
}

/// Uma Conexão gravada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub id: ConnectionId,
    pub profile: ProfileId,
    pub provider: String,
    pub role: ConnectionRole,
    pub kind: ConnectionKind,
    pub identifier: String,
    pub nickname: String,
    /// A padrão do Provedor (por papel) para o Perfil local.
    pub is_default: bool,
    pub created_at_ms: i64,
}

impl Connection {
    /// O nome a mostrar: o apelido, ou o identificador se não há apelido.
    pub fn display_name(&self) -> &str {
        if self.nickname.is_empty() {
            &self.identifier
        } else {
            &self.nickname
        }
    }
}

const COLUMNS: &str = "id, profile_id, provider, role, kind, identifier, nickname, is_default,
                       created_at_ms";

fn row_to_connection(r: &rusqlite::Row<'_>) -> rusqlite::Result<Connection> {
    let role: String = r.get(3)?;
    let kind: String = r.get(4)?;
    Ok(Connection {
        id: ConnectionId(r.get(0)?),
        profile: ProfileId(r.get(1)?),
        provider: r.get(2)?,
        role: ConnectionRole::from_db(&role),
        kind: ConnectionKind::from_db(&kind),
        identifier: r.get(5)?,
        nickname: r.get(6)?,
        is_default: r.get::<_, i64>(7)? != 0,
        created_at_ms: r.get(8)?,
    })
}

fn valid_provider(provider: &str) -> bool {
    !provider.is_empty()
        && provider.len() <= 40
        && provider
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn clean_identifier(raw: &str) -> Result<String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(StoreError::InvalidConnection("identificador vazio".into()));
    }
    if text.chars().count() > MAX_IDENTIFIER_LEN {
        return Err(StoreError::InvalidConnection(format!(
            "identificador com mais de {MAX_IDENTIFIER_LEN} caracteres"
        )));
    }
    if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(StoreError::InvalidConnection(
            "identificador com espaço ou caractere de controle".into(),
        ));
    }
    Ok(text.to_owned())
}

fn clean_nickname(raw: &str) -> Result<String> {
    let text = raw.trim();
    if text.chars().count() > MAX_NICKNAME_LEN || text.chars().any(char::is_control) {
        return Err(StoreError::InvalidConnection("apelido inválido".into()));
    }
    Ok(text.to_owned())
}

impl Store {
    /// Vincula o `profile` a um Provedor. A primeira Conexão do Provedor (por papel) vira a
    /// padrão. O mesmo identificador (sem distinguir maiúsculas) não entra duas vezes.
    pub fn add_connection(
        &mut self,
        profile: ProfileId,
        connection: &NewConnection,
    ) -> Result<ConnectionId> {
        if !valid_provider(&connection.provider) {
            return Err(StoreError::InvalidConnection("Provedor inválido".into()));
        }
        let identifier = clean_identifier(&connection.identifier)?;
        let nickname = clean_nickname(&connection.nickname)?;
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        let role = connection.role.as_str();
        let exists = tx
            .query_row(
                "SELECT 1 FROM connections
                 WHERE profile_id = ?1 AND provider = ?2 AND role = ?3
                   AND lower(identifier) = lower(?4)",
                params![profile.0, connection.provider, role, identifier],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            return Err(StoreError::ConnectionExists {
                provider: connection.provider.clone(),
                identifier,
            });
        }
        let has_default = tx
            .query_row(
                "SELECT 1 FROM connections
                 WHERE profile_id = ?1 AND provider = ?2 AND role = ?3 AND is_default = 1",
                params![profile.0, connection.provider, role],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        tx.execute(
            "INSERT INTO connections
                 (profile_id, provider, role, kind, identifier, nickname, is_default,
                  created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                profile.0,
                connection.provider,
                role,
                connection.kind.as_str(),
                identifier,
                nickname,
                i64::from(!has_default),
                now_ms()
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(ConnectionId(id))
    }

    /// Todas as Conexões do `profile` (de qualquer papel), de um Provedor ou de todos, das mais
    /// antigas às mais novas.
    pub fn list_connections(
        &self,
        profile: ProfileId,
        provider: Option<&str>,
    ) -> Result<Vec<Connection>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM connections
             WHERE profile_id = ?1 AND (?2 IS NULL OR provider = ?2)
             ORDER BY provider, created_at_ms, id"
        ))?;
        let rows = stmt.query_map(params![profile.0, provider], row_to_connection)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Como [`Store::list_connections`], mas só as de um papel (jogador ou Engine).
    pub fn list_connections_by_role(
        &self,
        profile: ProfileId,
        provider: Option<&str>,
        role: ConnectionRole,
    ) -> Result<Vec<Connection>> {
        Ok(self
            .list_connections(profile, provider)?
            .into_iter()
            .filter(|c| c.role == role)
            .collect())
    }

    fn connection_of(&self, profile: ProfileId, id: ConnectionId) -> Result<Connection> {
        let found = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM connections WHERE id = ?1"),
                [id.0],
                row_to_connection,
            )
            .optional()?
            .ok_or(StoreError::ConnectionNotFound(id))?;
        if found.profile != profile {
            return Err(StoreError::WrongProfile { profile });
        }
        Ok(found)
    }

    /// Faz desta a Conexão padrão do Provedor (no papel dela).
    pub fn set_default_connection(&mut self, profile: ProfileId, id: ConnectionId) -> Result<()> {
        let target = self.connection_of(profile, id)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE connections SET is_default = 0
             WHERE profile_id = ?1 AND provider = ?2 AND role = ?3",
            params![profile.0, target.provider, target.role.as_str()],
        )?;
        tx.execute(
            "UPDATE connections SET is_default = 1 WHERE id = ?1",
            [id.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Troca o apelido (vazio tira o apelido).
    pub fn rename_connection(
        &self,
        profile: ProfileId,
        id: ConnectionId,
        nickname: &str,
    ) -> Result<()> {
        self.connection_of(profile, id)?;
        let nickname = clean_nickname(nickname)?;
        self.conn.execute(
            "UPDATE connections SET nickname = ?2 WHERE id = ?1",
            params![id.0, nickname],
        )?;
        Ok(())
    }

    /// Remove a Conexão; se era a padrão, a mais antiga que sobra no Provedor assume.
    pub fn remove_connection(&mut self, profile: ProfileId, id: ConnectionId) -> Result<()> {
        let target = self.connection_of(profile, id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM connections WHERE id = ?1", [id.0])?;
        if target.is_default {
            tx.execute(
                "UPDATE connections SET is_default = 1 WHERE id = (
                     SELECT id FROM connections
                     WHERE profile_id = ?1 AND provider = ?2 AND role = ?3
                     ORDER BY created_at_ms, id LIMIT 1)",
                params![profile.0, target.provider, target.role.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
