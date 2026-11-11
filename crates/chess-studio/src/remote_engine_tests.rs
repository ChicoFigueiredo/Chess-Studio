//! A Engine remota no app (#252): aviso, consentimento por Provedor, Trava de fair play e
//! problemas, juntos como a interface os usa. Servidor falso e relógio falso: nenhuma chamada
//! ao Chess-API.com real. As respostas são as gravadas do serviço (`chess-api-start`) e as
//! sintéticas (`chess-api-illegal-move`, `chess-api-rate-limited`, `chess-api-malformed`) do
//! `chess-providers`, marcadas como tais nos próprios arquivos.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::analysis::{Lock, Status};
use crate::fair_play::{Computation, FairPlay, GuardedAnalysis, Subject};
use crate::session::Session;
use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{FairPlayGuard, FakeClock, Net, NetConfig};
use chess_uci::PositionSpec;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

fn fixture(name: &str) -> FakeResponse {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../chess-providers/tests/fixtures")
        .join(name);
    FakeResponse::recorded(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

struct Env {
    server: FakeServer,
    service: NetService,
    guard: FairPlayGuard,
    fair: FairPlay,
    analysis: Analysis,
    session: Session,
}

fn env(provider_on: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .route("chess-api.com", &server.base_url());
    let net = Net::open(config).unwrap();
    net.set_enabled(PROVIDER, provider_on).unwrap();
    let service = NetService::from_net(net, clock, false);
    let fair = service.fair_play().clone();
    let guard = fair.guard().clone();
    let mut analysis = Analysis::default();
    analysis.set_default_engine(None);
    analysis.set_remote_settle(Duration::ZERO);
    analysis
        .remote_mut()
        .set_api(ChessApi::new(service.net().clone(), service.clock()));
    Env {
        server,
        service,
        guard,
        fair,
        analysis,
        session: Session::new(),
    }
}

impl Env {
    fn posts(&self) -> Vec<chess_net::testing::ReceivedRequest> {
        self.server
            .requests()
            .into_iter()
            .filter(|r| r.method == "POST" && r.path == "/v1")
            .collect()
    }

    fn drive(&mut self) {
        self.analysis.drive(&self.session, false, &self.fair);
    }

    /// Dirige a análise até `done` ou 10 s.
    fn until(&mut self, mut done: impl FnMut(&Analysis) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            self.drive();
            if done(&self.analysis) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// O usuário escolhe a Engine remota e confirma o aviso; devolve o estado do fluxo.
    fn choose(&mut self, flow: &mut RemoteFlow, dialog: &mut ConsentDialog) -> Stage {
        flow.request();
        assert_eq!(flow.stage(), Stage::Warning);
        flow.confirm(&self.service, dialog, &mut self.analysis);
        flow.stage()
    }
}

#[test]
fn the_remote_engine_is_never_the_default_and_a_local_choice_drops_it() {
    let mut analysis = Analysis::default();
    assert!(!analysis.remote().is_selected());
    assert!(!analysis.engine_available() || analysis.engine_label().is_some());
    analysis.choose_remote();
    assert!(analysis.remote().is_selected());
    assert_eq!(
        analysis.engine_label().as_deref(),
        Some("Stockfish (Chess-API.com)")
    );
    // Escolher qualquer Engine local (ou a padrão) tira a remota.
    analysis.set_engine(None);
    assert!(!analysis.remote().is_selected());
}

#[test]
fn choosing_with_the_provider_off_asks_for_consent_and_sends_nothing() {
    let mut env = env(false);
    let mut flow = RemoteFlow::default();
    let mut dialog = ConsentDialog::default();

    assert_eq!(env.choose(&mut flow, &mut dialog), Stage::Consent);
    assert_eq!(dialog.current(), Some(PROVIDER));
    assert!(!env.analysis.remote().is_selected());

    // "Agora não": nada muda, nada sai.
    flow.answered(false, &mut env.analysis);
    assert_eq!(flow.stage(), Stage::Idle);
    assert!(!env.analysis.remote().is_selected());
    env.analysis.set_enabled(true);
    env.drive();
    assert!(env.posts().is_empty());
    assert!(!env.service.is_enabled(PROVIDER));
}

#[test]
fn accepting_the_consent_selects_the_engine_for_this_provider_only() {
    let mut env = env(false);
    let mut flow = RemoteFlow::default();
    let mut dialog = ConsentDialog::default();
    assert_eq!(env.choose(&mut flow, &mut dialog), Stage::Consent);

    // O diálogo liga o Provedor (o consentimento é por Provedor) e responde.
    env.service.set_enabled(PROVIDER, true).unwrap();
    flow.answered(true, &mut env.analysis);
    assert!(env.analysis.remote().is_selected());
    assert!(env.service.is_enabled(PROVIDER));
    // Outro Provedor segue desligado.
    assert!(!env.service.is_enabled(ProviderId::Lichess));
    // Uma resposta fora de hora (sem aviso aberto) não escolhe nada.
    env.analysis.remote_mut().deselect();
    flow.answered(true, &mut env.analysis);
    assert!(!env.analysis.remote().is_selected());
}

#[test]
fn with_the_provider_already_on_the_warning_still_comes_first() {
    let mut env = env(true);
    let mut flow = RemoteFlow::default();
    flow.request();
    // Antes do "Continuar" nada foi escolhido nem enviado.
    assert_eq!(flow.stage(), Stage::Warning);
    assert!(!env.analysis.remote().is_selected());
    env.analysis.set_enabled(true);
    env.drive();
    assert!(env.posts().is_empty());
    // Cancelar volta ao estado anterior.
    flow.cancel();
    assert_eq!(flow.stage(), Stage::Idle);
    assert!(!env.analysis.remote().is_selected());

    let mut dialog = ConsentDialog::default();
    assert_eq!(env.choose(&mut flow, &mut dialog), Stage::Idle);
    assert!(env.analysis.remote().is_selected());
    assert_eq!(dialog.current(), None);
}

#[test]
fn the_remote_answer_shows_like_any_engine_and_only_the_fen_leaves() {
    let mut env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);

    assert!(env.until(|a| a.shown().is_some()), "sem resposta");
    let shown = env.analysis.shown().unwrap();
    let line = &shown.update.lines[0];
    assert_eq!(line.pv[0].to_string(), "d2d4");
    assert_eq!(env.analysis.status(), &Status::Running);

    let posts = env.posts();
    assert_eq!(posts.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&posts[0].body).unwrap();
    let keys: std::collections::BTreeSet<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["depth", "fen", "variants"].into_iter().collect(),
        "só FEN, profundidade e variantes"
    );
}

#[test]
fn turning_the_provider_off_afterwards_sends_nothing_and_offers_to_turn_it_on() {
    let mut env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    env.analysis.choose_remote();
    env.service.set_enabled(PROVIDER, false).unwrap();
    env.analysis.set_enabled(true);

    assert!(env.until(|a| matches!(a.status(), Status::Failed(_))));
    assert!(env.posts().is_empty(), "Provedor desligado: nada sai");
    let problem = env.analysis.remote().problem().cloned().unwrap();
    let (text, remedy) = problem_text(&I18n::pt_br(), &problem, T0);
    assert_eq!(remedy, Remedy::TurnOn);
    assert!(text.contains("desligado") && text.contains("nada foi enviado"));
    assert!(env.analysis.shown().is_none());
}

#[test]
fn the_fair_play_lock_refuses_before_any_request() {
    let mut env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);
    env.guard.engage("Partida online contra pessoa");

    env.drive();
    env.drive();
    assert_eq!(env.analysis.status(), &Status::Locked(Lock::FairPlay));
    assert!(!env.analysis.has_engine());
    assert!(env.posts().is_empty(), "trava ligada: nada sai");
    let (text, remedy) = problem_text(
        &I18n::pt_br(),
        &chess_providers::ProviderError::FairPlayLocked { provider: PROVIDER },
        T0,
    );
    assert_eq!(remedy, Remedy::Locked);
    assert!(
        text.contains("Trava de fair play") && text.to_lowercase().contains("nada foi enviado")
    );
}

