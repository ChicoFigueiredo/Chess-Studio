//! [`SecretVault`]: a interface única do Cofre para o app.

use crate::error::{MIN_PASSWORD_CHARS, VaultError};
use crate::index::{Index, Rejection, SecretRef, write_private};
use crate::keyring::{OsKeyring, SystemKeyring};
use crate::own::{self, Derivation, KdfParams, Key};
use chess_net::{
    Authorization, AuthorizationSource, Clock, ConnectionId, LocalProfileId, ProviderId, Secret,
    SystemClock,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use zeroize::Zeroizing;

const INDEX_FILE: &str = "index.json";
const OWN_FILE: &str = "own.vault";

/// Onde o Cofre guarda os segredos agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VaultStatus {
    /// Cofre do sistema operacional.
    System,
    /// Cofre próprio, bloqueado: a senha-mestra é pedida quando um segredo for necessário.
    OwnLocked,
    /// Cofre próprio, desbloqueado.
    OwnUnlocked,
    /// Sem cofre do sistema e sem cofre próprio: nada pode ser guardado até o usuário criar
    /// o cofre próprio com uma senha-mestra (nunca há gravação em texto puro).
    Unavailable,
}

/// Configuração do Cofre. O padrão serve ao app; os testes trocam o cofre do sistema (por
/// um falso), o relógio e o custo do Argon2id.
#[derive(Debug, Clone)]
pub struct VaultConfig {
    dir: PathBuf,
    keyring: Arc<dyn OsKeyring>,
    clock: Arc<dyn Clock>,
    kdf: KdfParams,
}

impl VaultConfig {
    /// Cofre no diretório `dir` (dentro do diretório de dados do app), com o cofre do
    /// sistema de verdade.
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
            keyring: Arc::new(SystemKeyring),
            clock: Arc::new(SystemClock),
            kdf: KdfParams::default(),
        }
    }

    pub fn keyring(mut self, keyring: Arc<dyn OsKeyring>) -> Self {
        self.keyring = keyring;
        self
    }

    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Custo do Argon2id ao criar o cofre próprio ou trocar a senha (ao abrir, vale o que
    /// está no arquivo). O padrão é o recomendado; valores baixos são só para testes.
    pub fn kdf(mut self, params: KdfParams) -> Self {
        self.kdf = params;
        self
    }
}

/// Um segredo do cofre próprio, com o vínculo que o protege (cifrado junto).
#[derive(Clone, Serialize, Deserialize)]
struct OwnEntry {
    connection: ConnectionId,
    provider: ProviderId,
    profile: LocalProfileId,
    #[serde(with = "secret_text")]
    secret: Secret,
}

mod secret_text {
    use chess_net::Secret;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(secret: &Secret, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(secret.expose_secret())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Secret, D::Error> {
        String::deserialize(d).map(Secret::new)
    }
}

#[derive(Serialize, Deserialize)]
struct OwnContent {
    entries: Vec<OwnEntry>,
}

enum Backend {
    System {
        /// Segredos já lidos do cofre do sistema nesta sessão (evita uma ida ao D-Bus por
        /// pedido).
        cache: HashMap<ConnectionId, Secret>,
    },
    OwnLocked,
    OwnUnlocked {
        derivation: Derivation,
        key: Key,
        entries: HashMap<ConnectionId, OwnEntry>,
    },
    Unavailable,
}

impl Backend {
    fn status(&self) -> VaultStatus {
        match self {
            Self::System { .. } => VaultStatus::System,
            Self::OwnLocked => VaultStatus::OwnLocked,
            Self::OwnUnlocked { .. } => VaultStatus::OwnUnlocked,
            Self::Unavailable => VaultStatus::Unavailable,
        }
    }
}

struct State {
    backend: Backend,
    index: Index,
}

/// Segredo em conferência ([`SecretVault::stage`]).
struct Staged {
    id: u64,
    provider: ProviderId,
    profile: LocalProfileId,
    secret: Secret,
}

struct Inner {
    dir: PathBuf,
    keyring: Arc<dyn OsKeyring>,
    clock: Arc<dyn Clock>,
    kdf: KdfParams,
    state: Mutex<State>,
    staged: Mutex<HashMap<ConnectionId, Staged>>,
    next_stage: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// O Cofre (_SecretVault_): onde ficam os segredos das Conexões. Barato de clonar; os
/// clones compartilham o estado. Implementa [`AuthorizationSource`]: entregue-o à camada de
/// rede com `NetConfig::vault`.
///
/// - **Cofre do sistema primeiro.** Sem ele, o usuário cria o cofre próprio
///   ([`SecretVault::create_own`]); até lá nada é guardado. Nunca há gravação em texto puro
///   nem troca de cofre em silêncio: havendo o arquivo do cofre próprio, é ele que vale.
/// - **Cofre próprio** desbloqueado sob demanda ([`SecretVault::unlock`]), bloqueável
///   ([`SecretVault::lock`]), com troca de senha-mestra.
/// - O `Debug` não mostra segredo; segredos, chave e conteúdo decifrado são zerados na
///   memória ao descartar.
#[derive(Clone)]
pub struct SecretVault {
    inner: Arc<Inner>,
}

impl fmt::Debug for SecretVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretVault")
            .field("status", &self.status())
            .field("references", &self.references().len())
            .finish_non_exhaustive()
    }
}

