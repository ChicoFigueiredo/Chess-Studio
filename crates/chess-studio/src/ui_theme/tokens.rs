//! Tokens do design system (`docs/design/tokens.json`), lidos em tempo de compilação: o JSON é
//! a única fonte de verdade das cores, medidas e tipografia (ADR-0005). Nada aqui depende do
//! Bevy; o egui só entra na conversão para `Color32`.

use bevy_egui::egui::{self, Color32};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const EMBEDDED: &str = include_str!("../../../../docs/design/tokens.json");

/// Tema visual já resolvido (o "Seguir o sistema" vira um destes dois).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeKind {
    Dark,
    Light,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 2] = [ThemeKind::Dark, ThemeKind::Light];

    pub fn egui(self) -> egui::Theme {
        match self {
            ThemeKind::Dark => egui::Theme::Dark,
            ThemeKind::Light => egui::Theme::Light,
        }
    }

    pub fn from_dark_mode(dark: bool) -> Self {
        if dark {
            ThemeKind::Dark
        } else {
            ThemeKind::Light
        }
    }
}

/// Por que o JSON de tokens não pôde ser lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    Json(String),
    Missing(String),
    BadValue(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Json(e) => write!(f, "tokens.json ilegível: {e}"),
            TokenError::Missing(n) => write!(f, "token ausente: {n}"),
            TokenError::BadValue(v) => write!(f, "valor de token inválido: {v}"),
        }
    }
}

impl std::error::Error for TokenError {}

#[derive(Deserialize)]
struct RawFile {
    color: RawColors,
    #[serde(rename = "type")]
    typography: RawType,
    spacing: RawLengths,
    radius: RawLengths,
    shadow: RawShadows,
    size: RawLengths,
}

#[derive(Deserialize)]
struct RawColors {
    tokens: Vec<RawColor>,
}

#[derive(Deserialize)]
struct RawColor {
    name: String,
    value: RawPair,
}

#[derive(Deserialize)]
struct RawPair {
    dark: String,
    light: String,
}

#[derive(Deserialize)]
struct RawLengths {
    tokens: Vec<RawLength>,
}

#[derive(Deserialize)]
struct RawLength {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct RawShadows {
    tokens: Vec<RawShadow>,
}

#[derive(Deserialize)]
struct RawShadow {
    name: String,
    value: RawPair,
}

#[derive(Deserialize)]
struct RawType {
    groups: Vec<RawGroup>,
}

#[derive(Deserialize)]
struct RawGroup {
    styles: Vec<RawStyle>,
}

#[derive(Deserialize)]
struct RawStyle {
    name: String,
    #[serde(rename = "fontSize")]
    font_size: String,
    #[serde(rename = "lineHeight")]
    line_height: String,
    #[serde(rename = "fontWeight")]
    font_weight: u16,
}

/// Estilo tipográfico (`body`, `caption`, `clock`…).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeToken {
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
}

/// Todos os valores do design system, indexados pelo nome do token.
#[derive(Debug, Clone)]
pub struct Tokens {
    colors: HashMap<String, (Color32, Color32)>,
    lengths: HashMap<String, f32>,
    shadows: HashMap<String, (egui::Shadow, egui::Shadow)>,
    types: HashMap<String, TypeToken>,
}

impl Tokens {
    /// Lê o JSON do design system.
    pub fn parse(json: &str) -> Result<Self, TokenError> {
        let raw: RawFile =
            serde_json::from_str(json).map_err(|e| TokenError::Json(e.to_string()))?;
        let mut colors = HashMap::new();
        for c in raw.color.tokens {
            colors.insert(
                c.name,
                (parse_hex(&c.value.dark)?, parse_hex(&c.value.light)?),
            );
        }
        let mut lengths = HashMap::new();
        for l in raw
            .spacing
            .tokens
            .into_iter()
            .chain(raw.radius.tokens)
            .chain(raw.size.tokens)
        {
            lengths.insert(l.name, parse_px(&l.value)?);
        }
        let mut shadows = HashMap::new();
        for s in raw.shadow.tokens {
            shadows.insert(
                s.name,
                (parse_shadow(&s.value.dark)?, parse_shadow(&s.value.light)?),
            );
        }
        let mut types = HashMap::new();
        for style in raw.typography.groups.into_iter().flat_map(|g| g.styles) {
            types.insert(
                style.name,
                TypeToken {
                    size: parse_px(&style.font_size)?,
                    line_height: parse_px(&style.line_height)?,
                    weight: style.font_weight,
                },
            );
        }
        Ok(Self {
            colors,
            lengths,
            shadows,
            types,
        })
    }

