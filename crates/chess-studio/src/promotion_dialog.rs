//! Seletor de promoção (#336): as quatro peças desenhadas, com a Dama já selecionada.
//!
//! Enter confirma a selecionada, ← → mudam a seleção e D, T, B e C escolhem na hora (Dama, Torre,
//! Bispo, Cavalo). O clique continua valendo e Esc segue com o intérprete de gestos (retrata o
//! Lance, DEC-40). A escolha vira o mesmo `Command::ChoosePromotion` de sempre: este módulo só
//! apresenta e lê teclas, não decide nada da Partida (ADR-0001), e Enter não é "dama
//! automática": nas Políticas competitivas continua sendo uma escolha explícita (DEC-40).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use chess_core::{Color as Side, Piece};
use chess_game::Command;

use crate::i18n::I18n;
use crate::session::Session;
use crate::ui_theme::palette;

/// As peças oferecidas, na ordem em que aparecem (a Dama primeiro, é a padrão).
pub const CHOICES: [Piece; 4] = [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];

/// Uma entrada do teclado que o seletor entende.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Left,
    Right,
    /// Enter: confirma a peça selecionada.
    Confirm,
    /// Letra de uma peça: escolhe na hora.
    Pick(Piece),
}

/// A entrada que uma tecla representa.
pub fn input_for_key(key: egui::Key) -> Option<Input> {
    match key {
        egui::Key::ArrowLeft => Some(Input::Left),
        egui::Key::ArrowRight => Some(Input::Right),
        egui::Key::Enter => Some(Input::Confirm),
        egui::Key::D => Some(Input::Pick(Piece::Queen)),
        egui::Key::T => Some(Input::Pick(Piece::Rook)),
        egui::Key::B => Some(Input::Pick(Piece::Bishop)),
        egui::Key::C => Some(Input::Pick(Piece::Knight)),
        _ => None,
    }
}

/// Teclas que o seletor consome enquanto está aberto.
const KEYS: [egui::Key; 7] = [
    egui::Key::ArrowLeft,
    egui::Key::ArrowRight,
    egui::Key::Enter,
    egui::Key::D,
    egui::Key::T,
    egui::Key::B,
    egui::Key::C,
];

/// Seleção do seletor: volta à Dama a cada promoção nova.
#[derive(Resource, Debug, Default)]
pub struct PromotionUi {
    selected: usize,
}

impl PromotionUi {
    /// A peça selecionada.
    pub fn selected(&self) -> Piece {
        CHOICES[self.selected]
    }

    /// Promoção nova (ou fim da anterior): a Dama volta a ser a selecionada.
    pub fn reset(&mut self) {
        self.selected = 0;
    }

    /// Aplica uma entrada. Devolve a peça escolhida quando a entrada decide (Enter ou letra).
    /// ← e → andam em círculo.
    pub fn apply(&mut self, input: Input) -> Option<Piece> {
        let n = CHOICES.len();
        match input {
            Input::Left => {
                self.selected = (self.selected + n - 1) % n;
                None
            }
            Input::Right => {
                self.selected = (self.selected + 1) % n;
                None
            }
            Input::Confirm => Some(self.selected()),
            Input::Pick(piece) => {
                self.selected = CHOICES.iter().position(|p| *p == piece).unwrap_or(0);
                Some(piece)
            }
        }
    }
}

fn piece_keys(piece: Piece) -> (&'static str, &'static str) {
    match piece {
        Piece::Rook => ("promotion-rook", "promotion-rook-key"),
        Piece::Bishop => ("promotion-bishop", "promotion-bishop-key"),
        Piece::Knight => ("promotion-knight", "promotion-knight-key"),
        _ => ("promotion-queen", "promotion-queen-key"),
    }
}

/// Lado de cada cartão com a peça, em pontos.
const TILE: f32 = 76.0;

/// Um cartão: a peça desenhada no fundo do Tema, com o realce da seleção.
fn tile(ui: &mut egui::Ui, side: Side, piece: Piece, selected: bool) -> egui::Response {
    let colors = palette(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(TILE, TILE), egui::Sense::click());
    let fill = if selected {
        colors.accent_subtle
    } else if response.hovered() {
        colors.surface_3
    } else {
        colors.surface_2
    };
    let stroke = if selected {
        egui::Stroke::new(3.0, colors.accent)
    } else {
        egui::Stroke::new(1.0, colors.border)
    };
    let painter = ui.painter();
    painter.rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
    crate::board2d_panel::paint_piece(painter, rect.shrink(4.0), side, piece, 1.0, None);
    response
}

