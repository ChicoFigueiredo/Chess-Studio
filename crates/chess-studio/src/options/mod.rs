//! Diálogo Opções (DS-5, issue #164; `docs/design/componentes/OptionsDialog.md`): as
//! preferências persistentes por categoria. Cada linha tem rótulo, explicação curta e o controle
//! à direita; a mudança vale na hora.
//!
//! Nada aqui guarda preferência própria: toda linha lê e escreve o **mesmo recurso** que o
//! controle antigo do painel esquerdo usava (`Appearance`, `GraphicsQuality`, `AidSettings`…),
//! então a persistência existente continua valendo. Só Apresentação: nenhuma linha toca a Partida
//! (ADR-0001), exceto "Promover sempre a Dama", preferência do controlador já exposta por
//! `Session`.
//!
//! O inventário "item do painel esquerdo -> destino" está em [`inventory`].

// Dado de registro (tabela do PR #164), conferido pelos testes.
#[allow(dead_code)]
mod inventory;
mod theme_gallery;

use crate::aids::{AidProfile, AidSettings, ErrorExplanation};
use crate::camera::CameraRig;
use crate::controls::{Controls, ControlsUi};
use crate::coordinates::BoardCoordinates;
use crate::dock::Dock;
use crate::engine::{EngineUi, MAX_ELO, MIN_ELO};
use crate::hover::HoverHighlight;
use crate::hud::{HudProfile, HudSettings};
use crate::hud_policy::{self, Allowance};
use crate::i18n::I18n;
use crate::local_profiles::{LocalProfiles, Op};
use crate::presentation::{GpuSupport, GraphicsQuality, UltraEffects};
use crate::profiles::InterfaceProfiles;
use crate::session::Session;
use crate::ui_theme::{
    Appearance, ButtonVariant, Density, Palette, ThemeMode, UiScale, cs_button, cs_dropdown,
    cs_radio_value, cs_slider, cs_switch, icon_job, palette, strong,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;
use chess_input::{CameraView, MoveMethod};

/// Categorias da coluna da esquerda, na ordem exibida.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Category {
    #[default]
    Appearance,
    Board,
    Aids,
    Engines,
    Controls,
    Profile,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Appearance,
        Category::Board,
        Category::Aids,
        Category::Engines,
        Category::Controls,
        Category::Profile,
    ];

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as i;
        match self {
            Category::Appearance => i::PAINT_BRUSH,
            Category::Board => i::SQUARES_FOUR,
            Category::Aids => i::LIGHTBULB,
            Category::Engines => i::CPU,
            Category::Controls => i::KEYBOARD,
            Category::Profile => i::USER,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Category::Appearance => "options-cat-appearance",
            Category::Board => "options-cat-board",
            Category::Aids => "options-cat-aids",
            Category::Engines => "options-cat-engines",
            Category::Controls => "options-cat-controls",
            Category::Profile => "options-cat-profile",
        }
    }
}

/// Uma Opção do diálogo. É o vocabulário do inventário: cada item do painel esquerdo que não
/// é Ação aponta para um destes ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionId {
    Theme,
    Density,
    Scale,
    InterfaceProfile,
    Quality,
    Effects,
    Coordinates,
    Hover,
    CapturedPieces,
    MoveMethod,
    CameraView,
    AlwaysQueen,
    AidProfile,
    AidDestinations,
    AidGhosts,
    AidSelected,
    AidLastMove,
    AidCheck,
    AidExplanation,
    HudEnabled,
    HudProfile,
    EngineManager,
    EngineWizard,
    EngineElo,
    EngineSide,
    ControlsOpen,
    ControlsSummary,
    LocalProfile,
    LocalProfileManage,
}

