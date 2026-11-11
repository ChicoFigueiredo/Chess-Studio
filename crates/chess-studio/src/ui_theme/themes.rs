//! Catálogo de Temas (`docs/design/themes.json`, `temas.md`), lido em tempo de compilação.
//! São 22: Escuro e Claro, de fábrica (as bases, em `tokens.json`), e 20 de xadrez. Cada Tema
//! parte de uma base (Escuro ou Claro) e troca só as 22 cores listadas; o tabuleiro, a barra de
//! avaliação e as sombras vêm sempre da base. Nada aqui depende do Bevy.

use super::tokens::{Palette, ThemeKind, TokenError, Tokens, parse_hex};
use bevy_egui::egui::Color32;
use serde::Deserialize;
use std::sync::OnceLock;

const EMBEDDED: &str = include_str!("../../../../docs/design/themes.json");

/// As 22 cores que um Tema de xadrez define, na ordem de `themes.json`.
pub const THEME_COLOR_NAMES: [&str; 22] = [
    "surface-0",
    "surface-1",
    "surface-2",
    "surface-3",
    "surface-4",
    "border",
    "border-strong",
    "ink",
    "ink-muted",
    "ink-disabled",
    "accent",
    "accent-hover",
    "accent-pressed",
    "on-accent",
    "accent-subtle",
    "accent-ink",
    "focus-ring",
    "info",
    "success",
    "warning",
    "danger",
    "scrim",
];

/// Identifica um dos 22 Temas (posição no catálogo). Barato de copiar; o texto estável, que vai
/// para o banco, é [`ThemeId::key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThemeId(u8);

impl ThemeId {
    /// Escuro de fábrica.
    pub const DARK: ThemeId = ThemeId(0);
    /// Claro de fábrica.
    pub const LIGHT: ThemeId = ThemeId(1);

    /// O Tema de fábrica de uma base.
    pub fn factory(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Dark => Self::DARK,
            ThemeKind::Light => Self::LIGHT,
        }
    }

    pub(super) fn index(self) -> usize {
        usize::from(self.0)
    }

    /// Procura pelo id de `themes.json` (`"feltro"`).
    pub fn from_key(key: &str) -> Option<Self> {
        ThemeCatalog::embedded()
            .themes
            .iter()
            .position(|t| t.key == key)
            .map(|i| Self(i as u8))
    }

    pub fn info(self) -> &'static ThemeInfo {
        &ThemeCatalog::embedded().themes[usize::from(self.0)]
    }

    /// Id estável (`"feltro"`), usado na persistência.
    pub fn key(self) -> &'static str {
        &self.info().key
    }

    pub fn name(self) -> &'static str {
        &self.info().name
    }

    /// A base (Escuro ou Claro) de onde o Tema parte.
    pub fn base(self) -> ThemeKind {
        self.info().base
    }
}

impl From<ThemeKind> for ThemeId {
    fn from(kind: ThemeKind) -> Self {
        Self::factory(kind)
    }
}

/// Nível de contraste prometido pelo Tema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContrastLevel {
    /// Texto 4,5:1 e contornos 3:1 (WCAG 2 AA).
    Aa,
    /// Alto contraste: texto 7:1 e `border` 3:1 (Pretas e Brancas).
    Aaa,
}

/// Um Tema do catálogo.
#[derive(Clone, Debug)]
pub struct ThemeInfo {
    pub key: String,
    pub name: String,
    pub base: ThemeKind,
    pub family: String,
    pub contrast: ContrastLevel,
    pub inspiration: String,
    /// As cores que o Tema troca (vazio nos de fábrica).
    colors: Vec<(String, Color32)>,
}

impl ThemeInfo {
    /// Os pares (nome do token, cor) que o Tema troca sobre a base.
    pub fn colors(&self) -> &[(String, Color32)] {
        &self.colors
    }

    #[allow(dead_code)] // usado nos testes e pela galeria
    pub fn is_factory(&self) -> bool {
        self.colors.is_empty()
    }
}

#[derive(Deserialize)]
struct RawCatalog {
    themes: Vec<RawTheme>,
}

#[derive(Deserialize)]
struct RawTheme {
    id: String,
    name: String,
    base: String,
    family: String,
    contrast: String,
    #[serde(default)]
    inspiration: String,
    colors: std::collections::BTreeMap<String, String>,
}

/// O catálogo completo, na ordem de `themes.json`.
#[derive(Debug)]
pub struct ThemeCatalog {
    themes: Vec<ThemeInfo>,
}

