//! Adaptadores serde para os tipos de `chess-core`, que não dependem de serde.
//! Formato estável do payload: casas e Lances em UCI (`e2`, `e7e8q`) e cores por nome.

use chess_core::{Color, Move, Square};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

pub mod uci {
    use super::*;

    pub fn serialize<S: Serializer>(mv: &Move, s: S) -> Result<S::Ok, S::Error> {
        mv.to_string().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Move, D::Error> {
        String::deserialize(d)?.parse().map_err(D::Error::custom)
    }
}

pub mod square {
    use super::*;

    pub fn serialize<S: Serializer>(sq: &Square, s: S) -> Result<S::Ok, S::Error> {
        sq.to_string().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Square, D::Error> {
        String::deserialize(d)?.parse().map_err(D::Error::custom)
    }
}

pub mod color {
    use super::*;

    pub fn serialize<S: Serializer>(c: &Color, s: S) -> Result<S::Ok, S::Error> {
        match c {
            Color::White => "white",
            Color::Black => "black",
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        match String::deserialize(d)?.as_str() {
            "white" => Ok(Color::White),
            "black" => Ok(Color::Black),
            other => Err(D::Error::custom(format!("cor desconhecida: {other:?}"))),
        }
    }
}
