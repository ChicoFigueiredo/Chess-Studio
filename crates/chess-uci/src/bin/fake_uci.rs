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
//! Análise: com `--info`, a Engine anuncia `MultiPV` e, em `go infinite` (modo `normal`),
//! emite `info` sintéticas determinísticas (`--info-depths <n>`, padrão 5, iterações de
//! profundidade) com `multipv` linhas cada; `--mate` troca o placar por `score mate 2`.
//! A linha `i` joga o `i`-ésimo Lance legal em ordem alfabética, seguido de respostas.
//!
//! Lc0 falso (C10): `--lc0` anuncia `Backend` (lista de `--backends a,b`, padrão
//! `cuda-auto,blas`), `MinibatchSize`, `NNCacheSize` e `Threads`; `--fail-backend <nome>` faz a
//! Engine morrer ao receber `go` com esse backend escolhido, escrevendo no stderr um erro de
//! driver CUDA (como o Lc0 real, que só carrega a rede na primeira busca).
//!
//! Opções extras: `--elo` anuncia `UCI_Elo` (min 1320, max 3190); `--trace <arquivo>`
//! acrescenta ao arquivo cada linha recebida (para os testes inspecionarem `position`,
//! `go` e `setoption`); se o arquivo não abre, a Engine sai com erro em vez de seguir sem
//! rastro. `--deaf-after-infinite`: depois de responder a `go infinite`, a Engine para de
//! ler a entrada, como um processo que não ganha CPU; o `stop` fica no cano, sem ser lido
//! nem rastreado, até a Engine ser morta (#296).

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
    let mut extra_options = false;
    let mut info = false;
    let mut mate = false;
    let mut info_depths: u32 = 5;
    let mut multipv: usize = 1;
    let mut trace: Option<std::fs::File> = None;
    let mut lc0 = false;
    let mut backends = String::from("cuda-auto,blas");
    let mut fail_backend: Option<String> = None;
    let mut backend = String::new();
    let mut deaf_after_infinite = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => mode = args.next().unwrap_or_default(),
            "--elo" => elo = true,
            "--options" => extra_options = true,
            "--info" => info = true,
            "--mate" => mate = true,
            "--lc0" => lc0 = true,
            "--backends" => backends = args.next().unwrap_or_default(),
            "--fail-backend" => fail_backend = args.next(),
            "--info-depths" => info_depths = args.next().and_then(|v| v.parse().ok()).unwrap_or(5),
            "--deaf-after-infinite" => deaf_after_infinite = true,
            "--trace" => {
                let path = args.next().unwrap_or_default();
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    Ok(file) => trace = Some(file),
                    Err(e) => {
                        eprintln!("fake_uci: o rastro {path:?} não abriu: {e}");
                        std::process::exit(2);
                    }
                }
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
                if extra_options {
                    say("option name Threads type spin default 1 min 1 max 1024");
                    say("option name Hash type spin default 16 min 1 max 33554432");
                    say("option name Debug Log File type string default <empty>");
                    say("option name WeightsFile type string default <autodiscover>");
                }
                if lc0 {
                    if !extra_options {
                        say("option name Threads type spin default 0 min 0 max 128");
                    }
                    say("option name MinibatchSize type spin default 0 min 0 max 1024");
                    say("option name NNCacheSize type spin default 2000000 min 0 max 999999999");
                    let mut line = format!(
                        "option name Backend type combo default {}",
                        backends.split(',').next().unwrap_or_default()
                    );
                    for b in backends.split(',') {
                        line.push_str(" var ");
                        line.push_str(b);
                    }
                    say(&line);
                }
                if info {
                    say("option name MultiPV type spin default 1 min 1 max 256");
                }
                if elo {
                    say("option name UCI_Elo type spin default 1320 min 1320 max 3190");
                }
                if mode != "hang-handshake" {
                    say("uciok");
                }
            }
            Some("isready") if mode != "no-ready" => say("readyok"),
            Some("setoption") => {
                if let Some(rest) = line.strip_prefix("setoption name MultiPV value ") {
                    multipv = rest.trim().parse().unwrap_or(1).max(1);
                }
                if let Some(rest) = line.strip_prefix("setoption name Backend value ") {
                    rest.trim().clone_into(&mut backend);
                }
            }
            Some("position") => {
                if let Some(p) = parse_position(&line) {
                    position = p;
                }
            }
            Some("go") => {
                if fail_backend.as_deref() == Some(backend.as_str()) {
                    eprintln!("CUDA error: no CUDA-capable device is detected");
                    std::process::exit(3);
                }
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
                    if info {
                        emit_info(&position, multipv, info_depths, mate);
                    }
                    pending_stop = true;
                    if deaf_after_infinite {
                        loop {
                            std::thread::park();
                        }
                    }
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

/// `info` sintéticas: `depths` iterações, cada uma com `multipv` linhas. O placar é do
/// ponto de vista de quem joga (como manda o UCI).
fn emit_info(position: &CanonicalPosition, multipv: usize, depths: u32, mate: bool) {
    let mut moves: Vec<Move> = position.legal_moves().to_vec();
    moves.sort_by_key(Move::to_string);
    if moves.is_empty() {
        return;
    }
    for depth in 1..=depths {
        for (i, first) in moves.iter().enumerate().take(multipv) {
            let mut line = position.clone();
            let mut pv = Vec::new();
            let mut next = Some(*first);
            while let Some(mv) = next {
                if pv.len() >= 3 {
                    break;
                }
                let Ok(after) = line.play(mv) else { break };
                pv.push(mv.to_string());
                let mut replies: Vec<Move> = after.legal_moves().to_vec();
                replies.sort_by_key(Move::to_string);
                next = replies.first().copied();
                line = after;
            }
            let score = if mate {
                String::from("mate 2")
            } else {
                let cp =
                    30 + i32::try_from(depth).unwrap_or(0) - 10 * i32::try_from(i).unwrap_or(0);
                format!("cp {cp}")
            };
            say(&format!(
                "info depth {depth} seldepth {} multipv {} score {score} nodes {} nps 1000000 time {} pv {}",
                depth + 2,
                i + 1,
                u64::from(depth) * 1000,
                depth * 10,
                pv.join(" ")
            ));
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