#[test]
fn the_lock_turning_on_after_an_answer_stops_further_requests() {
    let mut env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);
    assert!(env.until(|a| a.shown().is_some()));
    assert_eq!(env.posts().len(), 1);

    env.guard.engage("Partida online contra pessoa");
    env.analysis.set_enabled(true);
    env.drive();
    assert_eq!(env.analysis.status(), &Status::Locked(Lock::FairPlay));
    env.drive();
    assert_eq!(env.posts().len(), 1, "nenhum pedido novo sob a trava");
}

#[test]
fn the_guarded_remote_engine_checks_the_lock_on_every_request() {
    let env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    let api = ChessApi::new(env.service.net().clone(), env.service.clock());
    let subject = Subject::position(&chess_core::CanonicalPosition::start());
    let mut remote = GuardedAnalysis::start_remote(&env.fair, &subject, api.clone()).unwrap();
    assert!(remote.is_remote());

    env.guard.engage("Partida online contra pessoa");
    let refused = remote
        .analyze(&env.fair, &subject, PositionSpec::startpos(), 1)
        .unwrap_err();
    assert_eq!(refused.computation, Computation::RemoteEngine);
    assert!(env.posts().is_empty());
    // Com a trava ligada a Engine remota nem sobe.
    assert!(GuardedAnalysis::start_remote(&env.fair, &subject, api).is_err());
}

