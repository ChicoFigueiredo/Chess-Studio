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
use crate::panel_chrome::{self, header_tools, palette_of, panel_icon, panel_menu, token};
use crate::session::Session;
use crate::ui_theme::{ThemeKind, Tokens};
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
    pub const LIBRARY: &str = "Library";
    pub const OPENINGS: &str = "Openings";
    pub const EXPLORER: &str = "Explorer";
    pub const SCORESHEET: &str = "Scoresheet";
    pub const BOARD2D: &str = "Board2d";
    /// Partida online no Lichess (#233).
    pub const ONLINE: &str = "Online";
}

/// Quando o painel pode ser aberto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAccess {
    Always,
    /// Dicas, análise e anotações: inacessíveis numa Partida competitiva em andamento
    /// (UI-08); liberados depois do resultado (`hud_policy`).
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
pub(crate) const MIN_WIDTH: f32 = 180.0;
pub(crate) const MAX_WIDTH: f32 = 640.0;

/// Grupos de abas do layout padrão, de cima para baixo.
const DEFAULT_GROUPS: [&[&str]; 3] = [
    &[ids::CLOCK, ids::CAPTURED, ids::BOARD2D],
    &[ids::NOTATION, ids::STUDY],
    &[
        ids::AIDS,
        ids::REVIEW,
        ids::ANALYSIS,
        ids::ARBITRATION,
        ids::TRAINING,
    ],
];

/// Fatia relativa do grupo do alto no layout padrão (os outros têm 1.0).
const TOP_GROUP_SHARE: f32 = 2.2;

fn default_rank(id: &str) -> usize {
    DEFAULT_GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .position(|d| *d == id)
        .unwrap_or(usize::MAX)
}

/// Tamanho mínimo e padrão de uma janela flutuante, em pontos do egui.
pub(crate) const MIN_FLOAT_SIZE: [f32; 2] = [180.0, 100.0];
const DEFAULT_FLOAT_SIZE: [f32; 2] = [320.0, 240.0];

/// Um painel destacado do dock numa janela flutuante dentro da janela do app (C18). Posição
/// e tamanho são do layout (por Perfil de interface); `home` lembra um irmão de grupo para
/// devolver o painel ao mesmo lugar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingPanel {
    pub id: PanelId,
    /// Canto superior esquerdo, em pontos do egui.
    pub pos: [f32; 2],
    /// Tamanho externo da janela, em pontos do egui.
    pub size: [f32; 2],
    /// Fechar a janela oculta o painel (como desmarcar na lista "Painéis").
    pub visible: bool,
    /// Outro painel do grupo de origem, se havia.
    #[serde(default)]
    pub home: Option<PanelId>,
}

impl FloatingPanel {
    pub fn new(id: PanelId, pos: [f32; 2], size: [f32; 2]) -> Self {
        let mut panel = Self {
            id,
            pos: [0.0; 2],
            size: DEFAULT_FLOAT_SIZE,
            visible: true,
            home: None,
        };
        panel.set_rect(pos, size);
        panel
    }

    /// Grava a geometria; valores não finitos viram o padrão e o tamanho respeita o mínimo.
    fn set_rect(&mut self, pos: [f32; 2], size: [f32; 2]) {
        for i in 0..2 {
            self.pos[i] = if pos[i].is_finite() { pos[i] } else { 0.0 };
            self.size[i] = if size[i].is_finite() {
                size[i].max(MIN_FLOAT_SIZE[i])
            } else {
                DEFAULT_FLOAT_SIZE[i]
            };
        }
    }
}

/// Layout do dock: a árvore de abas/divisões (com os tamanhos relativos e a visibilidade
/// de cada painel), a largura da região e as janelas flutuantes. É o que vai para o
/// SQLite e, por Perfil de interface, para o perfil.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    pub width: f32,
    pub tree: Tree<PanelId>,
    /// Painéis destacados. Ausente nos layouts anteriores ao C18 (vira lista vazia).
    #[serde(default)]
    pub floating: Vec<FloatingPanel>,
    /// Janelas do sistema com grupos de abas destacados (#168). No perfil de interface elas
    /// vão no campo `detached`, não dentro do layout.
    #[serde(default)]
    pub windows: Vec<crate::detached::DetachedWindow>,
}

impl std::fmt::Debug for DockLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockLayout")
            .field("width", &self.width)
            .field("tree", &self.tree)
            .field("floating", &self.floating)
            .field("windows", &self.windows)
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
        let first = groups.first().copied();
        let root = tiles.insert_vertical_tile(groups);
        // O grupo do alto leva a aba "Tabuleiro 2D", que precisa de altura (o tabuleiro é
        // quadrado): ele ganha o dobro da fatia dos demais.
        if let (Some(first), Some(Tile::Container(Container::Linear(linear)))) =
            (first, tiles.get_mut(root))
        {
            linear.shares.set_share(first, TOP_GROUP_SHARE);
        }
        Self {
            width: DEFAULT_WIDTH,
            tree: Tree::new("chess-studio-dock", root, tiles),
            floating: Vec::new(),
            windows: Vec::new(),
        }
    }
}

pub(crate) fn simplification() -> SimplificationOptions {
    // Mantém a barra de abas mesmo com um painel só: é a alça para arrastar.
    SimplificationOptions {
        prune_single_child_tabs: false,
        ..SimplificationOptions::default()
    }
}

impl DockLayout {
    pub(crate) fn floating_index(&self, id: &str) -> Option<usize> {
        self.floating.iter().position(|f| f.id.0 == id)
    }

    /// O painel está numa janela flutuante?
    pub fn is_floating(&self, id: &str) -> bool {
        self.floating_index(id).is_some()
    }

    /// Destaca o painel do dock para uma janela flutuante em `pos` com `size`. Falha (e não
    /// muda nada) se o painel não está no dock.
    pub fn detach(&mut self, id: &str, pos: [f32; 2], size: [f32; 2]) -> bool {
        let Some(tile) = self.tile_of(id) else {
            return false;
        };
        let visible = self.tree.is_visible(tile);
        let parent = self.tree.tiles.parent_of(tile);
        let home = parent.and_then(|p| match self.tree.tiles.get(p) {
            Some(Tile::Container(c)) => {
                c.children()
                    .find_map(|child| match self.tree.tiles.get(*child) {
                        Some(Tile::Pane(pane)) if *child != tile => Some(pane.clone()),
                        _ => None,
                    })
            }
            _ => None,
        });
        if let Some(parent) = parent
            && let Some(Tile::Container(c)) = self.tree.tiles.get_mut(parent)
        {
            c.remove_child(tile);
        }
        self.tree.tiles.remove(tile);
        self.tree.simplify(&simplification());
        let mut panel = FloatingPanel::new(PanelId::new(id), pos, size);
        panel.visible = visible;
        panel.home = home;
        self.floating.push(panel);
        true
    }

    /// Devolve a janela ao dock: ao grupo de origem se ainda existe, senão ao primeiro grupo
    /// de abas (recriando a raiz se o dock estava vazio). O painel volta visível e ativo.
    pub fn dock_back(&mut self, id: &str) -> bool {
        self.redock_for_drag(id).is_some()
    }

    /// Como [`Self::dock_back`], devolvendo o tile do Painel e o tamanho que a janela tinha:
    /// é o primeiro passo de arrastar o cabeçalho flutuante sobre o dock (#184), quando o
    /// Painel entra na árvore para o `egui_tiles` mostrar as guias e fazer o encaixe.
    pub fn redock_for_drag(&mut self, id: &str) -> Option<(TileId, [f32; 2])> {
        let index = self.floating_index(id)?;
        let panel = self.floating.remove(index);
        let home_group = panel
            .home
            .as_ref()
            .and_then(|h| self.tile_of(&h.0))
            .and_then(|t| self.tree.tiles.parent_of(t))
            .filter(|p| {
                matches!(
                    self.tree.tiles.get(*p),
                    Some(Tile::Container(Container::Tabs(_)))
                )
            });
        let group = home_group.or_else(|| self.first_tab_group());
        let size = panel.size;
        let pane = self.tree.tiles.insert_pane(panel.id);
        match group {
            Some(group) => {
                if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(group)
                {
                    tabs.add_child(pane);
                    tabs.set_active(pane);
                }
            }
            None => {
                let tabs = self.tree.tiles.insert_tab_tile(vec![pane]);
                match self.tree.root() {
                    Some(root) => match self.tree.tiles.get_mut(root) {
                        Some(Tile::Container(c)) => c.add_child(tabs),
                        _ => {
                            let both = self.tree.tiles.insert_vertical_tile(vec![root, tabs]);
                            self.tree.root = Some(both);
                        }
                    },
                    None => self.tree.root = Some(tabs),
                }
            }
        }
        Some((pane, size))
    }

