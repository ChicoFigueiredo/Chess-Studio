//! Achar uma Partida importada pelo id que o Provedor lhe deu (#239): o Explorador abre a
//! "Partida de referência" exata e, se ela já está na Biblioteca, só a abre em vez de baixá-la
//! de novo. O id remoto vem das tags do PGN no momento da importação (ver [`crate::remote_id`]).

use crate::error::Result;
use crate::imported::ImportedId;
use crate::store::{ProfileId, Store};
use rusqlite::OptionalExtension;

impl Store {
    /// A Partida importada do `profile` com este Provedor (`"lichess"`) e id remoto, se
    /// houver: vale tanto a Origem principal quanto uma Origem somada depois (a mesma Partida
    /// que chegou por arquivo e, mais tarde, pela API).
    pub fn imported_by_remote_id(
        &self,
        profile: ProfileId,
        provider: &str,
        remote_id: &str,
    ) -> Result<Option<ImportedId>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM imported_games
                 WHERE profile_id = ?1 AND provider = ?2 AND remote_id = ?3
                 UNION ALL
                 SELECT g.id FROM imported_game_origins o
                 JOIN imported_games g ON g.id = o.game_id
                 WHERE g.profile_id = ?1 AND o.provider = ?2 AND o.remote_id = ?3
                 LIMIT 1",
                rusqlite::params![profile.0, provider, remote_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id.map(ImportedId))
    }
}
