//! Tela cheia (UI-03) e modo Foco (UI-04/05, BDD-09): dois estados de interface
//! **independentes**, só Apresentação. Nenhum deles toca a Partida, o relógio ou o turno
//! (ADR-0001): os sistemas daqui não recebem a `Session` para escrita, exceto o botão do
//! relógio compacto do 2D, que envia a mesma intenção do relógio da mesa.
//!
//! - **Tela cheia**: F11 ou "Tela cheia" na janela principal alterna `BorderlessFullscreen`
//!   e `Windowed`, lembrando tamanho e posição da janela para a volta. Alguns sistemas
//!   interceptam F11 (macOS usa Ctrl+Cmd+F para a tela cheia do sistema); o item de menu
//!   funciona em todos. Ver `docs/decisoes/2026-11-06-tela-cheia-foco.md`.
//! - **Modo Foco**: F10 ou "Modo Foco" na janela principal esconde o dock e a janela
//!   principal, deixando tabuleiro, mesa, relógio 3D (e um relógio compacto no 2D técnico),
//!   mais o botão discreto "Sair do Foco". Janelas que a Partida exige (promoção, retomada,
//!   Nova Partida em curso) continuam. Os HUDs permitidos pela Política (C5) devem desenhar
//!   sob a condição [`in_focus`]/[`ViewState::focus`]: o Foco não os esconde.
//!
//! Ambos são preferências gravadas no ato (ver `persistence`).

use crate::clock::{clock_text, side_of_slot};
use crate::i18n::I18n;
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode, WindowPosition};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;

/// Estado de interface de tela cheia e Foco. Independentes entre si.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub focus: bool,
    pub fullscreen: bool,
}

impl ViewState {
    pub fn toggle_focus(&mut self) {
        self.focus = !self.focus;
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }
}

/// Condição de execução para sistemas que só valem no modo Foco (ganchos de HUD, C5).
#[allow(dead_code)] // gancho para os HUDs do C5
pub fn in_focus(state: Res<ViewState>) -> bool {
    state.focus
}

/// Modo de janela de cada estado de tela cheia.
pub fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    }
}

/// Tamanho e posição da janela antes da tela cheia, para a volta.
#[derive(Resource, Default, Debug)]
struct WindowedGeometry(Option<(Vec2, WindowPosition)>);

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        // `init_resource` preserva o estado carregado do banco pelo `PersistencePlugin`.
        app.init_resource::<ViewState>()
            .init_resource::<WindowedGeometry>()
            .add_systems(Update, (view_shortcuts, apply_fullscreen).chain())
            .add_systems(EguiPrimaryContextPass, focus_overlay);
    }
}

/// F11 alterna a tela cheia; F10, o modo Foco. Teclas de função não digitam texto, então
/// valem mesmo com um campo do egui em foco.
fn view_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<ViewState>) {
    let mut next = *state;
    if keys.just_pressed(KeyCode::F11) {
        next.toggle_fullscreen();
    }
    if keys.just_pressed(KeyCode::F10) {
        next.toggle_focus();
    }
    // Só escreve se mudou: `is_changed` aciona a janela e a gravação da preferência.
    if next != *state {
        *state = next;
    }
}

/// Leva a janela ao modo pedido (também no início, com a preferência carregada).
fn apply_fullscreen(
    state: Res<ViewState>,
    mut memory: ResMut<WindowedGeometry>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let wanted = window_mode(state.fullscreen);
    if window.mode == wanted {
        return;
    }
    if state.fullscreen {
        if window.mode == WindowMode::Windowed {
            memory.0 = Some((window.resolution.size(), window.position));
        }
        window.mode = wanted;
    } else {
        window.mode = wanted;
        if let Some((size, position)) = memory.0.take() {
            window.resolution.set(size.x, size.y);
            if matches!(position, WindowPosition::At(_)) {
                window.position = position;
            }
        }
    }
}

/// Seção "Exibição" da janela principal: alternativas de menu para F11 e F10.
pub fn view_section(ui: &mut egui::Ui, i18n: &I18n, state: &mut ViewState) {
    ui.strong(i18n.t("view-heading"));
    ui.checkbox(&mut state.fullscreen, i18n.t("view-fullscreen"));
    ui.checkbox(&mut state.focus, i18n.t("view-focus"));
}

