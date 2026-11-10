//! Flag de desenvolvimento da interface nova (design system, épico #159): `--nova-ui` ou
//! `CHESS_STUDIO_NOVA_UI=1`. Desligado, a interface atual fica intacta. Os tickets DS-3 a DS-5
//! (Ribbon, barra de status, Opções, dock) usam o mesmo `enabled()`.

/// `--nova-ui` na linha de comando ou `CHESS_STUDIO_NOVA_UI=1` no ambiente.
pub fn enabled() -> bool {
    from_sources(
        std::env::args().skip(1),
        std::env::var("CHESS_STUDIO_NOVA_UI").ok().as_deref(),
    )
}

fn from_sources(mut args: impl Iterator<Item = String>, env: Option<&str>) -> bool {
    args.any(|a| a == "--nova-ui") || matches!(env, Some("1" | "true"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn off_by_default() {
        assert!(!from_sources(args(&["--gallery"]), None));
        assert!(!from_sources(args(&[]), Some("0")));
    }

    #[test]
    fn on_with_the_flag_or_the_environment_variable() {
        assert!(from_sources(args(&["--nova-ui"]), None));
        assert!(from_sources(args(&[]), Some("1")));
    }
}
