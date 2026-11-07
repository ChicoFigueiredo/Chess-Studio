//! Painéis encaixáveis (C1/C2, UI-01/02): uma região de dock à direita, com abas e divisões
//! (`egui_tiles`), um registro de painéis e o recorte das câmeras do tabuleiro à área livre.
//!
//! O centro da janela **não** é um painel do dock: é o que sobra depois dos painéis, sem
//! nenhum widget do egui. Assim o egui não reivindica o ponteiro ali e o picking e a órbita
//! do tabuleiro 3D seguem funcionando (ver `docs/decisoes/spike-docking.md`). O layout
//! (posições, tamanhos, visibilidade) é só Apresentação: mostrar/ocultar/mover um painel
//! nunca emite Comando de Partida (ADR-0001) e é gravado no SQLite como preferência.
//!
//! # Registrar um painel
//!
//! ```ignore
//! dock::register_panel(app, dock::PanelDef {
//!     id: "Engines",                      // estável: vai para o layout salvo
//!     label_key: "<chave i18n>",          // título (aba e lista)
//!     access: dock::PanelAccess::Always,  // ou OutsideCompetitive (dicas/análise)
//!     ui: |ui, i18n, world| { /* desenha; intenções vão à `Session` */ },
//! });
//! ```
//!
//! O painel aparece na lista "Painéis" e como aba extra no layout (do usuário ou padrão).