impl ThemeCatalog {
    pub fn parse(json: &str) -> Result<Self, TokenError> {
        let raw: RawCatalog =
            serde_json::from_str(json).map_err(|e| TokenError::Json(e.to_string()))?;
        let mut themes: Vec<ThemeInfo> = Vec::new();
        for t in raw.themes {
            let base = match t.base.as_str() {
                "dark" => ThemeKind::Dark,
                "light" => ThemeKind::Light,
                other => return Err(TokenError::BadValue(format!("base {other:?} em {}", t.id))),
            };
            let contrast = match t.contrast.as_str() {
                "aa" => ContrastLevel::Aa,
                "aaa" => ContrastLevel::Aaa,
                other => {
                    return Err(TokenError::BadValue(format!(
                        "contraste {other:?} em {}",
                        t.id
                    )));
                }
            };
            let mut colors = Vec::new();
            for (name, hex) in t.colors {
                if !THEME_COLOR_NAMES.contains(&name.as_str()) {
                    return Err(TokenError::BadValue(format!("cor {name:?} em {}", t.id)));
                }
                colors.push((name, parse_hex(&hex)?));
            }
            if themes.iter().any(|x| x.key == t.id) {
                return Err(TokenError::BadValue(format!("id repetido: {}", t.id)));
            }
            themes.push(ThemeInfo {
                key: t.id,
                name: t.name,
                base,
                family: t.family,
                contrast,
                inspiration: t.inspiration,
                colors,
            });
        }
        // Os dois primeiros são os de fábrica (as bases).
        let factory_ok = themes.len() >= 2
            && themes[0].key == "dark"
            && themes[0].base == ThemeKind::Dark
            && themes[1].key == "light"
            && themes[1].base == ThemeKind::Light;
        if !factory_ok || themes.len() > usize::from(u8::MAX) {
            return Err(TokenError::BadValue(
                "themes.json precisa começar por dark e light".into(),
            ));
        }
        Ok(Self { themes })
    }

    /// O catálogo embutido no binário (conferido pelos testes).
    pub fn embedded() -> &'static ThemeCatalog {
        static CATALOG: OnceLock<ThemeCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            ThemeCatalog::parse(EMBEDDED)
                .expect("docs/design/themes.json inválido (ver testes de ui_theme)")
        })
    }

    #[allow(dead_code)] // usado nos testes
    pub fn len(&self) -> usize {
        self.themes.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ThemeId, &ThemeInfo)> {
        self.themes
            .iter()
            .enumerate()
            .map(|(i, t)| (ThemeId(i as u8), t))
    }
}

/// Todos os Temas, na ordem do catálogo.
pub fn all_themes() -> impl Iterator<Item = ThemeId> {
    ThemeCatalog::embedded().iter().map(|(id, _)| id)
}

/// O que a galeria de Temas precisa para desenhar um cartão.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // campos para a galeria de Temas (ticket seguinte)
pub struct ThemeSwatch {
    pub id: ThemeId,
    pub name: &'static str,
    pub base: ThemeKind,
    pub contrast: ContrastLevel,
    pub inspiration: &'static str,
    pub surface_1: Color32,
    pub accent: Color32,
    pub ink: Color32,
}