impl OptionId {
    /// Todas as linhas do diálogo (conferidas pelos testes do inventário).
    #[cfg(test)]
    pub const ALL: [OptionId; 29] = [
        OptionId::Theme,
        OptionId::Density,
        OptionId::Scale,
        OptionId::InterfaceProfile,
        OptionId::Quality,
        OptionId::Effects,
        OptionId::Coordinates,
        OptionId::Hover,
        OptionId::CapturedPieces,
        OptionId::MoveMethod,
        OptionId::CameraView,
        OptionId::AlwaysQueen,
        OptionId::AidProfile,
        OptionId::AidDestinations,
        OptionId::AidGhosts,
        OptionId::AidSelected,
        OptionId::AidLastMove,
        OptionId::AidCheck,
        OptionId::AidExplanation,
        OptionId::HudEnabled,
        OptionId::HudProfile,
        OptionId::EngineManager,
        OptionId::EngineWizard,
        OptionId::EngineElo,
        OptionId::EngineSide,
        OptionId::ControlsOpen,
        OptionId::ControlsSummary,
        OptionId::LocalProfile,
        OptionId::LocalProfileManage,
    ];

    #[cfg(test)]
    pub fn category(self) -> Category {
        use OptionId::*;
        match self {
            Theme | Density | Scale | InterfaceProfile | Quality | Effects => Category::Appearance,
            Coordinates | Hover | CapturedPieces | MoveMethod | CameraView | AlwaysQueen => {
                Category::Board
            }
            AidProfile | AidDestinations | AidGhosts | AidSelected | AidLastMove | AidCheck
            | AidExplanation | HudEnabled | HudProfile => Category::Aids,
            EngineManager | EngineWizard | EngineElo | EngineSide => Category::Engines,
            ControlsOpen | ControlsSummary => Category::Controls,
            LocalProfile | LocalProfileManage => Category::Profile,
        }
    }

    /// Chaves i18n do rótulo e da explicação curta.
    pub fn texts(self) -> (&'static str, &'static str) {
        use OptionId::*;
        match self {
            Theme => ("opt-theme", "opt-theme-hint"),
            Density => ("opt-density", "opt-density-hint"),
            Scale => ("opt-scale", "opt-scale-hint"),
            InterfaceProfile => ("opt-interface-profile", "opt-interface-profile-hint"),
            Quality => ("opt-quality", "opt-quality-hint"),
            Effects => ("opt-effects", "opt-effects-hint"),
            Coordinates => ("opt-coordinates", "opt-coordinates-hint"),
            Hover => ("opt-hover", "opt-hover-hint"),
            CapturedPieces => ("opt-captured-pieces", "opt-captured-pieces-hint"),
            MoveMethod => ("opt-move-method", "opt-move-method-hint"),
            CameraView => ("opt-default-view", "opt-default-view-hint"),
            AlwaysQueen => ("opt-always-queen", "opt-always-queen-hint"),
            AidProfile => ("opt-aid-profile", "opt-aid-profile-hint"),
            AidDestinations => ("opt-aid-destinations", "opt-aid-destinations-hint"),
            AidGhosts => ("opt-aid-ghosts", "opt-aid-ghosts-hint"),
            AidSelected => ("opt-aid-selected", "opt-aid-selected-hint"),
            AidLastMove => ("opt-aid-last-move", "opt-aid-last-move-hint"),
            AidCheck => ("opt-aid-check", "opt-aid-check-hint"),
            AidExplanation => ("opt-aid-explanation", "opt-aid-explanation-hint"),
            HudEnabled => ("opt-hud-enabled", "opt-hud-enabled-hint"),
            HudProfile => ("opt-hud-profile", "opt-hud-profile-hint"),
            EngineManager => ("opt-engine-manager", "opt-engine-manager-hint"),
            EngineWizard => ("opt-engine-wizard", "opt-engine-wizard-hint"),
            EngineElo => ("opt-engine-elo", "opt-engine-elo-hint"),
            EngineSide => ("opt-engine-side", "opt-engine-side-hint"),
            ControlsOpen => ("opt-controls", "opt-controls-hint"),
            ControlsSummary => ("opt-controls-summary", "opt-controls-summary-hint"),
            LocalProfile => ("opt-profile-active", "opt-profile-active-hint"),
            LocalProfileManage => ("opt-profile-manage", "opt-profile-manage-hint"),
        }
    }
}

/// Uma mudança feita pelo diálogo. Cada uma vai para o recurso que o controle antigo usava.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionChange {
    Theme(ThemeMode),
    Density(Density),
    Scale(u16),
    Quality(GraphicsQuality),
    Effects(bool),
    Coordinates(bool),
    Hover(bool),
    CapturedStyle(crate::captured::CapturedStyle),
    MoveMethod(MoveMethod),
    CameraView(CameraView),
    AlwaysQueen(bool),
    Aids(AidSettings),
    Hud(HudSettings),
    EngineElo(u32),
    EngineSide(Side),
}

