//! Painel destacado em janela do sistema (issue #168, DS-03): um grupo de abas inteiro (ou um
//! Painel só, que é um grupo de uma aba) vai para uma janela do SO, que pode ir para o
//! 2º monitor. A janela guarda a própria árvore `egui_tiles`: as abas se reorganizam e se
//! dividem lá dentro. "Encaixar" e "Encaixar grupo" são Ações explícitas: não existe arrasto
//! entre janelas do SO (ADR-0005); criar a janela arrastando um Painel para fora da janela
//! principal é permitido (#195).
//!
//! Este módulo tem três partes:
//! 1. o estado puro ([`DetachedWindow`] e as transições em `DockLayout`), testado sem Bevy;
//! 2. a posição segura ([`MonitorRect`], [`restore_windows`]): uma janela fora de todos os
//!    monitores volta para o dock da janela principal ao abrir;
//! 3. o plugin ([`DetachedPlugin`]): uma `Window` do Bevy + uma câmera de UI + um contexto
//!    egui por janela destacada. Só Apresentação (ADR-0001): os Painéis leem os mesmos
//!    recursos e só a `Session` recebe intenções. Nenhum Painel precisa de textura de render.

use crate::dock::{Dock, DockAction, DockBehavior, DockLayout, PanelId, PanelRegistry};
use crate::i18n::I18n;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::window::{
    CursorLeft, Monitor, PrimaryWindow, WindowCloseRequested, WindowPosition, WindowRef,
};
use bevy_egui::{EguiContext, EguiSchedule, egui};
use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use serde::{Deserialize, Serialize};

/// Quantas janelas destacadas ao mesmo tempo (cada uma tem um contexto egui próprio e o
/// `bevy_egui` pede um schedule único por contexto).
pub const MAX_WINDOWS: usize = 4;
/// Tamanho padrão de uma janela destacada, em pontos lógicos.
pub const DEFAULT_WINDOW_SIZE: [f32; 2] = [420.0, 520.0];
const MIN_WINDOW_SIZE: [f32; 2] = [220.0, 140.0];

/// Janela do sistema com um grupo de abas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetachedWindow {
    /// Identificador estável da janela no layout.
    pub id: u32,
    /// Abas da janela (reordenáveis e divisíveis).
    pub tree: Tree<PanelId>,
    /// Canto superior esquerdo em pixels físicos da área de trabalho; `None` deixa o sistema
    /// escolher.
    pub pos: Option<[i32; 2]>,
    /// Tamanho interno em pontos lógicos.
    pub size: [f32; 2],
    /// Índice do monitor onde estava (informativo; a posição é que manda).
    pub monitor: Option<u32>,
    /// Painel do grupo de origem no dock, para devolver um Painel destacado sozinho ao mesmo
    /// lugar.
    #[serde(default)]
    pub home: Option<PanelId>,
}

impl DetachedWindow {
    fn new(id: u32, panes: &[(PanelId, bool)], active: Option<&PanelId>) -> Self {
        let mut tiles = Tiles::default();
        let ids: Vec<TileId> = panes
            .iter()
            .map(|(p, _)| tiles.insert_pane(p.clone()))
            .collect();
        let root = tiles.insert_tab_tile(ids.clone());
        let mut tree = Tree::new(format!("chess-studio-window-{id}"), root, tiles);
        for ((_, visible), tile) in panes.iter().zip(&ids) {
            if !visible {
                tree.set_visible(*tile, false);
            }
        }
        if let Some(active) = active
            && let Some(tile) = tree.tiles.find_pane(active)
            && let Some(Tile::Container(Container::Tabs(tabs))) = tree.tiles.get_mut(root)
        {
            tabs.set_active(tile);
        }
        Self {
            id,
            tree,
            pos: None,
            size: DEFAULT_WINDOW_SIZE,
            monitor: None,
            home: None,
        }
    }

    /// Painéis da janela, em ordem de árvore (esquerda para a direita, de cima para baixo).
    pub fn pane_ids(&self) -> Vec<String> {
        self.panes().into_iter().map(|(p, _)| p.0).collect()
    }

    /// Painéis com a visibilidade, em ordem de árvore.
    fn panes(&self) -> Vec<(PanelId, bool)> {
        let mut out = Vec::new();
        let mut stack: Vec<TileId> = self.tree.root().into_iter().collect();
        while let Some(tile) = stack.pop() {
            match self.tree.tiles.get(tile) {
                Some(Tile::Pane(p)) => out.push((p.clone(), self.tree.is_visible(tile))),
                Some(Tile::Container(c)) => stack.extend(c.children_vec().into_iter().rev()),
                None => {}
            }
        }
        out
    }

    fn has_visible_pane(&self) -> bool {
        self.panes().iter().any(|(_, visible)| *visible)
    }

    fn set_rect(&mut self, pos: Option<[i32; 2]>, size: [f32; 2]) {
        self.pos = pos;
        for i in 0..2 {
            self.size[i] = if size[i].is_finite() {
                size[i].clamp(MIN_WINDOW_SIZE[i], 16_000.0)
            } else {
                DEFAULT_WINDOW_SIZE[i]
            };
        }
    }
}

impl DockLayout {
    fn window_index(&self, window: u32) -> Option<usize> {
        self.windows.iter().position(|w| w.id == window)
    }

    /// Janela e aba do Painel `id`, se ele está numa janela destacada.
    pub(crate) fn window_tile_of(&self, id: &str) -> Option<(usize, TileId)> {
        self.windows.iter().enumerate().find_map(|(i, w)| {
            w.tree
                .tiles
                .find_pane(&PanelId::new(id))
                .map(|tile| (i, tile))
        })
    }

    /// Traz a aba do Painel à frente na janela dele. `false` se ele não está numa janela.
    pub(crate) fn activate_in_window(&mut self, id: &str) -> bool {
        let Some((w, tile)) = self.window_tile_of(id) else {
            return false;
        };
        let tree = &mut self.windows[w].tree;
        if let Some(parent) = tree.tiles.parent_of(tile)
            && let Some(Tile::Container(Container::Tabs(tabs))) = tree.tiles.get_mut(parent)
        {
            tabs.set_active(tile);
        }
        tree.set_visible(tile, true);
        true
    }

    fn next_window_id(&self) -> u32 {
        self.windows.iter().map(|w| w.id).max().map_or(1, |m| m + 1)
    }

    /// Destaca o grupo de abas do Painel `id` para uma janela do sistema. Se o Painel está
    /// flutuante, ele vira uma janela de um Painel só. Devolve o id da janela; `None` (sem
    /// mudar nada) se o Painel não está no dock nem flutuante, ou se há janelas demais.
    pub fn detach_group_to_window(&mut self, id: &str, size: [f32; 2]) -> Option<u32> {
        self.detach_to_window(id, size, true)
    }

    /// Destaca só o Painel `id` (uma janela com uma aba).
    pub fn detach_panel_to_window(&mut self, id: &str, size: [f32; 2]) -> Option<u32> {
        self.detach_to_window(id, size, false)
    }

