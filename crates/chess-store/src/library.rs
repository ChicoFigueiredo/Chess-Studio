//! Biblioteca pesquisável (#49): um índice sobre as Partidas jogadas no app (`games`, via log
//! de Eventos) e as importadas de PGN (`imported_games`), com filtros, busca por Posição e
//! pesquisa de Exercícios por tipo.
//!
//! # Como o índice funciona
//!
//! O índice (`library_entries` + `library_fts` + `library_positions`) é **derivado**: a
//! verdade continua no log de Eventos (ADR-0002) e na árvore importada. Ele é mantido de
//! forma preguiçosa: cada busca primeiro sincroniza o que mudou (Partida com Lances novos,
//! importação nova, exclusão) comparando uma assinatura barata. Assim nenhum caminho de
//! escrita existente (`append_events`, `import_pgn`) precisa conhecer o índice, e um banco
//! antigo é indexado no primeiro uso sem perder nada (DB-06).
//!
//! O texto livre usa FTS5 (`unicode61` sem distinção de acentos, busca por prefixo); a
//! Posição usa uma chave normalizada (FEN sem os contadores e com en passant só quando há
//! captura legal), então "mesma Posição" ignora o número do Lance e o relógio de 50 Lances.

use crate::error::{Result, StoreError};
use crate::store::{GameId, ProfileId, Store};
use chess_core::{CanonicalPosition, Piece};
use chess_game::{EndReason, Game, GameSetup, PolicyKind};
use chess_pgn::{MoveNode, PgnGame};
use chess_training::{Exercise, ExerciseKind};
use rusqlite::types::Value as Sql;
use rusqlite::{params, params_from_iter};
use serde_json::Value;
use std::collections::HashMap;

/// De onde vem a entrada da biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LibrarySource {
    /// Partida jogada no app (`games`).
    Played,
    /// Partida importada de PGN (`imported_games`).
    Imported,
}

impl LibrarySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Played => "played",
            Self::Imported => "imported",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "played" => Some(Self::Played),
            "imported" => Some(Self::Imported),
            _ => None,
        }
    }
}

/// Identificador de uma entrada do índice (não confundir com `GameId`/`ImportedId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LibraryEntryId(pub i64);

/// Linha da biblioteca: metadados de uma Partida jogada ou importada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub id: LibraryEntryId,
    pub source: LibrarySource,
    /// `GameId.0` (jogada) ou `ImportedId.0` (importada).
    pub ref_id: i64,
    pub white: Option<String>,
    pub black: Option<String>,
    pub event: Option<String>,
    /// Data PGN (`AAAA.MM.DD`, partes desconhecidas como `??`).
    pub date: Option<String>,
    /// `1-0`, `0-1`, `1/2-1/2` ou `*`.
    pub result: String,
    /// Por que a Partida jogada terminou.
    pub reason: Option<EndReason>,
    /// Política da Partida jogada (`None` nas importadas).
    pub policy: Option<PolicyKind>,
    /// Adversário da Partida jogada, quando é a Engine (`Engine 1500`).
    pub opponent: Option<String>,
    /// Meios-lances da linha principal.
    pub ply_count: u32,
    /// Primeiros Lances em SAN (`1. e4 e5 2. Nf3 Nc6 3. Bb5 a6`).
    pub opening: Option<String>,
    pub finished: bool,
    /// Com filtro de Posição: o menor meio-lance da linha principal em que ela ocorre
    /// (`None` se só ocorre numa variação).
    pub matched_ply: Option<u32>,
}

/// Filtros de [`Store::search_library`]; o padrão lista tudo, do mais recente ao mais antigo.
#[derive(Debug, Clone, Default)]
pub struct LibraryQuery {
    /// Texto livre (todos os termos, por prefixo, sem distinguir maiúsculas nem acentos).
    pub text: Option<String>,
    /// Trecho do nome de um dos jogadores (ou da Engine).
    pub player: Option<String>,
    /// Resultado exato (`1-0`, `0-1`, `1/2-1/2`, `*`).
    pub result: Option<String>,
    /// Política (só Partidas jogadas).
    pub policy: Option<PolicyKind>,
    /// Intervalo de datas `AAAA.MM.DD` (inclusivo); data parcial (`2023.??.??`) casa se cruzar.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// FEN: Partidas em que a Posição ocorre (chave normalizada).
    pub position: Option<String>,
    pub source: Option<LibrarySource>,
    pub profile: Option<ProfileId>,
    pub limit: Option<u32>,
    pub offset: u32,
}

