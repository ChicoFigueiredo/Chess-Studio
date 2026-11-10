//! Painel Notação (UI-07): a lista de Lances em SAN é uma **leitura** do histórico legal
//! da Partida (`Snapshot::history`). Lances ilegais nunca entram nela, e clicar num Lance
//! só navega na revisão (`Session::set_review_ply`): nenhum Comando de Partida é emitido
//! (ADR-0001).

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::Color as Side;
use chess_game::{GameResult, HistoryEntry};

/// Um Lance (meio-lance) na lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ply: usize,
    pub san: String,
}

/// Uma linha da notação: número do lance completo, Lance das brancas e das pretas.
/// `white` é `None` quando a lista começa por um Lance das pretas (Posição por FEN).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub number: u16,
    pub white: Option<Cell>,
    pub black: Option<Cell>,
}

/// Agrupa o histórico em linhas numeradas a partir de `first_number` (o número do lance
/// completo da Posição inicial; 1 numa Partida normal).
pub fn rows(history: &[HistoryEntry], first_number: u16) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut number = first_number;
    for entry in history {
        let cell = Cell {
            ply: entry.ply,
            san: entry.san.clone(),
        };
        match entry.side {
            Side::White => out.push(Row {
                number,
                white: Some(cell),
                black: None,
            }),
            Side::Black => {
                match out.last_mut() {
                    Some(row) if row.number == number && row.black.is_none() => {
                        row.black = Some(cell);
                    }
                    _ => out.push(Row {
                        number,
                        white: None,
                        black: Some(cell),
                    }),
                }
                number = number.saturating_add(1);
            }
        }
    }
    out
}

/// Placar curto do resultado (`1-0`, `0-1`, `1/2-1/2`).
pub fn score(result: GameResult) -> &'static str {
    match result {
        GameResult::WhiteWins => "1-0",
        GameResult::BlackWins => "0-1",
        GameResult::Draw => "1/2-1/2",
    }
}

/// Meio-lance mostrado como atual: o da revisão, ou o último da Partida.
pub fn current_ply(session: &Session) -> usize {
    session
        .review()
        .map_or(session.snapshot().history.len(), |r| r.ply)
}

/// Navega para um Lance. O último Lance volta à Partida ao vivo (sai da revisão).
pub fn navigate(session: &mut Session, ply: usize) {
    let total = session.snapshot().history.len();
    session.set_review_ply(if ply >= total { None } else { Some(ply) });
}

pub struct NotationPlugin;

impl Plugin for NotationPlugin {
    fn build(&self, app: &mut App) {
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::NOTATION,
                label_key: "dock-panel-notation",
                access: PanelAccess::Always,
                ui: |ui, i18n, world| {
                    let hidden = world
                        .get_resource::<crate::scoresheet::ScoresheetUi>()
                        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
                    if let Some(mut session) = world.get_resource_mut::<Session>() {
                        self::ui(ui, i18n, &mut session, hidden);
                    }
                },
            },
        );
    }
}

