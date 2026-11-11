//! O rating FIDE no cartão da FIDE da Central, e a ponte entre a tela e o Perfil local (CX-19,
//! #242). O desenho e a consulta vivem em [`crate::fide_profile`]; aqui ficam só o encaixe no
//! cartão e a criação da Conexão quando a pessoa vincula um jogador achado na busca.

use super::{ConnectionsUi, Op, View};
use crate::fide_profile::{self, Env, FideAction, Look};
use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::provider_consent::{ConsentDialog, ProviderConsentAnswered};
use crate::training_ui::with_store;
use bevy_egui::egui::Ui;
use chess_net::ProviderId;
use chess_store::{ConnectionId, ConnectionKind, MAX_NICKNAME_LEN, NewConnection, StoreError};

impl View<'_> {
    /// O bloco do rating, logo abaixo de "O que você pode fazer agora" do cartão da FIDE.
    pub(super) fn fide_block(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        let id = fide_profile::linked_id(self.list);
        let look = Look {
            ctx: self.ctx,
            i18n: self.i18n,
            now: self.now,
            lichess_on: self.service.is_enabled(ProviderId::Lichess),
            on_card: true,
            in_central: true,
        };
        let actions = fide_profile::rating_block(ui, &look, self.fide, id);
        self.ops.extend(actions.into_iter().map(Op::Fide));
        ui.add_space(8.0);
    }
}

impl ConnectionsUi {
    /// Executa um pedido do bloco do rating (consultar, buscar ou vincular).
    pub(in crate::connections) fn fide_action(
        &mut self,
        action: FideAction,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
    ) {
        if let FideAction::Link { id, name } = action {
            self.link_fide(id, &name, handle);
            return;
        }
        let mut env = Env {
            service,
            dialog,
            handle,
            connections: &self.list,
        };
        self.fide.perform(action, &mut env);
    }

    /// A cada quadro, com ou sem a página aberta: recarrega o valor guardado, responde ao
    /// diálogo do Lichess e colhe a consulta que terminou.
    pub(in crate::connections) fn fide_drive(
        &mut self,
        service: &NetService,
        dialog: &mut ConsentDialog,
        handle: Option<&PersistenceHandle>,
        answers: &[ProviderConsentAnswered],
    ) {
        let mut env = Env {
            service,
            dialog,
            handle,
            connections: &self.list,
        };
        self.fide.drive(&mut env, answers);
    }

    /// Vincula o ID achado na busca e o faz a Conexão padrão da FIDE: é o ID que a pessoa
    /// acabou de escolher.
    fn link_fide(&mut self, id: u64, name: &str, handle: Option<&PersistenceHandle>) {
        let nickname: String = name.chars().take(MAX_NICKNAME_LEN).collect();
        let new = NewConnection::new(
            ProviderId::Fide.key(),
            ConnectionKind::PublicId,
            id.to_string(),
        )
        .nickname(nickname);
        let done = with_store(
            &mut self.local,
            handle,
            |store, profile| -> Result<(), StoreError> {
                let connection = match store.add_connection(profile, &new) {
                    Ok(connection) => connection,
                    // Já vinculado: só vira o padrão.
                    Err(StoreError::ConnectionExists { .. }) => store
                        .list_connections(profile, Some(ProviderId::Fide.key()))?
                        .into_iter()
                        .find(|c| c.identifier == id.to_string())
                        .map(|c| c.id)
                        .ok_or(StoreError::ConnectionNotFound(ConnectionId(0)))?,
                    Err(error) => return Err(error),
                };
                store.set_default_connection(profile, connection)
            },
        );
        if matches!(done, Some(Ok(()))) {
            self.dirty = true;
            self.fide.hits.clear();
            self.fide.searched = false;
            self.fide.search_open = false;
            self.fide.search_text.clear();
        }
    }
}
