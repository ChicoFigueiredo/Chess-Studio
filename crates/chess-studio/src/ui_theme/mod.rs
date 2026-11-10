//! Tema do design system em egui (DS-1, issue #160; ADR-0005): tokens -> `Style`/`Visuals`,
//! fontes Inter e JetBrains Mono, ícones Phosphor, Densidade, Escala e os widgets do projeto.
//! A fonte de verdade dos valores é `docs/design/tokens.json` (`tokens`).
//!
//! Só Apresentação: nada aqui toca a Partida (ADR-0001).

mod appearance;
mod fonts;
mod gallery;
mod style;
mod tokens;
mod widgets;

pub use appearance::{Appearance, Density, ThemeMode, UiScale, resolve_theme};
pub use gallery::GalleryMode;
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

/// Tema do sistema operacional, quando o Bevy sabe qual é.
fn system_dark(window: Option<&Window>) -> Option<bool> {
    match window?.window_theme? {
        WindowTheme::Dark => Some(true),
        WindowTheme::Light => Some(false),
    }
}

/// Estado do instalador (fontes, estilos e Escala).
#[derive(Default)]
pub struct ApplyState {
    fonts_requested: bool,
    /// Tema e Densidade já instalados nos estilos do egui.
    applied: Option<(ThemeKind, Density)>,
}

/// Instala fontes e estilos e aplica a Escala. Roda antes de qualquer painel, no passe do egui.
pub fn apply_appearance(
    mut contexts: EguiContexts,
    appearance: Res<Appearance>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: Local<ApplyState>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !state.fonts_requested {
        // As fontes só valem no passe seguinte; os estilos que as usam esperam por ele.
        ctx.set_fonts(fonts::definitions());
        // A Escala é nossa (Ações UiZoomIn e UiZoomOut, em degraus); o atalho de zoom do egui sai.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        state.fonts_requested = true;
        return Ok(());
    }
    widgets::mark_fonts_ready(ctx);
    widgets::track_keyboard_focus(ctx);

    let theme = resolve_theme(appearance.theme, system_dark(windows.single().ok()));
    if state.applied != Some((theme, appearance.density)) {
        let tokens = Tokens::embedded();
        for kind in ThemeKind::ALL {
            ctx.set_style_of(
                kind.egui(),
                style::build_style(kind, appearance.density, tokens),
            );
        }
        ctx.set_theme(match theme {
            ThemeKind::Dark => egui::ThemePreference::Dark,
            ThemeKind::Light => egui::ThemePreference::Light,
        });
        state.applied = Some((theme, appearance.density));
    }
    ctx.set_zoom_factor(appearance.scale.factor());
    Ok(())
}