use crate::capture::CaptureMode;
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPrimaryContextPass, PrimaryEguiContext, egui};
use egui_tiles::{
    Behavior, Container, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
use serde::{Deserialize, Serialize};

/// Identificador estável de um painel. É o que vai para o SQLite no layout; o painel
/// Notação mantém o id `"Notation"` do C1, então layouts antigos continuam legíveis.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PanelId(pub String);

impl PanelId {
    pub fn new(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// Ids dos painéis nativos.
pub mod ids {
    pub const NOTATION: &str = "Notation";
    pub const CLOCK: &str = "Clock";
    pub const CAPTURED: &str = "Captured";
    pub const STUDY: &str = "Study";
    pub const AIDS: &str = "Aids";
    pub const REVIEW: &str = "Review";
    pub const ARBITRATION: &str = "Arbitration";
    pub const TRAINING: &str = "Training";
    pub const ANALYSIS: &str = "Analysis";
}

/// Quando o painel pode ser aberto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAccess {
    Always,
    /// Dicas, análise e anotações: inacessíveis numa Partida competitiva (UI-08).
    OutsideCompetitive,
}

/// Desenha o conteúdo de um painel. Recebe o `World` sem o `Dock`, o registro e o `I18n`
/// (este vem à parte): painéis leem e escrevem recursos com `world.resource_scope`. Só a
/// `Session` recebe intenções (ADR-0001); o painel nunca muta a Partida diretamente.
pub type PanelUi = fn(&mut egui::Ui, &I18n, &mut World);

/// Definição de um painel registrável no dock.
#[derive(Clone, Copy)]
pub struct PanelDef {
    /// Id estável (ver [`PanelId`]); único.
    pub id: &'static str,
    /// Chave i18n do título (aba e lista "Painéis").
    pub label_key: &'static str,
    pub access: PanelAccess,
    pub ui: PanelUi,
}

impl PanelDef {
    /// O painel pode ser aberto agora, dada a Partida?
    pub fn accessible(&self, session: &Session) -> bool {
        self.access == PanelAccess::Always || !session.competitive_in_progress()
    }
}

/// Painéis conhecidos, na ordem de registro.
#[derive(Resource, Default, Clone)]
pub struct PanelRegistry {
    defs: Vec<PanelDef>,
}

impl PanelRegistry {
    pub fn get(&self, id: &str) -> Option<&PanelDef> {
        self.defs.iter().find(|d| d.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &PanelDef> {
        self.defs.iter()
    }

    /// Registra (ou substitui, se o id já existe) um painel. A lista segue a ordem do
    /// layout padrão; painéis de fora dela vêm depois, na ordem de registro.
    pub fn register(&mut self, def: PanelDef) {
        match self.defs.iter_mut().find(|d| d.id == def.id) {
            Some(existing) => *existing = def,
            None => self.defs.push(def),
        }
        self.defs.sort_by_key(|d| default_rank(d.id));
    }
}

/// API de registro: um plugin chama isto em `build` e o painel aparece no dock (lista
/// "Painéis" e abas). Painel novo entra no layout salvo como uma aba extra no primeiro
/// quadro; o layout do usuário não é reiniciado.
pub fn register_panel(app: &mut App, def: PanelDef) {
    app.world_mut()
        .get_resource_or_insert_with(PanelRegistry::default)
        .register(def);
}

/// Largura padrão e limites da região de dock, em pontos do egui.
const DEFAULT_WIDTH: f32 = 340.0;
const MIN_WIDTH: f32 = 180.0;
const MAX_WIDTH: f32 = 640.0;

/// Grupos de abas do layout padrão, de cima para baixo.
const DEFAULT_GROUPS: [&[&str]; 3] = [
    &[ids::CLOCK, ids::CAPTURED],
    &[ids::NOTATION, ids::STUDY],
    &[
        ids::AIDS,
        ids::REVIEW,
        ids::ANALYSIS,
        ids::ARBITRATION,
        ids::TRAINING,
    ],
];

fn default_rank(id: &str) -> usize {
    DEFAULT_GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .position(|d| *d == id)
        .unwrap_or(usize::MAX)
}

/// Layout do dock: a árvore de abas/divisões (com os tamanhos relativos e a visibilidade
/// de cada painel) e a largura da região. É o que vai para o SQLite.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    pub width: f32,
    pub tree: Tree<PanelId>,
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
    /// Padrão: Relógio e Peças capturadas no alto, Notação e Anotações no meio, Auxílios,
    /// Revisão, Arbitragem e Treino embaixo; cada grupo é uma barra de abas.
    fn default() -> Self {
        let mut tiles = Tiles::default();
        let groups: Vec<TileId> = DEFAULT_GROUPS
            .iter()
            .map(|group| {
                let panes = group
                    .iter()
                    .map(|id| tiles.insert_pane(PanelId::new(id)))
                    .collect();
                tiles.insert_tab_tile(panes)
            })
            .collect();
        let root = tiles.insert_vertical_tile(groups);
        Self {
            width: DEFAULT_WIDTH,
            tree: Tree::new("chess-studio-dock", root, tiles),
        }
    }
}

impl DockLayout {
    fn tile_of(&self, id: &str) -> Option<TileId> {
        self.tree.tiles.find_pane(&PanelId::new(id))
    }

    /// O painel está visível? (`false` também se ele não existir na árvore.)
    pub fn is_visible(&self, id: &str) -> bool {
        self.tile_of(id).is_some_and(|t| self.tree.is_visible(t))
    }

    /// Mostra ou oculta um painel (UI-01). Nada além do layout muda.
    pub fn set_visible(&mut self, id: &str, visible: bool) {
        if let Some(tile) = self.tile_of(id) {
            self.tree.set_visible(tile, visible);
        }
    }

    /// Traz a aba do painel para a frente no grupo em que está.
    pub fn activate(&mut self, id: &str) {
        let Some(tile) = self.tile_of(id) else { return };
        let Some(parent) = self.tree.tiles.parent_of(tile) else {
            return;
        };
        if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(parent) {
            tabs.set_active(tile);
        }
    }

    /// Algum painel visível? Sem nenhum, a região de dock some e o tabuleiro ocupa tudo.
    pub fn any_visible(&self) -> bool {
        self.tree
            .tiles
            .iter()
            .any(|(tile, t)| matches!(t, Tile::Pane(_)) && self.tree.is_visible(*tile))
    }

    /// Ids de todos os painéis presentes na árvore.
    pub fn pane_ids(&self) -> Vec<String> {
        self.tree
            .tiles
            .tiles()
            .filter_map(|t| match t {
                Tile::Pane(p) => Some(p.0.clone()),
                _ => None,
            })
            .collect()
    }

    /// Alinha a árvore ao registro: painéis registrados que faltam entram como abas extras
    /// na raiz; painéis que ninguém mais registra saem. Devolve se algo mudou.
    pub fn reconcile(&mut self, registry: &PanelRegistry) -> bool {
        let present = self.pane_ids();
        let unknown: Vec<TileId> = present
            .iter()
            .filter(|id| registry.get(id).is_none())
            .filter_map(|id| self.tile_of(id))
            .collect();
        let missing: Vec<&str> = registry
            .iter()
            .map(|d| d.id)
            .filter(|id| !present.iter().any(|p| p == id))
            .collect();
        if unknown.is_empty() && missing.is_empty() {
            return false;
        }
        for tile in unknown {
            for t in self.tree.tiles.tiles_mut() {
                if let Tile::Container(c) = t {
                    c.retain(|child| child != tile);
                }
            }
            self.tree.tiles.remove(tile);
        }
        if !missing.is_empty() {
            let panes: Vec<TileId> = missing
                .iter()
                .map(|id| self.tree.tiles.insert_pane(PanelId::new(id)))
                .collect();
            let extra = self.tree.tiles.insert_tab_tile(panes);
            match self.tree.root() {
                Some(root) => {
                    if let Some(Tile::Container(c)) = self.tree.tiles.get_mut(root) {
                        c.add_child(extra);
                    }
                }
                None => self.tree.root = Some(extra),
            }
        }
        true
    }

    /// A largura é sensata e a árvore tem raiz.
    fn is_valid(&self) -> bool {
        self.width.is_finite()
            && (MIN_WIDTH..=MAX_WIDTH).contains(&self.width)
            && self.tree.root().is_some()
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
    /// Pixels físicos por ponto do egui no último quadro.
    pixels_per_point: f32,
    /// `--tabs` das capturas já aplicado.
    tabs_applied: bool,
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
            pixels_per_point: 1.0,
            tabs_applied: false,
        }
    }

    /// "Restaurar layout padrão" (UI-02).
    pub fn reset(&mut self) {
        self.layout = DockLayout::default();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Deslocamento horizontal para âncoras à direita (negativo = para dentro da tela).
    /// Para janelas flutuantes que não devem ficar sob o dock.
    #[allow(dead_code)]
    pub fn right_offset(&self, margin: f32) -> f32 {
        -(self.inset + margin)
    }

    /// O ponteiro está sobre a área do tabuleiro (não sobre a região do dock)?
    /// Sem layout conhecido (primeiro quadro), vale a janela toda.
    pub fn pointer_in_board(&self, cursor: Vec2) -> bool {
        self.board_rect
            .is_none_or(|r| r.contains(egui::pos2(cursor.x, cursor.y)))
    }

    /// Tamanho da área do tabuleiro em pontos; sem layout conhecido, a janela toda.
    pub fn board_size(&self, window: Vec2) -> Vec2 {
        self.board_rect
            .map_or(window, |r| Vec2::new(r.width(), r.height()))
    }

    /// Retângulo (posição, tamanho) em pixels físicos onde as câmeras do tabuleiro
    /// renderizam; `None` = a janela inteira (sem dock aberto).
    pub fn board_viewport(&self, target: UVec2) -> Option<(UVec2, UVec2)> {
        board_viewport(self.board_rect?, self.pixels_per_point, target)
    }
}

/// Converte a área livre do tabuleiro (pontos do egui, canto superior esquerdo como origem)
/// em viewport físico `(posição, tamanho)` para `Camera::viewport`. `None` quando a área
/// cobre o alvo todo (não precisa de recorte) ou ainda não é utilizável.
pub fn board_viewport(
    board: egui::Rect,
    pixels_per_point: f32,
    target: UVec2,
) -> Option<(UVec2, UVec2)> {
    if target.x == 0 || target.y == 0 || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return None;
    }
    let to_px = |v: f32, max: u32| -> u32 {
        let px = (v * pixels_per_point).round().max(0.0);
        if px >= max as f32 { max } else { px as u32 }
    };
    let pos = UVec2::new(to_px(board.min.x, target.x), to_px(board.min.y, target.y));
    let end = UVec2::new(to_px(board.max.x, target.x), to_px(board.max.y, target.y));
    // O viewport nunca ultrapassa o alvo (o wgpu rejeita) nem tem tamanho zero.
    let size = end
        .saturating_sub(pos)
        .max(UVec2::ONE)
        .min(target.saturating_sub(pos).max(UVec2::ONE));
    (pos != UVec2::ZERO || size != target).then_some((pos, size))
}

struct DockBehavior<'a> {
    i18n: &'a I18n,
    registry: &'a PanelRegistry,
    world: &'a mut World,
}

impl Behavior<PanelId> for DockBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile: TileId, pane: &mut PanelId) -> UiResponse {
        let Some(def) = self.registry.get(&pane.0) else {
            return UiResponse::None;
        };
        let accessible = self
            .world
            .get_resource::<Session>()
            .is_none_or(|s| def.accessible(s));
        if accessible {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt(("dockpane", &pane.0))
                .show(ui, |ui| (def.ui)(ui, self.i18n, self.world));
        } else {
            ui.weak(self.i18n.t("dock-unavailable-competitive"));
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &PanelId) -> egui::WidgetText {
        self.registry
            .get(&pane.0)
            .map_or_else(|| pane.0.clone(), |d| self.i18n.t(d.label_key))
            .into()
    }

