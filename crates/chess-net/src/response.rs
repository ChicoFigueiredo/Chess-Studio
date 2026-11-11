//! O resultado: corpo, Origem e frescor. Todo dado externo sai daqui com a sua Origem.

use crate::catalog::ProviderId;
use crate::clock::Timestamp;
use crate::error::NetError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Como o dado entrou no app (o "modo de importação" da Origem).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Consulta à API do Provedor.
    Api,
    /// Pacote baixado do Provedor (base, problemas).
    Download,
    /// Arquivo que o usuário baixou e importou.
    ManualImport,
}

/// Origem (_Provenance_) de um dado externo: Provedor, instante da consulta, licença e modo
/// de importação, mais o endereço oficial (sem segredo). Serializável para acompanhar a
/// Partida importada, a estatística, a avaliação e o rating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub provider: ProviderId,
    /// Última vez que o Provedor confirmou o conteúdo (download ou `304`).
    pub queried_at: Timestamp,
    pub license: String,
    pub mode: ImportMode,
    /// URL oficial consultada, ou o nome do arquivo na importação manual.
    pub source: String,
}

impl Provenance {
    /// Origem de um arquivo importado à mão (TWIC, PGN Mentor, Queen Alice…).
    pub fn manual_import(
        provider: ProviderId,
        imported_at: Timestamp,
        license: impl Into<String>,
        file_name: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            queried_at: imported_at,
            license: license.into(),
            mode: ImportMode::ManualImport,
            source: file_name.into(),
        }
    }
}

/// De onde veio a resposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// Da rede, agora.
    Network,
    /// Do cache, confirmado agora pelo Provedor (`304`).
    Revalidated,
    /// Do cache, dentro da validade; a rede não foi tocada.
    Cache,
    /// Do cache vencido, porque a rede falhou (`cause`): mostrar com a idade.
    Stale { cause: NetError },
}

/// Resposta entregue a quem pediu.
#[derive(Clone, PartialEq, Eq)]
pub struct Fetched {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub provenance: Provenance,
    pub freshness: Freshness,
    /// Idade do conteúdo: agora menos `provenance.queried_at`.
    pub age: Duration,
    /// Escopos que o Provedor diz que o token da Conexão tem (`X-OAuth-Scopes` do Lichess);
    /// só em respostas da rede a pedidos autenticados.
    pub oauth_scopes: Option<String>,
}

impl Fetched {
    /// O corpo como texto, se for UTF-8.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

/// O `Debug` mostra o tamanho do corpo, não o conteúdo (pode ser PGN privado).
impl fmt::Debug for Fetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fetched")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("body_len", &self.body.len())
            .field("provenance", &self.provenance)
            .field("freshness", &self.freshness)
            .field("age", &self.age)
            .field("oauth_scopes", &self.oauth_scopes)
            .finish()
    }
}
