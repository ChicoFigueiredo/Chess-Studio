//! Engine UCI falsa para testes (ADR-0003: nunca depende de Engine real).
//!
//! Uso: `fake_uci [--mode <modo>] [--delay-ms <n>]`. Joga o primeiro Lance legal em
//! ordem alfabética, usando o `chess-core`, para que partidas completas sejam
//! determinísticas. Modos:
//!
//! - `normal`: handshake e busca corretos. `go infinite` só responde após `stop`.
//! - `illegal`: responde `bestmove a1a1` (Lance ilegal em qualquer Posição).
//! - `malformed`: responde `bestmove xyz` (texto que não é Lance UCI).
//! - `none`: responde `bestmove (none)`.
//! - `hang`: aceita `go` mas nunca responde `bestmove` (nem a `stop`).
//! - `hang-handshake`: nunca envia `uciok`.
//! - `no-ready`: nunca responde `isready`.
//! - `crash`: ao receber `go`, escreve uma linha `info` e termina com código 3.
//! - `slow`: espera `--delay-ms` (ignorando `stop`) antes de responder `bestmove`.
//! - `garbage`: intercala linhas de lixo e vazias com respostas válidas.
//! - `ignore-quit`: ignora `quit` e continua vivo.
//!
//! Opções extras: `--elo` anuncia `UCI_Elo` (min 1320, max 3190); `--trace <arquivo>`
//! acrescenta ao arquivo cada linha recebida (para os testes inspecionarem `position`,
//! `go` e `setoption`).

use std::io::{self, BufRead, Write};
use std::time::Duration;

use chess_core::{CanonicalPosition, Move};

fn say(line: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn main() {
    let mut mode = String::from("normal");
    let mut delay_ms: u64 = 0;
    let mut elo = false;
    let mut trace: Option<std::fs::File> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => mode = args.next().unwrap_or_default(),
            "--elo" => elo = true,
            "--trace" => {
                trace = args.next().and_then(|p| {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .ok()
                });
            }
            "--delay-ms" => delay_ms = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }

    let stdin = io::stdin();
    let mut position = CanonicalPosition::start();
    let mut pending_stop = false;
    let garbage = mode == "garbage";

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim().to_owned();
        if let Some(file) = trace.as_mut() {
            let _ = writeln!(file, "{line}");
        }
        match line.split_whitespace().next() {
            Some("uci") => {
                if garbage {
                    say("???? not a uci line ????");
                    say("");
                }
                say("id name FakeUci 1.0");
                say("id author Chess Studio tests");
                say("option name Skill Level type spin default 20 min 0 max 20");
                say("option name UCI_LimitStrength type check default false");
                if elo {
                    say("option name UCI_Elo type spin default 1320 min 1320 max 3190");
                }
                if mode != "hang-handshake" {
                    say("uciok");
                }
            }
            Some("isready") if mode != "no-ready" => say("readyok"),
            Some("position") => {
                if let Some(p) = parse_position(&line) {
                    position = p;
                }
            }
            Some("go") => {
                if mode == "crash" {
                    say("info depth 1 score cp 10");
                    std::process::exit(3);
                }
                if mode == "hang" {
                    continue;
                }
                if garbage {
                    say("info depth 1 seldepth 1 score cp 13 pv a2a3");
                    say("\u{1}\u{2} lixo binario ~~");
                    say("");
                }
                if mode == "slow" {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
                if line.contains("infinite") && mode == "normal" {
                    pending_stop = true;
                    continue;
                }
                answer(&mode, &position);
            }
            Some("stop") => {
                if pending_stop {
                    pending_stop = false;
                    answer(&mode, &position);
                }
            }
            Some("quit") if mode != "ignore-quit" => break,
            _ => {}
        }
    }
    // `ignore-quit` também ignora o fechamento do stdin: só morre morto à força.
    if mode == "ignore-quit" {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

fn answer(mode: &str, position: &CanonicalPosition) {
    let text = match mode {
        "illegal" => String::from("bestmove a1a1"),
        "malformed" => String::from("bestmove xyz"),
        "none" => String::from("bestmove (none)"),
        _ => {
            let mut moves: Vec<String> =
                position.legal_moves().iter().map(Move::to_string).collect();
            moves.sort();
            match moves.first() {
                Some(m) => format!("bestmove {m}"),
                None => String::from("bestmove (none)"),
            }
        }
    };
    say(&text);
}

fn parse_position(line: &str) -> Option<CanonicalPosition> {
    let rest = line.strip_prefix("position")?.trim();
    let (base, moves) = match rest.split_once(" moves ") {
        Some((b, m)) => (b.trim(), m.trim()),
        None => (rest.trim_end_matches(" moves").trim(), ""),
    };
    let mut position = if base == "startpos" {
        CanonicalPosition::start()
    } else {
        CanonicalPosition::from_fen(base.strip_prefix("fen")?.trim()).ok()?
    };
    for text in moves.split_whitespace() {
        position = position.play(text.parse().ok()?).ok()?;
    }
    Some(position)
}
