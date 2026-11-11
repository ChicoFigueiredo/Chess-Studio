//! Finais exatos do Lichess (`tablebase.lichess.ovh/standard`, bases Syzygy): categoria,
//! DTZ/DTM e Lances ordenados do melhor para o pior, para Posições de até 7 peças. Posições
//! maiores nunca geram pedido.
//!
//! O endereço é o `.ovh`: o catálogo do `chess-net` ganhou esse host nesta etapa (a
//! especificação citava `.org`, que o serviço não usa; medido em 2026-11-11).

use crate::answer::{Query, fetch, json};
use crate::error::{Missing, ProviderError};
use crate::lichess::{Lichess, PROVIDER, TABLEBASE_HOST};
use crate::ttl;
use chess_core::CanonicalPosition;
use chess_net::Request;
use serde::Deserialize;

/// Máximo de peças (Reis incluídos) que as bases cobrem.
pub const MAX_PIECES: usize = 7;

/// Resultado teórico, do ponto de vista de quem joga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Win,
    /// Vitória que a regra dos 50 Lances torna empate (cursed win).
    CursedWin,
    /// Provável vitória (DTZ fora das bases usadas).
    MaybeWin,
    Draw,
    /// Derrota que a regra dos 50 Lances torna empate (blessed loss).
    BlessedLoss,
    MaybeLoss,
    Loss,
    Unknown,
}

impl Category {
    fn parse(text: &str) -> Self {
        match text {
            "win" => Self::Win,
            "cursed-win" => Self::CursedWin,
            "maybe-win" => Self::MaybeWin,
            "draw" => Self::Draw,
            "blessed-loss" => Self::BlessedLoss,
            "maybe-loss" => Self::MaybeLoss,
            "loss" => Self::Loss,
            _ => Self::Unknown,
        }
    }

    /// Resultado visto pelo adversário (depois do Lance).
    pub fn flipped(self) -> Self {
        match self {
            Self::Win => Self::Loss,
            Self::CursedWin => Self::BlessedLoss,
            Self::MaybeWin => Self::MaybeLoss,
            Self::BlessedLoss => Self::CursedWin,
            Self::MaybeLoss => Self::MaybeWin,
            Self::Loss => Self::Win,
            other => other,
        }
    }
}

/// Um Lance legal da Posição com o resultado que ele leva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablebaseMove {
    pub uci: String,
    pub san: String,
    /// Resultado para **o adversário** depois do Lance (como o Provedor entrega); use
    /// [`TablebaseMove::category_for_mover`] para quem joga.
    pub category_after: Category,
    /// Lance que zera o contador de 50 Lances (captura ou Peão).
    pub zeroing: bool,
    pub dtz: Option<i32>,
    pub dtm: Option<i32>,
}

impl TablebaseMove {
    pub fn category_for_mover(&self) -> Category {
        self.category_after.flipped()
    }
}

/// Resposta dos finais exatos para uma Posição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tablebase {
    pub category: Category,
    /// Distância ao zeramento (50 Lances) em meios-Lances; o sinal segue o resultado.
    pub dtz: Option<i32>,
    /// Distância ao mate, quando as bases a têm.
    pub dtm: Option<i32>,
    pub checkmate: bool,
    pub stalemate: bool,
    pub insufficient_material: bool,
    /// Do melhor para o pior (a ordem do Provedor).
    pub moves: Vec<TablebaseMove>,
}

#[derive(Deserialize)]
struct Wire {
    #[serde(default)]
    checkmate: bool,
    #[serde(default)]
    stalemate: bool,
    #[serde(default)]
    insufficient_material: bool,
    #[serde(default)]
    dtz: Option<i32>,
    #[serde(default)]
    dtm: Option<i32>,
    category: String,
    #[serde(default)]
    moves: Vec<WireMove>,
}

#[derive(Deserialize)]
struct WireMove {
    uci: String,
    san: String,
    #[serde(default)]
    zeroing: bool,
    #[serde(default)]
    dtz: Option<i32>,
    #[serde(default)]
    dtm: Option<i32>,
    category: String,
}

/// Peças na Posição (Reis incluídos), pelo campo de peças do FEN.
fn piece_count(fen: &str) -> usize {
    fen.split_whitespace().next().map_or(0, |placement| {
        placement.chars().filter(char::is_ascii_alphabetic).count()
    })
}

impl Lichess {
    /// Finais exatos de `fen`. O FEN vai **completo** (o relógio de 50 Lances muda a
    /// resposta). Mais de [`MAX_PIECES`] peças: [`ProviderError::TooManyPieces`] sem pedido.
    /// Validade do cache: [`ttl::TABLEBASE`].
    pub fn tablebase(&self, fen: &str) -> Query<Tablebase> {
        if self.profile.tablebase_host.is_none() {
            return Query::failed(self.not_offered("os finais exatos"));
        }
        let fen = fen.trim();
        if let Err(e) = CanonicalPosition::from_fen(fen) {
            return Query::failed(ProviderError::invalid(format!("FEN inválido ({e})")));
        }
        let pieces = piece_count(fen);
        if pieces > MAX_PIECES {
            return Query::failed(ProviderError::TooManyPieces {
                pieces,
                max: MAX_PIECES,
            });
        }
        let request = Request::get(PROVIDER, format!("https://{TABLEBASE_HOST}/standard"))
            .query("fen", fen)
            .cache_for(ttl::TABLEBASE);
        fetch(&self.net, request, Missing::NoData, |fetched| {
            let wire: Wire = json(PROVIDER, fetched)?;
            Ok(Tablebase {
                category: Category::parse(&wire.category),
                dtz: wire.dtz,
                dtm: wire.dtm,
                checkmate: wire.checkmate,
                stalemate: wire.stalemate,
                insufficient_material: wire.insufficient_material,
                moves: wire
                    .moves
                    .into_iter()
                    .map(|m| TablebaseMove {
                        uci: m.uci,
                        san: m.san,
                        category_after: Category::parse(&m.category),
                        zeroing: m.zeroing,
                        dtz: m.dtz,
                        dtm: m.dtm,
                    })
                    .collect(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_pieces_from_the_placement_field() {
        assert_eq!(piece_count("4k3/6KP/8/8/8/8/7p/8 w - - 0 1"), 4);
        assert_eq!(
            piece_count("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            32
        );
    }
}