/// Os recursos que as Opções editam: os mesmos do painel esquerdo e das Ações.
#[derive(SystemParam)]
pub struct OptionsEnv<'w> {
    appearance: ResMut<'w, Appearance>,
    quality: ResMut<'w, GraphicsQuality>,
    effects: ResMut<'w, UltraEffects>,
    support: Res<'w, GpuSupport>,
    coordinates: ResMut<'w, BoardCoordinates>,
    hover: ResMut<'w, HoverHighlight>,
    captured: ResMut<'w, crate::captured::CapturedStyle>,
    aids: ResMut<'w, AidSettings>,
    hud: ResMut<'w, HudSettings>,
    session: ResMut<'w, Session>,
    engine_ui: ResMut<'w, EngineUi>,
    rig: ResMut<'w, CameraRig>,
    dock: ResMut<'w, Dock>,
    extra: OptionsScreens<'w>,
}

/// Telas e perfis que o diálogo abre ou embute (não são preferências próprias).
#[derive(SystemParam)]
pub struct OptionsScreens<'w> {
    controls_ui: ResMut<'w, ControlsUi>,
    controls: Res<'w, Controls>,
    local: ResMut<'w, LocalProfiles>,
    profiles: ResMut<'w, InterfaceProfiles>,
    training: Res<'w, crate::training_ui::TrainingUi>,
}

/// Escreve só quando o valor mudou: `ResMut` tocado sem necessidade acionaria `is_changed` em
/// quem reaplica qualidade, persistência etc.
macro_rules! set {
    ($slot:expr, $value:expr) => {{
        let value = $value;
        // A comparação lê pelo `Deref`; só a atribuição passa pelo `DerefMut`.
        if $slot != value {
            $slot = value;
        }
    }};
}

impl OptionsEnv<'_> {
    /// Aplica uma mudança ao recurso de origem.
    pub fn apply(&mut self, change: OptionChange) {
        match change {
            OptionChange::Theme(v) => set!(self.appearance.theme, v),
            OptionChange::Density(v) => set!(self.appearance.density, v),
            OptionChange::Scale(p) => {
                set!(self.appearance.scale, UiScale::from_percent(p.into()));
            }
            OptionChange::Quality(v) => set!(*self.quality, v),
            OptionChange::Effects(v) => set!(self.effects.0, v),
            OptionChange::Coordinates(v) => set!(self.coordinates.0, v),
            OptionChange::Hover(v) => set!(self.hover.0, v),
            OptionChange::CapturedStyle(v) => set!(*self.captured, v),
            OptionChange::MoveMethod(v) => {
                if self.session.move_method() != v {
                    self.session.set_move_method(v);
                }
            }
            OptionChange::CameraView(v) => {
                if self.rig.view != Some(v) {
                    self.rig.pose = v.pose();
                    self.rig.view = Some(v);
                }
            }
            OptionChange::AlwaysQueen(v) => {
                if self.session.always_queen() != v {
                    self.session.set_always_queen(v);
                }
            }
            OptionChange::Aids(v) => set!(*self.aids, v),
            OptionChange::Hud(v) => set!(*self.hud, v),
            OptionChange::EngineElo(v) => {
                if self.engine_ui.elo() != v.clamp(MIN_ELO, MAX_ELO) {
                    self.engine_ui.set_elo(v);
                }
            }
            OptionChange::EngineSide(v) => {
                if self.engine_ui.user_side() != v {
                    self.engine_ui.set_user_side(v);
                }
            }
        }
    }

    /// Devolve toda Opção ao valor de fábrica. Não toca Controles (têm o seu "Restaurar"),
    /// Perfil local nem Perfis de interface.
    pub fn restore_defaults(&mut self) {
        for change in [
            OptionChange::Theme(ThemeMode::default()),
            OptionChange::Density(Density::default()),
            OptionChange::Scale(UiScale::default().percent()),
            OptionChange::Quality(GraphicsQuality::default()),
            OptionChange::Effects(UltraEffects::default().0),
            OptionChange::Coordinates(BoardCoordinates::default().0),
            OptionChange::Hover(HoverHighlight::default().0),
            OptionChange::CapturedStyle(crate::captured::CapturedStyle::default()),
            OptionChange::MoveMethod(MoveMethod::default()),
            OptionChange::CameraView(CameraView::Standard),
            OptionChange::AlwaysQueen(false),
            OptionChange::Aids(AidSettings::default()),
            OptionChange::Hud(HudSettings::default()),
            OptionChange::EngineElo(DEFAULT_ENGINE_ELO),
            OptionChange::EngineSide(Side::White),
        ] {
            self.apply(change);
        }
    }

    /// Mostra o painel Engines do dock (o gerenciador de Engines) e ativa a aba.
    fn show_engine_manager(&mut self) {
        let layout = &mut self.dock.layout;
        if !layout.is_visible("Engines") {
            layout.set_visible("Engines", true);
        }
        layout.activate("Engines");
    }
}

