//! Cromo dos Painéis (DS-6, issue #165): aba, ferramentas do cabeçalho, guia de encaixe e
//! posição das janelas flutuantes, conforme `docs/design/componentes/{PanelTabs,FloatingPanel,
//! DockGuide}.md`. Só Apresentação (ADR-0001): os botões pedem `DockAction`s, que o dock
//! aplica depois de desenhar. As funções puras de posição são testadas aqui.

use crate::dock::{DockAction, PanelId, ids};
use crate::i18n::I18n;
use crate::ui_theme::{Palette, Tokens, icon_font};
use bevy_egui::egui;
use egui_phosphor::regular as icon;

/// Traz uma janela flutuante de volta para dentro da tela: inteira quando cabe; se for maior
/// que a tela, o canto superior esquerdo fica no canto da tela (o cabeçalho segue à mão).
pub fn clamp_floating(pos: [f32; 2], size: [f32; 2], screen: egui::Rect) -> [f32; 2] {
    let axis = |p: f32, extent: f32, lo: f32, hi: f32| {
        let p = if p.is_finite() { p } else { lo };
        p.clamp(lo, (hi - extent).max(lo))
    };
    [
        axis(pos[0], size[0], screen.min.x, screen.max.x),
        axis(pos[1], size[1], screen.min.y, screen.max.y),
    ]
}

/// Onde nasce a janela de uma aba solta fora de qualquer guia: o cabeçalho sob o ponteiro,
/// sempre dentro da tela.
pub fn float_pos_at_drop(pointer: egui::Pos2, size: [f32; 2], screen: egui::Rect) -> [f32; 2] {
    clamp_floating([pointer.x - size[0] * 0.5, pointer.y - 16.0], size, screen)
}

/// Soltar a aba fora da região do dock (e portanto fora de todo guia) deixa o Painel flutuante.
pub fn dropped_outside(pointer: Option<egui::Pos2>, dock: egui::Rect) -> bool {
    pointer.is_some_and(|p| !dock.contains(p))
}

/// Onde está o ponteiro durante o arrasto do cabeçalho de um Painel flutuante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatDragZone {
    /// Sobre o dock com Painéis: o arrasto passa ao `egui_tiles` (guias nativas).
    Dock,
    /// Sobre a borda direita com o dock vazio: a zona de soltura que recria o dock.
    EmptyDock,
    /// Fora: a janela segue o ponteiro e, solta, continua flutuante.
    Free,
}

/// Classifica o ponteiro em relação à região do dock (borda direita). `dock_has_panels`
/// diz se há algum Painel encaixado visível; sem ele a região vira a zona de soltura.
pub fn float_drag_zone(
    pointer: Option<egui::Pos2>,
    dock: egui::Rect,
    dock_has_panels: bool,
) -> FloatDragZone {
    match pointer {
        Some(p) if dock.contains(p) && dock_has_panels => FloatDragZone::Dock,
        Some(p) if dock.contains(p) => FloatDragZone::EmptyDock,
        _ => FloatDragZone::Free,
    }
}

/// Ícone Phosphor do Painel, para a aba (PanelTabs).
pub fn panel_icon(id: &str) -> &'static str {
    match id {
        ids::NOTATION => icon::LIST_NUMBERS,
        ids::CLOCK => icon::CLOCK,
        ids::CAPTURED => icon::STACK,
        ids::STUDY => icon::NOTE_PENCIL,
        ids::AIDS => icon::LIGHTBULB,
        ids::REVIEW => icon::MAGNIFYING_GLASS,
        ids::ARBITRATION => icon::GAVEL,
        ids::TRAINING => icon::BARBELL,
        ids::ANALYSIS => icon::CHART_LINE_UP,
        ids::LIBRARY => icon::BOOKS,
        ids::OPENINGS => icon::BOOK_OPEN_TEXT,
        ids::EXPLORER => icon::COMPASS,
        ids::SCORESHEET => icon::SCROLL,
        ids::BOARD2D => icon::SQUARES_FOUR,
        _ => icon::APP_WINDOW,
    }
}