    fn detach_to_window(&mut self, id: &str, size: [f32; 2], whole_group: bool) -> Option<u32> {
        if self.windows.len() >= MAX_WINDOWS {
            return None;
        }
        let window_id = self.next_window_id();
        // Painel flutuante: a janela do SO substitui a flutuante.
        if let Some(i) = self.floating_index(id) {
            let panel = self.floating.remove(i);
            let mut window = DetachedWindow::new(
                window_id,
                &[(panel.id.clone(), panel.visible)],
                Some(&panel.id),
            );
            window.home = panel.home;
            window.set_rect(None, size);
            self.windows.push(window);
            return Some(window_id);
        }
        let tile = self.tile_of(id)?;
        let parent = self.tree.tiles.parent_of(tile);
        let group_is_tabs = parent.is_some_and(|p| {
            matches!(
                self.tree.tiles.get(p),
                Some(Tile::Container(Container::Tabs(_)))
            )
        });
        let panel = PanelId::new(id);
        let mut window = if whole_group && group_is_tabs {
            let group = parent?;
            let tabs = match self.tree.tiles.get(group) {
                Some(Tile::Container(Container::Tabs(tabs))) => tabs.clone(),
                _ => return None,
            };
            let panes: Vec<(PanelId, bool)> = tabs
                .children
                .iter()
                .filter_map(|child| match self.tree.tiles.get(*child) {
                    Some(Tile::Pane(p)) => Some((p.clone(), self.tree.is_visible(*child))),
                    _ => None,
                })
                .collect();
            let active = tabs.active.and_then(|a| match self.tree.tiles.get(a) {
                Some(Tile::Pane(p)) => Some(p.clone()),
                _ => None,
            });
            self.remove_tile_and_simplify(group);
            DetachedWindow::new(window_id, &panes, active.as_ref().or(Some(&panel)))
        } else {
            let visible = self.tree.is_visible(tile);
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
            self.remove_tile_and_simplify(tile);
            let mut w = DetachedWindow::new(window_id, &[(panel.clone(), visible)], Some(&panel));
            w.home = home;
            w
        };
        window.set_rect(None, size);
        self.windows.push(window);
        Some(window_id)
    }

    /// Tira o ladrilho (e os filhos) da árvore principal; a raiz pode ficar vazia.
    fn remove_tile_and_simplify(&mut self, tile: TileId) {
        let was_root = self.tree.root() == Some(tile);
        self.tree.remove_recursively(tile);
        if was_root {
            self.tree.root = None;
        }
        self.tree.simplify(&crate::dock::simplification());
    }

    /// Põe os Painéis de volta no dock principal: no grupo de `home` se ele ainda existe,
    /// senão num grupo de abas novo no fim da raiz (ou como a raiz, se o dock está vazio). O
    /// primeiro Painel visível fica ativo.
    fn add_to_main(&mut self, panes: Vec<(PanelId, bool)>, home: Option<&PanelId>) {
        if panes.is_empty() {
            return;
        }
        let home_group = home
            .and_then(|h| self.tile_of(&h.0))
            .and_then(|t| self.tree.tiles.parent_of(t))
            .filter(|p| {
                matches!(
                    self.tree.tiles.get(*p),
                    Some(Tile::Container(Container::Tabs(_)))
                )
            });
        let tiles: Vec<(TileId, bool)> = panes
            .into_iter()
            .map(|(p, visible)| (self.tree.tiles.insert_pane(p), visible))
            .collect();
        let first_visible = tiles.iter().find(|(_, v)| *v).map(|(t, _)| *t);
        let group = match home_group {
            Some(group) => {
                if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(group)
                {
                    for (tile, _) in &tiles {
                        tabs.add_child(*tile);
                    }
                }
                group
            }
            None => {
                let group = self
                    .tree
                    .tiles
                    .insert_tab_tile(tiles.iter().map(|(t, _)| *t).collect());
                match self.tree.root() {
                    Some(root) => match self.tree.tiles.get_mut(root) {
                        Some(Tile::Container(c)) => c.add_child(group),
                        _ => {
                            let both = self.tree.tiles.insert_vertical_tile(vec![root, group]);
                            self.tree.root = Some(both);
                        }
                    },
                    None => self.tree.root = Some(group),
                }
                group
            }
        };
        for (tile, visible) in &tiles {
            if !visible {
                self.tree.set_visible(*tile, false);
            }
        }
        if let Some(active) = first_visible
            && let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(group)
        {
            tabs.set_active(active);
        }
    }

    /// "Encaixar grupo": todas as abas da janela voltam ao dock principal e a janela some.
    pub fn dock_window_back(&mut self, window: u32) -> bool {
        let Some(i) = self.window_index(window) else {
            return false;
        };
        let w = self.windows.remove(i);
        let panes = w.panes();
        self.add_to_main(panes, w.home.as_ref());
        true
    }

    /// "Encaixar" um Painel: só essa aba sai da janela (a janela some se esvaziar).
    pub fn dock_panel_from_window(&mut self, id: &str) -> bool {
        let Some((w, tile)) = self.window_tile_of(id) else {
            return false;
        };
        let visible = self.windows[w].tree.is_visible(tile);
        let window = &mut self.windows[w];
        let was_root = window.tree.root() == Some(tile);
        window.tree.remove_recursively(tile);
        if was_root {
            window.tree.root = None;
        }
        window.tree.simplify(&crate::dock::simplification());
        let home = window.home.clone();
        let empty = window.tree.root().is_none() || window.panes().is_empty();
        if empty {
            self.windows.remove(w);
        }
        self.add_to_main(vec![(PanelId::new(id), visible)], home.as_ref());
        true
    }

    /// Janela que "Encaixar" (a Ação) devolve: a última aberta.
    pub fn last_window(&self) -> Option<u32> {
        self.windows.last().map(|w| w.id)
    }

    /// O usuário moveu ou redimensionou a janela do SO.
    pub fn set_window_rect(
        &mut self,
        window: u32,
        pos: Option<[i32; 2]>,
        size: [f32; 2],
        monitor: Option<u32>,
    ) {
        if let Some(i) = self.window_index(window) {
            self.windows[i].set_rect(pos, size);
            self.windows[i].monitor = monitor;
        }
    }

    /// Janelas sem nenhuma aba visível (todas fechadas) voltam ao dock, ainda ocultas.
    /// Devolve se alguma voltou.
    pub fn dock_back_empty_windows(&mut self) -> bool {
        let empty: Vec<u32> = self
            .windows
            .iter()
            .filter(|w| !w.has_visible_pane())
            .map(|w| w.id)
            .collect();
        for id in &empty {
            self.dock_window_back(*id);
        }
        !empty.is_empty()
    }

    /// Tira das janelas os Painéis que ninguém mais registra. Devolve se algo mudou.
    pub(crate) fn retain_registered_in_windows(&mut self, registry: &PanelRegistry) -> bool {
        let mut changed = false;
        for w in &mut self.windows {
            for id in w.pane_ids() {
                if registry.get(&id).is_none()
                    && let Some(tile) = w.tree.tiles.find_pane(&PanelId::new(&id))
                {
                    w.tree.remove_recursively(tile);
                    if w.tree.root() == Some(tile) {
                        w.tree.root = None;
                    }
                    changed = true;
                }
            }
            if changed {
                w.tree.simplify(&crate::dock::simplification());
            }
        }
        let before = self.windows.len();
        self.windows.retain(|w| !w.panes().is_empty());
        changed || self.windows.len() != before
    }

