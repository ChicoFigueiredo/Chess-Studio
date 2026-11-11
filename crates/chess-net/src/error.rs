//! Erros tipados da camada de rede. Nenhuma variante carrega segredo, cabeçalho ou corpo de
//! resposta: o texto pode ir para log e diagnóstico como está.

use crate::catalog::{Capability, ProviderId};
use crate::clock::Timestamp;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetError {
    /// O Provedor está desligado para o Perfil local: nada foi enviado.
    ProviderDisabled { provider: ProviderId },
    /// O Provedor não tem serviço de rede que se possa ligar (só importação manual, requer
    /// aprovação ou não confirmado).
    NoNetworkService { provider: ProviderId },
    /// O Provedor não libera esta capacidade (requer aprovação, não confirmada…).
    NotOffered {
        provider: ProviderId,
        capability: Capability,
    },
    /// Endereço fora do catálogo do Provedor (host, caminho ou esquema).
    EndpointNotAllowed { provider: ProviderId, url: String },
    /// A Trava de fair play está ativa e o pedido calcula (ADR-0007).
    FairPlayLocked {
        provider: ProviderId,
        capability: Capability,
    },
    /// A capacidade precisa de uma Conexão autorizada e o Cofre não deu o cabeçalho.
    NotAuthorized {
        provider: ProviderId,
        capability: Capability,
    },
    /// O Provedor pediu pausa (`429`); nada sai para ele antes de `until`.
    RateLimited {
        provider: ProviderId,
        until: Timestamp,
    },
    /// O Provedor não respondeu no prazo.
    Timeout { provider: ProviderId },
    /// O pedido foi cancelado por quem pediu.
    Cancelled,
    /// Sem conexão com o Provedor (rede fora, DNS, conexão recusada ou interrompida).
    Unreachable {
        provider: ProviderId,
        detail: String,
    },
    /// Resposta HTTP final fora de 2xx/304.
    Http { provider: ProviderId, status: u16 },
    /// Redirecionamento para outro host (ou sem HTTPS): não seguido.
    RedirectRefused {
        provider: ProviderId,
        location: String,
    },
    /// Resposta acima do limite de bytes do pedido.
    BodyTooLarge { provider: ProviderId, limit: usize },
    /// Pedido malformado (FEN inválido, cabeçalho proibido…), recusado antes da rede.
    InvalidRequest { reason: String },
    /// Falha do banco local (consentimento ou cache).
    Storage { detail: String },
}

impl NetError {
    /// Status HTTP, quando o erro é uma resposta do Provedor (404 em avaliação em cache = sem
    /// avaliação).
    pub fn http_status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            Self::RateLimited { .. } => Some(429),
            _ => None,
        }
    }

    pub fn provider(&self) -> Option<ProviderId> {
        match self {
            Self::ProviderDisabled { provider }
            | Self::NoNetworkService { provider }
            | Self::NotOffered { provider, .. }
            | Self::EndpointNotAllowed { provider, .. }
            | Self::FairPlayLocked { provider, .. }
            | Self::NotAuthorized { provider, .. }
            | Self::RateLimited { provider, .. }
            | Self::Timeout { provider }
            | Self::Unreachable { provider, .. }
            | Self::Http { provider, .. }
            | Self::RedirectRefused { provider, .. }
            | Self::BodyTooLarge { provider, .. } => Some(*provider),
            Self::Cancelled | Self::InvalidRequest { .. } | Self::Storage { .. } => None,
        }
    }

    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::InvalidRequest {
            reason: reason.into(),
        }
    }

    pub(crate) fn storage(err: impl fmt::Display) -> Self {
        Self::Storage {
            detail: err.to_string(),
        }
    }
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderDisabled { provider } => write!(
                f,
                "O Provedor {provider} está desligado. Ligue-o em Arquivo > Conexões para usar \
                 este recurso; nada foi enviado."
            ),
            Self::NoNetworkService { provider } => write!(
                f,
                "{provider} não tem serviço de rede para ligar: use a importação manual."
            ),
            Self::NotOffered {
                provider,
                capability,
            } => write!(
                f,
                "{provider} não libera «{}» para este app.",
                capability.label()
            ),
            Self::EndpointNotAllowed { provider, url } => write!(
                f,
                "O endereço {url} não está no catálogo do Provedor {provider}; nada foi enviado."
            ),
            Self::FairPlayLocked {
                provider,
                capability,
            } => write!(
                f,
                "Trava de fair play: «{}» ({provider}) fica bloqueado enquanto a Partida online \
                 estiver em andamento.",
                capability.label()
            ),
            Self::NotAuthorized {
                provider,
                capability,
            } => write!(
                f,
                "«{}» no {provider} precisa de uma Conexão autorizada.",
                capability.label()
            ),
            Self::RateLimited { provider, .. } => write!(
                f,
                "{provider} pediu uma pausa nos pedidos; o app espera o prazo antes de tentar de novo."
            ),
            Self::Timeout { provider } => write!(f, "{provider} não respondeu a tempo."),
            Self::Cancelled => f.write_str("Pedido cancelado."),
            Self::Unreachable { provider, detail } => {
                write!(f, "Sem conexão com {provider}: {detail}.")
            }
            Self::Http { provider, status } => write!(f, "{provider} respondeu HTTP {status}."),
            Self::RedirectRefused { provider, location } => write!(
                f,
                "{provider} redirecionou para {location}, fora do catálogo; não foi seguido."
            ),
            Self::BodyTooLarge { provider, limit } => write!(
                f,
                "A resposta de {provider} passou do limite de {limit} bytes."
            ),
            Self::InvalidRequest { reason } => write!(f, "Pedido inválido: {reason}."),
            Self::Storage { detail } => write!(f, "Falha no banco local da rede: {detail}."),
        }
    }
}

impl std::error::Error for NetError {}