pub fn palette_of(visuals: &egui::Visuals) -> &'static Palette {
    Palette::from_visuals(visuals)
}

/// Medida do design system em pixels (com um valor de reserva se o token faltar).
pub fn token(name: &str, fallback: f32) -> f32 {
    Tokens::embedded().length(name).unwrap_or(fallback)
}

pub fn radius(name: &str) -> egui::CornerRadius {
    egui::CornerRadius::same(token(name, 4.0).round().clamp(0.0, 255.0) as u8)
}

/// Botão de ferramenta do cabeçalho (24x24, ícone Phosphor de 16px).
fn tool_button(ui: &mut egui::Ui, glyph: &str, tip: &str) -> egui::Response {
    let p = palette_of(ui.visuals());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, radius("radius-s"), p.surface_3);
        }
        let ink = if response.hovered() {
            p.ink
        } else {
            p.ink_muted
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            icon_font(ui.ctx(), 16.0),
            ink,
        );
    }
    response.on_hover_text(tip)
}

/// Itens do menu "⋯" (e do menu de contexto da aba): Flutuar, Destacar em janela, Encaixar
/// (só fora do dock) e Fechar Painel.
pub fn panel_menu(
    ui: &mut egui::Ui,
    i18n: &I18n,
    actions: &mut Vec<DockAction>,
    id: &PanelId,
    floating: bool,
) {
    if floating {
        if ui.button(i18n.t("dock-dock-back")).clicked() {
            actions.push(DockAction::DockBack(id.clone()));
            ui.close();
        }
    } else {
        if ui.button(i18n.t("dock-float")).clicked() {
            actions.push(DockAction::Detach(id.clone()));
            ui.close();
        }
        if ui.button(i18n.t("dock-detach")).clicked() {
            actions.push(DockAction::DetachWindow(id.clone()));
            ui.close();
        }
        if ui.button(i18n.t("dock-detach-group")).clicked() {
            actions.push(DockAction::DetachGroup(id.clone()));
            ui.close();
        }
    }
    ui.separator();
    if ui.button(i18n.t("dock-close")).clicked() {
        actions.push(DockAction::Close(id.clone()));
        ui.close();
    }
}

/// Itens do menu "⋯" (e do menu de contexto da aba) de um Painel numa janela do sistema:
/// Encaixar (só esta aba), Encaixar grupo (a janela toda) e Fechar Painel.
pub fn window_menu(
    ui: &mut egui::Ui,
    i18n: &I18n,
    actions: &mut Vec<DockAction>,
    id: &PanelId,
    window: u32,
) {
    if ui.button(i18n.t("dock-dock-back")).clicked() {
        actions.push(DockAction::DockBack(id.clone()));
        ui.close();
    }
    if ui.button(i18n.t("dock-dock-group")).clicked() {
        actions.push(DockAction::DockWindow(window));
        ui.close();
    }
    ui.separator();
    if ui.button(i18n.t("dock-close")).clicked() {
        actions.push(DockAction::Close(id.clone()));
        ui.close();
    }
}

/// Ferramentas do cabeçalho de um grupo numa janela do sistema: "⋯" e Encaixar grupo.
pub fn window_header_tools(
    ui: &mut egui::Ui,
    i18n: &I18n,
    actions: &mut Vec<DockAction>,
    id: &PanelId,
    window: u32,
) {
    ui.spacing_mut().item_spacing.x = 2.0;
    let more = tool_button(ui, icon::DOTS_THREE, &i18n.t("dock-more"));
    egui::Popup::menu(&more).show(|ui| window_menu(ui, i18n, actions, id, window));
    if tool_button(ui, icon::ARROW_BEND_DOWN_LEFT, &i18n.t("dock-dock-group")).clicked() {
        actions.push(DockAction::DockWindow(window));
    }
}

