//! Parte do painel das Políticas competitivas e do relógio: tempos, estado da vez (toque,
//! soltura, relógio: UX-05), aviso RULE-05, Pausar/Retomar, Retratar, Reclamar empate com
//! Lance anunciado opcional e os avisos da arbitragem.
//!
//! Respeita a DEC-32: o painel **não** diz se uma peça solta é legal. O texto de "peça solta"
//! é o mesmo para o Lance legal feito e para a casa ilegal; quem retrata um Lance legal
//! recebe a recusa do art. 4.7, como a arbitragem na mesa.

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::session::{Notice, Session};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::{Move, Square};
use chess_game::Snapshot;

fn squares_text(squares: impl IntoIterator<Item = Square>) -> String {
    squares
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Chave i18n do estado da vez: distingue seleção, Toque intencional, soltura e relógio.
pub fn turn_state(snapshot: &Snapshot, lifted: bool) -> (&'static str, String) {
    if snapshot.provisional.is_some() {
        return ("fide-state-promotion", String::new());
    }
    if let Some(release) = &snapshot.provisional_release {
        return (
            "fide-state-released",
            format!("{}-{}", release.from, release.to),
        );
    }
    if let Some(made) = &snapshot.made_move {
        return (
            "fide-state-released",
            format!("{}-{}", made.mv.from(), made.mv.to()),
        );
    }
    if !snapshot.touched.is_empty() {
        let key = if lifted {
            "fide-state-held"
        } else {
            "fide-state-touched"
        };
        return (key, squares_text(snapshot.touched.iter().map(|t| t.square)));
    }
    ("fide-state-idle", String::new())
}

fn notice_text(i18n: &I18n, notice: &Notice) -> String {
    match notice {
        Notice::Infraction { count, ended } => {
            let key = if *ended {
                "fide-notice-infraction-ended"
            } else {
                "fide-notice-infraction"
            };
            i18n.t_with(key, &[("count", &count.to_string())])
        }
        Notice::ClaimAccepted => i18n.t("fide-notice-claim-accepted"),
        Notice::ClaimRejected => i18n.t("fide-notice-claim-rejected"),
        Notice::CannotRetract => i18n.t("fide-notice-cannot-retract"),
        Notice::TouchObligation(squares) => i18n.t_with(
            "fide-notice-obligation",
            &[("squares", &squares_text(squares.iter().copied()))],
        ),
        Notice::ClaimRightLost => i18n.t("fide-notice-claim-right-lost"),
        Notice::ClaimedMoveIllegal => i18n.t("fide-notice-claimed-illegal"),
    }
}

/// Painel Arbitragem do dock (só faz sentido nas Políticas competitivas). Os tempos vivem
/// no painel Relógio; aqui ficam o estado da vez, as Infrações, Pausar, Retratar e Reclamar.
fn arbitration_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let Some(mut session) = world.get_resource_mut::<Session>() else {
        return;
    };
    if !session.is_competitive() {
        ui.weak(i18n.t("arbitration-not-competitive"));
        return;
    }
    arbitration_section(ui, i18n, &mut session);
}

pub struct ArbitrationPlugin;

impl Plugin for ArbitrationPlugin {
    fn build(&self, app: &mut App) {
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::ARBITRATION,
                label_key: "dock-panel-arbitration",
                access: PanelAccess::Always,
                ui: arbitration_panel_ui,
            },
        );
    }
}