fn account(provider: ProviderId, profile: LocalProfileId, connection: ConnectionId) -> String {
    format!("{}/{}/{}", provider.key(), profile.0, connection.0)
}

fn system_error(err: crate::keyring::KeyringError) -> VaultError {
    VaultError::System {
        detail: err.detail(),
    }
}

fn check_password(password: &Secret) -> Result<(), VaultError> {
    if password.expose_secret().chars().count() < MIN_PASSWORD_CHARS {
        return Err(VaultError::WeakPassword {
            min_chars: MIN_PASSWORD_CHARS,
        });
    }
    Ok(())
}

impl SecretVault {
    /// Abre o Cofre: o cofre próprio se o arquivo dele existe (bloqueado), senão o do
    /// sistema se responde, senão [`VaultStatus::Unavailable`]. Não grava nada.
    pub fn open(config: VaultConfig) -> Result<Self, VaultError> {
        create_private_dir(&config.dir)?;
        let index = Index::load(&config.dir.join(INDEX_FILE))?;
        let backend = if config.dir.join(OWN_FILE).exists() {
            Backend::OwnLocked
        } else if config.keyring.is_available() {
            Backend::System {
                cache: HashMap::new(),
            }
        } else {
            Backend::Unavailable
        };
        Ok(Self {
            inner: Arc::new(Inner {
                dir: config.dir,
                keyring: config.keyring,
                clock: config.clock,
                kdf: config.kdf,
                state: Mutex::new(State { backend, index }),
                staged: Mutex::new(HashMap::new()),
                next_stage: AtomicU64::new(1),
            }),
        })
    }

    pub fn status(&self) -> VaultStatus {
        lock(&self.inner.state).backend.status()
    }

    /// Referências guardadas (sem segredo), em ordem de Conexão. Funciona com o cofre
    /// bloqueado.
    pub fn references(&self) -> Vec<SecretRef> {
        lock(&self.inner.state).index.refs.clone()
    }

    pub fn reference(&self, connection: ConnectionId) -> Option<SecretRef> {
        lock(&self.inner.state).index.get(connection).cloned()
    }

    fn own_path(&self) -> PathBuf {
        self.inner.dir.join(OWN_FILE)
    }

    fn index_path(&self) -> PathBuf {
        self.inner.dir.join(INDEX_FILE)
    }

    // ---------------- Cofre próprio ----------------

    /// Cria o cofre próprio com a senha-mestra (só quando o sistema não oferece cofre e
    /// ainda não há um). Fica desbloqueado.
    pub fn create_own(&self, password: &Secret) -> Result<(), VaultError> {
        check_password(password)?;
        if self.status() != VaultStatus::Unavailable {
            return Err(VaultError::CannotCreateOwn);
        }
        // O Argon2id é lento de propósito: fora da trava, para `status()` seguir instantâneo.
        let (derivation, key) =
            own::derive_new(self.inner.kdf, password.expose_secret().as_bytes())?;
        let mut state = lock(&self.inner.state);
        if !matches!(state.backend, Backend::Unavailable) || self.own_path().exists() {
            return Err(VaultError::CannotCreateOwn);
        }
        let entries = HashMap::new();
        write_own(&self.own_path(), &derivation, &key, &entries)?;
        state.backend = Backend::OwnUnlocked {
            derivation,
            key,
            entries,
        };
        Ok(())
    }

