//! Autorização do Lichess contra o servidor falso (CX-17: nada de serviço real): fluxo PKCE
//! completo; `state` errado, código repetido, prazo, recusa e cancelamento tipados; uma única
//! requisição ao loopback; token pessoal com escopos conferidos; revogação que apaga o
//! segredo local mesmo com falha remota; e o token nunca em `Debug`, log, URL ou disco.

mod common;

use chess_net::testing::{FakeResponse, ReceivedRequest};
use chess_net::{ConnectionId, NetError, ProviderId, Secret};
use chess_providers::{
    AuthError, AuthFlow, AuthState, ConnectionRole, DEFAULT_DEADLINE, LichessAuth, MastersFilter,
    ProviderError, pkce_challenge,
};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{VaultError, VaultStatus};
use common::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

const TOKEN: &str = "lio_TOKEN_OAUTH_SECRETO_9f8e7d";
const CODE: &str = "codigo_de_autorizacao_123";
const PLAYER: ConnectionId = ConnectionId(7);
const WAIT: Duration = Duration::from_secs(10);

fn auth(env: &Env) -> LichessAuth {
    LichessAuth::new(env.net.clone(), env.vault.clone(), env.clock.clone())
}

fn token_response(token: &str) -> FakeResponse {
    FakeResponse::json(&format!(
        "{{\"token_type\":\"Bearer\",\"access_token\":\"{token}\",\"expires_in\":31536000}}"
    ))
}

fn account_response(scopes: &str) -> FakeResponse {
    FakeResponse::json("{\"id\":\"georges\",\"username\":\"Georges\"}")
        .header("x-oauth-scopes", scopes)
}

/// Decodifica `a=b&c=d` (consulta ou corpo form-urlencoded).
fn pairs(text: &str) -> Vec<(String, String)> {
    let decode = |t: &str| {
        let bytes = t.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let byte = u8::from_str_radix(&t[i + 1..i + 3], 16).expect("hex");
                out.push(byte);
                i += 3;
            } else {
                out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
                i += 1;
            }
        }
        String::from_utf8(out).expect("utf-8")
    };
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn param(pairs: &[(String, String)], key: &str) -> String {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("sem {key}"))
}

/// O "navegador": lê a URL de autorização e devolve `(redirect_uri, state, pares)`.
fn browser_view(flow: &AuthFlow) -> (String, String, Vec<(String, String)>) {
    let query = flow
        .url()
        .strip_prefix("https://lichess.org/oauth?")
        .expect("URL de autorização do Lichess");
    let params = pairs(query);
    (
        param(&params, "redirect_uri"),
        param(&params, "state"),
        params,
    )
}

/// O navegador volta ao app: `GET <redirect_uri>?<query>`. Devolve a resposta.
fn come_back(redirect_uri: &str, query: &str) -> std::io::Result<String> {
    let rest = redirect_uri
        .strip_prefix("http://127.0.0.1:")
        .expect("retorno em 127.0.0.1");
    let (port, path) = rest.split_at(rest.find('/').expect("caminho"));
    let mut stream = TcpStream::connect(("127.0.0.1", port.parse::<u16>().expect("porta")))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "GET {path}?{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn requests_to(env: &Env, path: &str) -> Vec<ReceivedRequest> {
    env.server
        .requests()
        .into_iter()
        .filter(|r| r.path == path)
        .collect()
}

/// Arquivos sob `dir` que contêm `needle` em claro.
fn files_containing(dir: &Path, needle: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_containing(&path, needle));
        } else if std::fs::read(&path)
            .is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes()))
        {
            out.push(path);
        }
    }
    out
}

