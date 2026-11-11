//! Os segredos das Conexões no app (CX-10, CX-11, #231): como o Cofre é aberto, o cofre falso das
//! capturas e dos testes, as tarefas de fundo e o **ciclo de vida seguro**.
//!
//! - **Abrir.** Em produção, [`open_vault`] abre o `SecretVault` do diretório de dados (cofre do
//!   sistema primeiro; cofre próprio se o arquivo existe). Sem diretório de dados (capturas,
//!   benchmark, testes) o Cofre é um [`volatile_vault`]: um cofre do sistema **falso** em memória
//!   e um diretório temporário, apagado ao sair. Nada disso toca o cofre real do usuário.
//! - **Ciclo de vida.** O `id` da Conexão é a chave do segredo. [`forget_connection`] e
//!   [`forget_profile`] apagam o segredo no Cofre **antes** de apagar a linha no banco; se o Cofre
//!   recusa, a linha fica (e a pessoa é avisada) em vez de deixar um segredo órfão. O banco usa
//!   `AUTOINCREMENT`, então o `id` apagado nunca volta, e uma Conexão nova jamais herda um
//!   segredo (teste com reaproveitamento simulado de `id`).
//! - **Nunca bloquear a interface.** Criar, desbloquear e trocar a senha-mestra rodam o Argon2id
//!   (lento de propósito): [`Job`] os leva para uma thread e a interface consulta a cada quadro.

use chess_net::{ConnectionId as NetConnectionId, LocalProfileId, ProviderId, Secret};
use chess_store::{Connection, ConnectionId, ProfileId, Store, StoreError};
use chess_vault::{
    KeyringError, OsKeyring, SecretRef, SecretVault, VaultConfig, VaultError, VaultStatus,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, PoisonError};
use std::thread;

/// Nome do diretório do Cofre, ao lado do banco das Partidas.
pub const VAULT_DIR: &str = "cofre";

/// O diretório do Cofre que fica ao lado do banco das Partidas.
pub fn vault_dir_beside(games_db: &Path) -> PathBuf {
    games_db.with_file_name(VAULT_DIR)
}

/// A Conexão do banco com o `id` que o Cofre e a rede usam.
pub fn net_id(id: ConnectionId) -> NetConnectionId {
    NetConnectionId(id.0)
}

pub fn net_profile(profile: ProfileId) -> LocalProfileId {
    LocalProfileId(profile.0)
}

// --- Cofre do sistema falso -------------------------------------------------------------------

/// Cofre do sistema **falso**, em memória: o que as capturas, o benchmark e os testes usam no
/// lugar do cofre real do usuário. Pode fingir que não existe.
#[derive(Debug, Default)]
pub struct VolatileKeyring {
    entries: Mutex<HashMap<String, Secret>>,
    missing: bool,
    failing: std::sync::atomic::AtomicBool,
}

impl VolatileKeyring {
    /// Um cofre do sistema que responde.
    pub fn new() -> Self {
        Self::default()
    }

    /// Um sistema sem cofre (Linux sem Secret Service): o Cofre fica "indisponível".
    pub fn missing() -> Self {
        Self {
            missing: true,
            ..Self::default()
        }
    }

