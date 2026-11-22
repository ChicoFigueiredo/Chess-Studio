//! Navegação da Revisão (#321): os quatro botões (início, anterior, próximo, fim) e as teclas
//! ← → Home End, compartilhados pelo Painel Notação e pelo Painel Revisão.
//!
//! # Autoridade (ADR-0001)
//!
//! Só navega na Revisão (`Session::set_review_ply`): nenhum Comando e nenhum Evento. O "fim"
//! sai da Revisão e volta à Partida ao vivo.

use crate::i18n::I18n;
use crate::session::Session;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::input::EguiWantsInput;

/// Um passo da Revisão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStep {
    /// Início da Partida.
    First,
    /// Lance anterior.
    Prev,
    /// Próximo Lance (o último volta à Partida ao vivo).
    Next,
    /// Posição atual: sai da Revisão.
    Last,
}

/// Para onde o `step` leva a Revisão, vista de `ply` num total de `total` meios-lances.
/// `None`: não há para onde ir; `Some(None)`: a Partida ao vivo; `Some(Some(p))`: o
/// meio-lance `p`.
pub fn review_target(step: ReviewStep, ply: usize, total: usize) -> Option<Option<usize>> {
    match step {
        ReviewStep::First if ply > 0 => Some(Some(0)),
        ReviewStep::Prev if ply > 0 => Some(Some(ply - 1)),
        ReviewStep::Next if ply < total => Some(Some(ply + 1).filter(|p| *p < total)),
        ReviewStep::Last if ply < total => Some(None),
        _ => None,
    }
}

/// O passo que uma tecla pede: ← → andam um meio-lance; Home e End vão ao início e ao fim.
pub fn step_for_key(key: KeyCode) -> Option<ReviewStep> {
    match key {
        KeyCode::ArrowLeft => Some(ReviewStep::Prev),
        KeyCode::ArrowRight => Some(ReviewStep::Next),
        KeyCode::Home => Some(ReviewStep::First),
        KeyCode::End => Some(ReviewStep::Last),
        _ => None,
    }
}

/// Dá o passo na Revisão da `Session`; devolve se algo mudou.
pub fn step(session: &mut Session, step: ReviewStep) -> bool {
    let total = session.snapshot().history.len();
    let ply = session.review().map_or(total, |r| r.ply);
    match review_target(step, ply, total) {
        Some(target) => {
            session.set_review_ply(target);
            true
        }
        None => false,
    }
}

/// As teclas só valem fora de campo de texto, sem KeyTips do Ribbon, com a notação à vista
/// (Súmula manual aberta a esconde) e sem um Exercício do Treino conduzindo a Revisão.
pub fn keys_enabled(
    typing: bool,
    keytips_active: bool,
    notation_hidden: bool,
    training_owns_review: bool,
) -> bool {
    !(typing || keytips_active || notation_hidden || training_owns_review)
}

/// ← → Home End navegam na Revisão, com a janela inteira em foco.
fn review_keys(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Option<Res<EguiWantsInput>>,
    ribbon: Option<Res<crate::ribbon::RibbonState>>,
    scoresheet: Option<Res<crate::scoresheet::ScoresheetUi>>,
    training: Option<Res<crate::training_ui::TrainingUi>>,
    mut session: ResMut<Session>,
) {
    let enabled = keys_enabled(
        egui.is_some_and(|e| e.wants_keyboard_input()),
        ribbon.is_some_and(|r| r.keys_captured),
        scoresheet.is_some_and(|s| s.hides_notation()),
        training.is_some_and(|t| t.session_swapped()),
    );
    if !enabled {
        return;
    }
    for key in keys.get_just_pressed() {
        if let Some(next) = step_for_key(*key) {
            step(&mut session, next);
        }
    }
}

pub struct ReviewNavPlugin;

impl Plugin for ReviewNavPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, review_keys);
    }
}

/// Os quatro botões da Revisão, na ordem início, anterior, próximo, posição atual. Usados
/// pelo Painel Revisão e pelo Painel Notação.
pub fn buttons(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session) {
    let total = session.snapshot().history.len();
    let ply = session.review().map_or(total, |r| r.ply);
    ui.horizontal(|ui| {
        for (nav, key) in [
            (ReviewStep::First, "review-first"),
            (ReviewStep::Prev, "review-prev"),
            (ReviewStep::Next, "review-next"),
            (ReviewStep::Last, "review-last"),
        ] {
            let enabled = review_target(nav, ply, total).is_some();
            if nav_button(ui, nav, enabled, &i18n.t(key)).clicked() {
                step(session, nav);
            }
        }
    });
}

