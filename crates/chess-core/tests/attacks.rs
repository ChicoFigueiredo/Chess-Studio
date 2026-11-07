//! Ataques, ameaças e dica de Lance: funções puras para o HUD (C5).

use chess_core::{CanonicalPosition, Color, Square};

fn pos(fen: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(fen).expect("FEN válido")
}

fn sq(name: &str) -> Square {
    name.parse().expect("casa válida")
}

fn names(squares: Vec<Square>) -> Vec<String> {
    let mut v: Vec<String> = squares.into_iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

#[test]
fn start_position_white_attacks_ranks_one_to_three_only() {
    let attacked = CanonicalPosition::start().attacked_squares(Color::White);
    for file in 'a'..='h' {
        assert!(attacked.contains(&sq(&format!("{file}3"))), "{file}3");
        assert!(attacked.contains(&sq(&format!("{file}2"))), "{file}2");
    }
    assert!(!attacked.iter().any(|s| s.rank() >= 3));
}

#[test]
fn sliders_stop_at_the_first_blocker_which_is_itself_attacked() {
    // Torre em a1, rei próprio em e1: a torre defende e1 mas não passa dele.
    let p = pos("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    let attacked = p.attacked_squares(Color::White);
    for name in ["b1", "c1", "d1", "a2", "a8"] {
        assert!(attacked.contains(&sq(name)), "{name}");
    }
    assert!(attacked.contains(&sq("f1")), "f1 pelo rei");
    assert!(!attacked.contains(&sq("g1")), "g1 está além do bloqueio");
}

#[test]
fn pawns_attack_diagonally_forward_only() {
    let p = pos("4k3/8/8/8/3P4/8/8/4K3 w - - 0 1");
    let attacked = p.attacked_squares(Color::White);
    assert!(attacked.contains(&sq("c5")) && attacked.contains(&sq("e5")));
    assert!(!attacked.contains(&sq("d5")), "peão não ataca à frente");
    assert!(!attacked.contains(&sq("c3")), "nem para trás");
    let black = pos("4k3/8/8/3p4/8/8/8/4K3 w - - 0 1");
    let a = black.attacked_squares(Color::Black);
    assert!(a.contains(&sq("c4")) && a.contains(&sq("e4")));
}

#[test]
fn attackers_of_lists_every_attacking_piece() {
    let p = pos("4k3/8/8/8/8/2n5/8/R3K3 w - - 0 1");
    assert_eq!(names(p.attackers_of(sq("e2"), Color::Black)), ["c3"]);
    assert_eq!(names(p.attackers_of(sq("a5"), Color::White)), ["a1"]);
    assert!(p.attackers_of(sq("h8"), Color::White).is_empty());
}

#[test]
fn threatened_pieces_are_attacked_by_the_opponent() {
    // O cavalo preto em c3 ataca a2, e2, d1, b1, a4, e4, b5 e d5.
    let p = pos("4k3/8/8/8/8/2n5/R3P3/4K3 w - - 0 1");
    assert_eq!(names(p.threatened_pieces(Color::White)), ["a2", "e2"]);
    assert!(p.threatened_pieces(Color::Black).is_empty());
}

#[test]
fn the_king_in_check_is_a_threatened_piece() {
    let p = pos("4k3/8/8/8/8/8/4q3/4K3 w - - 0 1");
    assert!(p.threatened_pieces(Color::White).contains(&sq("e1")));
}

#[test]
fn undefended_pieces_have_no_defender_and_exclude_the_king() {
    let p = pos("4k3/8/8/8/8/2n5/R3P3/4K3 w - - 0 1");
    // O rei defende e2; ninguém defende a torre em a2. O rei não conta.
    assert_eq!(names(p.undefended_pieces(Color::White)), ["a2"]);
    assert_eq!(names(p.undefended_pieces(Color::Black)), ["c3"]);
}

#[test]
fn hint_prefers_mate_in_one() {
    let p = pos("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1");
    let hint = p.suggest_hint().expect("há dica");
    assert_eq!((hint.from(), hint.to()), (sq("a1"), sq("a8")));
}

#[test]
fn hint_takes_a_free_piece() {
    let p = pos("4k3/8/8/3q4/8/8/8/3RK3 w - - 0 1");
    let hint = p.suggest_hint().expect("há dica");
    assert_eq!((hint.from(), hint.to()), (sq("d1"), sq("d5")));
}

#[test]
fn hint_does_not_offer_a_losing_capture() {
    // A dama só poderia capturar um peão defendido: perda líquida, sem dica.
    let p = pos("4k3/8/2p5/3p4/8/8/3Q4/4K3 w - - 0 1");
    assert!(p.suggest_hint().is_none());
}

#[test]
fn hint_is_none_in_the_start_position() {
    assert!(CanonicalPosition::start().suggest_hint().is_none());
}