    /// Move a aba `id` para o grupo de abas onde está `sibling` (o resultado de arrastá-la
    /// para lá). `false` se algum dos dois não está no dock ou o destino não é um grupo de abas.
    pub fn move_into_group_of(&mut self, id: &str, sibling: &str) -> bool {
        let (Some(tile), Some(group)) = (
            self.tile_of(id),
            self.tile_of(sibling)
                .and_then(|t| self.tree.tiles.parent_of(t)),
        ) else {
            return false;
        };
        if !matches!(
            self.tree.tiles.get(group),
            Some(Tile::Container(Container::Tabs(_)))
        ) {
            return false;
        }
        let end = self
            .tree
            .tiles
            .get_container(group)
            .map_or(0, |c| c.num_children());
        self.tree.move_tile_to_container(tile, group, end, false);
        self.tree.simplify(&simplification());
        self.activate(id);
        true
    }

    /// `a` e `b` estão no mesmo grupo de abas do dock.
    pub fn same_group(&self, a: &str, b: &str) -> bool {
        let group = |id: &str| self.tile_of(id).and_then(|t| self.tree.tiles.parent_of(t));
        matches!((group(a), group(b)), (Some(x), Some(y)) if x == y)
    }

    /// Primeiro grupo de abas na ordem da árvore (de cima para baixo).
    pub(crate) fn first_tab_group(&self) -> Option<TileId> {
        let mut stack = vec![self.tree.root()?];
        while let Some(tile) = stack.pop() {
            if let Some(Tile::Container(c)) = self.tree.tiles.get(tile) {
                if matches!(c, Container::Tabs(_)) {
                    return Some(tile);
                }
                stack.extend(c.children_vec().into_iter().rev());
            }
        }
        None
    }

    /// Atualiza posição e tamanho de uma janela flutuante (o usuário a moveu/redimensionou).
    pub fn set_floating_rect(&mut self, id: &str, pos: [f32; 2], size: [f32; 2]) {
        if let Some(i) = self.floating_index(id) {
            self.floating[i].set_rect(pos, size);
        }
    }

    pub(crate) fn tile_of(&self, id: &str) -> Option<TileId> {
        self.tree.tiles.find_pane(&PanelId::new(id))
    }

    /// O painel está visível? (`false` também se ele não existir na árvore.)
    pub fn is_visible(&self, id: &str) -> bool {
        if let Some(i) = self.floating_index(id) {
            return self.floating[i].visible;
        }
        if let Some((w, tile)) = self.window_tile_of(id) {
            return self.windows[w].tree.is_visible(tile);
        }
        self.tile_of(id).is_some_and(|t| self.tree.is_visible(t))
    }

    /// Mostra ou oculta um painel (UI-01). Nada além do layout muda.
    pub fn set_visible(&mut self, id: &str, visible: bool) {
        if let Some(i) = self.floating_index(id) {
            self.floating[i].visible = visible;
        } else if let Some((w, tile)) = self.window_tile_of(id) {
            self.windows[w].tree.set_visible(tile, visible);
        } else if let Some(tile) = self.tile_of(id) {
            self.tree.set_visible(tile, visible);
        }
    }

    /// Traz a aba do painel para a frente no grupo em que está.
    pub fn activate(&mut self, id: &str) {
        if self.is_floating(id) {
            self.set_visible(id, true);
            return;
        }
        if self.activate_in_window(id) {
            return;
        }
        let Some(tile) = self.tile_of(id) else { return };
        let Some(parent) = self.tree.tiles.parent_of(tile) else {
            return;
        };
        if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(parent) {
            tabs.set_active(tile);
        }
    }

    /// Algum painel **encaixado** visível? Sem nenhum, a região de dock some e o tabuleiro
    /// ocupa tudo (as janelas flutuantes não contam).
    pub fn any_visible(&self) -> bool {
        self.tree
            .tiles
            .iter()
            .any(|(tile, t)| matches!(t, Tile::Pane(_)) && self.tree.is_visible(*tile))
    }

    /// O painel **encaixado** e visível que está à frente no dock (a aba ativa do primeiro
    /// grupo). É o alvo da Ação "Flutuar Painel".
    pub fn active_docked_panel(&self) -> Option<String> {
        let active = self.tree.active_tiles();
        self.tree.tiles.iter().find_map(|(tile, t)| match t {
            Tile::Pane(p) if active.contains(tile) && self.tree.is_visible(*tile) => {
                Some(p.0.clone())
            }
            _ => None,
        })
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
            .chain(self.floating.iter().map(|f| f.id.0.clone()))
            .chain(self.windows.iter().flat_map(|w| w.pane_ids()))
            .collect()
    }

    /// Alinha a árvore ao registro: painéis registrados que faltam entram como abas extras
    /// na raiz; painéis que ninguém mais registra saem. Devolve se algo mudou.
    pub fn reconcile(&mut self, registry: &PanelRegistry) -> bool {
        let present = self.pane_ids();
        let floating_before = self.floating.len();
        self.floating.retain(|f| registry.get(&f.id.0).is_some());
        let windows_changed = self.retain_registered_in_windows(registry);
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
        let floating_changed = self.floating.len() != floating_before || windows_changed;
        if unknown.is_empty() && missing.is_empty() {
            return floating_changed;
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
    pub(crate) fn is_valid(&self) -> bool {
        self.width.is_finite()
            && (MIN_WIDTH..=MAX_WIDTH).contains(&self.width)
            // Um dock inteiro destacado em janelas (árvore vazia) é legítimo.
            && (self.tree.root().is_some() || !self.floating.is_empty() || !self.windows.is_empty())
    }

    /// Descarta janelas duplicadas ou de painéis que continuam na árvore e saneia a geometria.
    fn sanitize(mut self) -> Self {
        let in_tree: Vec<String> = self
            .tree
            .tiles
            .tiles()
            .filter_map(|t| match t {
                Tile::Pane(p) => Some(p.0.clone()),
                _ => None,
            })
            .collect();
        let mut seen: Vec<String> = Vec::new();
        let mut keep = Vec::new();
        for mut f in std::mem::take(&mut self.floating) {
            if in_tree.contains(&f.id.0) || seen.contains(&f.id.0) {
                continue;
            }
            seen.push(f.id.0.clone());
            let (pos, size) = (f.pos, f.size);
            f.set_rect(pos, size);
            keep.push(f);
        }
        self.floating = keep;
        self.sanitize_windows();
        self
    }

    /// Serialização para a preferência `dock_layout`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Lê o layout gravado; ilegível ou incompleto volta ao padrão (nunca falha).
    pub fn from_json(value: Option<serde_json::Value>) -> Self {
        value
            .and_then(|v| serde_json::from_value::<DockLayout>(v).ok())
            .map(DockLayout::sanitize)
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
    /// Janela pequena: o dock é sobreposição e está aberto pelo botão "Painéis" (C4, UI-10).
    pub overlay_open: bool,
    /// Região da sobreposição aberta, que não é tabuleiro para o ponteiro.
    blocked: Option<egui::Rect>,
    /// Retângulos das janelas flutuantes no último quadro: também não são tabuleiro.
    floating_rects: Vec<egui::Rect>,
    /// Janelas flutuantes cuja posição já foi conferida contra a tela ao abrir.
    float_checked: std::collections::HashSet<String>,
    /// Painel flutuante cujo cabeçalho virou arrasto de aba sobre o dock (#184).
    float_drag: Option<FloatDrag>,
    /// Arrasto de Painel em curso na janela principal, para detectar a saída dela (#195).
    drag_track: Option<DragTrack>,
    /// O Bevy avisou `CursorLeft` da janela principal neste quadro.
    cursor_left: bool,
    /// Painel que saiu da janela principal arrastado: vira janela destacada (#195).
    exit_request: Option<ExitRequest>,
}

/// Arrasto de um Painel (cabeçalho flutuante ou aba) que ainda pode sair da janela.
#[derive(Clone, Debug, PartialEq)]
struct DragTrack {
    panel: PanelId,
    /// Tamanho da janela destacada que nascer.
    size: [f32; 2],
    /// Última posição do ponteiro dentro da janela.
    last: Option<egui::Pos2>,
}

/// Pedido de janela destacada por um arrasto para fora (#195): o Bevy abre a janela do SO.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExitRequest {
    pub panel: PanelId,
    /// Ponto de saída, em pontos da janela principal.
    pub at: egui::Pos2,
    pub size: [f32; 2],
    /// Pixels por ponto no momento da saída.
    pub ppp: f32,
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
            overlay_open: false,
            blocked: None,
            floating_rects: Vec::new(),
            float_checked: std::collections::HashSet::new(),
            float_drag: None,
            drag_track: None,
            cursor_left: false,
            exit_request: None,
        }
    }