/// Controles mínimos do Foco: "Sair do Foco" e, no 2D técnico (sem relógio na mesa), o
/// relógio compacto.
fn focus_overlay(
    mut contexts: EguiContexts,
    mut state: ResMut<ViewState>,
    i18n: Res<I18n>,
    presentation: Res<Presentation>,
    mut session: ResMut<Session>,
) -> Result {
    if !state.focus {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut leave = false;
    egui::Area::new(egui::Id::new("focus-exit"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            leave = ui
                .add(egui::Button::new(
                    egui::RichText::new(i18n.t("focus-exit")).small().weak(),
                ))
                .on_hover_text(i18n.t("focus-exit-hint"))
                .clicked();
        });
    if !presentation.is_3d() {
        egui::Area::new(egui::Id::new("focus-clock"))
            .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                compact_clock(ui, &i18n, &mut session);
            });
    }
    if leave {
        state.focus = false;
    }
    Ok(())
}

/// Relógio mínimo do 2D técnico: os dois mostradores e o botão. O 3D usa o relógio da mesa.
fn compact_clock(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session) {
    let Some(reading) = session.clock_reading() else {
        return;
    };
    let user = session.user_side();
    for slot in 0..2 {
        let side = side_of_slot(user, slot);
        let ms = reading.remaining(side);
        let running = reading.running == Some(side) && !reading.paused;
        let color = if ms == 0 {
            egui::Color32::from_rgb(255, 60, 40)
        } else if running {
            egui::Color32::from_rgb(255, 189, 36)
        } else {
            egui::Color32::from_rgb(160, 118, 30)
        };
        ui.horizontal(|ui| {
            ui.label(i18n.t(match side {
                Side::White => "clock-white",
                Side::Black => "clock-black",
            }));
            ui.label(
                egui::RichText::new(clock_text(ms))
                    .monospace()
                    .size(28.0)
                    .color(color),
            );
        });
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::new_game::NewGameConfig;
    use chess_game::{PolicyKind, TimeControl};

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ViewState>()
            .add_systems(Update, view_shortcuts);
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    }

    #[test]
    fn f11_and_f10_toggle_independent_states() {
        let mut app = app();
        press(&mut app, KeyCode::F11);
        assert_eq!(
            *app.world().resource::<ViewState>(),
            ViewState {
                focus: false,
                fullscreen: true
            }
        );
        press(&mut app, KeyCode::F10);
        assert_eq!(
            *app.world().resource::<ViewState>(),
            ViewState {
                focus: true,
                fullscreen: true
            }
        );
        press(&mut app, KeyCode::F11);
        assert_eq!(
            *app.world().resource::<ViewState>(),
            ViewState {
                focus: true,
                fullscreen: false
            }
        );
        press(&mut app, KeyCode::F10);
        assert_eq!(*app.world().resource::<ViewState>(), ViewState::default());
    }

    #[test]
    fn fullscreen_and_focus_never_touch_the_game_or_the_clock() {
        // BDD-09 / UI-04: com a Partida em andamento e o relógio correndo.
        let mut app = app();
        let config = NewGameConfig {
            policy: PolicyKind::FideSimulation,
            time: Some(TimeControl {
                base_ms: 600_000,
                increment_ms: 5_000,
            }),
            ..NewGameConfig::default()
        };
        let mut session = Session::new();
        session.start(&config, None);
        app.insert_resource(session);
        let before = {
            let s = app.world().resource::<Session>();
            (
                s.snapshot().clone(),
                s.game().events().len(),
                s.clock_reading(),
            )
        };
        for key in [KeyCode::F11, KeyCode::F10, KeyCode::F10, KeyCode::F11] {
            press(&mut app, key);
        }
        let s = app.world().resource::<Session>();
        assert_eq!(s.snapshot(), &before.0);
        assert_eq!(s.game().events().len(), before.1, "nenhum Evento novo");
        let after = s.clock_reading();
        assert_eq!(after.map(|r| r.paused), before.2.map(|r| r.paused));
        assert!(
            after.is_some_and(|r| !r.paused),
            "o relógio não foi pausado"
        );
    }

    #[test]
    fn window_mode_follows_the_fullscreen_flag() {
        assert_eq!(window_mode(false), WindowMode::Windowed);
        assert!(matches!(
            window_mode(true),
            WindowMode::BorderlessFullscreen(_)
        ));
    }
}