    /// Os tokens embutidos no binário. O JSON é conferido pelos testes; um erro aqui é de build.
    pub fn embedded() -> &'static Tokens {
        static TOKENS: OnceLock<Tokens> = OnceLock::new();
        TOKENS.get_or_init(|| {
            Tokens::parse(EMBEDDED)
                .expect("docs/design/tokens.json inválido (ver testes de ui_theme)")
        })
    }

    pub fn color(&self, name: &str, theme: ThemeKind) -> Result<Color32, TokenError> {
        let (dark, light) = self
            .colors
            .get(name)
            .ok_or_else(|| TokenError::Missing(name.to_owned()))?;
        Ok(match theme {
            ThemeKind::Dark => *dark,
            ThemeKind::Light => *light,
        })
    }

    /// Espaço, raio ou tamanho em pixels lógicos (`space-2`, `radius-m`, `control-h`…).
    pub fn length(&self, name: &str) -> Result<f32, TokenError> {
        self.lengths
            .get(name)
            .copied()
            .ok_or_else(|| TokenError::Missing(name.to_owned()))
    }

    pub fn shadow(&self, name: &str, theme: ThemeKind) -> Result<egui::Shadow, TokenError> {
        let (dark, light) = self
            .shadows
            .get(name)
            .ok_or_else(|| TokenError::Missing(name.to_owned()))?;
        Ok(match theme {
            ThemeKind::Dark => *dark,
            ThemeKind::Light => *light,
        })
    }

    pub fn text(&self, name: &str) -> Result<TypeToken, TokenError> {
        self.types
            .get(name)
            .copied()
            .ok_or_else(|| TokenError::Missing(name.to_owned()))
    }
}

