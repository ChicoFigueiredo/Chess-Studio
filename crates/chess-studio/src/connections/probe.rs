//! "Testar" uma Conexão por nome ou ID público (#214): confirma que o nome existe no Provedor
//! usando os adaptadores de `chess-providers` (Lichess e Chess.com por nome; FIDE por ID, com o
//! dado vindo do **Lichess**, que é o Provedor a ligar e a Origem do resultado).
//!
//! Nunca bloqueia a interface: o teste é uma [`Query`] consultada a cada quadro (`try_take`).
//! Se o Provedor está desligado, o diálogo "Ligar <Provedor>?" pergunta antes; a resposta
//! (`ProviderConsentAnswered`) destrava o teste.

use super::model::test_via;
use crate::net_service::NetService;
use crate::provider_consent::ProviderConsentAnswered;
use chess_net::{ProviderId, Timestamp};
use chess_providers::{Answer, ChessComProfile, FideRating, LichessProfile, ProviderError, Query};
use std::sync::Mutex;

/// O que se pede ao Provedor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Name(String),
    FideId(String),
}

/// Resultado do teste, pronto para a tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// O nome (ou ID) existe; o texto é o nome que o Provedor devolveu.
    Confirmed(String),
    /// "Nome não encontrado" (ou ID inexistente).
    NotFound,
    /// O Provedor pediu uma pausa: limitado até o instante.
    Limited(Timestamp),
    /// O ID FIDE não é um número.
    InvalidId,
    /// Falha: id Fluent da frase curta.
    Failed(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub outcome: Outcome,
    /// O Provedor que respondeu (a Origem): o Lichess, no teste do FIDE.
    pub source: ProviderId,
    pub at: Timestamp,
}

/// Estado do teste de um cartão.
pub(crate) enum TestState {
    /// Esperando a resposta ao diálogo "Ligar <Provedor>?".
    AwaitingConsent {
        card: ProviderId,
        target: Target,
    },
    /// `Query` não é `Sync`: o `Mutex` deixa o estado viver num recurso do Bevy.
    Running {
        card: ProviderId,
        query: Box<Mutex<Running>>,
    },
    Done(Report),
}

impl std::fmt::Debug for TestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AwaitingConsent { .. } => f.write_str("AwaitingConsent"),
            Self::Running { .. } => f.write_str("Running"),
            Self::Done(report) => write!(f, "Done({report:?})"),
        }
    }
}

/// A consulta em andamento (cada adaptador devolve um tipo).
pub(crate) enum Running {
    Lichess(Query<LichessProfile>),
    ChessCom(Query<ChessComProfile>),
    Fide(Query<FideRating>),
}

/// O pedido do teste para o cartão `card`, ou o resultado imediato se o ID é inválido.
pub(crate) fn start(
    service: &NetService,
    card: ProviderId,
    target: &Target,
    now: Timestamp,
) -> Result<Running, Report> {
    match (card, target) {
        (ProviderId::Lichess, Target::Name(name)) => {
            Ok(Running::Lichess(service.lichess().profile(name)))
        }
        (ProviderId::ChessCom, Target::Name(name)) => {
            Ok(Running::ChessCom(service.chesscom().profile(name)))
        }
        (ProviderId::Fide, Target::FideId(id)) => match id.trim().parse::<u64>() {
            Ok(id) => Ok(Running::Fide(service.lichess().fide_player(id))),
            Err(_) => Err(Report {
                outcome: Outcome::InvalidId,
                source: test_via(card),
                at: now,
            }),
        },
        // Combinação que a tela não monta: trata como falha genérica.
        _ => Err(Report {
            outcome: Outcome::Failed("connections-err-other"),
            source: test_via(card),
            at: now,
        }),
    }
}

impl Running {
    /// O resultado, se já chegou (nunca bloqueia).
    pub(crate) fn poll(&mut self, card: ProviderId, now: Timestamp) -> Option<Report> {
        let via = test_via(card);
        match self {
            Self::Lichess(q) => q
                .try_take()
                .map(|r| report(r, via, now, |p| p.username.clone())),
            Self::ChessCom(q) => q
                .try_take()
                .map(|r| report(r, via, now, |p| p.username.clone())),
            Self::Fide(q) => q
                .try_take()
                .map(|r| report(r, via, now, |p| p.name.clone())),
        }
    }
}

/// Traduz a resposta do adaptador: nome confirmado, não encontrado, limitado ou falha.
pub(crate) fn report<T>(
    result: Result<Answer<T>, ProviderError>,
    via: ProviderId,
    now: Timestamp,
    name_of: impl FnOnce(&T) -> String,
) -> Report {
    match result {
        Ok(answer) => Report {
            outcome: Outcome::Confirmed(name_of(&answer.value)),
            source: answer.provenance.provider,
            at: now,
        },
        Err(error) => Report {
            outcome: outcome_of(&error),
            source: via,
            at: now,
        },
    }
}

