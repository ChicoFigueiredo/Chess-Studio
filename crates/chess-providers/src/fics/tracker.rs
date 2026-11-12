//! Do `style 12` ao retrato (#237): o FICS manda o tabuleiro depois de cada Lance, não a
//! lista de Lances. O rastreador guarda a lista desde o começo e só a estende quando o
//! núcleo de regras prova o caminho: do tabuleiro conhecido ao novo há **exatamente uma**
//! sequência de Lances legais que chega à mesma colocação, vez, roque e en passant.
//!
//! - Tabuleiro atrasado (meio-lance menor que o conhecido): ignorado, nada regride.
//! - Repetido (mesmo meio-lance e mesmo tabuleiro): só o relógio pode mudar.
//! - Um `style 12` perdido (linha cortada, por exemplo): o seguinte fica a dois meio-lances;
//!   a busca de profundidade 2 o recupera se o caminho for único.
//! - Qualquer outro caso (salto maior, caminho ambíguo, mesmo meio-lance com outro
//!   tabuleiro): inalcançável. A lista não muda; a sessão marca a dessincronização.

use super::parse::Style12;
use chess_core::{CanonicalPosition, Move};

/// Maior salto de meio-lances que a busca tenta fechar.
const MAX_GAP: u64 = 2;
/// Quantas buscas de dois meio-lances sem caminho único são aceitas por meio-lance
/// conhecido; depois, o tabuleiro vira inalcançável sem buscar de novo até a lista andar.
/// Quem está no caminho do FICS (texto puro) não gasta CPU a cada linha injetada (SEC-07);
/// uma linha perdida de verdade se recupera nas primeiras tentativas.
const MAX_FAILED_SEARCHES_PER_PLY: u8 = 8;

