//! Segredos e a entrada para o Cofre. Este crate nunca guarda segredo: a cada pedido com
//! Conexão, pergunta ao [`AuthorizationSource`] (o Cofre, `chess-vault`) e só põe o
//! cabeçalho nos endereços do catálogo que recebem o token daquele Provedor
//! ([`crate::Endpoint::takes_token`]). `Debug` de tudo aqui esconde o valor, e a memória é
//! zerada ao descartar.

use crate::LocalProfileId;
use crate::catalog::ProviderId;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroizing;

/// Valor secreto (token, chave, senha-mestra). Não tem `Display`; o `Debug` mostra só que
/// existe; a memória é zerada quando o valor é descartado (cada clone zera a sua cópia).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    /// O valor em claro. Só o Cofre (para gravar no cofre do sistema ou cifrar) e o
    /// transporte (para montar o cabeçalho) leem o segredo; um teste estático do
    /// `chess-vault` falha se outro arquivo de produção chamar esta função.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// O mesmo segredo sem espaços nas pontas (token colado com quebra de linha).
    pub fn trimmed(&self) -> Self {
        Self::new(self.0.trim())
    }

    /// Tem forma de token e pode ir num cabeçalho `Authorization`: de 1 a
    /// [`MAX_TOKEN_LEN`] bytes de ASCII visível, sem espaço nem caractere de controle.
    pub fn is_token_like(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= MAX_TOKEN_LEN
            && self.0.bytes().all(|b| b.is_ascii_graphic())
    }
}

/// Tamanho máximo de um token aceito (a especificação pede tolerar 512+ caracteres).
pub const MAX_TOKEN_LEN: usize = 4096;

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(«oculto»)")
    }
}

/// Cabeçalho `Authorization` de uma Conexão, preso ao Provedor dela: o gate só o envia para
/// os endereços desse Provedor que o catálogo marca como receptores do token
/// ([`crate::Endpoint::takes_token`]); nunca para outro Provedor, outro host do mesmo
/// Provedor que não esteja marcado, nem num redirecionamento.
#[derive(Clone, PartialEq, Eq)]
pub struct Authorization {
    provider: ProviderId,
    value: Secret,
}

impl Authorization {
    /// `Authorization: Bearer <token>` (Lichess, PlayStrategy).
    pub fn bearer(provider: ProviderId, token: &Secret) -> Self {
        // Capacidade exata: sem realocação, nenhuma cópia parcial fica na memória liberada.
        let mut value = String::with_capacity("Bearer ".len() + token.len());
        value.push_str("Bearer ");
        value.push_str(token.expose_secret());
        Self {
            provider,
            value: Secret::new(value),
        }
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    pub(crate) fn header_value(&self) -> &str {
        self.value.expose_secret()
    }
}

impl fmt::Debug for Authorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authorization")
            .field("provider", &self.provider)
            .field("value", &self.value)
            .finish()
    }
}

/// Uma Conexão do Perfil local com o Provedor (o identificador vem da Central).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConnectionId(pub i64);

/// De onde vem o `Authorization` de uma Conexão: o Cofre (`chess_vault::SecretVault`)
/// implementa este trait. Chamado a cada tentativa, logo antes da rede, para que trocar ou
/// revogar a Conexão valha no pedido seguinte.
pub trait AuthorizationSource: Send + Sync + fmt::Debug {
    fn authorization(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
    ) -> Option<Authorization>;

    /// O Provedor recusou o cabeçalho da Conexão: `401` (token expirado ou revogado) ou
    /// `403` (falta permissão ao token). O Cofre marca a Conexão; nada local se perde.
    fn rejected(
        &self,
        _profile: LocalProfileId,
        _provider: ProviderId,
        _connection: ConnectionId,
        _status: u16,
    ) {
    }
}

/// Sem Cofre: nenhuma Conexão tem autorização.
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
