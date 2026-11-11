//! Internacionalização com Fluent (DEC-48). Os textos vivem em `assets/i18n/<locale>/*.ftl`
//! e são embutidos no binário (o app é offline e a tradução nunca pode faltar em execução).
//! Hoje só existe pt-BR; outro idioma entra com um novo diretório e um novo `Locale`.

use bevy::prelude::*;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

const PT_BR: &[(&str, &str)] = &[
    (
        "actions.ftl",
        include_str!("../assets/i18n/pt-BR/actions.ftl"),
    ),
    ("app.ftl", include_str!("../assets/i18n/pt-BR/app.ftl")),
    ("shell.ftl", include_str!("../assets/i18n/pt-BR/shell.ftl")),
    ("dock.ftl", include_str!("../assets/i18n/pt-BR/dock.ftl")),
    (
        "profiles.ftl",
        include_str!("../assets/i18n/pt-BR/profiles.ftl"),
    ),
    ("focus.ftl", include_str!("../assets/i18n/pt-BR/focus.ftl")),
    (
        "library.ftl",
        include_str!("../assets/i18n/pt-BR/library.ftl"),
    ),
    ("game.ftl", include_str!("../assets/i18n/pt-BR/game.ftl")),
    (
        "endgame.ftl",
        include_str!("../assets/i18n/pt-BR/endgame.ftl"),
    ),
    (
        "engine.ftl",
        include_str!("../assets/i18n/pt-BR/engine.ftl"),
    ),
    ("aids.ftl", include_str!("../assets/i18n/pt-BR/aids.ftl")),
    (
        "wizard.ftl",
        include_str!("../assets/i18n/pt-BR/wizard.ftl"),
    ),
    (
        "manager.ftl",
        include_str!("../assets/i18n/pt-BR/manager.ftl"),
    ),
    ("fide.ftl", include_str!("../assets/i18n/pt-BR/fide.ftl")),
    (
        "newgame.ftl",
        include_str!("../assets/i18n/pt-BR/newgame.ftl"),
    ),
    (
        "analysis.ftl",
        include_str!("../assets/i18n/pt-BR/analysis.ftl"),
    ),
    (
        "controls.ftl",
        include_str!("../assets/i18n/pt-BR/controls.ftl"),
    ),
    (
        "localprofiles.ftl",
        include_str!("../assets/i18n/pt-BR/localprofiles.ftl"),
    ),
    (
        "scoresheet.ftl",
        include_str!("../assets/i18n/pt-BR/scoresheet.ftl"),
    ),
    (
        "training.ftl",
        include_str!("../assets/i18n/pt-BR/training.ftl"),
    ),
    (
        "openings.ftl",
        include_str!("../assets/i18n/pt-BR/openings.ftl"),
    ),
    (
        "explorer.ftl",
        include_str!("../assets/i18n/pt-BR/explorer.ftl"),
    ),
    (
        "authorization.ftl",
        include_str!("../assets/i18n/pt-BR/authorization.ftl"),
    ),
    (
        "options.ftl",
        include_str!("../assets/i18n/pt-BR/options.ftl"),
    ),
    (
        "gallery.ftl",
        include_str!("../assets/i18n/pt-BR/gallery.ftl"),
    ),
    (
        "ribbon.ftl",
        include_str!("../assets/i18n/pt-BR/ribbon.ftl"),
    ),
    (
        "connections.ftl",
        include_str!("../assets/i18n/pt-BR/connections.ftl"),
    ),
];

/// Tradutor do idioma ativo. Chave ausente devolve a própria chave (visível, nunca pânico).
#[derive(Resource)]
pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n")
            .field("locales", &self.bundle.locales)
            .finish()
    }
}

impl I18n {
    /// Português do Brasil, o idioma padrão do P0.
    pub fn pt_br() -> Self {
        let lang: LanguageIdentifier = "pt-BR".parse().unwrap_or_default();
        Self::from_sources(lang, PT_BR)
    }