/// O que um `style 12` fez com a lista de Lances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// Atrasado: ignorado.
    Stale,
    /// Mesmo meio-lance e mesmo tabuleiro (relógio novo, talvez).
    Same,
    /// A lista ganhou Lances.
    Advanced,
    /// Não há caminho único até este tabuleiro: nada mudou.
    Unreachable { ply: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct Tracker {
    /// Meio-lance (desde a Posição inicial padrão) da Posição de partida.
    start_ply: u64,
    position: CanonicalPosition,
    moves: Vec<Move>,
    /// Buscas de profundidade 2 que falharam no meio-lance conhecido atual.
    failed_searches: u8,
}

/// Colocação, vez, roque e en passant (o FEN do núcleo sempre põe o en passant depois do
/// avanço duplo, como o `style 12`).
fn key(position: &CanonicalPosition) -> String {
    position
        .to_fen()
        .split(' ')
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

impl Tracker {
    pub fn new(position: CanonicalPosition, start_ply: u64) -> Self {
        Self {
            start_ply,
            position,
            moves: Vec::new(),
            failed_searches: 0,
        }
    }

    pub fn known_ply(&self) -> u64 {
        self.start_ply + self.moves.len() as u64
    }

    pub fn position(&self) -> &CanonicalPosition {
        &self.position
    }

    /// Os Lances desde a Posição de partida, em UCI (o roque como o rei andando duas casas).
    pub fn uci(&self) -> Vec<String> {
        self.moves.iter().map(Move::to_string).collect()
    }

    pub fn apply(&mut self, board: &Style12) -> Step {
        let ply = board.ply();
        let known = self.known_ply();
        if ply < known {
            return Step::Stale;
        }
        let target = board.key();
        if ply == known {
            return if key(&self.position) == target {
                Step::Same
            } else {
                Step::Unreachable { ply }
            };
        }
        let gap = ply - known;
        if gap > MAX_GAP || (gap == MAX_GAP && self.failed_searches >= MAX_FAILED_SEARCHES_PER_PLY)
        {
            return Step::Unreachable { ply };
        }
        match paths(&self.position, gap, &target).as_slice() {
            [(path, position)] => {
                self.moves.extend(path);
                self.position = position.clone();
                self.failed_searches = 0;
                Step::Advanced
            }
            _ => {
                if gap == MAX_GAP {
                    self.failed_searches += 1;
                }
                Step::Unreachable { ply }
            }
        }
    }
}

/// Os caminhos de `depth` Lances legais de `from` até `target` (para em dois: basta saber se
/// é único).
fn paths(
    from: &CanonicalPosition,
    depth: u64,
    target: &str,
) -> Vec<(Vec<Move>, CanonicalPosition)> {
    let mut found = Vec::new();
    walk(from, depth, target, &mut Vec::new(), &mut found);
    found
}

fn walk(
    position: &CanonicalPosition,
    depth: u64,
    target: &str,
    path: &mut Vec<Move>,
    found: &mut Vec<(Vec<Move>, CanonicalPosition)>,
) {
    for mv in position.legal_moves() {
        if found.len() > 1 {
            return;
        }
        let Ok(next) = position.play(mv) else {
            continue;
        };
        path.push(mv);
        if depth == 1 {
            if key(&next) == target {
                found.push((path.clone(), next));
            }
        } else {
            walk(&next, depth - 1, target, path, found);
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fics::parse::parse_style12;

    fn board(position: &CanonicalPosition) -> Style12 {
        let fen = position.to_fen();
        let fields: Vec<&str> = fen.split(' ').collect();
        let ranks: Vec<String> = fields[0]
            .split('/')
            .map(|rank| {
                rank.chars()
                    .map(|c| match c.to_digit(10) {
                        Some(n) => "-".repeat(n as usize),
                        None => c.to_string(),
                    })
                    .collect()
            })
            .collect();
        let side = if fields[1] == "w" { "W" } else { "B" };
        let double_push = match fields[3] {
            "-" => "-1".to_owned(),
            ep => (ep.as_bytes()[0] - b'a').to_string(),
        };
        let flag = |c: char| if fields[2].contains(c) { "1" } else { "0" };
        let relation = if fields[1] == "w" { 1 } else { -1 };
        let line = format!(
            "<12> {} {side} {double_push} {} {} {} {} {} 1 Ana Bia {relation} 5 0 39 39 300 300 {} none (0:00) none 0 1 0",
            ranks.join(" "),
            flag('K'),
            flag('Q'),
            flag('k'),
            flag('q'),
            fields[4],
            fields[5],
        );
        parse_style12(&line).expect("style 12 sintético")
    }

    fn play(position: &CanonicalPosition, uci: &str) -> CanonicalPosition {
        position.play(uci.parse().expect("lance")).expect("legal")
    }

    #[test]
    fn hostile_two_ply_boards_stop_being_searched_until_the_list_moves() {
        let start = CanonicalPosition::start();
        let mut tracker = Tracker::new(start.clone(), 0);
        // Meio-lance 2, com o cavalo de g1 sumido: nenhuma sequência legal chega lá.
        let hostile_two_plies = parse_style12(
            "<12> rnbqkbnr pppppppp -------- -------- -------- -------- PPPPPPPP RNBQKB-R W -1 1 1 1 1 0 1 Ana Bia 1 5 0 39 39 300 300 2 none (0:00) none 0 1 0",
        )
        .expect("linha hostil");
        assert_eq!(hostile_two_plies.ply(), 2);
        for _ in 0..MAX_FAILED_SEARCHES_PER_PLY {
            assert!(matches!(
                tracker.apply(&hostile_two_plies),
                Step::Unreachable { .. }
            ));
        }
        assert_eq!(tracker.failed_searches, MAX_FAILED_SEARCHES_PER_PLY);

        // Passado o teto, nem o tabuleiro certo a dois meio-lances é buscado...
        let real = play(&play(&start, "e2e4"), "e7e5");
        assert!(matches!(
            tracker.apply(&board(&real)),
            Step::Unreachable { .. }
        ));
        // ...mas um Lance de cada vez continua andando e zera a conta.
        let one = play(&start, "e2e4");
        assert_eq!(tracker.apply(&board(&one)), Step::Advanced);
        assert_eq!(tracker.failed_searches, 0);
    }
}
