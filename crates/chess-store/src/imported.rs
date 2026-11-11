//! Partidas importadas de PGN (#48): a árvore de variações (com comentários e
//! NAGs) fica em JSON na tabela `imported_games`, com colunas de listagem. É a
//! base da biblioteca (C7). Não toca `games`/`game_events`: o log de Eventos
//! continua sendo a verdade das Partidas jogadas aqui (ADR-0002).
//!
//! Desde a migração 13 (CX-4, #216) cada Partida tem uma Origem estruturada
//! ([`Provenance`]), uma impressão digital para detectar duplicatas e colunas de
//! Site, Elo, TimeControl, ECO e ritmo. A importação em fluxo lê o arquivo em lotes.

use crate::error::{Result, StoreError};
use crate::origin::{ImportMode, Origin, Pace, Provenance, game_fingerprint, parse_elo, remote_id};
use crate::store::{ProfileId, Store, ensure_profile, now_ms};
use chess_pgn::{PgnError, PgnGame, PgnStream, StreamItem};
use rusqlite::{OptionalExtension, params};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Identificador de uma partida importada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImportedId(pub i64);

/// Resultado de [`Store::import_pgn`]: as partidas gravadas, quantas já existiam e os erros
/// das que foram puladas (cada erro tem partida, linha, coluna e o Lance problemático).
#[derive(Debug)]
pub struct ImportReport {
    pub imported: Vec<ImportedId>,
    /// Partidas que já estavam na Biblioteca (mesma impressão digital ou Provedor + id remoto).
    pub duplicates: usize,
    pub errors: Vec<PgnError>,
}

/// Como importar: nome do arquivo (texto histórico), Origem e tamanho do lote.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub source: Option<String>,
    pub provenance: Provenance,
    /// Partidas por transação na importação em fluxo.
    pub batch_size: usize,
    /// Máximo de bytes descompactados lidos de um `.pgn.gz` (protege de bomba de
    /// descompressão); passar do limite é [`StoreError::DecompressedTooLarge`].
    pub max_decompressed_bytes: u64,
}

/// Limite padrão de bytes descompactados de um `.pgn.gz`.
pub const DEFAULT_MAX_DECOMPRESSED: u64 = 4 << 30;

/// Marca o erro de E/S de um fluxo que passou do limite descompactado.
#[derive(Debug)]
struct DecompressionLimit(u64);

impl std::fmt::Display for DecompressionLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "limite de {} bytes descompactados", self.0)
    }
}

impl std::error::Error for DecompressionLimit {}

/// Lê no máximo `left` bytes; passar disso é erro (não fim de arquivo silencioso).
struct Limited<R> {
    inner: R,
    limit: u64,
    left: u64,
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n as u64 > self.left {
            return Err(std::io::Error::other(DecompressionLimit(self.limit)));
        }
        self.left -= n as u64;
        Ok(n)
    }
}

/// Partidas por lote, por padrão.
pub const DEFAULT_BATCH: usize = 1_000;
/// Quantos erros detalhados o resumo guarda (o total conta todos).
pub const MAX_ERROR_SAMPLES: usize = 100;

impl ImportOptions {
    pub fn new(source: Option<&str>, provenance: Provenance) -> Self {
        Self {
            source: source.map(str::to_owned),
            provenance,
            batch_size: DEFAULT_BATCH,
            max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED,
        }
    }
}

/// Andamento da importação em fluxo, relatado a cada lote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportProgress {
    /// Partidas lidas do arquivo (novas, repetidas e com erro).
    pub games_seen: usize,
    pub new: usize,
    pub duplicates: usize,
    pub errors: usize,
    /// Bytes lidos do arquivo (comprimidos, no caso de `.gz`) e o tamanho total, quando conhecido.
    pub bytes_read: u64,
    pub bytes_total: Option<u64>,
    /// `true` no relato final.
    pub finished: bool,
}

/// Resumo de uma importação em fluxo: quantas eram novas, repetidas e com erro.
#[derive(Debug, Clone, Default)]
pub struct ImportSummary {
    pub new: usize,
    pub duplicates: usize,
    pub errors: usize,
    pub error_samples: Vec<PgnError>,
    /// Partidas que não eram UTF-8 e foram convertidas de Windows-1252/Latin-1.
    pub converted_games: usize,
    pub cancelled: bool,
    /// Maior trecho de arquivo mantido em memória de uma só vez.
    pub peak_buffered_bytes: usize,
}

