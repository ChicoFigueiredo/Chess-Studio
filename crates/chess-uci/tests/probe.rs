//! A CLI de prova joga Engine × Engine validando cada Lance pelo núcleo.

use std::process::{Command, Output};

const PROBE: &str = env!("CARGO_BIN_EXE_uci-probe");
const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");

fn run(extra: &[&str]) -> Output {
    Command::new(PROBE)
        .args(["--engine", FAKE, "--movetime", "20"])
        .args(extra)
        .output()
        .expect("uci-probe deve executar")
}

#[test]
fn plays_the_requested_number_of_plies_printing_uci_moves() {
    let out = run(&["--max-plies", "12"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let moves: Vec<&str> = stdout
        .lines()
        .filter_map(|l| l.split_whitespace().nth(2))
        .filter(|w| w.len() == 4 && w.chars().next().is_some_and(|c| ('a'..='h').contains(&c)))
        .collect();
    assert_eq!(moves.len(), 12, "{stdout}");
    // Engine falsa: primeiro lance em ordem alfabética (1.a3 a5 2.Ra2 ...).
    assert_eq!(&moves[..3], ["a2a3", "a7a5", "a1a2"]);
    assert!(stdout.contains("Fim: 12 meios-lances (limite de meios-lances)"));
}

#[test]
fn fails_with_a_clear_message_when_the_engine_proposes_an_illegal_move() {
    let out = run(&["--engine-arg", "--mode", "--engine-arg", "illegal"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ilegal"), "{stderr}");
}

#[test]
fn rejects_missing_engine_argument() {
    let out = Command::new(PROBE).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
