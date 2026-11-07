//! Interpretação das linhas `info` da Engine (C8): só texto, sem processo.
#![allow(clippy::unwrap_used)]

use chess_uci::{Bound, InfoLine, UciScore};

#[test]
fn parses_a_full_stockfish_info_line() {
    let line = "info depth 18 seldepth 24 multipv 2 score cp -35 nodes 123456 nps 1500000 \
                hashfull 12 tbhits 0 time 82 pv e7e5 g1f3 b8c6";
    let info = InfoLine::parse(line).unwrap();
    assert_eq!(info.depth, Some(18));
    assert_eq!(info.seldepth, Some(24));
    assert_eq!(info.multipv, Some(2));
    assert_eq!(info.score, Some(UciScore::Cp(-35)));
    assert_eq!(info.nodes, Some(123_456));
    assert_eq!(info.nps, Some(1_500_000));
    assert_eq!(info.time_ms, Some(82));
    assert_eq!(info.bound, Bound::Exact);
    assert_eq!(info.pv, ["e7e5", "g1f3", "b8c6"]);
}

#[test]
fn parses_mate_scores_and_bounds() {
    let mate = InfoLine::parse("info depth 5 score mate -3 pv a1a2").unwrap();
    assert_eq!(mate.score, Some(UciScore::Mate(-3)));
    let lower = InfoLine::parse("info depth 5 score cp 100 lowerbound pv a1a2").unwrap();
    assert_eq!(lower.bound, Bound::Lower);
    let upper = InfoLine::parse("info depth 5 score cp 100 upperbound pv a1a2").unwrap();
    assert_eq!(upper.bound, Bound::Upper);
}

#[test]
fn ignores_non_info_and_garbage_numbers() {
    assert!(InfoLine::parse("bestmove e2e4").is_none());
    assert!(InfoLine::parse("").is_none());
    let bad = InfoLine::parse("info depth abc score cp x nodes -1 pv e2e4").unwrap();
    assert_eq!(bad.depth, None);
    assert_eq!(bad.score, None);
    assert_eq!(bad.nodes, None);
    assert_eq!(bad.pv, ["e2e4"]);
}

#[test]
fn info_string_does_not_leak_keywords_from_its_text() {
    let info = InfoLine::parse("info string depth 99 score cp 7 pv e2e4").unwrap();
    assert_eq!(info.depth, None);
    assert_eq!(info.score, None);
    assert!(info.pv.is_empty());
}

#[test]
fn skips_currmove_and_other_noise_before_pv() {
    let info =
        InfoLine::parse("info depth 3 currmove e2e4 currmovenumber 1 score cp 5 pv d2d4").unwrap();
    assert_eq!(info.depth, Some(3));
    assert_eq!(info.score, Some(UciScore::Cp(5)));
    assert_eq!(info.pv, ["d2d4"]);
}
