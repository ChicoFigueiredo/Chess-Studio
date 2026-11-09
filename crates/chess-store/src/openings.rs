//! Explorador de aberturas (C19, #147): estatísticas por lance candidato de uma Posição, sobre
//! as Partidas da Biblioteca do Perfil local (jogadas no app e importadas de PGN).
//!
//! # Índice
//!
//! `opening_moves` guarda, para cada lance da linha principal (até [`OPENING_PLIES`] meios-lances)
//! de cada entrada da Biblioteca: o hash de 64 bits (FNV-1a) da chave normalizada da Posição de
//! onde o lance sai, o lance em SAN, o resultado e o instante de uso. A chave normalizada é a de
//! [`position_key`](crate::position_key) (peças, vez, roques e en passant capturável, sem
//! relógios), então a mesma Posição alcançada por transposição junta as estatísticas.
//!
//! O índice é **derivado** e incremental: nasce junto com `library_entries` (mesma sincronização
//! por assinatura, que já roda a cada busca, na importação e no fim de cada Partida) e some em
//! cascata com a entrada, a Partida ou o Perfil. [`Store::rebuild_opening_index`] o refaz do zero.
//! Toda consulta leva o `ProfileId`; perfis nunca se misturam.

use crate::error::Result;
use crate::library::fen_key;
use crate::store::{ProfileId, Store};
use rusqlite::params;

/// Meios-lances da linha principal indexados por Partida.
pub const OPENING_PLIES: usize = 40;

/// Estatística de um lance candidato numa Posição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningMoveStats {
    /// O lance em SAN.
    pub san: String,
    /// Partidas distintas em que o lance foi jogado nesta Posição.
    pub games: u32,
    pub white_wins: u32,
    pub draws: u32,
    pub black_wins: u32,
    /// Partidas sem resultado (em andamento ou `*`); entram em `games`, não nas porcentagens.
    pub unfinished: u32,
    /// Instante (ms Unix) do uso mais recente: a última atividade da Partida jogada ou a
    /// importação da Partida importada.
    pub last_used_ms: i64,
}

impl OpeningMoveStats {
    /// Partidas com resultado.
    pub fn decided(&self) -> u32 {
        self.white_wins + self.draws + self.black_wins
    }

    /// Fração (0 a 1) das Partidas decididas: `(brancas, empates, pretas)`; zeros sem decididas.
    pub fn shares(&self) -> (f32, f32, f32) {
        let total = self.decided();
        if total == 0 {
            return (0.0, 0.0, 0.0);
        }
        let f = |n: u32| n as f32 / total as f32;
        (f(self.white_wins), f(self.draws), f(self.black_wins))
    }
}

/// Código do resultado no índice: 0 sem resultado, 1 brancas, 2 empate, 3 pretas.
pub(crate) fn result_code(token: &str) -> i64 {
    match token {
        "1-0" => 1,
        "1/2-1/2" => 2,
        "0-1" => 3,
        _ => 0,
    }
}

/// FNV-1a de 64 bits da chave da Posição, guardado como `i64`.
pub(crate) fn position_hash(key: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash as i64
}

impl Store {
    /// Lances jogados na Posição `fen` nas Partidas do `profile`, do mais frequente ao menos
    /// (empate: o mais recente, depois a SAN). Sincroniza o índice antes de consultar.
    pub fn opening_stats(&self, profile: ProfileId, fen: &str) -> Result<Vec<OpeningMoveStats>> {
        let (_, key) = fen_key(fen)?;
        self.sync_library()?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT san,
                    COUNT(*),
                    SUM(result = 1), SUM(result = 2), SUM(result = 3), SUM(result = 0),
                    MAX(used_ms)
             FROM opening_moves
             WHERE profile_id = ?1 AND pos_hash = ?2
             GROUP BY san
             ORDER BY COUNT(*) DESC, MAX(used_ms) DESC, san",
        )?;
        let rows = stmt.query_map(params![profile.0, position_hash(&key)], |r| {
            Ok(OpeningMoveStats {
                san: r.get(0)?,
                games: r.get(1)?,
                white_wins: r.get(2)?,
                draws: r.get(3)?,
                black_wins: r.get(4)?,
                unfinished: r.get(5)?,
                last_used_ms: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Descarta o índice da Biblioteca (inclusive o das aberturas) e o refaz a partir das
    /// Partidas. Sob demanda: o índice normal já é incremental.
    pub fn rebuild_opening_index(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM library_fts; DELETE FROM library_positions; DELETE FROM library_entries;",
        )?;
        self.sync_library()
    }
}