/// Ferramentas do lado direito do cabeçalho: "⋯", Destacar e Flutuar (ou Encaixar). O layout
/// do `Ui` é da direita para a esquerda.
pub fn header_tools(
    ui: &mut egui::Ui,
    i18n: &I18n,
    actions: &mut Vec<DockAction>,
    id: &PanelId,
    floating: bool,
) {
    ui.spacing_mut().item_spacing.x = 2.0;
    let more = tool_button(ui, icon::DOTS_THREE, &i18n.t("dock-more"));
    egui::Popup::menu(&more).show(|ui| panel_menu(ui, i18n, actions, id, floating));
    if floating {
        if tool_button(ui, icon::ARROW_BEND_DOWN_LEFT, &i18n.t("dock-dock-back")).clicked() {
            actions.push(DockAction::DockBack(id.clone()));
        }
    } else {
        if tool_button(ui, icon::ARROW_SQUARE_OUT, &i18n.t("dock-detach")).clicked() {
            actions.push(DockAction::DetachWindow(id.clone()));
        }
        if tool_button(ui, icon::BROWSERS, &i18n.t("dock-float")).clicked() {
            actions.push(DockAction::Detach(id.clone()));
        }
    }
}

/// Lado do botão X da aba, em pontos.
const CLOSE_SIZE: f32 = 16.0;

/// O X da aba aparece na aba ativa e ao apontar para a aba (como no VS Code); a aba em
/// arrasto não mostra nada.
pub fn close_visible(active: bool, hovered: bool, dragging: bool) -> bool {
    !dragging && (active || hovered)
}

/// Retângulo do X dentro da aba: encostado à direita, centralizado na altura.
pub fn close_rect(tab: egui::Rect) -> egui::Rect {
    let pad = 4.0;
    egui::Rect::from_center_size(
        egui::pos2(tab.right() - pad - CLOSE_SIZE * 0.5, tab.center().y),
        egui::vec2(CLOSE_SIZE, CLOSE_SIZE),
    )
}

/// Aba de Painel (PanelTabs): ativa em `surface-1` com traço `accent` de 2px no alto;
/// inativas em `ink-muted`; ícone + nome. `hidden` deixa só o vão (aba em arrasto). Com
/// `close_tip` ganha o botão X de fechar e devolve se ele foi clicado (o chamador pede
/// `DockAction::Close`). O vão do X é sempre reservado: a largura não pula ao apontar.
#[allow(clippy::too_many_arguments)]
pub fn tab_button_closable(
    ui: &mut egui::Ui,
    id: egui::Id,
    glyph: Option<&str>,
    title: &str,
    active: bool,
    hidden: bool,
    sense: egui::Sense,
    close_tip: Option<&str>,
) -> (egui::Response, bool) {
    tab_button_fitted(ui, id, glyph, title, active, hidden, sense, close_tip, None)
}

/// Largura mínima de uma aba inativa encolhida (`min-width` do PanelTabs).
pub const TAB_MIN_WIDTH: f32 = 56.0;

/// Largura natural de uma aba: ícone, nome, respiros e o vão do X.
pub fn tab_natural_width(
    ui: &egui::Ui,
    glyph: Option<&str>,
    title: &str,
    active: bool,
    closable: bool,
) -> f32 {
    let galley = tab_galley(ui, glyph, title, active, None);
    galley.size().x + 2.0 * token("space-2", 8.0) + if closable { CLOSE_SIZE + 4.0 } else { 0.0 }
}

