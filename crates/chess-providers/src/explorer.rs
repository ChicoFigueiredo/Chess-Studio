//! Opening Explorer do Lichess (`explorer.lichess.org`): `masters`, `lichess` e `player`.
//!
//! O resultado ([`ExplorerPosition`]) é o mesmo nos três: contagem de vitórias das Brancas,
//! empates e vitórias das Pretas da Posição e de cada Lance, rating médio, Partidas de
//! referência e a abertura (ECO e nome). `masters` e `lichess` são uma consulta comum
//! ([`Query`]); `player` responde em fluxo NDJSON, que a interface lê por quadro com
//! [`ExplorerStream::try_next`]: cada linha é um resultado parcial melhor que o anterior.
//!
//! Medido em 2026-11-11: o serviço responde `401` sem autenticação nos três endereços. Com
//! uma Conexão do Lichess escolhida ([`Lichess::use_connection`]), os pedidos levam o token
//! dela (do Cofre); sem ela, a consulta é recusada antes da rede com
//! [`crate::ProviderError::SignInRequired`] ("requer Conexão do Lichess").

use crate::answer::{Answer, Query, fetch, json, license_of, stream_provenance};
use crate::dates::YearMonth;
use crate::error::{Missing, ProviderError};
use crate::lichess::{EXPLORER_HOST, Lichess, PROVIDER};
use crate::ttl;
use chess_core::Color;
use chess_net::{Freshness, Provenance, Stream, StreamEvent};
use chess_store::Pace;
use serde::Deserialize;
use std::fmt;
use std::time::Duration;

/// Vitórias das Brancas, empates e vitórias das Pretas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcomes {
    pub white: u64,
    pub draws: u64,
    pub black: u64,
}

impl Outcomes {
    pub fn total(&self) -> u64 {
        self.white + self.draws + self.black
    }

    /// Pontos de `side` (vitória 1, empate 0,5) sobre o total, de 0 a 1; `None` sem Partidas.
    pub fn score_for(&self, side: Color) -> Option<f64> {
        let total = self.total();
        if total == 0 {
            return None;
        }
        let wins = match side {
            Color::White => self.white,
            Color::Black => self.black,
        };
        Some((wins as f64 + self.draws as f64 / 2.0) / total as f64)
    }
}

/// Um Lance do Explorador com a sua estatística.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerMove {
    /// Lance em UCI (`e2e4`). O roque pode vir como o Rei capturando a Torre (`e1h1`).
    pub uci: String,
    pub san: String,
    pub outcomes: Outcomes,
    /// Rating médio dos jogadores (`player`: dos adversários).
    pub average_rating: Option<u32>,
    /// Desempenho (rating) do jogador nesse Lance; só no endpoint `player`.
    pub performance: Option<u32>,
}

/// Jogador de uma Partida de referência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencePlayer {
    pub name: String,
    pub rating: Option<u32>,
}

/// Partida de referência (as melhores ou as mais recentes da Posição).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceGame {
    /// Identificador no Lichess (`https://lichess.org/<id>`).
    pub id: String,
    /// Lance dessa Partida na Posição consultada, em UCI.
    pub uci: Option<String>,
    /// `None` = empate.
    pub winner: Option<Color>,
    pub white: ReferencePlayer,
    pub black: ReferencePlayer,
    pub speed: Option<String>,
    pub year: Option<u32>,
    /// `AAAA-MM`.
    pub month: Option<String>,
    /// `true` se vem de «recentes»; `false` se de «melhores».
    pub recent: bool,
}

/// Abertura reconhecida (ECO e nome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningName {
    pub eco: String,
    pub name: String,
}

/// Resultado do Explorador para uma Posição.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExplorerPosition {
    pub outcomes: Outcomes,
    /// Lances em ordem decrescente de Partidas (a ordem do Provedor).
    pub moves: Vec<ExplorerMove>,
    pub games: Vec<ReferenceGame>,
    pub opening: Option<OpeningName>,
    /// Posição na fila de indexação do jogador (`player`), quando o Provedor ainda está
    /// indexando.
    pub queue_position: Option<u32>,
}

