//! O Exercício: posição inicial, objetivo, solução verificável e explicação.

use chess_core::{CanonicalPosition, FenError, GameStatus, Move};
use chess_game::{Game, GameSetup, PolicyKind};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseKind {
    Stalemate,
    Mate,
    Check,
    IllegalPawn,
    Castling,
    Promotion,
    /// Memória de sequência (B15).
    Memory,
    /// Memória de posição (C11): mostra a Posição, esvazia o tabuleiro e o usuário
    /// recoloca as peças.
    MemoryPosition,
    /// Tática vinda de um pacote externo (problemas do Lichess): FEN + linha de Lances.
    Tactic,
}

impl ExerciseKind {
    pub const ALL: [ExerciseKind; 9] = [
        ExerciseKind::Stalemate,
        ExerciseKind::Mate,
        ExerciseKind::Check,
        ExerciseKind::IllegalPawn,
        ExerciseKind::Castling,
        ExerciseKind::Promotion,
        ExerciseKind::Memory,
        ExerciseKind::MemoryPosition,
        ExerciseKind::Tactic,
    ];

    /// Identificador estável (coluna `kind` do banco).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stalemate => "stalemate",
            Self::Mate => "mate",
            Self::Check => "check",
            Self::IllegalPawn => "illegal_pawn",
            Self::Castling => "castling",
            Self::Promotion => "promotion",
            Self::Memory => "memory",
            Self::MemoryPosition => "memory_position",
            Self::Tactic => "tactic",
        }
    }
}

/// Como saber que o Exercício foi cumprido. O lado do usuário é o da vez no FEN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Solution {
    /// Sequência aceita em UCI, intercalando Lances do usuário (posições pares) e as
    /// respostas do adversário (ímpares), que a Resolução joga sozinha. Tamanho ímpar.
    Moves { line: Vec<String> },
    /// Qualquer Lance que dê xeque-mate.
    MateInOne,
    /// Qualquer Lance que deixe o adversário sem Lances legais e fora de xeque.
    ForceStalemate,
    /// Qualquer Lance que dê xeque.
    GiveCheck,
    /// Tentar o Lance ilegal `uci` e vê-lo recusado cumpre o objetivo.
    RefuseIllegal { uci: String },
    /// Memória de sequência (TRN-04, DEC-45): o app anima `line` (3 a 8 Lances, dos dois
    /// lados) a partir do FEN e o usuário a reproduz. `speed_ms` é o tempo de cada Lance.
    Memorize {
        line: Vec<String>,
        #[serde(default = "default_speed_ms")]
        speed_ms: u32,
    },
    /// Memória de posição (C11): o app mostra o FEN do Exercício por `seconds` e o
    /// usuário recoloca as peças num tabuleiro vazio.
    MemorizePosition { seconds: u32 },
}

/// Menor e maior sequência de um Exercício de memória (DEC-45).
pub const MEMORY_LINE_RANGE: std::ops::RangeInclusive<usize> = 3..=8;

fn default_speed_ms() -> u32 {
    1200
}

/// Tempo de estudo aceito num Exercício de memória de posição, em segundos.
pub const POSITION_STUDY_SECONDS: std::ops::RangeInclusive<u32> = 3..=120;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exercise {
    /// Identificador estável entre versões do catálogo.
    pub id: String,
    pub kind: ExerciseKind,
    pub title: String,
    pub fen: String,
    pub objective: String,
    pub solution: Solution,
    pub explanation: String,
    pub version: u32,
    /// Temas (Lichess: `mateIn2`, `fork`…). Vazio nos Exercícios de fábrica.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<String>,
    /// Dificuldade (rating do problema). `None` nos Exercícios de fábrica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u32>,
    /// Origem do Exercício; `None` é "de fábrica".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Pacote de Exercícios a que pertence; `None` é "de fábrica". Quem define é o
    /// carregador de pacotes, não o arquivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
}

/// Origem de um Exercício externo (GLOSSARY: Origem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Provedor do dado (`lichess`).
    pub provider: String,
    /// Licença dos dados (`CC0-1.0`).
    pub license: String,
    /// Página de referência (a partida do problema).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Identificador no Provedor (`PuzzleId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
}

/// Por que um Exercício não passa na validação do chess-core.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExerciseError {
    EmptyField(&'static str),
    BadVersion,
    InvalidFen(FenError),
    BadMove(String),
    IllegalLine {
        index: usize,
        uci: String,
    },
    LineLength(usize),
    NoSolutionFound(&'static str),
    MoveIsLegal(String),
    MemoryLength(usize),
    KindMismatch,
    /// Tempo de estudo fora de [`POSITION_STUDY_SECONDS`].
    StudyTime(u32),
    /// `memorize_position` só vale para o tipo `memory_position` (e vice-versa).
    PositionKindMismatch,
}

impl fmt::Display for ExerciseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(name) => write!(f, "campo `{name}` vazio"),
            Self::BadVersion => f.write_str("a versão do Exercício começa em 1"),
            Self::InvalidFen(e) => write!(f, "FEN inválido: {e}"),
            Self::BadMove(m) => write!(f, "Lance UCI inválido: {m}"),
            Self::IllegalLine { index, uci } => {
                write!(f, "o Lance {} da solução ({uci}) é ilegal", index + 1)
            }
            Self::LineLength(n) => write!(f, "a sequência precisa ter tamanho ímpar (tem {n})"),
            Self::NoSolutionFound(what) => write!(f, "nenhum Lance legal cumpre: {what}"),
            Self::MoveIsLegal(m) => write!(f, "o Lance {m} é legal, mas devia ser ilegal"),
            Self::MemoryLength(n) => {
                write!(
                    f,
                    "a sequência de memória precisa ter de 3 a 8 Lances (tem {n})"
                )
            }
            Self::KindMismatch => {
                f.write_str("`memorize` só vale para o tipo `memory` (e vice-versa)")
            }
            Self::StudyTime(n) => write!(f, "o tempo de estudo ({n} s) deve ficar entre 3 e 120 s"),
            Self::PositionKindMismatch => f.write_str(
                "`memorize_position` só vale para o tipo `memory_position` (e vice-versa)",
            ),
        }
    }
}