/// Estado da vez, Infrações e ações da arbitragem (Políticas competitivas).
pub fn arbitration_section(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session) {
    let snapshot = session.snapshot().clone();
    if !session.is_competitive() {
        return;
    }
    if let Some(version) = snapshot.rules_version {
        ui.small(i18n.t_with("fide-rules", &[("rules", version)]));
    }
    if snapshot.non_classical_notice {
        ui.colored_label(egui::Color32::LIGHT_YELLOW, i18n.t("fide-rule05-notice"));
    }
    if snapshot.outcome.is_some() {
        return;
    }

    let (key, arg) = turn_state(&snapshot, session.gesture_visual().lifted);
    ui.label(i18n.t_with(key, &[("squares", &arg)]));
    if let Some(notice) = session.notice() {
        ui.colored_label(egui::Color32::YELLOW, notice_text(i18n, notice));
    }
    ui.small(i18n.t_with(
        "fide-infractions",
        &[
            ("white", &snapshot.infractions_white.to_string()),
            ("black", &snapshot.infractions_black.to_string()),
        ],
    ));

    let loose = snapshot.made_move.is_some()
        || snapshot.provisional_release.is_some()
        || snapshot.provisional.is_some();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                session.can_press_clock(snapshot.side_to_move),
                egui::Button::new(i18n.t("clock-press")),
            )
            .clicked()
        {
            session.press_clock();
        }
        if ui
            .add_enabled(loose, egui::Button::new(i18n.t("fide-button-retract")))
            .clicked()
        {
            session.retract();
        }
        let pause_key = if snapshot.paused {
            "fide-button-resume"
        } else {
            "fide-button-pause"
        };
        if ui.button(i18n.t(pause_key)).clicked() {
            session.toggle_pause();
        }
    });
    claim_section(ui, i18n, session, &snapshot);
}

fn claim_section(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, snapshot: &Snapshot) {
    let quiet = snapshot.touched.is_empty()
        && snapshot.made_move.is_none()
        && snapshot.provisional_release.is_none()
        && snapshot.provisional.is_none()
        && !snapshot.paused;
    egui::CollapsingHeader::new(i18n.t("fide-claim-heading")).show(ui, |ui| {
        ui.small(i18n.t("fide-claim-hint"));
        let id = egui::Id::new("claim_draft_move");
        let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(i18n.t("fide-claim-move-label"));
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(70.0));
        });
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
        let announced = text.trim().to_lowercase();
        let parsed = (!announced.is_empty()).then(|| announced.parse::<Move>());
        if matches!(parsed, Some(Err(_))) {
            ui.colored_label(egui::Color32::LIGHT_RED, i18n.t("fide-claim-move-invalid"));
        }
        let can = quiet && !matches!(parsed, Some(Err(_)));
        if ui
            .add_enabled(can, egui::Button::new(i18n.t("fide-button-claim")))
            .clicked()
        {
            session.claim_draw(parsed.and_then(Result::ok));
            ui.data_mut(|d| d.insert_temp(id, String::new()));
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn game() -> Game {
        Game::new(GameSetup::standard(PolicyKind::FideSimulation))
    }

    #[test]
    fn the_state_line_follows_touch_release_and_clock() {
        let mut g = game();
        assert_eq!(turn_state(&g.snapshot(), false).0, "fide-state-idle");
        g.handle(Command::Touch { square: sq("e2") }, Timestamp::mono(1))
            .unwrap();
        assert_eq!(
            turn_state(&g.snapshot(), false),
            ("fide-state-touched", "e2".to_owned())
        );
        assert_eq!(turn_state(&g.snapshot(), true).0, "fide-state-held");
        g.handle(
            Command::Release {
                from: sq("e2"),
                to: sq("e4"),
            },
            Timestamp::mono(2),
        )
        .unwrap();
        assert_eq!(
            turn_state(&g.snapshot(), false),
            ("fide-state-released", "e2-e4".to_owned())
        );
        g.handle(Command::PressClock, Timestamp::mono(3)).unwrap();
        assert_eq!(turn_state(&g.snapshot(), false).0, "fide-state-idle");
    }

    #[test]
    fn the_panel_does_not_reveal_whether_a_loose_piece_is_legal() {
        let mut legal = game();
        legal
            .handle(
                Command::Release {
                    from: sq("e2"),
                    to: sq("e4"),
                },
                Timestamp::mono(1),
            )
            .unwrap();
        let mut illegal = game();
        illegal
            .handle(
                Command::Release {
                    from: sq("e2"),
                    to: sq("e5"),
                },
                Timestamp::mono(1),
            )
            .unwrap();
        // DEC-32: mesma chave de texto para o Lance legal feito e para a casa ilegal.
        assert_eq!(
            turn_state(&legal.snapshot(), false).0,
            "fide-state-released"
        );
        assert_eq!(
            turn_state(&illegal.snapshot(), false).0,
            "fide-state-released"
        );
    }
}