/// Elo de fábrica da Engine adversária (o mesmo de `EngineUi::default`).
const DEFAULT_ENGINE_ELO: u32 = 1500;

/// Estado do diálogo.
#[derive(Resource, Debug, Default)]
pub struct OptionsUi {
    pub open: bool,
    pub category: Category,
    /// "Restaurar padrões" pediu confirmação e ela ainda não veio.
    confirming_reset: bool,
}

impl OptionsUi {
    pub fn is_confirming_reset(&self) -> bool {
        self.confirming_reset
    }

    /// Primeiro clique em "Restaurar padrões": só pede a confirmação.
    pub fn ask_reset(&mut self) {
        self.confirming_reset = true;
    }

    pub fn cancel_reset(&mut self) {
        self.confirming_reset = false;
    }

    /// Confirmação: restaura. Sem o pedido antes, não faz nada.
    pub fn confirm_reset(&mut self, env: &mut OptionsEnv) -> bool {
        if !std::mem::take(&mut self.confirming_reset) {
            return false;
        }
        env.restore_defaults();
        true
    }

    fn close(&mut self) {
        self.open = false;
        self.confirming_reset = false;
    }
}

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        // `--options`: abre o diálogo já na partida (evidências com `--screenshot`).
        let open = std::env::args().any(|a| a == "--options");
        app.insert_resource(OptionsUi {
            open,
            ..OptionsUi::default()
        })
        .add_systems(
            EguiPrimaryContextPass,
            options_dialog.after(crate::ui_theme::apply_appearance),
        );
    }
}

fn view_key(view: CameraView) -> &'static str {
    match view {
        CameraView::Top => "view-top",
        CameraView::Oblique => "view-oblique",
        CameraView::Standard => "view-standard",
        CameraView::Inverted => "view-inverted",
    }
}

fn move_method_key(method: MoveMethod) -> &'static str {
    match method {
        MoveMethod::TwoTaps => "move-method-two-taps",
        MoveMethod::Drag => "move-method-drag",
        MoveMethod::Both => "move-method-both",
    }
}

const HEADER_HEIGHT: f32 = 60.0;
const FOOTER_HEIGHT: f32 = 56.0;

