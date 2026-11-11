//! Perfil público do Lichess (`/api/user/{nome}`), para validar um nome vinculado, e rating
//! FIDE reproduzido pelo Lichess (`/api/fide/player/{id}` e busca por nome). Nenhuma das
//! duas consultas calcula, então a Trava de fair play não as bloqueia.

use crate::answer::{Query, fetch, json, valid_name};
use crate::error::{Missing, ProviderError};
use crate::lichess::Lichess;
use crate::ttl;
use chess_net::Request;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Rating de um ritmo no perfil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfRating {
    /// `bullet`, `blitz`, `rapid`, `classical`, `correspondence`, `puzzle`…
    pub perf: String,
    pub games: u32,
    pub rating: u32,
    pub provisional: bool,
}

/// Perfil público de um usuário do Lichess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LichessProfile {
    /// Identificador em minúsculas (o que vai nos endereços).
    pub id: String,
    /// Nome com a capitalização do usuário.
    pub username: String,
    pub title: Option<String>,
    pub created_at_ms: Option<i64>,
    pub seen_at_ms: Option<i64>,
    pub games: u64,
    pub ratings: Vec<PerfRating>,
    /// Conta fechada: o nome existe, mas não tem mais perfil.
    pub closed: bool,
    /// Conta marcada por violação dos termos (rating sem valor competitivo).
    pub tos_violation: bool,
}

#[derive(Deserialize)]
struct WireProfile {
    id: String,
    username: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    perfs: BTreeMap<String, WirePerf>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(default, rename = "seenAt")]
    seen_at: Option<i64>,
    #[serde(default)]
    disabled: bool,
    #[serde(default, rename = "tosViolation")]
    tos_violation: bool,
    #[serde(default)]
    count: Option<WireCount>,
}

#[derive(Deserialize)]
struct WirePerf {
    #[serde(default)]
    games: u32,
    #[serde(default)]
    rating: Option<u32>,
    #[serde(default)]
    prov: bool,
}

#[derive(Deserialize)]
struct WireCount {
    #[serde(default)]
    all: u64,
}

/// Rating FIDE de um jogador, **reproduzido pelo Lichess** (os dados e os direitos são da
/// FIDE). A resposta não traz o período do rating: a data que vale é a da consulta
/// (`provenance.queried_at`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FideRating {
    pub id: u64,
    pub name: String,
    /// Federação (código de três letras).
    pub federation: Option<String>,
    pub birth_year: Option<u16>,
    pub title: Option<String>,
    pub standard: Option<u16>,
    pub rapid: Option<u16>,
    pub blitz: Option<u16>,
    pub inactive: bool,
}

#[derive(Deserialize)]
struct WireFide {
    id: u64,
    name: String,
    #[serde(default)]
    federation: Option<String>,
    #[serde(default)]
    year: Option<u16>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    standard: Option<u16>,
    #[serde(default)]
    rapid: Option<u16>,
    #[serde(default)]
    blitz: Option<u16>,
    #[serde(default)]
    inactive: bool,
}

impl From<WireFide> for FideRating {
    fn from(w: WireFide) -> Self {
        Self {
            id: w.id,
            name: w.name,
            federation: w.federation,
            birth_year: w.year,
            title: w.title,
            standard: w.standard,
            rapid: w.rapid,
            blitz: w.blitz,
            inactive: w.inactive,
        }
    }
}

impl Lichess {
    /// Perfil público de `name`. Nome inexistente: [`ProviderError::UnknownName`]. Validade do
    /// cache: [`ttl::LICHESS_PROFILE`].
    pub fn profile(&self, name: &str) -> Query<LichessProfile> {
        let name = match valid_name(name) {
            Ok(name) => name,
            Err(e) => return Query::failed(e),
        };
        let provider = self.profile.id;
        let request = Request::get(provider, self.profile.url(&format!("/api/user/{name}")))
            .cache_for(ttl::LICHESS_PROFILE);
        fetch(&self.net, request, Missing::name(name), move |fetched| {
            let wire: WireProfile = json(provider, fetched)?;
            Ok(LichessProfile {
                id: wire.id,
                username: wire.username,
                title: wire.title,
                created_at_ms: wire.created_at,
                seen_at_ms: wire.seen_at,
                games: wire.count.map_or(0, |c| c.all),
                ratings: wire
                    .perfs
                    .into_iter()
                    .filter_map(|(perf, p)| {
                        Some(PerfRating {
                            perf,
                            games: p.games,
                            rating: p.rating?,
                            provisional: p.prov,
                        })
                    })
                    .collect(),
                closed: wire.disabled,
                tos_violation: wire.tos_violation,
            })
        })
    }

    /// Rating FIDE do jogador de ID `fide_id` (standard, rápido e blitz separados). ID
    /// inexistente: [`ProviderError::NoData`]. Validade do cache: [`ttl::FIDE`].
    pub fn fide_player(&self, fide_id: u64) -> Query<FideRating> {
        if !self.profile.has_fide {
            return Query::failed(self.not_offered("o rating FIDE"));
        }
        let provider = self.profile.id;
        let request = Request::get(
            provider,
            self.profile.url(&format!("/api/fide/player/{fide_id}")),
        )
        .cache_for(ttl::FIDE);
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            Ok(json::<WireFide>(provider, fetched)?.into())
        })
    }

    /// Busca jogadores da FIDE por nome (até o limite do Provedor). Sem resultados: lista
    /// vazia.
    pub fn fide_search(&self, text: &str) -> Query<Vec<FideRating>> {
        if !self.profile.has_fide {
            return Query::failed(self.not_offered("o rating FIDE"));
        }
        let provider = self.profile.id;
        let text = text.trim();
        if text.len() < 2 || text.len() > 60 {
            return Query::failed(ProviderError::invalid(
                "a busca por nome vai de 2 a 60 caracteres",
            ));
        }
        let request = Request::get(provider, self.profile.url("/api/fide/player"))
            .query("q", text)
            .cache_for(ttl::FIDE);
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            let list: Vec<WireFide> = json(provider, fetched)?;
            Ok(list.into_iter().map(FideRating::from).collect())
        })
    }
}