    /// Desbloqueia o cofre próprio. Senha errada é [`VaultError::WrongPassword`]; arquivo
    /// adulterado ou cortado, [`VaultError::Corrupted`].
    pub fn unlock(&self, password: &Secret) -> Result<(), VaultError> {
        if matches!(
            self.status(),
            VaultStatus::System | VaultStatus::Unavailable
        ) {
            return Err(VaultError::NoOwnVault);
        }
        let (derivation, key, entries) = read_own(&self.own_path(), password)?;
        let mut state = lock(&self.inner.state);
        // Apagado com o cofre bloqueado: some do índice na hora e do arquivo agora.
        let known: Vec<ConnectionId> = state.index.refs.iter().map(|r| r.connection).collect();
        let before = entries.len();
        let entries: HashMap<ConnectionId, OwnEntry> = entries
            .into_iter()
            .filter(|(c, _)| known.contains(c))
            .collect();
        if entries.len() != before {
            write_own(&self.own_path(), &derivation, &key, &entries)?;
        }
        state.backend = Backend::OwnUnlocked {
            derivation,
            key,
            entries,
        };
        Ok(())
    }

    /// Bloqueia o cofre próprio: chave e segredos saem da memória (zerados).
    pub fn lock(&self) {
        let mut state = lock(&self.inner.state);
        if matches!(state.backend, Backend::OwnUnlocked { .. }) {
            state.backend = Backend::OwnLocked;
        }
    }

    /// Troca a senha-mestra (pede a atual; sal e chave novos). O cofre fica desbloqueado.
    pub fn change_password(&self, current: &Secret, new: &Secret) -> Result<(), VaultError> {
        check_password(new)?;
        if matches!(
            self.status(),
            VaultStatus::System | VaultStatus::Unavailable
        ) {
            return Err(VaultError::NoOwnVault);
        }
        let (_, _, entries) = read_own(&self.own_path(), current)?;
        let (derivation, key) = own::derive_new(self.inner.kdf, new.expose_secret().as_bytes())?;
        let mut state = lock(&self.inner.state);
        let entries: HashMap<ConnectionId, OwnEntry> = match &state.backend {
            // Desbloqueado: vale o que está em memória (igual ao arquivo).
            Backend::OwnUnlocked { entries, .. } => entries.clone(),
            _ => entries,
        };
        write_own(&self.own_path(), &derivation, &key, &entries)?;
        state.backend = Backend::OwnUnlocked {
            derivation,
            key,
            entries,
        };
        Ok(())
    }

    // ---------------- Segredos ----------------

    /// Guarda (ou substitui) o segredo da Conexão. O anterior é descartado e zerado.
    pub fn store(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
        secret: Secret,
    ) -> Result<(), VaultError> {
        if secret.is_empty() {
            return Err(VaultError::EmptySecret);
        }
        let system = {
            let state = lock(&self.inner.state);
            match &state.backend {
                Backend::Unavailable => return Err(VaultError::Unavailable),
                Backend::OwnLocked => return Err(VaultError::Locked),
                Backend::System { .. } => Some(
                    state
                        .index
                        .get(connection)
                        .map(|old| account(old.provider, old.profile, connection)),
                ),
                Backend::OwnUnlocked { .. } => None,
            }
        };
        if let Some(old_account) = system {
            // O cofre do sistema pode demorar (D-Bus, pedido de desbloqueio): fora da trava.
            let new_account = account(provider, profile, connection);
            if let Some(old) = old_account.filter(|old| *old != new_account) {
                // Outro Provedor ou Perfil na mesma Conexão: a conta antiga sai.
                self.inner.keyring.delete(&old).map_err(system_error)?;
            }
            self.inner
                .keyring
                .set(&new_account, &secret)
                .map_err(system_error)?;
        }
        let mut state = lock(&self.inner.state);
        let State { backend, index } = &mut *state;
        match backend {
            Backend::Unavailable => return Err(VaultError::Unavailable),
            Backend::OwnLocked => return Err(VaultError::Locked),
            Backend::System { cache } => {
                cache.insert(connection, secret);
            }
            Backend::OwnUnlocked {
                derivation,
                key,
                entries,
            } => {
                let mut next = entries.clone();
                next.insert(
                    connection,
                    OwnEntry {
                        connection,
                        provider,
                        profile,
                        secret,
                    },
                );
                write_own(&self.own_path(), derivation, key, &next)?;
                *entries = next;
            }
        }
        index.upsert(SecretRef {
            connection,
            provider,
            profile,
            stored_at: self.inner.clock.now(),
            rejected: None,
        });
        index.save(&self.index_path())
    }