    /// Ao carregar: janelas vazias, repetidas ou de Painéis que continuam no dock saem, o
    /// excesso de janelas é cortado e a geometria é saneada.
    pub(crate) fn sanitize_windows(&mut self) {
        let mut seen: Vec<String> = self
            .tree
            .tiles
            .tiles()
            .filter_map(|t| match t {
                Tile::Pane(p) => Some(p.0.clone()),
                _ => None,
            })
            .chain(self.floating.iter().map(|f| f.id.0.clone()))
            .collect();
        let mut ids: Vec<u32> = Vec::new();
        let mut keep = Vec::new();
        for mut w in std::mem::take(&mut self.windows) {
            if ids.contains(&w.id) || keep.len() >= MAX_WINDOWS {
                continue;
            }
            for id in w.pane_ids() {
                if seen.contains(&id)
                    && let Some(tile) = w.tree.tiles.find_pane(&PanelId::new(&id))
                {
                    w.tree.remove_recursively(tile);
                    if w.tree.root() == Some(tile) {
                        w.tree.root = None;
                    }
                }
            }
            w.tree.simplify(&crate::dock::simplification());
            let panes = w.pane_ids();
            if panes.is_empty() {
                continue;
            }
            seen.extend(panes);
            ids.push(w.id);
            let (pos, size) = (w.pos, w.size);
            w.set_rect(pos, size);
            keep.push(w);
        }
        self.windows = keep;
    }
}

/// Retângulo de um monitor em pixels físicos da área de trabalho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorRect {
    pub pos: [i32; 2],
    pub size: [u32; 2],
}

impl MonitorRect {
    fn contains(&self, point: [i32; 2]) -> bool {
        (0..2).all(|i| {
            point[i] >= self.pos[i]
                && i64::from(point[i]) < i64::from(self.pos[i]) + i64::from(self.size[i])
        })
    }
}

/// Folga, em pontos, para considerar que a soltura "encostou" na borda da janela principal.
const EDGE_TOUCH: f32 = 2.0;

/// Arrastar um Painel para fora (#195): o ponto, dentro da janela principal, por onde o
/// arrasto saiu, ou `None` se o Painel ainda está dentro. Sai quando o ponteiro está fora
/// de `screen` (ou sumiu, `pointer == None`), quando o Bevy avisou `CursorLeft`
/// (`cursor_left`), ou quando é solto (`released`) encostado na borda. `last` é a última
/// posição conhecida dentro da janela; `protected` marca a soltura em alvos de encaixe
/// junto da borda (dock, faixa de Painéis), que não contam como sair.
pub fn drag_exit_point(
    pointer: Option<egui::Pos2>,
    last: Option<egui::Pos2>,
    screen: egui::Rect,
    cursor_left: bool,
    released: bool,
    protected: bool,
) -> Option<egui::Pos2> {
    let clamp = |p: egui::Pos2| screen.clamp(p);
    let outside = cursor_left || pointer.is_none_or(|p| !screen.contains(p));
    if outside {
        return pointer.or(last).map(clamp);
    }
    let p = pointer?;
    let touching = p.x - screen.min.x <= EDGE_TOUCH
        || screen.max.x - p.x <= EDGE_TOUCH
        || p.y - screen.min.y <= EDGE_TOUCH
        || screen.max.y - p.y <= EDGE_TOUCH;
    (released && !protected && touching).then_some(p)
}

/// Posição (pixels físicos da área de trabalho) da janela destacada nascida ao arrastar:
/// a barra de título sob o ponto de saída (`exit`, em pontos da janela principal em
/// `main_pos`, com `ppp` pixels por ponto), trazida para dentro do monitor do ponteiro (ou
/// do primeiro, se ele estiver fora de todos). `size` é o tamanho lógico da janela.
pub fn exit_window_pos(
    main_pos: [i32; 2],
    exit: egui::Pos2,
    ppp: f32,
    size: [f32; 2],
    monitors: &[MonitorRect],
) -> [i32; 2] {
    let ppp = if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    };
    let pointer = [
        main_pos[0].saturating_add((exit.x * ppp).round() as i32),
        main_pos[1].saturating_add((exit.y * ppp).round() as i32),
    ];
    let wanted = [
        pointer[0].saturating_sub((size[0] * ppp * 0.5).round() as i32),
        pointer[1].saturating_sub((16.0 * ppp).round() as i32),
    ];
    let Some(monitor) = monitors
        .iter()
        .find(|m| m.contains(pointer))
        .or_else(|| monitors.first())
    else {
        return wanted;
    };
    let axis = |i: usize, extent: f32| {
        let lo = i64::from(monitor.pos[i]);
        let hi = (lo + i64::from(monitor.size[i]) - (extent * ppp).round() as i64).max(lo);
        i64::from(wanted[i]).clamp(lo, hi) as i32
    };
    [axis(0, size[0]), axis(1, size[1])]
}

/// A "alça" da janela (um pedaço da barra de título) que precisa estar num monitor para o
/// usuário conseguir alcançá-la.
fn handle_of(pos: [i32; 2]) -> [i32; 2] {
    [pos[0].saturating_add(64), pos[1].saturating_add(16)]
}

/// A janela em `pos` pode ser alcançada em algum monitor?
pub fn reachable(pos: [i32; 2], monitors: &[MonitorRect]) -> bool {
    monitors.iter().any(|m| m.contains(handle_of(pos)))
}

/// Índice do monitor onde está a alça da janela.
pub fn monitor_index_at(pos: [i32; 2], monitors: &[MonitorRect]) -> Option<u32> {
    monitors
        .iter()
        .position(|m| m.contains(handle_of(pos)))
        .map(|i| i as u32)
}

/// Ao abrir: janelas fora de todos os monitores (o monitor foi desligado, por exemplo)
/// voltam ao dock da janela principal. Sem monitores conhecidos, nada muda. Devolve quantas
/// voltaram.
pub fn restore_windows(layout: &mut DockLayout, monitors: &[MonitorRect]) -> usize {
    if monitors.is_empty() {
        return 0;
    }
    let lost: Vec<u32> = layout
        .windows
        .iter()
        .filter(|w| w.pos.is_some_and(|p| !reachable(p, monitors)))
        .map(|w| w.id)
        .collect();
    for id in &lost {
        layout.dock_window_back(*id);
    }
    lost.len()
}

// ---------------------------------------------------------------------------------------
// Plugin: janelas, câmeras e contextos egui.
// ---------------------------------------------------------------------------------------

/// Passe do egui de uma janela destacada (um schedule único por contexto, como pede o
/// `bevy_egui`).
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DetachedPass(pub usize);

/// Câmera de UI de uma janela destacada.
#[derive(Component, Debug)]
pub struct DetachedCamera;

/// Janela do Bevy de uma janela destacada.
#[derive(Component, Debug)]
pub struct DetachedWindowMarker {
    pub window_id: u32,
}

/// A janela nasceu de um arrasto com o botão ainda pressionado (#201): assim que o SO a
/// criou, `Window::start_drag_move` entrega o resto do arrasto ao gerenciador de janelas
/// (funciona em X11 e Wayland; a posição por `WindowPosition` não funciona no Wayland).
#[derive(Component, Debug, Default)]
pub struct FollowMouse {
    /// Quadros desde que a entidade foi criada.
    pub age: u32,
}

/// O que fazer com uma janela que deve acompanhar o mouse neste quadro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowStep {
    /// A janela do SO ainda não existe.
    Wait,
    /// Pedir o arrasto ao sistema.
    Start,
    /// O botão foi solto antes: não há o que acompanhar.
    Cancel,
}

/// Quadros até a janela do SO existir (o `spawn` é adiado e o Bevy cria a janela depois).
const FOLLOW_AFTER_FRAMES: u32 = 2;

