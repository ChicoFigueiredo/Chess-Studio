//! Painel egui dos Auxílios, da revisão Lance a Lance, do FEN/PGN e do aviso de Partida
//! retomada. Fica em janelas próprias para não disputar espaço com o painel principal.
//! A interface só envia intenções à `Session` (ADR-0001); todo texto vem do i18n.

use crate::aids::{
    self, AidProfile, AidSettings, ErrorExplanation, NoticeView, notice_view, reason_key,
};
use crate::capture::CaptureMode;
use crate::fen_pgn::FileTools;
use crate::hud::{AttackedSide, HudPalette, HudProfile, HudSettings, OPACITY_RANGE};
use crate::hud_policy::{self, Allowance};
use crate::i18n::I18n;
use crate::paths;
use crate::session::{IllegalNotice, Session};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;
use chess_game::Command;

pub struct AidsPanelPlugin;

impl Plugin for AidsPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FileTools>()
            .add_systems(EguiPrimaryContextPass, resume_banner);
        crate::dock::register_panel(
            app,
            crate::dock::PanelDef {
                id: crate::dock::ids::AIDS,
                label_key: "dock-panel-aids",
                // Sempre acessível: o que a Política veda é filtrado dentro do painel
                // (`hud_policy`), para o "Xeque!" e o Perfil seguirem funcionando.
                access: crate::dock::PanelAccess::Always,
                ui: aids_panel_ui,
            },
        );
        crate::dock::register_panel(
            app,
            crate::dock::PanelDef {
                id: crate::dock::ids::REVIEW,
                label_key: "dock-panel-review",
                access: crate::dock::PanelAccess::Always,
                ui: review_panel_ui,
            },
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    First,
    Prev,
    Next,
    Last,
}

/// Botão de navegação com triângulos desenhados (a fonte da interface não tem ◀ ▶).
fn nav_button(ui: &mut egui::Ui, nav: Nav, enabled: bool, hover: &str) -> egui::Response {
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
    let left = matches!(nav, Nav::First | Nav::Prev);
    let dir = if left { -1.0 } else { 1.0 };
    let tri = |x: f32| {
        vec![
            egui::pos2(x + dir * w, c.y),
            egui::pos2(x - dir * w, c.y - h),
            egui::pos2(x - dir * w, c.y + h),
        ]
    };
    let bar = matches!(nav, Nav::First | Nav::Last);
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

fn side_name(i18n: &I18n, side: Side) -> String {
    i18n.t(match side {
        Side::White => "aid-side-white",
        Side::Black => "aid-side-black",
    })
}

fn notice_text(i18n: &I18n, notice: IllegalNotice) -> String {
    i18n.t_with(
        "aid-illegal-with-reason",
        &[
            ("from", &notice.from.to_string()),
            ("to", &notice.to.to_string()),
            ("reason", &i18n.t(reason_key(notice.reason))),
        ],
    )
}

/// Painel Auxílios do dock: alertas (xeque, Lance ilegal), o Perfil de auxílios e o HUD.
/// Nas Políticas competitivas em andamento o painel continua aberto (o "Xeque!" segue o
/// Perfil), mas o que a política veda aparece desabilitado e nunca é desenhado (UI-08).
fn aids_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    world.resource_scope(|world, mut session: Mut<Session>| {
        let allowed = hud_policy::allowance(&session);
        let Some(mut aids) = world.get_resource_mut::<AidSettings>() else {
            return;
        };
        // Edita uma cópia: `&mut` todo quadro marcaria `is_changed` e reaplicaria tudo.
        let mut edited = *aids;
        alerts(
            ui,
            i18n,
            &mut session,
            &aids::effective_settings(edited, &allowed),
        );
        ui.strong(i18n.t("aid-heading"));
        if allowed.restricted() {
            // Partida online: destinos legais seguem (só mostram regras, ADR-0007).
            let key = if session.is_online() {
                "aid-restricted-online"
            } else {
                "aid-restricted"
            };
            ui.weak(i18n.t(key));
        }
        aid_controls(ui, i18n, &mut edited, &allowed);
        if edited != *aids {
            *aids = edited;
        }
        let Some(mut hud) = world.get_resource_mut::<HudSettings>() else {
            return;
        };
        let mut edited_hud = *hud;
        ui.separator();
        hud_controls(ui, i18n, &mut edited_hud, &allowed);
        if edited_hud != *hud {
            *hud = edited_hud;
        }
    });
}

