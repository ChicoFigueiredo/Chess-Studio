//! Leitor do formato de problemas do Lichess (CSV, licença CC0).
//!
//! Colunas: `PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,
//! OpeningTags`. O FEN é a posição **antes** do lance do adversário: o primeiro Lance de
//! `Moves` (UCI) é dele e leva à posição do problema; os seguintes alternam a solução do
//! usuário e as respostas do adversário.
//!
//! A leitura é em fluxo (uma linha por vez, memória proporcional ao que foi selecionado),
//! com progresso e cancelamento. Cada problema selecionado é validado pelo `chess-core`;
//! o inválido é contado e descartado. Para de ler ao atingir a quantidade máxima, então
//! as linhas depois dela não são nem validadas.

use crate::exercise::{Exercise, ExerciseKind, Provenance, Solution};
use chess_core::{CanonicalPosition, Move};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Quantas linhas entre dois avisos de progresso.
const PROGRESS_EVERY: u64 = 1000;

/// Seleção ao importar. Critérios combinados com "e"; `themes` é "qualquer um destes".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuzzleFilter {
    /// Temas aceitos (sem diferenciar maiúsculas); vazio aceita qualquer.
    pub themes: Vec<String>,
    pub min_rating: Option<u32>,
    pub max_rating: Option<u32>,
    /// Quantidade máxima de Exercícios; a leitura termina ao atingi-la.
    pub max_count: usize,
}

impl Default for PuzzleFilter {
    fn default() -> Self {
        Self {
            themes: Vec::new(),
            min_rating: None,
            max_rating: None,
            max_count: usize::MAX,
        }
    }
}

impl PuzzleFilter {
    fn accepts(&self, rating: u32, themes: &[String]) -> bool {
        self.min_rating.is_none_or(|min| rating >= min)
            && self.max_rating.is_none_or(|max| rating <= max)
            && (self.themes.is_empty()
                || self
                    .themes
                    .iter()
                    .any(|wanted| themes.iter().any(|t| t.eq_ignore_ascii_case(wanted))))
    }
}

/// Andamento da leitura (avisado a cada mil linhas e no fim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportProgress {
    pub lines: u64,
    /// Bytes lidos (descomprimidos); compare com o tamanho do arquivo só para `.csv`.
    pub bytes: u64,
    pub imported: usize,
}

/// Resultado da importação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportReport {
    /// Problemas lidos (sem cabeçalho nem linhas em branco).
    pub lines_read: u64,
    pub imported: usize,
    /// Válidos de formato, mas fora da seleção (tema ou faixa).
    pub filtered_out: u64,
    /// Linhas malformadas ou que o núcleo de regras recusou (FEN ou Lance ilegal).
    pub invalid: u64,
    pub cancelled: bool,
}

#[derive(Debug)]
pub struct PuzzleImportError(pub String);

impl fmt::Display for PuzzleImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "não foi possível ler os problemas: {}", self.0)
    }
}

impl std::error::Error for PuzzleImportError {}

/// Abre um arquivo de problemas: `.csv` direto ou `.csv.zst` (Zstandard, via `ruzstd`).
pub fn open_puzzle_file(path: &Path) -> Result<Box<dyn BufRead + Send>, PuzzleImportError> {
    let err = |e: &dyn fmt::Display| PuzzleImportError(format!("{}: {e}", path.display()));
    let file = File::open(path).map_err(|e| err(&e))?;
    if path.extension().is_some_and(|x| x == "zst") {
        let decoder =
            ruzstd::decoding::StreamingDecoder::new(BufReader::new(file)).map_err(|e| err(&e))?;
        Ok(Box::new(BufReader::with_capacity(64 * 1024, decoder)))
    } else {
        Ok(Box::new(BufReader::with_capacity(64 * 1024, file)))
    }
}

