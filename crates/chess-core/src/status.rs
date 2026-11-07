//! Fim de partida na Posição: mate, afogamento, material e contadores de lances.
//!
//! Referência: Leis do Xadrez da FIDE (2023), arts. 5.2 (mate, afogamento, posição
//! morta), 6.9, 7.5.5, 9.3 (50 lances) e 9.6 (75 lances).

use crate::position::CanonicalPosition;
use crate::types::{Color, Piece, Square};

/// Meio-lances sem captura nem lance de peão que dão direito à reclamação (art. 9.3).
pub const FIFTY_MOVE_HALFMOVES: u16 = 100;
/// Meio-lances que encerram a partida automaticamente (art. 9.6.2).
pub const SEVENTY_FIVE_MOVE_HALFMOVES: u16 = 150;

/// Situação da Partida na Posição, só pelas regras do tabuleiro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameStatus {
    /// O lado da vez tem Lances legais.
    Ongoing,
    /// O lado da vez está em xeque e sem Lances legais; `winner` é o outro lado.
    Checkmate { winner: Color },
    /// O lado da vez não está em xeque e não tem Lances legais: empate (art. 5.2.1), nunca mate.
    Stalemate,
}

/// Peças de um lado, fora o rei.
#[derive(Default)]
struct Material {
    pawns: u8,
    knights: u8,
    rooks: u8,
    queens: u8,
    /// Bispos em casas escuras / claras.
    dark_bishops: u8,
    light_bishops: u8,
}

impl Material {
    fn minors(&self) -> u8 {
        self.knights + self.dark_bishops + self.light_bishops
    }

    fn has_heavy_or_pawn(&self) -> bool {
        self.pawns + self.rooks + self.queens > 0
    }

    fn has_anything(&self) -> bool {
        self.has_heavy_or_pawn() || self.minors() > 0
    }

    /// Só bispos, todos em casas da mesma cor.
    fn only_same_color_bishops(&self) -> bool {
        self.knights == 0 && (self.dark_bishops == 0 || self.light_bishops == 0)
    }
}

impl CanonicalPosition {
    /// Mate, afogamento ou em andamento (CORE-06: afogamento nunca é mate).
    pub fn status(&self) -> GameStatus {
        if !self.legal_moves().is_empty() {
            GameStatus::Ongoing
        } else if self.is_check() {
            GameStatus::Checkmate {
                winner: self.side_to_move().opposite(),
            }
        } else {
            GameStatus::Stalemate
        }
    }

    fn material(&self, color: Color) -> Material {
        let mut m = Material::default();
        for rank in 0..8 {
            for file in 0..8 {
                let sq = Square::new(file, rank).expect("coordenadas em 0..8");
                if let Some((_, piece)) = self.piece_at(sq).filter(|(c, _)| *c == color) {
                    match piece {
                        Piece::Pawn => m.pawns += 1,
                        Piece::Knight => m.knights += 1,
                        Piece::Rook => m.rooks += 1,
                        Piece::Queen => m.queens += 1,
                        Piece::Bishop if (file + rank) % 2 == 0 => m.dark_bishops += 1,
                        Piece::Bishop => m.light_bishops += 1,
                        Piece::King => {}
                    }
                }
            }
        }
        m
    }

    /// Posição morta por material (art. 5.2.2): nenhuma sequência de Lances legais
    /// leva a mate para nenhum dos lados.
    ///
    /// Casos reconhecidos: K x K, K+B x K, K+N x K e K+B x K+B (ou mais bispos) com
    /// todos os bispos em casas da mesma cor. Dois cavalos contra rei sozinho **não**
    /// entram (o mate é possível com ajuda do adversário), nem K+N x K+N.
    /// Não cobre bloqueio de peões (ver [`Self::is_dead_position`]) nem fortalezas.
    pub fn is_insufficient_material(&self) -> bool {
        let (w, b) = (self.material(Color::White), self.material(Color::Black));
        if w.has_heavy_or_pawn() || b.has_heavy_or_pawn() {
            return false;
        }
        if w.minors() + b.minors() <= 1 {
            return true;
        }
        w.knights + b.knights == 0
            && (w.dark_bishops + b.dark_bishops == 0 || w.light_bishops + b.light_bishops == 0)
    }

    /// O lado `color` tem material para dar mate por *alguma* sequência de Lances
    /// legais (cooperação do adversário incluída), como pedem os arts. 6.9 e 7.5.5.
    ///
    /// Aproximação conservadora (`true` na dúvida). Primeiro, se a posição é morta
    /// ([`Self::is_dead_position`]: material insuficiente ou bloqueio total de
    /// peões), ninguém dá mate. Depois, por material: peão, torre ou dama bastam;
    /// duas ou mais peças menores bastam, exceto só bispos da mesma cor; uma peça
    /// menor (ou bispos de uma só cor) só basta se o adversário tiver alguma peça
    /// além do rei que sirva de bloqueio. Não analisa fortalezas.
    pub fn can_checkmate(&self, color: Color) -> bool {
        if self.is_dead_position() {
            return false;
        }
        let me = self.material(color);
        if me.has_heavy_or_pawn() {
            return true;
        }
        if me.minors() == 0 {
            return false;
        }
        if me.minors() >= 2 && !me.only_same_color_bishops() {
            return true;
        }
        self.material(color.opposite()).has_anything()
    }

