//! O cofre próprio: um arquivo cifrado com uma chave derivada da senha-mestra. Usado só
//! quando o sistema não oferece cofre, e nunca em silêncio (o usuário cria e desbloqueia).
//!
//! # Formato, versão 1 (inteiros em little-endian)
//!
//! | Bytes | Campo |
//! |---|---|
//! | 0..8 | assinatura `CSVAULT\0` |
//! | 8..10 | versão do formato (`1`) |
//! | 10 | KDF: `1` = Argon2id (versão 0x13) |
//! | 11..23 | memória (KiB), iterações e paralelismo do Argon2id (`u32` cada) |
//! | 23..39 | sal (16 bytes aleatórios) |
//! | 39..71 | verificador da senha: os primeiros 32 bytes do Argon2id |
//! | 71 | cifra: `1` = XChaCha20-Poly1305 |
//! | 72..96 | nonce (24 bytes aleatórios, novo a cada gravação) |
//! | 96.. | texto cifrado + etiqueta de 16 bytes |
//!
//! O Argon2id da senha com o sal dá 64 bytes: os 32 primeiros são o verificador (senha
//! errada é reconhecida sem tocar no conteúdo) e os 32 últimos, a chave da cifra. O
//! cabeçalho inteiro entra como dado associado (AAD) da cifra autenticada: mudar parâmetro,
//! sal, verificador ou nonce faz a leitura falhar. Conteúdo adulterado, cortado ou com
//! parâmetros fora dos limites é [`VaultError::Corrupted`]; nada dele é aproveitado.
//!
//! O texto em claro é JSON e só existe em memória, em buffers zerados ao descartar.

use crate::error::VaultError;
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{AeadInOut, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

const FILE: &str = "cofre próprio";
const MAGIC: &[u8; 8] = b"CSVAULT\0";
pub(crate) const FORMAT_VERSION: u16 = 1;
const KDF_ARGON2ID: u8 = 1;
const CIPHER_XCHACHA20POLY1305: u8 = 1;
const SALT_LEN: usize = 16;
const CHECK_LEN: usize = 32;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const HEADER_LEN: usize = 8 + 2 + 1 + 12 + SALT_LEN + CHECK_LEN + 1 + NONCE_LEN;

/// Limites aceitos ao ler um arquivo: um cabeçalho com custo absurdo (arquivo corrompido ou
/// forjado) não trava o app tentando derivar a chave.
const MAX_MEMORY_KIB: u32 = 1024 * 1024;
const MAX_ITERATIONS: u32 = 64;
const MAX_PARALLELISM: u32 = 16;

/// Custo do Argon2id. O padrão segue a recomendação da RFC 9106 para memória limitada
/// (64 MiB, 3 passadas); os testes usam valores pequenos para correr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl KdfParams {
    fn within_limits(&self) -> bool {
        (1..=MAX_MEMORY_KIB).contains(&self.memory_kib)
            && (1..=MAX_ITERATIONS).contains(&self.iterations)
            && (1..=MAX_PARALLELISM).contains(&self.parallelism)
    }
}

/// Chave da cifra, zerada ao descartar.
pub(crate) type Key = Zeroizing<[u8; KEY_LEN]>;

/// O que identifica a derivação: parâmetros, sal e verificador (o cabeçalho sem a cifra).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Derivation {
    params: KdfParams,
    salt: [u8; SALT_LEN],
    check: [u8; CHECK_LEN],
}

fn random<const N: usize>() -> Result<[u8; N], VaultError> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|_| VaultError::System {
        detail: "gerador aleatório do sistema indisponível",
    })?;
    Ok(bytes)
}

fn argon2(
    params: &KdfParams,
    password: &[u8],
    salt: &[u8],
) -> Result<Zeroizing<[u8; 64]>, VaultError> {
    let p = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(CHECK_LEN + KEY_LEN),
    )
    .map_err(|_| VaultError::Corrupted { file: FILE })?;
    let mut out = Zeroizing::new([0u8; 64]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into(password, salt, out.as_mut())
        .map_err(|_| VaultError::Corrupted { file: FILE })?;
    Ok(out)
}

fn split(derived: &[u8; 64]) -> ([u8; CHECK_LEN], Key) {
    let mut check = [0u8; CHECK_LEN];
    check.copy_from_slice(&derived[..CHECK_LEN]);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&derived[CHECK_LEN..]);
    (check, key)
}

/// Nova derivação (sal novo) para `password`: criar o cofre e trocar a senha-mestra.
pub(crate) fn derive_new(
    params: KdfParams,
    password: &[u8],
) -> Result<(Derivation, Key), VaultError> {
    let salt = random::<SALT_LEN>()?;
    let derived = argon2(&params, password, &salt)?;
    let (check, key) = split(&derived);
    Ok((
        Derivation {
            params,
            salt,
            check,
        },
        key,
    ))
}