fn options_dialog(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<OptionsUi>,
    mut env: OptionsEnv,
) -> Result {
    if !state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let p = Palette::from_visuals(&ctx.global_style().visuals);
    let screen = ctx.content_rect();
    let width = (screen.width() - 32.0).clamp(320.0, 760.0);
    let height = (screen.height() - 64.0).clamp(240.0, 560.0);
    let nav_width = if width < 560.0 { 120.0 } else { 200.0 };
    let body_height = height - HEADER_HEIGHT - FOOTER_HEIGHT;
    let frame = egui::Frame::new()
        .fill(p.surface_1)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(8)
        .inner_margin(0);
    let response = egui::Modal::new(egui::Id::new("options-dialog"))
        .backdrop_color(p.scrim)
        .frame(frame)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.set_width(width);
            // Cabeçalho: ícone de engrenagem e título (estilo `title`, 20/28).
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(24, 16))
                .show(ui, |ui| {
                    ui.set_width(width - 48.0);
                    ui.set_height(HEADER_HEIGHT - 32.0);
                    let font = crate::ui_theme::strong_font(ui.ctx(), 20.0);
                    let title = icon_job(
                        ui.ctx(),
                        Some(egui_phosphor::regular::GEAR),
                        &i18n.t("options-title"),
                        font,
                        p.ink,
                    );
                    ui.label(title);
                });
            let (line, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
            ui.painter().rect_filled(line, 0.0, p.border);
            ui.horizontal_top(|ui| {
                // Navegação por categorias: coluna `surface-2` com borda à direita.
                egui::Frame::new()
                    .fill(p.surface_2)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(nav_width - 16.0);
                        ui.set_height(body_height - 16.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for category in Category::ALL {
                                let selected = state.category == category;
                                if nav_item(
                                    ui,
                                    p,
                                    category.icon(),
                                    &i18n.t(category.key()),
                                    selected,
                                )
                                .clicked()
                                {
                                    state.category = category;
                                }
                            }
                        });
                    });
                let (line, _) =
                    ui.allocate_exact_size(egui::vec2(1.0, body_height), egui::Sense::hover());
                ui.painter().rect_filled(line, 0.0, p.border);
                // Conteúdo: padding de 24 e espaço à direita para a barra de rolagem.
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 8,
                        top: 8,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        let content_width = width - nav_width - 1.0 - 32.0;
                        ui.set_width(content_width);
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .max_height(body_height - 16.0)
                                .min_scrolled_height(body_height - 16.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    // Margem direita: nada encosta na barra de rolagem.
                                    egui::Frame::new()
                                        .inner_margin(egui::Margin {
                                            right: 16,
                                            ..egui::Margin::ZERO
                                        })
                                        .show(ui, |ui| {
                                            let category = state.category;
                                            category_ui(ui, &i18n, category, &mut env, &mut state);
                                        });
                                });
                        });
                    });
            });
            // Rodapé: barra `surface-2` com borda no topo e os botões à direita.
            egui::Frame::new()
                .fill(p.surface_2)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 8,
                    se: 8,
                })
                .inner_margin(egui::Margin::symmetric(24, 12))
                .show(ui, |ui| {
                    ui.set_width(width - 48.0);
                    ui.set_height(FOOTER_HEIGHT - 24.0);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    footer(ui, &i18n, &mut state, &mut env);
                });
        });
    if response.should_close() {
        state.close();
    }
    Ok(())
}

/// Item de menu da navegação: 32 px, ícone e rótulo; hover em `surface-3`, selecionado em
/// `accent-subtle` com texto `accent-ink`.
fn nav_item(
    ui: &mut egui::Ui,
    p: &Palette,
    icon: &str,
    label: &str,
    selected: bool,
) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 32.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let (fill, ink) = if selected {
        (p.accent_subtle, p.accent_ink)
    } else if response.hovered() {
        (p.surface_3, p.ink)
    } else {
        (egui::Color32::TRANSPARENT, p.ink)
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let job = icon_job(ui.ctx(), Some(icon), label, font, ink);
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, ink);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    response
}

fn footer(ui: &mut egui::Ui, i18n: &I18n, state: &mut OptionsUi, env: &mut OptionsEnv) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if state.is_confirming_reset() {
            // Da direita para a esquerda: Restaurar (perigo), Cancelar e a pergunta.
            if cs_button(ui, &i18n.t("options-restore-yes"), ButtonVariant::Danger).clicked() {
                state.confirm_reset(env);
            }
            if cs_button(
                ui,
                &i18n.t("options-restore-cancel"),
                ButtonVariant::Secondary,
            )
            .clicked()
            {
                state.cancel_reset();
            }
            ui.add(egui::Label::new(i18n.t("options-restore-confirm")).truncate());
        } else {
            if cs_button(ui, &i18n.t("options-close"), ButtonVariant::Primary).clicked() {
                state.close();
            }
            if cs_button(ui, &i18n.t("options-restore"), ButtonVariant::Secondary).clicked() {
                state.ask_reset();
            }
        }
    });
}

/// Linha: rótulo e explicação à esquerda, controle à direita.
fn row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    id: OptionId,
    label_override: Option<String>,
    control: impl FnOnce(&mut egui::Ui),
) {
    let (label_key, hint_key) = id.texts();
    let label = label_override.unwrap_or_else(|| i18n.t(label_key));
    let muted = palette(ui).ink_muted;
    egui::Sides::new().spacing(16.0).shrink_left().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.label(strong(ui.ctx(), label));
                ui.label(egui::RichText::new(i18n.t(hint_key)).small().color(muted));
            });
        },
        control,
    );
    ui.add_space(6.0);
    ui.separator();
}

