//! Central de Conexões (CX-3, #214; `docs/decisoes/2026-11-11-conexoes.md`, CX-05, CX-06,
//! CX-09): a página Arquivo > Conexões dos bastidores. Mostra o catálogo de Provedores do
//! `chess-net` em cartões (gratuitos primeiro, por categoria), liga e desliga cada um por Perfil
//! local, vincula nome ou ID público como Conexão e limpa o cache.
//!
//! O modelo (ordenação, estados, ações, campos) é puro e testado em [`model`]; a tela fica em
//! [`view`]. Aqui vivem o estado da tela e a execução dos pedidos da interface: a Central só
//! escreve no `NetService` (consentimento, cache) e no `chess-store` (Conexões); nunca na
//! Partida (ADR-0001).
//!
//! A autorização do Lichess (Cofre, senha-mestra, navegador, chave pessoal, desconectar) vive em
//! [`auth`]; as cenas de demonstração das capturas, em [`demo`].

mod auth;
mod browser;
pub mod demo;
pub mod model;
mod probe;
mod view;

use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::secrets;
use crate::training_ui::with_store;
use auth::{Intent, Slot, VaultMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use chess_net::ProviderId;
use chess_store::{
    Connection, ConnectionId, ConnectionKind, NewConnection, ProfileId, Store, StoreError,
};
use model::CostFilter;
use probe::{Target, TestState};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub use view::section;

/// Um pedido da tela, executado depois do desenho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    SetEnabled(ProviderId, bool),
    ClearCache(ProviderId),
    OpenDocs(ProviderId),
    OpenTerms(ProviderId),
    Link {
        provider: ProviderId,
        kind: ConnectionKind,
        identifier: String,
        nickname: String,
    },
    SetDefault(ConnectionId),
    /// Troca o apelido (vazio tira o apelido).
    Rename(ConnectionId, String),
    Remove(ConnectionId),
    /// Testa a Conexão padrão do cartão; `feature` é o nome do recurso na frase do diálogo.
    Test {
        provider: ProviderId,
        feature: String,
    },
    /// Autorizar a entrada pelo navegador (uma Conexão nova).
    Authorize(Slot),
    /// Autorizar de novo a Conexão expirada ou recusada pelo Provedor.
    Reauthorize(ConnectionId),
    /// Abrir o formulário da chave pessoal (`renew`: a Conexão expirada que ela substitui).
    OpenKeyForm {
        slot: Slot,
        renew: Option<ConnectionId>,
    },
    CloseKeyForm(Slot),
    /// Guardar a chave pessoal do formulário.
    SubmitKey(Slot),
    /// Cancelar a autorização em andamento (ou o pedido que espera o Cofre).
    CancelAuthorization,
    /// Abrir de novo a página de autorização no navegador.
    ReopenBrowser,
    /// Esquecer como terminou a última tentativa da entrada.
    DismissEnding(Slot),
    /// Tentar de novo o último pedido da entrada.
    Retry(Slot),
    /// Conferir a Conexão autorizada com o Provedor.
    TestAuthorized(ConnectionId),
    /// Desconectar: revogar no Provedor, apagar o segredo e remover a Conexão.
    Disconnect(ConnectionId),
    OpenVaultDialog(VaultMode),
    CloseVaultDialog,
    SubmitVaultDialog,
    LockVault,
}

/// Formulário de vincular, aberto num cartão.
#[derive(Clone, Debug, Default)]
pub(crate) struct LinkForm {
    pub identifier: String,
    pub nickname: String,
    /// Id Fluent do erro a mostrar abaixo do campo.
    pub error: Option<&'static str>,
}

/// Edição do apelido de uma Conexão, na própria linha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenameForm {
    pub id: ConnectionId,
    pub text: String,
    /// Id Fluent do erro a mostrar abaixo do campo.
    pub error: Option<&'static str>,
}

/// Retorno de uma ação, na linha de baixo do cartão: id Fluent e um argumento opcional.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub key: &'static str,
    pub arg: Option<(&'static str, String)>,
}

impl Note {
    fn plain(key: &'static str) -> Self {
        Self { key, arg: None }
    }