    fn simplification_options(&self) -> SimplificationOptions {
        // Mantém a barra de abas mesmo com um painel só: é a alça para arrastar.
        SimplificationOptions {
            prune_single_child_tabs: false,
            ..SimplificationOptions::default()
        }
    }
}

/// Seção "Painéis" da janela principal: mostrar/ocultar e restaurar o layout. Painéis
/// inacessíveis na Partida competitiva aparecem desabilitados.
pub fn panels_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    dock: &mut Dock,
    registry: &PanelRegistry,
    session: &Session,
) {
    ui.strong(i18n.t("dock-heading"));
    for def in registry.iter() {
        let mut visible = dock.layout.is_visible(def.id);
        let response = ui.add_enabled(
            def.accessible(session),
            egui::Checkbox::new(&mut visible, i18n.t(def.label_key)),
        );
        if response.changed() {
            dock.layout.set_visible(def.id, visible);
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
            .init_resource::<PanelRegistry>()
            .add_systems(EguiPrimaryContextPass, dock_ui)
            .add_systems(Update, apply_board_viewport);
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

/// Sistema exclusivo: os painéis recebem o `World` (ver [`PanelUi`]).
fn dock_ui(world: &mut World) {
    let Some(ctx) = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .iter_mut(world)
        .next()
        .map(|mut c| c.get_mut().clone())
    else {
        return;
    };
    let screen = ctx.viewport_rect();
    // As capturas antigas continuam limpas: o dock só aparece com `--panel` (ou no roteiro
    // de Treino, que vive num painel).
    let hidden = world
        .get_resource::<CaptureMode>()
        .is_some_and(|c| !c.show_panel && !crate::training_ui::demo_active())
        // Modo Foco: sem dock; o tabuleiro ocupa a janela toda.
        || world
            .get_resource::<crate::focus::ViewState>()
            .is_some_and(|v| v.focus);
    world.resource_scope(|world, mut dock: Mut<Dock>| {
        if !dock.tabs_applied {
            dock.tabs_applied = true;
            // Evidência (`--screenshot ... --tabs captured,study`): traz essas abas à frente.
            if world.contains_resource::<CaptureMode>()
                && let Some(list) = crate::capture::option_value("--tabs")
            {
                for id in list.split(',') {
                    let wanted = world
                        .resource::<PanelRegistry>()
                        .iter()
                        .map(|d| d.id)
                        .find(|d| d.eq_ignore_ascii_case(id));
                    if let Some(id) = wanted {
                        dock.layout.activate(id);
                    }
                }
            }
        }
        world.resource_scope(|world, registry: Mut<PanelRegistry>| {
            world.resource_scope(|world, i18n: Mut<I18n>| {
                let dock = &mut *dock;
                dock.pixels_per_point = ctx.pixels_per_point();
                dock.layout.reconcile(&registry);
                if hidden || !dock.layout.any_visible() {
                    dock.inset = 0.0;
                    dock.board_rect = Some(screen);
                    return;
                }
                let mut behavior = DockBehavior {
                    i18n: &i18n,
                    registry: &registry,
                    world,
                };
                let width = draw_dock(&ctx, dock.generation, &mut dock.layout, &mut behavior);
                dock.inset = width;
                dock.board_rect = Some(egui::Rect::from_min_max(
                    screen.min,
                    egui::pos2(screen.max.x - width, screen.max.y),
                ));
            });
        });
    });
}

/// Recorta o viewport das câmeras do tabuleiro (3D e 2D técnico) à área livre à esquerda
/// do dock. A câmera do egui continua com a janela inteira. Sem `Msaa` próprio aqui.
#[allow(clippy::type_complexity)]
fn apply_board_viewport(
    dock: Res<Dock>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<
        &mut Camera,
        Or<(
            With<crate::camera::MainCamera>,
            With<crate::presentation::TechnicalCamera>,
        )>,
    >,
) {
    let wanted = dock.board_viewport(window.physical_size());
    for mut camera in &mut cameras {
        let current = camera
            .viewport
            .as_ref()
            .map(|v| (v.physical_position, v.physical_size));
        if current != wanted {
            camera.viewport = wanted.map(|(position, size)| bevy::camera::Viewport {
                physical_position: position,
                physical_size: size,
                ..default()
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn noop(_: &mut egui::Ui, _: &I18n, _: &mut World) {}

    fn def(id: &'static str) -> PanelDef {
        PanelDef {
            id,
            label_key: "dock-panel-notation",
            access: PanelAccess::Always,
            ui: noop,
        }
    }

    fn builtin_registry() -> PanelRegistry {
        let mut registry = PanelRegistry::default();
        for group in DEFAULT_GROUPS {
            for id in group {
                registry.register(def(id));
            }
        }
        registry
    }

    #[test]
    fn default_layout_shows_every_builtin_panel() {
        let layout = DockLayout::default();
        for group in DEFAULT_GROUPS {
            for id in group {
                assert!(layout.is_visible(id), "{id}");
            }
        }
        assert!(layout.any_visible());
        assert!(layout.is_valid());
        assert!(!layout.clone().reconcile(&builtin_registry()));
    }

    #[test]
    fn layout_survives_a_json_round_trip_with_size_and_visibility() {
        let mut layout = DockLayout {
            width: 333.0,
            ..DockLayout::default()
        };
        layout.set_visible(ids::NOTATION, false);
        layout.set_visible(ids::CLOCK, false);
        let back = DockLayout::from_json(Some(layout.to_json()));
        assert_eq!(back, layout);
        assert_eq!(back.width, 333.0);
        assert!(!back.is_visible(ids::NOTATION));
        assert!(!back.is_visible(ids::CLOCK));
        assert!(back.is_visible(ids::STUDY));
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
    fn a_layout_saved_by_c1_still_loads_and_gains_the_new_panels() {
        // C1 gravava `Tree<PanelKind>` com a única aba "Notation".
        let mut tiles = Tiles::default();
        let pane = tiles.insert_pane(PanelId::new("Notation"));
        let root = tiles.insert_tab_tile(vec![pane]);
        let old = DockLayout {
            width: 300.0,
            tree: Tree::new("chess-studio-dock", root, tiles),
        };
        let mut loaded = DockLayout::from_json(Some(old.to_json()));
        assert_eq!(loaded.width, 300.0, "o layout antigo é aproveitado");
        assert!(loaded.reconcile(&builtin_registry()));
        for group in DEFAULT_GROUPS {
            for id in group {
                assert!(loaded.is_visible(id), "{id}");
            }
        }
        assert!(!loaded.reconcile(&builtin_registry()), "idempotente");
    }

    #[test]
    fn a_newly_registered_panel_joins_the_layout_and_an_unregistered_one_leaves() {
        let mut registry = builtin_registry();
        let mut layout = DockLayout::default();
        registry.register(def("Engines"));
        assert!(!layout.is_visible("Engines"));
        assert!(layout.reconcile(&registry));
        assert!(layout.is_visible("Engines"));
        assert!(layout.is_visible(ids::NOTATION), "o resto fica como estava");
        // Registrar de novo o mesmo id substitui (não duplica).
        registry.register(def("Engines"));
        assert_eq!(registry.iter().filter(|d| d.id == "Engines").count(), 1);
        let json = layout.to_json();
        let mut back = DockLayout::from_json(Some(json));
        assert!(back.is_visible("Engines"));
        let without = builtin_registry();
        assert!(back.reconcile(&without));
        assert!(!back.is_visible("Engines"));
        assert!(back.pane_ids().iter().all(|id| without.get(id).is_some()));
    }

    #[test]
    fn reset_restores_the_default_layout_and_forgets_the_dragged_width() {
        let mut dock = Dock::default();
        dock.layout.width = 500.0;
        dock.layout.set_visible(ids::NOTATION, false);
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
        for id in dock.layout.pane_ids() {
            dock.layout.set_visible(&id, false);
            assert!(!dock.layout.is_visible(&id));
            dock.layout.set_visible(&id, true);
        }
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
        assert_eq!(
            dock.board_size(Vec2::new(1280.0, 720.0)),
            Vec2::new(1000.0, 720.0)
        );
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    }

    #[test]
    fn the_viewport_follows_the_dock_width_and_the_scale_factor() {
        let window = UVec2::new(1280, 720);
        // Fator 1: o dock de 280 pt tira 280 px da direita.
        assert_eq!(
            board_viewport(rect(0.0, 0.0, 1000.0, 720.0), 1.0, window),
            Some((UVec2::ZERO, UVec2::new(1000, 720)))
        );
        // Fator 2 (HiDPI): a janela física tem o dobro; os pontos também dobram em pixels.
        let hidpi = UVec2::new(2560, 1440);
        assert_eq!(
            board_viewport(rect(0.0, 0.0, 1000.0, 720.0), 2.0, hidpi),
            Some((UVec2::ZERO, UVec2::new(2000, 1440)))
        );
        // Fator fracionário arredonda para o pixel mais próximo.
        assert_eq!(
            board_viewport(rect(0.0, 0.0, 1000.3, 720.0), 1.5, UVec2::new(1920, 1080)),
            Some((UVec2::ZERO, UVec2::new(1500, 1080)))
        );
    }

    #[test]
    fn the_viewport_is_absent_without_a_dock_and_never_exceeds_the_target() {
        let window = UVec2::new(1280, 720);
        assert_eq!(
            board_viewport(rect(0.0, 0.0, 1280.0, 720.0), 1.0, window),
            None,
            "sem dock a câmera usa a janela toda"
        );
        // Dock mais largo que a janela: nunca tamanho zero.
        let (_, size) = board_viewport(rect(0.0, 0.0, 0.0, 720.0), 1.0, window).unwrap();
        assert_eq!(size, UVec2::new(1, 720));
        let (pos, size) = board_viewport(rect(0.0, 0.0, 5000.0, 9000.0), 1.0, window)
            .unwrap_or((UVec2::ZERO, window));
        assert!(pos.x + size.x <= window.x && pos.y + size.y <= window.y);
        assert_eq!(
            board_viewport(rect(0.0, 0.0, 10.0, 10.0), 0.0, window),
            None
        );
    }

    /// Cursor -> casa com o viewport recortado: a casa sob o cursor continua a certa depois
    /// que o dock estreita a área (o 2D técnico centra o tabuleiro na área livre).
    #[test]
    fn the_cursor_to_square_conversion_uses_the_clipped_area() {
        use crate::board2d::{cursor_to_world, to_board_plane};
        use chess_input::ray_to_square;
        let dock = Dock {
            board_rect: Some(rect(0.0, 0.0, 1000.0, 720.0)),
            ..Dock::default()
        };
        let area = dock.board_size(Vec2::new(1280.0, 720.0));
        let at = |cursor: Vec2, size: Vec2| {
            let p = to_board_plane(cursor_to_world(cursor, size));
            ray_to_square(p + Vec3::Y, -Vec3::Y).map(|s| s.to_string())
        };
        // Centro da área livre (500, 360): o cruzamento d4/e4/d5/e5.
        assert_eq!(at(Vec2::new(501.0, 361.0), area).as_deref(), Some("e4"));
        assert_eq!(at(Vec2::new(499.0, 359.0), area).as_deref(), Some("d5"));
        // Com a janela toda como referência (comportamento antigo) o mesmo ponto erraria.
        assert_ne!(
            at(Vec2::new(501.0, 361.0), Vec2::new(1280.0, 720.0)).as_deref(),
            Some("e4")
        );
    }

    #[test]
    fn competitive_games_lock_hint_and_analysis_panels() {
        use crate::new_game::NewGameConfig;
        let hint = PanelDef {
            access: PanelAccess::OutsideCompetitive,
            ..def("Hint")
        };
        let mut session = Session::new();
        assert!(hint.accessible(&session));
        assert!(def("Any").accessible(&session));
        let config = NewGameConfig {
            policy: chess_game::PolicyKind::FideSimulation,
            ..NewGameConfig::default()
        };
        session.start(&config, None);
        assert!(session.is_competitive());
        assert!(!hint.accessible(&session), "dicas/análise travadas (UI-08)");
        assert!(def("Any").accessible(&session));
    }

    /// App com os plugins que registram os painéis nativos, sem janela nem GPU.
    fn app_with_all_panels() -> App {
        let mut app = App::new();
        app.add_plugins((
            crate::notation::NotationPlugin,
            crate::captured::CapturedPlugin,
            crate::study::StudyPlugin,
            crate::fide_panel::ArbitrationPlugin,
            crate::aids_panel::AidsPanelPlugin,
            crate::clock::ClockPlugin,
            crate::training_ui::TrainingPlugin,
            crate::analysis::AnalysisPlugin,
        ))
        .init_resource::<crate::aids::AidSettings>()
        .insert_resource(I18n::pt_br())
        .insert_resource(Session::new());
        app
    }

    fn render_all_panels(ctx: &egui::Context, app: &mut App) {
        let registry = app.world().resource::<PanelRegistry>().clone();
        let i18n = I18n::pt_br();
        for def in registry.iter() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(400.0, 600.0),
                )),
                ..Default::default()
            };
            let world = app.world_mut();
            let mut output = ctx.run_ui(input, |_| {
                egui::Area::new(egui::Id::new(def.id)).show(ctx, |ui| (def.ui)(ui, &i18n, world));
            });
            output.textures_delta.clear();
        }
    }

    /// UI-01: mostrar, ocultar e desenhar qualquer painel nativo não muda a Partida (nem
    /// emite Evento), em Partida casual e em competitiva com relógio.
    #[test]
    fn ui01_showing_and_hiding_every_panel_never_changes_the_game() {
        use crate::new_game::NewGameConfig;
        use chess_game::{PolicyKind, TimeControl};
        let mut app = app_with_all_panels();
        let ids_registered: Vec<&str> = app
            .world()
            .resource::<PanelRegistry>()
            .iter()
            .map(|d| d.id)
            .collect();
        for expected in DEFAULT_GROUPS.iter().flat_map(|g| g.iter()) {
            assert!(ids_registered.contains(expected), "{expected} sem registro");
        }
        let ctx = egui::Context::default();
        let competitive = NewGameConfig {
            policy: PolicyKind::FideSimulation,
            time: Some(TimeControl {
                base_ms: 600_000,
                increment_ms: 5_000,
            }),
            ..NewGameConfig::default()
        };
        for config in [NewGameConfig::default(), competitive] {
            app.world_mut()
                .resource_mut::<Session>()
                .start(&config, None);
            let (before, events) = {
                let s = app.world().resource::<Session>();
                (s.snapshot().clone(), s.game().events().len())
            };
            let mut dock = Dock::default();
            for id in dock.layout.pane_ids() {
                dock.layout.set_visible(&id, false);
                render_all_panels(&ctx, &mut app);
                dock.layout.set_visible(&id, true);
                render_all_panels(&ctx, &mut app);
            }
            let s = app.world().resource::<Session>();
            assert_eq!(s.snapshot(), &before);
            assert_eq!(s.game().events().len(), events);
        }
    }

    /// Spike: com o dock desenhado como no app (dentro de `run_ui`, como o bevy_egui), o egui
    /// só reivindica o ponteiro sobre o dock, não no centro do tabuleiro.
    #[test]
    fn the_centre_is_transparent_to_the_pointer_and_the_panel_is_not() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::default();
        let i18n = I18n::pt_br();
        let registry = builtin_registry();
        let mut world = World::new();
        world.insert_resource(Session::new());
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
                    registry: &registry,
                    world: &mut world,
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