#[test]
fn an_illegal_move_from_the_service_is_never_shown() {
    let mut env = env(true);
    env.server
        .route("/v1", fixture("chess-api-illegal-move.http"));
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);

    assert!(env.until(|a| matches!(a.status(), Status::Failed(_))));
    assert!(
        env.analysis.shown().is_none(),
        "nada do Lance ilegal na tela"
    );
    let problem = env.analysis.remote().problem().unwrap();
    assert!(matches!(
        problem,
        chess_providers::ProviderError::Malformed { .. }
    ));
    let (text, _) = problem_text(&I18n::pt_br(), problem, T0);
    assert!(text.contains("nada foi mostrado"));
}

#[test]
fn the_provider_pause_becomes_a_wait_in_seconds() {
    let mut env = env(true);
    env.server
        .route("/v1", fixture("chess-api-rate-limited.http"));
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);

    assert!(env.until(|a| matches!(a.status(), Status::Failed(_))));
    let problem = env.analysis.remote().problem().cloned().unwrap();
    let chess_providers::ProviderError::RateLimited { until, .. } = &problem else {
        panic!("esperava RateLimited, veio {problem:?}");
    };
    let (text, remedy) = problem_text(
        &I18n::pt_br(),
        &problem,
        Timestamp::from_millis(until.as_millis().saturating_sub(42_000)),
    );
    assert_eq!(remedy, Remedy::Wait);
    assert!(text.contains("42 s"), "{text}");
    // Dentro do prazo o app não insiste: um pedido só.
    assert_eq!(env.posts().len(), 1);
}

#[test]
fn the_remote_answer_records_the_instant_and_the_origin_names_provider_time_and_terms() {
    let mut env = env(true);
    env.server.route("/v1", fixture("chess-api-start.http"));
    assert!(env.analysis.remote().answered_at().is_none());
    env.analysis.choose_remote();
    env.analysis.set_enabled(true);
    assert!(env.until(|a| a.shown().is_some()), "sem resposta");
    let at = env
        .analysis
        .remote()
        .answered_at()
        .expect("instante da resposta");

    let i18n = I18n::pt_br();
    let later = Timestamp::from_millis(at.as_millis() + 5 * 60_000);
    let text = origin_text(&i18n, Some(at), later);
    assert!(text.contains("Chess-API.com"), "{text}");
    assert!(text.contains("5 min"), "{text}");
    assert!(text.contains("termos do Chess-API.com"), "{text}");
    // Sem resposta ainda: o Provedor e os termos aparecem, sem inventar instante.
    let pending = origin_text(&i18n, None, later);
    assert!(
        pending.contains("Chess-API.com") && pending.contains("termos"),
        "{pending}"
    );
    assert!(!pending.contains("min"), "{pending}");
    // Trocar a Engine esquece o instante.
    env.analysis.set_engine(None);
    assert!(env.analysis.remote().answered_at().is_none());
}

#[test]
fn the_local_engine_controls_do_not_apply_to_the_remote_engine() {
    let mut analysis = Analysis::default();
    assert!(analysis.shows_local_engine_controls());
    analysis.choose_remote();
    assert!(!analysis.shows_local_engine_controls());
    analysis.set_engine(None);
    assert!(analysis.shows_local_engine_controls());
}

#[test]
fn every_remote_text_exists_in_pt_br_and_stays_in_user_language() {
    let i18n = I18n::pt_br();
    let keys = [
        "remote-engine-entry",
        "remote-badge",
        "remote-warning-title",
        "remote-warning-sends-label",
        "remote-warning-sends",
        "remote-warning-footer",
        "remote-warning-continue",
        "remote-warning-cancel",
        "remote-act-turn-on",
        "remote-origin",
        "remote-origin-pending",
        "remote-problem-disabled",
        "remote-problem-fairplay",
        "remote-problem-invalid",
        "remote-problem-offline",
        "remote-problem-generic",
        "analysis-locked-fair-play",
    ];
    let mut all = String::new();
    for key in keys {
        let text = i18n.t(key);
        assert_ne!(text, key, "{key}");
        all.push_str(&text);
    }
    all.push_str(&i18n.t_with("remote-warning-lead", &[("provider", "Chess-API.com")]));
    all.push_str(&i18n.t_with("remote-problem-limit", &[("seconds", "5")]));
    let all = all.to_lowercase();
    for banned in ["http", "json", "timeout", "429", "endpoint", "api key"] {
        assert!(!all.contains(banned), "{banned}");
    }
    // O aviso diz que a Posição sai da máquina e para quem.
    let lead = i18n.t_with("remote-warning-lead", &[("provider", "Chess-API.com")]);
    assert!(lead.contains("sai da sua máquina") && lead.contains("Chess-API.com"));
}