/// Lê o fluxo e devolve os Exercícios selecionados com o relatório.
/// `cancel` é consultado a cada linha; `on_progress` a cada mil linhas.
pub fn import_lichess_csv<R: BufRead>(
    mut reader: R,
    filter: &PuzzleFilter,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> Result<(Vec<Exercise>, ImportReport), PuzzleImportError> {
    let mut out = Vec::new();
    let mut report = ImportReport::default();
    let mut bytes = 0u64;
    let mut line = Vec::new();
    let mut first = true;
    while out.len() < filter.max_count {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        line.clear();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| PuzzleImportError(e.to_string()))?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        let is_first = std::mem::take(&mut first);
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\n', '\r']);
        if text.trim().is_empty() || (is_first && text.starts_with("PuzzleId")) {
            continue;
        }
        report.lines_read += 1;
        match parse_row(text) {
            None => report.invalid += 1,
            Some(row) if !filter.accepts(row.rating, &row.themes) => report.filtered_out += 1,
            Some(row) => match row.into_exercise() {
                Some(exercise) => {
                    out.push(exercise);
                    report.imported += 1;
                }
                None => report.invalid += 1,
            },
        }
        if report.lines_read.is_multiple_of(PROGRESS_EVERY) {
            on_progress(&ImportProgress {
                lines: report.lines_read,
                bytes,
                imported: report.imported,
            });
        }
    }
    on_progress(&ImportProgress {
        lines: report.lines_read,
        bytes,
        imported: report.imported,
    });
    Ok((out, report))
}

struct Row<'a> {
    id: &'a str,
    fen: &'a str,
    moves: Vec<&'a str>,
    rating: u32,
    themes: Vec<String>,
    url: Option<&'a str>,
}

/// Separa as colunas; `None` se faltar coluna, o id for estranho ou o rating não for número.
fn parse_row(line: &str) -> Option<Row<'_>> {
    let mut cols = line.split(',');
    let id = cols.next()?.trim();
    let fen = cols.next()?.trim();
    let moves: Vec<&str> = cols.next()?.split_whitespace().collect();
    let rating = cols.next()?.trim().parse().ok()?;
    let mut rest = cols.skip(3); // RatingDeviation, Popularity, NbPlays
    let themes = rest.next()?.split_whitespace().map(str::to_owned).collect();
    let url = rest.next().map(str::trim).filter(|u| !u.is_empty());
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(Row {
        id,
        fen,
        moves,
        rating,
        themes,
        url,
    })
}

impl Row<'_> {
    /// Joga o 1º Lance (do adversário) e valida a linha de solução Lance a Lance.
    fn into_exercise(self) -> Option<Exercise> {
        // Adversário + ao menos um Lance do usuário.
        if self.moves.len() < 2 {
            return None;
        }
        let before = CanonicalPosition::from_fen(self.fen).ok()?;
        let opponent: Move = self.moves[0].parse().ok()?;
        let start = before.play(opponent).ok()?;
        // A linha do Exercício tem tamanho ímpar: termina num Lance do usuário.
        let mut line: Vec<String> = self.moves[1..].iter().map(|m| (*m).to_owned()).collect();
        if line.len().is_multiple_of(2) {
            line.pop();
        }
        let themes_text = if self.themes.is_empty() {
            String::new()
        } else {
            format!(" Temas: {}.", self.themes.join(", "))
        };
        let exercise = Exercise {
            id: format!("lichess-{}", self.id),
            kind: ExerciseKind::Tactic,
            title: format!("Lichess {}", self.id),
            fen: start.to_fen(),
            objective: "Encontre a melhor sequência de Lances.".to_owned(),
            solution: Solution::Moves { line },
            explanation: format!(
                "Problema do Lichess (CC0), dificuldade {}.{themes_text}",
                self.rating
            ),
            version: 1,
            themes: self.themes,
            rating: Some(self.rating),
            provenance: Some(Provenance {
                provider: "lichess".to_owned(),
                license: "CC0-1.0".to_owned(),
                url: self.url.map(str::to_owned),
                remote_id: Some(self.id.to_owned()),
            }),
            pack: None,
        };
        exercise.validate().ok()?;
        Some(exercise)
    }
}