/// Filtros do corpus de mestres. Anos de calendário.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MastersFilter {
    pub since_year: Option<u16>,
    pub until_year: Option<u16>,
    /// Máximo de Lances na resposta (o Provedor limita a 50).
    pub moves: Option<u8>,
    /// Máximo de Partidas de referência (até 15).
    pub top_games: Option<u8>,
}

/// Filtros do Explorador do Lichess (todas as Partidas da plataforma).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LichessFilter {
    /// Vazio = todos os ritmos.
    pub paces: Vec<Pace>,
    /// Faixas de rating pelo piso: 0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500. Vazio
    /// = todas.
    pub ratings: Vec<u16>,
    pub since: Option<YearMonth>,
    pub until: Option<YearMonth>,
    pub moves: Option<u8>,
    pub top_games: Option<u8>,
    pub recent_games: Option<u8>,
}

/// Filtros do Explorador de um jogador (as Partidas dele com a cor escolhida).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerFilter {
    pub paces: Vec<Pace>,
    pub since: Option<YearMonth>,
    pub until: Option<YearMonth>,
    pub moves: Option<u8>,
    pub recent_games: Option<u8>,
}

/// Pisos de rating que o Explorador aceita.
const RATING_BANDS: [u16; 9] = [0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500];

fn pace_list(paces: &[Pace]) -> Option<String> {
    if paces.is_empty() {
        return None;
    }
    let names: Vec<&str> = paces.iter().map(|p| p.as_str()).collect();
    Some(names.join(","))
}

// ---- Formato do Provedor ------------------------------------------------------------

#[derive(Deserialize)]
struct WirePosition {
    white: u64,
    draws: u64,
    black: u64,
    #[serde(default)]
    moves: Vec<WireMove>,
    #[serde(default, rename = "topGames")]
    top_games: Vec<WireGame>,
    #[serde(default, rename = "recentGames")]
    recent_games: Vec<WireGame>,
    opening: Option<WireOpening>,
    #[serde(default, rename = "queuePosition")]
    queue_position: Option<u32>,
}

#[derive(Deserialize)]
struct WireMove {
    uci: String,
    san: String,
    white: u64,
    draws: u64,
    black: u64,
    #[serde(default, rename = "averageRating")]
    average_rating: Option<u32>,
    #[serde(default, rename = "averageOpponentRating")]
    average_opponent_rating: Option<u32>,
    #[serde(default)]
    performance: Option<u32>,
}

#[derive(Deserialize)]
struct WireSide {
    #[serde(default)]
    name: String,
    #[serde(default)]
    rating: Option<u32>,
}

#[derive(Deserialize)]
struct WireGame {
    id: String,
    #[serde(default)]
    uci: Option<String>,
    #[serde(default)]
    winner: Option<String>,
    #[serde(default)]
    speed: Option<String>,
    white: WireSide,
    black: WireSide,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    month: Option<String>,
}

#[derive(Deserialize)]
struct WireOpening {
    eco: String,
    name: String,
}

impl WireGame {
    fn into_game(self, recent: bool) -> ReferenceGame {
        ReferenceGame {
            id: self.id,
            uci: self.uci,
            winner: match self.winner.as_deref() {
                Some("white") => Some(Color::White),
                Some("black") => Some(Color::Black),
                _ => None,
            },
            white: ReferencePlayer {
                name: self.white.name,
                rating: self.white.rating,
            },
            black: ReferencePlayer {
                name: self.black.name,
                rating: self.black.rating,
            },
            speed: self.speed,
            year: self.year,
            month: self.month,
            recent,
        }
    }
}

impl From<WirePosition> for ExplorerPosition {
    fn from(wire: WirePosition) -> Self {
        let mut games: Vec<ReferenceGame> = wire
            .top_games
            .into_iter()
            .map(|g| g.into_game(false))
            .collect();
        for game in wire.recent_games {
            if !games.iter().any(|g| g.id == game.id) {
                games.push(game.into_game(true));
            }
        }
        Self {
            outcomes: Outcomes {
                white: wire.white,
                draws: wire.draws,
                black: wire.black,
            },
            moves: wire
                .moves
                .into_iter()
                .map(|m| ExplorerMove {
                    uci: m.uci,
                    san: m.san,
                    outcomes: Outcomes {
                        white: m.white,
                        draws: m.draws,
                        black: m.black,
                    },
                    average_rating: m.average_rating.or(m.average_opponent_rating),
                    performance: m.performance,
                })
                .collect(),
            games,
            opening: wire.opening.map(|o| OpeningName {
                eco: o.eco,
                name: o.name,
            }),
            queue_position: wire.queue_position,
        }
    }
}

