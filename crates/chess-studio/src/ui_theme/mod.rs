//! Tema do design system em egui (DS-1, issue #160; ADR-0005): tokens -> `Style`/`Visuals`,
//! fontes Inter e JetBrains Mono, ícones Phosphor, Densidade, Escala e os widgets do projeto.
//! A fonte de verdade dos valores é `docs/design/tokens.json` (`tokens`).
//!
//! Só Apresentação: nada aqui toca a Partida (ADR-0001).

mod appearance;
mod fonts;
mod gallery;
mod style;
mod themes;
mod tokens;
mod widgets;

pub use appearance::{Appearance, Density, ThemeMode, UiScale, resolve_theme};
#[cfg(test)]
pub(crate) use fonts::definitions as font_definitions;
pub use gallery::GalleryMode;
// API para a galeria de Temas (ticket seguinte) e para os componentes com estilos novos.
#[allow(unused_imports)]
pub use style::{BODY_LARGE, RIBBON_LABEL, styled_text};
#[allow(unused_imports)]
pub use themes::{
    ContrastLevel, THEME_COLOR_NAMES, ThemeCatalog, ThemeId, ThemeInfo, ThemeSwatch, all_themes,
    themes_by_family,
};
pub use tokens::{Palette, ThemeKind, Tokens};
// Os widgets são a API do tema para os próximos tickets (Ribbon, Opções): o app ainda não usa todos.
#[allow(unused_imports)]
pub use widgets::*;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowTheme};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

pub struct UiThemePlugin;

impl Plugin for UiThemePlugin {
    fn build(&self, app: &mut App) {
        // `init_resource` preserva a aparência restaurada do banco (persistence).
        app.init_resource::<Appearance>().add_systems(
            EguiPrimaryContextPass,
            gallery::gallery_window.after(apply_appearance),
        );
    }
}

/// Fundo da cena (3D e 2D técnico): o `surface-0` do Tema efetivo.
pub fn scene_background(mode: ThemeMode, system_dark: Option<bool>) -> Color {
    let c = Palette::of_theme(resolve_theme(mode, system_dark)).surface_0;
    Color::srgba_u8(c.r(), c.g(), c.b(), c.a())
}

/// Tema do sistema operacional, quando o Bevy sabe qual é.
fn system_dark(window: Option<&Window>) -> Option<bool> {
    match window?.window_theme? {
        WindowTheme::Dark => Some(true),
        WindowTheme::Light => Some(false),
    }
}

/// Estado do instalador (fontes, estilos e Escala).
#[derive(Debug, Default)]
pub struct ApplyState {
    pub(crate) fonts_requested: bool,
    /// Tema e Densidade já instalados nos estilos do egui.
    applied: Option<(ThemeId, Density)>,
}

/// Instala fontes e estilos e aplica a Escala. Roda antes de qualquer painel, no passe do egui.
pub fn apply_appearance(
    mut contexts: EguiContexts,
    appearance: Res<Appearance>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: Local<ApplyState>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    apply_to_context(
        ctx,
        &appearance,
        system_dark(windows.single().ok()),
        &mut state,
    );
    Ok(())
}

/// O mesmo instalador para qualquer contexto egui (as janelas destacadas, #168, têm o seu).
/// `system_dark` é o tema do sistema, se conhecido.
pub fn apply_to_context(
    ctx: &egui::Context,
    appearance: &Appearance,
    system_dark: Option<bool>,
    state: &mut ApplyState,
) {
    if !state.fonts_requested {
        // As fontes só valem no passe seguinte; os estilos que as usam esperam por ele.
        ctx.set_fonts(fonts::definitions());
        // A Escala é nossa (Ações UiZoomIn e UiZoomOut, em degraus); o atalho de zoom do egui sai.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        state.fonts_requested = true;
        return;
    }
    widgets::mark_fonts_ready(ctx);
    widgets::track_keyboard_focus(ctx);

    let theme = resolve_theme(appearance.theme, system_dark);
    if state.applied != Some((theme, appearance.density)) {
        let tokens = Tokens::embedded();
        // Cada base guarda o seu estilo: os de fábrica, e por cima o do Tema escolhido na base dele.
        for kind in ThemeKind::ALL {
            let id = if kind == theme.base() {
                theme
            } else {
                ThemeId::factory(kind)
            };
            ctx.set_style_of(
                kind.egui(),
                style::build_style(id, appearance.density, tokens),
            );
        }
        ctx.set_theme(match theme.base() {
            ThemeKind::Dark => egui::ThemePreference::Dark,
            ThemeKind::Light => egui::ThemePreference::Light,
        });
        state.applied = Some((theme, appearance.density));
    }
    ctx.set_zoom_factor(appearance.scale.factor());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color) -> [u8; 3] {
        let s = c.to_srgba();
        [s.red, s.green, s.blue].map(|v| (v * 255.0).round() as u8)
    }

    #[test]
    fn the_scene_background_is_the_effective_themes_surface_0() {
        for key in ["feltro", "torneio", "dark", "light"] {
            let id = ThemeId::from_key(key).expect("tema");
            let p = Palette::of_theme(id).surface_0;
            assert_eq!(
                rgb(scene_background(ThemeMode::Fixed(id), None)),
                [p.r(), p.g(), p.b()],
                "{key}"
            );
        }
    }

    #[test]
    fn feltro_and_torneio_have_different_backgrounds_and_the_system_one_follows_the_os() {
        let bg = |key: &str| {
            rgb(scene_background(
                ThemeMode::Fixed(ThemeId::from_key(key).expect("tema")),
                Some(true),
            ))
        };
        assert_ne!(bg("feltro"), bg("torneio"));
        assert_ne!(
            rgb(scene_background(ThemeMode::System, Some(true))),
            rgb(scene_background(ThemeMode::System, Some(false)))
        );
    }
}