fn outcome_of(error: &ProviderError) -> Outcome {
    match error {
        ProviderError::UnknownName { .. } | ProviderError::NoData => Outcome::NotFound,
        ProviderError::RateLimited { until, .. } => Outcome::Limited(*until),
        ProviderError::InvalidInput { .. } => Outcome::InvalidId,
        ProviderError::Net(e) => Outcome::Failed(super::model::error_key(e)),
        ProviderError::Malformed { .. } => Outcome::Failed("connections-err-malformed"),
        _ => Outcome::Failed("connections-err-other"),
    }
}

/// Aplica as respostas ao diálogo e colhe os testes que terminaram.
pub(crate) fn advance(
    tests: &mut std::collections::HashMap<ProviderId, TestState>,
    service: &NetService,
    answers: &[ProviderConsentAnswered],
    now: Timestamp,
) {
    for state in tests.values_mut() {
        match state {
            TestState::AwaitingConsent { card: c, target } => {
                let via = test_via(*c);
                let Some(answer) = answers.iter().find(|a| a.provider == via) else {
                    continue;
                };
                *state = if answer.accepted {
                    match start(service, *c, target, now) {
                        Ok(query) => TestState::Running {
                            card: *c,
                            query: Box::new(Mutex::new(query)),
                        },
                        Err(report) => TestState::Done(report),
                    }
                } else {
                    // "Agora não": o teste não roda; some da tela.
                    TestState::Done(Report {
                        outcome: Outcome::Failed("connections-test-not-turned-on"),
                        source: via,
                        at: now,
                    })
                };
            }
            TestState::Running { card: c, query } => {
                if let Some(done) = query.get_mut().ok().and_then(|q| q.poll(*c, now)) {
                    *state = TestState::Done(done);
                }
            }
            TestState::Done(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::{ConnectionsUi, Op};
    use crate::provider_consent::ConsentDialog;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig, RetryPolicy};
    use chess_store::ConnectionKind;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// 2026-11-11T12:00:00Z.
    const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

    struct Env {
        server: FakeServer,
        service: NetService,
        ui: ConnectionsUi,
        dialog: ConsentDialog,
    }

    /// Rede em memória com os hosts dos Provedores apontando para o servidor falso: a suíte
    /// nunca chama serviço real (CX-17).
    fn env() -> Env {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let mut config = NetConfig::in_memory()
            .clock(clock.clone())
            .jitter_seed(7)
            .retry(RetryPolicy {
                max_retries: 0,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(1),
            });
        for host in ["lichess.org", "api.chess.com"] {
            config = config.route(host, &server.base_url());
        }
        let net = Net::open(config).expect("abre a rede");
        Env {
            server,
            service: NetService::from_net(net, clock, false),
            ui: ConnectionsUi::default(),
            dialog: ConsentDialog::default(),
        }
    }

    fn fixture(name: &str) -> FakeResponse {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../chess-providers/tests/fixtures")
            .join(name);
        FakeResponse::recorded(&path).expect("fixture gravada")
    }

    impl Env {
        fn link(&mut self, provider: ProviderId, kind: ConnectionKind, identifier: &str) {
            self.ui.forms.entry(provider).or_default();
            self.ui.apply(
                vec![Op::Link {
                    provider,
                    kind,
                    identifier: identifier.into(),
                    nickname: String::new(),
                }],
                &self.service,
                &mut self.dialog,
                None,
            );
            self.ui.refresh(None);
        }

        fn test(&mut self, provider: ProviderId) {
            self.ui.apply(
                vec![Op::Test {
                    provider,
                    feature: "O teste da Conexão".into(),
                }],
                &self.service,
                &mut self.dialog,
                None,
            );
        }

        /// Consulta a cada "quadro" até o teste terminar (nunca bloqueia: só `try_take`).
        fn finish(&mut self, provider: ProviderId, answers: &[ProviderConsentAnswered]) -> Report {
            let start = Instant::now();
            let mut answers = answers.to_vec();
            while start.elapsed() < Duration::from_secs(5) {
                self.ui.poll(&self.service, &answers);
                answers.clear();
                if let Some(TestState::Done(report)) = self.ui.tests.get(&provider) {
                    return report.clone();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("o teste não terminou: {:?}", self.ui.tests.get(&provider));
        }
    }

    #[test]
    fn lichess_name_confirmed_when_the_provider_is_on() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/user/lichess", fixture("lichess-user-profile.http"));
        env.link(ProviderId::Lichess, ConnectionKind::PublicName, "lichess");
        env.test(ProviderId::Lichess);
        assert!(matches!(
            env.ui.tests.get(&ProviderId::Lichess),
            Some(TestState::Running { .. })
        ));
        assert_eq!(env.dialog.current(), None, "ligado: sem diálogo");
        let report = env.finish(ProviderId::Lichess, &[]);
        assert_eq!(report.outcome, Outcome::Confirmed("Lichess".into()));
        assert_eq!(report.source, ProviderId::Lichess);
    }

    #[test]
    fn chesscom_name_confirmed() {
        let mut env = env();
        env.service.set_enabled(ProviderId::ChessCom, true).unwrap();
        env.server
            .route("/pub/player/erik", fixture("chesscom-profile.http"));
        env.link(ProviderId::ChessCom, ConnectionKind::PublicName, "Erik");
        env.test(ProviderId::ChessCom);
        let report = env.finish(ProviderId::ChessCom, &[]);
        assert_eq!(report.outcome, Outcome::Confirmed("erik".into()));
        assert_eq!(report.source, ProviderId::ChessCom);
    }

    #[test]
    fn a_provider_that_is_off_asks_first_and_sends_nothing_until_the_answer() {
        let mut env = env();
        env.server
            .route("/api/user/lichess", fixture("lichess-user-profile.http"));
        env.link(ProviderId::Lichess, ConnectionKind::PublicName, "lichess");
        env.test(ProviderId::Lichess);
        assert_eq!(env.dialog.current(), Some(ProviderId::Lichess));
        assert!(matches!(
            env.ui.tests.get(&ProviderId::Lichess),
            Some(TestState::AwaitingConsent { .. })
        ));
        env.ui.poll(&env.service, &[]);
        assert!(env.server.requests().is_empty(), "nada sai antes do aceite");
        // A pessoa liga no diálogo.
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        let answer = ProviderConsentAnswered {
            provider: ProviderId::Lichess,
            accepted: true,
        };
        let report = env.finish(ProviderId::Lichess, &[answer]);
        assert_eq!(report.outcome, Outcome::Confirmed("Lichess".into()));
    }

    #[test]
    fn declining_the_dialog_does_not_run_the_test() {
        let mut env = env();
        env.link(ProviderId::ChessCom, ConnectionKind::PublicName, "erik");
        env.test(ProviderId::ChessCom);
        let answer = ProviderConsentAnswered {
            provider: ProviderId::ChessCom,
            accepted: false,
        };
        let report = env.finish(ProviderId::ChessCom, &[answer]);
        assert_eq!(
            report.outcome,
            Outcome::Failed("connections-test-not-turned-on")
        );
        assert!(env.server.requests().is_empty());
        assert!(!env.service.is_enabled(ProviderId::ChessCom));
    }

    #[test]
    fn an_unknown_name_is_reported_as_not_found() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server
            .route("/api/user/fantasma", FakeResponse::status(404));
        env.link(ProviderId::Lichess, ConnectionKind::PublicName, "fantasma");
        env.test(ProviderId::Lichess);
        assert_eq!(
            env.finish(ProviderId::Lichess, &[]).outcome,
            Outcome::NotFound
        );
    }

    #[test]
    fn a_rate_limited_provider_reports_limited_until() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.server.route(
            "/api/user/lichess",
            FakeResponse::status(429).header("retry-after", "60"),
        );
        env.link(ProviderId::Lichess, ConnectionKind::PublicName, "lichess");
        env.test(ProviderId::Lichess);
        let report = env.finish(ProviderId::Lichess, &[]);
        let Outcome::Limited(until) = report.outcome else {
            panic!("esperava Limited, veio {:?}", report.outcome);
        };
        assert_eq!(until, T0.plus(Duration::from_secs(60)));
    }

    #[test]
    fn a_server_error_is_a_failure_with_a_short_reason() {
        let mut env = env();
        env.service.set_enabled(ProviderId::ChessCom, true).unwrap();
        env.server
            .route("/pub/player/erik", FakeResponse::status(500));
        env.link(ProviderId::ChessCom, ConnectionKind::PublicName, "erik");
        env.test(ProviderId::ChessCom);
        assert_eq!(
            env.finish(ProviderId::ChessCom, &[]).outcome,
            Outcome::Failed("connections-err-http")
        );
    }

    #[test]
    fn the_fide_id_is_checked_through_lichess_and_the_origin_says_so() {
        let mut env = env();
        env.server.route(
            "/api/fide/player/1503014",
            fixture("lichess-fide-player.http"),
        );
        env.link(ProviderId::Fide, ConnectionKind::PublicId, "1503014");
        env.test(ProviderId::Fide);
        // O Provedor a ligar é o Lichess, não o FIDE.
        assert_eq!(env.dialog.current(), Some(ProviderId::Lichess));
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        let answer = ProviderConsentAnswered {
            provider: ProviderId::Lichess,
            accepted: true,
        };
        let report = env.finish(ProviderId::Fide, &[answer]);
        assert_eq!(report.outcome, Outcome::Confirmed("Carlsen, Magnus".into()));
        assert_eq!(report.source, ProviderId::Lichess);
        assert!(!env.service.is_enabled(ProviderId::Fide));
    }

    #[test]
    fn an_invalid_fide_id_is_refused_before_any_request() {
        let mut env = env();
        env.service.set_enabled(ProviderId::Lichess, true).unwrap();
        env.link(ProviderId::Fide, ConnectionKind::PublicId, "abc");
        env.test(ProviderId::Fide);
        let report = env.finish(ProviderId::Fide, &[]);
        assert_eq!(report.outcome, Outcome::InvalidId);
        assert!(env.server.requests().is_empty());
    }

    #[test]
    fn testing_without_a_linked_connection_does_nothing() {
        let mut env = env();
        env.test(ProviderId::Lichess);
        assert!(env.ui.tests.is_empty());
        assert_eq!(env.dialog.current(), None);
    }
}