#[test]
fn the_full_pkce_flow_stores_the_token_and_the_explorer_works_with_it() {
    let env = env_with(MemoryKeyring::new(), true);
    env.net
        .set_enabled(ProviderId::Lichess, true)
        .expect("liga");
    env.server.route("/api/token", token_response(TOKEN));
    env.server
        .route("/api/account", account_response("board:play"));
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));
    env.server
        .route("/standard", fixture("lichess-tablebase-kpk.http"));
    let auth = auth(&env);

    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    assert_eq!(flow.state(), AuthState::Waiting);
    let (redirect_uri, state, params) = browser_view(&flow);
    assert_eq!(param(&params, "response_type"), "code");
    assert_eq!(param(&params, "client_id"), "chess-studio");
    assert_eq!(param(&params, "code_challenge_method"), "S256");
    assert_eq!(param(&params, "scope"), "board:play");
    assert!(
        redirect_uri.starts_with("http://127.0.0.1:"),
        "{redirect_uri}"
    );
    assert!(state.len() >= 43, "state imprevisível (256 bits)");
    let challenge = param(&params, "code_challenge");
    assert_eq!(challenge.len(), 43);
    assert!(!params.iter().any(|(k, _)| k == "code_verifier"));

    let page = come_back(&redirect_uri, &format!("code={CODE}&state={state}")).expect("volta");
    assert!(page.contains("Pode voltar ao Chess Studio"), "{page}");
    let AuthState::Authorized(account) = flow.wait(WAIT) else {
        panic!("esperava autorizado, veio {:?}", flow.state());
    };
    assert_eq!(account.username, "Georges");
    assert_eq!(account.id, "georges");
    assert_eq!(account.scopes, vec!["board:play".to_owned()]);
    assert!(!account.is_bot);

    // A troca: form-urlencoded, sem token nem segredo de cliente, com o verificador do
    // desafio (PKCE S256).
    let exchange = &requests_to(&env, "/api/token")[0];
    assert_eq!(exchange.method, "POST");
    assert_eq!(exchange.header("authorization"), None);
    assert_eq!(
        exchange.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    let form = pairs(std::str::from_utf8(&exchange.body).expect("utf-8"));
    assert_eq!(param(&form, "grant_type"), "authorization_code");
    assert_eq!(param(&form, "code"), CODE);
    assert_eq!(param(&form, "redirect_uri"), redirect_uri);
    assert_eq!(param(&form, "client_id"), "chess-studio");
    assert_eq!(pkce_challenge(&param(&form, "code_verifier")), challenge);
    assert!(!form.iter().any(|(k, _)| k == "client_secret"));
    // A conferência levou o token; o Cofre o guardou.
    let bearer = format!("Bearer {TOKEN}");
    assert_eq!(
        requests_to(&env, "/api/account")[0].header("authorization"),
        Some(bearer.as_str())
    );
    assert_eq!(
        env.vault.read(PLAYER).expect("no Cofre").expose_secret(),
        TOKEN
    );
    assert_eq!(env.keyring.accounts(), vec!["lichess/1/1", "lichess/1/7"]);

    // Uma segunda requisição ao loopback não encontra ninguém.
    assert!(come_back(&redirect_uri, &format!("code=OUTRO&state={state}")).is_err());

    // O Explorador volta a funcionar com a Conexão autorizada.
    env.lichess.use_connection(Some(PLAYER));
    env.lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect("Explorador com a Conexão");
    assert_eq!(
        requests_to(&env, "/masters")[0].header("authorization"),
        Some(bearer.as_str())
    );
    let _ = env.lichess.tablebase(KPK).wait();
    assert_eq!(
        requests_to(&env, "/standard")[0].header("authorization"),
        None,
        "o token nunca vai para outro host"
    );

    // O token nunca aparece em URL, Debug, log ou arquivo fora do Cofre.
    for r in env.server.requests() {
        assert!(!r.path.contains(TOKEN));
        assert!(!r.query.iter().any(|(_, v)| v.contains(TOKEN)));
    }
    assert!(!flow.url().contains(TOKEN));
    let texts = [
        format!("{flow:?}"),
        format!("{:?}", flow.state()),
        format!("{auth:?}"),
        format!("{:?}", env.vault),
        format!("{:?}", env.net.recent_operations()),
        format!("{:?}", env.net),
    ];
    for text in texts {
        assert!(!text.contains(TOKEN), "token vazou: {text}");
        assert!(!text.contains(CODE), "código vazou: {text}");
    }
    assert!(
        env.dir.join("rede.db").exists(),
        "o banco da rede está no disco"
    );
    assert_eq!(
        files_containing(&env.dir, TOKEN),
        Vec::<std::path::PathBuf>::new(),
        "token em claro no diretório de dados"
    );
    assert!(files_containing(&env.dir, "SECRETO").is_empty());
}

