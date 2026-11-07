//! Painéis encaixáveis (C1, UI-01/02): uma região de dock à direita, com abas e divisões
//! (`egui_tiles`), e o painel Notação como primeiro painel real.
//!
//! O centro da janela **não** é um painel do dock: é o que sobra depois dos painéis, sem
//! nenhum widget do egui. Assim o egui não reivindica o ponteiro ali e o picking e a órbita
//! do tabuleiro 3D seguem funcionando (ver `docs/decisoes/spike-docking.md`). O layout
//! (posições, tamanhos, visibilidade) é só Apresentação: mostrar/ocultar/mover um painel
//! nunca emite Comando de Partida (ADR-0001) e é gravado no SQLite como preferência.

use crate::capture::CaptureMode;
use crate::i18n::I18n;
use crate::notation;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui_tiles::{Behavior, SimplificationOptions, TileId, Tiles, Tree, UiResponse};
use serde::{Deserialize, Serialize};

/// Painéis que o dock sabe mostrar. Novos painéis (Relógio, Análise…) entram aqui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelKind {
    Notation,
}

impl PanelKind {
    pub const ALL: [PanelKind; 1] = [PanelKind::Notation];

    pub fn label_key(self) -> &'static str {
        match self {
            PanelKind::Notation => "dock-panel-notation",
        }
    }
}

/// Largura padrão e limites da região de dock, em pontos do egui.
const DEFAULT_WIDTH: f32 = 280.0;
const MIN_WIDTH: f32 = 180.0;
const MAX_WIDTH: f32 = 640.0;

/// Layout do dock: a árvore de abas/divisões (com os tamanhos relativos e a visibilidade
/// de cada painel) e a largura da região. É o que vai para o SQLite.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    pub width: f32,
    pub tree: Tree<PanelKind>,
}

impl std::fmt::Debug for DockLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockLayout")
            .field("width", &self.width)
            .field("tree", &self.tree)
            .finish()
    }
}

impl Default for DockLayout {
    /// Padrão: Notação à direita, numa aba.
    fn default() -> Self {
        let mut tiles = Tiles::default();
        let panes: Vec<TileId> = PanelKind::ALL
            .iter()
            .map(|kind| tiles.insert_pane(*kind))
            .collect();
        let root = tiles.insert_tab_tile(panes);
        Self {
            width: DEFAULT_WIDTH,
            tree: Tree::new("chess-studio-dock", root, tiles),
        }
    }
}

impl DockLayout {
    fn tile_of(&self, kind: PanelKind) -> Option<TileId> {
        self.tree.tiles.find_pane(&kind)
    }

    /// O painel está visível? (`false` também se ele não existir na árvore.)
    pub fn is_visible(&self, kind: PanelKind) -> bool {
        self.tile_of(kind)
            .is_some_and(|id| self.tree.is_visible(id))
    }

    /// Mostra ou oculta um painel (UI-01). Nada além do layout muda.
    pub fn set_visible(&mut self, kind: PanelKind, visible: bool) {
        if let Some(id) = self.tile_of(kind) {
            self.tree.set_visible(id, visible);
        }
    }

    /// Algum painel visível? Sem nenhum, a região de dock some e o tabuleiro ocupa tudo.
    pub fn any_visible(&self) -> bool {
        PanelKind::ALL.iter().any(|k| self.is_visible(*k))
    }

    /// Todo painel conhecido existe na árvore e a largura é sensata.
    fn is_valid(&self) -> bool {
        self.width.is_finite()
            && (MIN_WIDTH..=MAX_WIDTH).contains(&self.width)
            && PanelKind::ALL.iter().all(|k| self.tile_of(*k).is_some())
    }

    /// Serialização para a preferência `dock_layout`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Lê o layout gravado; ilegível ou incompleto volta ao padrão (nunca falha).
    pub fn from_json(value: Option<serde_json::Value>) -> Self {
        value
            .and_then(|v| serde_json::from_value::<DockLayout>(v).ok())
            .filter(DockLayout::is_valid)
            .unwrap_or_default()
    }
}

/// Estado do dock no app.
#[derive(Resource, Debug)]
pub struct Dock {
    pub layout: DockLayout,
    /// Muda a cada "Restaurar layout padrão" para o egui esquecer a largura arrastada.
    generation: u32,
    /// Largura ocupada à direita no último quadro (as janelas ancoradas à direita a respeitam).
    inset: f32,
    /// Área livre para o tabuleiro no último quadro (pontos do egui).
    board_rect: Option<egui::Rect>,
}