pub fn follow_step(age: u32, button_pressed: bool) -> FollowStep {
    if !button_pressed {
        FollowStep::Cancel
    } else if age < FOLLOW_AFTER_FRAMES {
        FollowStep::Wait
    } else {
        FollowStep::Start
    }
}

fn follow_mouse(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    mut windows: Query<(Entity, &mut Window, &mut FollowMouse)>,
) {
    for (entity, mut window, mut follow) in &mut windows {
        match follow_step(follow.age, buttons.pressed(MouseButton::Left)) {
            FollowStep::Wait => follow.age += 1,
            FollowStep::Start => {
                window.start_drag_move();
                commands.entity(entity).remove::<FollowMouse>();
            }
            FollowStep::Cancel => {
                commands.entity(entity).remove::<FollowMouse>();
            }
        }
    }
}

/// Estado de execução: que entidades existem para cada janela do layout.
#[derive(Resource, Default, Debug)]
pub struct DetachedRuntime {
    slots: [Option<Slot>; MAX_WINDOWS],
    /// Tentativas de ler os monitores antes de desistir de conferir a posição.
    monitor_wait: u32,
    restored: bool,
    /// Janela nascida ao arrastar com o botão ainda pressionado: o SO continua o arrasto.
    follow: Option<u32>,
}

#[derive(Debug)]
struct Slot {
    window_id: u32,
    window: Entity,
    camera: Entity,
    /// A janela do SO já apareceu como entidade ao menos uma vez. Os `spawn` são adiados: no
    /// quadro em que o slot nasce a entidade ainda não existe e isso não é "janela fechada"
    /// (#201: a câmera era destruída e o grupo voltava ao dock no mesmo quadro).
    seen: bool,
    /// Fontes e estilos já instalados no contexto egui desta janela.
    apply: crate::ui_theme::ApplyState,
}

pub struct DetachedPlugin;

impl Plugin for DetachedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DetachedRuntime>();
        for slot in 0..MAX_WINDOWS {
            app.add_systems(DetachedPass(slot), move |world: &mut World| {
                detached_ui(world, slot)
            });
        }
        app.add_systems(
            Update,
            (
                restore_on_open,
                note_cursor_left,
                detach_demo,
                open_dragged_out_window,
                handle_close_requests,
                sync_windows,
                follow_mouse,
                track_window_geometry,
            )
                .chain(),
        );
    }
}

/// Evidência no aplicativo de verdade (`--detach-demo button|drag`, sem captura interna):
/// depois de alguns quadros, destaca o Painel Capturadas como a Ação "Destacar em janela"
/// (`button`) ou como o arrasto para fora (`drag`, pedido de saída no meio da janela).
fn detach_demo(
    mut frames: Local<u32>,
    mut dock: ResMut<Dock>,
    mut new_game: ResMut<crate::new_game::NewGameDialog>,
    primary: Query<&Window, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    mut commands: Commands,
) {
    let Some(mode) = crate::capture::option_value("--detach-demo") else {
        return;
    };
    *frames += 1;
    new_game.open = false;
    if *frames == 480
        && let Some(dir) = crate::capture::option_value("--detach-shots")
    {
        // Captura do próprio Bevy de cada janela do SO (independe do compositor).
        for (i, (entity, _)) in windows.iter().enumerate() {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::window(entity))
                .observe(bevy::render::view::screenshot::save_to_disk(format!(
                    "{dir}/janela-{i}.png"
                )));
        }
    }
    if *frames != 240 {
        return;
    }
    let id = crate::dock::ids::CAPTURED;
    dock.layout.set_visible(id, true);
    if mode == "drag" {
        let size = primary.single().map_or(egui::vec2(1280.0, 720.0), |w| {
            egui::vec2(w.width(), w.height())
        });
        dock.request_exit(PanelId::new(id), egui::pos2(size.x - 4.0, size.y * 0.5));
    } else {
        dock.layout.detach_panel_to_window(id, DEFAULT_WINDOW_SIZE);
    }
}

/// O Bevy avisou que o cursor saiu da janela principal: se há um arrasto de Painel em curso,
/// ele vira janela destacada (#195).
fn note_cursor_left(
    mut left: MessageReader<CursorLeft>,
    primary: Query<(), With<PrimaryWindow>>,
    mut dock: ResMut<Dock>,
) {
    for event in left.read() {
        if primary.get(event.window).is_ok() {
            dock.note_cursor_left();
        }
    }
}

/// Abre a janela destacada do Painel arrastado para fora da janela principal, na posição
/// por onde ele saiu (#195).
fn open_dragged_out_window(
    mut dock: ResMut<Dock>,
    mut runtime: ResMut<DetachedRuntime>,
    buttons: Res<ButtonInput<MouseButton>>,
    primary: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
) {
    let Some(request) = dock.take_exit_request() else {
        return;
    };
    let Some(window) = dock
        .layout
        .detach_panel_to_window(&request.panel.0, request.size)
    else {
        return;
    };
    if buttons.pressed(MouseButton::Left) {
        runtime.follow = Some(window);
    }
    let rects = monitors_of(&monitors);
    if let Ok(main) = primary.single()
        && let WindowPosition::At(p) = main.position
    {
        let pos = exit_window_pos([p.x, p.y], request.at, request.ppp, request.size, &rects);
        dock.layout.set_window_rect(
            window,
            Some(pos),
            request.size,
            monitor_index_at(pos, &rects),
        );
    }
}

fn monitors_of(monitors: &Query<&Monitor>) -> Vec<MonitorRect> {
    monitors
        .iter()
        .map(|m| MonitorRect {
            pos: [m.physical_position.x, m.physical_position.y],
            size: [m.physical_width, m.physical_height],
        })
        .collect()
}

/// Uma vez, ao abrir: janelas que não cabem em nenhum monitor voltam ao dock principal.
fn restore_on_open(
    mut runtime: ResMut<DetachedRuntime>,
    mut dock: ResMut<Dock>,
    monitors: Query<&Monitor>,
) {
    if runtime.restored {
        return;
    }
    if dock.layout.windows.is_empty() {
        runtime.restored = true;
        return;
    }
    let rects = monitors_of(&monitors);
    if rects.is_empty() {
        // Os monitores aparecem nos primeiros quadros; sem eles, a posição vale como está.
        runtime.monitor_wait += 1;
        if runtime.monitor_wait < 120 {
            return;
        }
    }
    runtime.restored = true;
    restore_windows(&mut dock.layout, &rects);
}

/// Fechar a janela do SO devolve o grupo ao dock principal (o layout é que manda; o Bevy
/// fecha a janela depois).
fn handle_close_requests(
    mut requests: MessageReader<WindowCloseRequested>,
    windows: Query<&DetachedWindowMarker>,
    mut dock: ResMut<Dock>,
) {
    for request in requests.read() {
        if let Ok(marker) = windows.get(request.window)
            && dock.layout.dock_window_back(marker.window_id)
        {}
    }
}

