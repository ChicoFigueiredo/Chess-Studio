//! Tipos e (de)serialização do protocolo UCI. Sem E/S.

use std::fmt::Write as _;
use std::time::Duration;

use chess_core::{CanonicalPosition, Move};

/// Posição enviada à Engine: ponto de partida mais Lances jogados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionSpec {
    /// `None` = posição inicial; `Some(fen)` = FEN.
    pub fen: Option<String>,
    pub moves: Vec<Move>,
}

impl PositionSpec {
    pub fn startpos() -> Self {
        Self {
            fen: None,
            moves: Vec::new(),
        }
    }

    pub fn from_fen(fen: impl Into<String>) -> Self {
        Self {
            fen: Some(fen.into()),
            moves: Vec::new(),
        }
    }

    /// Posição canônica sem histórico (a Engine perde a contagem de repetições).
    pub fn from_position(position: &CanonicalPosition) -> Self {
        Self::from_fen(position.to_fen())
    }

    pub fn with_moves(mut self, moves: impl IntoIterator<Item = Move>) -> Self {
        self.moves.extend(moves);
        self
    }

    pub(crate) fn command(&self) -> String {
        let mut cmd = match &self.fen {
            None => String::from("position startpos"),
            Some(fen) => format!("position fen {fen}"),
        };
        if !self.moves.is_empty() {
            cmd.push_str(" moves");
            for m in &self.moves {
                let _ = write!(cmd, " {m}");
            }
        }
        cmd
    }
}

/// Relógio de partida enviado em `go wtime/btime/winc/binc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    pub wtime: Duration,
    pub btime: Duration,
    pub winc: Duration,
    pub binc: Duration,
}

/// Parâmetros de `go`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GoParams {
    pub clock: Option<Clock>,
    pub movetime: Option<Duration>,
    pub depth: Option<u32>,
    pub infinite: bool,
}

impl GoParams {
    pub fn movetime(movetime: Duration) -> Self {
        Self {
            movetime: Some(movetime),
            ..Self::default()
        }
    }

    pub fn depth(depth: u32) -> Self {
        Self {
            depth: Some(depth),
            ..Self::default()
        }
    }

    pub fn clock(clock: Clock) -> Self {
        Self {
            clock: Some(clock),
            ..Self::default()
        }
    }

    /// Busca sem limite; termina com [`crate::Engine::stop`].
    pub fn infinite() -> Self {
        Self {
            infinite: true,
            ..Self::default()
        }
    }

    pub(crate) fn command(&self) -> String {
        let mut cmd = String::from("go");
        if let Some(c) = &self.clock {
            let _ = write!(
                cmd,
                " wtime {} btime {} winc {} binc {}",
                c.wtime.as_millis(),
                c.btime.as_millis(),
                c.winc.as_millis(),
                c.binc.as_millis()
            );
        }
        if let Some(t) = self.movetime {
            let _ = write!(cmd, " movetime {}", t.as_millis());
        }
        if let Some(d) = self.depth {
            let _ = write!(cmd, " depth {d}");
        }
        if self.infinite {
            cmd.push_str(" infinite");
        }
        cmd
    }
}

/// Resposta `bestmove`. O texto é guardado como veio: quem decide se é um Lance
/// legal é o núcleo de regras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BestMove {
    Move {
        text: String,
        ponder: Option<String>,
    },
    /// `bestmove (none)` (ou `0000`): sem Lance (mate, afogamento).
    None,
}

impl BestMove {
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        if words.next()? != "bestmove" {
            return None;
        }
        let text = words.next()?;
        if text == "(none)" || text == "0000" {
            return Some(Self::None);
        }
        let ponder = match (words.next(), words.next()) {
            (Some("ponder"), Some(p)) => Some(p.to_owned()),
            _ => None,
        };
        Some(Self::Move {
            text: text.to_owned(),
            ponder,
        })
    }
}

/// Uma opção anunciada em `option name ... type ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOption {
    pub name: String,
    /// `check`, `spin`, `combo`, `button` ou `string`.
    pub kind: String,
    /// Restante da linha depois de `type <kind>` (default, min, max, var...).
    pub details: String,
}

impl EngineOption {
    /// Interpreta uma linha `option name … type …` (como anunciada no handshake).
    pub fn parse_line(line: &str) -> Option<Self> {
        Self::parse(line)
    }

    pub(crate) fn parse(line: &str) -> Option<Self> {
        let rest = line.strip_prefix("option name ")?;
        let (name, after) = rest.split_once(" type ")?;
        let mut parts = after.splitn(2, ' ');
        let kind = parts.next()?.to_owned();
        let details = parts.next().unwrap_or("").to_owned();
        Some(Self {
            name: name.trim().to_owned(),
            kind,
            details,
        })
    }
}

/// O que a Engine disse de si no handshake.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EngineInfo {
    pub name: Option<String>,
    pub author: Option<String>,
    pub options: Vec<EngineOption>,
}

/// Quem escreveu a linha do log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    /// Enviada por nós ao stdin da Engine.
    ToEngine,
    /// Lida do stdout da Engine.
    FromEngine,
    /// Lida do stderr da Engine.
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub source: LogSource,
    pub line: String,
}

/// Placar como a Engine o envia: do ponto de vista de **quem joga**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UciScore {
    /// Centipeões.
    Cp(i32),
    /// Mate em N lances de quem joga (negativo: quem joga leva mate).
    Mate(i32),
}

/// Se o placar é exato ou só um limite (`lowerbound`/`upperbound`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bound {
    #[default]
    Exact,
    Lower,
    Upper,
}

/// Uma linha `info` da busca. Campos ausentes ou ilegíveis ficam `None`; a Engine é
/// externa e não confiável: nada aqui entra em pânico por texto estranho.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoLine {
    pub depth: Option<u32>,
    pub seldepth: Option<u32>,
    pub multipv: Option<u32>,
    pub score: Option<UciScore>,
    pub bound: Bound,
    pub nodes: Option<u64>,
    pub nps: Option<u64>,
    pub time_ms: Option<u64>,
    /// Lances da variante principal, em texto UCI cru (quem consome valida pelo núcleo).
    pub pv: Vec<String>,
}

impl InfoLine {
    /// Interpreta `info ...`; `None` se a linha não começa por `info`.
    pub fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        if words.next()? != "info" {
            return None;
        }
        let mut info = Self::default();
        while let Some(word) = words.next() {
            match word {
                // O resto da linha é texto livre.
                "string" => break,
                "pv" => info.pv = words.by_ref().map(str::to_owned).collect(),
                "depth" => info.depth = words.next().and_then(|v| v.parse().ok()),
                "seldepth" => info.seldepth = words.next().and_then(|v| v.parse().ok()),
                "multipv" => info.multipv = words.next().and_then(|v| v.parse().ok()),
                "nodes" => info.nodes = words.next().and_then(|v| v.parse().ok()),
                "nps" => info.nps = words.next().and_then(|v| v.parse().ok()),
                "time" => info.time_ms = words.next().and_then(|v| v.parse().ok()),
                "score" => {
                    let kind = words.next();
                    let value = words.next().and_then(|v| v.parse::<i32>().ok());
                    info.score = match (kind, value) {
                        (Some("cp"), Some(v)) => Some(UciScore::Cp(v)),
                        (Some("mate"), Some(v)) => Some(UciScore::Mate(v)),
                        _ => None,
                    };
                }
                "lowerbound" => info.bound = Bound::Lower,
                "upperbound" => info.bound = Bound::Upper,
                _ => {}
            }
        }
        Some(info)
    }
}