impl Default for Dock {
    fn default() -> Self {
        Self::with_layout(DockLayout::default())
    }
}

impl Dock {
    pub fn with_layout(layout: DockLayout) -> Self {
        Self {
            layout,
            generation: 0,
            inset: 0.0,
            board_rect: None,
        }
    }

    /// "Restaurar layout padrão" (UI-02).
    pub fn reset(&mut self) {
        self.layout = DockLayout::default();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Deslocamento horizontal para âncoras à direita (negativo = para dentro da tela).
    pub fn right_offset(&self, margin: f32) -> f32 {
        -(self.inset + margin)
    }

    /// O ponteiro está sobre a área do tabuleiro (não sobre a região do dock)?
    /// Sem layout conhecido (primeiro quadro), vale a janela toda.
    pub fn pointer_in_board(&self, cursor: Vec2) -> bool {
        self.board_rect
            .is_none_or(|r| r.contains(egui::pos2(cursor.x, cursor.y)))
    }
}

struct DockBehavior<'a> {
    i18n: &'a I18n,
    session: &'a mut Session,
}

impl Behavior<PanelKind> for DockBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile: TileId, pane: &mut PanelKind) -> UiResponse {
        match pane {
            PanelKind::Notation => notation::ui(ui, self.i18n, self.session),
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &PanelKind) -> egui::WidgetText {
        self.i18n.t(pane.label_key()).into()
    }

    fn simplification_options(&self) -> SimplificationOptions {
        // Mantém a barra de abas mesmo com um painel só: é a alça para arrastar.
        SimplificationOptions {
            prune_single_child_tabs: false,
            ..SimplificationOptions::default()
        }
    }
}

/// Seção "Painéis" da janela principal: mostrar/ocultar e restaurar o layout.
pub fn panels_section(ui: &mut egui::Ui, i18n: &I18n, dock: &mut Dock) {
    ui.strong(i18n.t("dock-heading"));
    for kind in PanelKind::ALL {
        let mut visible = dock.layout.is_visible(kind);
        if ui
            .checkbox(&mut visible, i18n.t(kind.label_key()))
            .changed()
        {
            dock.layout.set_visible(kind, visible);
        }
    }
    if ui.button(i18n.t("dock-reset")).clicked() {
        dock.reset();
    }
}

pub struct DockPlugin;

impl Plugin for DockPlugin {
    fn build(&self, app: &mut App) {
        // `init_resource` preserva o layout carregado do banco pelo `PersistencePlugin`.
        app.init_resource::<Dock>()
            .add_systems(EguiPrimaryContextPass, dock_ui);
    }
}

/// Desenha a região de dock à direita da tela e devolve a largura usada.
///
/// Não usa `egui::Panel`: ele exige uma raiz `Ui` que o `bevy_egui` não entrega (só o
/// `Context`), e na camada de fundo o egui considera "livre" tudo fora da raiz, então o
/// painel não reivindicaria o ponteiro. Uma `Area` em `Order::Middle` reivindica só o
/// próprio retângulo: o centro fica livre para o picking e a órbita do tabuleiro.
/// A borda esquerda é a alça de redimensionamento.
fn draw_dock(
    ctx: &egui::Context,
    generation: u32,
    layout: &mut DockLayout,
    behavior: &mut DockBehavior<'_>,
) -> f32 {
    let screen = ctx.viewport_rect();
    let max_width = MAX_WIDTH.min(screen.width() * 0.6).max(MIN_WIDTH);
    let width = layout.width.clamp(MIN_WIDTH, max_width);
    let rect = egui::Rect::from_min_max(egui::pos2(screen.max.x - width, screen.min.y), screen.max);
    let id = egui::Id::new(("chess-studio-dock", generation));
    let mut new_width = width;
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(ui.visuals().panel_fill)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke);
            frame.show(ui, |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                ui.add_space(4.0);
                layout.tree.ui(behavior, ui);
            });
            let handle = egui::Rect::from_min_size(rect.min, egui::vec2(6.0, rect.height()));
            let response = ui
                .interact(handle, id.with("resize"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if response.dragged() {
                new_width = (width - response.drag_delta().x).clamp(MIN_WIDTH, max_width);
            }
        });
    layout.width = new_width;
    new_width
}

