//! CLI de prova: joga Engine × Engine (o mesmo executável nos dois lados), validando
//! cada Lance pelo `chess-core`, e imprime os Lances UCI.
//!
//! ```text
//! uci-probe --engine <caminho> [--engine-arg <arg>]... [--movetime <ms>] [--max-plies <n>]
//! ```
//!
//! Código de saída: 0 se a partida correu até o fim ou até `--max-plies`; 1 se a
//! Engine falhou (crash, timeout, Lance ilegal); 2 para uso incorreto.

use std::process::ExitCode;
use std::time::Duration;

use chess_core::{CanonicalPosition, Color};
use chess_uci::{Engine, EngineConfig, EngineMove, GoParams, Timeouts};

struct Args {
    engine: String,
    engine_args: Vec<String>,
    movetime: Duration,
    max_plies: u32,
}

fn parse_args() -> Result<Args, String> {
    let mut engine = None;
    let mut engine_args = Vec::new();
    let mut movetime = Duration::from_millis(100);
    let mut max_plies = 200;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} exige um valor"));
        match flag.as_str() {
            "--engine" => engine = Some(value("--engine")?),
            "--engine-arg" => engine_args.push(value("--engine-arg")?),
            "--movetime" => {
                let ms: u64 = value("--movetime")?
                    .parse()
                    .map_err(|_| "--movetime deve ser um inteiro em ms".to_owned())?;
                movetime = Duration::from_millis(ms);
            }
            "--max-plies" => {
                max_plies = value("--max-plies")?
                    .parse()
                    .map_err(|_| "--max-plies deve ser um inteiro".to_owned())?;
            }
            other => return Err(format!("argumento desconhecido: {other}")),
        }
    }
    Ok(Args {
        engine: engine.ok_or("falta --engine <caminho>")?,
        engine_args,
        movetime,
        max_plies,
    })
}

fn launch(args: &Args) -> Result<Engine, String> {
    let mut config = EngineConfig::new(&args.engine).timeouts(Timeouts {
        search_slack: Duration::from_secs(2),
        ..Timeouts::default()
    });
    for a in &args.engine_args {
        config = config.arg(a);
    }
    Engine::spawn(config).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("erro: {msg}");
            eprintln!(
                "uso: uci-probe --engine <caminho> [--engine-arg <arg>]... [--movetime <ms>] [--max-plies <n>]"
            );
            return ExitCode::from(2);
        }
    };
    match play(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("erro: {msg}");
            ExitCode::from(1)
        }
    }
}

fn play(args: &Args) -> Result<(), String> {
    let mut white = launch(args)?;
    let mut black = launch(args)?;
    println!(
        "Brancas e Pretas: {}",
        white.info().name.as_deref().unwrap_or("(sem nome)")
    );
    for e in [&mut white, &mut black] {
        e.new_game().map_err(|e| e.to_string())?;
    }

    let go = GoParams::movetime(args.movetime);
    let mut position = CanonicalPosition::start();
    let mut plies = 0;
    let reason = loop {
        if position.legal_moves().is_empty() {
            break if position.is_check() {
                "xeque-mate"
            } else {
                "afogamento"
            };
        }
        if position.halfmove_clock() >= 100 {
            break "regra dos 50 lances";
        }
        if plies >= args.max_plies {
            break "limite de meios-lances";
        }
        let side = position.side_to_move();
        let engine = if side == Color::White {
            &mut white
        } else {
            &mut black
        };
        let outcome = engine.search_legal_move(&position, &go).map_err(|e| {
            let log: Vec<String> = engine
                .log()
                .iter()
                .rev()
                .take(5)
                .map(|l| l.line.clone())
                .collect();
            format!("{e} (últimas linhas: {log:?})")
        })?;
        match outcome {
            EngineMove::Played { mv, next } => {
                plies += 1;
                let tag = if side == Color::White { "b" } else { "p" };
                println!("{plies:>3} {tag} {mv}");
                position = next;
            }
            EngineMove::NoMove => break "Engine sem Lance",
        }
    };
    println!(
        "Fim: {plies} meios-lances ({reason}); FEN final: {}",
        position.to_fen()
    );
    white.quit();
    black.quit();
    Ok(())
}
