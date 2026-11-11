//! Índice das referências (`index.json`): quais Conexões têm segredo guardado, de que
//! Provedor e Perfil local, desde quando, e se o Provedor recusou o token. **Nenhum
//! segredo**: lista-se mesmo com o cofre próprio bloqueado, e a Central mostra o estado sem
//! tocar no segredo. Quem garante o vínculo do segredo com Provedor e Perfil é o próprio
//! cofre (a conta no cofre do sistema e o conteúdo cifrado do cofre próprio levam os dois),
//! não este arquivo.

use crate::error::VaultError;
use chess_net::{ConnectionId, LocalProfileId, ProviderId, Timestamp};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

const FILE: &str = "índice";
const FORMAT: u32 = 1;

/// O Provedor recusou o token da Conexão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejection {
    /// `401`: expirado ou revogado; `403`: falta permissão ao token.
    pub status: u16,
    pub at: Timestamp,
}

/// Referência a um segredo guardado (o que a Central lista). Sem o segredo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    pub connection: ConnectionId,
    pub provider: ProviderId,
    pub profile: LocalProfileId,
    pub stored_at: Timestamp,
    /// A última recusa do Provedor desde que o segredo foi guardado.
    pub rejected: Option<Rejection>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Index {
    format: u32,
    pub(crate) refs: Vec<SecretRef>,
}

impl Index {
    pub(crate) fn load(path: &Path) -> Result<Self, VaultError> {
        let text = match fs::read(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    format: FORMAT,
                    refs: Vec::new(),
                });
            }
            Err(e) => return Err(VaultError::io(&e)),
        };
        let index: Self =
            serde_json::from_slice(&text).map_err(|_| VaultError::Corrupted { file: FILE })?;
        if index.format > FORMAT {
            return Err(VaultError::UnsupportedFormat {
                file: FILE,
                version: index.format,
            });
        }
        Ok(index)
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), VaultError> {
        let text = serde_json::to_vec_pretty(&Self {
            format: FORMAT,
            refs: self.refs.clone(),
        })
        .map_err(|_| VaultError::Io {
            detail: "índice ilegível".to_owned(),
        })?;
        write_private(path, &text)
    }

    pub(crate) fn get(&self, connection: ConnectionId) -> Option<&SecretRef> {
        self.refs.iter().find(|r| r.connection == connection)
    }

    pub(crate) fn upsert(&mut self, reference: SecretRef) {
        self.refs.retain(|r| r.connection != reference.connection);
        self.refs.push(reference);
        self.refs.sort_by_key(|r| r.connection);
    }

    pub(crate) fn remove(&mut self, connection: ConnectionId) -> bool {
        let before = self.refs.len();
        self.refs.retain(|r| r.connection != connection);
        self.refs.len() != before
    }
}

/// Grava `bytes` em `path` de forma atômica (arquivo temporário + renomear), legível só pelo
/// usuário no Unix.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let tmp = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|e| VaultError::io(&e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| VaultError::io(&e))?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| VaultError::io(&e))
}