/// Os Temas agrupados por família (`Padrão`, `Materiais`, `Aberturas`, `Jogo`, `Alto contraste`),
/// na ordem em que aparecem no catálogo.
#[allow(dead_code)] // API para a galeria de Temas (ticket seguinte)
pub fn themes_by_family() -> Vec<(&'static str, Vec<ThemeSwatch>)> {
    let mut families: Vec<(&'static str, Vec<ThemeSwatch>)> = Vec::new();
    for (id, info) in ThemeCatalog::embedded().iter() {
        let p = Palette::of_theme(id);
        let swatch = ThemeSwatch {
            id,
            name: &info.name,
            base: info.base,
            contrast: info.contrast,
            inspiration: &info.inspiration,
            surface_1: p.surface_1,
            accent: p.accent,
            ink: p.ink,
        };
        match families.iter_mut().find(|(f, _)| *f == info.family) {
            Some((_, list)) => list.push(swatch),
            None => families.push((&info.family, vec![swatch])),
        }
    }
    families
}

/// Paleta de um Tema: a base e, por cima, as cores do Tema.
pub fn build_palette(tokens: &Tokens, theme: ThemeId) -> Result<Palette, TokenError> {
    let info = theme.info();
    let mut palette = Palette::from_tokens(tokens, info.base)?;
    for (name, color) in info.colors() {
        palette.set_color(name, *color)?;
    }
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::tokens::contrast_ratio;

    fn catalog() -> &'static ThemeCatalog {
        ThemeCatalog::embedded()
    }

    #[test]
    fn the_catalog_has_22_themes_two_factory_and_20_of_chess() {
        assert_eq!(catalog().len(), 22);
        assert_eq!(all_themes().filter(|t| t.info().is_factory()).count(), 2);
        assert_eq!(ThemeId::DARK.key(), "dark");
        assert_eq!(ThemeId::LIGHT.key(), "light");
        let dark = all_themes().filter(|t| t.base() == ThemeKind::Dark).count();
        assert_eq!((dark, 22 - dark), (11, 11), "2 de fábrica + 10 + 10");
    }

    #[test]
    fn every_chess_theme_defines_exactly_the_22_colors_and_a_valid_base() {
        for (_, t) in catalog().iter().filter(|(_, t)| !t.is_factory()) {
            let mut names: Vec<&str> = t.colors().iter().map(|(n, _)| n.as_str()).collect();
            assert_eq!(names.len(), 22, "{}: {} cores", t.key, names.len());
            names.sort_unstable();
            let mut expected = THEME_COLOR_NAMES.to_vec();
            expected.sort_unstable();
            assert_eq!(names, expected, "{}: conjunto de cores", t.key);
        }
    }

    #[test]
    fn a_theme_with_a_bad_base_or_a_missing_color_name_is_rejected() {
        let bad_base = r#"{"themes":[{"id":"dark","name":"E","base":"azul","family":"F","contrast":"aa","colors":{}}]}"#;
        assert!(ThemeCatalog::parse(bad_base).is_err());
        let bad_color = r##"{"themes":[
            {"id":"dark","name":"E","base":"dark","family":"F","contrast":"aa","colors":{}},
            {"id":"light","name":"C","base":"light","family":"F","contrast":"aa","colors":{"nao-existe":"#ffffff"}}]}"##;
        assert!(ThemeCatalog::parse(bad_color).is_err());
    }

    #[test]
    fn the_palette_is_the_base_plus_the_theme_colors() {
        let tokens = Tokens::embedded();
        let feltro = ThemeId::from_key("feltro").expect("feltro");
        let p = Palette::of_theme(feltro);
        let base = Palette::of(ThemeKind::Dark);
        assert_eq!(p.surface_1, parse_hex("#111d16").unwrap());
        assert_eq!(p.accent, parse_hex("#d5aa55").unwrap());
        assert_ne!(p.surface_1, base.surface_1);
        // O que não é do Tema vem da base.
        assert_eq!(p.board_select, base.board_select);
        assert_eq!(
            (p.eval_white, p.eval_black),
            (base.eval_white, base.eval_black)
        );
        for name in ["board-light", "board-dark", "last-move", "hover-ring"] {
            assert!(
                !THEME_COLOR_NAMES.contains(&name),
                "{name} nunca é de um Tema"
            );
            tokens.color(name, ThemeKind::Dark).expect(name);
        }
        // Os de fábrica são a própria base.
        assert_eq!(
            Palette::of_theme(ThemeId::DARK),
            Palette::of(ThemeKind::Dark)
        );
        assert_eq!(
            Palette::of_theme(ThemeId::LIGHT),
            Palette::of(ThemeKind::Light)
        );
        // Um Tema claro parte da base clara.
        let torneio = ThemeId::from_key("torneio").expect("torneio");
        assert_eq!(torneio.base(), ThemeKind::Light);
        assert_eq!(
            Palette::of_theme(torneio).eval_white,
            Palette::of(ThemeKind::Light).eval_white
        );
    }

    #[test]
    fn the_gallery_api_lists_themes_by_family_with_name_base_and_colors() {
        let families = themes_by_family();
        let names: Vec<&str> = families.iter().map(|(f, _)| *f).collect();
        assert_eq!(
            names,
            ["Padrão", "Materiais", "Jogo", "Aberturas", "Alto contraste"]
        );
        assert_eq!(families.iter().map(|(_, l)| l.len()).sum::<usize>(), 22);
        let hc = &families[4].1;
        assert_eq!(hc.len(), 2);
        assert!(hc.iter().all(|s| s.contrast == ContrastLevel::Aaa));
        let feltro = families
            .iter()
            .flat_map(|(_, l)| l)
            .find(|s| s.name == "Feltro")
            .expect("Feltro");
        assert_eq!(feltro.base, ThemeKind::Dark);
        assert_eq!(feltro.surface_1, parse_hex("#111d16").unwrap());
        assert_eq!(feltro.accent, parse_hex("#d5aa55").unwrap());
    }

    // --- Contrato de contraste (temas.md) ---------------------------------------------------

    fn check(
        theme: ThemeId,
        name: &str,
        fg: Color32,
        bg: Color32,
        min: f32,
        fails: &mut Vec<String>,
    ) {
        let ratio = contrast_ratio(fg, bg);
        if ratio < min {
            fails.push(format!("{} {name}: {ratio:.2}:1 < {min}:1", theme.key()));
        }
    }

    /// Todo Tema cumpre os pares de `temas.md`: texto 4,5:1 (7:1 nos de alto contraste),
    /// contornos, foco e marcas 3:1.
    #[test]
    fn every_theme_meets_the_contrast_contract() {
        let tokens = Tokens::embedded();
        let mut fails = Vec::new();
        for theme in all_themes() {
            let p = Palette::of_theme(theme);
            let hc = theme.info().contrast == ContrastLevel::Aaa;
            let text = if hc { 7.0 } else { 4.5 };
            let s = [
                p.surface_0,
                p.surface_1,
                p.surface_2,
                p.surface_3,
                p.surface_4,
            ];

            let mut texts: Vec<(String, Color32, Color32)> = vec![];
            for (i, bg) in s.iter().enumerate() {
                texts.push((format!("ink/surface-{i}"), p.ink, *bg));
            }
            texts.push(("ink/accent-subtle".into(), p.ink, p.accent_subtle));
            for (i, bg) in s.iter().take(4).enumerate() {
                texts.push((format!("ink-muted/surface-{i}"), p.ink_muted, *bg));
            }
            texts.push((
                "ink-muted/accent-subtle".into(),
                p.ink_muted,
                p.accent_subtle,
            ));
            for (n, bg) in [
                ("accent", p.accent),
                ("accent-hover", p.accent_hover),
                ("accent-pressed", p.accent_pressed),
            ] {
                texts.push((format!("on-accent/{n}"), p.on_accent, bg));
            }
            texts.push((
                "accent-ink/accent-subtle".into(),
                p.accent_ink,
                p.accent_subtle,
            ));
            for (i, bg) in s.iter().enumerate().skip(1).take(3) {
                texts.push((format!("accent-ink/surface-{i}"), p.accent_ink, *bg));
                for (n, fg) in [
                    ("info", p.info),
                    ("success", p.success),
                    ("warning", p.warning),
                ] {
                    texts.push((format!("{n}/surface-{i}"), fg, *bg));
                }
            }
            for (i, bg) in s.iter().enumerate().skip(1) {
                texts.push((format!("danger/surface-{i}"), p.danger, *bg));
            }
            for (name, fg, bg) in texts {
                check(theme, &name, fg, bg, text, &mut fails);
            }

            // Contornos e marcas: 3:1.
            for (i, bg) in s.iter().take(3).enumerate() {
                check(
                    theme,
                    &format!("border-strong/surface-{i}"),
                    p.border_strong,
                    *bg,
                    3.0,
                    &mut fails,
                );
                check(
                    theme,
                    &format!("accent/surface-{i}"),
                    p.accent,
                    *bg,
                    3.0,
                    &mut fails,
                );
            }
            check(
                theme,
                "accent/accent-subtle",
                p.accent,
                p.accent_subtle,
                3.0,
                &mut fails,
            );
            for (i, bg) in s.iter().enumerate() {
                check(
                    theme,
                    &format!("focus-ring/surface-{i}"),
                    p.focus_ring,
                    *bg,
                    3.0,
                    &mut fails,
                );
            }
            for n in ["board-light", "board-dark"] {
                let square = tokens.color(n, theme.base()).unwrap();
                check(
                    theme,
                    &format!("board-select/{n}"),
                    p.board_select,
                    square,
                    3.0,
                    &mut fails,
                );
            }
            // Alto contraste: as divisórias também separam por contorno.
            if hc {
                for (i, bg) in s.iter().take(3).enumerate() {
                    check(
                        theme,
                        &format!("border/surface-{i}"),
                        p.border,
                        *bg,
                        3.0,
                        &mut fails,
                    );
                }
            }
        }
        assert!(
            fails.is_empty(),
            "contrato de contraste:\n{}",
            fails.join("\n")
        );
    }
}