    /// O cursor saiu da janela principal (Bevy `CursorLeft`); só conta durante um arrasto.
    pub(crate) fn note_cursor_left(&mut self) {
        if self.drag_track.is_some() {
            self.cursor_left = true;
        }
    }

    /// Registra que o arrasto do Painel saiu da janela em `at` (e a evidência `--detach-demo drag`).
    pub(crate) fn request_exit(&mut self, panel: PanelId, at: egui::Pos2) {
        self.exit_request = Some(ExitRequest {
            panel,
            at,
            size: crate::detached::DEFAULT_WINDOW_SIZE,
            ppp: 1.0,
        });
    }

    pub(crate) fn take_exit_request(&mut self) -> Option<ExitRequest> {
        self.exit_request.take()
    }

    /// Põe o painel numa janela flutuante, em cascata a partir do canto superior esquerdo
    /// (a Ação "Flutuar Painel"; o menu da aba posiciona junto ao dock).
    pub fn float(&mut self, id: &str) -> bool {
        let n = self.layout.floating.len() as f32;
        let pos = [96.0 + 24.0 * n, 96.0 + 24.0 * n];
        self.layout.detach(id, pos, DEFAULT_FLOAT_SIZE)
    }

    /// "Restaurar layout padrão" (UI-02): também fecha as janelas flutuantes.
    pub fn reset(&mut self) {
        self.set_layout(DockLayout::default());
    }

    /// Troca o layout inteiro (perfil, restauração). O egui esquece larguras e posições de
    /// janela arrastadas, para valer o que o layout novo guarda.
    pub fn set_layout(&mut self, layout: DockLayout) {
        self.layout = layout;
        self.generation = self.generation.wrapping_add(1);
        self.float_checked.clear();
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
        let at = egui::pos2(cursor.x, cursor.y);
        self.board_rect.is_none_or(|r| r.contains(at))
            && !self.blocked.is_some_and(|b| b.contains(at))
            && !self.floating_rects.iter().any(|r| r.contains(at))
    }

    /// Canto superior esquerdo da área do tabuleiro em pontos (abaixo do Ribbon, se houver).
    pub fn board_origin(&self) -> Vec2 {
        self.board_rect
            .map_or(Vec2::ZERO, |r| Vec2::new(r.min.x, r.min.y))
    }

    /// Área livre do tabuleiro no último quadro (pontos do egui), se já conhecida.
    pub fn board_area(&self) -> Option<egui::Rect> {
        self.board_rect
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

/// Pedido feito durante o desenho (menu da aba, botão da janela) e aplicado depois: a
/// árvore não pode mudar enquanto o `egui_tiles` a percorre.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DockAction {
    Detach(PanelId),
    DockBack(PanelId),
    /// Duplo clique na aba: encaixado vira flutuante e flutuante volta ao dock.
    Toggle(PanelId),
    /// Aba solta fora de todo guia: o Painel flutua com o cabeçalho sob o ponteiro.
    FloatAt(PanelId, egui::Pos2),
    /// "Fechar Painel": oculta, como desmarcar na lista "Painéis".
    Close(PanelId),
    /// "Destacar em janela": só este Painel vai para uma janela do sistema (#168).
    DetachWindow(PanelId),
    /// "Destacar grupo em janela": o grupo de abas inteiro vai para uma janela do sistema.
    DetachGroup(PanelId),
    /// "Encaixar grupo": a janela do sistema inteira volta ao dock principal.
    DockWindow(u32),
    /// Cabeçalho flutuante arrastado até o dock: o Painel volta à árvore e o arrasto passa
    /// ao `egui_tiles`, que mostra as guias e encaixa na soltura (#184).
    BeginTileDrag(PanelId),
}

/// Arrasto de um Painel flutuante que virou arrasto de aba: guarda o tamanho da janela para
/// que soltar fora do dock devolva a janela do mesmo tamanho.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FloatDrag {
    pub panel: PanelId,
    pub size: [f32; 2],
}

pub(crate) struct DockBehavior<'a> {
    pub(crate) i18n: &'a I18n,
    registry: &'a PanelRegistry,
    world: &'a mut World,
    pub(crate) actions: Vec<DockAction>,
    /// Janela do sistema que este comportamento desenha; `None` na janela principal.
    pub(crate) window: Option<u32>,
    /// Tamanho da janela que nasce se a aba for solta fora do dock.
    float_size: [f32; 2],
    /// Aba do dock em arrasto neste quadro (preenchido por `draw_dock`).
    tab_drag: Option<PanelId>,
    /// Largura de cada aba do grupo neste quadro (PanelTabs: as inativas encolhem sem espaço),
    /// calculada em `top_bar_right_ui`, que o `egui_tiles` chama antes das abas.
    tab_widths: std::collections::HashMap<TileId, f32>,
}

impl<'a> DockBehavior<'a> {
    pub(crate) fn new(i18n: &'a I18n, registry: &'a PanelRegistry, world: &'a mut World) -> Self {
        Self {
            i18n,
            registry,
            world,
            actions: Vec::new(),
            window: None,
            float_size: DEFAULT_FLOAT_SIZE,
            tab_drag: None,
            tab_widths: std::collections::HashMap::new(),
        }
    }

    /// Conteúdo de um painel (no dock ou numa janela flutuante).
    fn panel_body(&mut self, ui: &mut egui::Ui, id: &PanelId) {
        let Some(def) = self.registry.get(&id.0) else {
            return;
        };
        let accessible = self
            .world
            .get_resource::<Session>()
            .is_none_or(|s| def.accessible(s));
        if accessible {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt(("dockpane", &id.0))
                .show(ui, |ui| {
                    // Padding de Painel do design system: `space-4` (Confortável) ou `space-3`
                    // (Compacta), que o tema guarda em `window_margin`.
                    let margin = ui.spacing().window_margin;
                    egui::Frame::NONE
                        .inner_margin(margin)
                        .show(ui, |ui| (def.ui)(ui, self.i18n, self.world));
                });
        } else {
            ui.weak(self.i18n.t("dock-unavailable-competitive"));
        }
    }

    pub(crate) fn title(&self, id: &PanelId) -> String {
        self.registry
            .get(&id.0)
            .map_or_else(|| id.0.clone(), |d| self.i18n.t(d.label_key))
    }
}