    /// Lê o segredo da Conexão.
    pub fn read(&self, connection: ConnectionId) -> Result<Secret, VaultError> {
        let (reference, cached) = {
            let state = lock(&self.inner.state);
            let reference = state
                .index
                .get(connection)
                .cloned()
                .ok_or(VaultError::NotFound)?;
            match &state.backend {
                Backend::Unavailable => return Err(VaultError::Unavailable),
                Backend::OwnLocked => return Err(VaultError::Locked),
                Backend::OwnUnlocked { entries, .. } => {
                    return entries
                        .get(&connection)
                        .filter(|e| {
                            e.provider == reference.provider && e.profile == reference.profile
                        })
                        .map(|e| e.secret.clone())
                        .ok_or(VaultError::NotFound);
                }
                Backend::System { cache } => (reference, cache.get(&connection).cloned()),
            }
        };
        if let Some(secret) = cached {
            return Ok(secret);
        }
        // O cofre do sistema pode demorar (D-Bus, pedido de desbloqueio): fora da trava.
        let secret = self
            .inner
            .keyring
            .get(&account(reference.provider, reference.profile, connection))
            .map_err(system_error)?
            .ok_or(VaultError::NotFound)?;
        if let Backend::System { cache } = &mut lock(&self.inner.state).backend {
            cache.insert(connection, secret.clone());
        }
        Ok(secret)
    }

