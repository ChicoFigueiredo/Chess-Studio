//! Erros tipados dos adaptadores, em português. Nenhuma variante carrega segredo, cabeçalho
//! ou corpo de resposta: o texto pode ir para log e diagnóstico como está.

use chess_net::{NetError, ProviderId, Timestamp};
use std::fmt;

/// Por que um adaptador não entregou o resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProviderError {
    /// O Provedor está desligado para o Perfil local; nada foi enviado.
    Disabled { provider: ProviderId },
    /// O Provedor pediu pausa (`429`); nada sai para ele antes de `until`.
    RateLimited {
        provider: ProviderId,
        until: Timestamp,
    },
    /// A Trava de fair play está ativa (Partida online em andamento) e a consulta calcula.
    FairPlayLocked { provider: ProviderId },
    /// A consulta requer uma Conexão autorizada do Provedor e não há (nenhuma escolhida,
    /// Cofre bloqueado, segredo apagado): recusada antes da rede. Também um `401` sem token.
    /// Medido em 2026-11-11: o Explorador do Lichess passou a responder `401` sem token.
    SignInRequired { provider: ProviderId },
    /// O Provedor recusou o token da Conexão: `401` (expirado ou revogado) ou `403` (falta
    /// permissão). A Conexão ficou marcada no Cofre; nada local se perdeu.
    ConnectionRejected { provider: ProviderId, status: u16 },
    /// O Provedor respondeu, mas não tem dados para esta consulta (Posição fora do Explorador,
    /// sem avaliação em cache, mês sem Partidas, FIDE inexistente).
    NoData,
    /// O nome de usuário não existe no Provedor.
    UnknownName { provider: ProviderId, name: String },
    /// A consulta pede uma variante que o app recusa (só xadrez padrão).
    VariantRefused {
        provider: ProviderId,
        variant: String,
    },
    /// Finais exatos só existem até `max` peças; a Posição tem `pieces`. Nada foi enviado.
    TooManyPieces { pieces: usize, max: usize },
    /// Argumento inválido (FEN, nome, intervalo), recusado antes da rede.
    InvalidInput { reason: String },
    /// A resposta do Provedor não tem o formato documentado.
    Malformed {
        provider: ProviderId,
        detail: String,
    },
    /// O pedido foi cancelado por quem pediu.
    Cancelled,
    /// Qualquer outra falha da camada de rede (sem conexão, prazo, HTTP inesperado…).
    Net(NetError),
}

impl ProviderError {
    /// Falhas que passam sozinhas: vale mostrar "tentar de novo".
    pub fn is_transient(&self) -> bool {
        match self {
            Self::RateLimited { .. } => true,
            Self::Net(e) => matches!(
                e,
                NetError::Timeout { .. } | NetError::Unreachable { .. } | NetError::Http { .. }
            ),
            _ => false,
        }
    }

    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            reason: reason.into(),
        }
    }

    pub(crate) fn malformed(provider: ProviderId, detail: impl fmt::Display) -> Self {
        Self::Malformed {
            provider,
            detail: detail.to_string(),
        }
    }
}

/// O que um `404` (ou `410`) significa para a consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Missing {
    /// "Sem dados": resultado válido, não falha do Provedor.
    NoData,
    /// O nome pedido não existe.
    Name(String),
}

impl Missing {
    pub(crate) fn name(name: &str) -> Self {
        Self::Name(name.to_owned())
    }

    /// Traduz o erro da camada de rede para o da consulta.
    pub(crate) fn translate(&self, error: NetError) -> ProviderError {
        match error {
            NetError::ProviderDisabled { provider } => ProviderError::Disabled { provider },
            NetError::RateLimited { provider, until } => {
                ProviderError::RateLimited { provider, until }
            }
            NetError::FairPlayLocked { provider, .. } => ProviderError::FairPlayLocked { provider },
            NetError::Cancelled => ProviderError::Cancelled,
            NetError::InvalidRequest { reason } => ProviderError::InvalidInput { reason },
            NetError::Http {
                provider,
                status: 404 | 410,
            } => match self {
                Self::NoData => ProviderError::NoData,
                Self::Name(name) => ProviderError::UnknownName {
                    provider,
                    name: name.clone(),
                },
            },
            NetError::Http {
                provider,
                status: 401,
            }
            | NetError::NotAuthorized { provider, .. } => {
                ProviderError::SignInRequired { provider }
            }
            NetError::ConnectionRejected { provider, status } => {
                ProviderError::ConnectionRejected { provider, status }
            }
            other => ProviderError::Net(other),
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled { provider } => write!(
                f,
                "O Provedor {provider} está desligado. Ligue-o em Arquivo > Conexões para usar \
                 este recurso; nada foi enviado."
            ),
            Self::RateLimited { provider, .. } => write!(
                f,
                "{provider} pediu uma pausa nos pedidos; o app tenta de novo quando o prazo passar."
            ),
            Self::FairPlayLocked { provider } => write!(
                f,
                "Trava de fair play: consultas de apoio ({provider}) ficam bloqueadas enquanto a \
                 Partida online estiver em andamento."
            ),
            Self::SignInRequired { provider } => write!(
                f,
                "Esta consulta requer Conexão do {provider} autorizada: autorize em Arquivo > \
                 Conexões (ou desbloqueie o cofre)."
            ),
            Self::ConnectionRejected { provider, status } => {
                if *status == 403 {
                    write!(
                        f,
                        "{provider} recusou a Conexão para esta consulta: falta permissão ao token."
                    )
                } else {
                    write!(
                        f,
                        "A Conexão do {provider} expirou ou foi revogada: autorize de novo em \
                         Arquivo > Conexões."
                    )
                }
            }
            Self::NoData => f.write_str("Sem dados para esta consulta."),
            Self::UnknownName { provider, name } => {
                write!(f, "O nome «{name}» não existe no {provider}.")
            }
            Self::VariantRefused { provider, variant } => write!(
                f,
                "A variante «{variant}» ({provider}) não é aceita: o app só trabalha com xadrez padrão."
            ),
            Self::TooManyPieces { pieces, max } => write!(
                f,
                "Finais exatos só existem até {max} peças; esta Posição tem {pieces}."
            ),
            Self::InvalidInput { reason } => write!(f, "Consulta inválida: {reason}."),
            Self::Malformed { provider, detail } => {
                write!(
                    f,
                    "A resposta de {provider} não tem o formato esperado ({detail})."
                )
            }
            Self::Cancelled => f.write_str("Consulta cancelada."),
            Self::Net(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<NetError> for ProviderError {
    /// Tradução sem contexto: `404` vira [`ProviderError::NoData`].
    fn from(error: NetError) -> Self {
        Missing::NoData.translate(error)
    }
}
