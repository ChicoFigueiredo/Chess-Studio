//! Avaliação em cache do Lichess (`/api/cloud-eval`): só o que já está na nuvem do Lichess;
//! o serviço nunca calcula sob demanda. `404` é "sem avaliação": resultado válido
//! (`Ok(None)`), lembrado por pouco tempo ([`ttl::NO_EVAL`]) para a interface consultar por
//! quadro sem repetir o pedido.
//!
//! # Ponto de vista
//!
//! O placar vem **sempre do ponto de vista das Brancas** (medido em 2026-11-11: Pretas dando
//! mate em 1 respondem `mate: -1`). [`EvalLine::score_for`] converte para o lado que a
//! interface mostrar. Os Lances vêm em UCI; o roque pode vir como o Rei capturando a Torre
//! (`e1h1`), e quem aplica a linha valida cada Lance pelo núcleo de regras (ADR-0003).

use crate::answer::{Answer, Query, json, license_of};
use crate::error::{Missing, ProviderError};
use crate::lichess::{API_HOST, Lichess, NoEval, PROVIDER};
use crate::ttl;
use chess_core::{CanonicalPosition, Color};
use chess_net::{Capability, Freshness, ImportMode, Provenance, Request};
use serde::Deserialize;

/// Placar de uma linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    /// Centésimos de peão.
    Centipawns(i32),
    /// Mate em N Lances: positivo quando quem tem o ponto de vista dá o mate, negativo quando
    /// sofre.
    Mate(i32),
}

impl Score {
    /// Converte um placar do ponto de vista das Brancas para o de `side`.
    pub fn for_side(self, side: Color) -> Self {
        match side {
            Color::White => self,
            Color::Black => match self {
                Self::Centipawns(cp) => Self::Centipawns(-cp),
                Self::Mate(n) => Self::Mate(-n),
            },
        }
    }
}

/// Uma linha principal (variante) da avaliação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalLine {
    /// Lances em UCI, do primeiro em diante.
    pub moves: Vec<String>,
    /// Do ponto de vista das Brancas ([`POV`]).
    pub score_white: Score,
}

impl EvalLine {
    pub fn score_for(&self, side: Color) -> Score {
        self.score_white.for_side(side)
    }
}

/// O ponto de vista em que o Provedor entrega os placares.
pub const POV: Color = Color::White;

/// Avaliação em cache de uma Posição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudEval {
    pub fen: String,
    pub depth: u32,
    pub nodes: u64,
    /// Da melhor para a pior (a ordem do Provedor); no máximo `multi_pv` linhas.
    pub lines: Vec<EvalLine>,
}

#[derive(Deserialize)]
struct WireEval {
    fen: String,
    #[serde(default)]
    knodes: u64,
    #[serde(default)]
    depth: u32,
    #[serde(default)]
    pvs: Vec<WireLine>,
}

#[derive(Deserialize)]
struct WireLine {
    #[serde(default)]
    moves: String,
    cp: Option<i32>,
    mate: Option<i32>,
}

fn decode(fetched: &chess_net::Fetched) -> Result<Option<CloudEval>, ProviderError> {
    let wire: WireEval = json(PROVIDER, fetched)?;
    let lines = wire
        .pvs
        .into_iter()
        .map(|line| {
            let score_white = match (line.mate, line.cp) {
                (Some(n), _) => Score::Mate(n),
                (None, Some(cp)) => Score::Centipawns(cp),
                (None, None) => {
                    return Err(ProviderError::malformed(PROVIDER, "linha sem cp nem mate"));
                }
            };
            Ok(EvalLine {
                moves: line.moves.split_whitespace().map(str::to_owned).collect(),
                score_white,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(CloudEval {
        fen: wire.fen,
        depth: wire.depth,
        nodes: wire.knodes.saturating_mul(1000),
        lines,
    }))
}

impl Lichess {
    /// Avaliação em cache de `fen` com até `multi_pv` linhas (1 a 5). `Ok(None)` é "sem
    /// avaliação". Validade do cache: [`ttl::CLOUD_EVAL`]; "sem avaliação": [`ttl::NO_EVAL`].
    pub fn cloud_eval(&self, fen: &str, multi_pv: u8) -> Query<Option<CloudEval>> {
        if !(1..=5).contains(&multi_pv) {
            return Query::failed(ProviderError::invalid("MultiPV vai de 1 a 5"));
        }
        let position = match CanonicalPosition::from_fen(fen.trim()) {
            Ok(position) => position,
            Err(e) => return Query::failed(ProviderError::invalid(format!("FEN inválido ({e})"))),
        };
        let key = format!("{} {multi_pv}", chess_core::position_key(&position));
        let request = Request::get(PROVIDER, format!("https://{API_HOST}/api/cloud-eval"))
            .position("fen", fen)
            .query("multiPv", multi_pv.to_string())
            .cache_for(ttl::CLOUD_EVAL);

        if let Some(early) = self.remembered_no_eval(&key) {
            return early;
        }

        let clock = self.clock.clone();
        let memory = self.no_eval.clone();
        let source = format!("https://{API_HOST}/api/cloud-eval");
        let absent = Box::new(move || {
            let now = clock.now();
            let provenance = Provenance {
                provider: PROVIDER,
                queried_at: now,
                license: license_of(PROVIDER, API_HOST, "/api/cloud-eval"),
                mode: ImportMode::Api,
                source,
            };
            if let Ok(mut slot) = memory.lock() {
                slot.insert(
                    key,
                    NoEval {
                        until: now.plus(ttl::NO_EVAL),
                        provenance: provenance.clone(),
                    },
                );
            }
            Answer {
                value: None,
                provenance,
                freshness: Freshness::Network,
                age: std::time::Duration::ZERO,
            }
        });
        Query::waiting_or_absent(
            self.net.fetch(request),
            Missing::NoData,
            Box::new(decode),
            Some(absent),
        )
    }

    /// "Sem avaliação" ainda válido na memória: responde sem pedido, mas depois do mesmo gate
    /// (Provedor ligado e Trava de fair play livre).
    fn remembered_no_eval(&self, key: &str) -> Option<Query<Option<CloudEval>>> {
        let remembered = self.no_eval.lock().ok()?.get(key).cloned()?;
        let now = self.clock.now();
        if now >= remembered.until {
            return None;
        }
        if !self.net.is_enabled(PROVIDER) {
            return Some(Query::failed(ProviderError::Disabled {
                provider: PROVIDER,
            }));
        }
        if let Err(e) = self
            .net
            .fair_play_guard()
            .check(PROVIDER, Capability::CachedEval)
        {
            return Some(Query::failed(Missing::NoData.translate(e)));
        }
        let age = now.since(remembered.provenance.queried_at);
        Some(Query::ready(Ok(Answer {
            value: None,
            provenance: remembered.provenance,
            freshness: Freshness::Cache,
            age,
        })))
    }
}
