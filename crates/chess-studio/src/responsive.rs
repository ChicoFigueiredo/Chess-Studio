//! Reorganização responsiva (C4, UI-10): função **pura** que, dado o tamanho da janela (em
//! pontos do egui), decide onde fica o dock e se a janela principal se compacta. Só
//! Apresentação (ADR-0001): nada aqui toca a Partida.
//!
//! Regras:
//! - largura >= [`SIDE_FROM`]: dock à direita, como sempre;
//! - [`OVERLAY_BELOW`] <= largura < [`SIDE_FROM`]: dock à direita, mais estreito, e a
//!   janela principal compacta;
//! - largura < [`OVERLAY_BELOW`]: o dock vira uma sobreposição recolhível sobre o tabuleiro
//!   (o tabuleiro ocupa a janela toda);
//! - sempre que compacta, a barra de essenciais (Nova Partida, Painéis e, no 2D, o relógio
//!   com o botão de acioná-lo) fica à vista; promoção e "Sair do Foco" já são janelas/áreas
//!   ancoradas e restritas à tela.

use bevy::prelude::*;

/// Abaixo desta largura o dock é sobreposição recolhível.
pub const OVERLAY_BELOW: f32 = 720.0;
/// A partir desta largura a interface é a "grande" (dock largo, janela principal completa).
pub const SIDE_FROM: f32 = 1100.0;
/// Abaixo desta altura a janela principal também compacta.
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
    /// A janela principal mostra só o essencial (sem textos de ajuda).
    pub compact: bool,
    /// Barra de essenciais sempre visível (Nova Partida, Painéis, relógio no 2D).
    pub essentials_bar: bool,
    /// Altura máxima da janela principal (rola por dentro; nunca passa da tela).
    pub main_window_max_height: f32,
}

impl ResponsiveLayout {
    /// Largura que o dock tira do tabuleiro (zero na sobreposição).
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
    let _ = (window, wanted_dock_width);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::{MAX_WIDTH, MIN_WIDTH};

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
        assert!(width < 340.0 && width >= MIN_WIDTH, "{width}");
        assert!(l.compact && l.essentials_bar);
        assert!(1024.0 - width >= MIN_BOARD_WIDTH);
    }

    #[test]
    fn svga_800x600_compacts_and_leaves_room_for_the_board() {
        let l = layout(800.0, 600.0);
        assert!(l.compact && l.essentials_bar);
        assert!(800.0 - l.board_inset() >= MIN_BOARD_WIDTH);
        assert!(l.main_window_max_height <= 600.0);
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
                // Compacto implica barra de essenciais; a janela principal cabe na tela.
                assert_eq!(l.compact, l.essentials_bar, "{w}x{h}");
                assert!(l.main_window_max_height <= h, "{w}x{h}");
                assert!(l.main_window_max_height > 0.0, "{w}x{h}");
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