/// Mantém uma `Window` + câmera de UI por janela do layout, e desfaz as que saíram dele.
#[allow(clippy::too_many_arguments)]
fn sync_windows(
    mut commands: Commands,
    mut runtime: ResMut<DetachedRuntime>,
    mut dock: ResMut<Dock>,
    primary: Query<(), With<PrimaryWindow>>,
    main_window: Query<&Window, With<PrimaryWindow>>,
    existing: Query<(), With<Window>>,
    registry: Res<PanelRegistry>,
    i18n: Res<I18n>,
    monitors: Query<&Monitor>,
) {
    if !runtime.restored {
        return;
    }
    let primary_alive = !primary.is_empty();
    // Janelas do layout sem entidade: abre.
    let ids: Vec<u32> = dock.layout.windows.iter().map(|w| w.id).collect();
    for mut window in dock.layout.windows.clone() {
        if runtime
            .slots
            .iter()
            .flatten()
            .any(|s| s.window_id == window.id)
        {
            continue;
        }
        let Some(free) = runtime.slots.iter().position(Option::is_none) else {
            continue;
        };
        let title = window_title(&window, &registry, &i18n);
        let rects = monitors_of(&monitors);
        if window.pos.is_none()
            && let Ok(main) = main_window.single()
            && let WindowPosition::At(p) = main.position
        {
            // Aberta pela Ação: perto da janela principal (o Wayland decide por conta própria).
            let at = egui::pos2(main.width() * 0.5, main.height() * 0.3);
            let ppp = main.scale_factor();
            let pos = exit_window_pos([p.x, p.y], at, ppp, window.size, &rects);
            window.pos = Some(pos);
            dock.layout.set_window_rect(
                window.id,
                Some(pos),
                window.size,
                monitor_index_at(pos, &rects),
            );
        }
        let (window_entity, camera_entity) =
            spawn_detached_window(&mut commands, free, &window, title, &rects);
        if runtime.follow == Some(window.id) {
            runtime.follow = None;
            commands
                .entity(window_entity)
                .insert(FollowMouse::default());
        }
        runtime.slots[free] = Some(Slot {
            window_id: window.id,
            window: window_entity,
            camera: camera_entity,
            seen: false,
            apply: default(),
        });
    }
    // Entidades sem janela no layout (encaixada) ou cuja janela do SO sumiu.
    for slot in &mut runtime.slots {
        let Some(s) = slot.as_ref() else { continue };
        let in_layout = ids.contains(&s.window_id);
        let exists = existing.get(s.window).is_ok();
        if exists && let Some(s) = slot.as_mut() {
            s.seen = true;
        }
        let Some(s) = slot.as_ref() else { continue };
        // Ainda não aplicado pelo `Commands` (slot recém-criado): espera o próximo quadro.
        let alive = exists || !s.seen && in_layout;
        if in_layout && !exists && s.seen && primary_alive {
            // O usuário fechou a janela por fora do pedido de fechamento: encaixa.
            dock.layout.dock_window_back(s.window_id);
        }
        if !in_layout || !alive {
            // Mesmo recém-criada (spawn adiado) a janela some junto com o slot.
            commands.entity(s.window).try_despawn();
            commands.entity(s.camera).try_despawn();
            *slot = None;
        }
    }
}

/// Título da janela do SO: os Painéis do grupo.
fn window_title(window: &DetachedWindow, registry: &PanelRegistry, i18n: &I18n) -> String {
    let names: Vec<String> = window
        .pane_ids()
        .iter()
        .map(|id| {
            registry
                .get(id)
                .map_or_else(|| id.clone(), |d| i18n.t(d.label_key))
        })
        .collect();
    format!("{} — {}", i18n.t("app-title"), names.join(", "))
}

/// Cria a `Window` do SO e a câmera de UI dela. A câmera segue a regra de câmeras (CLAUDE.md,
/// issues #82, #142 e #156): sem `Msaa` nem `Hdr` próprios (`presentation::apply_quality` os
/// aplica a todas as câmeras) e `Tonemapping::None`.
pub fn spawn_detached_window(
    commands: &mut Commands,
    slot: usize,
    window: &DetachedWindow,
    title: String,
    monitors: &[MonitorRect],
) -> (Entity, Entity) {
    let position = match window.pos {
        Some(p) if monitors.is_empty() || reachable(p, monitors) => {
            WindowPosition::At(IVec2::new(p[0], p[1]))
        }
        _ => WindowPosition::Automatic,
    };
    let window_entity = commands
        .spawn((
            Window {
                title,
                resolution: bevy::window::WindowResolution::new(
                    window.size[0].round().max(1.0) as u32,
                    window.size[1].round().max(1.0) as u32,
                ),
                position,
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            },
            DetachedWindowMarker {
                window_id: window.id,
            },
        ))
        .id();
    let camera_entity = commands
        .spawn((
            Camera2d,
            DetachedCamera,
            EguiSchedule::new(DetachedPass(slot)),
            RenderTarget::Window(WindowRef::Entity(window_entity)),
            // Sem tonemapping nas câmeras 2D: ver `board2d::spawn_camera` (issue #156).
            Tonemapping::None,
            // A camada vazia impede que ela desenhe a cena.
            RenderLayers::layer(31),
            // A janela só tem a UI: limpa a cada quadro (a câmera da janela principal
            // compõe sobre a cena e por isso não limpa).
            Camera {
                order: 100,
                ..default()
            },
            // MSAA e HDR iguais aos das outras câmeras: `presentation::apply_quality`.
        ))
        .id();
    (window_entity, camera_entity)
}

/// Grava no layout onde o usuário deixou cada janela (posição, tamanho e monitor).
fn track_window_geometry(
    mut dock: ResMut<Dock>,
    windows: Query<(&Window, &DetachedWindowMarker)>,
    monitors: Query<&Monitor>,
) {
    let rects = monitors_of(&monitors);
    for (window, marker) in &windows {
        let pos = match window.position {
            WindowPosition::At(p) => Some([p.x, p.y]),
            _ => None,
        };
        let size = [window.resolution.width(), window.resolution.height()];
        let Some(current) = dock
            .layout
            .windows
            .iter()
            .find(|w| w.id == marker.window_id)
        else {
            continue;
        };
        let monitor = pos.and_then(|p| monitor_index_at(p, &rects));
        let moved = pos.is_some() && pos != current.pos
            || (size[0] - current.size[0]).abs() > 0.5
            || (size[1] - current.size[1]).abs() > 0.5
            || (pos.is_some() && monitor != current.monitor);
        if moved {
            let pos = pos.or(current.pos);
            dock.layout
                .set_window_rect(marker.window_id, pos, size, monitor);
        }
    }
}

/// Passe do egui de uma janela destacada: desenha a árvore de abas dela com os mesmos
/// Painéis do dock, lendo os mesmos recursos (ADR-0001 inalterado).
fn detached_ui(world: &mut World, slot: usize) {
    let Some(window_id) = world
        .resource::<DetachedRuntime>()
        .slots
        .get(slot)
        .and_then(Option::as_ref)
        .map(|s| (s.window_id, s.camera))
    else {
        return;
    };
    let (window_id, camera) = window_id;
    let Some(ctx) = world
        .get::<EguiContext>(camera)
        .map(|c| c.clone().get_mut().clone())
    else {
        return;
    };
    // Fontes e estilos do contexto desta janela (o contexto principal tem os dele).
    let appearance = *world.resource::<crate::ui_theme::Appearance>();
    let system_dark = None;
    let mut state = world.resource_mut::<DetachedRuntime>().slots[slot]
        .as_mut()
        .map(|s| std::mem::take(&mut s.apply))
        .unwrap_or_default();
    let first = !state.fonts_requested;
    crate::ui_theme::apply_to_context(&ctx, &appearance, system_dark, &mut state);
    if let Some(s) = world.resource_mut::<DetachedRuntime>().slots[slot].as_mut() {
        s.apply = state;
    }
    if first {
        // As fontes só valem no passe seguinte.
        return;
    }
    world.resource_scope(|world, mut dock: Mut<Dock>| {
        world.resource_scope(|world, registry: Mut<PanelRegistry>| {
            world.resource_scope(|world, i18n: Mut<I18n>| {
                let mut behavior = DockBehavior::new(&i18n, &registry, world);
                behavior.window = Some(window_id);
                if let Some(window) = dock.layout.windows.iter_mut().find(|w| w.id == window_id) {
                    draw_window(&ctx, window, &mut behavior);
                }
                let actions = std::mem::take(&mut behavior.actions);
                for action in actions {
                    apply_window_action(&mut dock.layout, action, window_id);
                }
                if dock.layout.dock_back_empty_windows() {}
            });
        });
    });
}