#[test]
fn a_wrong_state_fails_typed_and_nothing_is_exchanged_or_stored() {
    let env = env_enabled();
    env.server.route("/api/token", token_response(TOKEN));
    let auth = auth(&env);
    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, state, _) = browser_view(&flow);

    come_back(&redirect_uri, &format!("code={CODE}&state=forjado{state}")).expect("volta");

    assert_eq!(flow.wait(WAIT), AuthState::Failed(AuthError::StateMismatch));
    assert!(requests_to(&env, "/api/token").is_empty(), "nada trocado");
    assert!(env.vault.reference(PLAYER).is_none());
    // Uma única requisição: o retorno certo, depois do forjado, já não é ouvido.
    assert!(come_back(&redirect_uri, &format!("code={CODE}&state={state}")).is_err());
    assert_eq!(flow.state(), AuthState::Failed(AuthError::StateMismatch));
}

#[test]
fn state_repeated_or_missing_is_also_a_mismatch() {
    let env = env_enabled();
    let auth = auth(&env);
    for query in ["code=X", "code=X&state=A&state=B"] {
        let flow = auth
            .authorize(PLAYER, ConnectionRole::Player)
            .expect("começa");
        let (redirect_uri, state, _) = browser_view(&flow);
        let query = query.replace('A', &state);
        come_back(&redirect_uri, &query).expect("volta");
        assert_eq!(
            flow.wait(WAIT),
            AuthState::Failed(AuthError::StateMismatch),
            "{query}"
        );
    }
    assert!(env.server.requests().is_empty());
}

#[test]
fn the_user_refusing_in_the_browser_is_refused_only_with_the_right_state() {
    let env = env_enabled();
    let auth = auth(&env);

    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, state, _) = browser_view(&flow);
    come_back(
        &redirect_uri,
        &format!("error=access_denied&error_description=x&state={state}"),
    )
    .expect("volta");
    assert_eq!(flow.wait(WAIT), AuthState::Refused);

    let forged = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, _, _) = browser_view(&forged);
    come_back(&redirect_uri, "error=access_denied&state=outro").expect("volta");
    assert_eq!(
        forged.wait(WAIT),
        AuthState::Failed(AuthError::StateMismatch)
    );
    assert!(env.server.requests().is_empty());
    assert!(env.vault.reference(PLAYER).is_none());
}

#[test]
fn a_reused_code_is_refused_by_the_provider_and_typed() {
    let env = env_enabled();
    env.server.route_seq(
        "/api/token",
        vec![token_response(TOKEN), FakeResponse::status(400)],
    );
    env.server
        .route("/api/account", account_response("board:play"));
    let auth = auth(&env);

    let first = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (uri, state, _) = browser_view(&first);
    come_back(&uri, &format!("code={CODE}&state={state}")).expect("volta");
    assert!(matches!(first.wait(WAIT), AuthState::Authorized(_)));

    let second = auth
        .authorize(ConnectionId(8), ConnectionRole::Player)
        .expect("começa");
    let (uri, state, _) = browser_view(&second);
    come_back(&uri, &format!("code={CODE}&state={state}")).expect("volta");

    assert_eq!(
        second.wait(WAIT),
        AuthState::Failed(AuthError::CodeRejected)
    );
    assert!(env.vault.reference(ConnectionId(8)).is_none());
    assert_eq!(requests_to(&env, "/api/token").len(), 2);
}

#[test]
fn the_deadline_runs_on_the_injected_clock() {
    let env = env_enabled();
    let auth = auth(&env);
    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, state, _) = browser_view(&flow);

    env.clock.advance(DEFAULT_DEADLINE);

    assert_eq!(flow.wait(WAIT), AuthState::Expired);
    assert!(
        come_back(&redirect_uri, &format!("code={CODE}&state={state}")).is_err(),
        "a porta fecha no prazo"
    );
    assert!(env.server.requests().is_empty());
}

