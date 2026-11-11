//! Barra de status (DS-4, #163; `docs/design/componentes/StatusBar.md`): Perfil de interface
//! ativo, Engine e consentimento (ponto colorido **e** palavra), de quem é a vez e o número do
//! lance, casa sob o cursor e Vista da câmera, avisos, FPS opcional e Escala. É a única barra da
//! janela: reserva a faixa inferior (`bottom_inset`), de modo que o 3D e o dock terminam acima
//! dela. No modo Foco ela some e devolve a faixa.
//!
//! Só leitura (ADR-0001): o texto sai de um instantâneo (`status_text`, pura); os cliques viram
//! pedidos de interface (trocar de Perfil, abrir o Painel Análise).

use crate::actions::ActionRequested;
use crate::analysis::Analysis;
use crate::dock::Dock;
use crate::focus::ViewState;
use crate::i18n::I18n;
use crate::profiles::{FactoryProfile, InterfaceProfiles, ProfileKey, ProfileRequest};
use crate::session::Session;
use crate::ui_theme::{self, Appearance, Palette, Tokens};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::{Color as Side, Square};
use chess_game::ClockReading;
use chess_input::CameraView;
use chess_input::controls::Action;

/// Abaixo disto o relógio de quem corre vira aviso.
pub const LOW_CLOCK_MS: u64 = 30_000;

/// Preferências da barra. O FPS vem desligado; o diálogo de Opções (#164) vai expor a escolha.
#[derive(Resource, Debug, Default)]
pub struct StatusBarPrefs {
    pub show_fps: bool,
}

/// Cor do estado: `success` ou `warning`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Success,
    Warning,
}

/// O que a barra sabe, sem Bevy nem egui.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusSnapshot {
    pub profile: String,
    pub engine_name: Option<String>,
    pub engine_consented: bool,
    pub side_to_move: Side,
    pub move_number: u16,
    pub game_over: bool,
    pub in_check: bool,
    /// Lado que corre e o tempo que lhe resta, se o relógio está baixo.
    pub low_clock: Option<(Side, u64)>,
    /// FPS a mostrar; `None` com a preferência desligada.
    pub fps: Option<f32>,
    pub scale_percent: u16,
    /// Casa sob o cursor (o painel antigo mostrava "Casa sob o cursor").
    pub hovered: Option<Square>,
    /// Vista da câmera; `None` é a câmera livre.
    pub view: Option<CameraView>,
}

/// Engine e consentimento: o ponto leva a cor do `tone`; o texto, a palavra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineChip {
    pub name: String,
    pub consent: String,
    pub tone: Tone,
}

/// Os textos da barra, prontos para desenhar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusText {
    pub profile: String,
    pub engine: EngineChip,
    pub turn: String,
    /// "Casa: e4" (vazio fora do tabuleiro) e "Vista: Padrão".
    pub hover: Option<String>,
    pub view: String,
    pub warnings: Vec<String>,
    pub fps: Option<String>,
    pub scale: String,
}

/// O relógio de quem corre, se está abaixo de [`LOW_CLOCK_MS`] e a Partida segue.
pub fn low_clock(reading: &ClockReading) -> Option<(Side, u64)> {
    if reading.paused {
        return None;
    }
    let side = reading.running?;
    let left = reading.remaining(side);
    (left < LOW_CLOCK_MS).then_some((side, left))
}

