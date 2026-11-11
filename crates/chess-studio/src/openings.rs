//! Painel Aberturas (C19, #147): nome/ECO da Posição atual (livro offline `chess-openings`) e
//! estatísticas por lance candidato das Partidas da Biblioteca do Perfil local (índice do
//! `chess-store`), com barras de vitórias das Brancas, empates e vitórias das Pretas.
//!
//! O painel é só leitura (ADR-0001): clicar num lance mostra a Posição resultante como variante
//! (`Session::show_variation`, o mesmo mecanismo da Análise), sem Comando nem Evento. Fica
//! indisponível numa Partida competitiva em andamento (inclusive a Simulação FIDE), durante um
//! Exercício de Treino e com a Súmula manual em anotação (`hides_notation`), como a Análise.
//!
//! A lógica pura (disponibilidade, variante, texto de data, barras) fica no alto do arquivo e é
//! testada sem Bevy.

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::notation;
use crate::persistence::PersistenceHandle;
use crate::session::Session;
use crate::training_ui::{TrainingUi, with_store};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::CanonicalPosition;
use chess_game::{HistoryEntry, ReviewView};
use chess_openings::{Opening, OpeningBook};
use chess_store::{OpeningMoveStats, Store};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A consulta se refaz sozinha neste intervalo (Partida terminando, importação nova).
const AUTO_REFRESH: Duration = Duration::from_secs(5);

// --- Lógica pura (testável sem Bevy) ----------------------------------------------------------

/// Por que o painel não mostra o explorador, se for o caso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// Partida competitiva em andamento (Simulação FIDE, Morte súbita).
    Competitive,
    /// A `Session` mostra um Exercício de Treino.
    Training,
    /// Súmula manual em anotação: lances e variantes em SAN seriam resposta pronta.
    SheetHidesNotation,
}

/// Mesma regra da Análise e da notação: a Súmula e a competição vêm antes do Treino.
pub fn availability(
    competitive_in_progress: bool,
    training_swapped: bool,
    hides_notation: bool,
) -> Availability {
    if hides_notation {
        Availability::SheetHidesNotation
    } else if competitive_in_progress {
        Availability::Competitive
    } else if training_swapped {
        Availability::Training
    } else {
        Availability::Ready
    }
}

/// A Posição depois de `san` a partir de `base`, como revisão somente leitura (`base_ply` é o
/// meio-lance de `base`). `None` se o lance não é legal ali.
pub fn variation_after(
    base: &CanonicalPosition,
    base_ply: usize,
    total_plies: usize,
    san: &str,
) -> Option<ReviewView> {
    let mv = base.parse_san(san).ok()?;
    let side = base.side_to_move();
    let position = base.play(mv).ok()?;
    let san = base.san(mv).ok()?;
    Some(ReviewView {
        ply: base_ply,
        total_plies,
        side_to_move: position.side_to_move(),
        in_check: position.is_check(),
        position,
        last_move: Some(HistoryEntry {
            ply: base_ply + 1,
            side,
            mv,
            san,
        }),
        is_start: false,
        is_end: false,
    })
}

/// `dd/mm/aaaa` (UTC) a partir de milissegundos Unix; vazio para instante inválido (zero).
pub fn last_used_text(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let iso = chess_game::pgn_date_from_unix_ms(ms);
    let mut parts = iso.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => iso,
    }
}

/// Porcentagens inteiras `(brancas, empates, pretas)` das Partidas decididas, somando 100
/// (o resto do arredondamento vai para a maior fatia).
pub fn percentages(stats: &OpeningMoveStats) -> (u32, u32, u32) {
    let total = stats.decided();
    if total == 0 {
        return (0, 0, 0);
    }
    let mut parts = [
        stats.white_wins * 100 / total,
        stats.draws * 100 / total,
        stats.black_wins * 100 / total,
    ];
    let left = 100 - parts.iter().sum::<u32>();
    if let Some(max) = [stats.white_wins, stats.draws, stats.black_wins]
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| **n)
        .map(|(i, _)| i)
    {
        parts[max] += left;
    }
    (parts[0], parts[1], parts[2])
}

