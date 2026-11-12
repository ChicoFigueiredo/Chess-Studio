//! Origem (`Provenance`, glossário) das Partidas importadas e a impressão digital que
//! detecta duplicatas (CX-4, #216; decisões CX-14).
//!
//! # Impressão digital
//!
//! Texto canônico = jogadores normalizados (minúsculas, sem acento, só letras e números,
//! palavras em ordem alfabética, então `Carlsen, Magnus` = `magnus carlsen`; Brancas e
//! Pretas ficam em campos separados, então cores trocadas são Partidas diferentes) + data
//! (`Date`, ou `UTCDate` quando a primeira falta) + resultado + Posição inicial + Lances da
//! linha principal em SAN. Comentários, NAGs, variações, Elo e demais tags **não** entram: o
//! mesmo jogo baixado de dois lugares, com anotações diferentes, é o mesmo jogo. O texto é
//! resumido em 128 bits (dois FNV-1a de 64 bits com sementes diferentes), em hexadecimal.
//! Quando o Provedor dá um identificador remoto (`Site` do Lichess, `Link` do Chess.com), o
//! par `(Provedor, id remoto)` também identifica a Partida.

use chess_pgn::PgnGame;

/// Como a Partida chegou: arquivo escolhido pelo usuário, importação manual guiada (o
/// usuário disse de onde veio o arquivo) ou API de um Provedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImportMode {
    #[default]
    File,
    Manual,
    Api,
}

impl ImportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Manual => "manual",
            Self::Api => "api",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "file" => Some(Self::File),
            "manual" => Some(Self::Manual),
            "api" => Some(Self::Api),
            _ => None,
        }
    }
}

/// De onde o usuário diz que o arquivo PGN veio (importação manual guiada).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportSource {
    QueenAlice,
    ChessBase,
    Twic,
    PgnMentor,
    Lichess,
    ChessCom,
    PlayStrategy,
    Other,
}

impl ImportSource {
    pub const ALL: [Self; 8] = [
        Self::QueenAlice,
        Self::ChessBase,
        Self::Twic,
        Self::PgnMentor,
        Self::Lichess,
        Self::ChessCom,
        Self::PlayStrategy,
        Self::Other,
    ];

    /// Identificador estável do Provedor (gravado no banco).
    pub fn provider_id(self) -> &'static str {
        match self {
            Self::QueenAlice => "queen-alice",
            Self::ChessBase => "chessbase",
            Self::Twic => "twic",
            Self::PgnMentor => "pgn-mentor",
            Self::Lichess => "lichess",
            Self::ChessCom => "chess-com",
            Self::PlayStrategy => "playstrategy",
            Self::Other => "other",
        }
    }

    /// Código estável da licença dos dados daquela fonte (a interface traduz o texto).
    pub fn license_code(self) -> &'static str {
        match self {
            Self::QueenAlice | Self::ChessBase | Self::Other => "user-supplied",
            Self::Twic => "twic-personal",
            Self::PgnMentor => "pgn-mentor-free",
            Self::Lichess => "lichess-cc0",
            Self::ChessCom => "chesscom-terms",
            Self::PlayStrategy => "playstrategy-terms",
        }
    }

    /// Fontes de uso restrito: o app não conhece direito de redistribuição, então a pessoa
    /// confirma que pode usar as Partidas antes de importar (#270).
    pub fn is_restricted(self) -> bool {
        matches!(self, Self::ChessBase | Self::Twic | Self::PgnMentor)
    }

    /// A fonte que as tags da Partida indicam (`Site`, `LichessURL`, `Link`), quando indicam.
    /// Detecção parcial: TWIC, PGN Mentor e ChessBase não deixam marca nas tags.
    pub fn detect(game: &PgnGame) -> Option<Self> {
        let has = |tag: &str, host: &str| game.tag(tag).is_some_and(|v| v.contains(host));
        if has("Site", "playstrategy.org/") {
            Some(Self::PlayStrategy)
        } else if has("Site", "lichess.org/") || has("LichessURL", "lichess.org/") {
            Some(Self::Lichess)
        } else if has("Link", "chess.com/") || has("Site", "chess.com/") {
            Some(Self::ChessCom)
        } else {
            None
        }
    }

    pub fn from_provider_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.provider_id() == id)
    }
}

/// A Origem de uma importação: Provedor, modo, instante, licença e referência. O
/// identificador remoto é por Partida e vem das tags (ver [`remote_id`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub provider: Option<String>,
    pub mode: ImportMode,
    /// Instante da consulta/importação, injetado por quem chama (ms desde a época Unix).
    pub retrieved_at_ms: i64,
    pub license: Option<String>,
    pub reference_url: Option<String>,
}

impl Provenance {
    /// Origem de um arquivo sem Provedor conhecido.
    pub fn unknown(mode: ImportMode, retrieved_at_ms: i64) -> Self {
        Self {
            provider: None,
            mode,
            retrieved_at_ms,
            license: None,
            reference_url: None,
        }
    }

    pub fn from_source(source: ImportSource, mode: ImportMode, retrieved_at_ms: i64) -> Self {
        Self {
            provider: Some(source.provider_id().to_owned()),
            mode,
            retrieved_at_ms,
            license: Some(source.license_code().to_owned()),
            reference_url: None,
        }
    }
}

/// Uma Origem gravada de uma Partida importada. Uma Partida tem uma ou mais (a mesma
/// Partida chegando de dois Provedores soma uma Origem, não uma cópia).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub provider: Option<String>,
    pub mode: ImportMode,
    pub retrieved_at_ms: i64,
    pub license: Option<String>,
    pub reference_url: Option<String>,
    pub remote_id: Option<String>,
    /// Texto livre histórico: o nome do arquivo importado.
    pub source: Option<String>,
}

