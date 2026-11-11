//! Abrir a Partida de referência exata (#239). Clicar numa "Partida de referência" do
//! Explorador abre aquela Partida, não uma busca pelo nome dos jogadores:
//!
//! 1. se ela já está na Biblioteca (pelo id do Provedor), só é aberta;
//! 2. senão é baixada pelo id (`Lichess::game_pgn` ou `masters_game_pgn`), gravada na
//!    Biblioteca com a Origem da consulta (Provedor, licença, endereço, instante) e aberta.
//!
//! O download não trava a interface: a consulta fica aqui e é lida a cada quadro por
//! [`drive`]. Esta parte só cuida do estado e da decisão; quem escreve na Biblioteca é o
//! `chess-store` (a Partida jogada nunca é tocada: ADR-0001).

use super::model::Source;
use chess_providers::{Answer, ExportedGame, Lichess, ProviderError, Query};
use chess_store::{ImportedId, ProfileId, Store};
use std::sync::{Mutex, PoisonError};

/// O Provedor das Partidas de referência, como a Biblioteca o grava.
const PROVIDER: &str = "lichess";

/// Como a linha da Partida de referência aparece.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    /// Baixando a Partida `id`.
    Opening(String),
    /// Não foi possível abrir a Partida `id`; `key` é o texto Fluent do motivo.
    Failed { id: String, key: &'static str },
}

struct Pending {
    id: String,
    /// Em `Mutex` só para o recurso ser `Sync`; nunca há disputa.
    query: Mutex<Query<ExportedGame>>,
}

#[derive(Default)]
pub struct ReferenceOpen {
    pending: Option<Pending>,
    failed: Option<(String, &'static str)>,
    /// Estado fixo das capturas de evidência (`--explorer-demo reference-*`).
    demo: Option<Status>,
}

impl std::fmt::Debug for ReferenceOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReferenceOpen")
            .field("status", &self.status())
            .finish()
    }
}

/// O que fazer ao clicar numa Partida de referência.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Já está na Biblioteca: abrir.
    Open(ImportedId),
    /// Não está: baixar pelo id.
    Download,
}

/// Decide pelo que a Biblioteca já tem. Erro de leitura conta como "não tem": baixar é seguro
/// (a importação não duplica).
pub fn plan(store: &Store, profile: ProfileId, id: &str) -> Plan {
    match store.imported_by_remote_id(profile, PROVIDER, id) {
        Ok(Some(found)) => Plan::Open(found),
        _ => Plan::Download,
    }
}

/// Grava o PGN baixado na Biblioteca, com a Origem da consulta, e devolve o id da Partida
/// (nova ou a que já existia).
pub fn import(
    store: &mut Store,
    profile: ProfileId,
    answer: &Answer<ExportedGame>,
) -> Option<ImportedId> {
    let options = chess_providers::import_options(&answer.provenance);
    let report = store
        .import_pgn_with(profile, &answer.value.pgn, &options)
        .ok()?;
    report
        .imported
        .first()
        .copied()
        .or_else(|| match plan(store, profile, &answer.value.id) {
            Plan::Open(found) => Some(found),
            Plan::Download => None,
        })
}

/// A chave Fluent do motivo da falha, em palavras de usuário.
pub fn failure_key(error: &ProviderError) -> &'static str {
    match error {
        ProviderError::NoData => "explorer-reference-failed-gone",
        ProviderError::RateLimited { .. } => "explorer-reference-failed-limited",
        ProviderError::Disabled { .. } => "explorer-reference-failed-off",
        ProviderError::SignInRequired { .. } | ProviderError::ConnectionRejected { .. } => {
            "explorer-reference-failed-signin"
        }
        _ => "explorer-reference-failed",
    }
}

impl ReferenceOpen {
    /// Cena de evidência: mostra `status` sem pedir nada à rede.
    pub fn demo(&mut self, status: Status) {
        self.demo = Some(status);
    }

    pub fn status(&self) -> Status {
        if let Some(demo) = &self.demo {
            return demo.clone();
        }
        if let Some(pending) = &self.pending {
            return Status::Opening(pending.id.clone());
        }
        match &self.failed {
            Some((id, key)) => Status::Failed {
                id: id.clone(),
                key,
            },
            None => Status::Idle,
        }
    }

    /// Alguma Partida está sendo baixada (a interface pede quadros até terminar).
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Começa a baixar a Partida `id`. Um pedido por vez: clicar de novo enquanto baixa não
    /// faz nada.
    pub fn start(&mut self, id: &str, source: Source, lichess: &Lichess) {
        if self.pending.is_some() {
            return;
        }
        self.failed = None;
        let query = match source {
            Source::Masters => lichess.masters_game_pgn(id),
            _ => lichess.game_pgn(id),
        };
        self.pending = Some(Pending {
            id: id.to_owned(),
            query: Mutex::new(query),
        });
    }

