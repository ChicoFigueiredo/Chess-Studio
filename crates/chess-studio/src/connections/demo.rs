//! Cenas de demonstração da Central para as capturas (`--connections-auth <cena>`). Só existem
//! com `--screenshot`: o Cofre é sempre o **falso** (cofre do sistema em memória, diretório
//! temporário apagado ao sair), nenhum pedido sai e os nomes são fictícios. Nunca abrem o cofre
//! real do usuário.
//!
//! Cenas: `authorized`, `waiting`, `expired`, `locked`, `refused`, `timeout`, `wrong-key`,
//! `key-form`, `engine`, `vault-banner` (sem cofre do sistema), `vault-create`, `vault-unlock`,
//! `vault-change`, `vault-working` e `disconnect`.

use super::auth::{Ending, Opener, Slot, VaultDialog, VaultMode};
use super::{ConnectionsUi, Note};
use crate::net_service::NetService;
use crate::provider_consent::ConsentDialog;
use crate::secrets::{self, OpenedVault, VolatileKeyring, net_id, net_profile};
use crate::training_ui::with_store;
use chess_net::{AuthorizationSource, ProviderId, Secret};
use chess_store::{ConnectionKind, ConnectionRole};

/// A senha-mestra das cenas com cofre próprio. Fictícia, só existe no cofre temporário.
const DEMO_PASSWORD: &str = "senha-de-demonstracao";

/// Uma cena pronta: a Central e o serviço de rede com o Cofre falso que ela usa.
pub struct Scene {
    pub ui: ConnectionsUi,
    pub service: NetService,
}

impl Opener {
    /// Não abre nada: as capturas e a demonstração nunca lançam o navegador.
    fn silent() -> Self {
        Self::new(|_, _| true)
    }
}

fn base(vault: OpenedVault) -> Scene {
    let service = NetService::in_memory_with(vault);
    for provider in [ProviderId::Lichess, ProviderId::ChessCom] {
        let _ = service.set_enabled(provider, true);
    }
    let mut ui = ConnectionsUi::demo();
    ui.opener = Opener::silent();
    ui.expanded.insert(ProviderId::Lichess);
    ui.sync(&service, None);
    Scene { ui, service }
}

/// Uma Conexão do Lichess já autorizada, com o segredo no Cofre falso, como se a pessoa tivesse
/// autorizado `minutes` minutos atrás.
fn authorize(
    scene: &mut Scene,
    role: ConnectionRole,
    kind: ConnectionKind,
    name: &str,
    scopes: &[&str],
    minutes: i64,
) -> chess_store::ConnectionId {
    let scopes: Vec<String> = scopes.iter().map(|s| (*s).to_owned()).collect();
    let at = NetService::now().as_millis() - minutes * 60_000;
    let vault = scene.service.vault().clone();
    let id = with_store(&mut scene.ui.local, None, |store, profile| {
        let id = store
            .begin_authorization(profile, "lichess", role, kind)
            .expect("reserva");
        vault
            .store(
                net_profile(profile),
                ProviderId::Lichess,
                net_id(id),
                Secret::new("segredo-de-demonstracao"),
            )
            .expect("cofre falso");
        store
            .record_authorization(profile, id, kind, name, &scopes, at)
            .expect("registra");
        id
    })
    .expect("banco em memória");
    scene.ui.dirty = true;
    scene.ui.sync(&scene.service, None);
    id
}

fn own_vault() -> OpenedVault {
    let opened = secrets::volatile_vault(VolatileKeyring::missing(), "demo-cofre-proprio");
    opened
        .vault
        .create_own(&Secret::new(DEMO_PASSWORD))
        .expect("cofre próprio da demonstração");
    opened
}

fn authorized(scene: &mut Scene) -> chess_store::ConnectionId {
    authorize(
        scene,
        ConnectionRole::Player,
        ConnectionKind::AuthorizedBrowser,
        "ana_exemplo",
        &["board:play"],
        3,
    )
}

