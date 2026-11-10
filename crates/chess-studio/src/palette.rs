//! Paleta de Ações (Ctrl+K, DS-4, #163): busca qualquer Ação pelo nome e a executa com Enter.
//! A lista vem da lista única de Ações (`chess_input::actions`); executar é escrever um
//! [`ActionRequested`], o mesmo caminho de atalhos, Ribbon e Barra de Acesso Rápido. Ação
//! desabilitada aparece esmaecida com o motivo e nunca executa.
//!
//! Só Apresentação (ADR-0001): nada aqui toca a Partida. A busca e a escolha do que executar
//! são funções puras, testadas sem egui.

use crate::action_icons::glyph;
use crate::actions::{ActionEnv, ActionRequested};
use crate::controls::{Controls, trigger_label};
use crate::i18n::I18n;
use crate::screen_layouts::{FACTORY, LayoutRef, ScreenLayoutRequested, ScreenLayouts};
use crate::ui_theme::{self, Palette, ThemeKind, Tokens};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_input::actions::{ActionContext, ActionKind, meta, state};
use chess_input::controls::Action;

/// Estado da Paleta: aberta, o texto da busca e a linha escolhida.
#[derive(Resource, Debug, Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    /// O campo de busca ainda precisa receber o foco (primeiro quadro depois de abrir).
    focus_pending: bool,
}

impl PaletteState {
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus_pending = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }
}

/// O que uma linha da Paleta executa: uma Ação da lista única ou, como entrada dinâmica, abrir
/// um Layout de Painéis (#186; os Layouts salvos não cabem num enum fixo de Ações).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Action(Action),
    ApplyLayout(LayoutRef),
}

/// Uma Ação (ou Layout) como a Paleta a mostra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub target: Target,
    pub label: String,
    /// Glifo Phosphor.
    pub icon: &'static str,
    pub shortcut: Option<String>,
    /// Motivo (já traduzido) de a Ação estar desabilitada agora.
    pub disabled_reason: Option<String>,
}

