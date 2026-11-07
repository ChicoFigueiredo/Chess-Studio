//! Carregar FEN e exportar PGN (B11, DB-02/DB-03). A lógica de arquivo é pura e testável;
//! a interface fica em `aids_panel`. O PGN é gravado na pasta de documentos do usuário
//! (sem diálogo nativo: `rfd` arrastaria GTK/portais para uma ação ocasional) e a
//! mensagem de sucesso mostra o caminho completo.

use crate::session::Session;
use bevy::prelude::Resource;
use std::io;
use std::path::{Path, PathBuf};

/// Estado dos campos "Carregar FEN…" e "Exportar PGN…".
#[derive(Resource, Debug)]
pub struct FileTools {
    pub fen_text: String,
    pub include_comments: bool,
    pub fen_result: Option<Result<(), String>>,
    pub pgn_result: Option<Result<PathBuf, String>>,
}

impl Default for FileTools {
    fn default() -> Self {
        Self {
            fen_text: String::new(),
            include_comments: true,
            fen_result: None,
            pgn_result: None,
        }
    }
}

impl FileTools {
    /// Carrega o FEN digitado. Em erro, guarda a mensagem e a Partida fica intacta.
    pub fn load_fen(&mut self, session: &mut Session) {
        self.fen_result = Some(
            session
                .load_fen(&self.fen_text)
                .map_err(|err| err.to_string()),
        );
    }

    /// Exporta o PGN para `dir`.
    pub fn export(&mut self, session: &Session, notes: &[(usize, String)], dir: &Path) {
        let wall = session_wall_ms();
        self.pgn_result = Some(
            export_pgn(&session.export_pgn(self.include_comments, notes), dir, wall)
                .map_err(|err| format!("{}: {err}", dir.display())),
        );
    }
}

fn session_wall_ms() -> Option<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
}

/// `partida-AAAA-MM-DD-HHMMSS.pgn` (UTC), ou `partida.pgn` sem relógio.
pub fn pgn_file_name(wall_ms: Option<i64>) -> String {
    let Some(ms) = wall_ms else {
        return "partida.pgn".to_owned();
    };
    let date = chess_game::pgn_date_from_unix_ms(ms).replace('.', "-");
    let secs = ms.div_euclid(1000).rem_euclid(86_400);
    format!(
        "partida-{date}-{:02}{:02}{:02}.pgn",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Grava o PGN em `dir` (criado se preciso) sem sobrescrever arquivos existentes.
pub fn export_pgn(pgn: &str, dir: &Path, wall_ms: Option<i64>) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = pgn_file_name(wall_ms);
    let stem = name.trim_end_matches(".pgn");
    for n in 0..1000 {
        let file = if n == 0 {
            name.clone()
        } else {
            format!("{stem}-{n}.pgn")
        };
        let path = dir.join(file);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                io::Write::write_all(&mut f, pgn.as_bytes())?;
                return Ok(path);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "muitos arquivos com o mesmo nome",
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::Command;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("chess-studio-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn mv(session: &mut Session, from: &str, to: &str) {
        session.command(Command::Release {
            from: from.parse().unwrap(),
            to: to.parse().unwrap(),
        });
    }

    #[test]
    fn file_name_is_dated_and_filesystem_safe() {
        // 2026-11-06 13:45:09 UTC
        assert_eq!(
            pgn_file_name(Some(1_793_972_709_000)),
            "partida-2026-11-06-134509.pgn"
        );
        assert_eq!(pgn_file_name(None), "partida.pgn");
    }

    #[test]
    fn export_writes_a_valid_pgn_and_never_overwrites() {
        let dir = temp_dir("export");
        let mut s = Session::new();
        mv(&mut s, "e2", "e4");
        mv(&mut s, "e7", "e5");
        let pgn = s.export_pgn(true, &[]);
        let first = export_pgn(&pgn, &dir, Some(1_000)).unwrap();
        let second = export_pgn(&pgn, &dir, Some(1_000)).unwrap();
        assert_ne!(first, second);
        let text = std::fs::read_to_string(first).unwrap();
        assert!(text.contains("[Event \"Partida casual\"]"));
        assert!(text.contains("1. e4 e5"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pgn_without_comments_omits_infraction_comments_and_keeps_only_legal_moves() {
        let mut s = Session::new();
        mv(&mut s, "e2", "e4");
        mv(&mut s, "e7", "e5");
        mv(&mut s, "e1", "e5"); // ilegal no Assistido: fora do PGN
        let with = s.export_pgn(true, &[]);
        let without = s.export_pgn(false, &[]);
        assert!(with.contains("1. e4 e5"));
        assert!(without.contains("1. e4 e5"));
        assert!(!without.contains('{'));
        assert!(!with.contains("Ke5"), "Lance ilegal nunca entra no PGN");
    }

    #[test]
    fn invalid_fen_reports_a_clear_error_and_changes_nothing() {
        let mut tools = FileTools::default();
        let mut s = Session::new();
        mv(&mut s, "e2", "e4");
        let snapshot = s.snapshot().clone();
        let epoch = s.epoch();
        tools.fen_text = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1".into();
        tools.load_fen(&mut s);
        let Some(Err(message)) = &tools.fen_result else {
            panic!("esperava erro");
        };
        assert!(!message.is_empty());
        assert_eq!(s.snapshot(), &snapshot);
        assert_eq!(s.epoch(), epoch);
    }

    #[test]
    fn valid_fen_starts_a_new_game_from_that_position() {
        let mut tools = FileTools::default();
        let mut s = Session::new();
        tools.fen_text = "  4k3/8/8/8/8/8/4P3/4K3 w - - 0 1  ".into();
        tools.load_fen(&mut s);
        assert_eq!(tools.fen_result, Some(Ok(())));
        assert!(s.snapshot().history.is_empty());
        assert_eq!(
            s.snapshot().position.to_fen(),
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"
        );
        assert!(
            s.export_pgn(true, &[])
                .contains("[FEN \"4k3/8/8/8/8/8/4P3/4K3 w - - 0 1\"]")
        );
    }
}
