//! Apoio dos testes do Cofre: diretório temporário, cofre do sistema falso, relógio falso e
//! a varredura do disco atrás de segredo em claro.
#![allow(dead_code)]

use chess_net::{ConnectionId, FakeClock, LocalProfileId, ProviderId, Secret, Timestamp};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{KdfParams, SecretVault, VaultConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 2026-11-11T12:00:00Z.
pub const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
pub const TOKEN: &str = "lip_TOKEN_SECRETO_DO_COFRE_42";
pub const PASSWORD: &str = "senha-mestra-longa";
pub const PROFILE: LocalProfileId = LocalProfileId(1);
pub const LICHESS: ProviderId = ProviderId::Lichess;
pub const CONNECTION: ConnectionId = ConnectionId(7);

/// Argon2id barato: só para a suíte correr (o padrão é 64 MiB e 3 passadas).
pub const FAST_KDF: KdfParams = KdfParams {
    memory_kib: 64,
    iterations: 1,
    parallelism: 1,
};

/// Diretório temporário novo (apagado se já existia).
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chess-vault-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

pub fn clock() -> Arc<FakeClock> {
    Arc::new(FakeClock::new(T0))
}

pub fn config(dir: &Path, keyring: &Arc<MemoryKeyring>, clock: &Arc<FakeClock>) -> VaultConfig {
    VaultConfig::new(dir)
        .keyring(keyring.clone())
        .clock(clock.clone())
        .kdf(FAST_KDF)
}

pub fn open(dir: &Path, keyring: &Arc<MemoryKeyring>, clock: &Arc<FakeClock>) -> SecretVault {
    SecretVault::open(config(dir, keyring, clock)).expect("abre o Cofre")
}

pub fn secret(text: &str) -> Secret {
    Secret::new(text)
}

/// Todos os arquivos sob `dir` (recursivo).
pub fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Arquivos sob `dir` que contêm `needle` em claro.
pub fn files_containing(dir: &Path, needle: &str) -> Vec<PathBuf> {
    files(dir)
        .into_iter()
        .filter(|p| {
            fs::read(p)
                .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        })
        .collect()
}
