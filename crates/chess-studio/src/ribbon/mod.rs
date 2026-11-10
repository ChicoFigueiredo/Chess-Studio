//! Ribbon, Barra de Acesso Rápido, KeyTips e bastidores (DS-3, issue #162; ADR-0005).
//!
//! Todo item é uma Ação da lista única
//! (`chess_input::actions`): rótulo, ícone, KeyTip, grupo, estado e atalho vêm dela. Clicar,
//! digitar um KeyTip e apertar o atalho escrevem o mesmo `ActionRequested`.
//!
//! Só Apresentação (ADR-0001): nada aqui toca a Partida.

mod backstage;
mod icons;
mod layout_dialog;
pub mod model;
mod view;

use crate::actions::ActionRequested;
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use bevy_egui::egui;
use bevy_egui::input::EguiWantsInput;
use chess_input::actions::{ActionContext, RibbonTab, state};
use chess_input::controls::Action;
use model::{KeyTipInput, KeyTipOutcome, RibbonView};

pub use backstage::BackstagePage;

/// Modo do Ribbon aberto (o recolhido é um estado à parte, Ctrl+F1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RibbonMode {
    /// Botões grandes e pilhas de pequenos, rótulo do grupo embaixo.
    #[default]
    Classic,
    /// Uma linha; o que não cabe vai para o menu "⋯".
    Simple,
}

impl RibbonState {
    /// O que do Ribbon entra no Perfil de interface (#166): modo, recolhido, aba ativa e a
    /// Barra de Acesso Rápido.
    pub fn setup(&self) -> crate::profiles::RibbonSetup {
        crate::profiles::RibbonSetup {
            layout: match self.mode {
                RibbonMode::Classic => crate::profiles::RibbonLayout::Classic,
                RibbonMode::Simple => crate::profiles::RibbonLayout::Simple,
            },
            collapsed: self.collapsed,
            active_tab: Some(self.active.id().to_owned()),
            quick_access: self
                .quick_access
                .iter()
                .map(|a| a.id().to_owned())
                .collect(),
        }
    }

    /// Aplica um [`RibbonSetup`](crate::profiles::RibbonSetup); aba ausente ou desconhecida
    /// mantém a atual, ação desconhecida sai da Barra de Acesso Rápido.
    pub fn apply_setup(&mut self, setup: &crate::profiles::RibbonSetup) {
        let mode = match setup.layout {
            crate::profiles::RibbonLayout::Classic => RibbonMode::Classic,
            crate::profiles::RibbonLayout::Simple => RibbonMode::Simple,
        };
        let quick: Vec<Action> = setup
            .quick_access
            .iter()
            .filter_map(|id| Action::from_id(id))
            .collect();
        let active = setup.active_tab.as_deref().and_then(RibbonView::from_id);
        if self.mode != mode {
            self.mode = mode;
        }
        if self.collapsed != setup.collapsed {
            self.collapsed = setup.collapsed;
            self.body_open = false;
        }
        if self.quick_access != quick {
            self.quick_access = quick;
        }
        if let Some(active) = active
            && self.active != active
        {
            self.active = active;
            self.body_open = false;
        }
    }
}

/// Retângulos que o Ribbon registra a cada quadro, para os selos dos KeyTips.
#[derive(Debug, Default, Clone)]
pub struct Registered {
    pub tabs: Vec<(RibbonView, egui::Rect)>,
    pub actions: Vec<(Action, egui::Rect)>,
}

/// Estado do Ribbon. O modo, o recolhido, a aba ativa e a Barra de Acesso Rápido entram no
/// Perfil de interface (#166) e são gravados nas preferências (`persistence`).
#[derive(Resource, Debug)]
pub struct RibbonState {
    pub mode: RibbonMode,
    pub collapsed: bool,
    pub active: RibbonView,
    /// Recolhido: o corpo da aba aberto por cima do tabuleiro.
    pub body_open: bool,
    /// Bastidores abertos, com a página selecionada.
    pub backstage: Option<BackstagePage>,
    /// Ações fixas na Barra de Acesso Rápido, na ordem.
    pub quick_access: Vec<Action>,
    pub keytips: model::KeyTipNav,
    /// Os KeyTips consumiram as teclas deste quadro: os atalhos não disparam.
    pub keys_captured: bool,
    pub registered: Registered,
}

impl Default for RibbonState {
    fn default() -> Self {
        Self {
            mode: RibbonMode::Classic,
            collapsed: false,
            active: RibbonView::Tab(RibbonTab::Home),
            body_open: false,
            backstage: None,
            quick_access: vec![Action::Undo, Action::NewGame, Action::FlipBoard],
            keytips: model::KeyTipNav::default(),
            keys_captured: false,
            registered: Registered::default(),
        }
    }
}