/// Filtro de [`Store::list_imported`]; o padrão lista tudo, da mais recente à mais antiga.
/// `profile: None` lista de todos os perfis (testes/administração): a interface sempre passa o ativo.
#[derive(Debug, Clone, Default)]
pub struct ImportedQuery {
    /// Trecho (sem distinguir maiúsculas) de evento, brancas ou pretas.
    pub text: Option<String>,
    pub profile: Option<ProfileId>,
    /// Identificador do Provedor (qualquer das Origens da Partida).
    pub provider: Option<String>,
    /// Prefixo do código ECO (`C4`, `B20`).
    pub eco: Option<String>,
    pub pace: Option<Pace>,
    pub limit: Option<u32>,
}

/// Linha de listagem de uma partida importada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSummary {
    pub id: ImportedId,
    pub profile: ProfileId,
    /// Nome do arquivo importado (texto livre histórico da primeira Origem).
    pub source: Option<String>,
    pub event: Option<String>,
    pub white: Option<String>,
    pub black: Option<String>,
    pub date: Option<String>,
    /// `1-0`, `0-1`, `1/2-1/2` ou `*`.
    pub result: String,
    pub start_fen: String,
    /// Meios-lances em todas as linhas (principal e variações).
    pub ply_count: u32,
    pub variation_count: u32,
    pub imported_at_ms: i64,
    /// Provedor da primeira Origem.
    pub provider: Option<String>,
    pub import_mode: ImportMode,
    pub site: Option<String>,
    pub white_elo: Option<i64>,
    pub black_elo: Option<i64>,
    pub time_control: Option<String>,
    pub eco: Option<String>,
    pub pace: Option<Pace>,
}

/// Versão do formato JSON da árvore (para migrações futuras do modelo).
const TREE_VERSION: i64 = 1;

/// Colunas derivadas das tags de uma partida.
pub(crate) struct GameFacts {
    pub site: Option<String>,
    pub white_elo: Option<i64>,
    pub black_elo: Option<i64>,
    pub time_control: Option<String>,
    pub eco: Option<String>,
    pub pace: Option<Pace>,
}

pub(crate) fn facts(game: &PgnGame) -> GameFacts {
    let known = |name: &str| {
        game.tag(name)
            .map(str::trim)
            .filter(|v| !v.is_empty() && *v != "?" && *v != "-")
            .map(str::to_owned)
    };
    let time_control = known("TimeControl");
    GameFacts {
        site: known("Site"),
        white_elo: parse_elo(game.tag("WhiteElo")),
        black_elo: parse_elo(game.tag("BlackElo")),
        pace: Pace::derive(time_control.as_deref(), game.tag("Event")),
        time_control,
        eco: known("ECO").map(|e| e.to_uppercase()),
    }
}

fn io_error(e: std::io::Error) -> StoreError {
    match e
        .get_ref()
        .and_then(|r| r.downcast_ref::<DecompressionLimit>())
    {
        Some(DecompressionLimit(limit)) => StoreError::DecompressedTooLarge { limit: *limit },
        None => StoreError::Io(e.to_string()),
    }
}

enum Stored {
    New,
    Duplicate,
}

/// Instruções preparadas de uma transação de importação.
struct Writer<'t> {
    insert: rusqlite::Statement<'t>,
    by_fingerprint: rusqlite::Statement<'t>,
    by_remote: rusqlite::Statement<'t>,
    has_origin: rusqlite::Statement<'t>,
    add_origin: rusqlite::Statement<'t>,
}

