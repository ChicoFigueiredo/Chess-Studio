//! Posições favoritas (#49): FEN + nome + nota + origem (Partida e meio-lance). Uma favorita
//! por Posição (chave normalizada, como na busca): marcar de novo a mesma Posição atualiza
//! nome e nota em vez de duplicar.

use crate::error::{Result, StoreError};
use crate::library::{LibrarySource, fen_key};
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use rusqlite::{OptionalExtension, params};

/// Identificador de uma Posição favorita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FavoriteId(pub i64);

/// De onde a Posição foi marcada: a Partida (jogada ou importada) e o meio-lance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FavoriteOrigin {
    pub source: LibrarySource,
    /// `GameId.0` ou `ImportedId.0`, conforme `source`.
    pub ref_id: i64,
    pub ply: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFavorite {
    pub fen: String,
    pub name: String,
    pub note: String,
    pub origin: Option<FavoriteOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Favorite {
    pub id: FavoriteId,
    pub profile: ProfileId,
    /// FEN canônico da Posição.
    pub fen: String,
    pub name: String,
    pub note: String,
    pub origin: Option<FavoriteOrigin>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Filtro de [`Store::list_favorites`]; o padrão lista todas, da mais recente à mais antiga.
/// `profile: None` lista de todos os perfis (testes/administração): a interface sempre passa o ativo.
#[derive(Debug, Clone, Default)]
pub struct FavoriteQuery {
    /// Trecho (sem distinguir maiúsculas) do nome ou da nota.
    pub text: Option<String>,
    pub profile: Option<ProfileId>,
    pub limit: Option<u32>,
}

const COLUMNS: &str = "id, profile_id, fen, name, note, origin_source, origin_ref, origin_ply,
                       created_at_ms, updated_at_ms";

fn row_to_favorite(r: &rusqlite::Row<'_>) -> rusqlite::Result<Favorite> {
    let source: Option<String> = r.get(5)?;
    let reference: Option<i64> = r.get(6)?;
    let ply: Option<u32> = r.get(7)?;
    let origin = match (source.as_deref(), reference, ply) {
        (Some("played"), Some(ref_id), Some(ply)) => Some(FavoriteOrigin {
            source: LibrarySource::Played,
            ref_id,
            ply,
        }),
        (Some("imported"), Some(ref_id), Some(ply)) => Some(FavoriteOrigin {
            source: LibrarySource::Imported,
            ref_id,
            ply,
        }),
        _ => None,
    };
    Ok(Favorite {
        id: FavoriteId(r.get(0)?),
        profile: ProfileId(r.get(1)?),
        fen: r.get(2)?,
        name: r.get(3)?,
        note: r.get(4)?,
        origin,
        created_at_ms: r.get(8)?,
        updated_at_ms: r.get(9)?,
    })
}

impl Store {
    /// Marca a Posição como favorita no perfil padrão (ou atualiza a que já existe).
    pub fn add_favorite(&mut self, favorite: &NewFavorite) -> Result<FavoriteId> {
        let profile = self.default_profile()?;
        self.add_favorite_for(profile, favorite)
    }

    pub fn add_favorite_for(
        &mut self,
        profile: ProfileId,
        favorite: &NewFavorite,
    ) -> Result<FavoriteId> {
        let (position, key) = fen_key(&favorite.fen)?;
        let now = now_ms();
        let origin = favorite.origin;
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        tx.execute(
            "INSERT INTO favorite_positions (profile_id, fen, position_key, name, note,
                    origin_source, origin_ref, origin_ply, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT (profile_id, position_key) DO UPDATE SET
                 fen = excluded.fen, name = excluded.name, note = excluded.note,
                 origin_source = COALESCE(excluded.origin_source, origin_source),
                 origin_ref = COALESCE(excluded.origin_ref, origin_ref),
                 origin_ply = COALESCE(excluded.origin_ply, origin_ply),
                 updated_at_ms = excluded.updated_at_ms",
            params![
                profile.0,
                position.to_fen(),
                key,
                favorite.name,
                favorite.note,
                origin.map(|o| o.source.as_str()),
                origin.map(|o| o.ref_id),
                origin.map(|o| o.ply),
                now
            ],
        )?;
        let id = tx.query_row(
            "SELECT id FROM favorite_positions WHERE profile_id = ?1 AND position_key = ?2",
            params![profile.0, key],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(FavoriteId(id))
    }

    /// A favorita desta Posição no `profile`, se existir (FEN inválido é erro).
    pub fn find_favorite(&self, profile: ProfileId, fen: &str) -> Result<Option<FavoriteId>> {
        let (_, key) = fen_key(fen)?;
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM favorite_positions WHERE profile_id = ?1 AND position_key = ?2",
                params![profile.0, key],
                |r| r.get(0).map(FavoriteId),
            )
            .optional()?)
    }

    /// Lê a favorita; `WrongProfile` se ela é de outro perfil.
    pub fn get_favorite(&self, profile: ProfileId, id: FavoriteId) -> Result<Favorite> {
        let favorite = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM favorite_positions WHERE id = ?1"),
                [id.0],
                row_to_favorite,
            )
            .optional()?
            .ok_or(StoreError::FavoriteNotFound(id))?;
        if favorite.profile != profile {
            return Err(StoreError::WrongProfile { profile });
        }
        Ok(favorite)
    }

    pub fn list_favorites(&self, query: &FavoriteQuery) -> Result<Vec<Favorite>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM favorite_positions
             WHERE (?1 IS NULL OR profile_id = ?1)
             ORDER BY updated_at_ms DESC, id DESC"
        ))?;
        let rows = stmt.query_map([query.profile.map(|p| p.0)], row_to_favorite)?;
        let needle = query
            .text
            .as_deref()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty());
        let limit = query.limit.map_or(usize::MAX, |l| l as usize);
        let mut out = Vec::new();
        for row in rows {
            let favorite = row?;
            let hit = needle.as_ref().is_none_or(|n| {
                favorite.name.to_lowercase().contains(n.as_str())
                    || favorite.note.to_lowercase().contains(n.as_str())
            });
            if hit {
                out.push(favorite);
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn update_favorite(
        &self,
        profile: ProfileId,
        id: FavoriteId,
        name: &str,
        note: &str,
    ) -> Result<()> {
        self.get_favorite(profile, id)?;
        self.conn.execute(
            "UPDATE favorite_positions SET name = ?2, note = ?3, updated_at_ms = ?4
             WHERE id = ?1 AND profile_id = ?5",
            params![id.0, name, note, now_ms(), profile.0],
        )?;
        Ok(())
    }

    pub fn delete_favorite(&self, profile: ProfileId, id: FavoriteId) -> Result<()> {
        self.get_favorite(profile, id)?;
        self.conn.execute(
            "DELETE FROM favorite_positions WHERE id = ?1 AND profile_id = ?2",
            params![id.0, profile.0],
        )?;
        Ok(())
    }
}
