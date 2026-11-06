#![allow(clippy::unwrap_used)]
//! Exportação PGN (DEC-44, AC-08, DB-04, BDD-07) e revisão Lance a Lance (CORE-10).

mod common;
use chess_core::Color;
use chess_game::{Command, Game, PgnOptions, PolicyKind};
use common::*;

fn opts() -> PgnOptions {
    PgnOptions {
        event: "Casual".into(),
        site: "Chess Studio".into(),
        date: Some("2026.11.06".into()),
        round: "1".into(),
        white: "Paul Morphy".into(),
        black: "Duque e Conde".into(),
        ..PgnOptions::default()
    }
}

const OPERA: &str = "e4 e5 Nf3 d6 d4 Bg4 dxe5 Bxf3 Qxf3 dxe5 Bc4 Nf6 Qb3 Qe7 Nc3 c6 Bg5 b5 \
                     Nxb5 cxb5 Bxb5+ Nbd7 O-O-O Rd8 Rxd7 Rxd7 Rd1 Qe6 Bxd7+ Nxd7 Qb8+ Nxb8 Rd8#";

#[test]
fn opera_de_morphy_gera_o_texto_esperado() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, OPERA);

    let expected = "\
[Event \"Casual\"]
[Site \"Chess Studio\"]
[Date \"2026.11.06\"]
[Round \"1\"]
[White \"Paul Morphy\"]
[Black \"Duque e Conde\"]
[Result \"1-0\"]
[TimeControl \"-\"]
[Termination \"normal\"]
[Arbitration \"Assisted\"]

1. e4 e5 2. Nf3 d6 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5 6. Bc4 Nf6 7. Qb3 Qe7 8.
Nc3 c6 9. Bg5 b5 10. Nxb5 cxb5 11. Bxb5+ Nbd7 12. O-O-O Rd8 13. Rxd7 Rxd7 14.
Rd1 Qe6 15. Bxd7+ Nxd7 16. Qb8+ Nxb8 17. Rd8# 1-0
";
    assert_eq!(game.to_pgn(&opts()), expected);
}

#[test]
fn nenhuma_linha_passa_de_80_colunas() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, OPERA);
    for line in game.to_pgn(&opts()).lines() {
        assert!(line.chars().count() <= 80, "linha longa: {line}");
    }
}

#[test]
fn partida_em_andamento_termina_com_asterisco_e_termination_unterminated() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 c5");
    let pgn = game.to_pgn(&opts());
    assert!(pgn.contains("[Result \"*\"]"));
    assert!(pgn.contains("[Termination \"unterminated\"]"));
    assert!(pgn.trim_end().ends_with("1. e4 c5 *"));
}

#[test]
fn partida_a_partir_de_fen_com_pretas_a_jogar_traz_setup_fen_e_numeracao_com_reticencias() {
    let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let mut game = assisted_from(fen);
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "c5 Nf3 d6");
    let pgn = game.to_pgn(&opts());
    assert!(pgn.contains("[SetUp \"1\"]"));
    assert!(pgn.contains(&format!("[FEN \"{fen}\"]")));
    assert!(pgn.contains("\n1... c5 2. Nf3 d6 *"), "{pgn}");
}

#[test]
fn partida_padrao_nao_traz_setup_nem_fen() {
    let pgn = assisted().to_pgn(&opts());
    assert!(!pgn.contains("SetUp") && !pgn.contains("[FEN"));
}

#[test]
fn tag_engine_elo_e_time_control() {
    let setup = chess_game::GameSetup::standard(PolicyKind::Assisted).with_time_control(
        chess_game::TimeControl {
            base_ms: 90 * 60_000,
            increment_ms: 30_000,
        },
    );
    let game = Game::new(setup);
    let o = PgnOptions {
        engine_elo: Some(1500),
        ..opts()
    };
    let pgn = game.to_pgn(&o);
    assert!(pgn.contains("[TimeControl \"5400+30\"]"));
    assert!(pgn.contains("[EngineElo \"1500\"]"));
    assert!(!game.to_pgn(&opts()).contains("EngineElo"));
}

#[test]
fn escapa_aspas_e_barra_nas_tags() {
    let o = PgnOptions {
        white: "Jo\"ao \\ Silva".into(),
        ..opts()
    };
    assert!(
        assisted()
            .to_pgn(&o)
            .contains("[White \"Jo\\\"ao \\\\ Silva\"]")
    );
}

#[test]
fn nomes_vazios_viram_interrogacao() {
    let o = PgnOptions {
        white: String::new(),
        ..opts()
    };
    assert!(assisted().to_pgn(&o).contains("[White \"?\"]"));
}

#[test]
fn infracao_vira_comentario_no_ponto_certo_e_nao_vira_lance() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        90 * 60_000,
        30_000,
    );
    let t = fide_play(&mut game, 0, "e4");
    // As pretas soltam a torre a8 em a5 (bloqueada pelo peão a7): Lance ilegal.
    touch(&mut game, t, "a8").unwrap();
    drop_piece(&mut game, t + 1, "a8", "a5").unwrap();
    press(&mut game, t + 2).unwrap();
    let t = fide_play(&mut game, t + 3, "e5");
    let _ = t;
    let pgn = game.to_pgn(&opts());

    assert!(pgn.contains("[Arbitration \"FIDE Simulation"), "{pgn}");
    assert!(pgn.contains("[TimeControl \"5400+30\"]"));
    let body = pgn.split("\n\n").nth(1).unwrap();
    let flat = body.replace('\n', " ");
    assert!(flat.starts_with("1. e4 {"), "{flat}");
    assert!(flat.contains("Infração"), "{flat}");
    assert!(!flat.contains("Ra5"), "Lance ilegal não é Lance: {flat}");
    assert!(flat.contains("} 1... e5 *"), "{flat}");
}

#[test]
fn sem_comentarios_omite_infracoes() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        90 * 60_000,
        0,
    );
    touch(&mut game, 1, "a1").unwrap();
    drop_piece(&mut game, 2, "a1", "b2").unwrap();
    press(&mut game, 3).unwrap();
    let com = game.to_pgn(&opts());
    let sem = game.to_pgn(&PgnOptions {
        include_comments: false,
        ..opts()
    });
    assert!(com.contains('{'));
    assert!(!sem.contains('{'));
}

