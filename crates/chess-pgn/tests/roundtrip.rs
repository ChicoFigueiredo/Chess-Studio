#![allow(clippy::unwrap_used)]
//! Ida e volta: exportar e reimportar dá a mesma árvore; o PGN que o app exporta
//! hoje (`Game::to_pgn`) é importado sem perda; a linha principal vira uma `Game`.

use chess_core::Square;
use chess_game::{
    Command, EventKind, Game, GameResult, GameSetup, PgnOptions, PolicyKind, TimeControl, Timestamp,
};
use chess_pgn::{ConvertError, MoveNode, PgnGame, PgnResult, parse_pgn};

const CORPUS: &str = include_str!("corpus/classicas.pgn");

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

fn play(game: &mut Game, ms: &mut u64, san: &str) {
    let mv = game.snapshot().position.parse_san(san).unwrap();
    let mut cmd = |c: Command| {
        *ms += 1;
        game.handle(c, Timestamp::mono(*ms)).unwrap();
    };
    cmd(Command::Release {
        from: mv.from(),
        to: mv.to(),
    });
    if let Some(piece) = mv.promotion() {
        cmd(Command::ChoosePromotion { piece });
    }
}

#[test]
fn exportar_e_reimportar_o_corpus_da_a_mesma_arvore() {
    let games = parse_pgn(CORPUS).unwrap();
    for original in &games {
        let text = original.to_pgn();
        let again = parse_pgn(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].moves, original.moves, "{text}");
        assert_eq!(again[0].result, original.result);
        assert_eq!(again[0].start_fen, original.start_fen);
        assert_eq!(again[0].tags, original.tags);
        // E o texto é estável: exportar de novo não muda nada.
        assert_eq!(again[0].to_pgn(), text);
    }
}

#[test]
fn exportacao_tem_str_primeiro_linhas_curtas_e_variacoes_entre_parenteses() {
    let g = &parse_pgn(CORPUS).unwrap()[0];
    let text = g.to_pgn();
    let tag_names: Vec<&str> = text
        .lines()
        .take_while(|l| l.starts_with('['))
        .map(|l| l[1..].split(' ').next().unwrap())
        .collect();
    assert_eq!(
        tag_names[..7],
        ["Event", "Site", "Date", "Round", "White", "Black", "Result"]
    );
    assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    assert!(text.contains("(3... Nf6 4. Nc3 (4. e5 d5) 4... Bb4)"), "{text}");
    assert!(text.trim_end().ends_with("23. Be7# 1-0"));
}

#[test]
fn tags_ausentes_do_str_saem_com_interrogacao_e_result_vem_do_modelo() {
    let g = &parse_pgn("1. e4 e5 1/2-1/2").unwrap()[0];
    let text = g.to_pgn();
    assert!(text.contains("[Event \"?\"]") && text.contains("[Date \"????.??.??\"]"));
    assert!(text.contains("[Result \"1/2-1/2\"]"));
}

#[test]
fn arvore_montada_a_mao_exporta_comentarios_nags_e_numeracao_das_pretas() {
    let mut e4 = MoveNode::new("e4");
    e4.nags = vec![1];
    e4.comments = vec!["rei".into()];
    let mut e5 = MoveNode::new("e5");
    e5.variations = vec![vec![MoveNode::new("c5"), MoveNode::new("Nf3")]];
    let game = PgnGame {
        tags: vec![],
        start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
        moves: vec![e4, e5, MoveNode::new("Nf3")],
        result: PgnResult::Unknown,
        comments: vec![],
    };
    let text = game.to_pgn();
    assert!(text.contains("1. e4 $1 {rei} 1... e5 (1... c5 2. Nf3) 2. Nf3 *"), "{text}");
    assert_eq!(parse_pgn(&text).unwrap()[0].moves, game.moves);
}

#[test]
fn arvore_serializa_em_json_sem_perda() {
    let games = parse_pgn(CORPUS).unwrap();
    let json = serde_json::to_string(&games).unwrap();
    let back: Vec<PgnGame> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, games);
}

// ---- compatibilidade com o exportador do app (P0) ----

