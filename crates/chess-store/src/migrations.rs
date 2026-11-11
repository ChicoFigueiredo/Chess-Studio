//! Migrações versionadas, embutidas no binário. A versão fica em `PRAGMA
//! user_version`; cada migração roda na sua própria transação (DDL do SQLite é
//! transacional), então uma falha no meio deixa o banco na versão anterior.
//! Regra: migrações já publicadas nunca são editadas, só se acrescenta uma nova.

use crate::error::{Result, StoreError};
use rusqlite::Connection;

const V1_CORE: &str = "
CREATE TABLE profiles (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE,
    created_at_ms INTEGER NOT NULL
);
-- DEC-50: um perfil local no P0; o esquema já suporta vários.
INSERT INTO profiles (id, name, created_at_ms) VALUES (1, 'local', 0);

CREATE TABLE ui_preferences (
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value_json TEXT NOT NULL,
    PRIMARY KEY (profile_id, key)
) WITHOUT ROWID;

CREATE TABLE games (
    id            INTEGER PRIMARY KEY,
    profile_id    INTEGER NOT NULL REFERENCES profiles(id),
    policy        TEXT NOT NULL,
    start_fen     TEXT NOT NULL,
    setup_json    TEXT NOT NULL,
    title         TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    -- 1 enquanto a sessão está aberta; 0 após fechamento limpo. Se a reabertura
    -- encontra 1, o processo anterior morreu (fechamento abrupto).
    session_open  INTEGER NOT NULL DEFAULT 1 CHECK (session_open IN (0, 1)),
    -- Pausa (DEC-42): a Partida reabre pausada. Metadado enquanto o chess-game não tem Pausa.
    paused        INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0, 1)),
    finished      INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1)),
    outcome_json  TEXT
);
CREATE INDEX games_profile_recent ON games (profile_id, updated_at_ms DESC, id DESC);
CREATE INDEX games_unfinished ON games (finished, updated_at_ms DESC, id DESC);

CREATE TABLE game_events (
    game_id      INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    seq          INTEGER NOT NULL,
    version      INTEGER NOT NULL,
    kind         TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    PRIMARY KEY (game_id, seq)
) WITHOUT ROWID;
";

