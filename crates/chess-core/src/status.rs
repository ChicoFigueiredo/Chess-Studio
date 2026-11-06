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
    /// Não detecta posição morta por bloqueio de peões nem fortalezas.
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
    /// Aproximação por material, conservadora (`true` na dúvida): peão, torre ou
    /// dama bastam; duas ou mais peças menores bastam, exceto só bispos da mesma
    /// cor; uma peça menor (ou bispos de uma só cor) só basta se o adversário tiver
    /// alguma peça além do rei que sirva de bloqueio e a posição não for morta por
    /// material. Não analisa bloqueios de peões nem fortalezas.
    pub fn can_checkmate(&self, color: Color) -> bool {
        let me = self.material(color);
        if me.has_heavy_or_pawn() {
            return true;
        }
        if me.minors() == 0 || self.is_insufficient_material() {
            return false;
        }
        if me.minors() >= 2 && !me.only_same_color_bishops() {
            return true;
        }
        self.material(color.opposite()).has_anything()
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