/// Comparação em tempo constante (o verificador não vaza por tempo de resposta).
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn header(derivation: &Derivation, nonce: &[u8; NONCE_LEN]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.push(KDF_ARGON2ID);
    out.extend_from_slice(&derivation.params.memory_kib.to_le_bytes());
    out.extend_from_slice(&derivation.params.iterations.to_le_bytes());
    out.extend_from_slice(&derivation.params.parallelism.to_le_bytes());
    out.extend_from_slice(&derivation.salt);
    out.extend_from_slice(&derivation.check);
    out.push(CIPHER_XCHACHA20POLY1305);
    out.extend_from_slice(nonce);
    out
}

fn cipher(key: &Key) -> Result<XChaCha20Poly1305, VaultError> {
    XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| VaultError::System {
        detail: "chave da cifra inválida",
    })
}

/// Cifra `plaintext` num arquivo completo (cabeçalho + texto cifrado), com nonce novo.
pub(crate) fn seal(
    derivation: &Derivation,
    key: &Key,
    plaintext: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let nonce = random::<NONCE_LEN>()?;
    let mut file = header(derivation, &nonce);
    let aad = file.clone();
    let mut buffer = plaintext.to_vec();
    let tag = cipher(key)?
        .encrypt_inout_detached(&XNonce::from(nonce), &aad, buffer.as_mut_slice().into())
        .map_err(|_| VaultError::System {
            detail: "falha ao cifrar o cofre",
        })?;
    file.extend_from_slice(&buffer);
    file.extend_from_slice(&tag);
    Ok(file)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(b)
}

/// Cabeçalho lido do arquivo, antes de conferir a senha.
pub(crate) struct Sealed<'a> {
    derivation: Derivation,
    nonce: [u8; NONCE_LEN],
    aad: &'a [u8],
    body: &'a [u8],
}

/// Lê e valida a estrutura do arquivo (sem a senha).
pub(crate) fn parse(file: &[u8]) -> Result<Sealed<'_>, VaultError> {
    let corrupted = VaultError::Corrupted { file: FILE };
    if file.len() < 10 || &file[..8] != MAGIC {
        return Err(corrupted);
    }
    let version = u16::from_le_bytes([file[8], file[9]]);
    if version != FORMAT_VERSION {
        return Err(if version > FORMAT_VERSION {
            VaultError::UnsupportedFormat {
                file: FILE,
                version: u32::from(version),
            }
        } else {
            corrupted
        });
    }
    if file.len() < HEADER_LEN + TAG_LEN
        || file[10] != KDF_ARGON2ID
        || file[HEADER_LEN - NONCE_LEN - 1] != CIPHER_XCHACHA20POLY1305
    {
        return Err(corrupted);
    }
    let params = KdfParams {
        memory_kib: u32_at(file, 11),
        iterations: u32_at(file, 15),
        parallelism: u32_at(file, 19),
    };
    if !params.within_limits() {
        return Err(corrupted);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&file[23..23 + SALT_LEN]);
    let mut check = [0u8; CHECK_LEN];
    check.copy_from_slice(&file[39..39 + CHECK_LEN]);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&file[HEADER_LEN - NONCE_LEN..HEADER_LEN]);
    Ok(Sealed {
        derivation: Derivation {
            params,
            salt,
            check,
        },
        nonce,
        aad: &file[..HEADER_LEN],
        body: &file[HEADER_LEN..],
    })
}

impl Sealed<'_> {
    /// Confere a senha e devolve a chave e a derivação (para gravar de novo com a mesma).
    pub(crate) fn unlock(&self, password: &[u8]) -> Result<(Derivation, Key), VaultError> {
        let d = &self.derivation;
        let derived = argon2(&d.params, password, &d.salt)?;
        let (check, key) = split(&derived);
        if !same(&check, &d.check) {
            return Err(VaultError::WrongPassword);
        }
        Ok((d.clone(), key))
    }

    /// Decifra o conteúdo com a chave já conferida.
    pub(crate) fn open(&self, key: &Key) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let (body, tag) = self.body.split_at(self.body.len() - TAG_LEN);
        let mut buffer = Zeroizing::new(body.to_vec());
        let tag = chacha20poly1305::Tag::try_from(tag)
            .map_err(|_| VaultError::Corrupted { file: FILE })?;
        let result = cipher(key)?.decrypt_inout_detached(
            &XNonce::from(self.nonce),
            self.aad,
            buffer.as_mut_slice().into(),
            &tag,
        );
        if result.is_err() {
            buffer.zeroize();
            return Err(VaultError::Corrupted { file: FILE });
        }
        Ok(buffer)
    }
}