const V2_TRAINING_AND_ENGINES: &str = "
CREATE TABLE exercises (
    id            INTEGER PRIMARY KEY,
    kind          TEXT NOT NULL,
    initial_fen   TEXT NOT NULL,
    solution_json TEXT NOT NULL,
    version       INTEGER NOT NULL DEFAULT 1,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX exercises_kind ON exercises (kind);

-- Resoluções (tentativas de Exercício).
CREATE TABLE exercise_runs (
    id            INTEGER PRIMARY KEY,
    exercise_id   INTEGER NOT NULL REFERENCES exercises(id) ON DELETE CASCADE,
    profile_id    INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    correct       INTEGER NOT NULL CHECK (correct IN (0, 1)),
    hints_used    INTEGER NOT NULL DEFAULT 0,
    duration_ms   INTEGER NOT NULL,
    reviewed      INTEGER NOT NULL DEFAULT 0 CHECK (reviewed IN (0, 1)),
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX exercise_runs_by_exercise ON exercise_runs (exercise_id, created_at_ms);
CREATE INDEX exercise_runs_by_profile ON exercise_runs (profile_id, created_at_ms);

-- Posições relevantes (FEN) ligadas a Partida ou Exercício.
CREATE TABLE positions (
    id          INTEGER PRIMARY KEY,
    fen         TEXT NOT NULL,
    game_id     INTEGER REFERENCES games(id) ON DELETE CASCADE,
    exercise_id INTEGER REFERENCES exercises(id) ON DELETE CASCADE,
    marker      TEXT
);
CREATE INDEX positions_game ON positions (game_id);
CREATE INDEX positions_exercise ON positions (exercise_id);

-- Engines conhecidas; nunca o binário no SQLite.
CREATE TABLE engine_installations (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    version  TEXT,
    path     TEXT NOT NULL UNIQUE,
    protocol TEXT NOT NULL DEFAULT 'uci',
    checksum TEXT,
    status   TEXT NOT NULL DEFAULT 'unknown'
);
";

// Esquema de Exercícios do B13: identificador estável (`slug`), definição completa do
// catálogo e o registro imutável de cada Resolução. As linhas antigas são preservadas
// (`finished` = 1, sem `summary_json`, ficam fora das listagens).
const V3_EXERCISE_API: &str = "
ALTER TABLE exercises ADD COLUMN slug TEXT;
ALTER TABLE exercises ADD COLUMN definition_json TEXT NOT NULL DEFAULT '{}';
UPDATE exercises SET slug = 'legacy-' || id WHERE slug IS NULL;
CREATE UNIQUE INDEX exercises_slug ON exercises (slug);

ALTER TABLE exercise_runs ADD COLUMN finished INTEGER NOT NULL DEFAULT 1 CHECK (finished IN (0, 1));
ALTER TABLE exercise_runs ADD COLUMN errors INTEGER NOT NULL DEFAULT 0;
ALTER TABLE exercise_runs ADD COLUMN session INTEGER NOT NULL DEFAULT 0;
ALTER TABLE exercise_runs ADD COLUMN summary_json TEXT NOT NULL DEFAULT '{}';
";

// Partidas importadas de PGN (#48): a árvore de variações (JSON) mais colunas de
// listagem. Só acrescenta uma tabela; nada do que existe é alterado (DB-06).
const V4_IMPORTED_GAMES: &str = "
CREATE TABLE imported_games (
    id              INTEGER PRIMARY KEY,
    profile_id      INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    source          TEXT,
    event           TEXT,
    white           TEXT,
    black           TEXT,
    date            TEXT,
    result          TEXT NOT NULL,
    start_fen       TEXT NOT NULL,
    ply_count       INTEGER NOT NULL,
    variation_count INTEGER NOT NULL,
    tree_version    INTEGER NOT NULL DEFAULT 1,
    tree_json       TEXT NOT NULL,
    imported_at_ms  INTEGER NOT NULL
);
CREATE INDEX imported_games_recent ON imported_games (profile_id, imported_at_ms DESC, id DESC);
";

// Gerenciador de Engines (C9): origem, catálogo, pasta instalada e configuração. Registros
// antigos viram `manual` (nunca apagáveis pelo app) e ficam com configuração vazia.
const V5_ENGINE_MANAGER: &str = "
ALTER TABLE engine_installations ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual'
    CHECK (origin IN ('catalog', 'detected', 'manual'));
ALTER TABLE engine_installations ADD COLUMN catalog_id TEXT;
ALTER TABLE engine_installations ADD COLUMN variant TEXT;
ALTER TABLE engine_installations ADD COLUMN install_dir TEXT;
ALTER TABLE engine_installations ADD COLUMN settings_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE engine_installations ADD COLUMN installed_at_ms INTEGER NOT NULL DEFAULT 0;
";

// Perfis de treino (#53): avanço por passo cumprido e Perfil ativo de cada jogador.
// Só acrescenta tabelas; nada do que existe é alterado (DB-06).
const V6_TRAINING_PROFILES: &str = "
CREATE TABLE training_profile_progress (
    profile_id       INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    training_profile TEXT NOT NULL,
    exercise         TEXT NOT NULL,
    completed_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (profile_id, training_profile, exercise)
);
CREATE TABLE training_profile_active (
    profile_id       INTEGER PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    training_profile TEXT NOT NULL
);
";

// Biblioteca pesquisável e posições favoritas (#49). O índice é derivado (reconstruível a
// partir de `games`/`game_events`/`imported_games`) e preenchido sob demanda, então um banco
// antigo é indexado no primeiro uso. Só acrescenta tabelas; nada do que existe é alterado (DB-06).
const V7_LIBRARY: &str = "
CREATE TABLE library_entries (
    id         INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    source     TEXT NOT NULL CHECK (source IN ('played', 'imported')),
    ref_id     INTEGER NOT NULL,
    -- Muda quando a Partida ganha Eventos ou termina; força a reindexação.
    signature  INTEGER NOT NULL,
    white      TEXT,
    black      TEXT,
    event      TEXT,
    date       TEXT,
    date_lo    TEXT,
    date_hi    TEXT,
    players    TEXT NOT NULL DEFAULT '',
    result     TEXT NOT NULL,
    reason     TEXT,
    policy     TEXT,
    opponent   TEXT,
    ply_count  INTEGER NOT NULL,
    opening    TEXT,
    finished   INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1)),
    sort_ms    INTEGER NOT NULL,
    UNIQUE (source, ref_id)
);
CREATE INDEX library_entries_recent ON library_entries (profile_id, sort_ms DESC, id DESC);
CREATE INDEX library_entries_result ON library_entries (result);
CREATE INDEX library_entries_policy ON library_entries (policy);
CREATE INDEX library_entries_dates ON library_entries (date_lo, date_hi);