/// `m:ss` (arredonda para cima, como o relógio de mesa).
fn clock_text(ms: u64) -> String {
    let secs = ms.div_ceil(1000);
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Deriva os textos da barra do instantâneo.
pub fn status_text(snapshot: &StatusSnapshot, i18n: &I18n) -> StatusText {
    let engine = EngineChip {
        name: i18n.t_with(
            "statusbar-engine",
            &[(
                "name",
                &snapshot
                    .engine_name
                    .clone()
                    .unwrap_or_else(|| i18n.t("statusbar-engine-none")),
            )],
        ),
        consent: i18n.t(if snapshot.engine_consented {
            "statusbar-engine-consented"
        } else {
            "statusbar-engine-not-consented"
        }),
        tone: if snapshot.engine_consented {
            Tone::Success
        } else {
            Tone::Warning
        },
    };
    let move_text = i18n.t_with(
        "statusbar-move",
        &[("number", &snapshot.move_number.to_string())],
    );
    let turn = if snapshot.game_over {
        format!("{} · {move_text}", i18n.t("statusbar-game-over"))
    } else {
        let side = i18n.t(match snapshot.side_to_move {
            Side::White => "statusbar-turn-white",
            Side::Black => "statusbar-turn-black",
        });
        format!("{side} · {move_text}")
    };
    let mut warnings = Vec::new();
    if snapshot.in_check && !snapshot.game_over {
        warnings.push(i18n.t("statusbar-warn-check"));
    }
    if let Some((side, ms)) = snapshot.low_clock
        && !snapshot.game_over
    {
        let side = i18n.t(match side {
            Side::White => "statusbar-side-white",
            Side::Black => "statusbar-side-black",
        });
        warnings.push(i18n.t_with(
            "statusbar-warn-low-clock",
            &[("side", &side), ("time", &clock_text(ms))],
        ));
    }
    let view_name = snapshot.view.map_or_else(
        || i18n.t("view-free"),
        |v| {
            i18n.t(match v {
                CameraView::Top => "view-top",
                CameraView::Oblique => "view-oblique",
                CameraView::Standard => "view-standard",
                CameraView::Inverted => "view-inverted",
            })
        },
    );
    StatusText {
        hover: snapshot
            .hovered
            .map(|sq| i18n.t_with("statusbar-hover", &[("square", &sq.to_string())])),
        view: i18n.t_with("view-label", &[("name", &view_name)]),
        profile: i18n.t_with("statusbar-profile", &[("name", &snapshot.profile)]),
        engine,
        turn,
        warnings,
        fps: snapshot
            .fps
            .map(|fps| i18n.t_with("statusbar-fps", &[("fps", &format!("{fps:.0}"))])),
        scale: i18n.t_with(
            "statusbar-scale",
            &[("percent", &snapshot.scale_percent.to_string())],
        ),
    }
}

const INSET_ID: &str = "chess-studio/status-bar/bottom-inset";

/// Altura (em pontos) que a barra de status reserva no pé da janela; 0 sem a barra (Foco).
pub fn bottom_inset(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new(INSET_ID)))
        .unwrap_or(0.0)
}

pub(crate) fn set_bottom_inset(ctx: &egui::Context, inset: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INSET_ID), inset));
}

pub struct StatusBarPlugin;