impl Behavior<PanelId> for DockBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile: TileId, pane: &mut PanelId) -> UiResponse {
        self.panel_body(ui, pane);
        UiResponse::None
    }

    /// Menu de contexto da aba: o mesmo do "⋯".
    fn on_tab_button(
        &mut self,
        tiles: &mut Tiles<PanelId>,
        tile_id: TileId,
        button_response: egui::Response,
    ) -> egui::Response {
        if let Some(Tile::Pane(pane)) = tiles.get(tile_id) {
            let pane = pane.clone();
            button_response.context_menu(|ui| match self.window {
                Some(window) => {
                    panel_chrome::window_menu(ui, self.i18n, &mut self.actions, &pane, window)
                }
                None => panel_menu(ui, self.i18n, &mut self.actions, &pane, false),
            });
        }
        button_response
    }

    /// Aba conforme PanelTabs: ativa em `surface-1` com traço `accent` de 2px no alto;
    /// inativas em `ink-muted`; ícone + nome. Duplo clique alterna encaixado/flutuante.
    fn tab_ui(
        &mut self,
        tiles: &mut Tiles<PanelId>,
        ui: &mut egui::Ui,
        id: egui::Id,
        tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Response {
        let pane = match tiles.get(tile_id) {
            Some(Tile::Pane(pane)) => Some(pane.clone()),
            _ => None,
        };
        let title = self.tab_title_for_tile(tiles, tile_id).text().to_owned();
        let close_tip = self.i18n.t("dock-close");
        let (response, closed) = panel_chrome::tab_button_fitted(
            ui,
            id,
            pane.as_ref().map(|p| panel_icon(&p.0)),
            &title,
            state.active,
            state.is_being_dragged,
            egui::Sense::click_and_drag(),
            Some(&close_tip),
            self.tab_widths.get(&tile_id).copied(),
        );
        if closed && let Some(pane) = pane.clone() {
            self.actions.push(DockAction::Close(pane));
        } else if response.double_clicked()
            && let Some(pane) = pane
        {
            self.actions.push(DockAction::Toggle(pane));
        }
        self.on_tab_button(tiles, tile_id, response)
    }

    /// Ferramentas do grupo, sobre a aba ativa.
    fn top_bar_right_ui(
        &mut self,
        tiles: &Tiles<PanelId>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let active = tabs.active.and_then(|t| match tiles.get(t) {
            Some(Tile::Pane(pane)) => Some(pane.clone()),
            _ => None,
        });
        if let Some(active) = active {
            ui.add_space(4.0);
            match self.window {
                Some(window) => panel_chrome::window_header_tools(
                    ui,
                    self.i18n,
                    &mut self.actions,
                    &active,
                    window,
                ),
                None => header_tools(ui, self.i18n, &mut self.actions, &active, false),
            }
        }
        // Sobrou para as abas o que não é dos botões da direita: reparte entre elas.
        let ids: Vec<(TileId, Option<PanelId>)> = tabs
            .children
            .iter()
            .copied()
            .map(|child| match tiles.get(child) {
                Some(Tile::Pane(pane)) => (child, Some(pane.clone())),
                _ => (child, None),
            })
            .collect();
        let natural: Vec<f32> = ids
            .iter()
            .map(|(child, pane)| {
                let title = pane.as_ref().map_or_else(String::new, |p| self.title(p));
                panel_chrome::tab_natural_width(
                    ui,
                    pane.as_ref().map(|p| panel_icon(&p.0)),
                    &title,
                    tabs.active == Some(*child),
                    true,
                )
            })
            .collect();
        let active_index = ids
            .iter()
            .position(|(child, _)| tabs.active == Some(*child));
        let widths = panel_chrome::fit_tab_widths(
            &natural,
            active_index,
            ui.available_width(),
            panel_chrome::TAB_MIN_WIDTH,
        );
        self.tab_widths = ids.iter().map(|(child, _)| *child).zip(widths).collect();
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        token("panel-header-h", 32.0)
    }

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        palette_of(visuals).surface_2
    }

    /// Divisória de 1px `border` entre grupos; `accent` ao apontar ou arrastar.
    fn resize_stroke(
        &self,
        style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        let p = palette_of(&style.visuals);
        match resize_state {
            egui_tiles::ResizeState::Idle => egui::Stroke::new(1.0, p.border),
            _ => egui::Stroke::new(2.0, p.accent),
        }
    }

    fn tab_bar_hline_stroke(&self, visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, palette_of(visuals).border)
    }

    /// Guia de encaixe (DockGuide): o `egui_tiles` calcula acima, abaixo, lados e centro
    /// (como aba); a área de destino vai em `accent-subtle` com borda tracejada `accent`.
    fn paint_drag_preview(
        &self,
        visuals: &egui::Visuals,
        painter: &egui::Painter,
        _parent_rect: Option<egui::Rect>,
        preview_rect: egui::Rect,
    ) {
        panel_chrome::paint_guide(painter, palette_of(visuals), preview_rect);
    }

    fn tab_title_for_pane(&mut self, pane: &PanelId) -> egui::WidgetText {
        self.registry
            .get(&pane.0)
            .map_or_else(|| pane.0.clone(), |d| self.i18n.t(d.label_key))
            .into()
    }

    fn simplification_options(&self) -> SimplificationOptions {
        simplification()
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
    width: f32,
    overlay: bool,
) -> f32 {
    let screen = crate::ribbon::usable_rect(ctx);
    let max_width = if overlay {
        width
    } else {
        MAX_WIDTH.min(screen.width() * 0.6).max(MIN_WIDTH)
    };
    let rect = egui::Rect::from_min_max(egui::pos2(screen.max.x - width, screen.min.y), screen.max);
    let id = egui::Id::new(("chess-studio-dock", generation));
    let mut new_width = width;
    // Aba em arrasto (só Painéis): se for solta fora do dock, não há guia e o Painel flutua.
    let dragged = layout
        .tree
        .dragged_id(ctx)
        .and_then(|t| match layout.tree.tiles.get(t) {
            Some(Tile::Pane(pane)) => Some(pane.clone()),
            _ => None,
        });
    egui::Area::new(id)
        .order(if overlay {
            egui::Order::Foreground
        } else {
            egui::Order::Middle
        })
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
            if let Some(pane) = dragged {
                behavior.tab_drag = Some(pane.clone());
                let pointer = ctx.input(|i| i.pointer.latest_pos());
                if panel_chrome::dropped_outside(pointer, rect)
                    && let Some(pointer) = pointer
                {
                    let size = behavior.float_size;
                    let at = panel_chrome::float_pos_at_drop(pointer, size, screen);
                    if ctx.input(|i| i.pointer.any_released()) {
                        behavior
                            .actions
                            .push(DockAction::FloatAt(pane, egui::pos2(at[0], at[1])));
                    } else {
                        let painter = ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new("chess-studio-float-guide"),
                        ));
                        let ghost = egui::Rect::from_min_size(
                            egui::pos2(at[0], at[1]),
                            egui::vec2(size[0], size[1]),
                        );
                        panel_chrome::paint_guide(
                            &painter,
                            palette_of(&ctx.global_style().visuals),
                            ghost,
                        );
                    }
                }
            }
            let handle = egui::Rect::from_min_size(rect.min, egui::vec2(6.0, rect.height()));
            let response = ui
                .interact(handle, id.with("resize"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if response.dragged() {
                new_width = (width - response.drag_delta().x).clamp(MIN_WIDTH, max_width);
            }
        });
    // Só o arrasto do usuário muda a largura guardada: o layout responsivo não a sobrescreve.
    if (new_width - width).abs() > f32::EPSILON && !overlay {
        layout.width = new_width;
    }
    new_width
}

/// Desenha os painéis flutuantes como janelas do egui dentro da janela do app (FloatingPanel):
/// contorno `border-strong`, `radius-l` e `shadow-floating`, com o cabeçalho de PanelTabs.
/// Posição e tamanho vão para o layout (por Perfil de interface); uma posição fora da tela
/// volta para dentro ao abrir. Devolve os retângulos ocupados.
fn draw_floating(
    ctx: &egui::Context,
    generation: u32,
    layout: &mut DockLayout,
    behavior: &mut DockBehavior<'_>,
    checked: &mut std::collections::HashSet<String>,
) -> (Vec<egui::Rect>, Option<HeaderDrag>) {
    let screen = crate::ribbon::usable_rect(ctx);
    let visuals = ctx.global_style().visuals.clone();
    let p = palette_of(&visuals);
    let theme = ThemeKind::from_dark_mode(visuals.dark_mode);
    let shadow = Tokens::embedded()
        .shadow("shadow-floating", theme)
        .unwrap_or_default();
    // Fechada e reaberta, a janela é conferida de novo.
    checked.retain(|id| layout.floating.iter().any(|f| f.visible && f.id.0 == *id));
    let mut rects = Vec::new();
    let mut moved: Vec<(String, [f32; 2], [f32; 2])> = Vec::new();
    let mut header_drag = None;
    for panel in layout.floating.iter_mut().filter(|f| f.visible) {
        if checked.insert(panel.id.0.clone()) {
            panel.pos = panel_chrome::clamp_floating(panel.pos, panel.size, screen);
        }
        let title = behavior.title(&panel.id);
        let frame = egui::Frame::NONE
            .fill(p.surface_1)
            .stroke(egui::Stroke::new(1.0, p.border_strong))
            .corner_radius(panel_chrome::radius("radius-l"))
            .shadow(shadow);
        let shown = egui::Window::new(title.clone())
            .id(egui::Id::new(("chess-studio-float", generation, &panel.id)))
            .current_pos(egui::pos2(panel.pos[0], panel.pos[1]))
            .default_size(egui::vec2(panel.size[0], panel.size[1]))
            .min_size(egui::vec2(MIN_FLOAT_SIZE[0], MIN_FLOAT_SIZE[1]))
            .title_bar(false)
            .frame(frame)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                panel_chrome::floating_header(
                    ui,
                    behavior.i18n,
                    &mut behavior.actions,
                    &panel.id,
                    &title,
                );
                behavior.panel_body(ui, &panel.id);
            });
        if let Some(shown) = shown {
            let rect = shown.response.rect;
            rects.push(rect);
            // Só o arrasto que começou no cabeçalho tenta encaixar (corpo e bordas não).
            let grab = egui::Id::new(("chess-studio-float-grab", generation, &panel.id));
            if shown.response.drag_started() {
                let in_header = ctx
                    .input(|i| i.pointer.press_origin())
                    .is_some_and(|o| o.y <= rect.min.y + token("panel-header-h", 32.0) + 4.0);
                ctx.data_mut(|d| d.insert_temp(grab, in_header));
            }
            let stopped = shown.response.drag_stopped();
            if (shown.response.dragged() || stopped)
                && ctx.data(|d| d.get_temp::<bool>(grab)).unwrap_or(false)
            {
                header_drag = Some(HeaderDrag {
                    panel: panel.id.clone(),
                    stopped,
                });
            }
            let (pos, size) = ([rect.min.x, rect.min.y], [rect.width(), rect.height()]);
            let differs =
                |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() > 0.5 || (a[1] - b[1]).abs() > 0.5;
            if differs(pos, panel.pos) || differs(size, panel.size) {
                moved.push((panel.id.0.clone(), pos, size));
            }
        }
    }
    for (id, pos, size) in moved {
        layout.set_floating_rect(&id, pos, size);
    }
    (rects, header_drag)
}

