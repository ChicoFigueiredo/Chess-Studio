//! Central de Conexões (CX-3, #214; `docs/decisoes/2026-11-11-conexoes.md`, CX-05, CX-06,
//! CX-09): a página Arquivo > Conexões dos bastidores. Mostra o catálogo de Provedores do
//! `chess-net` em cartões (gratuitos primeiro, por categoria), liga e desliga cada um por Perfil
//! local, vincula nome ou ID público como Conexão e limpa o cache.
//!
//! O modelo (ordenação, estados, ações, campos) é puro e testado em [`model`]; a tela fica em
//! [`view`]. Aqui vivem o estado da tela e a execução dos pedidos da interface: a Central só
//! escreve no `NetService` (consentimento, cache) e no `chess-store` (Conexões); nunca na
//! Partida (ADR-0001).

mod browser;
pub mod model;
mod view;

use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::training_ui::with_store;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use chess_net::ProviderId;
use chess_store::{
    Connection, ConnectionId, ConnectionKind, NewConnection, ProfileId, Store, StoreError,
};
use model::CostFilter;
use std::collections::HashMap;
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
    Remove(ConnectionId),
}

/// Formulário de vincular, aberto num cartão.
#[derive(Clone, Debug, Default)]
pub(crate) struct LinkForm {
    pub identifier: String,
    pub nickname: String,
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
    persistence: Option<Res<'w, PersistenceHandle>>,
}

impl ConnectionsUi {
    /// Conexões de exemplo para as capturas (`--connections-demo`): sem banco em disco.
    pub fn demo() -> Self {
        let mut ui = Self::default();
        let samples = [
            NewConnection::new("lichess", ConnectionKind::PublicName, "DrNykterstein")
                .nickname("Principal"),
            NewConnection::new("lichess", ConnectionKind::PublicName, "Hikaru"),
            NewConnection::new("chesscom", ConnectionKind::PublicName, "MagnusCarlsen"),
            NewConnection::new("fide", ConnectionKind::PublicId, "1503014"),
        ];
        let _ = with_store(&mut ui.local, None, |store, profile| {
            for sample in &samples {
                let _ = store.add_connection(profile, sample);
            }
        });
        ui.dirty = true;
        ui
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
                Op::OpenDocs(provider) | Op::OpenTerms(provider) => {
                    // O endereço vem do catálogo, nunca da tela.
                    let info = provider.info();
                    let url = if matches!(op, Op::OpenDocs(_)) {
                        info.docs_url
                    } else {
                        info.terms_url
                    };
                    self.notes.remove(&provider);
                    if !browser::open(url) {
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
                Op::Remove(id) => {
                    let _ = with_store(&mut self.local, handle, |store, profile| {
                        store.remove_connection(profile, id)
                    });
                    self.dirty = true;
                }
            }
        }
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
        app.init_resource::<ConnectionsUi>();
    }
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
        ui.apply(vec![Op::SetDefault(second)], &service, None);
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::ChessCom)[0].id, second);
        ui.apply(vec![Op::Remove(second)], &service, None);
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
            None,
        );
        assert!(service.is_enabled(ProviderId::Lichess));
        ui.apply(
            vec![Op::SetEnabled(ProviderId::Lichess, false)],
            &service,
            None,
        );
        assert!(!service.is_enabled(ProviderId::Lichess));
        // Provedor sem serviço de rede não liga: a tela explica.
        ui.apply(vec![Op::SetEnabled(ProviderId::Twic, true)], &service, None);
        assert_eq!(
            ui.notes[&ProviderId::Twic],
            Note::plain("connections-switch-error")
        );
        ui.apply(vec![Op::ClearCache(ProviderId::Lichess)], &service, None);
        assert_eq!(
            ui.notes[&ProviderId::Lichess],
            Note::plain("connections-cache-cleared-none")
        );
    }

    #[test]
    fn the_demo_has_connections_for_the_captures() {
        let mut ui = ConnectionsUi::demo();
        ui.refresh(None);
        assert_eq!(ui.of_provider(ProviderId::Lichess).len(), 2);
        assert_eq!(ui.of_provider(ProviderId::Fide).len(), 1);
    }
}