#[test]
fn cancelling_or_dropping_the_flow_closes_the_port() {
    let env = env_enabled();
    let auth = auth(&env);

    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, _, _) = browser_view(&flow);
    flow.cancel();
    assert_eq!(flow.wait(WAIT), AuthState::Failed(AuthError::Cancelled));
    // A porta fecha de forma assíncrona: espera com prazo, sem conferir na hora (#236).
    assert!(eventually(WAIT, || come_back(
        &redirect_uri,
        "code=x&state=y"
    )
    .is_err()));

    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (redirect_uri, _, _) = browser_view(&flow);
    drop(flow);
    assert!(eventually(WAIT, || come_back(
        &redirect_uri,
        "code=x&state=y"
    )
    .is_err()));
}

#[test]
fn an_engine_scope_on_a_player_connection_is_refused_and_not_stored() {
    let env = env_enabled();
    env.server.route("/api/token", token_response(TOKEN));
    env.server
        .route("/api/account", account_response("board:play,bot:play"));
    let auth = auth(&env);
    let flow = auth
        .authorize(PLAYER, ConnectionRole::Player)
        .expect("começa");
    let (uri, state, _) = browser_view(&flow);
    come_back(&uri, &format!("code={CODE}&state={state}")).expect("volta");

    let AuthState::Failed(AuthError::WrongScopes { role, scopes }) = flow.wait(WAIT) else {
        panic!("esperava escopos errados, veio {:?}", flow.state());
    };
    assert_eq!(role, ConnectionRole::Player);
    assert_eq!(scopes, vec!["board:play".to_owned(), "bot:play".to_owned()]);
    assert!(env.vault.reference(PLAYER).is_none(), "nada guardado");
    assert!(env.vault.read(PLAYER).is_err());
}

#[test]
fn an_engine_connection_asks_for_bot_play_only() {
    let env = env_enabled();
    let auth = auth(&env);
    let flow = auth
        .authorize(ConnectionId(9), ConnectionRole::Engine)
        .expect("começa");
    let (_, _, params) = browser_view(&flow);
    assert_eq!(param(&params, "scope"), "bot:play");
}

#[test]
fn a_personal_token_is_checked_and_its_real_scopes_decide() {
    let env = env_enabled();
    let auth = auth(&env);
    let run = |token: &str, role: ConnectionRole, connection: ConnectionId| {
        auth.personal_token(connection, role, Secret::new(token))
            .wait_timeout(WAIT)
            .expect("resposta")
    };

    // Token de Engine numa Conexão de jogador, e o inverso: recusados.
    env.server
        .route("/api/account", account_response("bot:play"));
    assert!(matches!(
        run(TOKEN, ConnectionRole::Player, PLAYER),
        Err(AuthError::WrongScopes {
            role: ConnectionRole::Player,
            ..
        })
    ));
    env.server.route(
        "/api/account",
        account_response("board:play,preference:read"),
    );
    assert!(matches!(
        run(TOKEN, ConnectionRole::Engine, ConnectionId(9)),
        Err(AuthError::WrongScopes {
            role: ConnectionRole::Engine,
            ..
        })
    ));
    assert!(env.vault.reference(PLAYER).is_none());
    assert!(env.vault.reference(ConnectionId(9)).is_none());

    // O certo, colado com quebra de linha: guardado, com os escopos reais.
    let account = run(&format!("  {TOKEN}\n"), ConnectionRole::Player, PLAYER).expect("aceito");
    assert_eq!(
        account.scopes,
        vec!["board:play".to_owned(), "preference:read".to_owned()]
    );
    assert_eq!(
        env.vault.read(PLAYER).expect("guardado").expose_secret(),
        TOKEN
    );

    // Um token novo recusado não substitui o bom (substituição atômica).
    env.server.route("/api/account", FakeResponse::status(401));
    assert_eq!(
        run("lio_OUTRO_TOKEN_INVALIDO", ConnectionRole::Player, PLAYER),
        Err(AuthError::TokenRejected)
    );
    assert_eq!(
        env.vault.read(PLAYER).expect("o bom fica").expose_secret(),
        TOKEN
    );
    assert_eq!(
        env.vault.reference(PLAYER).expect("ref").rejected,
        None,
        "a recusa do token em conferência não marca a Conexão guardada"
    );

    // Sem o cabeçalho de escopos, nada é guardado.
    env.server.route(
        "/api/account",
        FakeResponse::json("{\"id\":\"georges\",\"username\":\"Georges\"}"),
    );
    assert_eq!(
        run(
            "lio_SEM_ESCOPOS_1",
            ConnectionRole::Player,
            ConnectionId(10)
        ),
        Err(AuthError::ScopesUnconfirmed)
    );
    assert!(env.vault.reference(ConnectionId(10)).is_none());
}

