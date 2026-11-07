use crate::{PgnError, PgnGame};

#[derive(Debug, Default)]
pub struct ParseReport {
    pub games: Vec<PgnGame>,
    pub errors: Vec<PgnError>,
}

pub fn parse_pgn(_text: &str) -> Result<Vec<PgnGame>, PgnError> {
    Ok(Vec::new())
}

pub fn parse_pgn_lenient(_text: &str) -> ParseReport {
    ParseReport::default()
}