/// Uma página de resultados e o total que casa com o filtro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPage {
    pub total: u64,
    pub entries: Vec<LibraryEntry>,
}

/// Chave normalizada da Posição: campos de peças, vez, roque e en passant (este só quando
/// há captura en passant legal). Ignora os contadores de Lances.
pub fn position_key(position: &CanonicalPosition) -> String {
    let fen = position.to_fen();
    let mut fields = fen.split(' ');
    let board = fields.next().unwrap_or_default();
    let side = fields.next().unwrap_or("w");
    let castling = fields.next().unwrap_or("-");
    let ep = fields.next().unwrap_or("-");
    let ep = if ep != "-" && en_passant_capturable(position, ep) {
        ep
    } else {
        "-"
    };
    format!("{board} {side} {castling} {ep}")
}

fn en_passant_capturable(position: &CanonicalPosition, target: &str) -> bool {
    position.legal_moves().into_iter().any(|mv| {
        mv.to().to_string() == target
            && mv.from().file() != mv.to().file()
            && matches!(position.piece_at(mv.from()), Some((_, Piece::Pawn)))
            && position.piece_at(mv.to()).is_none()
    })
}

/// Chave de uma FEN do usuário (erro tipado se o FEN é inválido).
pub fn fen_key(fen: &str) -> Result<(CanonicalPosition, String)> {
    let position = CanonicalPosition::from_fen(fen.trim())
        .map_err(|e| StoreError::InvalidFen(format!("{fen:?}: {e}")))?;
    let key = position_key(&position);
    Ok((position, key))
}

/// Primeiros Lances em SAN, numerados a partir da Posição inicial.
fn opening_text(start: &CanonicalPosition, sans: &[&str]) -> Option<String> {
    const OPENING_PLIES: usize = 6;
    if sans.is_empty() {
        return None;
    }
    let mut number = start.fullmove_number();
    let mut white_to_move = start.side_to_move() == chess_core::Color::White;
    let mut out = String::new();
    for (i, san) in sans.iter().take(OPENING_PLIES).enumerate() {
        if !out.is_empty() {
            out.push(' ');
        }
        if white_to_move {
            out.push_str(&format!("{number}. "));
        } else if i == 0 {
            out.push_str(&format!("{number}... "));
        }
        out.push_str(san);
        if !white_to_move {
            number += 1;
        }
        white_to_move = !white_to_move;
    }
    Some(out)
}

/// Limites inferior e superior (`AAAA.MM.DD`) de uma data PGN possivelmente parcial.
fn date_bounds(date: &str) -> Option<(String, String)> {
    let mut parts = date.split('.');
    let year = parts
        .next()
        .filter(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()))?;
    let part = |p: Option<&str>, max: u32| -> Option<(u32, u32)> {
        match p {
            Some(text) if text.bytes().all(|b| b.is_ascii_digit()) && !text.is_empty() => {
                let n: u32 = text.parse().ok()?;
                (1..=max).contains(&n).then_some((n, n))
            }
            _ => Some((1, max)),
        }
    };
    let (m_lo, m_hi) = part(parts.next(), 12)?;
    let (d_lo, d_hi) = if m_lo == m_hi {
        part(parts.next(), 31)?
    } else {
        (1, 31)
    };
    Some((
        format!("{year}.{m_lo:02}.{d_lo:02}"),
        format!("{year}.{m_hi:02}.{d_hi:02}"),
    ))
}

