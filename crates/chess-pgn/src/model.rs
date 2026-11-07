//! Modelo da partida importada: árvore de variações com comentários e NAGs por nó.

use serde::{Deserialize, Serialize};

/// Resultado declarado no PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PgnResult {
    WhiteWins,
    BlackWins,
    Draw,
    /// `*`: em andamento, abandonada ou sem resultado.
    Unknown,
}

impl PgnResult {
    /// Token do PGN (`1-0`, `0-1`, `1/2-1/2`, `*`).
    pub fn token(self) -> &'static str {
        match self {
            Self::WhiteWins => "1-0",
            Self::BlackWins => "0-1",
            Self::Draw => "1/2-1/2",
            Self::Unknown => "*",
        }
    }

    /// Lê o token de resultado (também `½-½`).
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "1-0" => Some(Self::WhiteWins),
            "0-1" => Some(Self::BlackWins),
            "1/2-1/2" | "½-½" => Some(Self::Draw),
            "*" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Um Lance da árvore. Cada Lance já foi validado pelo `chess-core` na Posição
/// em que aparece; `san` é a forma canônica (a que o exportador do app escreve).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveNode {
    pub san: String,
    /// Glifos numéricos de anotação (`$1` = `!`, `$2` = `?`...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nags: Vec<u8>,
    /// Comentários antes do Lance (só no primeiro Lance de uma linha).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_comments: Vec<String>,
    /// Comentários depois do Lance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
    /// Alternativas a este Lance: cada uma é uma linha que parte da Posição
    /// anterior a ele e o substitui (na ordem em que aparecem no PGN).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variations: Vec<Vec<MoveNode>>,
}

impl MoveNode {
    pub fn new(san: impl Into<String>) -> Self {
        Self {
            san: san.into(),
            ..Self::default()
        }
    }
}

/// Uma partida do PGN: tags, Posição inicial e a árvore (linha principal em
/// `moves`; alternativas em `MoveNode::variations`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PgnGame {
    /// Tags na ordem do arquivo (nomes repetidos preservados).
    pub tags: Vec<(String, String)>,
    /// Posição inicial em FEN (a padrão, ou a da tag `FEN`).
    pub start_fen: String,
    pub moves: Vec<MoveNode>,
    pub result: PgnResult,
    /// Comentários de uma partida sem nenhum Lance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
}

impl PgnGame {
    /// Valor da primeira tag com este nome.
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Total de meios-lances em todas as linhas (principal e variações).
    pub fn total_plies(&self) -> usize {
        fn count(line: &[MoveNode]) -> usize {
            line.iter()
                .map(|n| 1 + n.variations.iter().map(|v| count(v)).sum::<usize>())
                .sum()
        }
        count(&self.moves)
    }

    /// Quantidade de variações (linhas alternativas, em qualquer profundidade).
    pub fn variation_count(&self) -> usize {
        fn count(line: &[MoveNode]) -> usize {
            line.iter()
                .map(|n| n.variations.iter().map(|v| 1 + count(v)).sum::<usize>())
                .sum()
        }
        count(&self.moves)
    }

    /// SAN da linha principal.
    pub fn mainline_sans(&self) -> Vec<&str> {
        self.moves.iter().map(|n| n.san.as_str()).collect()
    }
}
