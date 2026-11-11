#![allow(clippy::unwrap_used)]
//! Leitor do formato de problemas do Lichess (CSV, CC0) e filtro de Exercícios.

use chess_training::{
    ExerciseFilter, ExerciseKind, ImportProgress, PuzzleFilter, Solution, import_lichess_csv,
    open_puzzle_file,
};
use std::io::{BufReader, Cursor, Read};
use std::sync::atomic::{AtomicBool, Ordering};

const HEADER: &str =
    "PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags";
/// Posição ANTES do lance do adversário (Pretas jogam b7b6; depois Ra8#).
const BEFORE: &str = "6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1";
const AFTER: &str = "6k1/5ppp/1p6/8/8/8/5PPP/R5K1 w - - 0 2";

fn row(id: &str, fen: &str, moves: &str, rating: &str, themes: &str) -> String {
    format!(
        "{id},{fen},{moves},{rating},75,90,1200,{themes},https://lichess.org/{id}#33,Sicilian_Defense\n"
    )
}

fn import(
    text: &str,
    filter: &PuzzleFilter,
) -> (Vec<chess_training::Exercise>, chess_training::ImportReport) {
    let cancel = AtomicBool::new(false);
    import_lichess_csv(Cursor::new(text.as_bytes()), filter, &cancel, &mut |_| {}).unwrap()
}

#[test]
fn a_puzzle_becomes_an_exercise_from_the_position_after_the_opponent_move() {
    let text = format!(
        "{HEADER}\n{}",
        row(
            "0009B",
            BEFORE,
            "b7b6 a1a8",
            "1500",
            "mate mateIn1 backRankMate"
        )
    );
    let (exercises, report) = import(&text, &PuzzleFilter::default());
    assert_eq!(report.imported, 1);
    assert_eq!(report.invalid, 0);
    let e = &exercises[0];
    assert_eq!(e.fen, AFTER, "o FEN do Exercício é depois do 1º lance");
    assert_eq!(e.kind, ExerciseKind::Tactic);
    assert_eq!(
        e.solution,
        Solution::Moves {
            line: vec!["a1a8".into()]
        }
    );
    assert_eq!(e.rating, Some(1500));
    assert_eq!(e.themes, ["mate", "mateIn1", "backRankMate"]);
    let origin = e.provenance.as_ref().unwrap();
    assert_eq!(origin.provider, "lichess");
    assert_eq!(origin.license, "CC0-1.0");
    assert_eq!(origin.url.as_deref(), Some("https://lichess.org/0009B#33"));
    assert_eq!(origin.remote_id.as_deref(), Some("0009B"));
    e.validate().unwrap();
}

#[test]
fn a_longer_line_alternates_solution_and_replies() {
    let text = row("00AAA", BEFORE, "b7b6 a1a7 f7f6 a7a8", "1800", "short");
    let (exercises, _) = import(&text, &PuzzleFilter::default());
    assert_eq!(
        exercises[0].solution,
        Solution::Moves {
            line: vec!["a1a7".into(), "f7f6".into(), "a7a8".into()]
        }
    );
}

#[test]
fn invalid_rows_are_counted_and_skipped() {
    let mut text = String::from(HEADER);
    text.push('\n');
    text += &row("ok001", BEFORE, "b7b6 a1a8", "1500", "mate");
    text += &row("badfen", "isto nao e fen", "b7b6 a1a8", "1500", "mate");
    text += &row("illegal", BEFORE, "b7b6 a1h8", "1500", "mate");
    text += &row("oppillegal", BEFORE, "b7b3 a1a8", "1500", "mate");
    text += &row("single", BEFORE, "b7b6", "1500", "mate");
    text += &row("norating", BEFORE, "b7b6 a1a8", "abc", "mate");
    text += "curta,so,tres\n";
    text += "\n";
    let (exercises, report) = import(&text, &PuzzleFilter::default());
    assert_eq!(exercises.len(), 1);
    assert_eq!(report.imported, 1);
    assert_eq!(report.invalid, 6);
    assert_eq!(
        report.lines_read, 7,
        "cabeçalho e linha em branco não contam como problema"
    );
}

#[test]
fn selection_by_theme_rating_and_count() {
    let mut text = String::new();
    for (i, (rating, themes)) in [
        (900, "fork"),
        (1400, "fork pin"),
        (1500, "pin"),
        (1600, "fork"),
        (2200, "fork"),
    ]
    .iter()
    .enumerate()
    {
        text += &row(
            &format!("p{i}"),
            BEFORE,
            "b7b6 a1a8",
            &rating.to_string(),
            themes,
        );
    }
    let filter = PuzzleFilter {
        themes: vec!["fork".into()],
        min_rating: Some(1000),
        max_rating: Some(2000),
        max_count: 10,
    };
    let (exercises, report) = import(&text, &filter);
    let ids: Vec<_> = exercises.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["lichess-p1", "lichess-p3"]);
    assert_eq!(report.filtered_out, 3);

    let capped = PuzzleFilter {
        max_count: 1,
        ..filter
    };
    let (exercises, report) = import(&text, &capped);
    assert_eq!(exercises.len(), 1);
    assert!(
        report.lines_read < 5,
        "para de ler ao atingir a quantidade máxima"
    );
}

