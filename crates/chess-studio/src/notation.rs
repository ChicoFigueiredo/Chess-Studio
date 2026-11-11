//! Painel Notação (UI-07): a lista de Lances em SAN é uma **leitura** do histórico legal
//! da Partida (`Snapshot::history`). Lances ilegais nunca entram nela, e clicar num Lance
//! só navega na revisão (`Session::set_review_ply`): nenhum Comando de Partida é emitido
//! (ADR-0001).

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::session::Session;
use crate::study::{StudyNotes, StudyPanelRequest};
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
                ui: panel_ui,
            },
        );
    }
}

fn panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let hidden = world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
    let swapped = world
        .get_resource::<crate::training_ui::TrainingUi>()
        .is_some_and(crate::training_ui::TrainingUi::session_swapped);
    let Some(mut session) = world.remove_resource::<Session>() else {
        return;
    };
    let mut notes = world.remove_resource::<StudyNotes>();
    let available = crate::study::quick_notes_available(session.competitive_in_progress(), swapped);
    let mut open_study = false;
    {
        let quick = notes
            .as_mut()
            .filter(|_| available)
            .map(|notes| QuickNotes {
                notes,
                open_study: &mut open_study,
            });
        ui_with_notes(ui, i18n, &mut session, hidden, quick);
    }
    world.insert_resource(session);
    if let Some(notes) = notes {
        world.insert_resource(notes);
    }
    if open_study && let Some(mut request) = world.get_resource_mut::<StudyPanelRequest>() {
        request.0 = true;
    }
}

/// Segunda vista da Anotação de estudo (#180): os mesmos dados do Painel Anotações.
pub struct QuickNotes<'a> {
    pub notes: &'a mut StudyNotes,
    /// Marcado quando o usuário pede (Shift+Enter) o Painel Anotações naquele Lance.
    pub open_study: &'a mut bool,
}

/// Editor inline em andamento: meio-lance e texto da 1ª linha.
#[derive(Clone)]
struct InlineEdit {
    ply: usize,
    text: String,
    focus: bool,
}

/// Largura máxima (em caracteres) da 1ª linha mostrada ao lado do Lance.
const QUICK_NOTE_CHARS: usize = 14;

/// 1ª linha cortada em `QUICK_NOTE_CHARS` com reticências; só aloca quando corta.
fn truncated(text: &str) -> std::borrow::Cow<'_, str> {
    match text.char_indices().nth(QUICK_NOTE_CHARS) {
        Some((end, _)) => std::borrow::Cow::Owned(format!("{}…", text[..end].trim_end())),
        None => std::borrow::Cow::Borrowed(text),
    }
}

/// Indicador + 1ª linha da anotação (ou o botão discreto) ao lado do Lance, e o editor
/// inline quando é este Lance. Devolve `true` se Shift+Enter pediu o Painel Anotações.
fn note_slot(
    ui: &mut egui::Ui,
    i18n: &I18n,
    p: &crate::ui_theme::Palette,
    quick: &mut QuickNotes<'_>,
    ply: usize,
    row_hovered: bool,
) -> bool {
    let edit_id = egui::Id::new("quick_note_inline_edit");
    let editing: Option<InlineEdit> = ui.data(|d| d.get_temp(edit_id));
    if let Some(mut edit) = editing.filter(|e| e.ply == ply) {
        let field =
            crate::ui_theme::cs_text_field(ui, &mut edit.text, &i18n.t("notation-note-hint"), None);
        if edit.focus {
            field.request_focus();
            edit.focus = false;
        }
        let (esc, enter, shift) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
                i.modifiers.shift,
            )
        });
        let mut open_panel = false;
        let finished = if esc {
            true
        } else if field.lost_focus() || (field.has_focus() && enter) {
            quick.notes.set_first_line(ply, &edit.text);
            open_panel = enter && shift;
            true
        } else {
            false
        };
        ui.data_mut(|d| {
            if finished {
                d.remove::<InlineEdit>(edit_id);
            } else {
                d.insert_temp(edit_id, edit);
            }
        });
        return open_panel;
    }
    let first = quick.notes.first_line(ply);
    let icon = if quick.notes.is_multiline(ply) {
        egui_phosphor::regular::LIST_DASHES
    } else if first.is_some() {
        egui_phosphor::regular::NOTE_PENCIL
    } else {
        egui_phosphor::regular::NOTE_BLANK
    };
    let color = if first.is_some() || row_hovered {
        p.ink_muted
    } else {
        egui::Color32::TRANSPARENT
    };
    let text = first.unwrap_or_default();
    let shown = truncated(text);
    let job = crate::ui_theme::icon_job(
        ui.ctx(),
        Some(icon),
        &shown,
        egui::FontId::proportional(12.0),
        color,
    );
    let button = ui
        .add(
            egui::Button::new(job)
                .frame(false)
                .wrap_mode(egui::TextWrapMode::Extend),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let button = if let Some(full) = quick.notes.get(ply) {
        button.on_hover_text(full)
    } else {
        button.on_hover_text(i18n.t("notation-note-add"))
    };
    if button.clicked() {
        let text = text.to_owned();
        ui.data_mut(|d| {
            d.insert_temp(
                edit_id,
                InlineEdit {
                    ply,
                    text,
                    focus: true,
                },
            );
        });
    }
    false
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
/// Conteúdo do painel Notação; com `quick`, cada Lance ganha a nota rápida (#180).
pub fn ui_with_notes(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &mut Session,
    hidden: bool,
    mut quick: Option<QuickNotes<'_>>,
) {
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
                        // Números tabulares: a coluna não muda de largura entre 9. e 10.
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        crate::ui_theme::tabular_text(
                            ui,
                            &format!("{}.", row.number),
                            font,
                            p.ink_muted,
                        );
                        for (cell, gap) in [(&row.white, row.white.is_none()), (&row.black, false)]
                        {
                            match cell {
                                Some(cell) => {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 2.0;
                                        let response =
                                            move_cell(ui, p, &cell.san, cell.ply == current);
                                        if cell.ply == current && follow {
                                            response.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        if response.clicked() {
                                            go_to = Some(cell.ply);
                                        }
                                        if let Some(quick) = quick.as_mut() {
                                            let hover = response.hovered();
                                            if note_slot(ui, i18n, p, quick, cell.ply, hover) {
                                                go_to = Some(cell.ply);
                                                *quick.open_study = true;
                                            }
                                        }
                                    });
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
    fn the_quick_note_preview_is_cut_only_when_long() {
        assert_eq!(truncated("curta"), "curta");
        assert_eq!(truncated("Ataca o centro e abre o jogo"), "Ataca o centro…");
        assert_eq!(truncated("áéíóúáéíóúáéíóúá"), "áéíóúáéíóúáéíó…");
        assert_eq!(truncated("exatamente 14c"), "exatamente 14c");
    }

    #[test]
    fn scores_follow_the_standard_notation() {
        assert_eq!(score(GameResult::WhiteWins), "1-0");
        assert_eq!(score(GameResult::BlackWins), "0-1");
        assert_eq!(score(GameResult::Draw), "1/2-1/2");
    }
}