/// Janela flutuante cujo cabeçalho está sendo arrastado (ou acabou de ser solto).
struct HeaderDrag {
    panel: PanelId,
    stopped: bool,
}

/// Sistema exclusivo: os painéis recebem o `World` (ver [`PanelUi`]).
pub(crate) fn dock_ui(world: &mut World) {
    let Some(ctx) = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .iter_mut(world)
        .next()
        .map(|mut c| c.get_mut().clone())
    else {
        return;
    };
    let screen = crate::ribbon::usable_rect(&ctx);
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
            // Evidência (`--dock-demo regroup|float`): reproduz o resultado de arrastar uma
            // aba para outro grupo ou de destacá-la em janela (C18).
            if world.contains_resource::<CaptureMode>()
                && let Some(demo) = crate::capture::option_value("--dock-demo")
            {
                dock.layout.reconcile(world.resource::<PanelRegistry>());
                match demo.as_str() {
                    "window" => {
                        dock.layout.detach_group_to_window(
                            ids::NOTATION,
                            crate::detached::DEFAULT_WINDOW_SIZE,
                        );
                    }
                    "regroup" => {
                        dock.layout.move_into_group_of(ids::CLOCK, ids::NOTATION);
                    }
                    "float" => {
                        dock.layout
                            .detach(ids::STUDY, [160.0, 90.0], DEFAULT_FLOAT_SIZE);
                    }
                    _ => {}
                }
            }
            // Evidência (`--screenshot ... --tabs captured,study`): traz essas abas à frente.
            if (world.contains_resource::<CaptureMode>()
                || world.contains_resource::<crate::benchmark::Bench>())
                && let Some(list) = crate::capture::option_value("--tabs")
            {
                // Painéis novos só entram no layout salvo na reconciliação; faça-a antes.
                dock.layout.reconcile(world.resource::<PanelRegistry>());
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
                let responsive = crate::responsive::responsive_layout(
                    Vec2::new(screen.width(), screen.height()),
                    dock.layout.width,
                );
                let (placed_width, overlay) = match responsive.dock {
                    crate::responsive::DockPlacement::Side { width } => (width, false),
                    crate::responsive::DockPlacement::Overlay { width } => (width, true),
                };
                dock.blocked = None;
                dock.floating_rects.clear();
                if hidden {
                    dock.inset = 0.0;
                    dock.board_rect = Some(screen);
                    return;
                }
                let mut behavior = DockBehavior::new(&i18n, &registry, world);
                frame_dock(&ctx, dock, &mut behavior, screen, placed_width, overlay);
            });
        });
    });
}

/// Painel arrastado (cabeçalho flutuante ou aba) que sai da janela principal vira janela
/// destacada (#195): registra o pedido para o Bevy abrir a janela do SO onde ele saiu e
/// cancela o resto do arrasto (flutuar, encaixar).
fn track_drag_exit(
    ctx: &egui::Context,
    dock: &mut Dock,
    behavior: &mut DockBehavior<'_>,
    screen: egui::Rect,
    placed_width: f32,
    header_panel: Option<PanelId>,
) {
    let tab_panel = behavior.tab_drag.take();
    let (pointer, down, released) = ctx.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.any_down(),
            i.pointer.any_released(),
        )
    });
    let cursor_left = std::mem::take(&mut dock.cursor_left);
    let dragging = header_panel
        .map(|p| {
            let size = dock
                .layout
                .floating
                .iter()
                .find(|f| f.id == p)
                .map_or(DEFAULT_FLOAT_SIZE, |f| f.size);
            (p, size)
        })
        .or_else(|| tab_panel.map(|p| (p, crate::detached::DEFAULT_WINDOW_SIZE)));
    if let Some((panel, size)) = dragging {
        let last = pointer.or(dock.drag_track.as_ref().and_then(|t| t.last));
        dock.drag_track = Some(DragTrack { panel, size, last });
    }
    let Some(track) = dock.drag_track.clone() else {
        return;
    };
    if !down && !released {
        dock.drag_track = None;
        return;
    }
    let window = ctx.content_rect();
    let region = egui::Rect::from_min_max(
        egui::pos2(window.max.x - placed_width, screen.min.y),
        window.max,
    );
    let protected = pointer.is_some_and(|p| {
        region.contains(p)
            || crate::panel_stripe::drop_on_stripe(Some(p), crate::panel_stripe::drop_zone(ctx))
    });
    if let Some(at) = crate::detached::drag_exit_point(
        pointer,
        track.last,
        window,
        cursor_left,
        released,
        protected,
    ) {
        dock.drag_track = None;
        dock.exit_request = Some(ExitRequest {
            panel: track.panel.clone(),
            at,
            size: track.size,
            ppp: ctx.pixels_per_point(),
        });
        behavior.actions.retain(|a| {
            !matches!(a, DockAction::FloatAt(p, _) | DockAction::BeginTileDrag(p) | DockAction::DockBack(p)
                if *p == track.panel)
        });
        ctx.stop_dragging();
    }
}