/// Leitor que fabrica `n` linhas sob demanda: nunca existe o arquivo inteiro na memória.
struct Synthetic {
    next: u32,
    total: u32,
    buf: Vec<u8>,
    pos: usize,
}

impl Synthetic {
    fn new(total: u32) -> Self {
        Self {
            next: 0,
            total,
            buf: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for Synthetic {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos == self.buf.len() {
            if self.next == self.total {
                return Ok(0);
            }
            let themes = if self.next % 10 == 0 { "fork" } else { "pin" };
            self.buf = row(
                &format!("s{:06}", self.next),
                BEFORE,
                "b7b6 a1a8",
                &(600 + self.next % 2000).to_string(),
                themes,
            )
            .into_bytes();
            self.pos = 0;
            self.next += 1;
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[test]
fn two_hundred_thousand_lines_stream_with_a_filter() {
    let filter = PuzzleFilter {
        themes: vec!["fork".into()],
        min_rating: Some(1000),
        max_rating: Some(1500),
        max_count: usize::MAX,
    };
    let cancel = AtomicBool::new(false);
    let mut ticks = 0;
    let (exercises, report) = import_lichess_csv(
        BufReader::with_capacity(4096, Synthetic::new(200_000)),
        &filter,
        &cancel,
        &mut |_: &ImportProgress| ticks += 1,
    )
    .unwrap();
    assert_eq!(report.lines_read, 200_000);
    assert!(!report.cancelled);
    assert!(ticks >= 100, "o progresso é avisado durante a leitura");
    assert_eq!(exercises.len(), report.imported);
    assert!(
        exercises
            .iter()
            .all(|e| (1000..=1500).contains(&e.rating.unwrap()))
    );
    assert!(report.imported > 500 && report.imported < 20_000);
}

#[test]
fn import_can_be_cancelled_midway() {
    let cancel = AtomicBool::new(false);
    let (_, report) = import_lichess_csv(
        BufReader::new(Synthetic::new(200_000)),
        &PuzzleFilter {
            max_count: usize::MAX,
            ..PuzzleFilter::default()
        },
        &cancel,
        &mut |p: &ImportProgress| {
            if p.lines >= 5_000 {
                cancel.store(true, Ordering::Relaxed);
            }
        },
    )
    .unwrap();
    assert!(report.cancelled);
    assert!(report.lines_read < 20_000, "leu {}", report.lines_read);
}

#[test]
fn zst_and_plain_files_read_the_same() {
    let dir = std::env::temp_dir().join(format!("cs-lichess-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = format!(
        "{HEADER}\n{}",
        row("zz001", BEFORE, "b7b6 a1a8", "1500", "mate")
    );
    let plain = dir.join("p.csv");
    std::fs::write(&plain, &csv).unwrap();
    let zst = dir.join("p.csv.zst");
    std::fs::write(
        &zst,
        ruzstd::encoding::compress_to_vec(
            csv.as_bytes(),
            ruzstd::encoding::CompressionLevel::Fastest,
        ),
    )
    .unwrap();
    for path in [&plain, &zst] {
        let cancel = AtomicBool::new(false);
        let (exercises, _) = import_lichess_csv(
            open_puzzle_file(path).unwrap(),
            &PuzzleFilter::default(),
            &cancel,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(exercises.len(), 1, "{path:?}");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn exercise_filter_by_pack_theme_and_rating() {
    let (exercises, _) = import(
        &(row("a1", BEFORE, "b7b6 a1a8", "1200", "fork")
            + &row("a2", BEFORE, "b7b6 a1a8", "1800", "pin")),
        &PuzzleFilter::default(),
    );
    let mut packed = exercises.clone();
    for e in &mut packed {
        e.pack = Some("lichess-1".into());
    }
    let bundled = chess_training::Catalog::bundled().unwrap();
    let factory = bundled.iter().next().unwrap();

    let by_theme = ExerciseFilter {
        theme: "FORK".into(),
        ..ExerciseFilter::default()
    };
    assert!(by_theme.matches(&packed[0]) && !by_theme.matches(&packed[1]));
    let by_rating = ExerciseFilter {
        min_rating: Some(1500),
        ..ExerciseFilter::default()
    };
    assert!(!by_rating.matches(&packed[0]) && by_rating.matches(&packed[1]));
    assert!(!by_rating.matches(factory), "de fábrica não tem rating");
    let factory_only = ExerciseFilter {
        pack: Some(None),
        ..ExerciseFilter::default()
    };
    assert!(factory_only.matches(factory) && !factory_only.matches(&packed[0]));
    let one_pack = ExerciseFilter {
        pack: Some(Some("lichess-1".into())),
        ..ExerciseFilter::default()
    };
    assert!(one_pack.matches(&packed[0]) && !one_pack.matches(factory));
    assert!(ExerciseFilter::default().matches(factory));
}

#[test]
fn factory_json_still_loads_without_new_fields() {
    let catalog = chess_training::Catalog::bundled().unwrap();
    assert!(catalog.iter().all(|e| e.themes.is_empty()
        && e.rating.is_none()
        && e.provenance.is_none()
        && e.pack.is_none()));
    let json = serde_json::to_string(catalog.iter().next().unwrap()).unwrap();
    assert!(!json.contains("provenance") && !json.contains("themes"));
}
