//! Uma Partida pública do Lichess, pelo id (#239): a "Partida de referência" do Explorador.
//!
//! [`Lichess::game_pgn`] exporta a Partida em PGN (`/game/export/{id}`); [`Lichess::masters_game_pgn`]
//! faz o mesmo para o corpus de mestres (`explorer.lichess.org/masters/pgn/{id}`, que exige a
//! Conexão). O PGN entra na Biblioteca com a Origem desta resposta
//! ([`crate::import_options`] sobre `answer.provenance`); o id remoto vem das tags, então
//! importar de novo a mesma Partida não a duplica.

use crate::answer::{Query, fetch};
use crate::error::{Missing, ProviderError};
use crate::lichess::{API_HOST, Lichess, PROVIDER};
use chess_net::Request;
use std::time::Duration;

/// Validade do cache: uma Partida terminada não muda.
const GAME_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// Uma Partida exportada: o id e o PGN completo (tags, Lances e resultado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedGame {
    pub id: String,
    pub pgn: String,
}

/// O id de Partida do Lichess tem 8 letras e números; só isso entra no endereço.
fn valid_game_id(id: &str) -> Result<&str, ProviderError> {
    let id = id.trim();
    if id.len() == 8 && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        Ok(id)
    } else {
        Err(ProviderError::invalid(
            "id de Partida do Lichess: 8 letras e números",
        ))
    }
}

fn decode(id: &str, body: &[u8]) -> Result<ExportedGame, ProviderError> {
    let pgn = String::from_utf8_lossy(body).into_owned();
    if pgn.contains("[Event ") {
        Ok(ExportedGame {
            id: id.to_owned(),
            pgn,
        })
    } else {
        Err(ProviderError::malformed(
            PROVIDER,
            "a resposta não é um PGN",
        ))
    }
}

impl Lichess {
    /// A Partida `id` do Lichess em PGN, sem relógios, avaliações nem abertura (só o que a
    /// Biblioteca guarda). Id desconhecido: [`ProviderError::NoData`].
    pub fn game_pgn(&self, id: &str) -> Query<ExportedGame> {
        let id = match valid_game_id(id) {
            Ok(id) => id.to_owned(),
            Err(e) => return Query::failed(e),
        };
        let request = Request::get(PROVIDER, format!("https://{API_HOST}/game/export/{id}"))
            .accept("application/x-chess-pgn")
            .query("clocks", "false")
            .query("evals", "false")
            .query("opening", "false")
            .cache_for(GAME_TTL);
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            decode(&id, &fetched.body)
        })
    }

    /// A Partida `id` do corpus de mestres em PGN. Exige uma Conexão autorizada, como o
    /// resto do Explorador. Id desconhecido: [`ProviderError::NoData`].
    pub fn masters_game_pgn(&self, id: &str) -> Query<ExportedGame> {
        let id = match valid_game_id(id) {
            Ok(id) => id.to_owned(),
            Err(e) => return Query::failed(e),
        };
        let request = self
            .explorer_request(&format!("/masters/pgn/{id}"))
            .accept("application/x-chess-pgn")
            .cache_for(GAME_TTL);
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            decode(&id, &fetched.body)
        })
    }
}