/// Linha de largura total, para conteúdo que não cabe à direita (listas, seções embutidas).
fn block_row(ui: &mut egui::Ui, i18n: &I18n, id: OptionId, content: impl FnOnce(&mut egui::Ui)) {
    let (label_key, hint_key) = id.texts();
    let muted = palette(ui).ink_muted;
    ui.label(strong(ui.ctx(), i18n.t(label_key)));
    ui.label(egui::RichText::new(i18n.t(hint_key)).small().color(muted));
    ui.add_space(4.0);
    content(ui);
    ui.add_space(6.0);
    ui.separator();
}

fn switch_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    id: OptionId,
    value: bool,
    enabled: bool,
) -> Option<bool> {
    let mut on = value;
    row(ui, i18n, id, None, |ui| {
        ui.add_enabled_ui(enabled, |ui| cs_switch(ui, &mut on, ""));
    });
    (on != value).then_some(on)
}

fn dropdown_row<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    i18n: &I18n,
    id: OptionId,
    salt: &'static str,
    value: &T,
    options: &[(T, String)],
) -> Option<T> {
    let mut chosen = value.clone();
    row(ui, i18n, id, None, |ui| {
        cs_dropdown(ui, salt, &mut chosen, options);
    });
    (chosen != *value).then_some(chosen)
}

fn category_ui(
    ui: &mut egui::Ui,
    i18n: &I18n,
    category: Category,
    env: &mut OptionsEnv,
    state: &mut OptionsUi,
) {
    match category {
        Category::Appearance => appearance(ui, i18n, env),
        Category::Board => board(ui, i18n, env),
        Category::Aids => aids(ui, i18n, env),
        Category::Engines => engines(ui, i18n, env, state),
        Category::Controls => controls(ui, i18n, env, state),
        Category::Profile => profile(ui, i18n, env, state),
    }
}

fn appearance(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv) {
    let mode = env.appearance.theme;
    let effective = crate::ui_theme::resolve_theme(mode, Some(ui.visuals().dark_mode));
    let mut picked = None;
    block_row(ui, i18n, OptionId::Theme, |ui| {
        picked = theme_gallery::theme_gallery(ui, i18n, mode, effective);
    });
    if let Some(v) = picked {
        env.apply(OptionChange::Theme(v));
    }

    let mut density = env.appearance.density;
    row(ui, i18n, OptionId::Density, None, |ui| {
        for d in Density::ALL {
            cs_radio_value(ui, &mut density, d, &i18n.t(d.key()));
        }
    });
    env.apply(OptionChange::Density(density));

    let percent = env.appearance.scale.percent();
    let mut slider = f32::from(percent);
    let label = i18n.t_with("opt-scale", &[("percent", &percent.to_string())]);
    row(ui, i18n, OptionId::Scale, Some(label), |ui| {
        cs_slider(
            ui,
            &mut slider,
            f32::from(UiScale::MIN_PERCENT)..=f32::from(UiScale::MAX_PERCENT),
            5.0,
        );
    });
    env.apply(OptionChange::Scale(slider.round() as u16));

    block_row(ui, i18n, OptionId::InterfaceProfile, |ui| {
        crate::profiles::profiles_section(ui, i18n, &mut env.extra.profiles);
    });

    let qualities: Vec<_> = GraphicsQuality::ALL
        .iter()
        .map(|q| (*q, i18n.t(q.label_key())))
        .collect();
    let quality = *env.quality;
    if let Some(v) = dropdown_row(
        ui,
        i18n,
        OptionId::Quality,
        "opt-quality",
        &quality,
        &qualities,
    ) {
        env.apply(OptionChange::Quality(v));
    }

    let available = quality.bloom_for(true, *env.support);
    let effects = env.effects.0;
    if let Some(v) = switch_row(ui, i18n, OptionId::Effects, effects, available) {
        env.apply(OptionChange::Effects(v));
    }
    if !available {
        ui.small(i18n.t("opt-effects-unavailable"));
    }
}