-- Texto livre: `rowid` = `library_entries.id`.
CREATE VIRTUAL TABLE library_fts USING fts5(body, tokenize = 'unicode61 remove_diacritics 2');

-- Posições (chave normalizada) de cada Partida: linha principal e variações.
CREATE TABLE library_positions (
    entry_id INTEGER NOT NULL REFERENCES library_entries(id) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    ply      INTEGER NOT NULL,
    main     INTEGER NOT NULL CHECK (main IN (0, 1)),
    PRIMARY KEY (entry_id, key, ply)
) WITHOUT ROWID;
CREATE INDEX library_positions_key ON library_positions (key);

CREATE TABLE favorite_positions (
    id            INTEGER PRIMARY KEY,
    profile_id    INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    fen           TEXT NOT NULL,
    position_key  TEXT NOT NULL,
    name          TEXT NOT NULL,
    note          TEXT NOT NULL DEFAULT '',
    origin_source TEXT CHECK (origin_source IN ('played', 'imported')),
    origin_ref    INTEGER,
    origin_ply    INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (profile_id, position_key)
);
CREATE INDEX favorite_positions_recent ON favorite_positions (profile_id, updated_at_ms DESC, id DESC);
";

// Vários perfis locais (#54): o perfil usado por último abre primeiro. Só acrescenta uma
// coluna; o perfil existente continua o ativo (empate -> o de menor id).
const V8_LOCAL_PROFILES: &str = "
ALTER TABLE profiles ADD COLUMN last_used_at_ms INTEGER NOT NULL DEFAULT 0;
";

// Desfazer após o fim reabre a Partida (#115): `finished`/`outcome_json` passam a ser derivados
// do último Evento terminal (`game_ended`) ou de reabertura (`move_undone`) do log. Recalcula
// os bancos que ficaram com o flag errado.
const V9_REOPEN_AFTER_UNDO: &str = "
UPDATE games SET outcome_json = (
    SELECT CASE WHEN e.kind = 'game_ended' THEN json_extract(e.payload_json, '$.kind.outcome') END
    FROM game_events e
    WHERE e.game_id = games.id AND e.kind IN ('game_ended', 'move_undone')
    ORDER BY e.seq DESC LIMIT 1);
UPDATE games SET finished = (outcome_json IS NOT NULL);
";

// Súmula manual (V2-SUM-01, #58): registro separado, por perfil local e por Partida. Só
// acrescenta tabelas; não toca `games` nem `game_events` (a Súmula nunca altera a Partida,
// o log nem o PGN) e some junto com a Partida ou o perfil (cascata).
const V10_MANUAL_SCORESHEET: &str = "
CREATE TABLE manual_scoresheets (
    id              INTEGER PRIMARY KEY,
    profile_id      INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    game_id         INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL,
    -- NULL: em anotação; preenchido: entregue (imutável, pronta para a conferência).
    delivered_at_ms INTEGER,
    UNIQUE (profile_id, game_id)
);
CREATE TABLE manual_scoresheet_entries (
    scoresheet_id INTEGER NOT NULL REFERENCES manual_scoresheets(id) ON DELETE CASCADE,
    ply           INTEGER NOT NULL CHECK (ply >= 1),
    text          TEXT NOT NULL,
    PRIMARY KEY (scoresheet_id, ply)
) WITHOUT ROWID;
";

