//! Reorganização responsiva (C4, UI-10): função **pura** que, dado o tamanho da janela (em
//! pontos do egui), decide onde fica o dock e se a interface se compacta (barra de essenciais). Só
//! Apresentação (ADR-0001): nada aqui toca a Partida.
//!
//! Regras:
//! - largura >= [`SIDE_FROM`]: dock à direita, como sempre;
//! - [`OVERLAY_BELOW`] <= largura < [`SIDE_FROM`]: dock à direita, mais estreito, e a
//!   interface compacta;
//! - largura < [`OVERLAY_BELOW`]: o dock vira uma sobreposição recolhível sobre o tabuleiro
//!   (o tabuleiro ocupa a janela toda);
//! - sempre que compacta, a barra de essenciais (Nova Partida, Painéis e, no 2D, o relógio
//!   com o botão de acioná-lo) fica à vista; promoção e "Sair do Foco" já são janelas/áreas
//!   ancoradas e restritas à tela.

use crate::aids::AidSettings;
use crate::capture::CaptureMode;
use crate::clock::{clock_text, side_of_slot};
use crate::dock::{Dock, MAX_WIDTH, MIN_WIDTH};
use crate::focus::ViewState;
use crate::i18n::I18n;
use crate::new_game::NewGameDialog;
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;

/// Abaixo desta largura o dock é sobreposição recolhível.
pub const OVERLAY_BELOW: f32 = 720.0;
/// A partir desta largura a interface é a "grande" (dock largo).
pub const SIDE_FROM: f32 = 1100.0;
/// Abaixo desta altura a interface também compacta.
pub const COMPACT_HEIGHT: f32 = 700.0;
/// Largura mínima garantida ao tabuleiro quando o dock fica ao lado.
pub const MIN_BOARD_WIDTH: f32 = 400.0;

/// Onde o dock é desenhado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockPlacement {
    /// Coluna à direita; o tabuleiro ocupa o resto.
    Side { width: f32 },
    /// Sobre o tabuleiro, só quando aberto pelo botão "Painéis".
    Overlay { width: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponsiveLayout {
    pub dock: DockPlacement,
    /// A interface se compacta.
    pub compact: bool,
    /// Barra de essenciais sempre visível (Nova Partida, Painéis, relógio no 2D).
    pub essentials_bar: bool,
}

impl ResponsiveLayout {
    /// Largura que o dock tira do tabuleiro (zero na sobreposição).
    #[cfg(test)]
    pub fn board_inset(&self) -> f32 {
        match self.dock {
            DockPlacement::Side { width } => width,
            DockPlacement::Overlay { .. } => 0.0,
        }
    }
}

/// Layout para uma janela de `window` pontos; `wanted_dock_width` é a largura que o usuário
/// escolheu arrastando a borda do dock.
pub fn responsive_layout(window: Vec2, wanted_dock_width: f32) -> ResponsiveLayout {
    let compact = window.x < SIDE_FROM || window.y < COMPACT_HEIGHT;
    let dock = if window.x < OVERLAY_BELOW {
        DockPlacement::Overlay {
            width: (window.x * 0.85).clamp(MIN_WIDTH, MAX_WIDTH).min(window.x),
        }
    } else {
        let share = if window.x < SIDE_FROM { 0.32 } else { 0.6 };
        let cap = (window.x * share)
            .min(window.x - MIN_BOARD_WIDTH)
            .clamp(MIN_WIDTH, MAX_WIDTH);
        DockPlacement::Side {
            width: wanted_dock_width.clamp(MIN_WIDTH, cap),
        }
    };
    ResponsiveLayout {
        dock,
        compact,
        essentials_bar: compact,
    }
}

pub struct ResponsivePlugin;

impl Plugin for ResponsivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, essentials_bar);
    }
}

/// Barra de essenciais das janelas pequenas, sempre sobre o dock:
/// Nova Partida, "Painéis" (abre/fecha a sobreposição) e, no 2D técnico (sem relógio na
/// mesa), o relógio com o botão de acioná-lo. No modo Foco quem cuida disso é `focus`.
#[allow(clippy::too_many_arguments)]
fn essentials_bar(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    view_state: Res<ViewState>,
    presentation: Res<Presentation>,
    capture: Option<Res<CaptureMode>>,
    aids: Res<AidSettings>,
    mut session: ResMut<Session>,
    mut dialog: ResMut<NewGameDialog>,
    mut dock: ResMut<Dock>,
) -> Result {
    if view_state.focus || capture.as_ref().is_some_and(|c| !c.show_panel) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let layout = responsive_layout(
        Vec2::new(screen.width(), screen.height()),
        dock.layout.width,
    );
    if !layout.essentials_bar {
        return Ok(());
    }
    let overlay = matches!(layout.dock, DockPlacement::Overlay { .. });
    let mut open_overlay = dock.overlay_open;
    egui::Area::new(egui::Id::new("essentials-bar"))
        .order(egui::Order::Foreground)
        // Acima da barra de status, que reserva a faixa inferior.
        .anchor(
            egui::Align2::LEFT_BOTTOM,
            [8.0, -8.0 - crate::status_bar::bottom_inset(ctx)],
        )
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("button-new-game")).clicked() {
                        dialog.open_from(session.config(), &aids);
                    }
                    if overlay {
                        ui.toggle_value(&mut open_overlay, i18n.t("dock-heading"));
                    }
                    if !presentation.is_3d() {
                        clock_row(ui, &i18n, &mut session);
                    }
                });
            });
        });
    if open_overlay != dock.overlay_open {
        dock.overlay_open = open_overlay;
    }
    Ok(())
}