impl Entry {
    /// A Ação, se a linha é de Ação.
    #[cfg(test)]
    pub fn action(&self) -> Option<Action> {
        match self.target {
            Target::Action(a) => Some(a),
            Target::ApplyLayout(_) => None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.disabled_reason.is_none()
    }
}

/// Texto sem caixa nem acentos do pt-BR, para comparar ("Ação" -> "acao").
pub fn normalize(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Classe de um termo no rótulo: 0 = começa o rótulo, 1 = começa uma palavra, 2 = aparece no
/// meio de uma palavra. `None` = não aparece.
fn match_class(label: &str, term: &str) -> Option<u8> {
    if label.starts_with(term) {
        return Some(0);
    }
    let mut best = None;
    for (at, _) in label.match_indices(term) {
        let word_start = label[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if word_start {
            return Some(1);
        }
        best = Some(2);
    }
    best
}

/// Índices dos rótulos que casam com a busca, do melhor para o pior. Sem busca, todos na ordem
/// original. Maiúsculas e acentos não contam. Cada palavra da busca precisa aparecer; a nota é
/// a pior das palavras (prefixo < início de palavra < trecho), e empates mantêm a ordem original.
pub fn rank<S: AsRef<str>>(query: &str, labels: &[S]) -> Vec<usize> {
    let normalized = normalize(query);
    let terms: Vec<&str> = normalized.split_whitespace().collect();
    if terms.is_empty() {
        return (0..labels.len()).collect();
    }
    let mut scored: Vec<(u8, usize)> = labels
        .iter()
        .enumerate()
        .filter_map(|(i, label)| {
            let label = normalize(label.as_ref());
            let mut worst = 0;
            for term in &terms {
                worst = worst.max(match_class(&label, term)?);
            }
            Some((worst, i))
        })
        .collect();
    scored.sort();
    scored.into_iter().map(|(_, i)| i).collect()
}

/// As linhas que a busca mostra, em ordem.
pub fn visible(entries: &[Entry], query: &str) -> Vec<usize> {
    let labels: Vec<&str> = entries.iter().map(|e| e.label.as_str()).collect();
    rank(query, &labels)
}

/// O que Enter (ou o clique) executa na linha escolhida: nada se está desabilitada.
pub fn activation(entries: &[Entry], shown: &[usize], selected: usize) -> Option<Target> {
    let entry = entries.get(*shown.get(selected)?)?;
    entry.is_enabled().then(|| entry.target.clone())
}

/// Executa o alvo: Ação vira [`ActionRequested`]; Layout vira [`ScreenLayoutRequested`] (abrir,
/// ou mostrar só com Shift).
fn dispatch(
    target: Target,
    show_only: bool,
    actions: &mut MessageWriter<ActionRequested>,
    sets: &mut MessageWriter<ScreenLayoutRequested>,
) {
    match target {
        Target::Action(action) => {
            actions.write(ActionRequested(action));
        }
        Target::ApplyLayout(set) => {
            sets.write(if show_only {
                ScreenLayoutRequested::ShowOnly(set)
            } else {
                ScreenLayoutRequested::Open(set)
            });
        }
    }
}

/// As Ações da Paleta: tudo menos os gestos e a própria Paleta.
pub fn entries(
    i18n: &I18n,
    controls: &Controls,
    ctx: &ActionContext,
    sets: &ScreenLayouts,
) -> Vec<Entry> {
    let set_icon = glyph(meta(Action::HideAllPanels).icon);
    let apply_layout = |set: LayoutRef, name: &str| Entry {
        target: Target::ApplyLayout(set),
        label: i18n.t_with("palette-apply-layout", &[("name", name)]),
        icon: set_icon,
        shortcut: None,
        disabled_reason: None,
    };
    let actions = Action::ALL
        .into_iter()
        .filter(|a| *a != Action::OpenPalette && meta(*a).kind != ActionKind::Gesture)
        .map(|action| {
            let m = meta(action);
            Entry {
                target: Target::Action(action),
                label: i18n.t(m.label),
                icon: glyph(m.icon),
                shortcut: controls
                    .map()
                    .triggers(action)
                    .first()
                    .map(|t| trigger_label(i18n, t)),
                disabled_reason: state(action, ctx).disabled_reason.map(|r| i18n.t(r)),
            }
        });
    let factory = FACTORY
        .iter()
        .map(|f| apply_layout(LayoutRef::Factory(f.id), &i18n.t(f.label_key)));
    let saved = sets
        .saved()
        .iter()
        .map(|s| apply_layout(LayoutRef::Saved(s.name.clone()), &s.name));
    actions.chain(factory).chain(saved).collect()
}

pub struct PalettePlugin;

impl Plugin for PalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteState>().add_systems(
            EguiPrimaryContextPass,
            palette_ui.after(ui_theme::apply_appearance),
        );
    }
}

const ROW_HEIGHT: f32 = 32.0;
const WIDTH: f32 = 560.0;

fn palette_ui(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    controls: Res<Controls>,
    mut env: ActionEnv,
    sets: Option<Res<ScreenLayouts>>,
    mut out: MessageWriter<ActionRequested>,
    mut set_out: MessageWriter<ScreenLayoutRequested>,
) -> Result {
    if !env.palette.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let list = entries(
        &i18n,
        &controls,
        &env.context(),
        sets.as_deref().unwrap_or(&ScreenLayouts::default()),
    );
    let palette = &mut env.palette;
    let mut shown = visible(&list, &palette.query);

    // Teclas antes do campo de texto, que do contrário as consumiria (setas e Enter).
    let (esc, enter, down, up) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
        )
    });
    if esc {
        palette.close();
        return Ok(());
    }
    let last = shown.len().saturating_sub(1);
    palette.selected = palette.selected.min(last);
    let moved = (down && palette.selected < last) || (up && palette.selected > 0);
    if down && palette.selected < last {
        palette.selected += 1;
    }
    if up && palette.selected > 0 {
        palette.selected -= 1;
    }
    let shift = ctx.input(|i| i.modifiers.shift);
    if enter && let Some(target) = activation(&list, &shown, palette.selected) {
        dispatch(target, shift, &mut out, &mut set_out);
        palette.close();
        return Ok(());
    }

    let theme = ThemeKind::from_dark_mode(ctx.global_style().visuals.dark_mode);
    let p = Palette::of(theme);
    let tokens = Tokens::embedded();
    let radius = tokens.length("radius-xl").unwrap_or(8.0);
    let frame = egui::Frame::new()
        .fill(p.surface_3)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(radius)
        .shadow(tokens.shadow("shadow-dialog", theme).unwrap_or_default())
        .inner_margin(egui::Margin::same(12));

    let mut clicked = None;
    let mut query = palette.query.clone();
    let mut selected = palette.selected;
    let focus = std::mem::take(&mut palette.focus_pending);
    let response = egui::Modal::new(egui::Id::new("action-palette"))
        .backdrop_color(p.scrim)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let field = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text(i18n.t("palette-hint"))
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(8, 6)),
            );
            if focus {
                field.request_focus();
            }
            ui.add_space(6.0);
            shown = visible(&list, &query);
            if shown.is_empty() {
                ui.label(egui::RichText::new(i18n.t("palette-empty")).color(p.ink_muted));
            }
            egui::ScrollArea::vertical()
                .max_height(ROW_HEIGHT * 10.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (row, &index) in shown.iter().enumerate() {
                        let hit = row_ui(ui, p, &list[index], row == selected);
                        if row == selected && moved {
                            hit.scroll_to_me(None);
                        }
                        if hit.clicked() && list[index].is_enabled() {
                            clicked = Some(list[index].target.clone());
                        }
                        if hit.hovered() && hit.ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                        {
                            selected = row;
                        }
                    }
                });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(i18n.t("palette-footer"))
                    .small()
                    .color(p.ink_muted),
            );
        });
    if query != palette.query {
        palette.query = query;
        palette.selected = 0;
    } else {
        palette.selected = selected;
    }
    if let Some(target) = clicked {
        dispatch(target, shift, &mut out, &mut set_out);
        palette.close();
    } else if response.backdrop_response.clicked() {
        palette.close();
    }
    Ok(())
}

