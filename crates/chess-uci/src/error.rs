//! Erros tipados e descritivos do adaptador.

use std::fmt;
use std::io;
use std::time::Duration;

use chess_core::MoveParseError;

/// Etapa em que um prazo estourou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Handshake,
    Ready,
    Search,
    Stop,
    Quit,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Handshake => "handshake (uci/uciok)",
            Self::Ready => "isready/readyok",
            Self::Search => "busca (go/bestmove)",
            Self::Stop => "parada (stop/bestmove)",
            Self::Quit => "encerramento (quit)",
        })
    }
}

#[derive(Debug)]
pub enum EngineError {
    /// O executável não pôde ser iniciado.
    Spawn { program: String, source: io::Error },
    /// Um prazo estourou. Quando `killed` é verdadeiro o processo foi morto e a
    /// instância não serve mais; senão a Engine continua viva (ex.: `wait_bestmove`).
    Timeout {
        stage: Stage,
        after: Duration,
        killed: bool,
    },
    /// O processo morreu ou fechou a saída (EOF) no meio da conversa.
    Crashed { exit: Option<String> },
    /// Falha ao escrever no stdin da Engine.
    Io(io::Error),
    /// A instância já foi encerrada (crash, timeout fatal ou `quit`).
    Terminated,
    /// `go` enquanto outra busca está em andamento.
    SearchInProgress,
    /// `wait_bestmove`/`stop` sem busca em andamento.
    NotSearching,
    /// A Engine respondeu um texto que não é Lance UCI. Recuperável.
    MalformedMove {
        text: String,
        source: MoveParseError,
    },
    /// A Engine propôs um Lance ilegal na Posição. Recuperável: a Posição não mudou.
    IllegalMove { text: String, fen: String },
    /// Argumento inválido (ex.: nome de opção com quebra de linha).
    InvalidInput(String),
}

impl EngineError {
    /// Erros em que a Engine segue utilizável e a Partida permanece intacta.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::MalformedMove { .. }
                | Self::IllegalMove { .. }
                | Self::SearchInProgress
                | Self::NotSearching
                | Self::InvalidInput(_)
                | Self::Timeout { killed: false, .. }
        )
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, source } => {
                write!(f, "não foi possível iniciar a Engine {program:?}: {source}")
            }
            Self::Timeout {
                stage,
                after,
                killed,
            } => write!(
                f,
                "a Engine não respondeu em {after:?} na etapa {stage}{}",
                if *killed { "; processo encerrado" } else { "" }
            ),
            Self::Crashed { exit } => match exit {
                Some(status) => write!(f, "a Engine encerrou inesperadamente ({status})"),
                None => f.write_str("a Engine encerrou inesperadamente (saída fechada)"),
            },
            Self::Io(e) => write!(f, "falha de E/S com a Engine: {e}"),
            Self::Terminated => f.write_str("a Engine já foi encerrada"),
            Self::SearchInProgress => f.write_str("já existe uma busca em andamento"),
            Self::NotSearching => f.write_str("não há busca em andamento"),
            Self::MalformedMove { text, source } => {
                write!(
                    f,
                    "a Engine respondeu um Lance malformado {text:?}: {source}"
                )
            }
            Self::IllegalMove { text, fen } => {
                write!(f, "a Engine propôs o Lance ilegal {text} na posição {fen}")
            }
            Self::InvalidInput(msg) => write!(f, "entrada inválida: {msg}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            Self::Io(e) => Some(e),
            Self::MalformedMove { source, .. } => Some(source),
            _ => None,
        }
    }
}