// ---- Consultas ----------------------------------------------------------------------

impl Lichess {
    /// Corpus de mestres (partidas de torneios de elite) na Posição `fen`. Validade do cache:
    /// [`ttl::MASTERS`].
    pub fn masters(&self, fen: &str, filter: &MastersFilter) -> Query<ExplorerPosition> {
        if self.profile.explorer_host.is_none() {
            return Query::failed(self.not_offered("o Explorador de aberturas"));
        }
        let mut request = self
            .explorer_request("/masters")
            .position("fen", fen)
            .cache_for(ttl::MASTERS);
        if let Some(year) = filter.since_year {
            request = request.query("since", year.to_string());
        }
        if let Some(year) = filter.until_year {
            request = request.query("until", year.to_string());
        }
        if let Some(n) = filter.moves {
            request = request.query("moves", n.to_string());
        }
        if let Some(n) = filter.top_games {
            request = request.query("topGames", n.to_string());
        }
        fetch(&self.net, request, Missing::NoData, decode_position)
    }

    /// Todas as Partidas do Lichess na Posição `fen`, por ritmo e faixa de rating. Validade do
    /// cache: [`ttl::LICHESS_EXPLORER`].
    pub fn explorer(&self, fen: &str, filter: &LichessFilter) -> Query<ExplorerPosition> {
        if self.profile.explorer_host.is_none() {
            return Query::failed(self.not_offered("o Explorador de aberturas"));
        }
        if let Some(bad) = filter.ratings.iter().find(|r| !RATING_BANDS.contains(r)) {
            return Query::failed(ProviderError::invalid(format!(
                "faixa de rating {bad} não existe no Explorador (use {RATING_BANDS:?})"
            )));
        }
        let mut request = self
            .explorer_request("/lichess")
            .position("fen", fen)
            .query("variant", "standard")
            .cache_for(ttl::LICHESS_EXPLORER);
        if let Some(speeds) = pace_list(&filter.paces) {
            request = request.query("speeds", speeds);
        }
        if !filter.ratings.is_empty() {
            let bands: Vec<String> = filter.ratings.iter().map(u16::to_string).collect();
            request = request.query("ratings", bands.join(","));
        }
        if let Some(since) = filter.since {
            request = request.query("since", since.to_string());
        }
        if let Some(until) = filter.until {
            request = request.query("until", until.to_string());
        }
        if let Some(n) = filter.moves {
            request = request.query("moves", n.to_string());
        }
        if let Some(n) = filter.top_games {
            request = request.query("topGames", n.to_string());
        }
        if let Some(n) = filter.recent_games {
            request = request.query("recentGames", n.to_string());
        }
        fetch(&self.net, request, Missing::NoData, decode_position)
    }

    /// Partidas de `player` com a cor `color` na Posição `fen`, em fluxo: o Provedor indexa o
    /// jogador na hora e manda resultados parciais cada vez melhores. Sem cache (o fluxo é
    /// vivo). Feche com [`ExplorerStream::close`] ou descartando.
    pub fn player_explorer(
        &self,
        fen: &str,
        player: &str,
        color: Color,
        filter: &PlayerFilter,
    ) -> ExplorerStream {
        if self.profile.explorer_host.is_none() {
            return ExplorerStream::failed(
                self.not_offered("o Explorador de aberturas"),
                self.provenance(),
            );
        }
        let name = match crate::answer::valid_name(player) {
            Ok(name) => name,
            Err(e) => return ExplorerStream::failed(e, self.provenance()),
        };
        let mut request = self
            .explorer_request("/player")
            .position("fen", fen)
            .query("player", name)
            .query(
                "color",
                match color {
                    Color::White => "white",
                    Color::Black => "black",
                },
            )
            .query("variant", "standard");
        if let Some(speeds) = pace_list(&filter.paces) {
            request = request.query("speeds", speeds);
        }
        if let Some(since) = filter.since {
            request = request.query("since", since.to_string());
        }
        if let Some(until) = filter.until {
            request = request.query("until", until.to_string());
        }
        if let Some(n) = filter.moves {
            request = request.query("moves", n.to_string());
        }
        if let Some(n) = filter.recent_games {
            request = request.query("recentGames", n.to_string());
        }
        let provenance = self.provenance();
        ExplorerStream {
            source: Some(self.net.stream(request)),
            early: None,
            provenance,
            latest: None,
            finished: false,
        }
    }