fn tab_galley(
    ui: &egui::Ui,
    glyph: Option<&str>,
    title: &str,
    active: bool,
    max_text_width: Option<f32>,
) -> std::sync::Arc<egui::Galley> {
    let size = Tokens::embedded().text("body").map_or(14.0, |t| t.size);
    let font = if active {
        crate::ui_theme::strong_font(ui.ctx(), size)
    } else {
        egui::FontId::proportional(size)
    };
    let mut job =
        crate::ui_theme::icon_job(ui.ctx(), glyph, title, font, egui::Color32::PLACEHOLDER);
    if let Some(width) = max_text_width {
        job.wrap = egui::text::TextWrapping {
            max_width: width.max(0.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
    }
    ui.painter().layout_job(job)
}

/// Reparte a largura disponível entre as abas de um grupo (PanelTabs): cabendo tudo, cada uma
/// fica com a largura natural; sem espaço, a aba ativa (`active`) mantém a sua e as inativas
/// encolhem por igual (nenhuma abaixo de `min`), cortando o nome com reticências. Se nem assim
/// couber, o `egui_tiles` rola as abas; a ativa e os botões da direita continuam visíveis.
pub fn fit_tab_widths(
    natural: &[f32],
    active: Option<usize>,
    available: f32,
    min: f32,
) -> Vec<f32> {
    if natural.iter().sum::<f32>() <= available {
        return natural.to_vec();
    }
    let fixed = active.and_then(|i| natural.get(i)).copied().unwrap_or(0.0);
    let budget = (available - fixed).max(0.0);
    let inactive = |i: usize| Some(i) != active;
    // Nível comum `level`: cada inativa ocupa `min(natural, level)`; acha o maior que cabe.
    let used = |level: f32| -> f32 {
        natural
            .iter()
            .enumerate()
            .filter(|(i, _)| inactive(*i))
            .map(|(_, w)| w.min(level).max(min.min(*w)))
            .sum()
    };
    let (mut lo, mut hi) = (min, natural.iter().copied().fold(min, f32::max));
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if used(mid) <= budget {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    natural
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if inactive(i) {
                w.min(lo).max(min.min(*w))
            } else {
                *w
            }
        })
        .collect()
}

/// Como [`tab_button_closable`], mas com `max_width` (largura total da aba, incluindo o X):
/// acima disso o nome é cortado com reticências.
#[allow(clippy::too_many_arguments)]
pub fn tab_button_fitted(
    ui: &mut egui::Ui,
    id: egui::Id,
    glyph: Option<&str>,
    title: &str,
    active: bool,
    hidden: bool,
    sense: egui::Sense,
    close_tip: Option<&str>,
    max_width: Option<f32>,
) -> (egui::Response, bool) {
    let p = palette_of(ui.visuals());
    let pad = token("space-2", 8.0);
    let close_slot = if close_tip.is_some() {
        CLOSE_SIZE + 4.0
    } else {
        0.0
    };
    let galley = tab_galley(
        ui,
        glyph,
        title,
        active,
        max_width.map(|w| w - 2.0 * pad - close_slot),
    );
    let (_, rect) = ui.allocate_space(egui::vec2(
        galley.size().x + 2.0 * pad + close_slot,
        ui.available_height(),
    ));
    let response = ui
        .interact(rect, id, sense)
        .on_hover_cursor(egui::CursorIcon::Grab);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled(),
            active,
            galley.text(),
        )
    });
    // O X é criado depois da aba: o egui dá o clique ao widget mais novo sob o ponteiro.
    let mut closed = false;
    let mut close_hovered = false;
    let close_response = close_tip.filter(|_| !hidden).map(|tip| {
        let r = ui.interact(close_rect(rect), id.with("close"), egui::Sense::click());
        close_hovered = r.hovered();
        closed = r.clicked();
        r.on_hover_text(tip)
    });
    if ui.is_rect_visible(rect) && !hidden {
        let hovered = response.hovered() || close_hovered;
        if active {
            ui.painter().rect_filled(rect, 0.0, p.surface_1);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 2.0)),
                0.0,
                p.accent,
            );
        } else if hovered {
            ui.painter().rect_filled(rect, 0.0, p.surface_3);
        }
        let ink = if active || hovered {
            p.ink
        } else {
            p.ink_muted
        };
        let at = egui::Align2::LEFT_CENTER
            .align_size_within_rect(
                galley.size(),
                egui::Rect::from_min_max(
                    rect.min + egui::vec2(pad, 0.0),
                    rect.max - egui::vec2(pad + close_slot, 0.0),
                ),
            )
            .min;
        ui.painter().galley(at, galley, ink);
        if close_response.is_some() && close_visible(active, hovered, false) {
            let x = close_rect(rect);
            if close_hovered {
                ui.painter().rect_filled(x, radius("radius-s"), p.surface_3);
            }
            ui.painter().text(
                x.center(),
                egui::Align2::CENTER_CENTER,
                icon::X,
                icon_font(ui.ctx(), 12.0),
                if close_hovered { p.ink } else { p.ink_muted },
            );
        }
    }
    (response, closed)
}