impl<'t> Writer<'t> {
    fn new(tx: &'t rusqlite::Transaction<'_>) -> Result<Self> {
        Ok(Self {
            insert: tx.prepare(
                "INSERT INTO imported_games (profile_id, source, event, white, black, date, result,
                                             start_fen, ply_count, variation_count, tree_version,
                                             tree_json, imported_at_ms, provider, import_mode,
                                             retrieved_at_ms, license, reference_url, remote_id,
                                             fingerprint, site, white_elo, black_elo, time_control,
                                             eco, pace)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                         ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            )?,
            by_fingerprint: tx.prepare(
                "SELECT id FROM imported_games WHERE profile_id = ?1 AND fingerprint = ?2",
            )?,
            by_remote: tx.prepare(
                "SELECT id FROM imported_games
                 WHERE profile_id = ?1 AND provider = ?2 AND remote_id = ?3",
            )?,
            has_origin: tx.prepare(
                "SELECT 1 FROM (
                     SELECT provider, import_mode, remote_id, source FROM imported_games WHERE id = ?1
                     UNION ALL
                     SELECT provider, import_mode, remote_id, source FROM imported_game_origins
                     WHERE game_id = ?1)
                 WHERE provider IS ?2 AND import_mode = ?3 AND remote_id IS ?4 AND source IS ?5",
            )?,
            add_origin: tx.prepare(
                "INSERT INTO imported_game_origins (game_id, provider, import_mode, retrieved_at_ms,
                                                    license, reference_url, remote_id, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
        })
    }

    /// Grava a partida ou, se já existe, soma a Origem (quando é uma Origem nova).
    fn store(
        &mut self,
        profile: ProfileId,
        game: &PgnGame,
        options: &ImportOptions,
        now: i64,
    ) -> Result<Stored> {
        let prov = &options.provenance;
        let fingerprint = game_fingerprint(game);
        let remote = remote_id(prov.provider.as_deref(), game);
        let mut existing: Option<i64> = self
            .by_fingerprint
            .query_row(params![profile.0, fingerprint], |r| r.get(0))
            .optional()?;
        if existing.is_none()
            && let (Some(provider), Some(remote)) = (&prov.provider, &remote)
        {
            existing = self
                .by_remote
                .query_row(params![profile.0, provider, remote], |r| r.get(0))
                .optional()?;
        }
        if let Some(id) = existing {
            let known = self
                .has_origin
                .query_row(
                    params![
                        id,
                        prov.provider,
                        prov.mode.as_str(),
                        remote,
                        options.source
                    ],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !known {
                self.add_origin.execute(params![
                    id,
                    prov.provider,
                    prov.mode.as_str(),
                    prov.retrieved_at_ms,
                    prov.license,
                    prov.reference_url,
                    remote,
                    options.source,
                ])?;
            }
            return Ok(Stored::Duplicate);
        }
        let known = |name: &str| game.tag(name).filter(|v| *v != "?").map(str::to_owned);
        let f = facts(game);
        self.insert.execute(params![
            profile.0,
            options.source,
            known("Event"),
            known("White"),
            known("Black"),
            game.tag("Date").filter(|v| !v.starts_with('?')),
            game.result.token(),
            game.start_fen,
            i64::try_from(game.total_plies()).unwrap_or(i64::MAX),
            i64::try_from(game.variation_count()).unwrap_or(i64::MAX),
            TREE_VERSION,
            serde_json::to_string(game)?,
            now,
            prov.provider,
            prov.mode.as_str(),
            prov.retrieved_at_ms,
            prov.license,
            prov.reference_url,
            remote,
            fingerprint,
            f.site,
            f.white_elo,
            f.black_elo,
            f.time_control,
            f.eco,
            f.pace.map(Pace::as_str),
        ])?;
        Ok(Stored::New)
    }
}

/// Conta os bytes que passam por ele (andamento da leitura de um arquivo).
struct Counting<R> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl Store {
    /// Importa as partidas do PGN no perfil padrão. Partidas com erro são puladas e
    /// relatadas; as válidas são gravadas numa única transação.
    pub fn import_pgn(&mut self, pgn: &str, source: Option<&str>) -> Result<ImportReport> {
        let profile = self.default_profile()?;
        self.import_pgn_for(profile, pgn, source)
    }

    /// Como [`Self::import_pgn`], para um perfil. Sem Provedor: a Origem é um arquivo.
    pub fn import_pgn_for(
        &mut self,
        profile: ProfileId,
        pgn: &str,
        source: Option<&str>,
    ) -> Result<ImportReport> {
        let options = ImportOptions::new(source, Provenance::unknown(ImportMode::File, now_ms()));
        self.import_pgn_with(profile, pgn, &options)
    }