#[test]
fn segunda_infracao_gera_termination_regras_e_resultado() {
    let mut game = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        90 * 60_000,
        0,
    );
    press(&mut game, 1_000).unwrap();
    press(&mut game, 2_000).unwrap();
    let pgn = game.to_pgn(&opts());
    assert!(pgn.contains("[Result \"0-1\"]"), "{pgn}");
    assert!(pgn.contains("[Termination \"rules infraction\"]"));
    assert_eq!(pgn.matches('{').count(), pgn.matches('}').count());
    assert!(pgn.matches('{').count() >= 2);
}

#[test]
fn abandono_tem_resultado_e_termination_normal() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    game.handle(Command::Resign { side: Color::Black }, clock.now())
        .unwrap();
    let pgn = game.to_pgn(&opts());
    assert!(pgn.contains("[Result \"1-0\"]"));
    assert!(pgn.contains("[Termination \"normal\"]"));
}

#[test]
fn desfazer_remove_o_lance_do_pgn() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    game.handle(Command::Undo, clock.now()).unwrap();
    play_san(&mut game, &mut clock, "c5");
    let pgn = game.to_pgn(&opts());
    assert!(pgn.trim_end().ends_with("1. e4 c5 *"), "{pgn}");
}

#[test]
fn a_exportacao_nao_altera_a_partida() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "e4 e5");
    let before = (game.snapshot(), game.events().len());
    let _ = game.to_pgn(&opts());
    assert_eq!((game.snapshot(), game.events().len()), before);
}

