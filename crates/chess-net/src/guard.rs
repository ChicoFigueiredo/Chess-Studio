//! Trava de fair play na camada de serviço (ADR-0007, CX-08): com a trava ativa, todo
//! pedido que calcula (Explorador, avaliação em cache, finais, Engine remota) é recusado
//! pelo gate antes de tocar a rede. Não há chave de depuração que a contorne: a
//! classificação vem do catálogo ([`Capability::computes`]), não de quem pede.
//!
//! Quem liga e desliga a trava é a Partida online (etapa 4): ligar quando a Partida contra
//! uma pessoa começa, desligar quando o Provedor confirma o fim. A trava é compartilhada:
//! clones apontam para o mesmo estado.

use crate::catalog::{Capability, ProviderId};
use crate::error::NetError;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct FairPlayGuard {
    reason: Arc<RwLock<Option<String>>>,
}

impl FairPlayGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Liga a trava; `reason` é diagnóstico ("Partida online contra pessoa no Lichess").
    pub fn engage(&self, reason: impl Into<String>) {
        if let Ok(mut slot) = self.reason.write() {
            *slot = Some(reason.into());
        }
    }

    /// Desliga a trava (o Provedor confirmou o fim da Partida).
    pub fn release(&self) {
        if let Ok(mut slot) = self.reason.write() {
            *slot = None;
        }
    }

    /// Ligada? Um lock envenenado conta como ligada: na dúvida, bloqueia.
    pub fn is_engaged(&self) -> bool {
        self.reason.read().map_or(true, |r| r.is_some())
    }

    pub fn reason(&self) -> Option<String> {
        self.reason.read().ok().and_then(|r| r.clone())
    }

    /// Recusa o que calcula enquanto a trava está ligada.
    pub fn check(&self, provider: ProviderId, capability: Capability) -> Result<(), NetError> {
        if capability.computes() && self.is_engaged() {
            return Err(NetError::FairPlayLocked {
                provider,
                capability,
            });
        }
        Ok(())
    }
}