/// Um Lance na lista: o atual em `accent-subtle` com texto `accent-ink`; clicar navega.
fn move_cell(
    ui: &mut egui::Ui,
    p: &crate::ui_theme::Palette,
    san: &str,
    current: bool,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        san.to_owned(),
        egui::FontId::proportional(14.0),
        egui::Color32::PLACEHOLDER,
    );
    let width = (galley.size().x + 16.0).max(64.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let corner = crate::panel_chrome::radius("radius-s");
        let ink = if current {
            ui.painter().rect_filled(rect, corner, p.accent_subtle);
            p.accent_ink
        } else {
            if response.hovered() {
                ui.painter().rect_filled(rect, corner, p.surface_3);
            }
            p.ink
        };
        let at = egui::Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), rect.shrink2(egui::vec2(8.0, 0.0)))
            .min;
        ui.painter().galley(at, galley, ink);
    }
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, current, san));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Conteúdo do painel Notação.
pub fn ui(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, hidden: bool) {
    if hidden {
        crate::scoresheet::hidden_notice(ui, i18n);
        return;
    }
    let snapshot = session.snapshot().clone();
    if snapshot.history.is_empty() {
        ui.weak(i18n.t("notation-empty"));
        return;
    }
    let first_number = session
        .game()
        .review_at_ply(0)
        .map_or(1, |v| v.position.fullmove_number());
    let current = current_ply(session);
    let previous_id = ui.id().with("last-current-ply");
    let previous: Option<usize> = ui.data(|d| d.get_temp(previous_id));
    // Rolagem automática só quando o Lance atual muda: não briga com a rolagem manual.
    let follow = previous != Some(current);
    let mut go_to = None;
    let p = crate::panel_chrome::palette_of(ui.visuals());
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // NotationList: três colunas (número, brancas, pretas).
            egui::Grid::new("notation_grid")
                .num_columns(3)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    for row in rows(&snapshot.history, first_number) {
                        ui.label(
                            egui::RichText::new(format!("{}.", row.number)).color(p.ink_muted),
                        );
                        for (cell, gap) in [(&row.white, row.white.is_none()), (&row.black, false)]
                        {
                            match cell {
                                Some(cell) => {
                                    let response = move_cell(ui, p, &cell.san, cell.ply == current);
                                    if cell.ply == current && follow {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if response.clicked() {
                                        go_to = Some(cell.ply);
                                    }
                                }
                                None if gap => {
                                    ui.label(egui::RichText::new("…").color(p.ink_muted));
                                }
                                None => {
                                    ui.label("");
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            if let Some(outcome) = snapshot.outcome {
                ui.separator();
                ui.strong(score(outcome.result));
                ui.small(crate::ui::outcome_text(i18n, outcome));
            }
        });
    ui.data_mut(|d| d.insert_temp(previous_id, current));
    if let Some(ply) = go_to {
        navigate(session, ply);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn play(game: &mut Game, from: &str, to: &str) -> bool {
        game.handle(
            Command::Release {
                from: from.parse().unwrap(),
                to: to.parse().unwrap(),
            },
            Timestamp::mono(0),
        )
        .is_ok()
    }

    fn session_with(moves: &[(&str, &str)]) -> Session {
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        for (from, to) in moves {
            play(&mut game, from, to);
        }
        Session::from_game(game)
    }

    /// UI-07: a notação corresponde aos Lances legais registrados, na ordem, e ignora
    /// tentativas ilegais.
    #[test]
    fn notation_matches_the_registered_legal_moves_and_ignores_illegal_attempts() {
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        assert!(play(&mut game, "e2", "e4"));
        assert!(play(&mut game, "e7", "e5"));
        let _ = play(&mut game, "e1", "e5"); // ilegal: fica fora do histórico
        assert!(play(&mut game, "g1", "f3"));
        let history = game.snapshot().history;
        let listed = rows(&history, 1);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].number, 1);
        assert_eq!(listed[0].white.as_ref().unwrap().san, "e4");
        assert_eq!(listed[0].black.as_ref().unwrap().san, "e5");
        assert_eq!(listed[1].number, 2);
        assert_eq!(listed[1].white.as_ref().unwrap().san, "Nf3");
        assert!(listed[1].black.is_none());
        let sans: Vec<&str> = listed
            .iter()
            .flat_map(|r| [&r.white, &r.black])
            .flatten()
            .map(|c| c.san.as_str())
            .collect();
        let registered: Vec<&str> = history.iter().map(|h| h.san.as_str()).collect();
        assert_eq!(sans, registered);
        // Cada célula navega até a Posição que realmente produziu aquele Lance.
        for cell in listed.iter().flat_map(|r| [&r.white, &r.black]).flatten() {
            let view = game.review_at_ply(cell.ply).unwrap();
            assert_eq!(view.last_move.unwrap().san, cell.san);
        }
    }

    #[test]
    fn a_list_that_starts_with_black_uses_the_fen_move_number() {
        let setup = GameSetup::from_fen(
            PolicyKind::Assisted,
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2",
        )
        .unwrap();
        let mut game = Game::new(setup);
        assert!(play(&mut game, "g8", "f6"));
        assert!(play(&mut game, "b1", "c3"));
        let first = game.review_at_ply(0).unwrap().position.fullmove_number();
        assert_eq!(first, 2);
        let listed = rows(&game.snapshot().history, first);
        assert_eq!(listed[0].number, 2);
        assert!(listed[0].white.is_none());
        assert_eq!(listed[0].black.as_ref().unwrap().san, "Nf6");
        assert_eq!(listed[1].number, 3);
        assert_eq!(listed[1].white.as_ref().unwrap().san, "Nc3");
    }

    /// Clicar num Lance navega na revisão sem gerar Eventos; o último volta ao vivo.
    #[test]
    fn navigating_is_read_only_and_the_last_move_returns_to_live() {
        let mut s = session_with(&[("e2", "e4"), ("e7", "e5"), ("g1", "f3")]);
        let events = s.game().events().len();
        let snapshot = s.snapshot().clone();
        assert_eq!(current_ply(&s), 3);
        navigate(&mut s, 1);
        assert_eq!(current_ply(&s), 1);
        assert_eq!(s.review().unwrap().last_move.as_ref().unwrap().san, "e4");
        navigate(&mut s, 2);
        assert_eq!(current_ply(&s), 2);
        navigate(&mut s, 3);
        assert!(s.review().is_none(), "o último Lance é a Partida ao vivo");
        assert_eq!(s.game().events().len(), events, "nenhum Evento gerado");
        assert_eq!(s.snapshot(), &snapshot);
    }

    #[test]
    fn scores_follow_the_standard_notation() {
        assert_eq!(score(GameResult::WhiteWins), "1-0");
        assert_eq!(score(GameResult::BlackWins), "0-1");
        assert_eq!(score(GameResult::Draw), "1/2-1/2");
    }
}