/// Relógio em uma linha: os dois mostradores e o botão de acioná-lo.
fn clock_row(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session) {
    let Some(reading) = session.clock_reading() else {
        return;
    };
    let user = session.user_side();
    for slot in 0..2 {
        let side = side_of_slot(user, slot);
        let running = reading.running == Some(side) && !reading.paused;
        let text = format!(
            "{} {}",
            i18n.t(match side {
                Side::White => "clock-white",
                Side::Black => "clock-black",
            }),
            clock_text(reading.remaining(side))
        );
        let text = egui::RichText::new(text).monospace();
        ui.label(if running { text.strong() } else { text });
    }
    let side_to_move = session.snapshot().side_to_move;
    if ui
        .add_enabled(
            session.can_press_clock(side_to_move),
            egui::Button::new(i18n.t("clock-press")),
        )
        .clicked()
    {
        session.press_clock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(w: f32, h: f32) -> ResponsiveLayout {
        responsive_layout(Vec2::new(w, h), 340.0)
    }

    #[test]
    fn full_hd_keeps_the_side_dock_and_the_full_main_window() {
        let l = layout(1920.0, 1080.0);
        assert_eq!(l.dock, DockPlacement::Side { width: 340.0 });
        assert!(!l.compact);
        assert!(!l.essentials_bar);
    }

    #[test]
    fn hd_720p_is_still_the_regular_layout() {
        let l = layout(1280.0, 720.0);
        assert_eq!(l.dock, DockPlacement::Side { width: 340.0 });
        assert!(!l.compact);
    }

    #[test]
    fn xga_gets_a_narrower_dock_and_a_compact_main_window() {
        let l = layout(1024.0, 768.0);
        let DockPlacement::Side { width } = l.dock else {
            panic!("dock ao lado em 1024x768");
        };
        assert!(width < 340.0 && (MIN_WIDTH..).contains(&width), "{width}");
        assert!(l.compact && l.essentials_bar);
        assert!(1024.0 - width >= MIN_BOARD_WIDTH);
    }

    #[test]
    fn svga_800x600_compacts_and_leaves_room_for_the_board() {
        let l = layout(800.0, 600.0);
        assert!(l.compact && l.essentials_bar);
        assert!(800.0 - l.board_inset() >= MIN_BOARD_WIDTH);
    }

    #[test]
    fn a_very_narrow_window_turns_the_dock_into_an_overlay_over_the_whole_board() {
        let l = layout(600.0, 500.0);
        let DockPlacement::Overlay { width } = l.dock else {
            panic!("sobreposição em 600x500");
        };
        assert!((MIN_WIDTH..=600.0).contains(&width));
        assert_eq!(l.board_inset(), 0.0);
        assert!(l.compact && l.essentials_bar);
    }

    #[test]
    fn a_short_window_compacts_even_when_wide() {
        let l = layout(1600.0, 600.0);
        assert!(l.compact && l.essentials_bar);
        assert!(matches!(l.dock, DockPlacement::Side { .. }));
    }

    #[test]
    fn essentials_are_always_reachable_at_every_size() {
        for w in (320..=2560).step_by(40) {
            for h in (240..=1440).step_by(60) {
                let (w, h) = (w as f32, h as f32);
                let l = layout(w, h);
                // Compacto implica barra de essenciais.
                assert_eq!(l.compact, l.essentials_bar, "{w}x{h}");
                match l.dock {
                    DockPlacement::Side { width } => {
                        assert!((MIN_WIDTH..=MAX_WIDTH).contains(&width), "{w}x{h}: {width}");
                        assert!(w - width >= MIN_BOARD_WIDTH, "{w}x{h}: {width}");
                    }
                    DockPlacement::Overlay { width } => {
                        assert!(width <= w, "{w}x{h}: {width}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_chosen_width_is_respected_when_there_is_room() {
        let l = responsive_layout(Vec2::new(1920.0, 1080.0), 500.0);
        assert_eq!(l.dock, DockPlacement::Side { width: 500.0 });
        let absurd = responsive_layout(Vec2::new(1920.0, 1080.0), 5000.0);
        assert_eq!(absurd.dock, DockPlacement::Side { width: MAX_WIDTH });
    }
}