/// Monta a cena `name`; `None` se o nome não existe.
pub fn scene(name: &str) -> Option<Scene> {
    let system = || secrets::volatile_vault(VolatileKeyring::new(), "demo-cofre");
    let mut scene = match name {
        "authorized" => {
            let mut scene = base(system());
            authorized(&mut scene);
            scene
        }
        "waiting" => {
            let mut scene = base(system());
            let mut dialog = ConsentDialog::default();
            scene.ui.authorize(
                Slot::player(ProviderId::Lichess),
                None,
                &scene.service,
                &mut dialog,
                None,
            );
            scene
        }
        "expired" => {
            let mut scene = base(system());
            let id = authorized(&mut scene);
            let profile = scene.ui.loaded_for.map_or(1, |p| p.0);
            scene.service.vault().rejected(
                chess_net::LocalProfileId(profile),
                ProviderId::Lichess,
                net_id(id),
                401,
            );
            scene.ui.sync(&scene.service, None);
            scene
        }
        "locked" => {
            let mut scene = base(own_vault());
            authorized(&mut scene);
            scene.service.vault().lock();
            scene.ui.sync(&scene.service, None);
            scene
        }
        "refused" => ending(Ending::Refused),
        "timeout" => ending(Ending::TimedOut),
        "wrong-key" => {
            let mut scene = base(system());
            scene.ui.auth.endings.insert(
                Slot::player(ProviderId::Lichess),
                Ending::Problem(Note::plain("auth-problem-wrong-player")),
            );
            scene.ui.auth.last.insert(
                Slot::player(ProviderId::Lichess),
                super::auth::Intent::Key {
                    slot: Slot::player(ProviderId::Lichess),
                    renew: None,
                },
            );
            scene
        }
        "key-form" => {
            let mut scene = base(system());
            scene
                .ui
                .open_key_form(Slot::player(ProviderId::Lichess), None);
            scene
        }
        "engine" => {
            let mut scene = base(system());
            authorized(&mut scene);
            scene.ui.auth.engine_open.insert(ProviderId::Lichess);
            scene
        }
        "disconnect" => {
            let mut scene = base(system());
            let id = authorized(&mut scene);
            scene.ui.auth.confirm_disconnect = Some(id);
            scene
        }
        "vault-banner" => base(secrets::volatile_vault(
            VolatileKeyring::missing(),
            "demo-sem-cofre",
        )),
        "vault-create" => {
            let mut scene = base(secrets::volatile_vault(
                VolatileKeyring::missing(),
                "demo-sem-cofre",
            ));
            scene.ui.auth.vault_dialog = Some(VaultDialog::new(VaultMode::Create));
            scene
        }
        "vault-unlock" => {
            let mut scene = base(own_vault());
            scene.service.vault().lock();
            scene.ui.auth.vault_dialog = Some(VaultDialog::new(VaultMode::Unlock));
            scene
        }
        "vault-change" => {
            let mut scene = base(own_vault());
            scene.ui.auth.vault_dialog = Some(VaultDialog::new(VaultMode::Change));
            scene
        }
        "vault-working" => {
            let mut scene = base(secrets::volatile_vault(
                VolatileKeyring::missing(),
                "demo-sem-cofre",
            ));
            let mut dialog = VaultDialog::new(VaultMode::Create);
            dialog.set_busy_for_demo();
            scene.ui.auth.vault_dialog = Some(dialog);
            scene
        }
        _ => return None,
    };
    scene.ui.sync(&scene.service, None);
    Some(scene)
}

fn ending(ending: Ending) -> Scene {
    let mut scene = base(secrets::volatile_vault(
        VolatileKeyring::new(),
        "demo-cofre",
    ));
    scene
        .ui
        .auth
        .endings
        .insert(Slot::player(ProviderId::Lichess), ending);
    scene
}
