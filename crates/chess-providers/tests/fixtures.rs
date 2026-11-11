//! Toda resposta gravada em `tests/fixtures/` é legível e diz se é REAL (gravada com
//! `curl -i` em consulta manual de leitura pública, com data) ou SINTÉTICA (formato
//! documentado). Nenhuma guarda cookie.

use chess_net::testing::FakeResponse;
use std::path::Path;

#[test]
fn every_fixture_parses_is_labelled_and_carries_no_cookie() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut real = 0;
    let mut synthetic = 0;
    for entry in std::fs::read_dir(&dir).expect("fixtures") {
        let path = entry.expect("entrada").path();
        if path.extension().is_none_or(|e| e != "http") {
            continue;
        }
        FakeResponse::recorded(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let text = std::fs::read_to_string(&path).expect("texto");
        let first = text.lines().next().unwrap_or_default();
        if first.starts_with("# Resposta REAL gravada em 2026-11-11") {
            real += 1;
        } else if first.starts_with("# Resposta SINTETICA") {
            synthetic += 1;
        } else {
            panic!(
                "{} sem rótulo REAL/SINTETICA na primeira linha",
                path.display()
            );
        }
        assert!(
            !text.to_ascii_lowercase().contains("\nset-cookie:"),
            "{} guarda cookie",
            path.display()
        );
    }
    assert!(real >= 10, "poucas reais: {real}");
    assert!(synthetic >= 3, "poucas sintéticas: {synthetic}");
}