// Conferência da Súmula (V2-SUM-02, #59): o resultado da conferência da Súmula entregue, um
// registro à parte (nunca toca `games`, `game_events` nem a própria Súmula). Guarda por
// meio-lance o texto escrito, o SAN real e a classe, pronto para o histórico (E3). Some junto
// com a Súmula (cascata).
const V11_SCORESHEET_CHECK: &str = "
CREATE TABLE manual_scoresheet_checks (
    scoresheet_id INTEGER PRIMARY KEY REFERENCES manual_scoresheets(id) ON DELETE CASCADE,
    checked_at_ms INTEGER NOT NULL,
    -- Histórico conferido: nº de meios-lances e identificador (FNV-1a, u64 guardado como i64);
    -- a conferência é refeita quando o histórico da Partida muda.
    history_plies INTEGER NOT NULL CHECK (history_plies >= 0),
    history_id    INTEGER NOT NULL
);
CREATE TABLE manual_scoresheet_check_entries (
    scoresheet_id INTEGER NOT NULL REFERENCES manual_scoresheet_checks(scoresheet_id) ON DELETE CASCADE,
    ply           INTEGER NOT NULL CHECK (ply >= 1),
    written       TEXT,
    expected      TEXT,
    verdict       TEXT NOT NULL CHECK (verdict IN
        ('correct', 'accepted', 'wrong_notation', 'swapped', 'missing', 'extra')),
    PRIMARY KEY (scoresheet_id, ply)
) WITHOUT ROWID;
";

// Explorador de aberturas (C19, #147): para cada lance da linha principal de cada Partida da
// Biblioteca (até 40 meios-lances), a Posição de onde ele sai (hash FNV-1a de 64 bits da chave
// normalizada, guardado como i64), o lance em SAN, o resultado e o instante de uso. É um índice
// derivado, alimentado junto com `library_entries` (some em cascata com a entrada e o perfil).
// As entradas da Biblioteca já indexadas são descartadas para serem reindexadas no primeiro uso
// (o índice é reconstruível; nenhuma Partida é tocada, DB-06).
const V12_OPENING_EXPLORER: &str = "
DELETE FROM library_fts;
DELETE FROM library_positions;
DELETE FROM library_entries;
CREATE TABLE opening_moves (
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    pos_hash   INTEGER NOT NULL,
    san        TEXT NOT NULL,
    entry_id   INTEGER NOT NULL REFERENCES library_entries(id) ON DELETE CASCADE,
    result     INTEGER NOT NULL CHECK (result IN (0, 1, 2, 3)),
    used_ms    INTEGER NOT NULL,
    PRIMARY KEY (profile_id, pos_hash, san, entry_id)
) WITHOUT ROWID;
CREATE INDEX opening_moves_entry ON opening_moves (entry_id);
";

// Origem estruturada, impressão digital e metadados das Partidas importadas (CX-4, #216).
// Só acrescenta colunas, índices e uma tabela: o texto livre `source` continua intacto e
// vira a primeira Origem (modo arquivo, instante = o da importação). A impressão digital e as
// colunas derivadas das tags (Site, Elo, TimeControl, ECO, ritmo) das linhas antigas são
// preenchidas ao abrir o banco (`Store::backfill_imported_facts`), porque dependem da árvore.
const V13_IMPORT_ORIGIN: &str = "
ALTER TABLE imported_games ADD COLUMN provider TEXT;
ALTER TABLE imported_games ADD COLUMN import_mode TEXT NOT NULL DEFAULT 'file'
    CHECK (import_mode IN ('file', 'manual', 'api'));
