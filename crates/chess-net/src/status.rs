//! Estado da rede para a barra de status e diagnóstico sanitizado das últimas operações.

use crate::catalog::{Capability, ProviderId};
use crate::clock::Timestamp;
use crate::error::NetError;
use crate::request::Method;
use crate::response::Freshness;

/// Situação de um Provedor agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderState {
    /// Desligado para o Perfil local (o padrão).
    Disabled,
    /// Ligado e sem problema conhecido.
    Available,
    /// O Provedor pediu pausa: nada sai para ele antes de `until`.
    Limited { until: Timestamp },
    /// A última tentativa falhou (rede, tempo esgotado ou erro do servidor).
    Failing { since: Timestamp, error: NetError },
}

/// Fotografia do estado da rede (barata: pode ser lida a cada quadro).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Todos os Provedores do catálogo, na ordem do catálogo.
    pub providers: Vec<(ProviderId, ProviderState)>,
    /// Pedidos e fluxos em andamento ou na fila.
    pub active_requests: usize,
}

impl NetworkStatus {
    pub fn state(&self, provider: ProviderId) -> &ProviderState {
        self.providers
            .iter()
            .find(|(id, _)| *id == provider)
            .map_or(&ProviderState::Disabled, |(_, s)| s)
    }

    pub fn enabled_count(&self) -> usize {
        self.count(|s| *s != ProviderState::Disabled)
    }

    pub fn limited_count(&self) -> usize {
        self.count(|s| matches!(s, ProviderState::Limited { .. }))
    }

    pub fn failing_count(&self) -> usize {
        self.count(|s| matches!(s, ProviderState::Failing { .. }))
    }

    /// Nenhum Provedor ligado: o app está offline (o padrão, ADR-0006).
    pub fn is_offline(&self) -> bool {
        self.enabled_count() == 0
    }

    fn count(&self, pred: impl Fn(&ProviderState) -> bool) -> usize {
        self.providers.iter().filter(|(_, s)| pred(s)).count()
    }
}

/// Registro sanitizado de uma operação (`ProviderOperationLog` da especificação 4.4): sem
/// cabeçalhos, consulta, corpo ou segredo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub provider: ProviderId,
    pub capability: Option<Capability>,
    pub method: Method,
    /// Só o caminho, sem a consulta.
    pub path: String,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub http_status: Option<u16>,
    pub outcome: Result<Freshness, NetError>,
}