#[test]
fn data_a_partir_de_milissegundos_unix() {
    assert_eq!(chess_game::pgn_date_from_unix_ms(0), "1970.01.01");
    assert_eq!(
        chess_game::pgn_date_from_unix_ms(1_793_923_200_000),
        "2026.11.06"
    );
}

// ---- Revisão Lance a Lance ------------------------------------------------

#[test]
fn revisao_navega_lance_a_lance_numa_partida_terminada_sem_altera_la() {
    let mut game = assisted();
    let mut clock = FakeClock::new();
    play_all(&mut game, &mut clock, "f3 e5 g4 Qh4#");
    let final_snapshot = game.snapshot();
    let events = game.events().len();

    let start = game.review_at_ply(0).unwrap();
    assert_eq!(
        start.position.to_fen(),
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    );
    assert!(start.last_move.is_none() && start.is_start && !start.is_end);
    assert_eq!(start.total_plies, 4);

    let two = game.review_at_ply(2).unwrap();
    assert_eq!(two.last_move.as_ref().unwrap().san, "e5");
    assert_eq!(two.side_to_move, Color::White);

    let last = game.review_at_ply(4).unwrap();
    assert!(last.is_end && last.in_check);
    assert_eq!(last.position, final_snapshot.position);

    assert!(game.review_at_ply(5).is_none());
    assert_eq!(game.snapshot(), final_snapshot);
    assert_eq!(game.events().len(), events);
    assert!(game.snapshot().outcome.is_some());
}

#[test]
fn revisao_nas_politicas_competitivas_nao_e_desfazer() {
    let mut game = fide();
    fide_play(&mut game, 0, "e4");
    let before = game.snapshot();
    let view = game.review_at_ply(0).unwrap();
    assert!(view.is_start);
    assert_eq!(game.snapshot(), before);
    assert_eq!(game.snapshot().history.len(), 1);
    assert!(
        game.events()
            .iter()
            .all(|e| !matches!(e.kind, chess_game::EventKind::MoveUndone { .. }))
    );
}

// ---- Validação por ferramenta independente (DB-03) -------------------------

/// Reabre o PGN exportado com python-chess (GPL: só como ferramenta externa de
/// teste, nunca dependência). Rode com:
/// `PYTHON=/caminho/do/python cargo test -p chess-game --test pgn_export -- --ignored`
/// (o Python precisa ter `pip install chess`; `PYTHON` padrão: `python3`).
#[test]
#[ignore = "exige python-chess instalado (ferramenta externa, DB-03)"]
fn python_chess_reabre_o_pgn_exportado() {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".into());
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/validate_pgn.py");
    let dir = std::env::temp_dir().join(format!("chess-studio-pgn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let mut cases: Vec<(&str, Game, usize)> = Vec::new();
    let mut clock = FakeClock::new();
    let mut opera = assisted();
    play_all(&mut opera, &mut clock, OPERA);
    cases.push(("opera", opera, 33));

    let mut fen = assisted_from("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
    play_all(&mut fen, &mut clock, "c5 Nf3 d6");
    cases.push(("fen", fen, 3));

    let mut infraction = fide_timed_from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        90 * 60_000,
        30_000,
    );
    let t = fide_play(&mut infraction, 0, "e4");
    touch(&mut infraction, t, "a8").unwrap();
    drop_piece(&mut infraction, t + 1, "a8", "a5").unwrap();
    press(&mut infraction, t + 2).unwrap();
    fide_play(&mut infraction, t + 3, "e5");
    cases.push(("infracao", infraction, 2));

    for (name, game, plies) in cases {
        let file = dir.join(format!("{name}.pgn"));
        std::fs::write(&file, game.to_pgn(&opts())).unwrap();
        let out = std::process::Command::new(&python)
            .args(["-I", script])
            .arg(&file)
            .arg(plies.to_string())
            .arg(game.snapshot().position.to_fen())
            .output()
            .expect("python executável");
        assert!(
            out.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