/// Botão de navegação com triângulos desenhados (a fonte da interface não tem ◀ ▶).
fn nav_button(ui: &mut egui::Ui, nav: ReviewStep, enabled: bool, hover: &str) -> egui::Response {
    let size = egui::vec2(28.0, 22.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let visuals = ui.style().interact(&response);
    if enabled {
        ui.painter().rect(
            rect,
            3.0,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    let color = if enabled {
        visuals.fg_stroke.color
    } else {
        ui.visuals()
            .widgets
            .noninteractive
            .fg_stroke
            .color
            .gamma_multiply(0.4)
    };
    let c = rect.center();
    let (h, w) = (6.0, 5.0);
    let left = matches!(nav, ReviewStep::First | ReviewStep::Prev);
    let dir = if left { -1.0 } else { 1.0 };
    let tri = |x: f32| {
        vec![
            egui::pos2(x + dir * w, c.y),
            egui::pos2(x - dir * w, c.y - h),
            egui::pos2(x - dir * w, c.y + h),
        ]
    };
    let bar = matches!(nav, ReviewStep::First | ReviewStep::Last);
    let tri_x = if bar { c.x + dir * 2.0 } else { c.x };
    ui.painter().add(egui::Shape::convex_polygon(
        tri(tri_x),
        color,
        egui::Stroke::NONE,
    ));
    if bar {
        let x = c.x - dir * 7.0;
        ui.painter().line_segment(
            [egui::pos2(x, c.y - h), egui::pos2(x, c.y + h)],
            egui::Stroke::new(2.0, color),
        );
    }
    response.on_hover_text(hover)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn session_with(moves: &[(&str, &str)]) -> Session {
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        for (from, to) in moves {
            game.handle(
                Command::Release {
                    from: from.parse().unwrap(),
                    to: to.parse().unwrap(),
                },
                Timestamp::mono(0),
            )
            .unwrap();
        }
        Session::from_game(game)
    }

    /// Quatro meios-lances: e4 e5 Nf3 Nc6.
    fn four_plies() -> Session {
        session_with(&[("e2", "e4"), ("e7", "e5"), ("g1", "f3"), ("b8", "c6")])
    }

    #[test]
    fn targets_step_one_ply_and_the_last_one_returns_to_live() {
        use ReviewStep::*;
        assert_eq!(review_target(Prev, 3, 4), Some(Some(2)));
        assert_eq!(review_target(Next, 2, 4), Some(Some(3)));
        // O penúltimo meio-lance + 1 é a posição ao vivo.
        assert_eq!(review_target(Next, 3, 4), Some(None));
        assert_eq!(review_target(First, 3, 4), Some(Some(0)));
        assert_eq!(review_target(Last, 1, 4), Some(None));
    }

    #[test]
    fn targets_do_nothing_at_the_edges() {
        use ReviewStep::*;
        assert_eq!(review_target(Prev, 0, 4), None);
        assert_eq!(review_target(First, 0, 4), None);
        assert_eq!(review_target(Next, 4, 4), None);
        assert_eq!(review_target(Last, 4, 4), None);
        // Partida sem Lances.
        for step in [First, Prev, Next, Last] {
            assert_eq!(review_target(step, 0, 0), None);
        }
    }

    #[test]
    fn keys_map_to_the_four_steps() {
        assert_eq!(step_for_key(KeyCode::ArrowLeft), Some(ReviewStep::Prev));
        assert_eq!(step_for_key(KeyCode::ArrowRight), Some(ReviewStep::Next));
        assert_eq!(step_for_key(KeyCode::Home), Some(ReviewStep::First));
        assert_eq!(step_for_key(KeyCode::End), Some(ReviewStep::Last));
        assert_eq!(step_for_key(KeyCode::ArrowUp), None);
        assert_eq!(step_for_key(KeyCode::KeyA), None);
    }

    #[test]
    fn stepping_is_read_only_and_walks_the_review() {
        let mut s = four_plies();
        let events = s.game().events().len();
        assert!(step(&mut s, ReviewStep::Prev));
        assert_eq!(s.review().unwrap().ply, 3);
        assert!(step(&mut s, ReviewStep::First));
        assert_eq!(s.review().unwrap().ply, 0);
        assert!(
            !step(&mut s, ReviewStep::Prev),
            "no início não há para onde ir"
        );
        assert!(step(&mut s, ReviewStep::Last));
        assert!(s.review().is_none(), "o fim é a Partida ao vivo");
        assert!(!step(&mut s, ReviewStep::Next));
        assert_eq!(s.game().events().len(), events, "nenhum Evento novo");
    }

    #[test]
    fn keys_are_off_while_typing_or_when_the_notation_is_hidden_or_training_owns_the_review() {
        assert!(keys_enabled(false, false, false, false));
        assert!(
            !keys_enabled(true, false, false, false),
            "campo de texto em foco"
        );
        assert!(!keys_enabled(false, true, false, false), "KeyTips ativos");
        assert!(
            !keys_enabled(false, false, true, false),
            "Súmula escondendo a notação"
        );
        assert!(
            !keys_enabled(false, false, false, true),
            "Exercício do Treino"
        );
    }

    fn app_with(session: Session) -> App {
        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(session)
            .add_systems(Update, review_keys);
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(key);
        app.update();
    }

    #[test]
    fn arrow_keys_walk_the_review_of_the_live_session() {
        let mut app = app_with(four_plies());
        press(&mut app, KeyCode::ArrowLeft);
        assert_eq!(app.world().resource::<Session>().review().unwrap().ply, 3);
        press(&mut app, KeyCode::Home);
        assert_eq!(app.world().resource::<Session>().review().unwrap().ply, 0);
        press(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<Session>().review().unwrap().ply, 1);
        press(&mut app, KeyCode::End);
        assert!(app.world().resource::<Session>().review().is_none());
    }
}