/// Linha pronta para gravar no índice.
struct Draft {
    source: LibrarySource,
    ref_id: i64,
    profile: i64,
    signature: i64,
    white: Option<String>,
    black: Option<String>,
    event: Option<String>,
    date: Option<String>,
    result: String,
    reason: Option<String>,
    policy: Option<String>,
    opponent: Option<String>,
    ply_count: i64,
    opening: Option<String>,
    finished: bool,
    sort_ms: i64,
    /// Texto para o FTS.
    body: String,
    /// `(chave, meio-lance, linha principal)`.
    positions: Vec<(String, u32, bool)>,
    /// `(chave da Posição, lance em SAN)` da linha principal, para o explorador de aberturas.
    moves: Vec<(String, String)>,
}

impl Draft {
    /// Linha sem Lances: guardada para a assinatura (não se reindexa), mas nunca listada.
    fn hidden(source: LibrarySource, ref_id: i64, profile: i64, signature: i64) -> Self {
        Self {
            source,
            ref_id,
            profile,
            signature,
            white: None,
            black: None,
            event: None,
            date: None,
            result: "*".into(),
            reason: None,
            policy: None,
            opponent: None,
            ply_count: 0,
            opening: None,
            finished: false,
            sort_ms: 0,
            body: String::new(),
            positions: Vec::new(),
            moves: Vec::new(),
        }
    }
}

fn enum_id<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).ok()? {
        Value::String(s) => Some(s),
        _ => None,
    }
}

