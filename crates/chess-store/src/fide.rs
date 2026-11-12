//! Último rating FIDE consultado por Perfil local (CX-19, #242; migração 17).
//!
//! O dado vem do Lichess, que o reproduz da FIDE. Standard, rápido e blitz são campos
//! separados e nunca se misturam; a data guardada é a da **consulta**, porque a resposta do
//! Lichess não traz o período da lista da FIDE.

use crate::error::Result;
use crate::store::{ProfileId, Store, ensure_profile};
use rusqlite::{OptionalExtension, params};

/// Licença dos dados da FIDE reproduzidos pelo Lichess (a única fonte do rating hoje): a
/// reprodução é da FIDE e fica restrita ao uso pessoal. Guarda-se um identificador estável; o
/// texto para o usuário vem do Fluent.
pub const FIDE_LICENSE_VIA_LICHESS: &str = "fide-via-lichess";

/// O que se guarda de uma consulta ao rating FIDE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FideSnapshot {
    pub fide_id: u64,
    pub name: String,
    /// Federação (código de três letras).
    pub federation: Option<String>,
    pub title: Option<String>,
    pub birth_year: Option<u16>,
    pub standard: Option<u16>,
    pub rapid: Option<u16>,
    pub blitz: Option<u16>,
    pub inactive: bool,
    /// Instante da consulta (ms desde a época Unix), injetado por quem chama.
    pub queried_at_ms: i64,
    /// Chave do Provedor que respondeu (`lichess`).
    pub source: String,
    /// Identificador da licença dos dados (ver [`FIDE_LICENSE_VIA_LICHESS`]): a Origem completa
    /// do rating (ADR-0006).
    pub license: String,
}

fn id_column(fide_id: u64) -> i64 {
    i64::try_from(fide_id).unwrap_or(i64::MAX)
}

impl Store {
    /// Guarda (ou troca) o último rating do ID FIDE no Perfil local. Um ID por linha: trocar
    /// o ID vinculado não apaga o valor guardado do anterior.
    pub fn save_fide_rating(&self, profile: ProfileId, snapshot: &FideSnapshot) -> Result<()> {
        ensure_profile(&self.conn, profile)?;
        self.conn.execute(
            "INSERT INTO fide_ratings (profile_id, fide_id, name, federation, title, birth_year,
                                       standard, rapid, blitz, inactive, queried_at_ms, source, license)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT (profile_id, fide_id) DO UPDATE SET
                 name = excluded.name, federation = excluded.federation, title = excluded.title,
                 birth_year = excluded.birth_year, standard = excluded.standard,
                 rapid = excluded.rapid, blitz = excluded.blitz, inactive = excluded.inactive,
                 queried_at_ms = excluded.queried_at_ms, source = excluded.source,
                 license = excluded.license",
            params![
                profile.0,
                id_column(snapshot.fide_id),
                snapshot.name,
                snapshot.federation,
                snapshot.title,
                snapshot.birth_year,
                snapshot.standard,
                snapshot.rapid,
                snapshot.blitz,
                snapshot.inactive,
                snapshot.queried_at_ms,
                snapshot.source,
                snapshot.license,
            ],
        )?;
        Ok(())
    }

    /// O último rating guardado do ID FIDE neste Perfil local (leitura offline).
    pub fn fide_rating(&self, profile: ProfileId, fide_id: u64) -> Result<Option<FideSnapshot>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, federation, title, birth_year, standard, rapid, blitz, inactive,
                        queried_at_ms, source, license
                 FROM fide_ratings WHERE profile_id = ?1 AND fide_id = ?2",
                params![profile.0, id_column(fide_id)],
                |r| {
                    Ok(FideSnapshot {
                        fide_id,
                        name: r.get(0)?,
                        federation: r.get(1)?,
                        title: r.get(2)?,
                        birth_year: r.get(3)?,
                        standard: r.get(4)?,
                        rapid: r.get(5)?,
                        blitz: r.get(6)?,
                        inactive: r.get(7)?,
                        queried_at_ms: r.get(8)?,
                        source: r.get(9)?,
                        license: r.get(10)?,
                    })
                },
            )
            .optional()?)
    }
}