/// Um quadro do dock e das janelas flutuantes: desenha, aplica as ações e atualiza as
/// áreas do tabuleiro. Separado de [`dock_ui`] para os testes dirigirem o mesmo caminho.
fn frame_dock(
    ctx: &egui::Context,
    dock: &mut Dock,
    behavior: &mut DockBehavior<'_>,
    screen: egui::Rect,
    placed_width: f32,
    overlay: bool,
) {
    let any_visible = dock.layout.any_visible();
    let show_dock = any_visible && (!overlay || dock.overlay_open);
    let mut dock_left = screen.max.x;
    let mut width = 0.0;
    // Soltar a aba fora do dock devolve a janela do tamanho que ela tinha.
    behavior.float_size = dock
        .float_drag
        .as_ref()
        .map_or(DEFAULT_FLOAT_SIZE, |d| d.size);
    if show_dock {
        width = draw_dock(
            ctx,
            dock.generation,
            &mut dock.layout,
            behavior,
            placed_width,
            overlay,
        );
        dock_left = screen.max.x - width;
    }
    let (floating_rects, header_drag) = draw_floating(
        ctx,
        dock.generation,
        &mut dock.layout,
        behavior,
        &mut dock.float_checked,
    );
    dock.floating_rects = floating_rects;
    track_drag_exit(
        ctx,
        dock,
        behavior,
        screen,
        placed_width,
        header_drag.as_ref().map(|d| d.panel.clone()),
    );
    if let Some(drag) = header_drag
        && dock.float_drag.is_none()
        && (!overlay || dock.overlay_open)
    {
        // A região do dock é sempre a borda direita; com o dock vazio ela vira a zona de
        // soltura (guia + rótulo) e a soltura sobre ela recria o dock.
        let region = egui::Rect::from_min_max(
            egui::pos2(screen.max.x - placed_width, screen.min.y),
            screen.max,
        );
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        // A faixa de Painéis (#185) é alvo de soltura; a região do dock termina antes dela.
        if drag.stopped
            && crate::panel_stripe::drop_on_stripe(pointer, crate::panel_stripe::drop_zone(ctx))
        {
            behavior
                .actions
                .push(DockAction::DockBack(drag.panel.clone()));
        }
        match panel_chrome::float_drag_zone(pointer, region, any_visible) {
            panel_chrome::FloatDragZone::Dock if !drag.stopped => {
                behavior.actions.push(DockAction::BeginTileDrag(drag.panel));
            }
            panel_chrome::FloatDragZone::EmptyDock if drag.stopped => {
                behavior.actions.push(DockAction::DockBack(drag.panel));
            }
            panel_chrome::FloatDragZone::EmptyDock => {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("chess-studio-empty-dock-zone"),
                ));
                let palette = palette_of(&ctx.global_style().visuals);
                panel_chrome::paint_guide(&painter, palette, region.shrink(6.0));
                painter.text(
                    region.center(),
                    egui::Align2::CENTER_CENTER,
                    behavior.i18n.t("dock-drop-here"),
                    egui::FontId::proportional(token("font-size-body", 14.0)),
                    palette.accent,
                );
            }
            _ => {}
        }
    }
    for action in std::mem::take(&mut behavior.actions) {
        let float_here = |layout: &mut DockLayout, id: &PanelId| {
            let n = layout.floating.len() as f32;
            let size = DEFAULT_FLOAT_SIZE;
            let pos = [
                (dock_left - size[0] - 16.0 - 24.0 * n).max(0.0),
                (72.0 + 24.0 * n).min((screen.height() - size[1]).max(0.0)),
            ];
            layout.detach(&id.0, pos, size);
        };
        match action {
            DockAction::Detach(id) => float_here(&mut dock.layout, &id),
            DockAction::DockBack(id) => {
                dock.layout.dock_back(&id.0);
            }
            DockAction::Toggle(id) => {
                if dock.layout.is_floating(&id.0) {
                    dock.layout.dock_back(&id.0);
                } else {
                    float_here(&mut dock.layout, &id);
                }
            }
            DockAction::FloatAt(id, at) => {
                let size = dock
                    .float_drag
                    .as_ref()
                    .filter(|d| d.panel == id)
                    .map_or(DEFAULT_FLOAT_SIZE, |d| d.size);
                dock.layout.detach(&id.0, [at.x, at.y], size);
            }
            DockAction::Close(id) => dock.layout.set_visible(&id.0, false),
            DockAction::DetachWindow(id) => {
                dock.layout
                    .detach_panel_to_window(&id.0, crate::detached::DEFAULT_WINDOW_SIZE);
            }
            DockAction::DetachGroup(id) => {
                dock.layout
                    .detach_group_to_window(&id.0, crate::detached::DEFAULT_WINDOW_SIZE);
            }
            DockAction::DockWindow(window) => {
                dock.layout.dock_window_back(window);
            }
            DockAction::BeginTileDrag(id) => {
                if let Some((tile, size)) = dock.layout.redock_for_drag(&id.0) {
                    // O arrasto do mouse continua; agora quem o conduz é o `egui_tiles`.
                    ctx.set_dragged_id(tile.egui_id(dock.layout.tree.id()));
                    dock.float_drag = Some(FloatDrag { panel: id, size });
                }
            }
        }
    }
    if dock.float_drag.is_some() && !ctx.input(|i| i.pointer.any_down()) {
        dock.float_drag = None;
    }
    if !show_dock {
        dock.inset = 0.0;
        dock.board_rect = Some(screen);
        return;
    }
    if overlay {
        // Sobre o tabuleiro: ele segue com a janela toda, mas a área coberta não é dele.
        dock.inset = 0.0;
        dock.board_rect = Some(screen);
        dock.blocked = Some(egui::Rect::from_min_max(
            egui::pos2(screen.max.x - width, screen.min.y),
            screen.max,
        ));
    } else {
        dock.inset = width;
        dock.board_rect = Some(egui::Rect::from_min_max(
            screen.min,
            egui::pos2(screen.max.x - width, screen.max.y),
        ));
    }
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
            floating: Vec::new(),
            windows: Vec::new(),
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
            crate::board2d_panel::Board2dPanelPlugin,
            crate::study::StudyPlugin,
            crate::fide_panel::ArbitrationPlugin,
            crate::aids_panel::AidsPanelPlugin,
            crate::clock::ClockPlugin,
            crate::training_ui::TrainingPlugin,
            crate::analysis::AnalysisPlugin,
            crate::net_service::NetServicePlugin,
            crate::explorer::ExplorerPlugin,
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
                let mut behavior = DockBehavior::new(&i18n, &registry, &mut world);
                let mut output = ctx.run_ui(input, |_| {
                    let width = layout.width;
                    draw_dock(&ctx, 0, &mut layout, &mut behavior, width, false);
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

    // ---- C18: layout flexível (arrastar, dividir, destacar em janela) ----

    fn group_of(layout: &DockLayout, id: &str) -> Option<TileId> {
        layout
            .tile_of(id)
            .and_then(|t| layout.tree.tiles.parent_of(t))
    }

    #[test]
    fn dragging_a_tab_into_another_group_moves_it_there() {
        let mut layout = DockLayout::default();
        let tile = layout.tile_of(ids::CLOCK).unwrap();
        let target = group_of(&layout, ids::NOTATION).unwrap();
        layout.tree.move_tile_to_container(tile, target, 1, false);
        assert_eq!(group_of(&layout, ids::CLOCK), Some(target));
        assert_eq!(group_of(&layout, ids::NOTATION), Some(target));
        let back = DockLayout::from_json(Some(layout.to_json()));
        assert_eq!(back, layout, "o novo agrupamento é gravado");
    }

    #[test]
    fn detaching_a_panel_floats_it_and_keeps_it_visible_and_saved() {
        let mut layout = DockLayout::default();
        assert!(layout.detach(ids::STUDY, [40.0, 60.0], [300.0, 220.0]));
        assert!(layout.is_floating(ids::STUDY));
        assert!(layout.is_visible(ids::STUDY));
        assert!(layout.tile_of(ids::STUDY).is_none(), "saiu da árvore");
        assert!(layout.pane_ids().contains(&ids::STUDY.to_owned()));
        assert!(layout.any_visible(), "o dock segue com os outros painéis");
        assert!(
            !layout.detach(ids::STUDY, [0.0, 0.0], [1.0, 1.0]),
            "só uma vez"
        );
        assert!(!layout.detach("Inexistente", [0.0, 0.0], [1.0, 1.0]));
        // Posição e tamanho vão para o layout salvo.
        layout.set_floating_rect(ids::STUDY, [100.0, 120.0], [410.0, 250.0]);
        let back = DockLayout::from_json(Some(layout.to_json()));
        assert_eq!(back, layout);
        let f = &back.floating[0];
        assert_eq!((f.pos, f.size), ([100.0, 120.0], [410.0, 250.0]));
        // Fechar a janela oculta o painel sem devolvê-lo.
        layout.set_visible(ids::STUDY, false);
        assert!(!layout.is_visible(ids::STUDY));
        assert!(layout.is_floating(ids::STUDY));
    }

    #[test]
    fn docking_back_returns_the_panel_to_its_origin_group() {
        let mut layout = DockLayout::default();
        let origin = group_of(&layout, ids::NOTATION).unwrap();
        layout.detach(ids::STUDY, [0.0, 0.0], [300.0, 200.0]);
        assert!(layout.dock_back(ids::STUDY));
        assert!(!layout.is_floating(ids::STUDY));
        assert_eq!(group_of(&layout, ids::STUDY), Some(origin));
        assert!(layout.is_visible(ids::STUDY));
        assert!(!layout.dock_back(ids::STUDY), "já está no dock");
    }

    #[test]
    fn docking_back_falls_back_to_the_first_group_when_the_origin_is_gone() {
        let mut layout = DockLayout::default();
        // Notação e Anotações viram janelas: o grupo do meio desaparece.
        layout.detach(ids::NOTATION, [0.0, 0.0], [300.0, 200.0]);
        layout.detach(ids::STUDY, [20.0, 20.0], [300.0, 200.0]);
        assert!(layout.dock_back(ids::STUDY));
        assert!(layout.tile_of(ids::STUDY).is_some());
        assert!(layout.is_visible(ids::STUDY));
        // Mesmo com o dock inteiro destacado, devolver recria a raiz.
        let mut all = DockLayout::default();
        for (i, id) in all.pane_ids().iter().enumerate() {
            all.detach(id, [i as f32, 0.0], [200.0, 100.0]);
        }
        assert!(!all.any_visible(), "nada no dock, só janelas");
        assert!(all.is_valid(), "só janelas é um layout legítimo");
        let back = DockLayout::from_json(Some(all.to_json()));
        assert_eq!(back, all);
        assert!(all.dock_back(ids::CLOCK));
        assert!(all.any_visible());
        assert!(all.tile_of(ids::CLOCK).is_some());
    }

    #[test]
    fn floating_panels_survive_reconcile_and_unregistered_ones_leave() {
        let mut layout = DockLayout::default();
        layout.detach(ids::STUDY, [0.0, 0.0], [300.0, 200.0]);
        layout.detach(ids::CLOCK, [0.0, 0.0], [300.0, 200.0]);
        let mut registry = builtin_registry();
        assert!(!layout.reconcile(&registry), "janela não é painel faltando");
        assert!(layout.tile_of(ids::STUDY).is_none());
        let mut reduced = PanelRegistry::default();
        for d in registry.iter().filter(|d| d.id != ids::STUDY) {
            reduced.register(*d);
        }
        assert!(layout.reconcile(&reduced));
        assert!(!layout.is_floating(ids::STUDY));
        assert!(layout.is_floating(ids::CLOCK));
        registry.register(def("Novo"));
        assert!(layout.reconcile(&registry));
        assert!(layout.tile_of("Novo").is_some());
    }

    #[test]
    fn layouts_saved_before_floating_windows_still_open() {
        // Até o C17 só havia `width` e `tree`, sem o campo `floating`.
        let mut json = DockLayout::default().to_json();
        json.as_object_mut().unwrap().remove("floating");
        let loaded = DockLayout::from_json(Some(json));
        assert_eq!(loaded, DockLayout::default());
        assert!(loaded.floating.is_empty());
    }

    #[test]
    fn a_layout_with_a_corrupt_floating_entry_falls_back_to_the_default() {
        let mut json = DockLayout::default().to_json();
        json["floating"] = serde_json::json!([{ "id": 7 }]);
        assert_eq!(DockLayout::from_json(Some(json)), DockLayout::default());
    }

    #[test]
    fn a_floating_window_of_a_pane_still_in_the_tree_is_dropped_on_load() {
        let mut layout = DockLayout::default();
        layout.floating.push(FloatingPanel::new(
            PanelId::new(ids::CLOCK),
            [0.0, 0.0],
            [300.0, 200.0],
        ));
        let loaded = DockLayout::from_json(Some(layout.to_json()));
        assert!(loaded.floating.is_empty(), "sem painel duplicado");
        assert_eq!(
            loaded.pane_ids().len(),
            DockLayout::default().pane_ids().len()
        );
    }

    #[test]
    fn non_finite_floating_geometry_is_sanitized() {
        let mut layout = DockLayout::default();
        layout.detach(ids::STUDY, [0.0, 0.0], [300.0, 200.0]);
        layout.set_floating_rect(ids::STUDY, [f32::NAN, 5.0], [f32::INFINITY, 2.0]);
        let f = &layout.floating[0];
        assert!(f.pos.iter().chain(f.size.iter()).all(|v| v.is_finite()));
        assert!(f.size[0] >= MIN_FLOAT_SIZE[0] && f.size[1] >= MIN_FLOAT_SIZE[1]);
    }

    #[test]
    fn reset_clears_floating_windows_and_applying_a_layout_forgets_window_memory() {
        let mut dock = Dock::default();
        dock.layout.detach(ids::STUDY, [0.0, 0.0], [300.0, 200.0]);
        dock.reset();
        assert!(dock.layout.floating.is_empty());
        let generation = dock.generation;
        dock.set_layout(DockLayout::default());
        assert_ne!(dock.generation, generation);
    }

    #[test]
    fn floating_windows_are_not_the_board() {
        let dock = Dock {
            board_rect: Some(rect(0.0, 0.0, 1000.0, 720.0)),
            floating_rects: vec![rect(100.0, 100.0, 300.0, 300.0)],
            ..Dock::default()
        };
        assert!(!dock.pointer_in_board(Vec2::new(150.0, 150.0)));
        assert!(dock.pointer_in_board(Vec2::new(500.0, 500.0)));
    }

    /// Arrasta de verdade, com eventos do egui, a aba do Relógio para a borda esquerda do
    /// grupo da Notação: o `egui_tiles` está ligado ao nosso `Behavior` e reagrupa.
    #[test]
    fn a_real_pointer_drag_of_a_tab_into_another_group_regroups_it() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::default();
        let i18n = I18n::pt_br();
        let registry = builtin_registry();
        let mut world = World::new();
        world.insert_resource(Session::new());
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let mut frame = |layout: &mut DockLayout, events: Vec<egui::Event>| {
            for pass in 0..2 {
                let mut behavior = DockBehavior::new(&i18n, &registry, &mut world);
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events: if pass == 0 { events.clone() } else { vec![] },
                    ..Default::default()
                };
                let width = layout.width;
                let mut out = ctx.run_ui(input, |_| {
                    draw_dock(&ctx, 0, layout, &mut behavior, width, false);
                });
                out.textures_delta.clear();
            }
        };
        frame(&mut layout, vec![]);
        let clock_group = group_of(&layout, ids::CLOCK).unwrap();
        let notation_group = group_of(&layout, ids::NOTATION).unwrap();
        let from = layout.tree.tiles.rect(clock_group).unwrap();
        let start = from.left_top() + egui::vec2(20.0, 8.0);
        let target = layout.tree.tiles.rect(notation_group).unwrap();
        let drop_at = egui::pos2(target.center().x, target.center().y);
        let mouse = |pos: egui::Pos2, pressed: Option<bool>| {
            let mut v = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = pressed {
                v.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            v
        };
        frame(&mut layout, mouse(start, None));
        frame(&mut layout, mouse(start, Some(true)));
        frame(&mut layout, mouse(start + egui::vec2(0.0, 30.0), None));
        frame(&mut layout, mouse(drop_at, None));
        frame(&mut layout, mouse(drop_at, None));
        // O egui_tiles reconhece o arrasto da aba (alguma das abas do grupo do Relógio).
        let dragged = layout
            .tree
            .dragged_id(&ctx)
            .expect("o arrasto da aba começou");
        let moved = layout.tree.tiles.get_pane(&dragged).unwrap().0.clone();
        let before = group_of(&layout, &moved);
        frame(&mut layout, mouse(drop_at, Some(false)));
        frame(&mut layout, vec![]);
        assert_ne!(group_of(&layout, &moved), before, "a aba mudou de grupo");
        assert!(layout.is_visible(&moved));
        assert_eq!(
            layout.pane_ids().len(),
            DockLayout::default().pane_ids().len()
        );
    }

    #[test]
    fn redocking_for_a_drag_puts_the_panel_in_the_tree_and_remembers_the_window_size() {
        let mut layout = DockLayout::default();
        layout.detach(ids::STUDY, [200.0, 100.0], [300.0, 200.0]);
        let (tile, size) = layout.redock_for_drag(ids::STUDY).unwrap();
        assert_eq!(size, [300.0, 200.0]);
        assert_eq!(layout.tile_of(ids::STUDY), Some(tile));
        assert!(!layout.is_floating(ids::STUDY));
        assert!(layout.redock_for_drag(ids::STUDY).is_none(), "só flutuante");
    }

    #[test]
    fn redocking_into_an_empty_dock_recreates_the_root() {
        let mut layout = DockLayout::default();
        for id in layout.pane_ids() {
            layout.detach(&id, [0.0, 0.0], [300.0, 200.0]);
        }
        assert!(!layout.any_visible());
        let (tile, _) = layout.redock_for_drag(ids::CLOCK).unwrap();
        assert!(layout.any_visible());
        assert_eq!(layout.tile_of(ids::CLOCK), Some(tile));
    }

    /// Dirige o mesmo caminho do app (`frame_dock`) com eventos reais do egui.
    struct FloatRig {
        ctx: egui::Context,
        dock: Dock,
        i18n: I18n,
        registry: PanelRegistry,
        world: World,
        screen: egui::Rect,
    }

    impl FloatRig {
        fn new(dock: Dock) -> Self {
            let mut world = World::new();
            world.insert_resource(Session::new());
            Self {
                ctx: egui::Context::default(),
                dock,
                i18n: I18n::pt_br(),
                registry: builtin_registry(),
                world,
                screen: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0)),
            }
        }

        fn frame(&mut self, pos: Option<egui::Pos2>, pressed: Option<bool>) {
            let mut events = Vec::new();
            if let Some(pos) = pos {
                events.push(egui::Event::PointerMoved(pos));
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            for pass in 0..2 {
                let mut behavior = DockBehavior::new(&self.i18n, &self.registry, &mut self.world);
                let input = egui::RawInput {
                    screen_rect: Some(self.screen),
                    events: if pass == 0 { events.clone() } else { vec![] },
                    ..Default::default()
                };
                let (ctx, dock, screen) = (self.ctx.clone(), &mut self.dock, self.screen);
                let mut out = ctx.run_ui(input, |_| {
                    frame_dock(&ctx, dock, &mut behavior, screen, DEFAULT_WIDTH, false);
                });
                out.textures_delta.clear();
            }
        }

        /// Pressiona o cabeçalho da janela flutuante e arrasta até `to`.
        fn drag_header_to(&mut self, grab: egui::Pos2, to: egui::Pos2) {
            self.frame(Some(grab), None);
            self.frame(Some(grab), Some(true));
            self.frame(Some(grab + egui::vec2(12.0, 6.0)), None);
            self.frame(Some(grab + egui::vec2(40.0, 20.0)), None);
            self.frame(Some(to), None);
            self.frame(Some(to), None);
        }

        /// Continua arrastando (botão preso) até `to`.
        fn drag_header_to_dock_target(&mut self, to: egui::Pos2) {
            for _ in 0..3 {
                self.frame(Some(to), None);
            }
        }

        fn release_at(&mut self, at: egui::Pos2) {
            self.frame(Some(at), Some(false));
            self.frame(None, None);
        }
    }

    fn dock_with_study_floating() -> Dock {
        let mut dock = Dock::default();
        dock.layout
            .detach(ids::STUDY, [200.0, 100.0], [300.0, 200.0]);
        dock
    }

    /// Ponto vazio do cabeçalho da janela (longe da aba e das ferramentas).
    const HEADER_GRAB: egui::Pos2 = egui::pos2(340.0, 116.0);

    #[test]
    fn dragging_a_floating_header_over_the_dock_hands_the_drag_to_the_tiles() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(1100.0, 300.0));
        assert!(rig.dock.float_drag.is_some(), "o arrasto passou ao dock");
        assert!(!rig.dock.layout.is_floating(ids::STUDY));
        let dragged = rig.dock.layout.tree.dragged_id(&rig.ctx);
        assert_eq!(
            dragged,
            rig.dock.layout.tile_of(ids::STUDY),
            "aba em arrasto"
        );
    }

    #[test]
    fn dropping_a_floating_header_in_the_middle_of_a_group_makes_it_a_tab_there() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(1100.0, 300.0));
        let aids = group_of(&rig.dock.layout, ids::AIDS).unwrap();
        let target = rig.dock.layout.tree.tiles.rect(aids).unwrap();
        rig.drag_header_to_dock_target(target.center());
        rig.release_at(target.center());
        assert_eq!(
            group_of(&rig.dock.layout, ids::STUDY),
            group_of(&rig.dock.layout, ids::AIDS),
            "virou aba do grupo"
        );
        assert!(rig.dock.layout.floating.is_empty());
    }

    #[test]
    fn dropping_a_floating_header_on_a_group_edge_splits_beside_it() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(1100.0, 300.0));
        let aids = group_of(&rig.dock.layout, ids::AIDS).unwrap();
        let target = rig.dock.layout.tree.tiles.rect(aids).unwrap();
        let side = egui::pos2(target.center().x, target.top() + 4.0);
        rig.drag_header_to_dock_target(side);
        rig.release_at(side);
        let study = group_of(&rig.dock.layout, ids::STUDY).unwrap();
        assert_ne!(
            Some(study),
            group_of(&rig.dock.layout, ids::AIDS),
            "grupo novo"
        );
        assert!(rig.dock.layout.floating.is_empty());
        assert_eq!(
            rig.dock.layout.pane_ids().len(),
            DockLayout::default().pane_ids().len()
        );
    }

    #[test]
    fn dropping_outside_the_dock_keeps_the_panel_floating_where_it_was_dropped() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(1100.0, 300.0));
        let outside = egui::pos2(500.0, 400.0);
        rig.drag_header_to_dock_target(outside);
        rig.release_at(outside);
        assert!(rig.dock.layout.is_floating(ids::STUDY));
        let f = &rig.dock.layout.floating[0];
        assert_eq!(f.size, [300.0, 200.0], "mesmo tamanho");
        assert!(
            (f.pos[0] + 150.0 - outside.x).abs() < 1.0,
            "sob o ponteiro: {f:?}"
        );
        assert!(rig.dock.layout.tile_of(ids::STUDY).is_none());
    }

    #[test]
    fn an_empty_dock_shows_a_drop_zone_and_a_release_on_it_recreates_the_dock() {
        let mut dock = Dock::default();
        for id in dock.layout.pane_ids() {
            dock.layout.detach(&id, [600.0, 400.0], [300.0, 200.0]);
        }
        dock.layout.floating[0].pos = [200.0, 100.0];
        let first = dock.layout.floating[0].id.0.clone();
        let mut rig = FloatRig::new(dock);
        rig.frame(None, None);
        assert!(!rig.dock.layout.any_visible());
        let zone = egui::pos2(1100.0, 300.0);
        rig.drag_header_to(HEADER_GRAB, zone);
        assert!(
            rig.dock.layout.is_floating(&first),
            "ainda flutua até soltar"
        );
        rig.release_at(zone);
        assert!(!rig.dock.layout.is_floating(&first));
        assert!(rig.dock.layout.any_visible(), "o dock voltou");
        assert!(rig.dock.layout.tile_of(&first).is_some());
    }

    #[test]
    fn dragging_a_floating_header_out_of_the_window_asks_for_a_detached_window() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(500.0, 400.0));
        assert!(rig.dock.exit_request.is_none(), "dentro da janela");
        rig.frame(Some(egui::pos2(1290.0, 380.0)), None);
        let request = rig.dock.exit_request.clone().expect("saiu da janela");
        assert_eq!(request.panel.0, ids::STUDY);
        assert_eq!(request.size, [300.0, 200.0], "mesmo tamanho");
        assert_eq!(request.at, egui::pos2(1280.0, 380.0));
    }

    #[test]
    fn the_cursor_left_event_during_a_drag_asks_for_a_detached_window() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(500.0, 400.0));
        rig.dock.note_cursor_left();
        rig.frame(Some(egui::pos2(500.0, 400.0)), None);
        assert!(rig.dock.exit_request.is_some());
        // Sem arrasto, o aviso não conta.
        let mut idle = FloatRig::new(dock_with_study_floating());
        idle.frame(None, None);
        idle.dock.note_cursor_left();
        idle.frame(Some(egui::pos2(500.0, 400.0)), None);
        assert!(idle.dock.exit_request.is_none());
    }

    #[test]
    fn releasing_a_floating_header_on_the_left_edge_asks_for_a_detached_window() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(300.0, 300.0));
        let edge = egui::pos2(1.0, 300.0);
        rig.drag_header_to_dock_target(edge);
        assert!(rig.dock.exit_request.is_none(), "ainda segurando");
        rig.release_at(edge);
        assert!(rig.dock.exit_request.is_some());
        assert!(
            rig.dock.layout.is_floating(ids::STUDY),
            "o Bevy é que abre a janela"
        );
    }

    #[test]
    fn an_ordinary_drop_never_asks_for_a_detached_window() {
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(500.0, 400.0));
        rig.release_at(egui::pos2(500.0, 400.0));
        assert!(rig.dock.exit_request.is_none());
        assert!(rig.dock.layout.is_floating(ids::STUDY));
        // Soltar no dock, mesmo encostado na borda direita, encaixa.
        let mut rig = FloatRig::new(dock_with_study_floating());
        rig.frame(None, None);
        rig.drag_header_to(HEADER_GRAB, egui::pos2(1100.0, 300.0));
        let aids = group_of(&rig.dock.layout, ids::AIDS).unwrap();
        let target = rig.dock.layout.tree.tiles.rect(aids).unwrap().center();
        rig.drag_header_to_dock_target(target);
        let edge = egui::pos2(1279.0, target.y);
        rig.drag_header_to_dock_target(edge);
        rig.release_at(edge);
        assert!(rig.dock.exit_request.is_none());
        assert!(rig.dock.layout.floating.is_empty());
    }
}