/// Cabeçalho de um Painel flutuante: a mesma aba (ativa) e as mesmas ferramentas, com
/// "Encaixar" no lugar de Flutuar. Duplo clique na aba devolve o Painel ao dock.
pub fn floating_header(
    ui: &mut egui::Ui,
    i18n: &I18n,
    actions: &mut Vec<DockAction>,
    id: &PanelId,
    title: &str,
) {
    let p = palette_of(ui.visuals());
    let r = token("radius-l", 8.0).round().clamp(0.0, 255.0) as u8;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), token("panel-header-h", 32.0)),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        p.surface_2,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        egui::Stroke::new(1.0, p.border),
    );
    let mut left = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    left.spacing_mut().item_spacing.x = 0.0;
    let (tab, closed) = tab_button_closable(
        &mut left,
        egui::Id::new(("chess-studio-float-tab", &id.0)),
        Some(panel_icon(&id.0)),
        title,
        true,
        false,
        egui::Sense::click(),
        Some(&i18n.t("dock-close")),
    );
    if closed {
        actions.push(DockAction::Close(id.clone()));
    }
    if tab.double_clicked() {
        actions.push(DockAction::DockBack(id.clone()));
    }
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    header_tools(&mut right, i18n, actions, id, true);
}

/// Retângulo tracejado (borda `accent` do guia de encaixe).
pub fn dashed_rect(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    for edge in corners.windows(2) {
        painter.extend(egui::Shape::dashed_line(edge, stroke, 6.0, 4.0));
    }
}