    /// Cada operação passa a falhar (o cofre do sistema caiu ou foi trancado): os testes provam
    /// que nada é removido quando o segredo não pôde ser apagado.
    #[cfg(test)]
    pub fn set_failing(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    fn check(&self) -> Result<(), KeyringError> {
        if self.missing {
            return Err(KeyringError::Unavailable);
        }
        if self.failing.load(Ordering::SeqCst) {
            return Err(KeyringError::Failed("falha simulada"));
        }
        Ok(())
    }

    /// Contas com segredo, em ordem (os testes conferem que nada sobrou).
    pub fn accounts(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        out.sort();
        out
    }
}

impl OsKeyring for VolatileKeyring {
    fn is_available(&self) -> bool {
        !self.missing
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeyringError> {
        self.check()?;
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(account.to_owned(), secret.clone());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, KeyringError> {
        self.check()?;
        Ok(self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(account)
            .cloned())
    }

    fn delete(&self, account: &str) -> Result<(), KeyringError> {
        self.check()?;
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(account);
        Ok(())
    }
}

/// Diretório temporário do Cofre falso: apagado ao descartar.
#[derive(Debug)]
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("chess-studio-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// O Cofre aberto e o que ele precisa para viver.
#[derive(Debug)]
pub struct OpenedVault {
    pub vault: SecretVault,
    /// Diretório temporário do Cofre falso (apagado ao descartar); `None` com o Cofre real.
    pub scratch: Option<ScratchDir>,
    /// O Cofre real não pôde ser aberto (índice adulterado, disco): a Central explica, e nada é
    /// guardado nem trocado em silêncio.
    pub error: Option<VaultError>,
    /// É o Cofre real (cofre do sistema ou arquivo do usuário). Só ele passa por `preload`.
    pub real: bool,
}

/// Abre o Cofre **real** em `dir`. Se não abrir, devolve um Cofre fechado e falso (nada é
/// guardado) junto com o erro, para a Central mostrar o motivo.
pub fn open_vault(dir: &Path) -> OpenedVault {
    match SecretVault::open(VaultConfig::new(dir)) {
        Ok(vault) => OpenedVault {
            vault,
            scratch: None,
            error: None,
            real: true,
        },
        Err(error) => {
            let mut closed = volatile_vault(VolatileKeyring::missing(), "cofre-fechado");
            closed.error = Some(error);
            closed
        }
    }
}

/// Cofre falso e temporário (capturas, benchmark, testes): nunca o cofre real do usuário.
pub fn volatile_vault(keyring: VolatileKeyring, label: &str) -> OpenedVault {
    volatile_vault_with(std::sync::Arc::new(keyring), label, None)
}

/// Como [`volatile_vault`], com um `keyring` que o chamador também guarda (os testes olham o
/// que sobrou nele) e, se dado, um custo menor para o Argon2id.
pub fn volatile_vault_with(
    keyring: std::sync::Arc<VolatileKeyring>,
    label: &str,
    kdf: Option<chess_vault::KdfParams>,
) -> OpenedVault {
    let scratch = ScratchDir::new(label);
    let mut config = VaultConfig::new(scratch.path().join(VAULT_DIR)).keyring(keyring);
    if let Some(kdf) = kdf {
        config = config.kdf(kdf);
    }
    // O diretório temporário é criado pelo próprio Cofre; só falha com o disco temporário cheio,
    // e nesse caso o app segue sem Cofre (fechado).
    let (vault, error) = match SecretVault::open(config) {
        Ok(vault) => (vault, None),
        Err(error) => (closed_vault(), Some(error)),
    };
    OpenedVault {
        vault,
        scratch: Some(scratch),
        error,
        real: false,
    }
}

/// Cofre sem diretório útil: indisponível e sem onde gravar.
fn closed_vault() -> SecretVault {
    let config = VaultConfig::new(std::env::temp_dir().join("chess-studio-sem-cofre"))
        .keyring(std::sync::Arc::new(VolatileKeyring::missing()));
    // `open` só falha se o diretório não puder ser criado; o Cofre fechado é o melhor que resta.
    SecretVault::open(config).expect("diretório temporário indisponível")
}

// --- Ciclo de vida seguro ---------------------------------------------------------------------

/// Por que uma Conexão ou um Perfil local não foi apagado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgetError {
    /// O Cofre não apagou o segredo: **nada foi removido** (o segredo seguiria órfão).
    Vault(VaultError),
    Store(String),
}

impl From<StoreError> for ForgetError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// Apaga a Conexão `id`: primeiro o segredo no Cofre, depois a linha no banco. Se o Cofre falha, a
/// linha fica. Apagar o que não tem segredo é inofensivo.
pub fn forget_connection(
    store: &mut Store,
    vault: &SecretVault,
    profile: ProfileId,
    id: ConnectionId,
) -> Result<(), ForgetError> {
    // Confere o dono antes de mexer no Cofre: a Conexão de outro Perfil local não se apaga.
    if !store
        .list_connections(profile, None)?
        .iter()
        .any(|c| c.id == id)
    {
        return Err(ForgetError::Store("Conexão não encontrada".to_owned()));
    }
    vault.delete(net_id(id)).map_err(ForgetError::Vault)?;
    store.remove_connection(profile, id)?;
    Ok(())
}

/// Apaga o Perfil local `profile`: antes, **todos** os segredos dele no Cofre (os das Conexões do
/// banco e quaisquer referências do Cofre que apontem para o Perfil). Só então o banco apaga o
/// Perfil e as Conexões. Se o Cofre falha, o Perfil fica.
pub fn forget_profile(
    store: &mut Store,
    vault: &SecretVault,
    profile: ProfileId,
) -> Result<(), ForgetError> {
    let mut ids: Vec<NetConnectionId> = store
        .list_connections(profile, None)?
        .iter()
        .map(|c| net_id(c.id))
        .collect();
    let mine = net_profile(profile);
    ids.extend(
        vault
            .references()
            .into_iter()
            .filter(|r| r.profile == mine)
            .map(|r| r.connection),
    );
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    for id in ids {
        vault.delete(id).map_err(ForgetError::Vault)?;
    }
    store.delete_profile(profile)?;
    Ok(())
}

// --- Estado da autorização de uma Conexão -----------------------------------------------------

/// Como está a autorização de uma Conexão **autorizada**, vista pelo Cofre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Tem segredo, o Cofre está aberto e o Provedor não o recusou.
    Active,
    /// O cofre próprio está bloqueado: a autorização existe, mas não pode ser usada agora.
    Locked,
    /// O Provedor recusou (expirada, revogada ou sem permissão) ou o segredo sumiu do Cofre:
    /// reautorizar. Os dados locais ficam.
    Expired,
    /// Sem cofre para guardar nada (indisponível): não pode estar autorizada.
    Unusable,
}

/// A situação da Conexão `connection` (que precisa ser autorizada) no `vault`. `loaded` diz que
/// o Cofre real já leu os segredos (a leitura de fundo terminou).
pub fn standing(connection: &Connection, vault: &SecretVault, loaded: bool) -> Standing {
    match vault.status() {
        VaultStatus::Unavailable => Standing::Unusable,
        VaultStatus::OwnLocked => Standing::Locked,
        VaultStatus::System | VaultStatus::OwnUnlocked => {
            if !loaded {
                return Standing::Locked;
            }
            match vault.reference(net_id(connection.id)) {
                Some(SecretRef {
                    rejected: None,
                    provider,
                    ..
                }) if ProviderId::from_key(&connection.provider) == Some(provider) => {
                    Standing::Active
                }
                _ => Standing::Expired,
            }
        }
    }
}

// --- Tarefas de fundo -------------------------------------------------------------------------

/// Uma tarefa de fundo com um resultado só, consultada a cada quadro (nunca bloqueia).
#[derive(Debug)]
pub struct Job<T> {
    /// `Receiver` não é `Sync`: o `Mutex` deixa a tarefa viver num recurso do Bevy.
    rx: Mutex<Receiver<T>>,
    finished: bool,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new().name(name.to_owned()).spawn(move || {
            let _ = tx.send(work());
        });
        // Sem a thread (sem recursos do sistema), o canal fecha e `poll` devolve `None` para
        // sempre; o chamador tem o prazo da própria tela.
        let _ = spawned;
        Self {
            rx: Mutex::new(rx),
            finished: false,
        }
    }

