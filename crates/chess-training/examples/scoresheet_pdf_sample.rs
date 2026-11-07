//! Gera os PDFs de exemplo da evidência do E3 (#60):
//! `cargo run -p chess-training --example scoresheet_pdf_sample -- docs/evidencias/e3`

use chess_core::Color;
use chess_training::{
    ManualScoresheet, SheetContent, SheetHeader, SheetLabels, SheetSpec, render_scoresheet_pdf,
};

const FONT: &[u8] = include_bytes!("../../chess-studio/assets/fonts/NotoSans-Regular.ttf");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let header = SheetHeader {
        event: "Torneio Aberto de Inverno".into(),
        date: "07/11/2026".into(),
        white: "Ana Souza".into(),
        black: "Beto Lima".into(),
        result: "1-0".into(),
    };
    let labels = SheetLabels::pt_br();
    let moves = [
        "e4", "e5", "Cf3", "Cc6", "Bb5", "a6", "Ba4", "Cf6", "O-O", "Be7", "Te1", "b5", "Bb3",
        "d6", "c3", "O-O", "h3", "Ca5", "Bc2", "c5", "d4", "Dc7",
    ];
    let sheet = ManualScoresheet::from_parts(
        moves
            .iter()
            .enumerate()
            .map(|(i, m)| (u16::try_from(i + 1).unwrap_or(1), (*m).to_owned())),
        None,
    )?;
    let spec = |content| SheetSpec {
        header: &header,
        labels: &labels,
        first_move_number: 1,
        first_side: Color::White,
        content,
    };
    let annotated = spec(SheetContent::Annotated {
        sheet: &sheet,
        played_plies: moves.len(),
    });
    std::fs::write(
        format!("{dir}/sumula-anotada.pdf"),
        render_scoresheet_pdf(FONT, &annotated)?,
    )?;
    let blank_header = SheetHeader::default();
    let blank = SheetSpec {
        header: &blank_header,
        ..spec(SheetContent::Blank { pages: 1 })
    };
    std::fs::write(
        format!("{dir}/sumula-em-branco.pdf"),
        render_scoresheet_pdf(FONT, &blank)?,
    )?;
    Ok(())
}