/// `#rrggbb` ou `#rrggbbaa` (alfa não pré-multiplicado no JSON).
pub fn parse_hex(text: &str) -> Result<Color32, TokenError> {
    let bad = || TokenError::BadValue(text.to_owned());
    let digits = text.strip_prefix('#').ok_or_else(bad)?;
    let byte = |i: usize| {
        digits
            .get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .ok_or_else(bad)
    };
    match digits.len() {
        6 => Ok(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Ok(Color32::from_rgba_unmultiplied(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            byte(6)?,
        )),
        _ => Err(bad()),
    }
}

/// `"32px"` -> 32.0.
pub fn parse_px(text: &str) -> Result<f32, TokenError> {
    text.trim()
        .strip_suffix("px")
        .and_then(|n| n.trim().parse::<f32>().ok())
        .ok_or_else(|| TokenError::BadValue(text.to_owned()))
}

/// `"0 4px 12px rgba(0,0,0,0.45)"` (x, y, desfoque, cor; sem espalhamento).
pub fn parse_shadow(text: &str) -> Result<egui::Shadow, TokenError> {
    let bad = || TokenError::BadValue(text.to_owned());
    let (lengths, color) = text.split_once("rgba(").ok_or_else(bad)?;
    let mut parts = lengths.split_whitespace();
    let x = parse_len(parts.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let y = parse_len(parts.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let blur = parse_len(parts.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let channels: Vec<&str> = color
        .trim_end_matches(')')
        .split(',')
        .map(str::trim)
        .collect();
    let [r, g, b, a] = channels[..] else {
        return Err(bad());
    };
    let channel = |s: &str| s.parse::<u8>().map_err(|_| bad());
    let alpha: f32 = a.parse().map_err(|_| bad())?;
    Ok(egui::Shadow {
        offset: [x as i8, y as i8],
        blur: blur as u8,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(
            channel(r)?,
            channel(g)?,
            channel(b)?,
            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        ),
    })
}

fn parse_len(text: &str) -> Option<f32> {
    if text == "0" {
        return Some(0.0);
    }
    text.strip_suffix("px")?.parse().ok()
}

/// As cores de um Tema, com os nomes do glossário de tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub surface_0: Color32,
    pub surface_1: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub surface_4: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub ink: Color32,
    pub ink_muted: Color32,
    pub ink_disabled: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub on_accent: Color32,
    pub accent_subtle: Color32,
    pub accent_ink: Color32,
    pub focus_ring: Color32,
    pub info: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub scrim: Color32,
}

impl Palette {
    pub fn from_tokens(tokens: &Tokens, theme: ThemeKind) -> Result<Self, TokenError> {
        let c = |name: &str| tokens.color(name, theme);
        Ok(Self {
            surface_0: c("surface-0")?,
            surface_1: c("surface-1")?,
            surface_2: c("surface-2")?,
            surface_3: c("surface-3")?,
            surface_4: c("surface-4")?,
            border: c("border")?,
            border_strong: c("border-strong")?,
            ink: c("ink")?,
            ink_muted: c("ink-muted")?,
            ink_disabled: c("ink-disabled")?,
            accent: c("accent")?,
            accent_hover: c("accent-hover")?,
            accent_pressed: c("accent-pressed")?,
            on_accent: c("on-accent")?,
            accent_subtle: c("accent-subtle")?,
            accent_ink: c("accent-ink")?,
            focus_ring: c("focus-ring")?,
            info: c("info")?,
            success: c("success")?,
            warning: c("warning")?,
            danger: c("danger")?,
            scrim: c("scrim")?,
        })
    }

    /// Paleta embutida do Tema (calculada uma vez).
    pub fn of(theme: ThemeKind) -> &'static Palette {
        static PALETTES: OnceLock<[Palette; 2]> = OnceLock::new();
        let all = PALETTES.get_or_init(|| {
            ThemeKind::ALL.map(|t| {
                Palette::from_tokens(Tokens::embedded(), t)
                    .expect("paleta incompleta em tokens.json (ver testes de ui_theme)")
            })
        });
        match theme {
            ThemeKind::Dark => &all[0],
            ThemeKind::Light => &all[1],
        }
    }
}

/// Luminância relativa WCAG 2.x (sRGB, ignora o alfa).
#[cfg(test)]
pub fn relative_luminance(color: Color32) -> f32 {
    let lin = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(color.r()) + 0.7152 * lin(color.g()) + 0.0722 * lin(color.b())
}

/// Razão de contraste WCAG entre duas cores opacas (de 1 a 21).
#[cfg(test)]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(theme: ThemeKind) -> Palette {
        Palette::from_tokens(Tokens::embedded(), theme).expect("paleta completa")
    }

    #[test]
    fn hex_colors_parse_with_and_without_alpha() {
        assert_eq!(
            parse_hex("#141210"),
            Ok(Color32::from_rgb(0x14, 0x12, 0x10))
        );
        assert_eq!(
            parse_hex("#c8893b73"),
            Ok(Color32::from_rgba_unmultiplied(0xc8, 0x89, 0x3b, 0x73))
        );
        assert!(parse_hex("141210").is_err());
        assert!(parse_hex("#12345").is_err());
        assert!(parse_hex("#gggggg").is_err());
    }

    #[test]
    fn lengths_and_shadows_parse() {
        assert_eq!(parse_px("32px"), Ok(32.0));
        assert!(parse_px("32").is_err());
        let s = parse_shadow("0 8px 24px rgba(0,0,0,0.5)").expect("sombra");
        assert_eq!((s.offset, s.blur, s.spread), ([0, 8], 24, 0));
        assert_eq!(s.color, Color32::from_rgba_unmultiplied(0, 0, 0, 128));
    }

    #[test]
    fn embedded_tokens_have_everything_the_theme_needs() {
        let t = Tokens::embedded();
        for theme in ThemeKind::ALL {
            Palette::from_tokens(t, theme).expect("cores");
            for name in ["shadow-flyout", "shadow-floating", "shadow-dialog"] {
                t.shadow(name, theme).expect(name);
            }
        }
        for name in [
            "space-05",
            "space-1",
            "space-2",
            "space-3",
            "space-4",
            "space-6",
            "radius-s",
            "radius-m",
            "radius-l",
            "radius-xl",
            "control-h",
            "control-h-compact",
            "icon-s",
        ] {
            t.length(name).expect(name);
        }
        for name in ["caption", "body", "body-strong", "subtitle", "title", "log"] {
            t.text(name).expect(name);
        }
        assert_eq!(t.length("control-h"), Ok(32.0));
        assert_eq!(t.length("control-h-compact"), Ok(24.0));
        assert_eq!(t.text("body").map(|b| b.size), Ok(14.0));
    }

    #[test]
    fn an_unknown_token_is_an_error_not_a_panic() {
        assert_eq!(
            Tokens::embedded().color("nao-existe", ThemeKind::Dark),
            Err(TokenError::Missing("nao-existe".to_owned()))
        );
    }

    #[test]
    fn contrast_ratio_matches_known_values() {
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-4);
    }

    /// Texto: 4,5:1 nos pares que o design system promete (README, "Cor").
    #[test]
    fn text_pairs_reach_4_5_to_1_in_both_themes() {
        for theme in ThemeKind::ALL {
            let p = palette(theme);
            let mut pairs = vec![];
            for (name, surface) in [
                ("surface-0", p.surface_0),
                ("surface-1", p.surface_1),
                ("surface-2", p.surface_2),
                ("surface-3", p.surface_3),
                ("surface-4", p.surface_4),
            ] {
                pairs.push((format!("ink/{name}"), p.ink, surface));
            }
            for (name, surface) in [("surface-1", p.surface_1), ("surface-2", p.surface_2)] {
                pairs.push((format!("ink-muted/{name}"), p.ink_muted, surface));
                pairs.push((format!("accent-ink/{name}"), p.accent_ink, surface));
                for (state, fg) in [
                    ("info", p.info),
                    ("success", p.success),
                    ("warning", p.warning),
                    ("danger", p.danger),
                ] {
                    pairs.push((format!("{state}/{name}"), fg, surface));
                }
            }
            pairs.push((
                "accent-ink/accent-subtle".into(),
                p.accent_ink,
                p.accent_subtle,
            ));
            pairs.push(("ink/accent-subtle".into(), p.ink, p.accent_subtle));
            for (name, bg) in [
                ("accent", p.accent),
                ("accent-hover", p.accent_hover),
                ("accent-pressed", p.accent_pressed),
            ] {
                pairs.push((format!("on-accent/{name}"), p.on_accent, bg));
            }
            for (name, fg, bg) in pairs {
                let ratio = contrast_ratio(fg, bg);
                assert!(ratio >= 4.5, "{theme:?} {name}: {ratio:.2}:1 < 4,5:1");
            }
        }
    }

    /// Contornos de controle e anel de foco: 3:1.
    #[test]
    fn borders_and_focus_ring_reach_3_to_1_in_both_themes() {
        for theme in ThemeKind::ALL {
            let p = palette(theme);
            for (name, surface) in [("surface-1", p.surface_1), ("surface-2", p.surface_2)] {
                let ratio = contrast_ratio(p.border_strong, surface);
                assert!(ratio >= 3.0, "{theme:?} border-strong/{name}: {ratio:.2}:1");
            }
            for (name, surface) in [
                ("surface-0", p.surface_0),
                ("surface-1", p.surface_1),
                ("surface-2", p.surface_2),
                ("surface-3", p.surface_3),
                ("surface-4", p.surface_4),
            ] {
                let ratio = contrast_ratio(p.focus_ring, surface);
                assert!(ratio >= 3.0, "{theme:?} focus-ring/{name}: {ratio:.2}:1");
            }
        }
    }
}