    fn with(key: &'static str, name: &'static str, value: impl ToString) -> Self {
        Self {
            key,
            arg: Some((name, value.to_string())),
        }
    }
}

/// Estado da tela da Central.
#[derive(Resource, Debug, Default)]
pub struct ConnectionsUi {
    pub(crate) filter: CostFilter,
    pub(crate) query: String,
    pub(crate) forms: HashMap<ProviderId, LinkForm>,
    pub(crate) notes: HashMap<ProviderId, Note>,
    /// O teste de Conexão de cada cartão (em andamento ou o último resultado).
    pub(crate) tests: HashMap<ProviderId, TestState>,
    /// Os cartões com os detalhes abertos.
    pub(crate) expanded: HashSet<ProviderId>,
    /// O apelido em edição, se há um.
    pub(crate) renaming: Option<RenameForm>,
    /// A Conexão que espera a confirmação para ser removida.
    pub(crate) confirm_remove: Option<ConnectionId>,
    /// Provedores com Conexão **autorizada e em uso** (segredo no Cofre aberto, não recusada
    /// pelo Provedor). Derivado do banco e do Cofre por [`ConnectionsUi::sync`]: "Autorizado"
    /// nunca aparece sem isto.
    pub(crate) authorized: HashSet<ProviderId>,
    /// Autorizações em andamento, diálogos do Cofre e situação de cada Conexão autorizada.
    pub(crate) auth: auth::Auth,
    /// Como abrir páginas no navegador (os testes trocam por um gravador).
    pub(crate) opener: auth::Opener,
    /// Rolagem vertical inicial, uma vez (`--connections-scroll`, para as capturas).
    pub(crate) scroll: Option<(f32, u8)>,
    /// As Conexões do Perfil local ativo.
    pub(crate) list: Vec<Connection>,
    loaded_for: Option<ProfileId>,
    dirty: bool,
    /// Banco em memória quando não há persistência (capturas, benchmark).
    local: Option<Mutex<Store>>,
}

/// O que a página precisa dos recursos do app (cabe num só parâmetro do sistema dos bastidores).
#[derive(SystemParam)]
pub struct CentralParams<'w> {
    pub(crate) ui: ResMut<'w, ConnectionsUi>,
    pub(crate) service: Res<'w, NetService>,
    pub(crate) dialog: ResMut<'w, ConsentDialog>,
    persistence: Option<Res<'w, PersistenceHandle>>,
}

impl ConnectionsUi {
    /// Conexões de exemplo para as capturas (`--connections-demo`): sem banco em disco. Os nomes
    /// são fictícios: nunca apelidos de pessoas reais.
    pub fn demo() -> Self {
        let mut ui = Self::default();
        let samples = [
            NewConnection::new("lichess", ConnectionKind::PublicName, "ana_exemplo")
                .nickname("Principal"),
            NewConnection::new("lichess", ConnectionKind::PublicName, "bruno_exemplo")
                .nickname("Treino"),
            NewConnection::new("chesscom", ConnectionKind::PublicName, "carla_exemplo"),
            NewConnection::new("fide", ConnectionKind::PublicId, "9999991"),
        ];
        let _ = with_store(&mut ui.local, None, |store, profile| {
            for sample in &samples {
                let _ = store.add_connection(profile, sample);
            }
        });
        // Um teste feito há alguns minutos, para a evidência mostrar o resultado.
        let now = NetService::now();
        let at = chess_net::Timestamp::from_millis(now.as_millis() - 3 * 60_000);
        ui.tests.insert(
            ProviderId::Lichess,
            TestState::Done(probe::Report {
                outcome: probe::Outcome::Confirmed("ana_exemplo".to_owned()),
                source: ProviderId::Lichess,
                at,
            }),
        );
        ui.dirty = true;
        ui
    }

    /// A página já rolada `pixels` para baixo (capturas de evidência).
    pub fn scrolled(&mut self, pixels: f32) {
        self.scroll = Some((pixels, 8));
    }

