//! Fontes embutidas (ADR-0004): Inter (interface) e JetBrains Mono (log UCI e FEN), as duas
//! SIL OFL 1.1, e os ícones Phosphor (MIT OR Apache-2.0). As fontes embutidas do egui usam a
//! Ubuntu Font Licence, fora da allowlist, e por isso não são ativadas.
//! Licenças e origem: `assets/LICENSES.md`.

use bevy_egui::egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Família do rótulo em negrito (Inter SemiBold): `FontFamily::Name`.
pub const SEMIBOLD: &str = "inter-semibold";

/// Família só de ícones Phosphor.
pub const ICONS: &str = "icons";

const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
const NOTO_SANS: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const NOTO_CHESS: &[u8] = include_bytes!("../../assets/fonts/NotoSansSymbols2-chess.ttf");

fn insert(fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]) {
    fonts
        .font_data
        .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
}

/// Definições de fonte do app.
///
/// Ordem de busca de um glifo: a fonte do texto, os ícones Phosphor, o Noto Sans (cobertura
/// extra de alfabetos) e os glifos de xadrez U+2654–U+265F (bandeja do editor de posição, C11).
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    insert(&mut fonts, "inter", INTER_REGULAR);
    insert(&mut fonts, SEMIBOLD, INTER_SEMIBOLD);
    insert(&mut fonts, "jetbrains-mono", JETBRAINS_MONO);
    insert(&mut fonts, "noto-sans", NOTO_SANS);
    insert(&mut fonts, "noto-chess", NOTO_CHESS);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // Ícones numa família própria: a Inter tem glifos próprios em parte da faixa privada
    // (U+E000) e esconderia alguns ícones; já a Phosphor tem "letras" em branco, e por isso não
    // pode ficar à frente do texto. Use `widgets::icon_font` para os ícones.
    let family = |first: &str| {
        [first, "noto-sans", "noto-chess"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("inter"));
    fonts
        .families
        .insert(FontFamily::Monospace, family("jetbrains-mono"));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), family(SEMIBOLD));
    fonts
        .families
        .insert(FontFamily::Name(ICONS.into()), vec!["phosphor".to_owned()]);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interface_text_uses_inter_and_the_log_jetbrains_mono() {
        let fonts = definitions();
        assert_eq!(fonts.families[&FontFamily::Proportional][0], "inter");
        assert_eq!(fonts.families[&FontFamily::Monospace][0], "jetbrains-mono");
        assert_eq!(
            fonts.families[&FontFamily::Name(SEMIBOLD.into())][0],
            SEMIBOLD
        );
    }

    #[test]
    fn the_icon_family_is_phosphor_and_every_listed_font_exists() {
        let fonts = definitions();
        assert_eq!(
            fonts.families[&FontFamily::Name(ICONS.into())],
            ["phosphor"]
        );
        for list in fonts.families.values() {
            for name in list {
                assert!(fonts.font_data.contains_key(name), "{name} não embutida");
            }
        }
    }

    #[test]
    fn egui_default_fonts_are_not_used() {
        // Ubuntu-Light e afins (Ubuntu Font Licence) não podem entrar (ADR-0004).
        let fonts = definitions();
        assert!(fonts.font_data.keys().all(|n| !n.contains("Ubuntu")));
        assert_eq!(fonts.font_data.len(), 6);
    }
}
