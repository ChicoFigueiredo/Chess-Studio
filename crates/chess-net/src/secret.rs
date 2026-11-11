//! Segredos e a entrada para o Cofre (etapa 3). Este crate nunca guarda segredo: a cada
//! pedido com Conexão, pergunta ao [`AuthorizationSource`] e usa o cabeçalho só no host a
//! que ele pertence. `Debug` de tudo aqui esconde o valor.

use crate::LocalProfileId;
use crate::catalog::ProviderId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Valor secreto (token, chave). Não tem `Display`; o `Debug` mostra só que existe.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Só a camada de transporte lê o valor, para montar o cabeçalho.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(«oculto»)")
    }
}

/// Cabeçalho `Authorization` de uma Conexão, preso a um host: nunca vai para outro host do
/// mesmo Provedor, nem num redirecionamento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    host: String,
    value: Secret,
}

impl Authorization {
    /// `Authorization: Bearer <token>` (Lichess, PlayStrategy).
    pub fn bearer(host: impl Into<String>, token: Secret) -> Self {
        Self {
            host: host.into().to_ascii_lowercase(),
            value: Secret(format!("Bearer {}", token.expose())),
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub(crate) fn header_value(&self) -> &str {
        self.value.expose()
    }
}

/// Uma Conexão do Perfil local com o Provedor (o identificador vem da Central, etapa 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConnectionId(pub i64);

/// De onde vem o `Authorization` de uma Conexão: o Cofre (`SecretVault`) da etapa 3
/// implementa este trait. Chamado a cada tentativa, logo antes da rede, para que trocar ou
/// revogar a Conexão valha no pedido seguinte.
pub trait AuthorizationSource: Send + Sync + fmt::Debug {
    fn authorization(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
    ) -> Option<Authorization>;
}

/// Sem Cofre (etapas 1 e 2): nenhuma Conexão tem autorização.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoVault;

impl AuthorizationSource for NoVault {
    fn authorization(
        &self,
        _profile: LocalProfileId,
        _provider: ProviderId,
        _connection: ConnectionId,
    ) -> Option<Authorization> {
        None
    }
}