fn dock_ui(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
    mut dock: ResMut<Dock>,
    capture: Option<Res<CaptureMode>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    // As capturas antigas continuam limpas: o dock só aparece com `--panel`.
    let hidden = capture.as_ref().is_some_and(|c| !c.show_panel);
    if hidden || !dock.layout.any_visible() {
        dock.inset = 0.0;
        dock.board_rect = Some(screen);
        return Ok(());
    }
    let dock = &mut *dock;
    let mut behavior = DockBehavior {
        i18n: &i18n,
        session: &mut session,
    };
    let width = draw_dock(ctx, dock.generation, &mut dock.layout, &mut behavior);
    dock.inset = width;
    dock.board_rect = Some(egui::Rect::from_min_max(
        screen.min,
        egui::pos2(screen.max.x - width, screen.max.y),
    ));
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_shows_the_notation_panel() {
        let layout = DockLayout::default();
        assert!(layout.is_visible(PanelKind::Notation));
        assert!(layout.any_visible());
        assert!(layout.is_valid());
    }

    #[test]
    fn layout_survives_a_json_round_trip_with_size_and_visibility() {
        let mut layout = DockLayout {
            width: 333.0,
            ..DockLayout::default()
        };
        layout.set_visible(PanelKind::Notation, false);
        let back = DockLayout::from_json(Some(layout.to_json()));
        assert_eq!(back, layout);
        assert_eq!(back.width, 333.0);
        assert!(!back.is_visible(PanelKind::Notation));
        assert!(!back.any_visible());
    }

    #[test]
    fn unreadable_or_incomplete_layouts_fall_back_to_the_default() {
        let default = DockLayout::default();
        assert_eq!(DockLayout::from_json(None), default);
        assert_eq!(
            DockLayout::from_json(Some(serde_json::json!("lixo"))),
            default
        );
        let absurd = DockLayout {
            width: 99_999.0,
            ..DockLayout::default()
        };
        assert_eq!(DockLayout::from_json(Some(absurd.to_json())), default);
    }

    #[test]
    fn reset_restores_the_default_layout_and_forgets_the_dragged_width() {
        let mut dock = Dock::default();
        dock.layout.width = 500.0;
        dock.layout.set_visible(PanelKind::Notation, false);
        let generation = dock.generation;
        dock.reset();
        assert_eq!(dock.layout, DockLayout::default());
        assert_ne!(dock.generation, generation);
    }

    #[test]
    fn hiding_a_panel_does_not_touch_the_game() {
        // UI-01: o layout é independente da Partida (nenhum acesso à `Session`).
        let session = Session::new();
        let before = session.snapshot().clone();
        let events = session.game().events().len();
        let mut dock = Dock::default();
        dock.layout.set_visible(PanelKind::Notation, false);
        dock.layout.set_visible(PanelKind::Notation, true);
        assert_eq!(session.snapshot(), &before);
        assert_eq!(session.game().events().len(), events);
    }

    #[test]
    fn the_board_area_excludes_the_dock_region() {
        let mut dock = Dock::default();
        assert!(
            dock.pointer_in_board(Vec2::new(1200.0, 300.0)),
            "sem layout: janela toda"
        );
        dock.board_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(1000.0, 720.0),
        ));
        assert!(dock.pointer_in_board(Vec2::new(500.0, 300.0)));
        assert!(!dock.pointer_in_board(Vec2::new(1100.0, 300.0)));
        dock.inset = 280.0;
        assert_eq!(dock.right_offset(8.0), -288.0);
    }

    /// Spike: com o dock desenhado como no app (dentro de `run_ui`, como o bevy_egui), o egui
    /// só reivindica o ponteiro sobre o dock, não no centro do tabuleiro.
    #[test]
    fn the_centre_is_transparent_to_the_pointer_and_the_panel_is_not() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::default();
        let i18n = I18n::pt_br();
        let mut session = Session::new();
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let mut run = |pointer: egui::Pos2| {
            let mut over = false;
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                };
                // Como o bevy_egui: os sistemas rodam dentro de `run_ui` e o `EguiWantsInput`
                // é lido depois do quadro.
                let mut behavior = DockBehavior {
                    i18n: &i18n,
                    session: &mut session,
                };
                let mut output = ctx.run_ui(input, |_| {
                    draw_dock(&ctx, 0, &mut layout, &mut behavior);
                });
                output.textures_delta.clear();
                over = ctx.is_pointer_over_egui();
            }
            over
        };
        assert!(!run(egui::pos2(400.0, 300.0)), "centro livre para o 3D");
        assert!(
            run(egui::pos2(1200.0, 300.0)),
            "o painel reivindica o ponteiro"
        );
    }
}