impl Plugin for StatusBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StatusBarPrefs>().add_systems(
            EguiPrimaryContextPass,
            status_bar.after(ui_theme::apply_appearance),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn status_bar(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    session: Res<Session>,
    analysis: Res<Analysis>,
    appearance: Res<Appearance>,
    view: Res<ViewState>,
    dock: Res<Dock>,
    time: Res<Time>,
    mut prefs: ResMut<StatusBarPrefs>,
    mut profiles: ResMut<InterfaceProfiles>,
    mut actions: MessageWriter<ActionRequested>,
    hovered: Res<crate::picking::HoveredSquare>,
    rig: Res<crate::camera::CameraRig>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut fps: Local<f32>,
) -> Result {
    // Média móvel: o número não pisca a cada quadro.
    let dt = time.delta_secs();
    if dt > 0.0 {
        let now = 1.0 / dt;
        *fps = if *fps == 0.0 {
            now
        } else {
            *fps * 0.95 + now * 0.05
        };
    }
    let ctx = contexts.ctx_mut()?;
    // Foco e capturas limpas (sem `--panel`): sem barra e sem faixa reservada.
    if view.focus || capture.is_some_and(|c| !c.show_panel) {
        set_bottom_inset(ctx, 0.0);
        return Ok(());
    }
    let snap = session.snapshot();
    let status = StatusSnapshot {
        profile: profiles.active_label(&i18n),
        engine_name: analysis.engine_label(),
        engine_consented: analysis.engine_available(),
        side_to_move: snap.side_to_move,
        move_number: snap.position.fullmove_number(),
        game_over: snap.outcome.is_some(),
        in_check: snap.in_check,
        low_clock: session.clock_reading().as_ref().and_then(low_clock),
        fps: prefs.show_fps.then_some(*fps),
        scale_percent: appearance.scale.percent(),
        hovered: hovered.0,
        view: rig.view,
    };
    let text = status_text(&status, &i18n);

    let p = Palette::from_visuals(&ctx.global_style().visuals);
    let height = Tokens::embedded().length("statusbar-h").unwrap_or(24.0);
    let frame = egui::Frame::new()
        .fill(p.surface_1)
        .stroke(egui::Stroke::new(1.0, p.border))
        .inner_margin(egui::Margin::symmetric(12, 0));
    set_bottom_inset(ctx, height);
    let analysis_visible = dock.layout.is_visible("Analysis");

    // O egui só dá painéis a partir de um `Ui` raiz: monta o mesmo que `Context::run_ui`.
    let mut root = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("statusbar_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    let mut chosen: Option<ProfileKey> = None;
    egui::Panel::bottom("statusbar_panel")
        .exact_size(height)
        .resizable(false)
        .show_separator_line(false)
        .frame(frame)
        .show(&mut root, |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
            ui.visuals_mut().override_text_color = Some(p.ink_muted);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let profile = ui
                    .add(egui::Label::new(&text.profile).sense(egui::Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(i18n.t("statusbar-profile-hint"));
                egui::Popup::menu(&profile).show(|ui| {
                    chosen = profile_menu(ui, &i18n, &profiles);
                });
                let engine = engine_chip(ui, p, &text.engine)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(i18n.t("statusbar-engine-hint"));
                if engine.clicked() && !analysis_visible {
                    actions.write(ActionRequested(Action::PanelAnalysis));
                }
                ui.label(&text.turn);
                if let Some(hover) = &text.hover {
                    ui.label(hover);
                }
                ui.label(&text.view);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    // Números tabulares: FPS e Escala não empurram os vizinhos ao mudar.
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    let ink = ui.visuals().text_color();
                    let scale = crate::ui_theme::tabular_text(ui, &text.scale, font.clone(), ink);
                    scale.context_menu(|ui| {
                        if ui
                            .checkbox(&mut prefs.show_fps, i18n.t("statusbar-show-fps"))
                            .changed()
                        {
                            ui.close();
                        }
                    });
                    if let Some(fps) = &text.fps {
                        crate::ui_theme::tabular_text(ui, fps, font, ink);
                    }
                    for warning in &text.warnings {
                        ui.label(
                            egui::RichText::new(warning)
                                .color(p.warning)
                                .text_style(egui::TextStyle::Small),
                        );
                    }
                });
            });
        });
    if let Some(key) = chosen {
        profiles.request = Some(ProfileRequest::Apply(key));
    }
    Ok(())
}

/// Ponto colorido e palavra do consentimento, depois do nome da Engine.
fn engine_chip(ui: &mut egui::Ui, p: &Palette, chip: &EngineChip) -> egui::Response {
    let color = match chip.tone {
        Tone::Success => p.success,
        Tone::Warning => p.warning,
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(&chip.name);
        let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 4.0, color);
        ui.label(egui::RichText::new(&chip.consent).color(color));
    })
    .response
    .interact(egui::Sense::click())
}