impl RibbonState {
    /// A Ação está na Barra de Acesso Rápido.
    pub fn is_pinned(&self, action: Action) -> bool {
        self.quick_access.contains(&action)
    }

    /// Fixa ou solta a Ação da Barra de Acesso Rápido.
    pub fn set_pinned(&mut self, action: Action, pinned: bool) {
        match (pinned, self.is_pinned(action)) {
            (true, false) => self.quick_access.push(action),
            (false, true) => self.quick_access.retain(|a| *a != action),
            _ => {}
        }
    }
}

/// O instantâneo de Ações do quadro anterior ao desenho: o Ribbon não lê o app, lê isto.
#[derive(Resource, Debug, Default)]
pub struct RibbonActionContext(pub ActionContext);

const INSET_ID: &str = "chess-studio/ribbon/top-inset";

/// Altura (em pontos) que o Ribbon ocupa no topo da janela; 0 sem Ribbon.
pub fn top_inset(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new(INSET_ID)))
        .unwrap_or(0.0)
}

/// A área da janela que sobra para o tabuleiro e o dock: tudo abaixo do Ribbon e acima da barra
/// de status.
pub fn usable_rect(ctx: &egui::Context) -> egui::Rect {
    let mut rect = ctx.viewport_rect();
    let top = top_inset(ctx).min(rect.height() * 0.5);
    let bottom = crate::status_bar::bottom_inset(ctx).min(rect.height() * 0.25);
    // A faixa de Painéis (#185) reserva a borda direita.
    let right = crate::panel_stripe::right_inset(ctx).min(rect.width() * 0.25);
    rect.min.y += top;
    rect.max.y -= bottom;
    rect.max.x -= right;
    rect
}

pub(crate) fn set_top_inset(ctx: &egui::Context, inset: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INSET_ID), inset));
}

pub struct RibbonPlugin;

impl Plugin for RibbonPlugin {
    fn build(&self, app: &mut App) {
        let mut state = RibbonState::default();
        if let Some(setup) = crate::persistence::load_ribbon_setup(app.world()) {
            state.apply_setup(&setup);
        }
        apply_capture_options(&mut state);
        app.insert_resource(state)
            .init_resource::<RibbonActionContext>()
            .add_systems(Update, crate::persistence::persist_ribbon)
            .add_systems(
                Update,
                (refresh_context, keytips_input, collapse_requested)
                    .chain()
                    .before(crate::actions::dispatch_shortcuts),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (view::ribbon_ui, backstage::backstage_ui)
                    .chain()
                    .after(crate::ui_theme::apply_appearance),
            );
    }
}

/// Opções de captura para as evidências: `--ribbon simple|collapsed`, `--ribbon-tab <aba>`,
/// `--keytips` e `--backstage`.
fn apply_capture_options(state: &mut RibbonState) {
    use crate::capture::option_value;
    match option_value("--ribbon").as_deref() {
        Some("simple") => state.mode = RibbonMode::Simple,
        Some("collapsed") => state.collapsed = true,
        _ => {}
    }
    if let Some(tab) = option_value("--ribbon-tab") {
        let view = [
            ("file", RibbonView::Tab(RibbonTab::File)),
            ("home", RibbonView::Tab(RibbonTab::Home)),
            ("analysis", RibbonView::Tab(RibbonTab::Analysis)),
            ("library", RibbonView::Tab(RibbonTab::Library)),
            ("training", RibbonView::Tab(RibbonTab::Training)),
            ("view", RibbonView::Tab(RibbonTab::View)),
            ("scoresheet", RibbonView::Scoresheet),
            ("board", RibbonView::Board),
        ]
        .into_iter()
        .find(|(name, _)| *name == tab)
        .map(|(_, v)| v);
        if let Some(view) = view {
            state.active = view;
            state.body_open = state.collapsed;
        }
    }
    if std::env::args().any(|a| a == "--keytips") {
        state
            .keytips
            .handle(KeyTipInput::AltTap, &model::visible_tabs(false), |_| true);
        if let Some(letter) = option_value("--keytips-tab").and_then(|s| s.chars().next()) {
            state.keytips.handle(
                KeyTipInput::Char(letter),
                &model::visible_tabs(false),
                |_| true,
            );
        }
    }
    if std::env::args().any(|a| a == "--backstage") {
        state.backstage = Some(BackstagePage::Profiles);
    }
}

/// Monta o `ActionContext` do quadro com tudo o que o app sabe (e o que é do Ribbon).
fn refresh_context(env: crate::actions::ActionEnv, mut cache: ResMut<RibbonActionContext>) {
    let ctx = env.context();
    if cache.0 != ctx {
        cache.0 = ctx;
    }
}