    /// Lê de uma vez, do cofre do sistema, os segredos de todas as referências e devolve
    /// quantos vieram. Chame numa thread de fundo ao abrir o app: o cofre do sistema pode
    /// demorar ou pedir desbloqueio ao usuário, e o gate de rede consulta o Cofre na thread
    /// de quem pede (a interface). Com o cofre próprio, não faz nada.
    pub fn preload(&self) -> Result<usize, VaultError> {
        if self.status() != VaultStatus::System {
            return Ok(0);
        }
        let mut loaded = 0;
        for reference in self.references() {
            match self.read(reference.connection) {
                Ok(_) => loaded += 1,
                // Referência sem segredo no cofre do sistema: os pedidos dela são recusados.
                Err(VaultError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(loaded)
    }

    /// Apaga o segredo da Conexão. Com o cofre próprio bloqueado, a referência sai na hora (o
    /// segredo fica inalcançável) e o conteúdo cifrado é limpo no próximo desbloqueio.
    pub fn delete(&self, connection: ConnectionId) -> Result<(), VaultError> {
        lock(&self.inner.staged).remove(&connection);
        let system = {
            let mut state = lock(&self.inner.state);
            let State { backend, index } = &mut *state;
            let Some(reference) = index.get(connection) else {
                return Ok(());
            };
            match backend {
                Backend::System { cache } => {
                    cache.remove(&connection);
                    Some(account(reference.provider, reference.profile, connection))
                }
                _ => None,
            }
        };
        if let Some(account) = system {
            // Fora da trava; se o cofre do sistema falhar, a referência fica para nova
            // tentativa (o segredo continua lá e o app não finge que apagou).
            self.inner.keyring.delete(&account).map_err(system_error)?;
        }
        let mut state = lock(&self.inner.state);
        let State { backend, index } = &mut *state;
        match backend {
            Backend::System { .. } => {}
            Backend::OwnUnlocked {
                derivation,
                key,
                entries,
            } => {
                let mut next = entries.clone();
                next.remove(&connection);
                write_own(&self.own_path(), derivation, key, &next)?;
                *entries = next;
            }
            Backend::OwnLocked | Backend::Unavailable => {}
        }
        index.remove(connection);
        index.save(&self.index_path())
    }

    /// Põe `secret` em conferência: enquanto o [`StagedSecret`] existir, os pedidos da
    /// Conexão usam este segredo, só em memória; o guardado (se houver) não muda. Confirme
    /// com [`StagedSecret::commit`]; descartar sem confirmar joga o segredo fora.
    pub fn stage(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
        secret: Secret,
    ) -> StagedSecret {
        let id = self.inner.next_stage.fetch_add(1, Ordering::Relaxed);
        lock(&self.inner.staged).insert(
            connection,
            Staged {
                id,
                provider,
                profile,
                secret,
            },
        );
        StagedSecret {
            vault: self.clone(),
            connection,
            id,
        }
    }

    fn take_staged(&self, connection: ConnectionId, id: u64) -> Option<Staged> {
        let mut staged = lock(&self.inner.staged);
        if staged.get(&connection).is_some_and(|s| s.id == id) {
            staged.remove(&connection)
        } else {
            None
        }
    }
}

/// Um segredo em conferência. Descartar sem [`StagedSecret::commit`] o joga fora.
pub struct StagedSecret {
    vault: SecretVault,
    connection: ConnectionId,
    id: u64,
}

impl fmt::Debug for StagedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagedSecret")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}

impl StagedSecret {
    /// Guarda o segredo em conferência no Cofre (substituindo o anterior).
    pub fn commit(self) -> Result<(), VaultError> {
        let staged = self
            .vault
            .take_staged(self.connection, self.id)
            .ok_or(VaultError::NotFound)?;
        self.vault.store(
            staged.profile,
            staged.provider,
            self.connection,
            staged.secret,
        )
    }
}

impl Drop for StagedSecret {
    fn drop(&mut self) {
        let _ = self.vault.take_staged(self.connection, self.id);
    }
}

impl AuthorizationSource for SecretVault {
    /// O cabeçalho só sai para a Conexão do mesmo Provedor e do mesmo Perfil local com que o
    /// segredo foi guardado. Cofre bloqueado ou indisponível: nenhum cabeçalho (o gate
    /// recusa o pedido com `NotAuthorized`, sem rede).
    fn authorization(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
    ) -> Option<Authorization> {
        if let Some(staged) = lock(&self.inner.staged).get(&connection) {
            return (staged.provider == provider && staged.profile == profile)
                .then(|| Authorization::bearer(provider, &staged.secret));
        }
        let reference = self.reference(connection)?;
        if reference.provider != provider || reference.profile != profile {
            return None;
        }
        let secret = self.read(connection).ok()?;
        Some(Authorization::bearer(provider, &secret))
    }

    fn rejected(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
        status: u16,
    ) {
        if lock(&self.inner.staged).contains_key(&connection) {
            // Segredo em conferência: quem confere trata a recusa; nada guardado muda.
            return;
        }
        let mut state = lock(&self.inner.state);
        let at = self.inner.clock.now();
        let Some(reference) =
            state.index.refs.iter_mut().find(|r| {
                r.connection == connection && r.provider == provider && r.profile == profile
            })
        else {
            return;
        };
        reference.rejected = Some(Rejection { status, at });
        let _ = state.index.save(&self.index_path());
    }
}

fn create_private_dir(dir: &Path) -> Result<(), VaultError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(|e| VaultError::io(&e))
}

fn write_own(
    path: &Path,
    derivation: &Derivation,
    key: &Key,
    entries: &HashMap<ConnectionId, OwnEntry>,
) -> Result<(), VaultError> {
    let mut list: Vec<OwnEntry> = entries.values().cloned().collect();
    list.sort_by_key(|e| e.connection);
    let plaintext = Zeroizing::new(serde_json::to_vec(&OwnContent { entries: list }).map_err(
        |_| VaultError::System {
            detail: "conteúdo do cofre ilegível",
        },
    )?);
    let file = own::seal(derivation, key, &plaintext)?;
    write_private(path, &file)
}

type OwnRead = (Derivation, Key, HashMap<ConnectionId, OwnEntry>);

fn read_own(path: &Path, password: &Secret) -> Result<OwnRead, VaultError> {
    let file = fs::read(path).map_err(|e| VaultError::io(&e))?;
    let sealed = own::parse(&file)?;
    let (derivation, key) = sealed.unlock(password.expose_secret().as_bytes())?;
    let plaintext = sealed.open(&key)?;
    let content: OwnContent =
        serde_json::from_slice(&plaintext).map_err(|_| VaultError::Corrupted {
            file: "cofre próprio",
        })?;
    let entries = content
        .entries
        .into_iter()
        .map(|e| (e.connection, e))
        .collect();
    Ok((derivation, key, entries))
}