/// Guia de encaixe (DockGuide): área de destino em `accent-subtle` com borda tracejada `accent`.
pub fn paint_guide(painter: &egui::Painter, p: &Palette, rect: egui::Rect) {
    painter.rect_filled(rect, radius("radius-s"), p.accent_subtle);
    dashed_rect(painter, rect.shrink(1.0), egui::Stroke::new(1.5, p.accent));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))
    }

    #[test]
    fn a_position_inside_the_screen_is_left_alone() {
        assert_eq!(
            clamp_floating([100.0, 80.0], [320.0, 240.0], screen()),
            [100.0, 80.0]
        );
    }

    #[test]
    fn an_off_screen_position_comes_back_inside() {
        assert_eq!(
            clamp_floating([-500.0, -40.0], [320.0, 240.0], screen()),
            [0.0, 0.0]
        );
        assert_eq!(
            clamp_floating([5000.0, 5000.0], [320.0, 240.0], screen()),
            [960.0, 480.0]
        );
        // Meio para fora à direita: a janela inteira volta.
        assert_eq!(
            clamp_floating([1200.0, 100.0], [320.0, 240.0], screen()),
            [960.0, 100.0]
        );
    }

    #[test]
    fn a_window_larger_than_the_screen_keeps_its_header_in_the_corner() {
        assert_eq!(
            clamp_floating([300.0, 300.0], [2000.0, 900.0], screen()),
            [0.0, 0.0]
        );
    }

    #[test]
    fn a_non_finite_position_is_clamped_too() {
        let pos = clamp_floating([f32::NAN, f32::INFINITY], [320.0, 240.0], screen());
        assert!(pos[0].is_finite() && pos[1].is_finite());
        assert!(screen().contains(egui::pos2(pos[0], pos[1])));
    }

    #[test]
    fn a_dropped_tab_floats_under_the_pointer_and_inside_the_screen() {
        assert_eq!(
            float_pos_at_drop(egui::pos2(600.0, 300.0), [320.0, 240.0], screen()),
            [440.0, 284.0]
        );
        assert_eq!(
            float_pos_at_drop(egui::pos2(1279.0, 719.0), [320.0, 240.0], screen()),
            [960.0, 480.0]
        );
    }

    #[test]
    fn only_a_drop_outside_the_dock_floats_the_panel() {
        let dock = egui::Rect::from_min_max(egui::pos2(940.0, 0.0), egui::pos2(1280.0, 720.0));
        assert!(dropped_outside(Some(egui::pos2(300.0, 300.0)), dock));
        assert!(!dropped_outside(Some(egui::pos2(1000.0, 300.0)), dock));
        assert!(!dropped_outside(None, dock));
    }

    #[test]
    fn a_floating_header_over_the_dock_hands_the_drag_to_the_tiles() {
        let dock = egui::Rect::from_min_max(egui::pos2(940.0, 0.0), egui::pos2(1280.0, 720.0));
        let at = |x, y| Some(egui::pos2(x, y));
        assert_eq!(
            float_drag_zone(at(1000.0, 300.0), dock, true),
            FloatDragZone::Dock
        );
        assert_eq!(
            float_drag_zone(at(300.0, 300.0), dock, true),
            FloatDragZone::Free
        );
        assert_eq!(float_drag_zone(None, dock, true), FloatDragZone::Free);
    }

    #[test]
    fn an_empty_dock_turns_its_region_into_the_drop_zone() {
        let dock = egui::Rect::from_min_max(egui::pos2(940.0, 0.0), egui::pos2(1280.0, 720.0));
        assert_eq!(
            float_drag_zone(Some(egui::pos2(1000.0, 300.0)), dock, false),
            FloatDragZone::EmptyDock
        );
        assert_eq!(
            float_drag_zone(Some(egui::pos2(100.0, 300.0)), dock, false),
            FloatDragZone::Free
        );
    }

    #[test]
    fn every_native_panel_has_its_own_icon() {
        let ids = [
            ids::NOTATION,
            ids::CLOCK,
            ids::CAPTURED,
            ids::STUDY,
            ids::AIDS,
            ids::REVIEW,
            ids::ARBITRATION,
            ids::TRAINING,
            ids::ANALYSIS,
            ids::LIBRARY,
            ids::OPENINGS,
            ids::EXPLORER,
            ids::SCORESHEET,
            ids::BOARD2D,
        ];
        let mut icons: Vec<&str> = ids.iter().map(|i| panel_icon(i)).collect();
        icons.sort_unstable();
        icons.dedup();
        assert_eq!(icons.len(), ids.len());
        assert_ne!(panel_icon("Outro"), "");
    }

    #[test]
    fn the_close_x_shows_on_the_active_tab_and_on_hover_only() {
        assert!(close_visible(true, false, false));
        assert!(close_visible(false, true, false));
        assert!(!close_visible(false, false, false));
        assert!(!close_visible(true, true, true), "aba em arrasto");
    }

    #[test]
    fn the_close_x_sits_inside_the_right_end_of_the_tab() {
        let tab = egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(120.0, 32.0));
        let x = close_rect(tab);
        assert!(tab.contains_rect(x));
        assert!(x.center().x > tab.center().x);
        assert_eq!(x.center().y, tab.center().y);
    }

    /// Desenha uma aba com X num quadro com o ponteiro em `at`; devolve (retângulo, fechou).
    fn tab_frame(
        ctx: &egui::Context,
        at: Option<egui::Pos2>,
        click: Option<bool>,
        active: bool,
    ) -> (egui::Rect, bool) {
        let mut events = vec![];
        if let Some(pos) = at {
            events.push(egui::Event::PointerMoved(pos));
            if let Some(pressed) = click {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(screen()),
            events,
            ..Default::default()
        };
        let mut out = (egui::Rect::NOTHING, false);
        let mut full = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (r, closed) = tab_button_closable(
                    ui,
                    egui::Id::new("tab"),
                    None,
                    "Estudo",
                    active,
                    false,
                    egui::Sense::click_and_drag(),
                    Some("Fechar"),
                );
                out = (r.rect, closed);
            });
        });
        full.textures_delta.clear();
        out
    }

    #[test]
    fn clicking_the_x_reports_a_close_and_clicking_the_tab_does_not() {
        let ctx = egui::Context::default();
        tab_frame(&ctx, None, None, true);
        let (rect, _) = tab_frame(&ctx, None, None, true);
        let x = close_rect(rect).center();
        tab_frame(&ctx, Some(x), None, true);
        tab_frame(&ctx, Some(x), Some(true), true);
        let (_, closed) = tab_frame(&ctx, Some(x), Some(false), true);
        assert!(closed, "o X fecha");
        let ctx = egui::Context::default();
        tab_frame(&ctx, None, None, true);
        let (rect, _) = tab_frame(&ctx, None, None, true);
        let body = egui::pos2(rect.left() + 10.0, rect.center().y);
        tab_frame(&ctx, Some(body), None, true);
        tab_frame(&ctx, Some(body), Some(true), true);
        let (_, closed) = tab_frame(&ctx, Some(body), Some(false), true);
        assert!(!closed, "o corpo da aba não fecha");
    }

    #[test]
    fn tabs_that_fit_keep_their_natural_width() {
        let w = fit_tab_widths(&[100.0, 120.0, 90.0], Some(0), 400.0, 56.0);
        assert_eq!(w, vec![100.0, 120.0, 90.0]);
    }

    #[test]
    fn without_room_inactive_tabs_shrink_and_the_active_one_keeps_its_width() {
        let natural = [100.0, 140.0, 120.0, 160.0];
        let w = fit_tab_widths(&natural, Some(1), 400.0, 56.0);
        assert_eq!(w[1], 140.0, "a aba ativa não encolhe");
        assert!((w.iter().sum::<f32>() - 400.0).abs() < 0.5, "{w:?}");
        for (i, width) in w.iter().enumerate() {
            assert!(*width >= 56.0 && *width <= natural[i] + 0.01, "{w:?}");
        }
        // A mais larga encolhe mais que a mais estreita, nunca abaixo do mínimo.
        assert!(w[3] <= natural[3] && w[0] <= natural[0]);
    }

    #[test]
    fn tabs_never_shrink_below_the_minimum_even_when_nothing_fits() {
        let w = fit_tab_widths(&[100.0, 140.0, 120.0, 160.0], Some(0), 150.0, 56.0);
        assert_eq!(w[0], 100.0);
        assert!(w[1..].iter().all(|x| (*x - 56.0).abs() < 0.01), "{w:?}");
    }

    #[test]
    fn a_narrow_tab_title_is_cut_with_an_ellipsis() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui_theme::font_definitions());
        let mut width = 0.0;
        let mut natural = 0.0;
        for _ in 0..3 {
            let mut full = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen()),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        natural = tab_natural_width(ui, None, "Súmula da Partida", false, true);
                        let (r, _) = tab_button_fitted(
                            ui,
                            egui::Id::new("t"),
                            None,
                            "Súmula da Partida",
                            false,
                            false,
                            egui::Sense::click(),
                            Some("Fechar"),
                            Some(TAB_MIN_WIDTH),
                        );
                        width = r.rect.width();
                    });
                },
            );
            full.textures_delta.clear();
        }
        assert!(
            width <= TAB_MIN_WIDTH + 0.5 && width < natural,
            "{width} {natural}"
        );
    }
}
