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