/// Painel Revisão do dock: navegação Lance a Lance, incidentes, FEN e PGN.
fn review_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let epoch = world.get_resource::<Session>().map(Session::epoch);
    let notes = world
        .get_resource::<crate::study::StudyNotes>()
        .zip(epoch)
        .map(|(notes, epoch)| notes.export(epoch))
        .unwrap_or_default();
    let hide_notation = world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
    world.resource_scope(|world, mut session: Mut<Session>| {
        world.resource_scope(|_, mut tools: Mut<FileTools>| {
            review_section(ui, i18n, &mut session, hide_notation);
            ui.separator();
            file_section(ui, i18n, &mut session, &mut tools, &notes, hide_notation);
        });
    });
}

/// Xeque (se o Auxílio estiver ligado) e aviso de Lance ilegal conforme a explicação.
fn alerts(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, aids: &AidSettings) {
    let snapshot = session.snapshot();
    if aids.check && snapshot.in_check && snapshot.outcome.is_none() && session.review().is_none() {
        ui.colored_label(egui::Color32::LIGHT_RED, i18n.t("aid-check-alert"));
    }
    let Some(notice) = session.illegal_notice() else {
        return;
    };
    match notice_view(aids.explanation, session.illegal_revealed()) {
        NoticeView::Hidden => {}
        NoticeView::Brief => {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::YELLOW, i18n.t("aid-illegal-brief"));
                if ui.small_button(i18n.t("aid-why")).clicked() {
                    session.reveal_illegal();
                }
            });
        }
        NoticeView::WithReason => {
            ui.colored_label(egui::Color32::YELLOW, notice_text(i18n, notice));
        }
    }
}

/// Caixa de seleção que a política pode vedar: desabilitada (e desmarcada na tela) quando
/// vedada, sem mexer no ajuste guardado.
fn gated_checkbox(ui: &mut egui::Ui, allowed: bool, value: &mut bool, label: String) {
    if allowed {
        ui.checkbox(value, label);
    } else {
        let mut shown = false;
        ui.add_enabled(false, egui::Checkbox::new(&mut shown, label));
    }
}

fn aid_controls(ui: &mut egui::Ui, i18n: &I18n, aids: &mut AidSettings, allowed: &Allowance) {
    let current = aids.profile();
    egui::ComboBox::from_label(i18n.t("aid-profile-label"))
        .selected_text(i18n.t(current.label_key()))
        .show_ui(ui, |ui| {
            for profile in [AidProfile::PhysicalGame, AidProfile::Training] {
                if ui
                    .selectable_label(current == profile, i18n.t(profile.label_key()))
                    .clicked()
                    && let Some(preset) = AidSettings::for_profile(profile)
                {
                    *aids = preset;
                }
            }
            if current == AidProfile::Custom {
                let _ = ui.selectable_label(true, i18n.t(AidProfile::Custom.label_key()));
            }
        });
    gated_checkbox(
        ui,
        allowed.destinations,
        &mut aids.destinations,
        i18n.t("aid-destinations"),
    );
    gated_checkbox(ui, allowed.ghosts, &mut aids.ghosts, i18n.t("aid-ghosts"));
    gated_checkbox(
        ui,
        allowed.selection,
        &mut aids.selected,
        i18n.t("aid-selected"),
    );
    gated_checkbox(
        ui,
        allowed.last_move,
        &mut aids.last_move,
        i18n.t("aid-last-move"),
    );
    gated_checkbox(ui, allowed.check, &mut aids.check, i18n.t("aid-check"));
    egui::ComboBox::from_label(i18n.t("aid-explanation-label"))
        .selected_text(i18n.t(aids.explanation.label_key()))
        .show_ui(ui, |ui| {
            for e in ErrorExplanation::ALL {
                ui.selectable_value(&mut aids.explanation, e, i18n.t(e.label_key()));
            }
        });
}