    /// Abre os Detalhes dos Provedores de chave dada (capturas de evidência).
    pub fn open_details<'a>(&mut self, keys: impl IntoIterator<Item = &'a str>) {
        for key in keys {
            if let Some(provider) = ProviderId::from_key(key.trim()) {
                self.expanded.insert(provider);
            }
        }
    }

    /// Recarrega as Conexões quando o Perfil local mudou ou algo foi gravado.
    pub(crate) fn refresh(&mut self, handle: Option<&PersistenceHandle>) {
        let (loaded_for, dirty) = (self.loaded_for, self.dirty);
        let loaded = with_store(&mut self.local, handle, |store, profile| {
            if loaded_for == Some(profile) && !dirty {
                return (profile, None);
            }
            (profile, store.list_connections(profile, None).ok())
        });
        let Some((profile, list)) = loaded else {
            return;
        };
        if loaded_for != Some(profile) {
            self.list.clear();
            self.forms.clear();
            self.notes.clear();
        }
        if let Some(list) = list {
            self.list = list;
        }
        self.loaded_for = Some(profile);
        self.dirty = false;
    }

    /// Executa os pedidos que a tela juntou no quadro.
    pub(crate) fn apply(
        &mut self,
        ops: Vec<Op>,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        for op in ops {
            match op {
                Op::SetEnabled(provider, enabled) => {
                    self.notes.remove(&provider);
                    if service.set_enabled(provider, enabled).is_err() {
                        self.notes
                            .insert(provider, Note::plain("connections-switch-error"));
                    }
                }
                Op::ClearCache(provider) => {
                    let note = match service.net().clear_cache(provider) {
                        Ok(0) => Note::plain("connections-cache-cleared-none"),
                        Ok(1) => Note::plain("connections-cache-cleared-one"),
                        Ok(n) => Note::with("connections-cache-cleared-many", "count", n),
                        Err(_) => Note::plain("connections-switch-error"),
                    };
                    self.notes.insert(provider, note);
                }
                Op::Authorize(slot) => self.authorize(slot, None, service, dialog, handle),
                Op::Reauthorize(id) => {
                    let slot = self.list.iter().find(|c| c.id == id).and_then(|c| {
                        ProviderId::from_key(&c.provider).map(|provider| Slot {
                            provider,
                            role: c.role,
                        })
                    });
                    if let Some(slot) = slot {
                        self.authorize(slot, Some(id), service, dialog, handle);
                    }
                }
                Op::OpenKeyForm { slot, renew } => self.open_key_form(slot, renew),
                Op::CloseKeyForm(slot) => {
                    self.auth.key_forms.remove(&slot);
                }
                Op::SubmitKey(slot) => self.submit_key(slot, service, dialog, handle),
                Op::CancelAuthorization => self.cancel_authorization(service, handle),
                Op::ReopenBrowser => self.reopen_browser(),
                Op::DismissEnding(slot) => {
                    self.auth.endings.remove(&slot);
                }
                Op::Retry(slot) => match self.auth.last.get(&slot).copied() {
                    // A chave digitada já foi consumida: reabre o formulário vazio.
                    Some(Intent::Key { renew, .. }) => {
                        self.open_key_form(slot, renew);
                    }
                    Some(Intent::Browser { renew, .. }) => {
                        self.authorize(slot, renew, service, dialog, handle);
                    }
                    None => self.authorize(slot, None, service, dialog, handle),
                },
                Op::TestAuthorized(id) => self.check_authorized(id, service),
                Op::Disconnect(id) => self.disconnect(id, service),
                Op::OpenVaultDialog(mode) => self.open_vault_dialog(mode),
                Op::CloseVaultDialog => self.close_vault_dialog(),
                Op::SubmitVaultDialog => self.submit_vault_dialog(service),
                Op::LockVault => self.lock_vault(service, handle),
                Op::OpenDocs(provider) | Op::OpenTerms(provider) => {
                    // O endereço vem do catálogo, nunca da tela.
                    let info = provider.info();
                    let url = if matches!(op, Op::OpenDocs(_)) {
                        info.docs_url
                    } else {
                        info.terms_url
                    };
                    self.notes.remove(&provider);
                    if !self.opener.open(url, false) {
                        self.notes
                            .insert(provider, Note::with("connections-open-failed", "url", url));
                    }
                }
                Op::Link {
                    provider,
                    kind,
                    identifier,
                    nickname,
                } => self.link(provider, kind, identifier, nickname, handle),
                Op::SetDefault(id) => {
                    let _ = with_store(&mut self.local, handle, |store, profile| {
                        store.set_default_connection(profile, id)
                    });
                    self.dirty = true;
                }
                Op::Rename(id, nickname) => {
                    let result = with_store(&mut self.local, handle, |store, profile| {
                        store.rename_connection(profile, id, &nickname)
                    });
                    match result {
                        Some(Ok(())) => {
                            self.renaming = None;
                            self.dirty = true;
                        }
                        _ => {
                            if let Some(form) = self.renaming.as_mut() {
                                form.error = Some("connections-rename-error");
                            }
                        }
                    }
                }
                Op::Remove(id) => {
                    // O segredo sai do Cofre antes da linha sair do banco; se o Cofre recusar,
                    // a Conexão fica e a tela explica.
                    let vault = service.vault().clone();
                    let result = with_store(&mut self.local, handle, |store, profile| {
                        secrets::forget_connection(store, &vault, profile, id)
                    });
                    let provider = self
                        .list
                        .iter()
                        .find(|c| c.id == id)
                        .and_then(|c| ProviderId::from_key(&c.provider));
                    if let (Some(Err(_)), Some(provider)) = (result, provider) {
                        self.notes
                            .insert(provider, Note::plain("connections-remove-error"));
                    }
                    self.confirm_remove = None;
                    self.dirty = true;
                }
                Op::Test { provider, feature } => {
                    self.start_test(provider, feature, service, dialog);
                }
            }
        }
        self.sync(service, handle);
    }

    /// Testa a Conexão padrão do Provedor. Provedor desligado: abre o diálogo "Ligar
    /// <Provedor>?" e espera a resposta; ligado: consulta já, sem bloquear.
    fn start_test(
        &mut self,
        provider: ProviderId,
        feature: String,
        service: &NetService,
        dialog: &mut ConsentDialog,
    ) {
        let Some(connection) = self.of_provider_default(provider) else {
            return;
        };
        let target = match connection.kind {
            ConnectionKind::PublicId => Target::FideId(connection.identifier.clone()),
            // A autorizada também se testa pelo nome público que o Provedor confirmou.
            ConnectionKind::PublicName
            | ConnectionKind::AuthorizedBrowser
            | ConnectionKind::AuthorizedKey => Target::Name(connection.identifier.clone()),
        };
        let now = NetService::now();
        let via = model::test_via(provider);
        let state = if service.is_enabled(via) {
            match probe::start(service, provider, &target, now) {
                Ok(query) => TestState::Running {
                    card: provider,
                    query: Box::new(std::sync::Mutex::new(query)),
                },
                Err(report) => TestState::Done(report),
            }
        } else {
            dialog.ask(via, Some(feature));
            if dialog.current() != Some(via) {
                // Outro diálogo está aberto: a pessoa tenta de novo depois.
                return;
            }
            TestState::AwaitingConsent {
                card: provider,
                target,
            }
        };
        self.tests.insert(provider, state);
    }

    /// Avança os testes: respostas do diálogo e consultas que terminaram (a cada quadro).
    pub(crate) fn poll(&mut self, service: &NetService, answers: &[ProviderConsentAnswered]) {
        probe::advance(&mut self.tests, service, answers, NetService::now());
    }

    /// Tudo o que a Central faz a cada quadro, com ou sem a página aberta: recarrega as
    /// Conexões, mantém a Conexão autorizada do `NetService` em dia, avança as autorizações,
    /// os diálogos do Cofre e os testes.
    pub(crate) fn drive(
        &mut self,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
        answers: &[ProviderConsentAnswered],
    ) {
        self.poll_auth(service, dialog, handle, answers);
        if !self.tests.is_empty() {
            self.poll(service, answers);
        }
        self.sync(service, handle);
    }

    fn of_provider_default(&self, provider: ProviderId) -> Option<&Connection> {
        self.list
            .iter()
            .filter(|c| c.provider == provider.key())
            .filter(|c| c.role == chess_store::ConnectionRole::Player)
            .filter(|c| !c.is_pending())
            .find(|c| c.is_default)
    }

    fn link(
        &mut self,
        provider: ProviderId,
        kind: ConnectionKind,
        identifier: String,
        nickname: String,
        handle: Option<&PersistenceHandle>,
    ) {
        let new = NewConnection::new(provider.key(), kind, identifier).nickname(nickname);
        let result = with_store(&mut self.local, handle, |store, profile| {
            store.add_connection(profile, &new)
        });
        match result {
            Some(Ok(_)) => {
                self.forms.remove(&provider);
                self.dirty = true;
            }
            Some(Err(err)) => {
                if let Some(form) = self.forms.get_mut(&provider) {
                    form.error = Some(link_error_key(&err));
                }
            }
            None => {
                if let Some(form) = self.forms.get_mut(&provider) {
                    form.error = Some("connections-link-error-other");
                }
            }
        }
    }

    /// Conexões do Provedor, do papel de jogador, a padrão primeiro.
    #[cfg(test)]
    pub(crate) fn of_provider(&self, provider: ProviderId) -> Vec<&Connection> {
        let mut found: Vec<&Connection> = self
            .list
            .iter()
            .filter(|c| c.provider == provider.key())
            .filter(|c| c.role == chess_store::ConnectionRole::Player)
            .filter(|c| !c.is_pending())
            .collect();
        found.sort_by_key(|c| !c.is_default);
        found
    }
}