/// Letra ou dígito de uma tecla, para os KeyTips.
fn key_char(key: KeyCode) -> Option<char> {
    Some(match key {
        KeyCode::KeyA => 'A',
        KeyCode::KeyB => 'B',
        KeyCode::KeyC => 'C',
        KeyCode::KeyD => 'D',
        KeyCode::KeyE => 'E',
        KeyCode::KeyF => 'F',
        KeyCode::KeyG => 'G',
        KeyCode::KeyH => 'H',
        KeyCode::KeyI => 'I',
        KeyCode::KeyJ => 'J',
        KeyCode::KeyK => 'K',
        KeyCode::KeyL => 'L',
        KeyCode::KeyM => 'M',
        KeyCode::KeyN => 'N',
        KeyCode::KeyO => 'O',
        KeyCode::KeyP => 'P',
        KeyCode::KeyQ => 'Q',
        KeyCode::KeyR => 'R',
        KeyCode::KeyS => 'S',
        KeyCode::KeyT => 'T',
        KeyCode::KeyU => 'U',
        KeyCode::KeyV => 'V',
        KeyCode::KeyW => 'W',
        KeyCode::KeyX => 'X',
        KeyCode::KeyY => 'Y',
        KeyCode::KeyZ => 'Z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        _ => return None,
    })
}

fn is_alt(key: KeyCode) -> bool {
    matches!(key, KeyCode::AltLeft | KeyCode::AltRight)
}