/// A abertura nomeada mais recente na linha da Partida até o meio-lance `ply`.
pub fn opening_at(book: &OpeningBook, session: &Session, ply: usize) -> Option<Opening> {
    let positions: Vec<CanonicalPosition> = (0..=ply)
        .filter_map(|p| session.game().review_at_ply(p))
        .map(|view| view.position)
        .collect();
    book.along(positions.iter()).cloned()
}

// --- Estado e painel --------------------------------------------------------------------------

/// O que identifica a consulta atual: mudou, consulta de novo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryKey {
    epoch: u64,
    ply: usize,
    total: usize,
    fen: String,
}

#[derive(Resource, Default)]
pub struct OpeningsUi {
    key: Option<QueryKey>,
    refreshed: Option<Instant>,
    dirty: bool,
    opening: Option<Opening>,
    stats: Vec<OpeningMoveStats>,
    error: Option<String>,
    /// Lance (SAN) em exibição como variante.
    preview: Option<String>,
    /// Banco em memória quando não há persistência (testes e demonstrações).
    local: Option<Mutex<Store>>,
    /// A Trava de fair play recusou a estatística desta Posição (PO-13).
    fair_locked: bool,
}

pub struct OpeningsPlugin;

impl Plugin for OpeningsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpeningsUi>();
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::OPENINGS,
                label_key: "dock-panel-openings",
                access: PanelAccess::OutsideCompetitive,
                ui: openings_panel,
            },
        );
    }
}

enum Action {
    Show(String),
    Back,
    Reindex,
}

fn openings_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let hides = world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
    let swapped = world
        .get_resource::<TrainingUi>()
        .is_some_and(TrainingUi::session_swapped);
    // Sem o ponto único da Trava de fair play, nada se consulta.
    let Some(fair) = world.get_resource::<crate::fair_play::FairPlay>().cloned() else {
        return;
    };
    world.resource_scope(|world, mut state: Mut<OpeningsUi>| {
        world.resource_scope(|world, mut session: Mut<Session>| {
            match availability(session.competitive_in_progress(), swapped, hides) {
                Availability::Ready => {}
                Availability::SheetHidesNotation => {
                    crate::scoresheet::hidden_notice(ui, i18n);
                    return;
                }
                Availability::Competitive => {
                    ui.weak(i18n.t("openings-locked-competitive"));
                    return;
                }
                Availability::Training => {
                    ui.weak(i18n.t("openings-locked-training"));
                    return;
                }
            }
            let handle = world.get_resource::<PersistenceHandle>();
            state.refresh(&session, handle, false, &fair);
            if state.fair_locked {
                ui.weak(i18n.t("openings-locked-fairplay"));
                return;
            }
            let mut actions = Vec::new();
            draw(ui, i18n, &state, &session, &mut actions);
            for action in actions {
                match action {
                    Action::Show(san) => state.show(&mut session, &san),
                    Action::Back => {
                        let ply = notation::current_ply(&session);
                        notation::navigate(&mut session, ply);
                        state.preview = None;
                    }
                    Action::Reindex => state.refresh(&session, handle, true, &fair),
                }
            }
        });
    });
}

impl OpeningsUi {
    /// Consulta o livro e o índice quando a Posição mudou, a cada [`AUTO_REFRESH`] ou sob
    /// demanda (`rebuild`: refaz o índice do zero antes).
    fn refresh(
        &mut self,
        session: &Session,
        handle: Option<&PersistenceHandle>,
        rebuild: bool,
        fair: &crate::fair_play::FairPlay,
    ) {
        let ply = notation::current_ply(session);
        let total = session.snapshot().history.len();
        let Some(view) = session.game().review_at_ply(ply) else {
            return;
        };
        let Some(subject) = crate::fair_play::Subject::at(session.game(), ply) else {
            return;
        };
        // A Trava vale a cada quadro, antes do cache.
        self.fair_locked = fair
            .check(crate::fair_play::Computation::LibraryStats, &subject)
            .is_err();
        if self.fair_locked {
            self.stats.clear();
            self.key = None;
            return;
        }
        let fen = view.position.to_fen();
        let key = QueryKey {
            epoch: session.epoch(),
            ply,
            total,
            fen: fen.clone(),
        };
        let due = self.refreshed.is_none_or(|at| at.elapsed() >= AUTO_REFRESH);
        if self.key.as_ref() == Some(&key) && !due && !self.dirty && !rebuild {
            return;
        }
        if self.key.as_ref() != Some(&key) {
            self.preview = None;
        }
        self.key = Some(key);
        self.refreshed = Some(Instant::now());
        self.dirty = false;
        self.opening = opening_at(OpeningBook::embedded(), session, ply);
        let result = with_store(&mut self.local, handle, |store, profile| {
            if rebuild && let Err(err) = store.rebuild_opening_index() {
                return Ok(Err(err));
            }
            crate::fair_play::opening_stats(fair, &subject, store, profile, &fen)
        });
        let result = match result {
            Some(Err(_locked)) => {
                self.fair_locked = true;
                self.stats.clear();
                return;
            }
            Some(Ok(stats)) => Some(stats),
            None => None,
        };
        match result {
            Some(Ok(stats)) => {
                self.error = None;
                self.stats = stats;
            }
            Some(Err(err)) => {
                self.error = Some(err.to_string());
                self.stats.clear();
            }
            None => self.stats.clear(),
        }
    }