impl std::error::Error for ExerciseError {}

fn parse_move(uci: &str) -> Result<Move, ExerciseError> {
    uci.parse::<Move>()
        .map_err(|_| ExerciseError::BadMove(uci.to_owned()))
}

impl Exercise {
    pub fn position(&self) -> Result<CanonicalPosition, FenError> {
        CanonicalPosition::from_fen(&self.fen)
    }

    /// Partida nova do Exercício: Política Assistido, desfazer permitido (DEC-39), a
    /// partir da posição **original** (TRN-02).
    pub fn new_game(&self) -> Result<Game, FenError> {
        Ok(Game::new(GameSetup::from_fen(
            PolicyKind::Assisted,
            &self.fen,
        )?))
    }

    /// Valida o Exercício com o chess-core: FEN, solução legal e objetivo alcançável.
    pub fn validate(&self) -> Result<(), ExerciseError> {
        for (name, value) in [
            ("id", &self.id),
            ("title", &self.title),
            ("objective", &self.objective),
            ("explanation", &self.explanation),
        ] {
            if value.trim().is_empty() {
                return Err(ExerciseError::EmptyField(name));
            }
        }
        if self.version == 0 {
            return Err(ExerciseError::BadVersion);
        }
        let start = self.position().map_err(ExerciseError::InvalidFen)?;
        if (self.kind == ExerciseKind::Memory) != matches!(self.solution, Solution::Memorize { .. })
        {
            return Err(ExerciseError::KindMismatch);
        }
        if (self.kind == ExerciseKind::MemoryPosition)
            != matches!(self.solution, Solution::MemorizePosition { .. })
        {
            return Err(ExerciseError::PositionKindMismatch);
        }
        match &self.solution {
            Solution::MemorizePosition { seconds } => {
                if !POSITION_STUDY_SECONDS.contains(seconds) {
                    return Err(ExerciseError::StudyTime(*seconds));
                }
            }
            Solution::Moves { line } => {
                if line.len() % 2 == 0 {
                    return Err(ExerciseError::LineLength(line.len()));
                }
                let mut position = start;
                for (index, uci) in line.iter().enumerate() {
                    let mv = parse_move(uci)?;
                    position = position.play(mv).map_err(|_| ExerciseError::IllegalLine {
                        index,
                        uci: uci.clone(),
                    })?;
                }
            }
            Solution::Memorize { line, speed_ms } => {
                if !MEMORY_LINE_RANGE.contains(&line.len()) {
                    return Err(ExerciseError::MemoryLength(line.len()));
                }
                if *speed_ms == 0 {
                    return Err(ExerciseError::EmptyField("speed_ms"));
                }
                let mut position = start;
                for (index, uci) in line.iter().enumerate() {
                    let mv = parse_move(uci)?;
                    position = position.play(mv).map_err(|_| ExerciseError::IllegalLine {
                        index,
                        uci: uci.clone(),
                    })?;
                }
            }
            Solution::MateInOne => {
                require(
                    first_solution(&start, &self.solution),
                    "xeque-mate em um Lance",
                )?;
            }
            Solution::ForceStalemate => {
                require(first_solution(&start, &self.solution), "afogamento")?;
            }
            Solution::GiveCheck => {
                require(first_solution(&start, &self.solution), "xeque")?;
            }
            Solution::RefuseIllegal { uci } => {
                let mv = parse_move(uci)?;
                if start.is_legal(mv) {
                    return Err(ExerciseError::MoveIsLegal(uci.clone()));
                }
            }
        }
        Ok(())
    }

    /// Lances da solução: a sequência dada, ou o primeiro Lance que cumpre o objetivo
    /// (para a revisão mostrar uma resposta). Vazio para "recusar o Lance ilegal".
    pub fn solution_line(&self) -> Vec<Move> {
        let Ok(start) = self.position() else {
            return Vec::new();
        };
        match &self.solution {
            Solution::Moves { line } | Solution::Memorize { line, .. } => {
                line.iter().filter_map(|m| m.parse().ok()).collect()
            }
            Solution::RefuseIllegal { .. } | Solution::MemorizePosition { .. } => Vec::new(),
            other => first_solution(&start, other).into_iter().collect(),
        }
    }
}

fn require(found: Option<Move>, what: &'static str) -> Result<Move, ExerciseError> {
    found.ok_or(ExerciseError::NoSolutionFound(what))
}

/// A posição `after` (depois do Lance do usuário) cumpre o objetivo "de critério"?
pub(crate) fn satisfied(solution: &Solution, after: &CanonicalPosition) -> bool {
    match solution {
        Solution::MateInOne => matches!(after.status(), GameStatus::Checkmate { .. }),
        Solution::ForceStalemate => matches!(after.status(), GameStatus::Stalemate),
        Solution::GiveCheck => after.is_check(),
        Solution::Moves { .. }
        | Solution::RefuseIllegal { .. }
        | Solution::Memorize { .. }
        | Solution::MemorizePosition { .. } => false,
    }
}

fn first_solution(start: &CanonicalPosition, solution: &Solution) -> Option<Move> {
    start
        .legal_moves()
        .into_iter()
        .find(|mv| start.play(*mv).is_ok_and(|p| satisfied(solution, &p)))
}
