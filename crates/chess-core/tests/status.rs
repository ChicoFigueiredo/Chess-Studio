use chess_core::{CanonicalPosition, Color, GameStatus};

fn pos(fen: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(fen).expect("FEN válido")
}

fn insufficient(fen: &str) -> bool {
    pos(fen).is_insufficient_material()
}

#[test]
fn start_position_is_ongoing() {
    assert_eq!(CanonicalPosition::start().status(), GameStatus::Ongoing);
}

#[test]
fn fools_mate_is_checkmate_won_by_black() {
    let p = pos("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3");
    assert!(p.is_check());
    assert_eq!(
        p.status(),
        GameStatus::Checkmate {
            winner: Color::Black
        }
    );
}

#[test]
fn back_rank_mate_is_won_by_white() {
    let p = pos("6k1/5ppp/8/8/8/8/8/R5K1 b - - 0 1");
    assert_eq!(p.status(), GameStatus::Ongoing);
    let p = pos("R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1");
    assert_eq!(
        p.status(),
        GameStatus::Checkmate {
            winner: Color::White
        }
    );
}

#[test]
fn stalemate_is_never_checkmate() {
    // CORE-06: sem lances legais e fora de xeque é afogamento.
    let p = pos("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
    assert!(!p.is_check());
    assert!(p.legal_moves().is_empty());
    assert_eq!(p.status(), GameStatus::Stalemate);
}

#[test]
fn check_with_escape_is_ongoing() {
    let p = pos("R3k3/8/8/8/8/8/8/4K3 b - - 0 1");
    assert!(p.is_check());
    assert_eq!(p.status(), GameStatus::Ongoing);
}

#[test]
fn insufficient_material_fide_cases() {
    assert!(insufficient("4k3/8/8/8/8/8/8/4K3 w - - 0 1"), "K x K");
    assert!(insufficient("4k3/8/8/8/8/8/8/3BK3 w - - 0 1"), "K+B x K");
    assert!(insufficient("4k3/8/8/8/8/8/8/3NK3 w - - 0 1"), "K+N x K");
    assert!(insufficient("3bk3/8/8/8/8/8/8/4K3 w - - 0 1"), "K x K+B");
    // Bispos da mesma cor: c1 e f8 são casas escuras.
    assert!(
        insufficient("5b1k/8/8/8/8/8/8/2B1K3 w - - 0 1"),
        "K+B x K+B, mesma cor"
    );
}

#[test]
fn material_that_is_not_insufficient() {
    assert!(!insufficient("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"), "peão");
    assert!(!insufficient("4k3/8/8/8/8/8/8/3RK3 w - - 0 1"), "torre");
    assert!(!insufficient("4k3/8/8/8/8/8/8/3QK3 w - - 0 1"), "dama");
    assert!(
        !insufficient("4k3/8/8/8/8/8/8/2NNK3 w - - 0 1"),
        "dois cavalos"
    );
    assert!(!insufficient("4kn2/8/8/8/8/8/8/3NK3 w - - 0 1"), "N x N");
    assert!(!insufficient("4kb2/8/8/8/8/8/8/3NK3 w - - 0 1"), "N x B");
    assert!(!insufficient("4k3/8/8/8/8/8/8/2BNK3 w - - 0 1"), "B+N x K");
    assert!(
        !insufficient("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1"),
        "dois bispos de cores opostas"
    );
    assert!(
        !insufficient("2b1k3/8/8/8/8/8/8/2B1K3 w - - 0 1"),
        "bispos de cores opostas, um de cada lado"
    );
}

#[test]
fn can_checkmate_by_material() {
    let kk = pos("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
    assert!(!kk.can_checkmate(Color::White));
    assert!(!kk.can_checkmate(Color::Black));

    // Peão, torre ou dama bastam.
    let p = pos("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
    assert!(p.can_checkmate(Color::White));
    assert!(!p.can_checkmate(Color::Black));
    assert!(pos("4k3/8/8/8/8/8/8/3RK3 w - - 0 1").can_checkmate(Color::White));
    assert!(pos("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").can_checkmate(Color::White));

    // Uma peça menor sozinha não dá mate.
    assert!(!pos("4k3/8/8/8/8/8/8/3NK3 w - - 0 1").can_checkmate(Color::White));
    assert!(!pos("4k3/8/8/8/8/8/8/3BK3 w - - 0 1").can_checkmate(Color::White));
    // Com qualquer peça adversária para bloquear, existe sequência legal (mate ajudado).
    let nn = pos("4kn2/8/8/8/8/8/8/3NK3 w - - 0 1");
    assert!(nn.can_checkmate(Color::White));
    assert!(nn.can_checkmate(Color::Black));
    assert!(pos("4kb2/8/8/8/8/8/8/3NK3 w - - 0 1").can_checkmate(Color::White));
    assert!(pos("4k3/4p3/8/8/8/8/8/3NK3 w - - 0 1").can_checkmate(Color::White));

    // Dois cavalos: só com ajuda do adversário, mas é sequência legal (art. 6.9).
    assert!(pos("4k3/8/8/8/8/8/8/2NNK3 w - - 0 1").can_checkmate(Color::White));
    assert!(pos("4k3/8/8/8/8/8/8/2BNK3 w - - 0 1").can_checkmate(Color::White));
    assert!(pos("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1").can_checkmate(Color::White));

    // Bispos de mesma cor dos dois lados: posição morta, ninguém dá mate.
    let dead = pos("5b1k/8/8/8/8/8/8/2B1K3 w - - 0 1");
    assert!(!dead.can_checkmate(Color::White));
    assert!(!dead.can_checkmate(Color::Black));
}

#[test]
fn fifty_move_counter_thresholds() {
    assert!(!pos("4k3/8/8/8/8/8/8/R3K3 w - - 99 80").can_claim_fifty_move());
    assert!(pos("4k3/8/8/8/8/8/8/R3K3 w - - 100 80").can_claim_fifty_move());
    assert!(pos("4k3/8/8/8/8/8/8/R3K3 w - - 120 80").can_claim_fifty_move());
}

#[test]
fn seventy_five_move_counter_thresholds() {
    assert!(!pos("4k3/8/8/8/8/8/8/R3K3 w - - 149 90").is_seventy_five_move_draw());
    assert!(pos("4k3/8/8/8/8/8/8/R3K3 w - - 150 90").is_seventy_five_move_draw());
}

#[test]
fn seventy_five_move_does_not_apply_if_last_move_was_mate() {
    // Art. 9.6.2: o mate no 75º lance prevalece.
    let p = pos("R5k1/5ppp/8/8/8/8/8/6K1 b - - 150 90");
    assert!(!p.is_seventy_five_move_draw());
    assert!(p.can_claim_fifty_move());
}

#[test]
fn counter_constants_match_the_rules() {
    assert_eq!(chess_core::FIFTY_MOVE_HALFMOVES, 100);
    assert_eq!(chess_core::SEVENTY_FIVE_MOVE_HALFMOVES, 150);
}

/// Muro total: peões brancos em a4,c4,e4,g4,b5,d5,f5,h5 contra pretos em a5,c5,e5,g5,b6,d6,f6,h6.
/// Cada peão está travado de frente por um peão adversário e nenhum tem peão adversário na
/// diagonal. Os reis só alcançam casas que não tocam peão adversário.
const WALL: &str = "4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/4K3 w - - 0 1";

fn dead(fen: &str) -> bool {
    pos(fen).is_dead_position()
}

#[test]
fn dead_position_by_pawn_blockade_is_detected() {
    assert!(dead(WALL), "muro total, reis separados");
    assert!(dead(&WALL.replace(" w ", " b ")), "mesma posição, vez das pretas");
    // Reis em cantos e do mesmo lado do muro: ainda sem captura nem xeque possíveis.
    assert!(dead("8/7k/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/K7 w - - 0 1"));
    assert!(dead("8/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/7k/8/K7 w - - 0 1"));
    // Material insuficiente continua sendo posição morta.
    assert!(dead("4k3/8/8/8/8/8/8/4K3 w - - 0 1"));
    assert!(dead("5b1k/8/8/8/8/8/8/2B1K3 w - - 0 1"));
}

#[test]
fn dead_position_makes_checkmate_impossible_for_both_sides() {
    let p = pos(WALL);
    assert!(!p.can_checkmate(Color::White));
    assert!(!p.can_checkmate(Color::Black));
}

#[test]
fn blockade_with_possible_capture_is_not_dead() {
    // a4 captura b5 (peão preto na diagonal), mesmo com a4 travado por a5.
    assert!(!dead("4k3/8/8/pp6/P7/8/8/4K3 w - - 0 1"));
    // Ruptura: remover o peão preto de a5 libera o avanço de a4.
    assert!(!dead("4k3/8/1p1p1p1p/1PpPpPpP/P1P1P1P1/8/8/4K3 w - - 0 1"));
    // Peão livre (coluna aberta): avança, promove.
    assert!(!dead("4k3/8/8/8/8/8/P7/4K3 w - - 0 1"));
}

#[test]
fn king_that_reaches_an_enemy_pawn_is_not_dead() {
    // Peões travados em a4/a5, mas os reis chegam ao flanco e capturam.
    assert!(!dead("4k3/8/8/p7/P7/8/8/4K3 w - - 0 1"));
}

#[test]
fn pieces_other_than_kings_and_pawns_are_never_declared_dead_by_blockade() {
    // Conservador: com bispo, cavalo, torre ou dama o detector não afirma nada.
    assert!(!dead("4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/3BK3 w - - 0 1"));
    assert!(!dead("4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/3RK3 w - - 0 1"));
    assert!(!dead("4k3/8/1p1p1p1p/pPpPpPpP/P1P1P1P1/8/8/3NK3 w - - 0 1"));
}

#[test]
fn king_in_check_by_pawn_or_en_passant_is_not_dead() {
    // Rei preto em e8 atacado por peão branco em d7... (xeque de peão)
    assert!(!dead("4k3/3P4/8/8/8/8/8/4K3 b - - 0 1"));
    // Captura en passant disponível: conservador.
    assert!(!dead("4k3/8/8/pP6/8/8/8/4K3 w - a6 0 1"));
}