/// Ritmo de jogo, derivado de `TimeControl` (ou do nome do evento).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pace {
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Correspondence,
}

impl Pace {
    pub const ALL: [Self; 5] = [
        Self::Bullet,
        Self::Blitz,
        Self::Rapid,
        Self::Classical,
        Self::Correspondence,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Blitz => "blitz",
            Self::Rapid => "rapid",
            Self::Classical => "classical",
            Self::Correspondence => "correspondence",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }

    /// `TimeControl` do PGN (`300+3`, `300`, `40/7200:3600`, `1/86400`, `-`) ou, na falta
    /// dele, a palavra no nome do evento (`Rated Blitz game`). Mesmos cortes do Lichess:
    /// tempo estimado = base + 40 x incremento; < 180 s bullet, < 480 s blitz, < 1500 s
    /// rápido, o resto clássico.
    pub fn derive(time_control: Option<&str>, event: Option<&str>) -> Option<Self> {
        if let Some(tc) = time_control.map(str::trim)
            && let Some(p) = Self::from_time_control(tc)
        {
            return Some(p);
        }
        let event = event?.to_lowercase();
        [
            ("bullet", Self::Bullet),
            ("blitz", Self::Blitz),
            ("rapid", Self::Rapid),
            ("classical", Self::Classical),
            ("correspondence", Self::Correspondence),
        ]
        .into_iter()
        .find(|(word, _)| event.contains(word))
        .map(|(_, p)| p)
    }

    fn from_time_control(tc: &str) -> Option<Self> {
        if tc.is_empty() || tc == "-" || tc == "?" {
            return None;
        }
        // Mais de um período: `40/7200:3600`; um lance por dia ou mais: correspondência.
        if let Some((moves, rest)) = tc.split_once('/') {
            let seconds: u64 = rest.split(':').next()?.parse().ok()?;
            let moves: u64 = moves.parse().ok()?;
            return Some(if moves <= 1 && seconds >= 86_400 {
                Self::Correspondence
            } else {
                Self::Classical
            });
        }
        let (base, inc) = match tc.split_once('+') {
            Some((b, i)) => (b, i.parse::<u64>().ok()?),
            None => (tc, 0),
        };
        let estimate = base.parse::<u64>().ok()?.saturating_add(40 * inc);
        Some(match estimate {
            0..180 => Self::Bullet,
            180..480 => Self::Blitz,
            480..1500 => Self::Rapid,
            _ => Self::Classical,
        })
    }
}

/// Elo de uma tag (`2100`); `?`, `-` e texto viram `None`.
pub(crate) fn parse_elo(value: Option<&str>) -> Option<i64> {
    value?.trim().parse::<i64>().ok().filter(|e| *e > 0)
}

/// Identificador da Partida no Provedor, quando as tags o trazem.
/// O id de Partida da família do Lichess: `Site` (ou a tag alternativa) com o endereço do
/// Provedor; o id tem 8 caracteres e a URL pode trazer o lado (`/black`).
fn lila_id(game: &PgnGame, host: &str, alt_tag: &str) -> Option<String> {
    let site = game.tag("Site").filter(|s| s.contains(host));
    let id = site.or_else(|| game.tag(alt_tag))?;
    let tail = id.split(['?', '#']).next()?.trim_end_matches('/');
    let id = tail.rsplit('/').next()?;
    let id: String = id.chars().take(8).collect();
    (id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some(id)
}

pub fn remote_id(provider: Option<&str>, game: &PgnGame) -> Option<String> {
    let url_tail = |tag: &str| -> Option<String> {
        let url = game.tag(tag)?;
        let path = url.split(['?', '#']).next()?;
        path.trim_end_matches('/')
            .rsplit('/')
            .next()
            .map(str::to_owned)
    };
    match provider? {
        "lichess" => lila_id(game, "lichess.org/", "LichessURL"),
        // Mesma família de API do Lichess: o `Site` aponta para a Partida em playstrategy.org.
        "playstrategy" => lila_id(game, "playstrategy.org/", "LichessURL"),
        "chess-com" => {
            let id = url_tail("Link").or_else(|| {
                game.tag("Site")
                    .filter(|s| s.contains("chess.com/"))
                    .and_then(|_| url_tail("Site"))
            })?;
            (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then_some(id)
        }
        _ => None,
    }
}

fn fold_char(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' | 'ń' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'š' | 'ś' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        c => c,
    }
}

/// Nome de jogador normalizado: minúsculas, sem acento, só letras e números, palavras em
/// ordem alfabética. `?` e vazio ficam vazios.
pub(crate) fn normalize_player(name: &str) -> String {
    let folded: String = name
        .to_lowercase()
        .chars()
        .map(fold_char)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut words: Vec<&str> = folded.split_whitespace().collect();
    words.sort_unstable();
    words.join(" ")
}

fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    let mut hash = seed;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Impressão digital estável da Partida (ver a documentação do módulo).
pub fn game_fingerprint(game: &PgnGame) -> String {
    let date = game
        .tag("Date")
        .filter(|d| d.chars().any(|c| c.is_ascii_digit()))
        .or_else(|| game.tag("UTCDate"))
        .unwrap_or("")
        .replace(['-', '/'], ".");
    let canonical = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        normalize_player(game.tag("White").unwrap_or("")),
        normalize_player(game.tag("Black").unwrap_or("")),
        date,
        game.result.token(),
        game.start_fen,
        game.mainline_sans().join(" "),
    );
    let bytes = canonical.as_bytes();
    format!(
        "{:016x}{:016x}",
        fnv1a(0xcbf2_9ce4_8422_2325, bytes),
        fnv1a(0x9e37_79b9_7f4a_7c15, bytes)
    )
}
