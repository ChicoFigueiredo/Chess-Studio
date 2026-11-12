//! Cofre próprio (sem cofre do sistema): nada em texto puro, senha errada, arquivo
//! adulterado ou cortado, troca de senha, bloqueio e nenhuma troca de cofre em silêncio.

mod common;

use chess_net::{AuthorizationSource, ConnectionId};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{SecretVault, VaultError, VaultStatus};
use common::*;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Cofre próprio criado, com o [`TOKEN`] guardado na Conexão 7.
fn own_with_token(name: &str) -> (std::path::PathBuf, Arc<MemoryKeyring>, SecretVault) {
    let dir = temp_dir(name);
    let keyring = Arc::new(MemoryKeyring::unavailable());
    let vault = open(&dir, &keyring, &clock());
    vault
        .create_own(&secret(PASSWORD))
        .expect("cria o cofre próprio");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda");
    (dir, keyring, vault)
}

fn own_file(dir: &Path) -> std::path::PathBuf {
    dir.join("own.vault")
}

#[test]
fn without_a_system_vault_nothing_is_stored_until_the_user_creates_the_own_vault() {
    let dir = temp_dir("sem-sistema");
    let keyring = Arc::new(MemoryKeyring::unavailable());
    let vault = open(&dir, &keyring, &clock());

    assert_eq!(vault.status(), VaultStatus::Unavailable);
    assert_eq!(
        vault.store(PROFILE, LICHESS, CONNECTION, secret(TOKEN)),
        Err(VaultError::Unavailable)
    );
    assert!(
        files(&dir).is_empty(),
        "nada pode ir para o disco: {:?}",
        files(&dir)
    );
    assert!(keyring.accounts().is_empty());
    assert!(vault.authorization(PROFILE, LICHESS, CONNECTION).is_none());

    assert_eq!(
        vault.create_own(&secret("curta")),
        Err(VaultError::WeakPassword { min_chars: 8 })
    );
    vault.create_own(&secret(PASSWORD)).expect("cria");
    assert_eq!(vault.status(), VaultStatus::OwnUnlocked);
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret(TOKEN))
        .expect("guarda no cofre próprio");

    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
    assert!(
        files_containing(&dir, TOKEN).is_empty(),
        "token em claro no disco"
    );
    assert!(
        files_containing(&dir, PASSWORD).is_empty(),
        "senha em claro no disco"
    );
    assert!(files_containing(&dir, "lip_").is_empty());
    let bytes = fs::read(own_file(&dir)).expect("arquivo do cofre");
    assert!(bytes.starts_with(b"CSVAULT\0"));
    assert_eq!(&bytes[8..10], &1u16.to_le_bytes(), "formato versionado");
}

#[test]
fn reopening_starts_locked_and_only_the_right_password_unlocks() {
    let (dir, keyring, vault) = own_with_token("reabre");
    drop(vault);

    let vault = open(&dir, &keyring, &clock());
    assert_eq!(vault.status(), VaultStatus::OwnLocked);
    assert_eq!(vault.references().len(), 1, "a lista funciona bloqueado");
    assert_eq!(vault.read(CONNECTION).unwrap_err(), VaultError::Locked);
    assert!(
        vault.authorization(PROFILE, LICHESS, CONNECTION).is_none(),
        "bloqueado, nenhum cabeçalho"
    );

    assert_eq!(
        vault.unlock(&secret("senha-errada-123")),
        Err(VaultError::WrongPassword)
    );
    assert_eq!(vault.status(), VaultStatus::OwnLocked);
    vault.unlock(&secret(PASSWORD)).expect("desbloqueia");
    assert_eq!(vault.status(), VaultStatus::OwnUnlocked);
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
    assert!(vault.authorization(PROFILE, LICHESS, CONNECTION).is_some());
}

#[test]
fn locking_forgets_the_key_and_the_secrets() {
    let (_dir, _keyring, vault) = own_with_token("bloqueia");

    vault.lock();

    assert_eq!(vault.status(), VaultStatus::OwnLocked);
    assert_eq!(vault.read(CONNECTION).unwrap_err(), VaultError::Locked);
    assert_eq!(
        vault.store(PROFILE, LICHESS, ConnectionId(8), secret(TOKEN)),
        Err(VaultError::Locked)
    );
    assert!(vault.authorization(PROFILE, LICHESS, CONNECTION).is_none());
}