/// Seletor de promoção, aberto enquanto a Partida tem promoção pendente.
pub fn promotion_selector(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
    mut state: ResMut<PromotionUi>,
) -> Result {
    let Some(pending) = session.promotion_pending() else {
        state.reset();
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let mut choice = None;
    // No Modo de estudo não há arbitragem: o seletor é o do Assistido (#322).
    let competitive = session.is_competitive() && !session.studying();
    let mut always_queen = session.always_queen();
    // Teclas só valem sem campo de texto em foco.
    if !ctx.egui_wants_keyboard_input() {
        for key in KEYS {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
                && let Some(input) = input_for_key(key)
            {
                choice = choice.or(state.apply(input));
            }
        }
    }
    egui::Window::new(i18n.t("promotion-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("promotion-prompt"));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for piece in CHOICES {
                    let (name_key, letter_key) = piece_keys(piece);
                    ui.vertical(|ui| {
                        ui.set_width(TILE);
                        let selected = state.selected() == piece;
                        let response =
                            tile(ui, pending.side, piece, selected).on_hover_text(i18n.t(name_key));
                        if response.clicked() {
                            choice = Some(piece);
                        }
                        ui.vertical_centered(|ui| {
                            ui.label(i18n.t(name_key));
                            ui.small(i18n.t(letter_key));
                        });
                    });
                }
            });
            ui.add_space(4.0);
            ui.small(i18n.t("promotion-keys-hint"));
            if competitive {
                // Sem dama automática nas Políticas competitivas (DEC-40).
                ui.small(i18n.t("promotion-competitive-hint"));
            } else {
                ui.checkbox(&mut always_queen, i18n.t("promotion-always-queen"));
                ui.small(i18n.t("promotion-cancel-hint"));
            }
        });
    if always_queen != session.always_queen() {
        session.set_always_queen(always_queen);
    } else if let Some(piece) = choice {
        session.command(Command::ChoosePromotion { piece });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queen_starts_selected_and_enter_confirms_it() {
        let mut ui = PromotionUi::default();
        assert_eq!(ui.selected(), Piece::Queen);
        assert_eq!(ui.apply(Input::Confirm), Some(Piece::Queen));
    }

    #[test]
    fn arrows_walk_the_pieces_in_a_circle_and_enter_confirms_the_selected_one() {
        let mut ui = PromotionUi::default();
        assert_eq!(ui.apply(Input::Right), None);
        assert_eq!(ui.selected(), Piece::Rook);
        ui.apply(Input::Right);
        ui.apply(Input::Right);
        assert_eq!(ui.selected(), Piece::Knight);
        // Depois do Cavalo volta à Dama; antes da Dama vem o Cavalo.
        ui.apply(Input::Right);
        assert_eq!(ui.selected(), Piece::Queen);
        ui.apply(Input::Left);
        assert_eq!(ui.selected(), Piece::Knight);
        assert_eq!(ui.apply(Input::Confirm), Some(Piece::Knight));
    }

    #[test]
    fn a_piece_letter_chooses_at_once() {
        let mut ui = PromotionUi::default();
        for (key, piece) in [
            (egui::Key::D, Piece::Queen),
            (egui::Key::T, Piece::Rook),
            (egui::Key::B, Piece::Bishop),
            (egui::Key::C, Piece::Knight),
        ] {
            let input = input_for_key(key).unwrap();
            assert_eq!(ui.apply(input), Some(piece));
            assert_eq!(ui.selected(), piece);
        }
    }

    #[test]
    fn other_keys_do_nothing_and_a_new_promotion_goes_back_to_the_queen() {
        assert_eq!(input_for_key(egui::Key::A), None);
        assert_eq!(
            input_for_key(egui::Key::Escape),
            None,
            "Esc é do intérprete"
        );
        let mut ui = PromotionUi::default();
        ui.apply(Input::Right);
        ui.reset();
        assert_eq!(ui.selected(), Piece::Queen);
    }

    #[test]
    fn every_key_the_dialog_consumes_means_something() {
        for key in KEYS {
            assert!(input_for_key(key).is_some(), "{key:?}");
        }
    }
}
