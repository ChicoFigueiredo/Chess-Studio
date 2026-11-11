//! Trava de fair play na camada de serviço (ADR-0007, CX-08): com a trava ativa, todo
//! pedido que calcula (Explorador, avaliação em cache, finais, Engine remota) é recusado
//! pelo gate antes de tocar a rede. Não há chave de depuração que a contorne: a
//! classificação vem do catálogo ([`Capability::computes`]), não de quem pede.
//!
//! Quem liga e desliga a trava é a Partida online (etapa 4): cada Partida online contra uma
//! pessoa em andamento segura uma [`FairPlayHold`] e a solta quando o Provedor confirma o
//! fim; com várias Partidas ao mesmo tempo (correspondência), a trava só cai quando a última
//! solta. A trava é compartilhada: clones apontam para o mesmo estado.

use crate::catalog::{Capability, ProviderId};
use crate::error::NetError;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
struct State {
    /// Ligada à mão ([`FairPlayGuard::engage`]).
    manual: Option<String>,
    /// Uma por Partida online em andamento.
    holds: BTreeMap<u64, String>,
    next: u64,
}

#[derive(Debug, Clone, Default)]
pub struct FairPlayGuard {
    state: Arc<RwLock<State>>,
}

impl FairPlayGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Liga a trava; `reason` é diagnóstico ("Partida online contra pessoa no Lichess").
    pub fn engage(&self, reason: impl Into<String>) {
        if let Ok(mut state) = self.state.write() {
            state.manual = Some(reason.into());
        }
    }

    /// Desliga o que foi ligado por [`FairPlayGuard::engage`]. Não solta as
    /// [`FairPlayHold`]s: cada Partida solta a sua.
    pub fn release(&self) {
        if let Ok(mut state) = self.state.write() {
            state.manual = None;
        }
    }

    /// Liga a trava enquanto a [`FairPlayHold`] devolvida existir (uma por Partida online
    /// contra pessoa em andamento).
    pub fn hold(&self, reason: impl Into<String>) -> FairPlayHold {
        let id = match self.state.write() {
            Ok(mut state) => {
                let id = state.next;
                state.next += 1;
                state.holds.insert(id, reason.into());
                Some(id)
            }
            Err(_) => None,
        };
        FairPlayHold {
            guard: self.clone(),
            id,
        }
    }

    /// Ligada? Um lock envenenado conta como ligada: na dúvida, bloqueia.
    pub fn is_engaged(&self) -> bool {
        self.state
            .read()
            .map_or(true, |s| s.manual.is_some() || !s.holds.is_empty())
    }

    pub fn reason(&self) -> Option<String> {
        self.state.read().ok().and_then(|s| {
            s.manual
                .clone()
                .or_else(|| s.holds.values().next().cloned())
        })
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

/// Uma Partida segurando a trava ligada. Descartar solta (só esta).
#[derive(Debug)]
pub struct FairPlayHold {
    guard: FairPlayGuard,
    id: Option<u64>,
}

impl Drop for FairPlayHold {
    fn drop(&mut self) {
        if let (Some(id), Ok(mut state)) = (self.id, self.guard.state.write()) {
            state.holds.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trava_so_cai_quando_a_ultima_partida_solta() {
        let guard = FairPlayGuard::new();
        let a = guard.hold("Partida A");
        let b = guard.clone().hold("Partida B");
        assert!(guard.is_engaged());
        drop(a);
        assert!(guard.is_engaged());
        assert_eq!(guard.reason().as_deref(), Some("Partida B"));
        guard.release(); // soltar à mão não solta as Partidas
        assert!(guard.is_engaged());
        drop(b);
        assert!(!guard.is_engaged());
        assert!(
            guard
                .check(ProviderId::Lichess, Capability::Explore)
                .is_ok()
        );
    }
}