#[test]
fn changing_the_master_password() {
    let (dir, keyring, vault) = own_with_token("troca-senha");
    let before = fs::read(own_file(&dir)).expect("arquivo");
    let new = "outra-senha-mestra";

    assert_eq!(
        vault.change_password(&secret("nao-e-esta-senha"), &secret(new)),
        Err(VaultError::WrongPassword)
    );
    assert_eq!(
        vault.change_password(&secret(PASSWORD), &secret("curta")),
        Err(VaultError::WeakPassword { min_chars: 8 })
    );
    vault
        .change_password(&secret(PASSWORD), &secret(new))
        .expect("troca");
    let after = fs::read(own_file(&dir)).expect("arquivo");
    assert_ne!(&before[23..39], &after[23..39], "sal novo");
    drop(vault);

    let vault = open(&dir, &keyring, &clock());
    assert_eq!(
        vault.unlock(&secret(PASSWORD)),
        Err(VaultError::WrongPassword)
    );
    vault.unlock(&secret(new)).expect("a nova vale");
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
    assert!(files_containing(&dir, TOKEN).is_empty());
    assert!(files_containing(&dir, new).is_empty());
}

#[test]
fn a_tampered_truncated_or_unknown_file_is_refused_and_nothing_is_read() {
    let (dir, keyring, vault) = own_with_token("adulterado");
    drop(vault);
    let path = own_file(&dir);
    let original = fs::read(&path).expect("arquivo");

    let mut cases: Vec<(&str, Vec<u8>, VaultError)> = Vec::new();
    let corrupted = VaultError::Corrupted {
        file: "cofre próprio",
    };
    let mut body = original.clone();
    let last = body.len() - 20;
    body[last] ^= 0x01;
    cases.push(("conteúdo", body, corrupted.clone()));
    let mut tag = original.clone();
    let end = tag.len() - 1;
    tag[end] ^= 0x80;
    cases.push(("etiqueta", tag, corrupted.clone()));
    let mut nonce = original.clone();
    nonce[80] ^= 0x01;
    cases.push(("nonce (no AAD)", nonce, corrupted.clone()));
    cases.push((
        "cortado",
        original[..original.len() - 30].to_vec(),
        corrupted.clone(),
    ));
    cases.push(("só o cabeçalho", original[..50].to_vec(), corrupted.clone()));
    let mut magic = original.clone();
    magic[0] = b'X';
    cases.push(("assinatura", magic, corrupted.clone()));
    let mut huge = original.clone();
    huge[11..15].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(("custo absurdo", huge, corrupted.clone()));
    let mut future = original.clone();
    future[8..10].copy_from_slice(&99u16.to_le_bytes());
    cases.push((
        "versão nova",
        future,
        VaultError::UnsupportedFormat {
            file: "cofre próprio",
            version: 99,
        },
    ));
    let mut salt = original.clone();
    salt[30] ^= 0x01;
    // Sal trocado muda a chave derivada: indistinguível de senha errada.
    cases.push(("sal", salt, VaultError::WrongPassword));

    for (what, bytes, expected) in cases {
        fs::write(&path, &bytes).expect("grava o adulterado");
        let vault = open(&dir, &keyring, &clock());
        assert_eq!(vault.unlock(&secret(PASSWORD)), Err(expected), "{what}");
        assert_eq!(vault.status(), VaultStatus::OwnLocked, "{what}");
        assert!(
            vault.read(CONNECTION).is_err(),
            "{what}: nada lido do arquivo adulterado"
        );
    }

    fs::write(&path, &original).expect("restaura");
    let vault = open(&dir, &keyring, &clock());
    vault.unlock(&secret(PASSWORD)).expect("íntegro de novo");
    assert_eq!(vault.read(CONNECTION).expect("lê").expose_secret(), TOKEN);
}