    /// Mostra o lance como variante (somente leitura): nenhum Comando, nenhum Evento.
    fn show(&mut self, session: &mut Session, san: &str) {
        let ply = notation::current_ply(session);
        let total = session.snapshot().history.len();
        let Some(base) = session.game().review_at_ply(ply).map(|v| v.position) else {
            return;
        };
        if let Some(view) = variation_after(&base, ply, total, san) {
            session.show_variation(view);
            self.preview = Some(san.to_owned());
        }
    }
}

fn draw(
    ui: &mut egui::Ui,
    i18n: &I18n,
    state: &OpeningsUi,
    session: &Session,
    actions: &mut Vec<Action>,
) {
    match &state.opening {
        Some(o) => {
            ui.strong(i18n.t_with(
                "openings-current",
                &[("eco", &o.eco), ("name", &o.name_pt_br())],
            ));
        }
        None => {
            ui.weak(i18n.t("openings-unnamed"));
        }
    }
    if let Some(error) = &state.error {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            i18n.t_with("openings-error", &[("error", error)]),
        );
    }
    ui.add_space(4.0);
    if state.stats.is_empty() {
        ui.weak(i18n.t("openings-empty"));
    } else {
        stats_table(ui, i18n, state, actions);
        ui.small(i18n.t("openings-legend"));
    }
    ui.add_space(4.0);
    if session.showing_variation() {
        ui.small(i18n.t("openings-showing"));
        if ui.button(i18n.t("openings-back")).clicked() {
            actions.push(Action::Back);
        }
    } else {
        ui.weak(i18n.t("openings-hint"));
    }
    ui.separator();
    if ui.button(i18n.t("openings-reindex")).clicked() {
        actions.push(Action::Reindex);
    }
    ui.small(i18n.t("openings-source-note"));
}

fn stats_table(ui: &mut egui::Ui, i18n: &I18n, state: &OpeningsUi, actions: &mut Vec<Action>) {
    egui::Grid::new("opening_moves_table")
        .num_columns(4)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.strong(i18n.t("openings-col-move"));
            ui.strong(i18n.t("openings-col-games"));
            ui.strong(i18n.t("openings-col-results"));
            ui.strong(i18n.t("openings-col-last"));
            ui.end_row();
            for stats in &state.stats {
                let selected = state.preview.as_deref() == Some(stats.san.as_str());
                if ui.selectable_label(selected, &stats.san).clicked() {
                    actions.push(Action::Show(stats.san.clone()));
                }
                ui.label(stats.games.to_string());
                result_bar(ui, i18n, stats);
                ui.label(last_used_text(stats.last_used_ms));
                ui.end_row();
            }
        });
}

