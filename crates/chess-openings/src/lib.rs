//! Livro de aberturas offline (C19, #147): dada uma Posição, devolve o código ECO e o nome da
//! abertura conhecida, inclusive quando a Posição foi alcançada por transposição.
//!
//! Os dados são o conjunto CC0 `lichess-org/chess-openings` (`data/*.tsv`), embutido no
//! binário. Cada linha é jogada uma vez a partir da Posição inicial; o livro indexa a Posição
//! resultante pela chave de repetição FIDE (peças, vez, roques e en passant legal, sem
//! relógios). Se várias linhas chegam à mesma Posição, vale a mais longa (a mais específica);
//! em empate, a primeira do arquivo.
//!
//! Sem dependência de Bevy nem de SQLite (ADR-0001): só regras do `chess-core`.

use chess_core::{CanonicalPosition, RepetitionKey};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Os cinco arquivos TSV de origem (`a` a `e`).
const EMBEDDED: [&str; 5] = [
    include_str!("../data/a.tsv"),
    include_str!("../data/b.tsv"),
    include_str!("../data/c.tsv"),
    include_str!("../data/d.tsv"),
    include_str!("../data/e.tsv"),
];

/// Uma abertura nomeada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Código ECO (`C60`).
    pub eco: String,
    /// Nome em inglês, como no conjunto de origem (`Ruy Lopez: Morphy Defense`).
    pub name: String,
    /// Meios-lances da linha que define a abertura.
    pub plies: usize,
}

/// Índice Posição -> abertura.
#[derive(Debug, Clone, Default)]
pub struct OpeningBook {
    by_position: HashMap<RepetitionKey, Opening>,
}

impl OpeningBook {
    /// O livro embutido no app, construído na primeira chamada.
    pub fn embedded() -> &'static Self {
        static BOOK: OnceLock<OpeningBook> = OnceLock::new();
        BOOK.get_or_init(|| Self::from_tsv(EMBEDDED))
    }

    /// Constrói o livro de textos TSV (`eco<TAB>nome<TAB>pgn`, com cabeçalho opcional).
    /// Linhas malformadas ou com Lance ilegal são ignoradas.
    pub fn from_tsv<'a>(sources: impl IntoIterator<Item = &'a str>) -> Self {
        let mut by_position: HashMap<RepetitionKey, Opening> = HashMap::new();
        for source in sources {
            for line in source.lines() {
                let mut cols = line.split('\t');
                let (Some(eco), Some(name), Some(pgn)) = (cols.next(), cols.next(), cols.next())
                else {
                    continue;
                };
                if eco == "eco" {
                    continue;
                }
                let Some((position, plies)) = play_line(pgn) else {
                    continue;
                };
                let key = position.repetition_key();
                let better = by_position.get(&key).is_none_or(|old| plies > old.plies);
                if better {
                    by_position.insert(
                        key,
                        Opening {
                            eco: eco.to_owned(),
                            name: name.to_owned(),
                            plies,
                        },
                    );
                }
            }
        }
        Self { by_position }
    }

    /// Abertura exata da Posição (ignora relógios; vale por transposição).
    pub fn lookup(&self, position: &CanonicalPosition) -> Option<&Opening> {
        self.by_position.get(&position.repetition_key())
    }

    /// A abertura mais recente ao longo de uma linha de Posições (da inicial à atual): a da
    /// última Posição nomeada. Serve para continuar mostrando o nome depois que a Partida sai
    /// da teoria.
    pub fn along<'a>(
        &self,
        positions: impl IntoIterator<Item = &'a CanonicalPosition>,
    ) -> Option<&Opening> {
        positions.into_iter().filter_map(|p| self.lookup(p)).last()
    }

    /// Quantas Posições nomeadas o livro tem.
    pub fn len(&self) -> usize {
        self.by_position.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_position.is_empty()
    }
}

/// Joga uma linha `1. e4 e5 2. Nf3` a partir da Posição inicial.
fn play_line(pgn: &str) -> Option<(CanonicalPosition, usize)> {
    let mut position = CanonicalPosition::start();
    let mut plies = 0;
    for token in pgn.split_whitespace() {
        if token.ends_with('.') {
            continue;
        }
        let mv = position.parse_san(token).ok()?;
        position = position.play(mv).ok()?;
        plies += 1;
    }
    Some((position, plies))
}