#[test]
fn a_malformed_personal_token_is_refused_before_the_network() {
    let env = env_enabled();
    let auth = auth(&env);
    let long = "a".repeat(5000);
    for bad in [
        "",
        "   ",
        "lio_com espaço",
        "lio_quebra\r\nX-Injetado: 1",
        long.as_str(),
    ] {
        let result = auth
            .personal_token(PLAYER, ConnectionRole::Player, Secret::new(bad))
            .wait_timeout(WAIT)
            .expect("resposta");
        assert_eq!(result, Err(AuthError::InvalidToken), "{bad:?}");
    }
    assert!(env.server.requests().is_empty());
}

#[test]
fn a_long_token_of_512_and_more_characters_is_accepted() {
    let env = env_enabled();
    env.server
        .route("/api/account", account_response("board:play"));
    let auth = auth(&env);
    let long = format!("lio_{}", "x".repeat(600));

    auth.personal_token(PLAYER, ConnectionRole::Player, Secret::new(long.clone()))
        .wait_timeout(WAIT)
        .expect("resposta")
        .expect("aceito");

    assert_eq!(
        env.vault.read(PLAYER).expect("guardado").expose_secret(),
        long
    );
}

#[test]
fn nothing_starts_without_consent_or_a_vault_that_can_store() {
    // Lichess desligado: nada começa, nem a porta local.
    let env = env();
    let auth = auth(&env);
    assert_eq!(
        auth.authorize(PLAYER, ConnectionRole::Player).unwrap_err(),
        AuthError::Disabled
    );
    assert_eq!(
        auth.personal_token(PLAYER, ConnectionRole::Player, Secret::new(TOKEN))
            .wait_timeout(WAIT),
        Some(Err(AuthError::Disabled))
    );

    // Sem cofre do sistema e sem cofre próprio: nada é autorizado (seria guardado onde?).
    let env = env_with(MemoryKeyring::unavailable(), false);
    env.net
        .set_enabled(ProviderId::Lichess, true)
        .expect("liga");
    assert_eq!(env.vault.status(), VaultStatus::Unavailable);
    let auth = LichessAuth::new(env.net.clone(), env.vault.clone(), env.clock.clone());
    assert_eq!(
        auth.authorize(PLAYER, ConnectionRole::Player).unwrap_err(),
        AuthError::Vault(VaultError::Unavailable)
    );
    // Cofre próprio bloqueado: pede o desbloqueio antes.
    env.vault
        .create_own(&Secret::new("senha-mestra-longa"))
        .expect("cria");
    env.vault.lock();
    assert_eq!(
        auth.authorize(PLAYER, ConnectionRole::Player).unwrap_err(),
        AuthError::Vault(VaultError::Locked)
    );
    assert!(env.server.requests().is_empty());
}