fn board(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv) {
    let coordinates = env.coordinates.0;
    if let Some(v) = switch_row(ui, i18n, OptionId::Coordinates, coordinates, true) {
        env.apply(OptionChange::Coordinates(v));
    }
    let hover = env.hover.0;
    if let Some(v) = switch_row(ui, i18n, OptionId::Hover, hover, true) {
        env.apply(OptionChange::Hover(v));
    }
    let pieces = env.captured.is_pieces();
    if let Some(v) = switch_row(ui, i18n, OptionId::CapturedPieces, pieces, true) {
        env.apply(OptionChange::CapturedStyle(if v {
            crate::captured::CapturedStyle::Pieces
        } else {
            crate::captured::CapturedStyle::Text
        }));
    }
    let methods: Vec<_> = [MoveMethod::Both, MoveMethod::TwoTaps, MoveMethod::Drag]
        .into_iter()
        .map(|m| (m, i18n.t(move_method_key(m))))
        .collect();
    let current = env.session.move_method();
    if let Some(v) = dropdown_row(
        ui,
        i18n,
        OptionId::MoveMethod,
        "opt-move-method",
        &current,
        &methods,
    ) {
        env.apply(OptionChange::MoveMethod(v));
    }
    let views: Vec<_> = CameraView::ALL
        .into_iter()
        .map(|v| (v, i18n.t(view_key(v))))
        .collect();
    let view = env.rig.view.unwrap_or(CameraView::Standard);
    if let Some(v) = dropdown_row(ui, i18n, OptionId::CameraView, "opt-view", &view, &views) {
        env.apply(OptionChange::CameraView(v));
    }
    let competitive = env.session.is_competitive();
    let always = env.session.always_queen();
    if let Some(v) = switch_row(ui, i18n, OptionId::AlwaysQueen, always, !competitive) {
        env.apply(OptionChange::AlwaysQueen(v));
    }
}

fn aids(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv) {
    let allowed: Allowance = hud_policy::allowance(&env.session);
    if allowed.restricted() {
        ui.label(
            egui::RichText::new(i18n.t("aid-restricted"))
                .small()
                .color(palette(ui).warning),
        );
    }
    let aids = *env.aids;
    let current = aids.profile();
    let profiles = [
        AidProfile::PhysicalGame,
        AidProfile::Training,
        AidProfile::Custom,
    ]
    .map(|p| (p, i18n.t(p.label_key())));
    if let Some(profile) = dropdown_row(
        ui,
        i18n,
        OptionId::AidProfile,
        "opt-aid-profile",
        &current,
        &profiles,
    ) && let Some(preset) = AidSettings::for_profile(profile)
    {
        env.apply(OptionChange::Aids(preset));
    }
    let aids = *env.aids;
    let mut edited = aids;
    let rows = [
        (
            OptionId::AidDestinations,
            aids.destinations,
            allowed.destinations,
        ),
        (OptionId::AidGhosts, aids.ghosts, allowed.ghosts),
        (OptionId::AidSelected, aids.selected, allowed.selection),
        (OptionId::AidLastMove, aids.last_move, allowed.last_move),
        (OptionId::AidCheck, aids.check, allowed.check),
    ];
    for (id, value, enabled) in rows {
        if let Some(v) = switch_row(ui, i18n, id, value, enabled) {
            match id {
                OptionId::AidDestinations => edited.destinations = v,
                OptionId::AidGhosts => edited.ghosts = v,
                OptionId::AidSelected => edited.selected = v,
                OptionId::AidLastMove => edited.last_move = v,
                _ => edited.check = v,
            }
        }
    }
    let explanations: Vec<_> = ErrorExplanation::ALL
        .iter()
        .map(|e| (*e, i18n.t(e.label_key())))
        .collect();
    if let Some(v) = dropdown_row(
        ui,
        i18n,
        OptionId::AidExplanation,
        "opt-aid-explanation",
        &aids.explanation,
        &explanations,
    ) {
        edited.explanation = v;
    }
    env.apply(OptionChange::Aids(edited));

    let hud = *env.hud;
    let mut edited_hud = hud;
    if let Some(v) = switch_row(ui, i18n, OptionId::HudEnabled, hud.enabled, true) {
        edited_hud.enabled = v;
    }
    let hud_profiles = [
        HudProfile::Off,
        HudProfile::Study,
        HudProfile::Analysis,
        HudProfile::Custom,
    ]
    .map(|p| (p, i18n.t(p.label_key())));
    if let Some(profile) = dropdown_row(
        ui,
        i18n,
        OptionId::HudProfile,
        "opt-hud-profile",
        &hud.profile(),
        &hud_profiles,
    ) && let Some(preset) = HudSettings::for_profile(profile)
    {
        // O preset troca os elementos; aparência e chave geral ficam (como no painel Auxílios).
        edited_hud = HudSettings {
            enabled: edited_hud.enabled,
            opacity_pct: hud.opacity_pct,
            palette: hud.palette,
            ..preset
        };
    }
    env.apply(OptionChange::Hud(edited_hud));
}