/// Uma linha: ícone, rótulo e atalho (ou o motivo, se desabilitada, esmaecida).
fn row_ui(ui: &mut egui::Ui, p: &Palette, entry: &Entry, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 4.0, p.accent_subtle);
    }
    let enabled = entry.is_enabled();
    let ink = if enabled { p.ink } else { p.ink_disabled };
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::TextStyle::Small.resolve(ui.style());
    painter.text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        entry.icon,
        ui_theme::icon_font(ui.ctx(), body.size + 2.0),
        ink,
    );
    painter.text(
        egui::pos2(rect.left() + 36.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &entry.label,
        body,
        ink,
    );
    let right = match (&entry.disabled_reason, &entry.shortcut) {
        (Some(reason), _) => Some((reason, p.ink_disabled)),
        (None, Some(shortcut)) => Some((shortcut, p.ink_muted)),
        (None, None) => None,
    };
    if let Some((text, color)) = right {
        painter.text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            small,
            color,
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(query: &str, all: &[&str]) -> Vec<String> {
        rank(query, all)
            .into_iter()
            .map(|i| all[i].to_owned())
            .collect()
    }

    #[test]
    fn accents_and_case_do_not_matter() {
        assert_eq!(normalize("Ação"), "acao");
        assert_eq!(
            labels("acao", &["Nova Partida", "Ação rápida"]),
            ["Ação rápida"]
        );
        assert_eq!(labels("ACOES", &["Paleta de Ações"]), ["Paleta de Ações"]);
        // A busca também pode vir com acento.
        assert_eq!(labels("ações", &["Paleta de Ações"]), ["Paleta de Ações"]);
    }

    #[test]
    fn prefix_then_word_start_then_substring() {
        let all = ["Desvirar", "Tabuleiro: virar lado", "Virar tabuleiro"];
        assert_eq!(
            labels("virar", &all),
            ["Virar tabuleiro", "Tabuleiro: virar lado", "Desvirar"]
        );
    }

    #[test]
    fn ties_keep_the_original_order() {
        let all = ["Vista de topo", "Vista padrão", "Vista invertida"];
        assert_eq!(labels("vista", &all), all);
    }

    #[test]
    fn every_word_of_the_query_must_match() {
        let all = ["Vista de topo", "Vista padrão", "Painel de topo"];
        assert_eq!(labels("vista topo", &all), ["Vista de topo"]);
        assert!(labels("vista xadrez", &all).is_empty());
    }

    #[test]
    fn an_empty_query_lists_everything_in_order() {
        let all = ["B", "A"];
        assert_eq!(rank("", &all), [0, 1]);
        assert_eq!(rank("   ", &all), [0, 1]);
    }

    fn entry(action: Action, label: &str, reason: Option<&str>) -> Entry {
        Entry {
            target: Target::Action(action),
            label: label.to_owned(),
            icon: "",
            shortcut: None,
            disabled_reason: reason.map(str::to_owned),
        }
    }

    #[test]
    fn enter_executes_the_selected_enabled_action() {
        let list = [
            entry(Action::NewGame, "Nova Partida", None),
            entry(Action::FlipBoard, "Virar tabuleiro", None),
        ];
        let shown = visible(&list, "virar");
        assert_eq!(
            activation(&list, &shown, 0),
            Some(Target::Action(Action::FlipBoard))
        );
    }

    #[test]
    fn a_disabled_action_is_not_executed() {
        let list = [entry(
            Action::Undo,
            "Desfazer",
            Some("Só no modo Assistido."),
        )];
        let shown = visible(&list, "");
        assert_eq!(activation(&list, &shown, 0), None);
        // Fora da lista também não executa nada.
        assert_eq!(activation(&list, &[], 0), None);
    }

    #[test]
    fn the_palette_lists_every_non_gesture_action_with_the_reason_when_disabled() {
        let i18n = I18n::pt_br();
        let controls = Controls::default();
        // Sem Partida competitiva: Desfazer é permitido, mas não há o que desfazer.
        let ctx = ActionContext::default();
        let list = entries(&i18n, &controls, &ctx, &ScreenLayouts::default());
        assert!(list.iter().all(|e| e.action() != Some(Action::OpenPalette)));
        assert!(list.iter().all(|e| {
            e.action()
                .is_none_or(|a| meta(a).kind != ActionKind::Gesture)
        }));
        let undo = list
            .iter()
            .find(|e| e.action() == Some(Action::Undo))
            .unwrap();
        assert!(!undo.is_enabled());
        assert_eq!(
            undo.disabled_reason.as_deref(),
            Some("Não há Lance para desfazer.")
        );
        let new_game = list
            .iter()
            .find(|e| e.action() == Some(Action::NewGame))
            .unwrap();
        assert!(new_game.is_enabled());
        assert!(new_game.shortcut.is_some());
        assert!(
            list.iter()
                .all(|e| !e.icon.is_empty() && !e.label.is_empty())
        );
    }

    /// #186: "Aplicar Layout: <nome>" é uma entrada dinâmica (de fábrica e salvos), achada pela
    /// busca e executada como pedido de Layout, sem Ação no enum.
    #[test]
    fn the_palette_lists_apply_layout_entries_for_factory_and_saved_sets() {
        let i18n = I18n::pt_br();
        let mut sets = ScreenLayouts::default();
        sets.save("Meu final", vec!["Clock".to_owned()]).unwrap();
        let list = entries(
            &i18n,
            &Controls::default(),
            &ActionContext::default(),
            &sets,
        );
        let apply_entries: Vec<&Entry> = list
            .iter()
            .filter(|e| matches!(e.target, Target::ApplyLayout(_)))
            .collect();
        assert_eq!(apply_entries.len(), FACTORY.len() + 1);
        assert!(
            apply_entries
                .iter()
                .all(|e| e.is_enabled() && !e.icon.is_empty())
        );
        let shown = visible(&list, "aplicar layout analise");
        assert_eq!(
            activation(&list, &shown, 0),
            Some(Target::ApplyLayout(LayoutRef::Factory("analysis")))
        );
        assert_eq!(list[shown[0]].label, "Aplicar Layout: Análise");
        let shown = visible(&list, "layout final");
        assert_eq!(
            activation(&list, &shown, 0),
            Some(Target::ApplyLayout(LayoutRef::Saved("Meu final".into())))
        );
    }

    #[test]
    fn opening_resets_the_search() {
        let mut state = PaletteState {
            query: "x".into(),
            selected: 3,
            ..default()
        };
        state.open();
        assert!(state.open && state.query.is_empty() && state.selected == 0);
        state.toggle();
        assert!(!state.open);
    }
}