fn join_body(parts: &[Option<&str>]) -> String {
    parts
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `(chave da Posição, lance em SAN)` da linha principal, até [`crate::openings::OPENING_PLIES`].
fn mainline_moves(start: &CanonicalPosition, line: &[MoveNode]) -> Vec<(String, String)> {
    let mut position = start.clone();
    let mut out = Vec::new();
    for node in line.iter().take(crate::openings::OPENING_PLIES) {
        let Ok(mv) = position.parse_san(&node.san) else {
            break;
        };
        let Ok(next) = position.play(mv) else { break };
        // SAN canônico (sem depender de como o PGN escreveu `+`, `#` ou desambiguações).
        let san = position.san(mv).unwrap_or_else(|_| node.san.clone());
        out.push((position_key(&position), san));
        position = next;
    }
    out
}

fn collect_line(
    start: &CanonicalPosition,
    line: &[MoveNode],
    first_ply: u32,
    main: bool,
    out: &mut Vec<(String, u32, bool)>,
) {
    let mut position = start.clone();
    for (i, node) in line.iter().enumerate() {
        let ply = first_ply + i as u32;
        for variation in &node.variations {
            collect_line(&position, variation, ply, false, out);
        }
        let Ok(mv) = position.parse_san(&node.san) else {
            return;
        };
        let Ok(next) = position.play(mv) else { return };
        position = next;
        out.push((position_key(&position), ply + 1, main));
    }
}

impl Store {
    // ---- Índice -----------------------------------------------------------------

    /// Verificação barata (agregados, sem carregar linhas) de que o índice reflete as fontes:
    /// mesmos totais, somas de ids e de assinaturas (Partidas jogadas) ou de instantes de
    /// importação (importadas). Um falso "em dia" exigiria uma troca que preserve as três somas;
    /// nesse caso a próxima mudança real, ou `rebuild_opening_index`, corrige.
    fn library_in_sync(&self) -> Result<bool> {
        let sources: (i64, i64, i64, i64, i64, i64) = self.conn.query_row(
            "SELECT
                (SELECT COUNT(*) FROM games),
                (SELECT COALESCE(SUM(id), 0) FROM games),
                (SELECT COALESCE(SUM((COALESCE((SELECT MAX(seq) FROM game_events e WHERE e.game_id = g.id), -1) + 1) * 2
                                      + g.finished), 0) FROM games g),
                (SELECT COUNT(*) FROM imported_games),
                (SELECT COALESCE(SUM(id), 0) FROM imported_games),
                (SELECT COALESCE(SUM(imported_at_ms), 0) FROM imported_games)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
        let indexed: (i64, i64, i64, i64, i64, i64) = self.conn.query_row(
            "SELECT
                COALESCE(SUM(source = 'played'), 0),
                COALESCE(SUM(CASE WHEN source = 'played' THEN ref_id END), 0),
                COALESCE(SUM(CASE WHEN source = 'played' THEN signature END), 0),
                COALESCE(SUM(source = 'imported'), 0),
                COALESCE(SUM(CASE WHEN source = 'imported' THEN ref_id END), 0),
                COALESCE(SUM(CASE WHEN source = 'imported' THEN sort_ms END), 0)
             FROM library_entries",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;
        Ok(sources == indexed)
    }

    /// Atualiza o índice com o que mudou desde a última busca. Barato quando nada mudou.
    pub(crate) fn sync_library(&self) -> Result<()> {
        if self.library_in_sync()? {
            return Ok(());
        }
        // Estado atual das fontes: (fonte, id) -> (perfil, assinatura).
        let mut current: HashMap<(LibrarySource, i64), (i64, i64)> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT g.id, g.profile_id,
                        (COALESCE((SELECT MAX(seq) FROM game_events e WHERE e.game_id = g.id), -1) + 1) * 2
                            + g.finished
                 FROM games g",
            )?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })? {
                let (id, profile, signature) = row?;
                current.insert((LibrarySource::Played, id), (profile, signature));
            }
            let mut stmt = self
                .conn
                .prepare("SELECT id, profile_id FROM imported_games")?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (id, profile) = row?;
                current.insert((LibrarySource::Imported, id), (profile, 1));
            }
        }
        let mut indexed: HashMap<(LibrarySource, i64), (i64, i64)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, source, ref_id, signature FROM library_entries")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })? {
                let (id, source, ref_id, signature) = row?;
                if let Some(source) = LibrarySource::parse(&source) {
                    indexed.insert((source, ref_id), (id, signature));
                }
            }
        }
        let stale: Vec<i64> = indexed
            .iter()
            .filter(|(key, (_, signature))| {
                current
                    .get(key)
                    .is_none_or(|(_, current_signature)| current_signature != signature)
            })
            .map(|(_, (id, _))| *id)
            .collect();
        let mut missing: Vec<(LibrarySource, i64, i64, i64)> = current
            .iter()
            .filter(|(key, (_, signature))| {
                indexed
                    .get(key)
                    .is_none_or(|(_, indexed_signature)| indexed_signature != signature)
            })
            .map(|(key, (profile, signature))| (key.0, key.1, *profile, *signature))
            .collect();
        if stale.is_empty() && missing.is_empty() {
            return Ok(());
        }
        missing.sort_by_key(|(source, id, _, _)| (*source, *id));
        let drafts: Vec<Draft> = missing
            .into_iter()
            .map(|(source, id, profile, signature)| match source {
                LibrarySource::Played => self.draft_played(GameId(id), profile, signature),
                LibrarySource::Imported => self.draft_imported(id, profile, signature),
            })
            .collect();

        let tx = self.conn.unchecked_transaction()?;
        for id in stale {
            tx.execute("DELETE FROM library_fts WHERE rowid = ?1", [id])?;
            tx.execute("DELETE FROM library_entries WHERE id = ?1", [id])?;
        }
        for draft in drafts {
            let (date_lo, date_hi) = draft
                .date
                .as_deref()
                .and_then(date_bounds)
                .map_or((None, None), |(lo, hi)| (Some(lo), Some(hi)));
            let players = [&draft.white, &draft.black, &draft.opponent]
                .into_iter()
                .flatten()
                .map(|p| p.to_lowercase())
                .collect::<Vec<_>>()
                .join("\t");
            tx.execute(
                "INSERT INTO library_entries (profile_id, source, ref_id, signature, white, black,
                        event, date, date_lo, date_hi, players, result, reason, policy, opponent,
                        ply_count, opening, finished, sort_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    draft.profile,
                    draft.source.as_str(),
                    draft.ref_id,
                    draft.signature,
                    draft.white,
                    draft.black,
                    draft.event,
                    draft.date,
                    date_lo,
                    date_hi,
                    players,
                    draft.result,
                    draft.reason,
                    draft.policy,
                    draft.opponent,
                    draft.ply_count,
                    draft.opening,
                    draft.finished,
                    draft.sort_ms,
                ],
            )?;
            let entry = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO library_fts (rowid, body) VALUES (?1, ?2)",
                params![entry, draft.body],
            )?;
            let mut insert = tx.prepare_cached(
                "INSERT INTO library_positions (entry_id, key, ply, main) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (entry_id, key, ply) DO UPDATE SET main = MAX(main, excluded.main)",
            )?;
            for (key, ply, main) in &draft.positions {
                insert.execute(params![entry, key, ply, main])?;
            }
            let code = crate::openings::result_code(&draft.result);
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO opening_moves (profile_id, pos_hash, san, entry_id, result, used_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (key, san) in &draft.moves {
                insert.execute(params![
                    draft.profile,
                    crate::openings::position_hash(key),
                    san,
                    entry,
                    code,
                    draft.sort_ms
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn draft_played(&self, id: GameId, profile: i64, signature: i64) -> Draft {
        let hidden = || Draft::hidden(LibrarySource::Played, id.0, profile, signature);
        let Ok(game) = self.load_game(id) else {
            return hidden();
        };
        let Ok((created, updated)) = self.conn.query_row(
            "SELECT created_at_ms, updated_at_ms FROM games WHERE id = ?1",
            [id.0],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        ) else {
            return hidden();
        };
        played_draft(&game, id, profile, signature, created, updated)
    }

    fn draft_imported(&self, id: i64, profile: i64, signature: i64) -> Draft {
        let hidden = || Draft::hidden(LibrarySource::Imported, id, profile, signature);
        let Ok(game) = self.load_imported_any(crate::ImportedId(id)) else {
            return hidden();
        };
        let imported_at: i64 = self
            .conn
            .query_row(
                "SELECT imported_at_ms FROM imported_games WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        imported_draft(&game, id, profile, signature, imported_at).unwrap_or_else(|| {
            // Sem Lances: não listada, mas guarda o instante para a verificação de "em dia".
            let mut draft = hidden();
            draft.sort_ms = imported_at;
            draft
        })
    }

    // ---- Busca ------------------------------------------------------------------

    /// Pesquisa Partidas jogadas e importadas. Sincroniza o índice antes de consultar.
    pub fn search_library(&self, query: &LibraryQuery) -> Result<LibraryPage> {
        let position = query.position.as_deref().map(fen_key).transpose()?;
        self.sync_library()?;

        let mut args: Vec<Sql> = Vec::new();
        let mut join = String::new();
        let mut conds = vec!["e.ply_count > 0".to_owned()];
        if let Some((_, key)) = &position {
            let p = bind(&mut args, Sql::Text(key.clone()));
            join = format!(
                " JOIN (SELECT entry_id, MIN(CASE WHEN main = 1 THEN ply END) AS mply
                         FROM library_positions WHERE key = {p} GROUP BY entry_id) m
                       ON m.entry_id = e.id"
            );
        }
        if let Some(profile) = query.profile {
            conds.push(format!(
                "e.profile_id = {}",
                bind(&mut args, Sql::Integer(profile.0))
            ));
        }
        if let Some(source) = query.source {
            conds.push(format!(
                "e.source = {}",
                bind(&mut args, Sql::Text(source.as_str().to_owned()))
            ));
        }
        if let Some(result) = query.result.as_deref().filter(|r| !r.is_empty()) {
            conds.push(format!(
                "e.result = {}",
                bind(&mut args, Sql::Text(result.to_owned()))
            ));
        }
        if let Some(policy) = query.policy.and_then(|p| enum_id(&p)) {
            conds.push(format!("e.policy = {}", bind(&mut args, Sql::Text(policy))));
        }
        if let Some(player) = query
            .player
            .as_deref()
            .map(|p| p.trim().to_lowercase())
            .filter(|p| !p.is_empty())
        {
            conds.push(format!(
                "instr(e.players, {}) > 0",
                bind(&mut args, Sql::Text(player))
            ));
        }
        if let Some(from) = query.date_from.as_deref().filter(|d| !d.is_empty()) {
            conds.push(format!(
                "e.date_hi >= {}",
                bind(&mut args, Sql::Text(from.to_owned()))
            ));
        }
        if let Some(to) = query.date_to.as_deref().filter(|d| !d.is_empty()) {
            conds.push(format!(
                "e.date_lo <= {}",
                bind(&mut args, Sql::Text(to.to_owned()))
            ));
        }
        if let Some(expr) = query.text.as_deref().and_then(fts_expression) {
            conds.push(format!(
                "e.id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH {})",
                bind(&mut args, Sql::Text(expr))
            ));
        }
        let filter = format!("FROM library_entries e{join} WHERE {}", conds.join(" AND "));

        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) {filter}"),
            params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let matched = if position.is_some() { "m.mply" } else { "NULL" };
        let limit = bind(&mut args, Sql::Integer(query.limit.map_or(-1, i64::from)));
        let offset = bind(&mut args, Sql::Integer(i64::from(query.offset)));
        let sql = format!(
            "SELECT e.id, e.source, e.ref_id, e.white, e.black, e.event, e.date, e.result, e.reason,
                    e.policy, e.opponent, e.ply_count, e.opening, e.finished, {matched}
             {filter} ORDER BY e.sort_ms DESC, e.id DESC LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            let source: String = r.get(1)?;
            let reason: Option<String> = r.get(8)?;
            let policy: Option<String> = r.get(9)?;
            Ok(LibraryEntry {
                id: LibraryEntryId(r.get(0)?),
                source: LibrarySource::parse(&source).unwrap_or(LibrarySource::Played),
                ref_id: r.get(2)?,
                white: r.get(3)?,
                black: r.get(4)?,
                event: r.get(5)?,
                date: r.get(6)?,
                result: r.get(7)?,
                reason: reason.and_then(|s| serde_json::from_value(Value::String(s)).ok()),
                policy: policy.and_then(|s| serde_json::from_value(Value::String(s)).ok()),
                opponent: r.get(10)?,
                ply_count: r.get(11)?,
                opening: r.get(12)?,
                finished: r.get(13)?,
                matched_ply: r.get(14)?,
            })
        })?;
        Ok(LibraryPage {
            total: u64::try_from(total).unwrap_or(0),
            entries: rows.collect::<std::result::Result<_, _>>()?,
        })
    }

    // ---- Exercícios -------------------------------------------------------------

    /// Exercícios do catálogo gravado, por tipo e/ou texto (título, id, objetivo).
    pub fn search_exercises(
        &self,
        kind: Option<ExerciseKind>,
        text: Option<&str>,
    ) -> Result<Vec<Exercise>> {
        let mut stmt = self.conn.prepare(
            "SELECT definition_json FROM exercises
             WHERE definition_json != '{}' AND (?1 IS NULL OR kind = ?1) ORDER BY id",
        )?;
        let rows = stmt.query_map([kind.map(ExerciseKind::as_str)], |r| r.get::<_, String>(0))?;
        let needle = text
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty());
        let mut out = Vec::new();
        for row in rows {
            let Ok(exercise) = serde_json::from_str::<Exercise>(&row?) else {
                continue;
            };
            let hit = needle.as_ref().is_none_or(|n| {
                [&exercise.title, &exercise.id, &exercise.objective]
                    .iter()
                    .any(|field| field.to_lowercase().contains(n.as_str()))
            });
            if hit {
                out.push(exercise);
            }
        }
        Ok(out)
    }
}