/// Controles do HUD virtual: perfil, elementos, transparência e paleta. Elementos vedados
/// pela Política ficam desabilitados.
fn hud_controls(ui: &mut egui::Ui, i18n: &I18n, hud: &mut HudSettings, allowed: &Allowance) {
    ui.strong(i18n.t("hud-heading"));
    ui.checkbox(&mut hud.enabled, i18n.t("hud-enabled"));
    if allowed.restricted() {
        ui.weak(i18n.t("hud-restricted"));
    }
    let current = hud.profile();
    egui::ComboBox::from_label(i18n.t("hud-profile-label"))
        .selected_text(i18n.t(current.label_key()))
        .show_ui(ui, |ui| {
            for profile in [HudProfile::Off, HudProfile::Study, HudProfile::Analysis] {
                if ui
                    .selectable_label(current == profile, i18n.t(profile.label_key()))
                    .clicked()
                    && let Some(preset) = HudSettings::for_profile(profile)
                {
                    // O preset troca os elementos; aparência e chave geral ficam.
                    *hud = HudSettings {
                        enabled: hud.enabled,
                        opacity_pct: hud.opacity_pct,
                        palette: hud.palette,
                        ..preset
                    };
                }
            }
            if current == HudProfile::Custom {
                let _ = ui.selectable_label(true, i18n.t(HudProfile::Custom.label_key()));
            }
        });
    let arrows = allowed.suggestion_arrows;
    gated_checkbox(
        ui,
        allowed.last_move,
        &mut hud.last_move_arrow,
        i18n.t("hud-last-move-arrow"),
    );
    gated_checkbox(
        ui,
        arrows && allowed.engine_line,
        &mut hud.engine_arrow,
        i18n.t("hud-engine-arrow"),
    );
    gated_checkbox(
        ui,
        arrows && allowed.hints,
        &mut hud.hint,
        i18n.t("hud-hint"),
    );
    gated_checkbox(
        ui,
        allowed.threats,
        &mut hud.threatened,
        i18n.t("hud-threatened"),
    );
    gated_checkbox(
        ui,
        allowed.threats,
        &mut hud.undefended,
        i18n.t("hud-undefended"),
    );
    ui.add_enabled_ui(allowed.attacked_squares, |ui| {
        egui::ComboBox::from_label(i18n.t("hud-attacked-label"))
            .selected_text(i18n.t(hud.attacked.label_key()))
            .show_ui(ui, |ui| {
                for side in AttackedSide::ALL {
                    ui.selectable_value(&mut hud.attacked, side, i18n.t(side.label_key()));
                }
            });
    });
    ui.add(
        egui::Slider::new(&mut hud.opacity_pct, OPACITY_RANGE)
            .suffix("%")
            .text(i18n.t("hud-opacity")),
    );
    egui::ComboBox::from_label(i18n.t("hud-palette-label"))
        .selected_text(i18n.t(hud.palette.label_key()))
        .show_ui(ui, |ui| {
            for palette in HudPalette::ALL {
                ui.selectable_value(&mut hud.palette, palette, i18n.t(palette.label_key()));
            }
        });
}