    /// Importa um texto PGN já em memória com a Origem de `options`. Partidas repetidas não
    /// são gravadas de novo (e uma Origem nova se soma à Partida existente).
    pub fn import_pgn_with(
        &mut self,
        profile: ProfileId,
        pgn: &str,
        options: &ImportOptions,
    ) -> Result<ImportReport> {
        let parsed = chess_pgn::parse_pgn_lenient(pgn);
        let now = now_ms();
        let tx = self.conn.transaction()?;
        ensure_profile(&tx, profile)?;
        let mut imported = Vec::with_capacity(parsed.games.len());
        let mut duplicates = 0;
        {
            let mut writer = Writer::new(&tx)?;
            for game in &parsed.games {
                match writer.store(profile, game, options, now)? {
                    Stored::New => imported.push(ImportedId(tx.last_insert_rowid())),
                    Stored::Duplicate => duplicates += 1,
                }
            }
        }
        tx.commit()?;
        // Indexa já na importação (Biblioteca e explorador de aberturas, C19). Melhor esforço:
        // o índice é derivado e a próxima consulta o completa se esta etapa falhar.
        let _ = self.sync_library();
        Ok(ImportReport {
            imported,
            duplicates,
            errors: parsed.errors,
        })
    }

    /// Importação em fluxo: lê o PGN de `reader` partida a partida (nunca o arquivo inteiro),
    /// grava em lotes de `options.batch_size` (uma transação por lote), relata o andamento a
    /// cada lote e para quando `cancel` vira `true` (o que já foi gravado fica). Partidas com
    /// erro entram na contagem e em até [`MAX_ERROR_SAMPLES`] exemplos.
    pub fn import_pgn_stream<R: BufRead>(
        &mut self,
        profile: ProfileId,
        reader: R,
        options: &ImportOptions,
        cancel: &AtomicBool,
        on_progress: &mut dyn FnMut(&ImportProgress),
    ) -> Result<ImportSummary> {
        self.import_stream_counted(profile, reader, options, cancel, on_progress, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn import_stream_counted<R: BufRead>(
        &mut self,
        profile: ProfileId,
        reader: R,
        options: &ImportOptions,
        cancel: &AtomicBool,
        on_progress: &mut dyn FnMut(&ImportProgress),
        bytes: Option<&Arc<AtomicU64>>,
        bytes_total: Option<u64>,
    ) -> Result<ImportSummary> {
        ensure_profile(&self.conn, profile)?;
        let batch_size = options.batch_size.max(1);
        let now = now_ms();
        let mut stream = PgnStream::new(reader);
        let mut summary = ImportSummary::default();
        let mut progress = ImportProgress {
            bytes_total,
            ..ImportProgress::default()
        };
        let mut done = false;
        while !done {
            let tx = self.conn.transaction()?;
            {
                let mut writer = Writer::new(&tx)?;
                let mut in_batch = 0;
                while in_batch < batch_size {
                    if cancel.load(Ordering::Relaxed) {
                        summary.cancelled = true;
                        done = true;
                        break;
                    }
                    let Some(item) = stream.next() else {
                        done = true;
                        break;
                    };
                    in_batch += 1;
                    progress.games_seen += 1;
                    match item.map_err(io_error)? {
                        StreamItem::Game(game) => {
                            match writer.store(profile, &game, options, now)? {
                                Stored::New => summary.new += 1,
                                Stored::Duplicate => summary.duplicates += 1,
                            }
                        }
                        StreamItem::Error(error) => {
                            summary.errors += 1;
                            if summary.error_samples.len() < MAX_ERROR_SAMPLES {
                                summary.error_samples.push(error);
                            }
                        }
                    }
                }
            }
            tx.commit()?;
            progress.new = summary.new;
            progress.duplicates = summary.duplicates;
            progress.errors = summary.errors;
            progress.bytes_read = bytes.map_or(0, |b| b.load(Ordering::Relaxed));
            progress.finished = done;
            on_progress(&progress);
        }
        summary.converted_games = stream.converted_games();
        summary.peak_buffered_bytes = stream.peak_buffered_bytes();
        // O índice da Biblioteca é derivado; indexa o que entrou (também no cancelamento).
        let _ = self.sync_library();
        Ok(summary)
    }

    /// Importa um arquivo `.pgn` ou `.pgn.gz` (detectado pelos bytes iniciais, não pelo nome)
    /// em fluxo. Só lê o arquivo; nada é escrito fora do banco.
    pub fn import_pgn_file(
        &mut self,
        profile: ProfileId,
        path: &Path,
        options: &ImportOptions,
        cancel: &AtomicBool,
        on_progress: &mut dyn FnMut(&ImportProgress),
    ) -> Result<ImportSummary> {
        let file = std::fs::File::open(path).map_err(|e| StoreError::Io(e.to_string()))?;
        let total = file.metadata().ok().map(|m| m.len());
        let count = Arc::new(AtomicU64::new(0));
        let mut raw = BufReader::new(Counting {
            inner: file,
            count: Arc::clone(&count),
        });
        let gzip = raw
            .fill_buf()
            .map_err(|e| StoreError::Io(e.to_string()))?
            .starts_with(&[0x1f, 0x8b]);
        if gzip {
            let reader = BufReader::new(Limited {
                inner: flate2::bufread::MultiGzDecoder::new(raw),
                limit: options.max_decompressed_bytes,
                left: options.max_decompressed_bytes,
            });
            self.import_stream_counted(
                profile,
                reader,
                options,
                cancel,
                on_progress,
                Some(&count),
                total,
            )
        } else {
            self.import_stream_counted(
                profile,
                raw,
                options,
                cancel,
                on_progress,
                Some(&count),
                total,
            )
        }
    }

    /// As Origens de uma partida importada do `profile`: a primeira e as somadas depois.
    pub fn origins(&self, profile: ProfileId, id: ImportedId) -> Result<Vec<Origin>> {
        let owner: Option<i64> = self
            .conn
            .query_row(
                "SELECT profile_id FROM imported_games WHERE id = ?1",
                [id.0],
                |r| r.get(0),
            )
            .optional()?;
        match owner {
            None => return Err(StoreError::ImportedGameNotFound(id)),
            Some(o) if o != profile.0 => return Err(StoreError::WrongProfile { profile }),
            Some(_) => {}
        }
        let mut stmt = self.conn.prepare(
            "SELECT provider, import_mode, retrieved_at_ms, license, reference_url, remote_id, source
             FROM (
                 SELECT 0 AS ord, 0 AS oid, provider, import_mode, retrieved_at_ms, license,
                        reference_url, remote_id, source
                 FROM imported_games WHERE id = ?1
                 UNION ALL
                 SELECT 1, id, provider, import_mode, retrieved_at_ms, license, reference_url,
                        remote_id, source
                 FROM imported_game_origins WHERE game_id = ?1)
             ORDER BY ord, oid",
        )?;
        let rows = stmt.query_map([id.0], |r| {
            let mode: String = r.get(1)?;
            Ok(Origin {
                provider: r.get(0)?,
                mode: ImportMode::parse(&mode).unwrap_or_default(),
                retrieved_at_ms: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                license: r.get(3)?,
                reference_url: r.get(4)?,
                remote_id: r.get(5)?,
                source: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Partidas importadas, da mais recente à mais antiga.
    pub fn list_imported(&self, query: &ImportedQuery) -> Result<Vec<ImportedSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile_id, source, event, white, black, date, result, start_fen,
                    ply_count, variation_count, imported_at_ms, provider, import_mode, site,
                    white_elo, black_elo, time_control, eco, pace
             FROM imported_games
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND (?2 IS NULL OR instr(lower(coalesce(event, '') || char(9) || coalesce(white, '')
                                              || char(9) || coalesce(black, '')), lower(?2)) > 0)
               AND (?4 IS NULL OR provider = ?4 OR EXISTS (
                        SELECT 1 FROM imported_game_origins o
                        WHERE o.game_id = imported_games.id AND o.provider = ?4))
               AND (?5 IS NULL OR eco LIKE ?5 || '%')
               AND (?6 IS NULL OR pace = ?6)
             ORDER BY imported_at_ms DESC, id DESC
             LIMIT ?3",
        )?;
        let limit = query.limit.map_or(-1, i64::from);
        let eco = query
            .eco
            .as_deref()
            .map(|e| e.trim().to_uppercase())
            .filter(|e| !e.is_empty());
        let rows = stmt.query_map(
            params![
                query.profile.map(|p| p.0),
                query.text,
                limit,
                query.provider,
                eco,
                query.pace.map(Pace::as_str)
            ],
            |r| {
                let mode: String = r.get(13)?;
                let pace: Option<String> = r.get(19)?;
                Ok(ImportedSummary {
                    id: ImportedId(r.get(0)?),
                    profile: ProfileId(r.get(1)?),
                    source: r.get(2)?,
                    event: r.get(3)?,
                    white: r.get(4)?,
                    black: r.get(5)?,
                    date: r.get(6)?,
                    result: r.get(7)?,
                    start_fen: r.get(8)?,
                    ply_count: r.get(9)?,
                    variation_count: r.get(10)?,
                    imported_at_ms: r.get(11)?,
                    provider: r.get(12)?,
                    import_mode: ImportMode::parse(&mode).unwrap_or_default(),
                    site: r.get(14)?,
                    white_elo: r.get(15)?,
                    black_elo: r.get(16)?,
                    time_control: r.get(17)?,
                    eco: r.get(18)?,
                    pace: pace.as_deref().and_then(Pace::parse),
                })
            },
        )?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// A árvore completa (linha principal, variações, comentários e NAGs) de uma partida
    /// do `profile`; `WrongProfile` se ela é de outro perfil.
    pub fn load_imported(&self, profile: ProfileId, id: ImportedId) -> Result<PgnGame> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT profile_id, tree_json FROM imported_games WHERE id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (owner, json) = row.ok_or(StoreError::ImportedGameNotFound(id))?;
        if owner != profile.0 {
            return Err(StoreError::WrongProfile { profile });
        }
        Ok(serde_json::from_str(&json)?)
    }

    /// Leitura sem filtro de perfil, só para o índice interno da Biblioteca.
    pub(crate) fn load_imported_any(&self, id: ImportedId) -> Result<PgnGame> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT tree_json FROM imported_games WHERE id = ?1",
                [id.0],
                |r| r.get(0),
            )
            .optional()?;
        let json = json.ok_or(StoreError::ImportedGameNotFound(id))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn delete_imported(&self, profile: ProfileId, id: ImportedId) -> Result<()> {
        self.load_imported(profile, id)?;
        self.conn.execute(
            "DELETE FROM imported_games WHERE id = ?1 AND profile_id = ?2",
            params![id.0, profile.0],
        )?;
        Ok(())
    }

    /// Preenche as colunas da migração 13 nas partidas que vieram de antes dela (impressão
    /// digital, Site, Elo, TimeControl, ECO e ritmo, lidos da árvore gravada). Idempotente:
    /// só toca linhas sem impressão digital. Repetidas antigas ganham uma impressão própria
    /// (`dup:<id>`) para não serem apagadas nem barrar o índice único.
    pub(crate) fn backfill_imported_facts(&mut self) -> Result<()> {
        loop {
            let rows: Vec<(i64, i64, String)> = {
                let mut stmt = self.conn.prepare(
                    "SELECT id, profile_id, tree_json FROM imported_games
                     WHERE fingerprint IS NULL ORDER BY id LIMIT 500",
                )?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                rows.collect::<std::result::Result<_, _>>()?
            };
            if rows.is_empty() {
                return Ok(());
            }
            let tx = self.conn.transaction()?;
            for (id, profile, json) in rows {
                let Ok(game) = serde_json::from_str::<PgnGame>(&json) else {
                    tx.execute(
                        "UPDATE imported_games SET fingerprint = 'bad:' || id WHERE id = ?1",
                        [id],
                    )?;
                    continue;
                };
                let f = facts(&game);
                let fingerprint = game_fingerprint(&game);
                let taken: bool = tx
                    .query_row(
                        "SELECT 1 FROM imported_games WHERE profile_id = ?1 AND fingerprint = ?2",
                        params![profile, fingerprint],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                let fingerprint = if taken {
                    format!("dup:{id}")
                } else {
                    fingerprint
                };
                tx.execute(
                    "UPDATE imported_games SET fingerprint = ?2, site = ?3, white_elo = ?4,
                         black_elo = ?5, time_control = ?6, eco = ?7, pace = ?8 WHERE id = ?1",
                    params![
                        id,
                        fingerprint,
                        f.site,
                        f.white_elo,
                        f.black_elo,
                        f.time_control,
                        f.eco,
                        f.pace.map(Pace::as_str),
                    ],
                )?;
            }
            tx.commit()?;
        }
    }
}