ALTER TABLE imported_games ADD COLUMN retrieved_at_ms INTEGER;
ALTER TABLE imported_games ADD COLUMN license TEXT;
ALTER TABLE imported_games ADD COLUMN reference_url TEXT;
ALTER TABLE imported_games ADD COLUMN remote_id TEXT;
ALTER TABLE imported_games ADD COLUMN fingerprint TEXT;
ALTER TABLE imported_games ADD COLUMN site TEXT;
ALTER TABLE imported_games ADD COLUMN white_elo INTEGER;
ALTER TABLE imported_games ADD COLUMN black_elo INTEGER;
ALTER TABLE imported_games ADD COLUMN time_control TEXT;
ALTER TABLE imported_games ADD COLUMN eco TEXT;
ALTER TABLE imported_games ADD COLUMN pace TEXT;
UPDATE imported_games SET retrieved_at_ms = imported_at_ms;
CREATE UNIQUE INDEX imported_games_fingerprint
    ON imported_games (profile_id, fingerprint) WHERE fingerprint IS NOT NULL;
CREATE UNIQUE INDEX imported_games_remote
    ON imported_games (profile_id, provider, remote_id)
    WHERE provider IS NOT NULL AND remote_id IS NOT NULL;
CREATE INDEX imported_games_provider ON imported_games (provider);
CREATE INDEX imported_games_eco ON imported_games (eco);
CREATE INDEX imported_games_pace ON imported_games (pace);

-- Origens somadas depois da primeira: a mesma Partida vinda de outro Provedor ou arquivo.
CREATE TABLE imported_game_origins (
    id              INTEGER PRIMARY KEY,
    game_id         INTEGER NOT NULL REFERENCES imported_games(id) ON DELETE CASCADE,
    provider        TEXT,
    import_mode     TEXT NOT NULL CHECK (import_mode IN ('file', 'manual', 'api')),
    retrieved_at_ms INTEGER NOT NULL,
    license         TEXT,
    reference_url   TEXT,
    remote_id       TEXT,
    source          TEXT
);
CREATE INDEX imported_game_origins_game ON imported_game_origins (game_id);
CREATE INDEX imported_game_origins_provider ON imported_game_origins (provider);
";

// Pacotes de Exercícios externos (CX-5, #217): Origem (`bundled` ou o Provedor, ex.:
// `lichess`), pacote a que o Exercício pertence e marca de oculto. Um Exercício de pacote
// removido que já tem Resolução fica oculto (`hidden = 1`) para a Resolução, que referencia
// `exercises(id)` com ON DELETE CASCADE, não ser apagada junto.
const V14_EXERCISE_PACKS: &str = "
ALTER TABLE exercises ADD COLUMN origin TEXT NOT NULL DEFAULT 'bundled';
ALTER TABLE exercises ADD COLUMN pack TEXT;
ALTER TABLE exercises ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1));
CREATE INDEX exercises_pack ON exercises (pack) WHERE pack IS NOT NULL;
";

/// Migrações em ordem; o índice + 1 é a versão resultante.
const MIGRATIONS: &[&str] = &[
    V1_CORE,
    V2_TRAINING_AND_ENGINES,
    V3_EXERCISE_API,
    V4_IMPORTED_GAMES,
    V5_ENGINE_MANAGER,
    V6_TRAINING_PROFILES,
    V7_LIBRARY,
    V8_LOCAL_PROFILES,
    V9_REOPEN_AFTER_UNDO,
    V10_MANUAL_SCORESHEET,
    V11_SCORESHEET_CHECK,
    V12_OPENING_EXPLORER,
    V13_IMPORT_ORIGIN,
    V14_EXERCISE_PACKS,
];

/// Versão de esquema desta build.
pub const LATEST_VERSION: u32 = MIGRATIONS.len() as u32;

pub fn current_version(conn: &Connection) -> Result<u32> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Leva o banco até `target` (no máximo [`LATEST_VERSION`]).
pub fn migrate_to(conn: &mut Connection, target: u32) -> Result<()> {
    let found = current_version(conn)?;
    if found > LATEST_VERSION {
        return Err(StoreError::SchemaTooNew {
            found,
            supported: LATEST_VERSION,
        });
    }
    for version in found + 1..=target.min(LATEST_VERSION) {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATIONS[version as usize - 1])?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    migrate_to(conn, LATEST_VERSION)
}