/// Alt apertado e solto sozinho liga os KeyTips; depois as letras os conduzem. Enquanto
/// estão ativos, as teclas do quadro não viram atalhos (`keys_captured`).
fn keytips_input(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Option<Res<EguiWantsInput>>,
    context: Res<RibbonActionContext>,
    mut state: ResMut<RibbonState>,
    mut out: MessageWriter<ActionRequested>,
    mut alt_armed: Local<bool>,
) {
    let was_active = state.keytips.is_active();
    let typing = egui.is_some_and(|e| e.wants_keyboard_input());
    let mut inputs: Vec<KeyTipInput> = Vec::new();

    if keys.get_just_pressed().any(|k| is_alt(*k)) {
        *alt_armed = true;
    } else if keys.get_just_pressed().any(|k| !is_alt(*k)) {
        *alt_armed = false;
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if was_active && !ctrl {
        for key in keys.get_just_pressed() {
            if *key == KeyCode::Escape {
                inputs.push(KeyTipInput::Escape);
            } else if let Some(c) = key_char(*key) {
                inputs.push(KeyTipInput::Char(c));
            }
        }
    }
    if keys.any_just_released([KeyCode::AltLeft, KeyCode::AltRight]) && *alt_armed {
        *alt_armed = false;
        if !typing || was_active {
            inputs.push(KeyTipInput::AltTap);
        }
    }

    let scoresheet_open = context.0.visible_panels.contains("Scoresheet");
    let tabs = model::visible_tabs_for(
        scoresheet_open,
        model::board_tab_visible(context.0.presentation),
    );
    let mut captured = was_active && !inputs.is_empty();
    for input in inputs {
        let outcome = state
            .keytips
            .handle(input, &tabs, |a| state_of(a, &context.0));
        match outcome {
            KeyTipOutcome::None => {}
            KeyTipOutcome::OpenTab(view) => {
                state.active = view;
                state.body_open = state.collapsed;
            }
            KeyTipOutcome::OpenBackstage => {
                state.backstage = Some(BackstagePage::Profiles);
                state.body_open = false;
            }
            KeyTipOutcome::Perform(action) => {
                out.write(ActionRequested(action));
                state.body_open = false;
            }
        }
        captured = true;
    }
    if state.keytips.is_active() {
        captured = true;
    }
    if state.keys_captured != captured {
        state.keys_captured = captured;
    }
}

fn state_of(action: Action, ctx: &ActionContext) -> bool {
    state(action, ctx).is_enabled()
}

/// Ctrl+F1 (ou o botão do Ribbon) recolhe e expande o Ribbon.
fn collapse_requested(
    mut requests: MessageReader<ActionRequested>,
    mut state: ResMut<RibbonState>,
) {
    for request in requests.read() {
        if request.0 == Action::ToggleRibbonCollapse {
            state.collapsed = !state.collapsed;
            state.body_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{press, test_app};
    use chess_input::controls::Action;

    fn app() -> App {
        let mut app = test_app();
        app.init_resource::<RibbonState>()
            .init_resource::<RibbonActionContext>()
            .add_systems(
                Update,
                (refresh_context, keytips_input, collapse_requested)
                    .chain()
                    .before(crate::actions::dispatch_shortcuts),
            );
        app
    }

    /// Alt apertado e solto sozinho, em dois quadros.
    fn tap_alt(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltLeft);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::AltLeft);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    #[test]
    fn a_ribbon_click_requests_the_same_action_as_the_shortcut() {
        // O clique escreve `ActionRequested(ação)`; o atalho também. O mesmo efeito.
        let mut by_key = app();
        press(&mut by_key, KeyCode::KeyH);
        let hud_by_key = by_key.world().resource::<crate::hud::HudSettings>().enabled;

        let mut by_click = app();
        by_click
            .world_mut()
            .write_message(ActionRequested(Action::ToggleHud));
        by_click.update();
        let hud_by_click = by_click
            .world()
            .resource::<crate::hud::HudSettings>()
            .enabled;
        assert_eq!(hud_by_key, hud_by_click);
        assert!(!hud_by_click);
    }

    #[test]
    fn alt_then_letters_perform_an_action_without_firing_the_plain_shortcut() {
        let mut app = app();
        tap_alt(&mut app);
        assert!(app.world().resource::<RibbonState>().keytips.is_active());
        // Aba Exibir (X) e KeyTip "TH" (Mostrar/ocultar o HUD): "H" sozinho também é atalho.
        press(&mut app, KeyCode::KeyX);
        press(&mut app, KeyCode::KeyT);
        assert!(app.world().resource::<crate::hud::HudSettings>().enabled);
        press(&mut app, KeyCode::KeyH);
        assert!(!app.world().resource::<crate::hud::HudSettings>().enabled);
        assert!(!app.world().resource::<RibbonState>().keytips.is_active());
        // De volta ao normal: a tecla H volta a ser atalho.
        press(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<crate::hud::HudSettings>().enabled);
    }

    #[test]
    fn keys_do_not_fire_shortcuts_while_the_keytips_are_on() {
        let mut app = app();
        tap_alt(&mut app);
        // "H" não é letra de aba: nada acontece, e o atalho H do HUD também não dispara.
        press(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<crate::hud::HudSettings>().enabled);
        assert!(app.world().resource::<RibbonState>().keytips.is_active());
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<RibbonState>().keytips.is_active());
    }

    #[test]
    fn the_collapse_action_toggles_the_ribbon() {
        let mut app = app();
        app.world_mut()
            .write_message(ActionRequested(Action::ToggleRibbonCollapse));
        app.update();
        assert!(app.world().resource::<RibbonState>().collapsed);
        app.world_mut()
            .write_message(ActionRequested(Action::ToggleRibbonCollapse));
        app.update();
        assert!(!app.world().resource::<RibbonState>().collapsed);
    }

    #[test]
    fn pinning_to_the_quick_access_bar_is_idempotent() {
        let mut state = RibbonState::default();
        assert_eq!(
            state.quick_access,
            [Action::Undo, Action::NewGame, Action::FlipBoard]
        );
        state.set_pinned(Action::Redo, true);
        state.set_pinned(Action::Redo, true);
        assert_eq!(state.quick_access.len(), 4);
        state.set_pinned(Action::Undo, false);
        assert!(!state.is_pinned(Action::Undo));
    }

    #[test]
    fn the_ribbon_setup_round_trips_and_ignores_unknown_ids() {
        let mut state = RibbonState {
            mode: RibbonMode::Simple,
            collapsed: true,
            active: RibbonView::Scoresheet,
            quick_access: vec![Action::Redo],
            ..RibbonState::default()
        };
        let setup = state.setup();
        let mut other = RibbonState::default();
        other.apply_setup(&setup);
        assert_eq!(other.setup(), setup);
        assert_eq!(other.mode, RibbonMode::Simple);
        assert_eq!(other.active, RibbonView::Scoresheet);
        // Aba e Ações desconhecidas (versão futura): aba atual fica, Ação sai.
        let mut odd = setup.clone();
        odd.active_tab = Some("nao-existe".into());
        odd.quick_access.push("acao-nao-existe".into());
        state.apply_setup(&odd);
        assert_eq!(state.active, RibbonView::Scoresheet);
        assert_eq!(state.quick_access, vec![Action::Redo]);
        for tab in model::visible_tabs(true) {
            assert_eq!(RibbonView::from_id(tab.id()), Some(tab));
        }
    }

    #[test]
    fn the_usable_rect_starts_below_the_ribbon() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let full = ctx.viewport_rect();
        assert_eq!(usable_rect(&ctx), full);
        set_top_inset(&ctx, 100.0);
        assert!((usable_rect(&ctx).min.y - (full.min.y + 100.0).min(full.center().y)).abs() < 1e-3);
    }

    #[test]
    fn the_usable_rect_ends_above_the_status_bar() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let full = ctx.viewport_rect();
        crate::status_bar::set_bottom_inset(&ctx, 24.0);
        assert!((usable_rect(&ctx).max.y - (full.max.y - 24.0)).abs() < 1e-3);
        // Foco: a barra some e devolve a faixa.
        crate::status_bar::set_bottom_inset(&ctx, 0.0);
        assert_eq!(usable_rect(&ctx), full);
    }
}