/// Expressão FTS5: cada termo entre aspas e por prefixo; `None` se não há termo útil.
fn fts_expression(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn played_draft(
    game: &Game,
    id: GameId,
    profile: i64,
    signature: i64,
    created_ms: i64,
    updated_ms: i64,
) -> Draft {
    let snapshot = game.snapshot();
    let setup: &GameSetup = game.setup();
    let plies = snapshot.history.len();
    if plies == 0 {
        return Draft::hidden(LibrarySource::Played, id.0, profile, signature);
    }
    let (white, black, opponent) = match setup.engine() {
        Some(seat) => {
            let engine = format!("Engine {}", seat.elo);
            match seat.color {
                chess_core::Color::White => (engine.clone(), "Você".to_owned(), Some(engine)),
                chess_core::Color::Black => ("Você".to_owned(), engine.clone(), Some(engine)),
            }
        }
        None => ("Brancas".to_owned(), "Pretas".to_owned(), None),
    };
    let wall = game
        .events()
        .iter()
        .find_map(|e| e.at.wall_ms)
        .unwrap_or(created_ms);
    let date = chess_game::pgn_date_from_unix_ms(wall);
    let result = snapshot
        .outcome
        .map_or("*", |o| match o.result {
            chess_game::GameResult::WhiteWins => "1-0",
            chess_game::GameResult::BlackWins => "0-1",
            chess_game::GameResult::Draw => "1/2-1/2",
        })
        .to_owned();
    let reason = snapshot.outcome.and_then(|o| enum_id(&o.reason));
    let policy = enum_id(&setup.policy());
    let sans: Vec<&str> = snapshot.history.iter().map(|h| h.san.as_str()).collect();
    let opening = opening_text(setup.start(), &sans);
    let positions: Vec<(String, u32, bool)> = (0..=plies)
        .filter_map(|ply| game.review_at_ply(ply))
        .map(|view| (position_key(&view.position), view.ply as u32, true))
        .collect();
    let moves = positions
        .iter()
        .zip(&snapshot.history)
        .take(crate::openings::OPENING_PLIES)
        .map(|((key, _, _), entry)| (key.clone(), entry.san.clone()))
        .collect();
    let body = join_body(&[
        Some(&white),
        Some(&black),
        Some(&date),
        Some(&result),
        reason.as_deref(),
        policy.as_deref(),
        opening.as_deref(),
    ]);
    Draft {
        source: LibrarySource::Played,
        ref_id: id.0,
        profile,
        signature,
        white: Some(white),
        black: Some(black),
        event: None,
        date: Some(date),
        result,
        reason,
        policy,
        opponent,
        ply_count: plies as i64,
        opening,
        finished: snapshot.outcome.is_some(),
        sort_ms: updated_ms,
        body,
        positions,
        moves,
    }
}

fn imported_draft(
    game: &PgnGame,
    id: i64,
    profile: i64,
    signature: i64,
    imported_at: i64,
) -> Option<Draft> {
    if game.moves.is_empty() {
        return None;
    }
    let start = CanonicalPosition::from_fen(&game.start_fen).ok()?;
    let known = |name: &str| game.tag(name).filter(|v| *v != "?").map(str::to_owned);
    let mut positions = vec![(position_key(&start), 0, true)];
    collect_line(&start, &game.moves, 0, true, &mut positions);
    let sans = game.mainline_sans();
    let moves = mainline_moves(&start, &game.moves);
    let opening = opening_text(&start, &sans);
    let tags: Vec<&str> = game
        .tags
        .iter()
        .filter(|(name, _)| name != "FEN" && name != "SetUp")
        .map(|(_, value)| value.as_str())
        .filter(|v| *v != "?")
        .collect();
    let body = format!(
        "{} {}",
        tags.join(" "),
        opening.as_deref().unwrap_or_default()
    );
    Some(Draft {
        source: LibrarySource::Imported,
        ref_id: id,
        profile,
        signature,
        white: known("White"),
        black: known("Black"),
        event: known("Event"),
        date: game
            .tag("Date")
            .filter(|v| !v.starts_with('?'))
            .map(str::to_owned),
        result: game.result.token().to_owned(),
        reason: None,
        policy: None,
        opponent: None,
        ply_count: game.moves.len() as i64,
        opening,
        finished: game.result != chess_pgn::PgnResult::Unknown,
        sort_ms: imported_at,
        body,
        positions,
        moves,
    })
}

/// Acrescenta o argumento e devolve o marcador numerado (`?N`) para o SQL.
fn bind(args: &mut Vec<Sql>, value: Sql) -> String {
    args.push(value);
    format!("?{}", args.len())
}
