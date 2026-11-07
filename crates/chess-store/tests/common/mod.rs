#![allow(clippy::unwrap_used, dead_code)]
//! Apoio dos testes: banco temporário e partidas roteirizadas.

use chess_core::Square;
use chess_game::{Command, Event, Game, GameSetup, Timestamp};
use std::path::{Path, PathBuf};

/// Diretório temporário único, apagado no `Drop`.
pub struct TempDb {
    dir: PathBuf,
}

impl TempDb {
    pub fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Contador: o relógio do macOS é grosso e dois testes na mesma fração colidiriam.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "chess-store-{tag}-{}-{nanos}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("chess.db")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

/// Joga os `(origem, destino)` no Assistido e devolve a Partida e todos os Eventos.
pub fn play(setup: GameSetup, moves: &[(&str, &str)]) -> (Game, Vec<Event>) {
    let mut game = Game::new(setup);
    let mut events = Vec::new();
    let mut t = 0;
    for (from, to) in moves {
        t += 1000;
        let out = game
            .handle(
                Command::Release {
                    from: sq(from),
                    to: sq(to),
                },
                Timestamp::with_wall(t, 1_700_000_000_000 + t as i64),
            )
            .unwrap();
        events.extend(out);
    }
    (game, events)
}
