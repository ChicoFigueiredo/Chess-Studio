#![allow(clippy::unwrap_used)]
//! PDF A4 da Súmula (V2-SUM-03): verificação estrutural, sem comparar o binário inteiro.

use chess_core::Color;
use chess_training::{
    A4_HEIGHT_PT, A4_WIDTH_PT, ManualScoresheet, ROWS_PER_PAGE, SheetContent, SheetHeader,
    SheetLabels, SheetSpec, page_count, render_scoresheet_pdf,
};
use std::collections::HashMap;

const FONT: &[u8] = include_bytes!("../../chess-studio/assets/fonts/NotoSans-Regular.ttf");

fn header() -> SheetHeader {
    SheetHeader {
        event: "Torneio Aberto de Inverno".into(),
        date: "07/11/2026".into(),
        white: "Ana Souza".into(),
        black: "Beto Lima".into(),
        result: "1-0".into(),
    }
}

fn render(content: SheetContent<'_>, first_side: Color) -> Vec<u8> {
    let header = header();
    let labels = SheetLabels::pt_br();
    let spec = SheetSpec {
        header: &header,
        labels: &labels,
        first_move_number: 1,
        first_side,
        content,
    };
    render_scoresheet_pdf(FONT, &spec).unwrap()
}

fn sheet(n: u16) -> ManualScoresheet {
    ManualScoresheet::from_parts((1..=n).map(|p| (p, format!("Cf{}", p % 8 + 1))), None).unwrap()
}

fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

fn page_count_in(pdf: &str) -> usize {
    pdf.matches("/Type /Page\n").count() + pdf.matches("/Type /Page ").count()
}