/// Id Fluent da mensagem de um erro ao vincular.
pub(crate) fn link_error_key(error: &StoreError) -> &'static str {
    match error {
        StoreError::InvalidConnection(_) => "connections-link-error-invalid",
        StoreError::ConnectionExists { .. } => "connections-link-error-exists",
        _ => "connections-link-error-other",
    }
}

pub struct ConnectionsPlugin;

impl Plugin for ConnectionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectionsUi>()
            .add_systems(Update, drive_connections);
    }
}

/// A Central trabalha a cada quadro mesmo com a página fechada: o Explorador depende da Conexão
/// autorizada que ela mantém em dia, e uma autorização em andamento não pode parar.
fn drive_connections(
    mut ui: ResMut<ConnectionsUi>,
    service: Res<NetService>,
    mut dialog: ResMut<ConsentDialog>,
    persistence: Option<Res<PersistenceHandle>>,
    mut answers: MessageReader<ProviderConsentAnswered>,
) {
    let answered: Vec<ProviderConsentAnswered> = answers.read().copied().collect();
    ui.drive(&service, &mut dialog, persistence.as_deref(), &answered);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> NetService {
        NetService::in_memory()
    }

    fn link(ui: &mut ConnectionsUi, service: &NetService, provider: ProviderId, id: &str) {
        ui.forms.entry(provider).or_default();
        ui.apply(
            vec![Op::Link {
                provider,
                kind: ConnectionKind::PublicName,
                identifier: id.into(),
                nickname: String::new(),
            }],
            service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
    }

    #[test]
    fn linking_a_public_name_persists_and_closes_the_form() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        ui.refresh(None);
        assert!(ui.of_provider(ProviderId::Lichess).is_empty());
        link(&mut ui, &service, ProviderId::Lichess, "magnus");
        let list = ui.of_provider(ProviderId::Lichess);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].identifier, "magnus");
        assert!(list[0].is_default);
        assert!(!ui.forms.contains_key(&ProviderId::Lichess));
    }

    #[test]
    fn several_connections_per_provider_with_a_default_that_can_change() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        link(&mut ui, &service, ProviderId::ChessCom, "hikaru");
        link(&mut ui, &service, ProviderId::ChessCom, "fabi");
        let list = ui.of_provider(ProviderId::ChessCom);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].identifier, "hikaru", "a padrão vem primeiro");
        let second = list[1].id;
        ui.apply(
            vec![Op::SetDefault(second)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::ChessCom)[0].id, second);
        ui.apply(
            vec![Op::Remove(second)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
        let left = ui.of_provider(ProviderId::ChessCom);
        assert_eq!(left.len(), 1);
        assert!(left[0].is_default);
    }

    #[test]
    fn a_bad_or_repeated_name_keeps_the_form_open_with_a_message() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        link(&mut ui, &service, ProviderId::Lichess, "magnus");
        link(&mut ui, &service, ProviderId::Lichess, "MAGNUS");
        assert_eq!(
            ui.forms[&ProviderId::Lichess].error,
            Some("connections-link-error-exists")
        );
        link(&mut ui, &service, ProviderId::Lichess, "com espaço");
        assert_eq!(
            ui.forms[&ProviderId::Lichess].error,
            Some("connections-link-error-invalid")
        );
        assert_eq!(ui.of_provider(ProviderId::Lichess).len(), 1);
    }

    #[test]
    fn switching_a_provider_goes_through_the_net_service_and_clearing_the_cache_reports() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        ui.apply(
            vec![Op::SetEnabled(ProviderId::Lichess, true)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        assert!(service.is_enabled(ProviderId::Lichess));
        ui.apply(
            vec![Op::SetEnabled(ProviderId::Lichess, false)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        assert!(!service.is_enabled(ProviderId::Lichess));
        // Provedor sem serviço de rede não liga: a tela explica.
        ui.apply(
            vec![Op::SetEnabled(ProviderId::Twic, true)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        assert_eq!(
            ui.notes[&ProviderId::Twic],
            Note::plain("connections-switch-error")
        );
        ui.apply(
            vec![Op::ClearCache(ProviderId::Lichess)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        assert_eq!(
            ui.notes[&ProviderId::Lichess],
            Note::plain("connections-cache-cleared-none")
        );
    }

    #[test]
    fn renaming_a_nickname_persists_and_a_bad_one_keeps_the_form_with_a_message() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        link(&mut ui, &service, ProviderId::Lichess, "ana_exemplo");
        let id = ui.of_provider(ProviderId::Lichess)[0].id;
        ui.renaming = Some(RenameForm {
            id,
            text: "Principal".into(),
            error: None,
        });
        ui.apply(
            vec![Op::Rename(id, "Principal".into())],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::Lichess)[0].nickname, "Principal");
        assert_eq!(ui.renaming, None, "salvar fecha a edição");
        // Um apelido longe do limite mantém a edição aberta, com o motivo.
        ui.renaming = Some(RenameForm {
            id,
            text: "x".repeat(200),
            error: None,
        });
        ui.apply(
            vec![Op::Rename(id, "x".repeat(200))],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        assert_eq!(
            ui.renaming.as_ref().and_then(|f| f.error),
            Some("connections-rename-error")
        );
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::Lichess)[0].nickname, "Principal");
        // Apelido vazio tira o apelido.
        ui.apply(
            vec![Op::Rename(id, String::new())],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
        assert!(ui.of_provider(ProviderId::Lichess)[0].nickname.is_empty());
    }

    #[test]
    fn removing_clears_the_confirmation_and_the_nobody_is_authorized_yet() {
        let service = service();
        let mut ui = ConnectionsUi::default();
        link(&mut ui, &service, ProviderId::ChessCom, "carla_exemplo");
        let id = ui.of_provider(ProviderId::ChessCom)[0].id;
        ui.confirm_remove = Some(id);
        ui.apply(
            vec![Op::Remove(id)],
            &service,
            &mut ConsentDialog::default(),
            None,
        );
        ui.refresh(None);
        assert!(ui.of_provider(ProviderId::ChessCom).is_empty());
        assert_eq!(ui.confirm_remove, None);
        // Nenhum Provedor tem Conexão autorizada até o Cofre existir.
        assert!(ui.authorized.is_empty());
    }

    #[test]
    fn the_demo_has_connections_for_the_captures() {
        let mut ui = ConnectionsUi::demo();
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::Lichess).len(), 2);
        assert_eq!(ui.of_provider(ProviderId::Fide).len(), 1);
        // Nomes fictícios: nunca apelidos de pessoas reais.
        let names: Vec<&str> = ui.list.iter().map(|c| c.identifier.as_str()).collect();
        assert!(names.contains(&"ana_exemplo"));
        assert!(
            names
                .iter()
                .all(|n| n.contains("exemplo") || n.chars().all(|c| c.is_ascii_digit()))
        );
    }
}
