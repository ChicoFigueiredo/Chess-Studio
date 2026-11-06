#![allow(clippy::unwrap_used)]
//! DB-05: matar o processo no meio de `append_events` não deixa Evento parcial.
//!
//! O próprio binário de teste é re-executado como "filho gravador" (variável de
//! ambiente `CHESS_STORE_CRASH_DB`), que grava lotes em laço até levar SIGKILL
//! (`Child::kill`). O pai então reabre o banco e confere integridade e atomicidade.

mod common;

use chess_core::Color;
use chess_game::{Event, EventKind, GameSetup, PolicyKind, Timestamp};
use chess_store::{GameId, GameMetadata, Store};
use common::TempDb;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

const ENV_DB: &str = "CHESS_STORE_CRASH_DB";
const ENV_GAME: &str = "CHESS_STORE_CRASH_GAME";
/// Grande o bastante para a transação durar o suficiente para ser interrompida.
const BATCH: usize = 3_000;

fn batch(round: u64) -> Vec<Event> {
    (0..BATCH as u64)
        .map(|i| {
            Event::new(
                Timestamp::mono(round * BATCH as u64 + i),
                EventKind::IllegalMoveRejected {
                    side: Color::White,
                    from: "e2".parse().unwrap(),
                    to: "e5".parse().unwrap(),
                    reason: chess_game::IllegalReason::NotLegal { in_check: false },
                },
            )
        })
        .collect()
}

/// Papel de filho: só age quando o pai o dispara pela variável de ambiente.
#[test]
fn crash_child_writer() {
    let Ok(path) = std::env::var(ENV_DB) else {
        return;
    };
    let game = GameId(std::env::var(ENV_GAME).unwrap().parse().unwrap());
    let mut store = Store::open(path).unwrap();
    println!("READY");
    let mut round = 0;
    loop {
        store.append_events(game, &batch(round)).unwrap();
        round += 1;
    }
}

fn run_and_kill(delay: Duration) -> usize {
    let db = TempDb::new("crash");
    let game = {
        let mut store = Store::open(db.path()).unwrap();
        store
            .create_game(
                &GameSetup::standard(PolicyKind::Assisted),
                &GameMetadata::default(),
            )
            .unwrap()
    };

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "crash_child_writer",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_DB, db.path())
        .env(ENV_GAME, game.0.to_string())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    loop {
        let line = lines
            .next()
            .expect("filho encerrou antes de gravar")
            .unwrap();
        if line.contains("READY") {
            break;
        }
    }
    std::thread::sleep(delay);
    child.kill().unwrap(); // SIGKILL
    let status = child.wait().unwrap();
    assert!(!status.success(), "o filho deveria ter morrido por sinal");

    // Integridade do arquivo.
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    drop(conn);

    // Atomicidade: só lotes inteiros (ou nada) e sequência contígua.
    let store = Store::open(db.path()).unwrap();
    let loaded = store.load_game(game).unwrap();
    let count = loaded.events().len();
    assert_eq!(count % BATCH, 0, "evento parcial no banco: {count} eventos");
    println!("eventos após a queda ({delay:?}): {count}");
    count
}

#[test]
fn killing_the_writer_mid_batch_leaves_whole_batches_or_nothing() {
    let counts: Vec<usize> = [0, 30, 120, 400, 900]
        .into_iter()
        .map(|ms| run_and_kill(Duration::from_millis(ms)))
        .collect();
    // Prova de que o laço realmente gravou antes da queda em alguma rodada.
    assert!(
        counts.iter().any(|&c| c > 0),
        "nenhuma rodada gravou: {counts:?}"
    );
}