/// Barra horizontal Brancas | Empates | Pretas, com as porcentagens na dica.
fn result_bar(ui: &mut egui::Ui, i18n: &I18n, stats: &OpeningMoveStats) {
    let size = egui::vec2(120.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let (w, d, b) = stats.shares();
    let colors = [
        egui::Color32::from_gray(235),
        egui::Color32::from_gray(128),
        egui::Color32::from_gray(38),
    ];
    if stats.decided() == 0 {
        painter.rect_filled(rect, 2.0, egui::Color32::from_gray(70));
    } else {
        let mut x = rect.left();
        for (share, color) in [w, d, b].into_iter().zip(colors) {
            let width = rect.width() * share;
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, size.y)),
                0.0,
                color,
            );
            x += width;
        }
    }
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
        egui::StrokeKind::Inside,
    );
    let (pw, pd, pb) = percentages(stats);
    let mut tip = i18n.t_with(
        "openings-bar-tip",
        &[
            ("white", &pw.to_string()),
            ("draw", &pd.to_string()),
            ("black", &pb.to_string()),
            ("decided", &stats.decided().to_string()),
        ],
    );
    if stats.unfinished > 0 {
        tip.push_str(" · ");
        tip.push_str(&i18n.t_with(
            "openings-unfinished",
            &[("count", &stats.unfinished.to_string())],
        ));
    }
    response.on_hover_text(tip);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn stats(w: u32, d: u32, b: u32) -> OpeningMoveStats {
        OpeningMoveStats {
            san: "e4".into(),
            games: w + d + b,
            white_wins: w,
            draws: d,
            black_wins: b,
            unfinished: 0,
            last_used_ms: 0,
        }
    }

    #[test]
    fn unavailable_in_competitive_training_and_with_the_sheet_open() {
        assert_eq!(availability(false, false, false), Availability::Ready);
        assert_eq!(availability(true, false, false), Availability::Competitive);
        assert_eq!(availability(false, true, false), Availability::Training);
        assert_eq!(
            availability(false, false, true),
            Availability::SheetHidesNotation
        );
        // A Súmula em anotação vence, mesmo numa Simulação FIDE em andamento.
        assert_eq!(
            availability(true, false, true),
            Availability::SheetHidesNotation
        );
    }

    #[test]
    fn variation_plays_the_move_from_the_base_position() {
        let base = CanonicalPosition::start();
        let view = variation_after(&base, 0, 0, "e4").unwrap();
        assert_eq!(view.ply, 0);
        assert_eq!(view.last_move.as_ref().unwrap().san, "e4");
        assert_eq!(view.last_move.as_ref().unwrap().ply, 1);
        assert_eq!(view.side_to_move, chess_core::Color::Black);
        assert!(!view.is_start && !view.is_end);
        assert!(
            view.position
                .to_fen()
                .starts_with("rnbqkbnr/pppppppp/8/8/4P3")
        );
        assert!(variation_after(&base, 0, 0, "Ke4").is_none());
    }

    #[test]
    fn dates_are_shown_day_first() {
        assert_eq!(last_used_text(1_700_000_000_000), "14/11/2023");
        assert_eq!(last_used_text(0), "");
    }

    #[test]
    fn percentages_sum_to_one_hundred() {
        assert_eq!(percentages(&stats(1, 1, 1)), (33, 33, 34));
        assert_eq!(percentages(&stats(2, 0, 0)), (100, 0, 0));
        assert_eq!(percentages(&stats(0, 0, 0)), (0, 0, 0));
        let (w, d, b) = percentages(&stats(7, 2, 5));
        assert_eq!(w + d + b, 100);
    }

    #[test]
    fn names_the_opening_of_the_current_game_line() {
        use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        let book = OpeningBook::embedded();
        let mut t = 0;
        let mut play = |game: &mut Game, from: &str, to: &str| {
            t += 1000;
            game.handle(
                Command::Release {
                    from: from.parse().unwrap(),
                    to: to.parse().unwrap(),
                },
                Timestamp::mono(t),
            )
            .unwrap();
        };
        let session = Session::from_game(game.clone());
        assert!(opening_at(book, &session, 0).is_none());
        for (from, to) in [
            ("e2", "e4"),
            ("e7", "e5"),
            ("g1", "f3"),
            ("b8", "c6"),
            ("f1", "b5"),
        ] {
            play(&mut game, from, to);
        }
        // Mais dois lances fora da teoria: o nome da última Posição nomeada continua.
        for (from, to) in [("a7", "a6"), ("h2", "h3")] {
            play(&mut game, from, to);
        }
        let session = Session::from_game(game);
        let at_5 = opening_at(book, &session, 5).unwrap();
        assert_eq!(at_5.eco, "C60");
        let at_end = opening_at(book, &session, 7).unwrap();
        assert!(at_end.name.starts_with("Ruy Lopez"), "{}", at_end.name);
    }
}