#[test]
fn a_system_vault_that_appears_later_does_not_take_over_silently() {
    let (dir, _unavailable, vault) = own_with_token("sem-troca");
    drop(vault);

    let system = Arc::new(MemoryKeyring::new());
    let vault = open(&dir, &system, &clock());

    assert_eq!(vault.status(), VaultStatus::OwnLocked);
    assert_eq!(
        vault.create_own(&secret(PASSWORD)),
        Err(VaultError::CannotCreateOwn)
    );
    assert!(system.accounts().is_empty());
}

#[test]
fn deleting_while_locked_drops_the_reference_now_and_the_secret_at_the_next_unlock() {
    let (dir, keyring, vault) = own_with_token("apaga-bloqueado");
    vault
        .store(
            PROFILE,
            LICHESS,
            ConnectionId(8),
            secret("lip_OUTRO_TOKEN_123"),
        )
        .expect("guarda a segunda");
    vault.lock();

    vault.delete(CONNECTION).expect("apaga bloqueado");

    assert_eq!(vault.references().len(), 1);
    assert!(vault.reference(CONNECTION).is_none());
    drop(vault);
    let vault = open(&dir, &keyring, &clock());
    vault.unlock(&secret(PASSWORD)).expect("desbloqueia");
    assert_eq!(vault.read(CONNECTION).unwrap_err(), VaultError::NotFound);
    assert_eq!(
        vault
            .read(ConnectionId(8))
            .expect("a outra fica")
            .expose_secret(),
        "lip_OUTRO_TOKEN_123"
    );
    // A senha certa ainda abre o conteúdo regravado sem a Conexão apagada.
    vault.lock();
    vault
        .unlock(&secret(PASSWORD))
        .expect("desbloqueia de novo");
    assert_eq!(vault.read(CONNECTION).unwrap_err(), VaultError::NotFound);
}

#[test]
fn operations_of_the_own_vault_need_the_own_vault() {
    let dir = temp_dir("sem-proprio");
    let keyring = Arc::new(MemoryKeyring::new());
    let vault = open(&dir, &keyring, &clock());

    assert_eq!(vault.status(), VaultStatus::System);
    assert_eq!(
        vault.create_own(&secret(PASSWORD)),
        Err(VaultError::CannotCreateOwn)
    );
    assert_eq!(vault.unlock(&secret(PASSWORD)), Err(VaultError::NoOwnVault));
    assert_eq!(
        vault.change_password(&secret(PASSWORD), &secret("outra-senha-boa")),
        Err(VaultError::NoOwnVault)
    );
    assert!(!own_file(&dir).exists());
}

/// Revisão de segurança (#273): o arquivo cortado em **qualquer** ponto (queda no meio da
/// gravação, cópia incompleta) é recusado como corrompido, sem pânico e sem ler nada.
#[test]
fn a_file_cut_at_any_length_is_refused_without_panic() {
    let (dir, keyring, vault) = own_with_token("cortado-em-todo-ponto");
    drop(vault);
    let path = own_file(&dir);
    let original = fs::read(&path).expect("arquivo");
    for len in 0..original.len() {
        fs::write(&path, &original[..len]).expect("grava o cortado");
        let vault = open(&dir, &keyring, &clock());
        assert_eq!(
            vault.unlock(&secret(PASSWORD)),
            Err(VaultError::Corrupted {
                file: "cofre próprio"
            }),
            "cortado em {len} bytes"
        );
        assert!(vault.read(CONNECTION).is_err(), "cortado em {len} bytes");
    }
}

/// Revisão de segurança (#273): no Unix, o diretório do Cofre é só do usuário (0700) e os
/// arquivos que ele grava (cofre próprio e índice) também (0600), inclusive depois de
/// regravar.
#[cfg(unix)]
#[test]
fn the_vault_directory_and_files_are_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, _keyring, vault) = own_with_token("permissoes");
    vault
        .store(PROFILE, LICHESS, CONNECTION, secret("lip_outro_token"))
        .expect("regrava");
    let mode = |p: &Path| fs::metadata(p).expect("metadados").permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700, "diretório do Cofre");
    assert_eq!(mode(&own_file(&dir)), 0o600, "cofre próprio");
    assert_eq!(mode(&dir.join("index.json")), 0o600, "índice");
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .expect("lista")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "tmp"))
        .collect();
    assert!(leftovers.is_empty(), "nenhum temporário fica para trás");
}
