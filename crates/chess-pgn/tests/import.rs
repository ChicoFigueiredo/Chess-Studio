#![allow(clippy::unwrap_used)]
//! Importação PGN (C6, #48): parser tolerante, árvore de variações, erros tipados.

use chess_pgn::{PgnErrorKind, PgnGame, PgnResult, parse_pgn, parse_pgn_lenient};

const CORPUS: &str = include_str!("corpus/classicas.pgn");

fn one(text: &str) -> PgnGame {
    let mut games = parse_pgn(text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(games.len(), 1, "esperava uma partida");
    games.remove(0)
}

fn err_kind(text: &str) -> (u32, u32, PgnErrorKind) {
    let e = parse_pgn(text).expect_err("devia falhar");
    (e.line, e.column, e.kind)
}

#[test]
fn corpus_tem_quatro_partidas_com_resultados_e_tags() {
    let games = parse_pgn(CORPUS).unwrap();
    assert_eq!(games.len(), 4);
    assert_eq!(games[0].tag("White"), Some("Adolf Anderssen"));
    assert_eq!(games[0].result, PgnResult::WhiteWins);
    assert_eq!(games[2].result, PgnResult::BlackWins);
    assert_eq!(games[3].result, PgnResult::Unknown);
    assert_eq!(games[0].moves.len(), 45); // 23 lances completos, mate das brancas
    assert_eq!(games[1].moves.len(), 47);
    assert_eq!(games[2].moves.len(), 82);
    assert_eq!(games[0].moves.last().unwrap().san, "Be7#");
}

#[test]
fn corpus_variacoes_aninhadas_comentarios_e_nags_ficam_na_arvore() {
    let g = &parse_pgn(CORPUS).unwrap()[0];
    // 3... Qh4+ tem uma variação (3... Nf6 4. Nc3 ...) e, dentro dela, outra em 4. Nc3.
    let qh4 = &g.moves[5];
    assert_eq!(qh4.san, "Qh4+");
    assert_eq!(qh4.variations.len(), 1);
    let var = &qh4.variations[0];
    assert_eq!(var.iter().map(|n| n.san.as_str()).collect::<Vec<_>>(), ["Nf6", "Nc3", "Bb4"]);
    assert_eq!(var[1].variations.len(), 1);
    assert_eq!(var[1].variations[0][0].san, "e5");
    assert_eq!(g.variation_count(), 2);
    // Comentário depois do lance, e NAG por glifo.
    assert_eq!(g.moves[3].comments, ["Gambito do Rei."]);
    let b5 = g.moves.iter().find(|n| n.san == "b5").unwrap();
    assert_eq!(b5.nags, [5]); // !?
    assert_eq!(b5.comments, ["O gambito do Bispo de Bryan."]);
    // `!!` e `$3` no mesmo corpus; comentário com espaços nas pontas é normalizado.
    let bd6 = g.moves.iter().find(|n| n.san == "Bd6").unwrap();
    assert_eq!(bd6.nags, [3]);
    let bxg1 = g.moves.iter().find(|n| n.san == "Bxg1").unwrap();
    assert_eq!(bxg1.nags, [3]);
    assert_eq!(bxg1.comments, ["18... Qxa1+ também ganhava."]);
    // Comentário antes do primeiro lance.
    assert_eq!(g.moves[0].pre_comments, ["A \"Imortal\": domínio público, anotações próprias."]);
}

#[test]
fn comentario_de_linha_com_ponto_e_virgula_e_numeracao_colada() {
    let g = &parse_pgn(CORPUS).unwrap()[1];
    assert_eq!(g.moves[0].pre_comments.len(), 1);
    assert!(g.moves[0].pre_comments[0].starts_with("A \"Sempre-viva\""));
    let d3 = g.moves.iter().find(|n| n.san == "d3").unwrap();
    assert_eq!(d3.comments, ["roque curto"]);
    assert_eq!(g.moves[0].san, "e4"); // "1.e4" sem espaço
}

#[test]
fn partida_a_partir_de_fen_e_promocao() {
    let g = &parse_pgn(CORPUS).unwrap()[3];
    assert_eq!(g.start_fen, "8/P7/8/8/8/8/7k/K7 w - - 0 1");
    assert_eq!(g.mainline_sans(), ["a8=Q", "Kg3", "Qa3+", "Kh4", "Qe3"]);
}

#[test]
fn tags_com_escape_e_ordem_preservada() {
    let g = one("[White \"O'Brien \\\"Bob\\\" \\\\ Jr.\"]\n[Black \"x\"]\n[Black \"y\"]\n\n1. e4 *");
    assert_eq!(g.tag("White"), Some("O'Brien \"Bob\" \\ Jr."));
    let names: Vec<_> = g.tags.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    assert_eq!(names[1..], [("Black", "x"), ("Black", "y")]);
}

#[test]
fn nags_em_todas_as_formas() {
    let g = one("1. e4! e5? 2. Nf3!! Nc6?? 3. Bb5!? a6?! $14 $32 4. Ba4 $1 $2 *");
    let nags: Vec<_> = g.moves.iter().map(|n| n.nags.clone()).collect();
    assert_eq!(
        nags,
        [vec![1], vec![2], vec![3], vec![4], vec![5], vec![6, 14, 32], vec![1, 2]]
    );
}

#[test]
fn simbolo_isolado_de_anotacao_depois_de_espaco() {
    let g = one("1. e4 ! e5 ?! *");
    assert_eq!(g.moves[0].nags, [1]);
    assert_eq!(g.moves[1].nags, [6]);
}

#[test]
fn comentario_multilinha_e_linha_de_escape_percent() {
    let g = one("% linha de escape ignorada\n1. e4 {primeira linha\n  segunda   linha} e5 *");
    assert_eq!(g.moves[0].comments, ["primeira linha segunda linha"]);
    assert_eq!(g.moves.len(), 2);
}

#[test]
fn numeracao_com_reticencias_e_roque_com_zeros() {
    let g = one("1. e4 e5 2. Nf3 Nc6 3. Bc4 Nf6 4. 0-0 Bc5 5... d6 *");
    assert_eq!(g.mainline_sans()[6], "O-O");
    assert_eq!(g.mainline_sans().len(), 9);
}

#[test]
fn variacao_a_partir_de_lance_das_pretas_usa_a_posicao_certa() {
    let g = one("1. e4 e5 (1... c5 2. Nf3 (2. c3)) (1... e6) 2. Nf3 *");
    assert_eq!(g.moves[1].variations.len(), 2);
    assert_eq!(g.moves[1].variations[0][0].san, "c5");
    assert_eq!(g.moves[1].variations[0][1].variations[0][0].san, "c3");
    assert_eq!(g.moves[1].variations[1][0].san, "e6");
}

#[test]
fn bom_crlf_e_linha_muito_longa() {
    let long = "1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 ";
    let text = format!("\u{feff}[Event \"x\"]\r\n[Result \"*\"]\r\n\r\n{long}*\r\n");
    let g = one(&text);
    assert_eq!(g.tag("Event"), Some("x"));
    assert_eq!(g.moves.len(), 8);
    // Uma linha única bem longa (comentário de 200 mil caracteres) também passa.
    let big = format!("1. e4 {{{}}} e5 *", "palavra ".repeat(25_000));
    let g = one(&big);
    assert_eq!(g.moves.len(), 2);
    assert_eq!(g.moves[0].comments[0].len(), 25_000 * 8 - 1);
}

#[test]
fn varias_partidas_sem_linha_em_branco_e_resultado_nas_tags() {
    let games = parse_pgn("[Event \"a\"]\n\n1. e4 1-0\n[Event \"b\"]\n\n1. d4 1/2-1/2\n[Event \"c\"]\n\n1. c4 0-1").unwrap();
    assert_eq!(games.len(), 3);
    assert_eq!(games[1].result, PgnResult::Draw);
    assert_eq!(games[2].result, PgnResult::BlackWins);
}

#[test]
fn partida_sem_resultado_no_movetext_usa_a_tag_result() {
    let g = one("[Result \"0-1\"]\n\n1. e4 e5");
    assert_eq!(g.result, PgnResult::BlackWins);
}

#[test]
fn texto_vazio_nao_tem_partidas() {
    assert!(parse_pgn("").unwrap().is_empty());
    assert!(parse_pgn("  \n\n \u{feff}").unwrap().is_empty());
}

// ---------------------------------------------------------------- erros

#[test]
fn lance_ilegal_informa_linha_coluna_e_o_lance() {
    let (line, col, kind) = err_kind("[Event \"x\"]\n\n1. e4 e5\n2. Nf3 Ke6 3. Qd4 *");
    assert_eq!((line, col), (4, 8));
    match kind {
        PgnErrorKind::IllegalMove { san, ply_in_line, fen, .. } => {
            assert_eq!(san, "Ke6");
            assert_eq!(ply_in_line, 4);
            assert!(fen.starts_with("rnbqkbnr"), "{fen}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lance_ilegal_dentro_de_variacao_conta_o_meio_lance_da_linha() {
    let (line, col, kind) = err_kind("1. e4 e5 (1... c5 2. Ke3) *");
    assert_eq!((line, col), (1, 22));
    assert!(matches!(kind, PgnErrorKind::IllegalMove { ply_in_line: 2, .. }), "{kind:?}");
}

#[test]
fn texto_que_nao_e_san_vira_erro_de_lance() {
    let (_, _, kind) = err_kind("1. e4 xyz *");
    assert!(matches!(kind, PgnErrorKind::IllegalMove { ref san, .. } if san == "xyz"), "{kind:?}");
}

#[test]
fn erros_estruturais_tipados() {
    assert_eq!(err_kind("1. e4 {sem fim e5 *"), (1, 7, PgnErrorKind::UnterminatedComment));
    assert_eq!(err_kind("1. e4 (1. d4 *").2, PgnErrorKind::UnclosedVariation);
    assert_eq!(err_kind("1. e4 e5) 2. Nf3 *"), (1, 9, PgnErrorKind::UnbalancedCloseParen));
    assert_eq!(err_kind("1. e4 () e5 *").2, PgnErrorKind::EmptyVariation);
    assert_eq!(err_kind("(1. d4) 1. e4 *").2, PgnErrorKind::VariationWithoutMove);
    assert_eq!(err_kind("$1 1. e4 *").2, PgnErrorKind::AnnotationWithoutMove);
    assert!(matches!(err_kind("1. e4 $999 *").2, PgnErrorKind::InvalidNag(_)));
    assert!(matches!(err_kind("1. e4 -- 2. d4 *").2, PgnErrorKind::NullMove(_)));
    assert_eq!(err_kind("[Event \"aberta\n\n1. e4 *").2, PgnErrorKind::UnterminatedTagValue);
    assert!(matches!(err_kind("[Event x]\n1. e4 *").2, PgnErrorKind::MalformedTag(_)));
    assert!(matches!(err_kind("1. e4 \"solta\" *").2, PgnErrorKind::UnexpectedToken(_)));
}

#[test]
fn fen_invalido_e_reportado_na_linha_da_tag() {
    let (line, _, kind) = err_kind("[Event \"x\"]\n[SetUp \"1\"]\n[FEN \"lixo\"]\n\n1. e4 *");
    assert_eq!(line, 3);
    assert!(matches!(kind, PgnErrorKind::InvalidFen { ref fen, .. } if fen == "lixo"), "{kind:?}");
}

#[test]
fn variacoes_aninhadas_demais_sao_erro_e_nao_estouram_a_pilha() {
    let deep = format!("1. e4 e5 {}1... c5{} *", "(1... c5 ".repeat(5_000), ")".repeat(5_000));
    assert_eq!(err_kind(&deep).2, PgnErrorKind::NestingTooDeep);
}

#[test]
fn nenhum_prefixo_truncado_do_corpus_causa_panic() {
    let chars: Vec<char> = CORPUS.chars().collect();
    for cut in (0..chars.len()).step_by(37) {
        let prefix: String = chars[..cut].iter().collect();
        let _ = parse_pgn(&prefix);
        let _ = parse_pgn_lenient(&prefix);
    }
}

#[test]
fn modo_tolerante_pula_a_partida_ruim_e_importa_as_outras() {
    let text = "[Event \"a\"]\n\n1. e4 e5 *\n\n[Event \"b\"]\n\n1. e4 e5 2. Ke6 *\n\n[Event \"c\"]\n\n1. d4 d5 *\n";
    let report = parse_pgn_lenient(text);
    assert_eq!(report.games.len(), 2);
    assert_eq!(report.games[1].tag("Event"), Some("c"));
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].game, 2);
    assert_eq!(report.errors[0].line, 7);
    assert!(report.errors[0].to_string().contains("Ke7"));
}

#[test]
fn modo_tolerante_com_tag_quebrada_nao_cria_partida_fantasma() {
    let text = "[Event \"a\"]\n[Site \"quebrada\n[Round \"1\"]\n\n1. e4 *\n\n[Event \"b\"]\n\n1. d4 *\n";
    let report = parse_pgn_lenient(text);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.games.len(), 1);
    assert_eq!(report.games[0].tag("Event"), Some("b"));
}