/// O seletor de Perfil de interface (os mesmos perfis da janela principal).
fn profile_menu(
    ui: &mut egui::Ui,
    i18n: &I18n,
    profiles: &InterfaceProfiles,
) -> Option<ProfileKey> {
    let mut chosen = None;
    let mut item = |ui: &mut egui::Ui, key: ProfileKey, label: String| {
        if ui
            .selectable_label(profiles.state.active == key, label)
            .clicked()
        {
            chosen = Some(key);
        }
    };
    for f in FactoryProfile::ALL {
        item(ui, ProfileKey::Factory(f), i18n.t(f.label_key()));
    }
    for u in &profiles.state.user {
        item(ui, ProfileKey::User(u.name.clone()), u.name.clone());
    }
    item(ui, ProfileKey::Custom, i18n.t("iprofile-custom"));
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> StatusSnapshot {
        StatusSnapshot {
            profile: "Estudo e análise".into(),
            engine_name: Some("Stockfish".into()),
            engine_consented: true,
            side_to_move: Side::White,
            move_number: 12,
            game_over: false,
            in_check: false,
            low_clock: None,
            fps: None,
            scale_percent: 100,
            hovered: None,
            view: Some(CameraView::Standard),
        }
    }

    fn text(s: &StatusSnapshot) -> StatusText {
        status_text(s, &I18n::pt_br())
    }

    #[test]
    fn shows_profile_engine_turn_move_and_scale() {
        let t = text(&snapshot());
        assert_eq!(t.profile, "Perfil: Estudo e análise");
        assert_eq!(t.engine.name, "Engine: Stockfish");
        assert_eq!(t.engine.consent, "Consentida");
        assert_eq!(t.engine.tone, Tone::Success);
        assert_eq!(t.turn, "Vez das brancas · Lance 12");
        assert_eq!(t.scale, "Escala 100%");
        assert_eq!(t.hover, None);
        assert_eq!(t.view, "Vista: Padrão");
        assert!(t.warnings.is_empty());
        assert_eq!(t.fps, None);
    }

    #[test]
    fn engine_without_consent_is_a_warning_with_a_word() {
        let t = text(&StatusSnapshot {
            engine_name: None,
            engine_consented: false,
            ..snapshot()
        });
        assert_eq!(t.engine.name, "Engine: nenhuma");
        assert_eq!(t.engine.consent, "Sem consentimento");
        assert_eq!(t.engine.tone, Tone::Warning);
    }

    #[test]
    fn black_to_move_and_game_over() {
        let t = text(&StatusSnapshot {
            side_to_move: Side::Black,
            move_number: 3,
            ..snapshot()
        });
        assert_eq!(t.turn, "Vez das pretas · Lance 3");
        let over = text(&StatusSnapshot {
            game_over: true,
            in_check: true,
            low_clock: Some((Side::White, 5_000)),
            ..snapshot()
        });
        assert_eq!(over.turn, "Partida encerrada · Lance 12");
        assert!(over.warnings.is_empty(), "sem avisos depois do fim");
    }

    #[test]
    fn warns_about_check_and_low_clock() {
        let t = text(&StatusSnapshot {
            in_check: true,
            low_clock: Some((Side::Black, 24_300)),
            ..snapshot()
        });
        assert_eq!(t.warnings, ["Xeque", "Pouco tempo: pretas 0:25"]);
    }

    #[test]
    fn shows_the_hovered_square_and_the_free_camera() {
        let t = text(&StatusSnapshot {
            hovered: "e4".parse().ok(),
            view: None,
            ..snapshot()
        });
        assert_eq!(t.hover.as_deref(), Some("Casa: e4"));
        assert_eq!(t.view, "Vista: Livre");
    }

    #[test]
    fn fps_and_scale_follow_the_snapshot() {
        let t = text(&StatusSnapshot {
            fps: Some(59.6),
            scale_percent: 125,
            ..snapshot()
        });
        assert_eq!(t.fps.as_deref(), Some("60 FPS"));
        assert_eq!(t.scale, "Escala 125%");
    }

    fn reading(running: Option<Side>, paused: bool, white_ms: u64, black_ms: u64) -> ClockReading {
        ClockReading {
            white_ms,
            black_ms,
            running,
            paused,
            flag: None,
        }
    }

    #[test]
    fn low_clock_only_for_the_running_side_below_the_threshold() {
        assert_eq!(
            low_clock(&reading(Some(Side::White), false, 29_999, 5_000)),
            Some((Side::White, 29_999))
        );
        assert_eq!(
            low_clock(&reading(Some(Side::White), false, 30_000, 5_000)),
            None
        );
        assert_eq!(
            low_clock(&reading(Some(Side::White), true, 1_000, 5_000)),
            None
        );
        assert_eq!(low_clock(&reading(None, false, 1_000, 1_000)), None);
    }
}