#[test]
fn pgn_exportado_pelo_app_e_importado_sem_perda() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    let mut ms = 0;
    let opera = "e4 e5 Nf3 d6 d4 Bg4 dxe5 Bxf3 Qxf3 dxe5 Bc4 Nf6 Qb3 Qe7 Nc3 c6 Bg5 b5 \
                 Nxb5 cxb5 Bxb5+ Nbd7 O-O-O Rd8 Rxd7 Rxd7 Rd1 Qe6 Bxd7+ Nxd7 Qb8+ Nxb8 Rd8#";
    for san in opera.split_whitespace() {
        play(&mut game, &mut ms, san);
    }
    let options = PgnOptions {
        white: "Paul Morphy".into(),
        black: "Duque \"e\" Conde".into(),
        date: Some("2026.11.06".into()),
        ..PgnOptions::default()
    };
    let parsed = &parse_pgn(&game.to_pgn(&options)).unwrap()[0];
    let history: Vec<String> = game.snapshot().history.iter().map(|h| h.san.clone()).collect();
    assert_eq!(parsed.mainline_sans(), history.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(parsed.result, PgnResult::WhiteWins);
    assert_eq!(parsed.tag("Black"), Some("Duque \"e\" Conde"));
    assert_eq!(parsed.tag("Termination"), Some("normal"));
    assert_eq!(parsed.variation_count(), 0);
}

#[test]
fn comentarios_de_infracao_do_app_voltam_como_comentarios_no_lance_certo() {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let mut game = Game::new(
        GameSetup::from_fen(PolicyKind::FideSimulation, fen)
            .unwrap()
            .with_time_control(TimeControl {
                base_ms: 90 * 60_000,
                increment_ms: 30_000,
            }),
    );
    let at = |ms| Timestamp::mono(ms);
    let h = |game: &mut Game, ms, c| game.handle(c, at(ms)).unwrap();
    h(&mut game, 1, Command::Touch { square: sq("e2") });
    h(&mut game, 2, Command::Release { from: sq("e2"), to: sq("e4") });
    h(&mut game, 3, Command::PressClock);
    // As pretas soltam a torre a8 em a5 (bloqueada): Infração.
    h(&mut game, 4, Command::Touch { square: sq("a8") });
    h(&mut game, 5, Command::Release { from: sq("a8"), to: sq("a5") });
    h(&mut game, 6, Command::PressClock);
    h(&mut game, 7, Command::Touch { square: sq("e7") });
    h(&mut game, 8, Command::Release { from: sq("e7"), to: sq("e5") });
    h(&mut game, 9, Command::PressClock);
    assert!(game.events().iter().any(|e| matches!(e.kind, EventKind::InfractionRecorded { .. })));

    let parsed = &parse_pgn(&game.to_pgn(&PgnOptions::default())).unwrap()[0];
    assert_eq!(parsed.mainline_sans(), ["e4", "e5"]);
    assert_eq!(parsed.moves[0].comments.len(), 1);
    assert!(parsed.moves[0].comments[0].starts_with("Infração 1 das pretas"), "{:?}", parsed.moves[0]);
    assert_eq!(parsed.tag("TimeControl"), Some("5400+30"));
}

// ---- conversão para Game ----

#[test]
fn linha_principal_vira_partida_assistida_com_o_mesmo_historico() {
    let games = parse_pgn(CORPUS).unwrap();
    let game = games[0].to_assisted_game().unwrap();
    let snap = game.snapshot();
    let sans: Vec<&str> = snap.history.iter().map(|h| h.san.as_str()).collect();
    assert_eq!(sans, games[0].mainline_sans());
    assert_eq!(snap.outcome.unwrap().result, GameResult::WhiteWins);
    assert_eq!(game.setup().policy(), PolicyKind::Assisted);
}

#[test]
fn conversao_respeita_fen_inicial_e_promocao() {
    let games = parse_pgn(CORPUS).unwrap();
    let game = games[3].to_assisted_game().unwrap();
    assert_eq!(game.setup().start().to_fen(), games[3].start_fen);
    assert_eq!(game.snapshot().history.len(), 5);
    assert_eq!(game.snapshot().history[0].san, "a8=Q");
}

#[test]
fn conversao_de_arvore_invalida_falha_com_erro_tipado() {
    let mut g = parse_pgn("1. e4 e5 *").unwrap().remove(0);
    g.moves[1].san = "Ke6".into(); // árvore adulterada depois de importada
    match g.to_assisted_game() {
        Err(ConvertError::IllegalMove { ply, san }) => assert_eq!((ply, san.as_str()), (2, "Ke6")),
        other => panic!("{other:?}"),
    }
    g.start_fen = "lixo".into();
    assert!(matches!(g.to_assisted_game(), Err(ConvertError::InvalidFen(_))));
}