    /// O resultado, uma vez.
    pub fn poll(&mut self) -> Option<T> {
        if self.finished {
            return None;
        }
        let received = self
            .rx
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv();
        match received {
            Ok(value) => {
                self.finished = true;
                Some(value)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }

    /// Espera (tempo real) pelo resultado. Para testes.
    #[cfg(test)]
    pub fn wait(&mut self, timeout: std::time::Duration) -> Option<T> {
        let start = std::time::Instant::now();
        loop {
            if let Some(value) = self.poll() {
                return Some(value);
            }
            if self.finished || start.elapsed() >= timeout {
                return None;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_store::{ConnectionKind, ConnectionRole, NewConnection};
    use chess_vault::KdfParams;
    use std::sync::Arc;

    const FAST: KdfParams = KdfParams {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };

    fn fast_vault() -> (OpenedVault, Arc<VolatileKeyring>) {
        let keyring = Arc::new(VolatileKeyring::new());
        (
            volatile_vault_with(keyring.clone(), "teste-segredos", Some(FAST)),
            keyring,
        )
    }

    /// Uma Conexão autorizada do Lichess com segredo no Cofre.
    fn authorized(store: &mut Store, vault: &SecretVault, profile: ProfileId) -> ConnectionId {
        let id = store
            .begin_authorization(
                profile,
                "lichess",
                ConnectionRole::Player,
                ConnectionKind::AuthorizedBrowser,
            )
            .unwrap();
        vault
            .store(
                net_profile(profile),
                ProviderId::Lichess,
                net_id(id),
                Secret::new("lio_SEGREDO_DE_TESTE"),
            )
            .unwrap();
        store
            .record_authorization(
                profile,
                id,
                ConnectionKind::AuthorizedBrowser,
                "georges",
                &["board:play".to_owned()],
                1,
            )
            .unwrap();
        id
    }

    #[test]
    fn deleting_a_connection_deletes_its_secret_first() {
        let (opened, keyring) = fast_vault();
        let mut store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        let id = authorized(&mut store, &opened.vault, profile);
        assert_eq!(keyring.accounts().len(), 1);
        assert!(opened.vault.reference(net_id(id)).is_some());

        forget_connection(&mut store, &opened.vault, profile, id).unwrap();

        assert!(keyring.accounts().is_empty(), "o segredo saiu do cofre");
        assert!(opened.vault.reference(net_id(id)).is_none());
        assert!(store.list_connections(profile, None).unwrap().is_empty());
    }

    #[test]
    fn when_the_vault_refuses_to_delete_nothing_is_removed() {
        let (opened, keyring) = fast_vault();
        let mut store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        let other = store.create_profile("Outro").unwrap();
        let id = authorized(&mut store, &opened.vault, profile);
        keyring.set_failing(true);
        let result = forget_connection(&mut store, &opened.vault, profile, id);
        assert!(matches!(result, Err(ForgetError::Vault(_))), "{result:?}");
        assert_eq!(store.list_connections(profile, None).unwrap().len(), 1);
        assert!(opened.vault.reference(net_id(id)).is_some());
        // O mesmo vale para o Perfil local inteiro.
        let result = forget_profile(&mut store, &opened.vault, profile);
        assert!(matches!(result, Err(ForgetError::Vault(_))));
        assert!(
            store
                .list_profiles()
                .unwrap()
                .iter()
                .any(|p| p.id == profile)
        );
        // Uma Conexão de outro Perfil local não se apaga por este Perfil, nem o segredo dela.
        keyring.set_failing(false);
        let result = forget_connection(&mut store, &opened.vault, other, id);
        assert!(matches!(result, Err(ForgetError::Store(_))));
        assert!(opened.vault.reference(net_id(id)).is_some());
        // Com o cofre de volta, apagar funciona.
        forget_connection(&mut store, &opened.vault, profile, id).unwrap();
        assert!(keyring.accounts().is_empty());
    }

    #[test]
    fn deleting_a_local_profile_deletes_every_secret_of_it_and_only_of_it() {
        let (opened, keyring) = fast_vault();
        let mut store = Store::open_in_memory().unwrap();
        let ana = store.create_profile("Ana").unwrap();
        let beto = store.create_profile("Beto").unwrap();
        let ana_id = authorized(&mut store, &opened.vault, ana);
        let beto_id = authorized(&mut store, &opened.vault, beto);
        // Um segredo do Perfil da Ana que o banco já não conhece (referência órfã).
        opened
            .vault
            .store(
                net_profile(ana),
                ProviderId::Lichess,
                NetConnectionId(900),
                Secret::new("lio_ORFAO"),
            )
            .unwrap();
        assert_eq!(keyring.accounts().len(), 3);

        forget_profile(&mut store, &opened.vault, ana).unwrap();

        assert!(opened.vault.reference(net_id(ana_id)).is_none());
        assert!(opened.vault.reference(NetConnectionId(900)).is_none());
        assert!(
            opened.vault.reference(net_id(beto_id)).is_some(),
            "o segredo do outro Perfil local fica"
        );
        assert_eq!(keyring.accounts().len(), 1);
        assert!(store.list_connections(ana, None).unwrap().is_empty());
        assert!(!store.list_profiles().unwrap().iter().any(|p| p.id == ana));
    }

    #[test]
    fn a_new_connection_never_inherits_a_secret_even_if_an_id_were_reused() {
        // Defesa em profundidade, dos dois lados: (1) o banco não reaproveita ids; (2) mesmo que
        // um id voltasse (banco antigo, linha apagada por fora), apagar a Conexão pelo caminho
        // seguro já removeu o segredo, então a "nova" não encontra nada no Cofre.
        let (opened, keyring) = fast_vault();
        let mut store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        let old = authorized(&mut store, &opened.vault, profile);
        forget_connection(&mut store, &opened.vault, profile, old).unwrap();

        let fresh = store
            .add_connection(
                profile,
                &NewConnection::new("lichess", ConnectionKind::PublicName, "nova"),
            )
            .unwrap();
        assert!(fresh > old, "AUTOINCREMENT: o id não volta");
        // Simula o pior caso: a Conexão nova usa exatamente o id antigo.
        for candidate in [net_id(old), net_id(fresh)] {
            assert!(opened.vault.reference(candidate).is_none());
            assert!(opened.vault.read(candidate).is_err());
        }
        assert!(keyring.accounts().is_empty());
        let conn = store.list_connections(profile, None).unwrap().remove(0);
        assert_ne!(
            standing(&conn, &opened.vault, true),
            Standing::Active,
            "sem segredo, nunca ativa"
        );
    }

    #[test]
    fn the_standing_follows_the_vault_and_the_providers_refusal() {
        let (opened, _keyring) = fast_vault();
        let mut store = Store::open_in_memory().unwrap();
        let profile = store.default_profile().unwrap();
        let id = authorized(&mut store, &opened.vault, profile);
        let conn = |store: &Store| store.list_connections(profile, None).unwrap().remove(0);
        assert_eq!(
            standing(&conn(&store), &opened.vault, true),
            Standing::Active
        );
        assert_eq!(
            standing(&conn(&store), &opened.vault, false),
            Standing::Locked,
            "antes da leitura de fundo terminar, ainda não vale"
        );
        // Segredo apagado por fora: a autorização sumiu.
        opened.vault.delete(net_id(id)).unwrap();
        assert_eq!(
            standing(&conn(&store), &opened.vault, true),
            Standing::Expired
        );
    }

    /// O código que mexe com segredo nunca escreve em log nem na saída: nem a senha-mestra, nem
    /// a chave pessoal, nem o que o Cofre devolve.
    #[test]
    fn secret_handling_code_never_logs_or_prints() {
        let sources = [
            ("secrets.rs", include_str!("secrets.rs")),
            ("connections/auth.rs", include_str!("connections/auth.rs")),
            (
                "connections/view_auth.rs",
                include_str!("connections/view_auth.rs"),
            ),
            ("connections/demo.rs", include_str!("connections/demo.rs")),
        ];
        for (name, source) in sources {
            // O código de produção: tudo antes do último módulo de testes.
            let code = source
                .rsplit_once("#[cfg(test)]")
                .map_or(source, |(head, _)| head);
            for forbidden in [
                "println!",
                "eprintln!",
                "print!(",
                "dbg!",
                "info!(",
                "warn!(",
                "debug!(",
                "trace!(",
                "error!(",
                "tracing::",
                " log::",
                "(log::",
                "set_contents",
            ] {
                assert!(!code.contains(forbidden), "{forbidden} em {name}");
            }
        }
    }

    #[test]
    fn a_job_delivers_one_result_without_blocking() {
        let mut job = Job::spawn("teste-job", || 7);
        assert_eq!(job.wait(std::time::Duration::from_secs(5)), Some(7));
        assert_eq!(job.poll(), None, "só uma vez");
    }

    #[test]
    fn the_fake_system_vault_can_pretend_not_to_exist() {
        let opened = volatile_vault(VolatileKeyring::missing(), "teste-sem-cofre");
        assert_eq!(opened.vault.status(), VaultStatus::Unavailable);
        assert!(!opened.real);
        let normal = volatile_vault(VolatileKeyring::new(), "teste-com-cofre");
        assert_eq!(normal.vault.status(), VaultStatus::System);
    }
}