fn apply_window_action(layout: &mut DockLayout, action: DockAction, window: u32) {
    match action {
        DockAction::DockWindow(w) => {
            layout.dock_window_back(w);
        }
        DockAction::DockBack(id) | DockAction::Toggle(id) => {
            layout.dock_panel_from_window(&id.0);
        }
        DockAction::Close(id) => layout.set_visible(&id.0, false),
        // Destacar de dentro de uma janela do SO não se aplica; flutuar também não.
        DockAction::Detach(_)
        | DockAction::DetachWindow(_)
        | DockAction::DetachGroup(_)
        | DockAction::BeginTileDrag(_)
        | DockAction::FloatAt(..) => {
            let _ = window;
        }
    }
}

/// Desenha a janela: moldura e árvore de abas ocupando a área toda.
fn draw_window(ctx: &egui::Context, window: &mut DetachedWindow, behavior: &mut DockBehavior<'_>) {
    let screen = ctx.content_rect();
    let id = egui::Id::new(("chess-studio-detached", window.id));
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE.fill(ui.visuals().panel_fill);
            frame.show(ui, |ui| {
                ui.set_min_size(screen.size());
                ui.set_max_size(screen.size());
                window.tree.ui(behavior, ui);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::ids;
    use crate::i18n::I18n;

    fn group_of(layout: &DockLayout, id: &str) -> Vec<String> {
        let tile = layout.tile_of(id).unwrap();
        let parent = layout.tree.tiles.parent_of(tile).unwrap();
        match layout.tree.tiles.get(parent) {
            Some(Tile::Container(c)) => c
                .children()
                .filter_map(|t| match layout.tree.tiles.get(*t) {
                    Some(Tile::Pane(p)) => Some(p.0.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn detaching_a_group_moves_all_its_tabs_to_one_window() {
        let mut layout = DockLayout::default();
        let group = group_of(&layout, ids::NOTATION);
        assert!(group.len() >= 2);
        let id = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert_eq!(layout.windows.len(), 1);
        assert_eq!(layout.windows[0].id, id);
        let mut in_window = layout.windows[0].pane_ids();
        let mut expected = group.clone();
        in_window.sort();
        expected.sort();
        assert_eq!(in_window, expected);
        for panel in &group {
            assert!(layout.tile_of(panel).is_none(), "{panel} saiu do dock");
            assert!(layout.window_tile_of(panel).is_some());
            assert!(layout.is_visible(panel));
        }
        // Os outros grupos continuam no dock.
        assert!(layout.is_visible(ids::CLOCK));
    }

    #[test]
    fn detaching_a_single_panel_leaves_its_siblings_docked() {
        let mut layout = DockLayout::default();
        layout
            .detach_panel_to_window(ids::BOARD2D, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert_eq!(layout.windows[0].pane_ids(), vec![ids::BOARD2D]);
        assert!(layout.tile_of(ids::CLOCK).is_some());
        assert!(layout.tile_of(ids::BOARD2D).is_none());
    }

    #[test]
    fn docking_a_window_back_returns_every_tab_to_the_main_dock() {
        let mut layout = DockLayout::default();
        let before = {
            let mut v = layout.pane_ids();
            v.sort();
            v
        };
        let id = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert!(layout.dock_window_back(id));
        assert!(layout.windows.is_empty());
        let mut after = layout.pane_ids();
        after.sort();
        assert_eq!(after, before, "nenhum Painel perdido ou duplicado");
        // O grupo volta junto (um grupo de abas novo).
        let group = group_of(&layout, ids::NOTATION);
        assert!(group.contains(&ids::STUDY.to_owned()));
        assert!(!layout.dock_window_back(id), "janela inexistente");
    }

    #[test]
    fn a_single_detached_panel_returns_to_its_origin_group() {
        let mut layout = DockLayout::default();
        layout
            .detach_panel_to_window(ids::STUDY, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert!(layout.dock_panel_from_window(ids::STUDY));
        assert!(layout.windows.is_empty());
        assert!(group_of(&layout, ids::STUDY).contains(&ids::NOTATION.to_owned()));
    }

    #[test]
    fn docking_one_tab_out_of_a_window_keeps_the_others_there() {
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert!(layout.dock_panel_from_window(ids::STUDY));
        assert_eq!(layout.windows.len(), 1);
        assert_eq!(layout.windows[0].pane_ids(), vec![ids::NOTATION]);
        assert!(layout.tile_of(ids::STUDY).is_some());
        // A última aba leva a janela junto.
        assert!(layout.dock_panel_from_window(ids::NOTATION));
        assert!(layout.windows.is_empty());
    }

    #[test]
    fn detaching_the_only_group_leaves_an_empty_dock_and_it_comes_back() {
        let mut layout = DockLayout::default();
        // Destaca todos os grupos, um a um.
        let mut ids_seen = Vec::new();
        for id in [ids::CLOCK, ids::NOTATION, ids::AIDS] {
            ids_seen.push(
                layout
                    .detach_group_to_window(id, DEFAULT_WINDOW_SIZE)
                    .unwrap(),
            );
        }
        assert!(!layout.any_visible(), "dock vazio");
        assert!(layout.tree.root().is_none());
        for id in ids_seen {
            assert!(layout.dock_window_back(id));
        }
        assert!(layout.any_visible());
        assert_eq!(
            layout.pane_ids().len(),
            DockLayout::default().pane_ids().len()
        );
    }

    #[test]
    fn the_number_of_windows_is_limited() {
        let mut layout = DockLayout::default();
        for id in [ids::CLOCK, ids::NOTATION, ids::AIDS] {
            layout
                .detach_group_to_window(id, DEFAULT_WINDOW_SIZE)
                .unwrap();
        }
        assert!(
            layout
                .detach_group_to_window("Nada", DEFAULT_WINDOW_SIZE)
                .is_none()
        );
        assert_eq!(layout.windows.len(), 3);
    }

    #[test]
    fn a_floating_panel_can_become_a_window_and_unknown_ones_cannot() {
        let mut layout = DockLayout::default();
        assert!(layout.detach(ids::STUDY, [10.0, 10.0], [300.0, 200.0]));
        assert!(
            layout
                .detach_panel_to_window(ids::STUDY, DEFAULT_WINDOW_SIZE)
                .is_some()
        );
        assert!(!layout.is_floating(ids::STUDY));
        assert!(layout.window_tile_of(ids::STUDY).is_some());
        assert!(
            layout
                .detach_panel_to_window("Nada", DEFAULT_WINDOW_SIZE)
                .is_none()
        );
    }

    #[test]
    fn hidden_tabs_stay_hidden_across_detach_and_dock_back() {
        let mut layout = DockLayout::default();
        layout.set_visible(ids::STUDY, false);
        let id = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        assert!(!layout.is_visible(ids::STUDY));
        assert!(layout.is_visible(ids::NOTATION));
        layout.dock_window_back(id);
        assert!(!layout.is_visible(ids::STUDY));
        assert!(layout.is_visible(ids::NOTATION));
    }

    #[test]
    fn a_window_whose_tabs_are_all_closed_goes_back_to_the_dock_hidden() {
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        layout.set_visible(ids::NOTATION, false);
        assert!(
            !layout.dock_back_empty_windows(),
            "ainda há a aba Anotações"
        );
        layout.set_visible(ids::STUDY, false);
        assert!(layout.dock_back_empty_windows());
        assert!(layout.windows.is_empty());
        assert!(!layout.is_visible(ids::NOTATION));
        assert!(layout.tile_of(ids::NOTATION).is_some());
    }

    #[test]
    fn showing_a_panel_from_a_window_brings_its_tab_to_the_front() {
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        layout.activate(ids::STUDY);
        let (w, tile) = layout.window_tile_of(ids::STUDY).unwrap();
        assert!(layout.windows[w].tree.active_tiles().contains(&tile));
    }

    #[test]
    fn windows_survive_a_json_round_trip_with_geometry_and_monitor() {
        let mut layout = DockLayout::default();
        let id = layout
            .detach_group_to_window(ids::NOTATION, [500.0, 400.0])
            .unwrap();
        layout.set_window_rect(id, Some([2100, 60]), [510.0, 410.0], Some(1));
        let back = DockLayout::from_json(Some(layout.to_json()));
        assert_eq!(back, layout);
        assert_eq!(back.windows[0].pos, Some([2100, 60]));
        assert_eq!(back.windows[0].monitor, Some(1));
        assert_eq!(back.windows[0].size, [510.0, 410.0]);
        assert!(back.window_tile_of(ids::STUDY).is_some());
    }

    #[test]
    fn layouts_saved_before_detached_windows_still_open() {
        let mut json = DockLayout::default().to_json();
        json.as_object_mut().unwrap().remove("windows");
        let back = DockLayout::from_json(Some(json));
        assert!(back.windows.is_empty());
        assert_eq!(back, DockLayout::default());
    }

    #[test]
    fn a_window_of_a_panel_still_in_the_dock_is_dropped_on_load() {
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        let dup = layout.windows[0].clone();
        let mut docked = DockLayout::default();
        docked.windows.push(dup);
        let back = DockLayout::from_json(Some(docked.to_json()));
        assert!(back.windows.is_empty(), "os Painéis continuam no dock");
        assert_eq!(
            back.pane_ids().len(),
            DockLayout::default().pane_ids().len()
        );
    }

    #[test]
    fn unregistered_panels_leave_windows_and_empty_windows_vanish() {
        let mut registry = PanelRegistry::default();
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        for id in layout.pane_ids() {
            if id != ids::STUDY {
                registry.register(crate::dock::PanelDef {
                    id: Box::leak(id.into_boxed_str()),
                    label_key: "dock-panel-notation",
                    access: crate::dock::PanelAccess::Always,
                    ui: |_, _, _| {},
                });
            }
        }
        assert!(layout.reconcile(&registry));
        assert_eq!(layout.windows[0].pane_ids(), vec![ids::NOTATION]);
        assert!(!layout.pane_ids().contains(&ids::STUDY.to_owned()));
    }

    #[derive(Resource, Default)]
    struct Drawn(Vec<String>);

    fn stub(id: &'static str) -> crate::dock::PanelDef {
        crate::dock::PanelDef {
            id,
            label_key: "dock-panel-notation",
            access: crate::dock::PanelAccess::Always,
            ui: |_, _, world| world.resource_mut::<Drawn>().0.push("desenhou".into()),
        }
    }

    /// A janela desenha as abas do grupo e o conteúdo da aba ativa no contexto dela.
    #[test]
    fn a_detached_window_draws_the_active_panel_in_its_own_context() {
        let mut layout = DockLayout::default();
        layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        let mut registry = PanelRegistry::default();
        registry.register(stub(ids::NOTATION));
        registry.register(stub(ids::STUDY));
        let i18n = I18n::pt_br();
        let mut world = World::new();
        world.init_resource::<Drawn>();
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(420.0, 520.0));
        for _ in 0..2 {
            let mut behavior = DockBehavior::new(&i18n, &registry, &mut world);
            behavior.window = Some(layout.windows[0].id);
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let window = &mut layout.windows[0];
            let mut out = ctx.run_ui(input, |_| draw_window(&ctx, window, &mut behavior));
            out.textures_delta.clear();
        }
        assert_eq!(
            world.resource::<Drawn>().0.len(),
            2,
            "uma aba, dois quadros"
        );
    }

    fn two_monitors() -> Vec<MonitorRect> {
        vec![
            MonitorRect {
                pos: [0, 0],
                size: [1920, 1080],
            },
            MonitorRect {
                pos: [1920, 0],
                size: [2560, 1440],
            },
        ]
    }

    #[test]
    fn the_monitor_of_a_window_is_found_by_its_handle() {
        let m = two_monitors();
        assert_eq!(monitor_index_at([100, 100], &m), Some(0));
        assert_eq!(monitor_index_at([2000, 100], &m), Some(1));
        assert_eq!(monitor_index_at([-3000, 100], &m), None);
        // Monitor à esquerda da principal (coordenadas negativas).
        let left = [MonitorRect {
            pos: [-1280, 0],
            size: [1280, 720],
        }];
        assert_eq!(monitor_index_at([-1000, 50], &left), Some(0));
    }

    #[test]
    fn a_window_outside_every_monitor_comes_back_to_the_main_dock_on_load() {
        let mut layout = DockLayout::default();
        let lost = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        let kept = layout
            .detach_group_to_window(ids::AIDS, DEFAULT_WINDOW_SIZE)
            .unwrap();
        layout.set_window_rect(lost, Some([5000, 300]), DEFAULT_WINDOW_SIZE, Some(2));
        layout.set_window_rect(kept, Some([2000, 100]), DEFAULT_WINDOW_SIZE, Some(1));
        assert_eq!(restore_windows(&mut layout, &two_monitors()), 1);
        assert_eq!(layout.windows.len(), 1);
        assert_eq!(layout.windows[0].id, kept);
        assert!(layout.tile_of(ids::NOTATION).is_some(), "voltou ao dock");
        assert!(layout.is_visible(ids::NOTATION));
    }

    #[test]
    fn without_known_monitors_or_a_saved_position_windows_stay_open() {
        let mut layout = DockLayout::default();
        let id = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        layout.set_window_rect(id, Some([9000, 9000]), DEFAULT_WINDOW_SIZE, None);
        assert_eq!(restore_windows(&mut layout, &[]), 0);
        layout.set_window_rect(id, None, DEFAULT_WINDOW_SIZE, None);
        assert_eq!(restore_windows(&mut layout, &two_monitors()), 0);
        assert_eq!(layout.windows.len(), 1);
    }

    #[test]
    fn non_finite_window_geometry_is_sanitized() {
        let mut layout = DockLayout::default();
        let id = layout
            .detach_group_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        layout.set_window_rect(id, None, [f32::NAN, 1.0], None);
        assert_eq!(
            layout.windows[0].size,
            [DEFAULT_WINDOW_SIZE[0], MIN_WINDOW_SIZE[1]]
        );
    }

    fn win() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))
    }

    #[test]
    fn a_drag_inside_the_window_does_not_exit() {
        let p = Some(egui::pos2(600.0, 300.0));
        assert_eq!(drag_exit_point(p, p, win(), false, false, false), None);
        assert_eq!(drag_exit_point(p, p, win(), false, true, false), None);
    }

    #[test]
    fn the_pointer_outside_the_window_exits_where_it_left() {
        let out = Some(egui::pos2(1400.0, 300.0));
        assert_eq!(
            drag_exit_point(out, None, win(), false, false, false),
            Some(egui::pos2(1280.0, 300.0)),
            "ponto de saída preso à borda"
        );
        // Ponteiro sumiu: vale a última posição conhecida.
        let last = Some(egui::pos2(1270.0, 40.0));
        assert_eq!(
            drag_exit_point(None, last, win(), false, false, false),
            last
        );
        assert_eq!(
            drag_exit_point(None, None, win(), false, false, false),
            None
        );
    }

    #[test]
    fn cursor_left_exits_even_with_a_stale_position_inside() {
        let p = Some(egui::pos2(1275.0, 300.0));
        assert_eq!(drag_exit_point(p, p, win(), true, false, false), p);
    }

    #[test]
    fn releasing_touching_the_edge_exits_unless_it_is_a_dock_target() {
        let edge = Some(egui::pos2(1.0, 300.0));
        assert_eq!(drag_exit_point(edge, edge, win(), false, true, false), edge);
        assert_eq!(
            drag_exit_point(edge, edge, win(), false, false, false),
            None
        );
        assert_eq!(drag_exit_point(edge, edge, win(), false, true, true), None);
        let near = Some(egui::pos2(20.0, 300.0));
        assert_eq!(drag_exit_point(near, near, win(), false, true, false), None);
    }

    #[test]
    fn the_new_window_sits_with_its_title_bar_under_the_exit_point() {
        // Janela principal em (100, 50); saiu pela direita em x=1280, y=300; janela 400 de largura.
        let pos = exit_window_pos(
            [100, 50],
            egui::pos2(1280.0, 300.0),
            1.0,
            [400.0, 500.0],
            &[],
        );
        assert_eq!(pos, [100 + 1280 - 200, 50 + 300 - 16]);
        // Em tela com escala 2, tudo em pixels físicos.
        let pos = exit_window_pos([0, 0], egui::pos2(100.0, 100.0), 2.0, [400.0, 500.0], &[]);
        assert_eq!(pos, [200 - 400, 200 - 32]);
    }

    #[test]
    fn the_new_window_is_clamped_to_the_monitor_of_the_pointer() {
        let monitors = two_monitors();
        // Saiu pela direita do monitor 0 (1920 de largura) perto do canto: a janela cabe no
        // monitor 1? O ponteiro absoluto 1900 ainda está no monitor 0.
        let pos = exit_window_pos(
            [0, 0],
            egui::pos2(1900.0, 1000.0),
            1.0,
            [420.0, 520.0],
            &monitors,
        );
        let m = monitors[0];
        assert!(pos[0] + 420 <= m.pos[0] + m.size[0] as i32);
        assert!(pos[1] + 520 <= m.pos[1] + m.size[1] as i32);
        assert!(pos[0] >= m.pos[0] && pos[1] >= m.pos[1]);
        assert!(reachable(pos, &monitors));
        // Saiu para além de todos os monitores: cai no primeiro.
        let far = exit_window_pos(
            [0, 0],
            egui::pos2(9000.0, 9000.0),
            1.0,
            [420.0, 520.0],
            &monitors,
        );
        assert!(reachable(far, &monitors));
    }

    // ---- #201: a janela destacada nasce com o Painel e acompanha o mouse -----------------

    fn sync_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(I18n::pt_br())
            .init_resource::<PanelRegistry>()
            .init_resource::<Dock>()
            .insert_resource(DetachedRuntime {
                restored: true,
                ..default()
            })
            .add_systems(
                Update,
                (open_dragged_out_window, sync_windows, follow_mouse).chain(),
            );
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app
    }

    fn cameras(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<DetachedCamera>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn a_detached_window_keeps_its_camera_and_stays_out_of_the_dock() {
        let mut app = sync_app();
        {
            let mut dock = app.world_mut().resource_mut::<Dock>();
            dock.layout.set_visible(ids::CAPTURED, true);
            dock.layout
                .detach_panel_to_window(ids::CAPTURED, DEFAULT_WINDOW_SIZE)
                .unwrap();
        }
        for _ in 0..5 {
            app.update();
        }
        let dock = app.world().resource::<Dock>();
        assert_eq!(
            dock.layout.windows.len(),
            1,
            "a janela não pode voltar ao dock"
        );
        assert!(dock.layout.tile_of(ids::CAPTURED).is_none());
        // Sem a correção a câmera (que tem o contexto egui) era destruída no mesmo quadro
        // em que nascia e a janela do SO ficava vazia.
        assert_eq!(cameras(&mut app), 1);
        let windows = app
            .world_mut()
            .query_filtered::<(), With<DetachedWindowMarker>>()
            .iter(app.world())
            .count();
        assert_eq!(windows, 1);
    }

    #[test]
    fn the_context_of_a_detached_window_runs_its_own_pass() {
        let mut app = sync_app();
        app.world_mut()
            .resource_mut::<Dock>()
            .layout
            .detach_panel_to_window(ids::NOTATION, DEFAULT_WINDOW_SIZE)
            .unwrap();
        app.update();
        app.update();
        let (schedule, _) = app
            .world_mut()
            .query::<(&EguiSchedule, &DetachedCamera)>()
            .single(app.world())
            .map(|(s, c)| (s.0, c))
            .unwrap();
        assert_eq!(schedule, DetachedPass(0).intern());
    }

    #[test]
    fn dragging_out_with_the_button_down_follows_the_mouse() {
        let mut app = sync_app();
        app.world_mut()
            .resource_mut::<Dock>()
            .layout
            .set_visible(ids::CAPTURED, true);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<Dock>()
            .request_exit(PanelId::new(ids::CAPTURED), egui::pos2(10.0, 10.0));
        app.update();
        let follow = app
            .world_mut()
            .query::<&FollowMouse>()
            .iter(app.world())
            .count();
        assert_eq!(follow, 1, "a janela nova pede o arrasto ao sistema");
        for _ in 0..4 {
            app.update();
        }
        let follow = app
            .world_mut()
            .query::<&FollowMouse>()
            .iter(app.world())
            .count();
        assert_eq!(follow, 0, "o pedido de arrasto é feito uma vez");
    }

    #[test]
    fn dropping_with_the_button_up_does_not_follow() {
        let mut app = sync_app();
        app.world_mut()
            .resource_mut::<Dock>()
            .layout
            .set_visible(ids::CAPTURED, true);
        app.world_mut()
            .resource_mut::<Dock>()
            .request_exit(PanelId::new(ids::CAPTURED), egui::pos2(10.0, 10.0));
        app.update();
        let follow = app
            .world_mut()
            .query::<&FollowMouse>()
            .iter(app.world())
            .count();
        assert_eq!(follow, 0);
        assert_eq!(app.world().resource::<Dock>().layout.windows.len(), 1);
    }

    #[test]
    fn follow_waits_for_the_os_window_and_cancels_on_release() {
        assert_eq!(follow_step(0, true), FollowStep::Wait);
        assert_eq!(follow_step(1, true), FollowStep::Wait);
        assert_eq!(follow_step(2, true), FollowStep::Start);
        assert_eq!(follow_step(0, false), FollowStep::Cancel);
        assert_eq!(follow_step(9, false), FollowStep::Cancel);
    }
}