#[test]
fn revoking_deletes_locally_even_when_the_remote_call_fails_and_says_so() {
    let env = env_enabled();
    env.vault
        .store(
            chess_net::LocalProfileId(1),
            ProviderId::Lichess,
            PLAYER,
            Secret::new(TOKEN),
        )
        .expect("guarda");
    env.server.route("/api/token", FakeResponse::status(500));
    let auth = auth(&env);

    let revocation = auth.revoke(PLAYER).wait_timeout(WAIT).expect("resposta");

    assert_eq!(
        revocation.remote,
        Err(AuthError::Net(NetError::Http {
            provider: ProviderId::Lichess,
            status: 500
        }))
    );
    assert_eq!(revocation.local, Ok(()));
    assert!(env.vault.reference(PLAYER).is_none());
    assert!(env.vault.read(PLAYER).is_err());
    assert!(!env.keyring.accounts().contains(&"lichess/1/7".to_owned()));
    let sent = &requests_to(&env, "/api/token")[0];
    assert_eq!(sent.method, "DELETE");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
}

#[test]
fn revoking_succeeds_remotely_and_the_explorer_needs_a_connection_again() {
    let env = env_enabled();
    env.server.route("/api/token", FakeResponse::status(204));
    let auth = auth(&env);

    let revocation = auth
        .revoke(EXPLORER_CONNECTION)
        .wait_timeout(WAIT)
        .expect("resposta");

    assert_eq!(revocation.remote, Ok(()));
    assert_eq!(revocation.local, Ok(()));
    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("sem Conexão");
    assert!(
        matches!(error, ProviderError::SignInRequired { .. }),
        "{error:?}"
    );
}

#[test]
fn revoking_with_the_provider_disabled_still_deletes_locally() {
    let env = env();
    let auth = auth(&env);

    let revocation = auth
        .revoke(EXPLORER_CONNECTION)
        .wait_timeout(WAIT)
        .expect("resposta");

    assert_eq!(revocation.remote, Err(AuthError::Disabled));
    assert_eq!(revocation.local, Ok(()));
    assert!(env.vault.reference(EXPLORER_CONNECTION).is_none());
    assert!(env.server.requests().is_empty());
}

#[test]
fn checking_a_connection_reports_the_account_and_a_401_marks_it() {
    let env = env_enabled();
    env.server.route_seq(
        "/api/account",
        vec![account_response("board:play"), FakeResponse::status(401)],
    );
    let auth = auth(&env);

    let account = auth
        .check(EXPLORER_CONNECTION)
        .wait_timeout(WAIT)
        .expect("resposta")
        .expect("conta");
    assert_eq!(account.username, "Georges");

    let error = auth
        .check(EXPLORER_CONNECTION)
        .wait_timeout(WAIT)
        .expect("resposta")
        .unwrap_err();
    assert_eq!(error, AuthError::TokenRejected);
    let reference = env.vault.reference(EXPLORER_CONNECTION).expect("ref");
    assert_eq!(reference.rejected.map(|r| r.status), Some(401));
    assert!(
        env.vault.read(EXPLORER_CONNECTION).is_ok(),
        "o segredo fica"
    );
}

#[test]
fn revoking_with_the_own_vault_locked_cannot_reach_the_provider_but_still_forgets_locally() {
    let env = env_with(MemoryKeyring::unavailable(), true);
    env.net
        .set_enabled(ProviderId::Lichess, true)
        .expect("liga");
    let password = Secret::new("senha-mestra-longa");
    env.vault
        .create_own(&password)
        .expect("cria o cofre próprio");
    env.vault
        .store(
            chess_net::LocalProfileId(1),
            ProviderId::Lichess,
            PLAYER,
            Secret::new(TOKEN),
        )
        .expect("guarda");
    env.vault.lock();
    let auth = auth(&env);

    let revocation = auth.revoke(PLAYER).wait_timeout(WAIT).expect("resposta");

    assert!(
        matches!(
            revocation.remote,
            Err(AuthError::Net(NetError::NotAuthorized { .. }))
        ),
        "sem o token, o Lichess não foi chamado: {:?}",
        revocation.remote
    );
    assert_eq!(revocation.local, Ok(()));
    assert!(env.server.requests().is_empty());
    assert!(env.vault.reference(PLAYER).is_none());
    env.vault.unlock(&password).expect("desbloqueia");
    assert!(
        env.vault.read(PLAYER).is_err(),
        "o conteúdo cifrado foi limpo"
    );
    assert!(files_containing(&env.dir, TOKEN).is_empty());
}
