//! Abrir uma página no navegador do sistema. O app não abre a conexão: entrega o endereço ao
//! navegador do usuário. Só há duas origens de endereço, e nenhuma é um campo digitado:
//!
//! - instruções e termos do Provedor, do catálogo (`Provider::docs_url` e `terms_url`), por
//!   [`open`];
//! - a página de **autorização do Lichess**, que o `chess-providers` monta e o app só repassa,
//!   por [`open_authorization`], que a confere por uma regra explícita antes de abrir
//!   ([`is_lichess_authorization`]): só `https://lichess.org/oauth?...`, com o retorno no
//!   próprio computador (`127.0.0.1`) e o desafio de segurança presente.

use crate::paths::Platform;

/// Início da página de autorização do Lichess (sem usuário, porta nem outro domínio no meio).
const LICHESS_AUTHORIZE: &str = "https://lichess.org/oauth?";
/// O retorno da autorização vai para o próprio computador, no caminho do app.
const LOOPBACK_PREFIX: &str = "http%3A%2F%2F127.0.0.1%3A";
const LOOPBACK_PATH: &str = "%2Fchess-studio%2Fretorno";

/// O endereço é exatamente uma página de autorização do Lichess montada pelo app: `https`,
/// domínio `lichess.org` (nada de usuário, porta ou domínio parecido), consulta com o fluxo de
/// código + desafio S256, o `state`, um escopo mínimo de jogo (`board:play` ou `bot:play`) e o
/// retorno em `127.0.0.1` na porta efêmera do app. Qualquer outro endereço é recusado.
pub fn is_lichess_authorization(url: &str) -> bool {
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) || url.contains('#') {
        return false;
    }
    let Some(query) = url.strip_prefix(LICHESS_AUTHORIZE) else {
        return false;
    };
    let mut seen: Vec<&str> = Vec::new();
    let value_of = |wanted: &str| -> Option<&str> {
        let mut found = None;
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=')?;
            if key == wanted {
                if found.is_some() {
                    return None; // chave repetida
                }
                found = Some(value);
            }
        }
        found
    };
    for key in [
        "response_type",
        "client_id",
        "redirect_uri",
        "code_challenge_method",
        "code_challenge",
        "scope",
        "state",
    ] {
        match value_of(key) {
            Some(value) if !value.is_empty() => seen.push(value),
            _ => return false,
        }
    }
    let [
        response_type,
        client_id,
        redirect,
        method,
        challenge,
        scope,
        state,
    ] = seen[..]
    else {
        return false;
    };
    let port_ok = redirect
        .strip_prefix(LOOPBACK_PREFIX)
        .and_then(|rest| rest.strip_suffix(LOOPBACK_PATH))
        .is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    let plain = |text: &str| {
        text.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
    };
    response_type == "code"
        && client_id == "chess-studio"
        && method == "S256"
        && port_ok
        && matches!(scope, "board%3Aplay" | "bot%3Aplay")
        && plain(challenge)
        && plain(state)
}

/// Programa e argumentos que abrem `url` no navegador padrão, ou `None` se o endereço não é um
/// `https://` simples (sem espaço nem caractere de controle).
pub fn command(platform: Platform, url: &str) -> Option<(&'static str, Vec<String>)> {
    let simple =
        url.starts_with("https://") && !url.chars().any(|c| c.is_whitespace() || c.is_control());
    if !simple {
        return None;
    }
    Some(match platform {
        Platform::Linux => ("xdg-open", vec![url.to_owned()]),
        Platform::MacOs => ("open", vec![url.to_owned()]),
        Platform::Windows => (
            "rundll32",
            vec!["url.dll,FileProtocolHandler".to_owned(), url.to_owned()],
        ),
    })
}

/// Pede ao sistema que abra a página de autorização do Lichess. Devolve `false`, sem abrir
/// nada, se o endereço não é exatamente uma página de autorização esperada.
pub fn open_authorization(url: &str) -> bool {
    is_lichess_authorization(url) && open(url)
}