    fn from_sources(lang: LanguageIdentifier, sources: &[(&str, &str)]) -> Self {
        let mut bundle = FluentBundle::new_concurrent(vec![lang]);
        // Sem marcas de isolamento bidi: o texto vai direto para o egui.
        bundle.set_use_isolating(false);
        for (name, text) in sources {
            match FluentResource::try_new((*text).to_owned()) {
                Ok(resource) => {
                    if let Err(errors) = bundle.add_resource(resource) {
                        error!("i18n: mensagens duplicadas em {name}: {errors:?}");
                    }
                }
                Err((resource, errors)) => {
                    error!("i18n: erros de sintaxe em {name}: {errors:?}");
                    let _ = bundle.add_resource(resource);
                }
            }
        }
        Self { bundle }
    }

    /// Texto da mensagem sem argumentos.
    pub fn t(&self, key: &str) -> String {
        self.format(key, None)
    }

    /// Texto da mensagem com argumentos de texto (`{ $nome }`).
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, FluentValue::from((*value).to_owned()));
        }
        self.format(key, Some(&fluent_args))
    }

    fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        let Some(pattern) = self
            .bundle
            .get_message(key)
            .and_then(|message| message.value())
        else {
            warn!("i18n: chave ausente `{key}`");
            return key.to_owned();
        };
        let mut errors = Vec::new();
        let text = self
            .bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned();
        if !errors.is_empty() {
            warn!("i18n: erros ao formatar `{key}`: {errors:?}");
        }
        text
    }

    /// A chave existe no idioma ativo?
    #[cfg(test)]
    fn has(&self, key: &str) -> bool {
        self.bundle.has_message(key)
    }
}

pub struct I18nPlugin;

impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(I18n::pt_br());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn all_ftl_files_parse_without_errors() {
        for (name, text) in PT_BR {
            assert!(
                FluentResource::try_new((*text).to_owned()).is_ok(),
                "{name} tem erro de sintaxe"
            );
        }
    }

    #[test]
    fn formats_messages_with_and_without_arguments() {
        let i18n = I18n::pt_br();
        assert_eq!(i18n.t("button-undo"), "Desfazer");
        assert_eq!(
            i18n.t_with("game-last-move", &[("san", "Nf3")]),
            "Último lance: Nf3"
        );
    }

    #[test]
    fn missing_key_returns_the_key() {
        assert_eq!(I18n::pt_br().t("nao-existe"), "nao-existe");
    }

    /// Todo literal do código que parece um identificador de mensagem existe no .ftl.
    #[test]
    fn every_message_id_used_in_the_source_exists() {
        const PREFIXES: [&str; 26] = [
            "app-",
            "presentation-",
            "quality-",
            "view-",
            "square-",
            "help-",
            "game-",
            "button-",
            "move-method-",
            "promotion-",
            "status-",
            "aid-",
            "review-",
            "fen-",
            "pgn-",
            "resume-",
            "persist-",
            "clock-",
            "fide-",
            "newgame-",
            "dock-",
            "notation-",
            "analysis-",
            "scoresheet-",
            "openings-",
            "explorer-",
        ];
        let i18n = I18n::pt_br();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            // `action_icons.rs` guarda nomes de ícones Phosphor, que não são ids Fluent.
            if path.extension().is_none_or(|e| e != "rs")
                || path.file_name().is_some_and(|n| n == "action_icons.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for literal in text.split('"').skip(1).step_by(2) {
                let is_id = !literal.ends_with('-')
                    && literal
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    && PREFIXES.iter().any(|p| literal.starts_with(p));
                if is_id {
                    checked += 1;
                    assert!(i18n.has(literal), "{}: falta `{literal}`", path.display());
                }
            }
        }
        assert!(checked > 10, "o teste não encontrou chaves ({checked})");
    }
}
