//! Abrir uma página no navegador do sistema (instruções e termos do Provedor). O endereço vem
//! sempre do catálogo (`Provider::docs_url` e `terms_url`), nunca de um campo digitado. O app
//! não abre a conexão: entrega o endereço ao navegador do usuário.

use crate::paths::Platform;

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

    #[test]
    fn every_catalog_page_can_be_opened() {
        for p in chess_net::catalog() {
            for url in [p.docs_url, p.terms_url] {
                assert!(command(Platform::Linux, url).is_some(), "{}: {url}", p.name);
            }
        }
    }
}