    fn provenance(&self) -> Provenance {
        stream_provenance(PROVIDER, EXPLORER_HOST, "/player", self.clock.as_ref())
    }
}

fn decode_position(fetched: &chess_net::Fetched) -> Result<ExplorerPosition, ProviderError> {
    Ok(json::<WirePosition>(PROVIDER, fetched)?.into())
}

// ---- Fluxo do jogador ---------------------------------------------------------------

/// O que o fluxo do Explorador de jogador entrega.
#[derive(Debug, Clone, PartialEq)]
pub enum ExplorerEvent {
    /// Um resultado parcial, melhor que o anterior (o último é o definitivo).
    Partial(Answer<ExplorerPosition>),
    /// O Provedor terminou de indexar e encerrou o fluxo.
    Finished,
    /// O fluxo não abriu ou caiu; o último parcial continua válido em [`ExplorerStream::latest`].
    Failed(ProviderError),
}

/// Fluxo aberto por [`Lichess::player_explorer`]. Consulte por quadro com `try_next`.
pub struct ExplorerStream {
    source: Option<Stream>,
    early: Option<ProviderError>,
    provenance: Provenance,
    latest: Option<Answer<ExplorerPosition>>,
    finished: bool,
}

impl fmt::Debug for ExplorerStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExplorerStream")
            .field("finished", &self.finished)
            .field("has_result", &self.latest.is_some())
            .finish()
    }
}

impl ExplorerStream {
    fn failed(error: ProviderError, provenance: Provenance) -> Self {
        // O erro sai no primeiro `try_next`, como se viesse do fluxo.
        Self {
            source: None,
            early: Some(error),
            provenance,
            latest: None,
            finished: false,
        }
    }

    /// Próximo evento, se já chegou (nunca bloqueia).
    pub fn try_next(&mut self) -> Option<ExplorerEvent> {
        if let Some(error) = self.early.take() {
            self.finished = true;
            return Some(ExplorerEvent::Failed(error));
        }
        if self.finished {
            return None;
        }
        let event = self.source.as_ref()?.try_next()?;
        match event {
            StreamEvent::Line(line) => match serde_json::from_str::<WirePosition>(&line) {
                Ok(wire) => {
                    let answer = Answer {
                        value: ExplorerPosition::from(wire),
                        provenance: self.provenance.clone(),
                        freshness: Freshness::Network,
                        age: Duration::ZERO,
                    };
                    self.latest = Some(answer.clone());
                    Some(ExplorerEvent::Partial(answer))
                }
                Err(e) => {
                    self.close();
                    Some(ExplorerEvent::Failed(ProviderError::malformed(PROVIDER, e)))
                }
            },
            StreamEvent::Ended => {
                self.finished = true;
                Some(ExplorerEvent::Finished)
            }
            StreamEvent::Failed(error) => {
                self.finished = true;
                Some(ExplorerEvent::Failed(Missing::NoData.translate(error)))
            }
        }
    }

    /// O melhor resultado recebido até agora.
    pub fn latest(&self) -> Option<&Answer<ExplorerPosition>> {
        self.latest.as_ref()
    }

    /// O fluxo acabou (fim, falha ou fechado).
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Encerra o fluxo; a conexão cai na próxima leitura.
    pub fn close(&mut self) {
        self.finished = true;
        if let Some(stream) = &self.source {
            stream.close();
        }
    }
}

/// Licença do Explorador, para a interface mostrar mesmo antes da primeira resposta.
pub fn explorer_license() -> String {
    license_of(PROVIDER, EXPLORER_HOST, "/")
}