fn engines(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv, state: &mut OptionsUi) {
    let mut open_manager = false;
    row(ui, i18n, OptionId::EngineManager, None, |ui| {
        let label = i18n.t("opt-engine-manager-open");
        open_manager = cs_button(ui, &label, ButtonVariant::Secondary).clicked();
    });
    if open_manager {
        env.show_engine_manager();
        // O diálogo é modal: fecha para o painel Engines poder ser usado.
        state.close();
    }
    let mut open_wizard = false;
    row(ui, i18n, OptionId::EngineWizard, None, |ui| {
        let label = i18n.t("opt-engine-wizard-open");
        open_wizard = cs_button(ui, &label, ButtonVariant::Secondary).clicked();
    });
    if open_wizard {
        env.engine_ui.request_wizard();
        state.close();
    }
    let elo = env.engine_ui.elo();
    let mut slider = elo as f32;
    let label = i18n.t_with("opt-engine-elo", &[("elo", &elo.to_string())]);
    row(ui, i18n, OptionId::EngineElo, Some(label), |ui| {
        cs_slider(ui, &mut slider, MIN_ELO as f32..=MAX_ELO as f32, 10.0);
    });
    env.apply(OptionChange::EngineElo(slider.round() as u32));
    let mut side = env.engine_ui.user_side();
    row(ui, i18n, OptionId::EngineSide, None, |ui| {
        cs_radio_value(ui, &mut side, Side::White, &i18n.t("engine-side-white"));
        cs_radio_value(ui, &mut side, Side::Black, &i18n.t("engine-side-black"));
    });
    env.apply(OptionChange::EngineSide(side));
}

fn controls(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv, state: &mut OptionsUi) {
    let mut open = false;
    row(ui, i18n, OptionId::ControlsOpen, None, |ui| {
        let label = i18n.t("opt-controls-open");
        open = cs_button(ui, &label, ButtonVariant::Secondary).clicked();
    });
    if open {
        env.extra.controls_ui.open = true;
        state.close();
    }
    let lines = crate::controls::help_lines(i18n, env.extra.controls.map());
    block_row(ui, i18n, OptionId::ControlsSummary, |ui| {
        for line in &lines {
            ui.small(line);
        }
    });
}

fn profile(ui: &mut egui::Ui, i18n: &I18n, env: &mut OptionsEnv, state: &mut OptionsUi) {
    let swapped = env.extra.training.session_swapped();
    let local = &mut env.extra.local;
    let Some(active) = local.active() else {
        ui.label(i18n.t("opt-profile-none"));
        return;
    };
    let options: Vec<_> = local
        .list()
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let mut chosen = active;
    row(ui, i18n, OptionId::LocalProfile, None, |ui| {
        ui.add_enabled_ui(!swapped, |ui| {
            cs_dropdown(ui, "opt-local-profile", &mut chosen, &options);
        });
    });
    if swapped {
        ui.small(i18n.t("opt-profile-busy"));
    }
    if chosen != active {
        local.request(Op::Switch(chosen));
    }
    let mut manage = false;
    row(ui, i18n, OptionId::LocalProfileManage, None, |ui| {
        let label = i18n.t("opt-profile-manage-open");
        manage = cs_button(ui, &label, ButtonVariant::Secondary).clicked();
    });
    if manage {
        local.manager_open = true;
        state.close();
    }
}

#[cfg(test)]
mod tests;