fn review_section(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, hide_notation: bool) {
    let total = session.snapshot().history.len();
    let ply = session.review().map_or(total, |r| r.ply);
    let reviewing = session.review().is_some();
    ui.strong(i18n.t("review-heading"));
    ui.horizontal(|ui| {
        let can_back = ply > 0;
        let can_forward = ply < total;
        if nav_button(ui, Nav::First, can_back, &i18n.t("review-first")).clicked() {
            session.set_review_ply(Some(0));
        }
        if nav_button(ui, Nav::Prev, can_back, &i18n.t("review-prev")).clicked() {
            session.set_review_ply(Some(ply - 1));
        }
        if nav_button(ui, Nav::Next, can_forward, &i18n.t("review-next")).clicked() {
            session.set_review_ply(Some(ply + 1).filter(|p| *p < total));
        }
        if nav_button(ui, Nav::Last, can_forward, &i18n.t("review-last")).clicked() {
            session.set_review_ply(None);
        }
    });
    if reviewing {
        let san = session
            .review()
            .and_then(|r| r.last_move.as_ref())
            .map_or_else(|| i18n.t("review-start"), |h| h.san.clone());
        if hide_notation {
            // Súmula manual aberta: sem a SAN do Lance (V2-SUM-01).
            ui.label(i18n.t_with(
                "scoresheet-review-hidden",
                &[("ply", &ply.to_string()), ("total", &total.to_string())],
            ));
        } else {
            ui.label(i18n.t_with(
                "review-position",
                &[
                    ("ply", &ply.to_string()),
                    ("total", &total.to_string()),
                    ("san", &san),
                ],
            ));
        }
        ui.small(i18n.t("review-readonly"));
        if ui.button(i18n.t("review-back")).clicked() {
            session.set_review_ply(None);
        }
    } else {
        ui.small(i18n.t_with("review-live", &[("total", &total.to_string())]));
    }
    let incidents = session.illegal_incidents();
    if !incidents.is_empty() {
        egui::CollapsingHeader::new(i18n.t_with(
            "review-incidents",
            &[("count", &incidents.len().to_string())],
        ))
        .show(ui, |ui| {
            for incident in &incidents {
                ui.label(i18n.t_with(
                    "review-incident",
                    &[
                        ("ply", &incident.after_ply.to_string()),
                        ("side", &side_name(i18n, incident.side)),
                        ("text", &notice_text(i18n, incident.notice)),
                    ],
                ));
            }
        });
    }
}

fn file_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    session: &mut Session,
    tools: &mut FileTools,
    notes: &[(usize, String)],
    hide_notation: bool,
) {
    egui::CollapsingHeader::new(i18n.t("fen-heading")).show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut tools.fen_text)
                .hint_text(i18n.t("fen-hint"))
                .desired_width(f32::INFINITY),
        );
        if ui.button(i18n.t("fen-load")).clicked() {
            tools.load_fen(session);
        }
        match &tools.fen_result {
            Some(Ok(())) => {
                ui.colored_label(egui::Color32::LIGHT_GREEN, i18n.t("fen-loaded"));
            }
            Some(Err(message)) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    i18n.t_with("fen-error", &[("detail", message)]),
                );
                ui.small(i18n.t("fen-unchanged"));
            }
            None => {}
        }
    });
    egui::CollapsingHeader::new(i18n.t("pgn-heading")).show(ui, |ui| {
        if hide_notation {
            // O arquivo PGN traria a notação inteira: bloqueado com a Súmula aberta.
            ui.weak(i18n.t("scoresheet-pgn-hidden"));
            return;
        }
        ui.checkbox(&mut tools.include_comments, i18n.t("pgn-comments"));
        if ui.button(i18n.t("pgn-export")).clicked() {
            let dir = paths::documents_dir(paths::Platform::current(), &paths::process_env);
            tools.export(session, notes, &dir);
        }
        match &tools.pgn_result {
            Some(Ok(path)) => {
                ui.colored_label(
                    egui::Color32::LIGHT_GREEN,
                    i18n.t_with("pgn-saved", &[("path", &path.display().to_string())]),
                );
            }
            Some(Err(message)) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    i18n.t_with("pgn-error", &[("detail", message)]),
                );
            }
            None => {}
        }
    });
}

/// Partida retomada do banco volta pausada (DEC-42): um aviso central pede para retomar.
fn resume_banner(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
    capture: Option<Res<CaptureMode>>,
) -> Result {
    if capture.is_some() {
        return Ok(());
    }
    let snapshot = session.snapshot();
    if !snapshot.paused || snapshot.outcome.is_some() {
        return Ok(());
    }
    let mut resume = false;
    egui::Window::new(i18n.t("resume-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
            ui.label(i18n.t("resume-text"));
            resume = ui.button(i18n.t("resume-button")).clicked();
        });
    if resume {
        session.command(Command::Resume);
    }
    Ok(())
}