/// Texto desenhado: decodifica os `<glifos> Tj` com o `ToUnicode` do próprio PDF.
fn drawn_text(pdf: &str) -> String {
    let mut map: HashMap<u16, char> = HashMap::new();
    let cmap_start = pdf.find("beginbfchar").unwrap();
    let cmap_end = pdf.find("endbfchar").unwrap();
    for line in pdf[cmap_start..cmap_end].lines().skip(1) {
        let mut parts = line.split(['<', '>']).filter(|s| !s.trim().is_empty());
        let (Some(g), Some(u)) = (parts.next(), parts.next()) else {
            continue;
        };
        let g = u16::from_str_radix(g, 16).unwrap();
        let u = u32::from(u16::from_str_radix(u, 16).unwrap());
        map.insert(g, char::from_u32(u).unwrap());
    }
    let mut out = String::new();
    for chunk in pdf.split("> Tj") {
        let Some(open) = chunk.rfind('<') else {
            continue;
        };
        let hex = &chunk[open + 1..];
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        for i in (0..hex.len()).step_by(4) {
            let g = u16::from_str_radix(&hex[i..i + 4], 16).unwrap();
            out.push(map.get(&g).copied().unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

#[test]
fn is_a_pdf_with_a4_pages_and_the_header_text() {
    let s = sheet(6);
    let bytes = render(
        SheetContent::Annotated {
            sheet: &s,
            played_plies: 6,
        },
        Color::White,
    );
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF") || text(&bytes).trim_end().ends_with("%%EOF"));
    let pdf = text(&bytes);
    assert_eq!(page_count_in(&pdf), 1);
    assert!(pdf.contains("/MediaBox [0 0 595 842]"), "{}", &pdf[..400]);
    assert_eq!((A4_WIDTH_PT, A4_HEIGHT_PT), (595.0, 842.0));
    let drawn = drawn_text(&pdf);
    for expected in [
        "Súmula",
        "Torneio Aberto de Inverno",
        "07/11/2026",
        "Ana Souza",
        "Beto Lima",
        "1-0",
        "Brancas",
        "Pretas",
        "Resultado",
        "Assinatura das Brancas",
        "Assinatura das Pretas",
        "Árbitro",
        "Página 1 / 1",
    ] {
        assert!(drawn.lines().any(|l| l == expected), "falta {expected:?}");
    }
}

#[test]
fn annotated_sheet_prints_what_the_player_wrote_not_the_real_san() {
    let mut s = ManualScoresheet::new();
    s.write(1, "e4").unwrap();
    s.write(2, "e5").unwrap();
    s.write(3, "Cf3").unwrap();
    let bytes = render(
        SheetContent::Annotated {
            sheet: &s,
            played_plies: 3,
        },
        Color::White,
    );
    let drawn = drawn_text(&text(&bytes));
    let lines: Vec<&str> = drawn.lines().collect();
    for w in ["e4", "e5", "Cf3"] {
        assert!(lines.contains(&w), "falta {w}");
    }
    assert!(!lines.contains(&"Nf3"));
    // Numeração dos lances 1 e 2 (a linha 2 só tem as brancas).
    assert!(lines.contains(&"1.") && lines.contains(&"2."));
}

#[test]
fn page_count_follows_the_number_of_moves() {
    let header = header();
    let labels = SheetLabels::pt_br();
    let pages = |plies: usize, side: Color| {
        let s = sheet(u16::try_from(plies).unwrap());
        let spec = SheetSpec {
            header: &header,
            labels: &labels,
            first_move_number: 1,
            first_side: side,
            content: SheetContent::Annotated {
                sheet: &s,
                played_plies: plies,
            },
        };
        page_count(&spec)
    };
    assert_eq!(pages(0, Color::White), 1, "sempre ao menos uma folha");
    assert_eq!(
        pages(2 * ROWS_PER_PAGE, Color::White),
        1,
        "60 lances = 1 folha"
    );
    assert_eq!(pages(2 * ROWS_PER_PAGE + 1, Color::White), 2);
    assert_eq!(pages(4 * ROWS_PER_PAGE, Color::White), 2);
    // Começando pelas pretas, a primeira linha só tem o meio-lance das pretas.
    assert_eq!(pages(2 * ROWS_PER_PAGE, Color::Black), 2);

    // E o PDF tem mesmo esse número de páginas, com a numeração seguindo na 2ª folha.
    let s = sheet(250);
    let bytes = render(
        SheetContent::Annotated {
            sheet: &s,
            played_plies: 250,
        },
        Color::White,
    );
    let pdf = text(&bytes);
    assert_eq!(page_count_in(&pdf), 3);
    assert!(pdf.contains("/Count 3"));
    let drawn = drawn_text(&pdf);
    assert!(drawn.lines().any(|l| l == "65."));
    assert!(drawn.lines().any(|l| l == "Página 3 / 3"));
}

#[test]
fn blank_sheet_has_the_grid_but_no_annotations() {
    let bytes = render(SheetContent::Blank { pages: 2 }, Color::White);
    let pdf = text(&bytes);
    assert_eq!(page_count_in(&pdf), 2);
    let drawn = drawn_text(&pdf);
    let lines: Vec<&str> = drawn.lines().collect();
    assert!(lines.contains(&"1.") && lines.contains(&"120."));
    assert!(lines.contains(&"Página 2 / 2"));
    assert!(!lines.contains(&"e4"));
}

#[test]
fn generation_is_deterministic_and_the_font_is_a_subset() {
    let s = sheet(10);
    let make = || {
        render(
            SheetContent::Annotated {
                sheet: &s,
                played_plies: 10,
            },
            Color::White,
        )
    };
    let (a, b) = (make(), make());
    assert_eq!(a, b);
    assert!(
        a.len() < FONT.len() / 2,
        "a fonte inteira não deve ir no PDF ({} bytes)",
        a.len()
    );
}

#[test]
fn invalid_font_is_an_error_not_a_panic() {
    let header = header();
    let labels = SheetLabels::pt_br();
    let spec = SheetSpec {
        header: &header,
        labels: &labels,
        first_move_number: 1,
        first_side: Color::White,
        content: SheetContent::Blank { pages: 1 },
    };
    assert!(render_scoresheet_pdf(b"nao e fonte", &spec).is_err());
}