/// Pede ao sistema que abra `url` no navegador. Devolve se o pedido foi entregue.
pub fn open(url: &str) -> bool {
    let Some((program, args)) = command(Platform::current(), url) else {
        return false;
    };
    let spawned = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match spawned {
        Ok(mut child) => {
            // Recolhe o processo filho sem bloquear a interface.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_platform_has_its_own_opener() {
        let url = "https://lichess.org/terms-of-service";
        assert_eq!(
            command(Platform::Linux, url),
            Some(("xdg-open", vec![url.to_owned()]))
        );
        assert_eq!(
            command(Platform::MacOs, url),
            Some(("open", vec![url.to_owned()]))
        );
        let (program, args) = command(Platform::Windows, url).unwrap();
        assert_eq!(program, "rundll32");
        assert_eq!(args.last().map(String::as_str), Some(url));
    }

    #[test]
    fn only_plain_https_addresses_are_opened() {
        for bad in [
            "http://example.org",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://a b",
            "https://a\nb",
            "",
        ] {
            assert_eq!(command(Platform::Linux, bad), None, "{bad:?}");
        }
    }

    /// Uma página de autorização como o `chess-providers` a monta.
    fn authorization(scope: &str, redirect: &str) -> String {
        format!(
            "https://lichess.org/oauth?response_type=code&client_id=chess-studio\
             &redirect_uri={redirect}&code_challenge_method=S256\
             &code_challenge=E9Melhoxx-0qb_aB&scope={scope}&state=Xy12-_ab"
        )
    }

    const REDIRECT: &str = "http%3A%2F%2F127.0.0.1%3A45123%2Fchess-studio%2Fretorno";

    #[test]
    fn the_real_lichess_authorization_page_is_accepted_by_an_explicit_rule() {
        for scope in ["board%3Aplay", "bot%3Aplay"] {
            assert!(is_lichess_authorization(&authorization(scope, REDIRECT)));
        }
    }

    #[test]
    fn any_other_authorization_address_is_refused() {
        let good = authorization("board%3Aplay", REDIRECT);
        let cases = [
            good.replacen("https://", "http://", 1),
            good.replacen("lichess.org", "lichess.org.exemplo.com", 1),
            good.replacen("https://lichess.org", "https://lichess.org@exemplo.com", 1),
            good.replacen("https://lichess.org", "https://lichess.org:444", 1),
            good.replacen("/oauth?", "/outra?", 1),
            good.replacen("lichess.org/oauth", "exemplo.org/oauth", 1),
            authorization("board%3Aplay", "https%3A%2F%2Fexemplo.org%2Fretorno"),
            authorization(
                "board%3Aplay",
                "http%3A%2F%2Fexemplo.org%3A80%2Fchess-studio%2Fretorno",
            ),
            authorization(
                "board%3Aplay",
                "http%3A%2F%2F127.0.0.1%3A%2Fchess-studio%2Fretorno",
            ),
            authorization("board%3Aplay", "http%3A%2F%2F127.0.0.1%3A80%2Foutro"),
            // Permissões demais ou de outro tipo.
            authorization("board%3Aplay%20bot%3Aplay", REDIRECT),
            authorization("email%3Aread", REDIRECT),
            // Sem desafio, sem estado, com chave repetida, com fragmento ou espaço.
            good.replacen("&code_challenge=", "&xcode_challenge=", 1),
            good.replacen("&state=", "&xstate=", 1),
            format!("{good}&state=outro"),
            format!("{good}#fragmento"),
            format!("{good} &x=1"),
            good.replacen(
                "code_challenge_method=S256",
                "code_challenge_method=plain",
                1,
            ),
            good.replacen("client_id=chess-studio", "client_id=outro-app", 1),
            "https://lichess.org/oauth".to_owned(),
            "https://lichess.org/oauth?".to_owned(),
            "https://lichess.org/terms-of-service".to_owned(),
            String::new(),
        ];
        for bad in &cases {
            assert!(!is_lichess_authorization(bad), "{bad}");
            assert!(!open_authorization(bad), "nada pode ser aberto para {bad}");
        }
    }

    #[test]
    fn every_catalog_page_can_be_opened() {
        for p in chess_net::catalog() {
            for url in [p.docs_url, p.terms_url] {
                assert!(command(Platform::Linux, url).is_some(), "{}: {url}", p.name);
            }
        }
    }
}