    /// Lê o resultado se já chegou (nunca bloqueia). Falhas ficam em [`Status::Failed`].
    pub fn poll(&mut self) -> Option<Answer<ExportedGame>> {
        let pending = self.pending.as_ref()?;
        let taken = pending
            .query
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_take()?;
        let id = self.pending.take().map(|p| p.id).unwrap_or_default();
        match taken {
            Ok(answer) => Some(answer),
            Err(error) => {
                self.failed = Some((id, failure_key(&error)));
                None
            }
        }
    }

    /// A Partida chegou, mas a Biblioteca não conseguiu gravá-la.
    pub fn fail_import(&mut self, id: &str) {
        self.failed = Some((id.to_owned(), "explorer-reference-failed-save"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig, ProviderId, Timestamp};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const PGN: &str = "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/AbCd1234\"]\n[White \"ana\"]\n[Black \"beto\"]\n[Date \"2026.11.07\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n";
    const ME: ProfileId = ProfileId(1);

    struct Rig {
        server: FakeServer,
        lichess: Lichess,
    }

    fn rig() -> Rig {
        let clock = Arc::new(FakeClock::new(Timestamp::from_millis(1_794_398_400_000)));
        let server = FakeServer::with_clock(clock.clone());
        let config = NetConfig::in_memory()
            .clock(clock.clone())
            .route("lichess.org", &server.base_url());
        let net = Net::open(config).expect("rede");
        net.set_enabled(ProviderId::Lichess, true).expect("liga");
        Rig {
            lichess: Lichess::new(net, clock),
            server,
        }
    }

    fn wait(open: &mut ReferenceOpen) -> Option<Answer<ExportedGame>> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if let Some(answer) = open.poll() {
                return Some(answer);
            }
            if !open.is_busy() {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("a Partida não chegou");
    }

    #[test]
    fn a_game_not_in_the_library_is_downloaded_imported_and_then_found() {
        let rig = rig();
        rig.server.route("/game/export/AbCd1234", FakeResponse::ok(PGN));
        let mut store = Store::open_in_memory().expect("banco");
        assert_eq!(plan(&store, ME, "AbCd1234"), Plan::Download);

        let mut open = ReferenceOpen::default();
        open.start("AbCd1234", Source::Lichess, &rig.lichess);
        assert_eq!(open.status(), Status::Opening("AbCd1234".into()));
        let answer = wait(&mut open).expect("chegou");
        assert_eq!(open.status(), Status::Idle);

        let id = import(&mut store, ME, &answer).expect("gravou");
        assert_eq!(plan(&store, ME, "AbCd1234"), Plan::Open(id));
        let origins = store.origins(ME, id).expect("Origem");
        assert_eq!(origins[0].provider.as_deref(), Some("lichess"));
        assert_eq!(origins[0].remote_id.as_deref(), Some("AbCd1234"));
        assert!(
            origins[0]
                .reference_url
                .as_deref()
                .is_some_and(|u| u.contains("/game/export/AbCd1234"))
        );
    }

    #[test]
    fn a_game_already_in_the_library_is_opened_without_touching_the_network() {
        let rig = rig();
        rig.server.route("/game/export/AbCd1234", FakeResponse::ok(PGN));
        let mut store = Store::open_in_memory().expect("banco");
        let mut open = ReferenceOpen::default();
        open.start("AbCd1234", Source::Lichess, &rig.lichess);
        let answer = wait(&mut open).expect("chegou");
        let id = import(&mut store, ME, &answer).expect("gravou");
        let sent = rig.server.requests().len();

        // Segundo clique: a Biblioteca decide, e a rede não é chamada.
        assert_eq!(plan(&store, ME, "AbCd1234"), Plan::Open(id));
        assert_eq!(rig.server.requests().len(), sent);
        // E importar de novo o mesmo PGN devolve a mesma Partida, sem cópia.
        assert_eq!(import(&mut store, ME, &answer), Some(id));
    }

    #[test]
    fn a_failed_download_says_why_in_plain_words_and_can_be_retried() {
        let rig = rig();
        rig.server
            .route("/game/export/ZZZZ9999", FakeResponse::status(404));
        let mut open = ReferenceOpen::default();
        open.start("ZZZZ9999", Source::Lichess, &rig.lichess);
        assert!(wait(&mut open).is_none());
        assert_eq!(
            open.status(),
            Status::Failed {
                id: "ZZZZ9999".into(),
                key: "explorer-reference-failed-gone"
            }
        );
        // Tentar de novo limpa a falha e volta a baixar.
        rig.server.route("/game/export/ZZZZ9999", FakeResponse::ok(PGN));
        open.start("ZZZZ9999", Source::Lichess, &rig.lichess);
        assert_eq!(open.status(), Status::Opening("ZZZZ9999".into()));
    }

    #[test]
    fn only_one_download_runs_at_a_time() {
        let rig = rig();
        rig.server.route("/game/export/AbCd1234", FakeResponse::stall());
        let mut open = ReferenceOpen::default();
        open.start("AbCd1234", Source::Lichess, &rig.lichess);
        open.start("Other123", Source::Lichess, &rig.lichess);
        assert_eq!(open.status(), Status::Opening("AbCd1234".into()));
    }
}