    /// Posição morta (art. 5.2.2): nenhuma série de Lances legais leva a xeque-mate.
    ///
    /// **Detector conservador**: `true` só quando a posição é comprovadamente morta;
    /// na dúvida devolve `false` (falso negativo permitido, falso positivo nunca).
    /// Cobre (a) material insuficiente ([`Self::is_insufficient_material`]) e
    /// (b) bloqueio total de peões, pelo algoritmo abaixo.
    ///
    /// Bloqueio total, só com reis e peões e sem en passant:
    /// 1. todo peão está travado de frente por um peão adversário (nenhum avança,
    ///    nem duplo) e nenhum tem peão adversário numa casa de captura;
    /// 2. para cada rei, calcula-se por busca em largura o conjunto de casas que ele
    ///    alcança com os peões fixos, sem pisar em peão nem em casa atacada por peão
    ///    adversário (ignora-se o outro rei: superconjunto, portanto seguro);
    /// 3. nenhum rei pode já estar atacado por peão, e nenhuma casa alcançável pode
    ///    ser vizinha de peão adversário (senão o rei poderia capturá-lo).
    ///
    /// Se 1-3 valem, o conjunto de peões é invariante (ninguém captura nem avança),
    /// os reis ficam presos aos seus conjuntos e, sem poder pisar em casa atacada
    /// por peão, nunca há xeque; logo não há mate.
    ///
    /// Não cobre: bispos presos atrás de cadeias, cavalos, torres, damas, fortalezas
    /// nem posições em que a captura é possível mas inútil. Nesses casos devolve `false`.
    pub fn is_dead_position(&self) -> bool {
        self.is_insufficient_material() || self.is_dead_by_pawn_blockade()
    }

    fn is_dead_by_pawn_blockade(&self) -> bool {
        // En passant pendente: captura possível, não afirma nada.
        if self.to_fen().split(' ').nth(3) != Some("-") {
            return false;
        }
        let mut pawns = [[None::<Color>; 8]; 8];
        let mut kings = Vec::new();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(file, rank).expect("coordenadas em 0..8");
                match self.piece_at(sq) {
                    None => {}
                    Some((color, Piece::Pawn)) => {
                        if rank == 0 || rank == 7 {
                            return false;
                        }
                        pawns[usize::from(file)][usize::from(rank)] = Some(color);
                    }
                    Some((color, Piece::King)) => kings.push((color, file, rank)),
                    Some(_) => return false,
                }
            }
        }
        let at = |file: i8, rank: i8| -> Option<Color> {
            if (0..8).contains(&file) && (0..8).contains(&rank) {
                pawns[file as usize][rank as usize]
            } else {
                None
            }
        };
        let forward = |c: Color| -> i8 { if c == Color::White { 1 } else { -1 } };
        // Passo 1: travamento frontal e ausência de captura.
        for file in 0..8i8 {
            for rank in 0..8i8 {
                let Some(color) = at(file, rank) else {
                    continue;
                };
                let f = forward(color);
                if at(file, rank + f) != Some(color.opposite()) {
                    return false;
                }
                if at(file - 1, rank + f) == Some(color.opposite())
                    || at(file + 1, rank + f) == Some(color.opposite())
                {
                    return false;
                }
            }
        }
        // `color` ataca (file, rank) com algum peão?
        let attacked_by_pawn = |color: Color, file: i8, rank: i8| -> bool {
            let f = forward(color);
            at(file - 1, rank - f) == Some(color) || at(file + 1, rank - f) == Some(color)
        };
        // Passos 2 e 3.
        for &(color, kf, kr) in &kings {
            let enemy = color.opposite();
            let (kf, kr) = (kf as i8, kr as i8);
            if attacked_by_pawn(enemy, kf, kr) {
                return false;
            }
            let mut seen = [[false; 8]; 8];
            seen[kf as usize][kr as usize] = true;
            let mut stack = vec![(kf, kr)];
            while let Some((f, r)) = stack.pop() {
                for df in -1..=1i8 {
                    for dr in -1..=1i8 {
                        let (nf, nr) = (f + df, r + dr);
                        if (df, dr) == (0, 0) || !(0..8).contains(&nf) || !(0..8).contains(&nr) {
                            continue;
                        }
                        match at(nf, nr) {
                            Some(c) if c == enemy => return false,
                            Some(_) => continue,
                            None => {}
                        }
                        if attacked_by_pawn(enemy, nf, nr) || seen[nf as usize][nr as usize] {
                            continue;
                        }
                        seen[nf as usize][nr as usize] = true;
                        stack.push((nf, nr));
                    }
                }
            }
        }
        true
    }

    /// Cinquenta lances: o relógio de meio-lance chegou a 100, o que permite
    /// reclamar o empate (art. 9.3). A reclamação em si é da arbitragem.
    pub fn can_claim_fifty_move(&self) -> bool {
        self.halfmove_clock() >= FIFTY_MOVE_HALFMOVES
    }

    /// Setenta e cinco lances: relógio em 150 ou mais encerra a Partida como empate,
    /// salvo se a Posição é xeque-mate (art. 9.6.2).
    pub fn is_seventy_five_move_draw(&self) -> bool {
        self.halfmove_clock() >= SEVENTY_FIVE_MOVE_HALFMOVES
            && !matches!(self.status(), GameStatus::Checkmate { .. })
    }
}
