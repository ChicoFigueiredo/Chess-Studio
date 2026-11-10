//! Modelo puro do Ribbon (sem egui nem Bevy): abas, grupos tirados da lista única de Ações,
//! o corte do modo simples (grupos que não cabem vão para o menu "⋯") e a máquina de estados
//! dos KeyTips.

use chess_input::actions::{ActionKind, RibbonTab, meta};
use chess_input::controls::Action;

/// O que uma aba mostra: uma das abas fixas ou a aba contextual da Súmula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RibbonView {
    Tab(RibbonTab),
    /// Aparece só enquanto o Painel Súmula está aberto (DS-02).
    Scoresheet,
}

impl RibbonView {
    /// Id Fluent do nome da aba.
    pub fn label(self) -> &'static str {
        match self {
            RibbonView::Tab(tab) => tab.label(),
            RibbonView::Scoresheet => "ribbon-tab-scoresheet",
        }
    }

    /// Nome estável da aba, gravado no Perfil de interface (nunca muda).
    pub fn id(self) -> &'static str {
        match self {
            RibbonView::Tab(RibbonTab::File) => "file",
            RibbonView::Tab(RibbonTab::Home) => "home",
            RibbonView::Tab(RibbonTab::Analysis) => "analysis",
            RibbonView::Tab(RibbonTab::Library) => "library",
            RibbonView::Tab(RibbonTab::Training) => "training",
            RibbonView::Tab(RibbonTab::View) => "view",
            RibbonView::Scoresheet => "scoresheet",
        }
    }

    /// Lê o nome gravado; desconhecido vira `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        RibbonTab::ALL
            .into_iter()
            .map(RibbonView::Tab)
            .chain([RibbonView::Scoresheet])
            .find(|v| v.id() == id)
    }

    pub fn is_file(self) -> bool {
        self == RibbonView::Tab(RibbonTab::File)
    }

    /// Letra do KeyTip da aba (estável e única entre as abas).
    pub fn keytip(self) -> &'static str {
        match self {
            RibbonView::Tab(RibbonTab::File) => "A",
            RibbonView::Tab(RibbonTab::Home) => "I",
            RibbonView::Tab(RibbonTab::Analysis) => "N",
            RibbonView::Tab(RibbonTab::Library) => "B",
            RibbonView::Tab(RibbonTab::Training) => "T",
            RibbonView::Tab(RibbonTab::View) => "X",
            RibbonView::Scoresheet => "S",
        }
    }
}

/// As abas visíveis, na ordem; a da Súmula só com ela aberta. Uma aba sem nenhuma Ação (a
/// Biblioteca e o Treino, depois que os Painéis foram para Exibir, #183) não aparece.
pub fn visible_tabs(scoresheet_open: bool) -> Vec<RibbonView> {
    let mut tabs: Vec<_> = RibbonTab::ALL
        .into_iter()
        .map(RibbonView::Tab)
        .filter(|v| v.is_file() || !groups(*v).is_empty())
        .collect();
    if scoresheet_open {
        tabs.push(RibbonView::Scoresheet);
    }
    tabs
}

/// Um grupo do corpo da aba: rótulo (id Fluent) e Ações, na ordem da lista única.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: &'static str,
    pub actions: Vec<Action>,
}

/// Os grupos de uma aba, montados só a partir de `meta()`. A primeira Ação do grupo é o botão
/// grande do modo clássico. A aba Arquivo não tem corpo: abre os bastidores.
pub fn groups(view: RibbonView) -> Vec<Group> {
    let tab = match view {
        RibbonView::Scoresheet => {
            return vec![Group {
                label: "ribbon-group-scoresheet",
                actions: vec![Action::PanelScoresheet],
            }];
        }
        RibbonView::Tab(RibbonTab::File) => return Vec::new(),
        RibbonView::Tab(tab) => tab,
    };
    let mut out: Vec<Group> = Vec::new();
    for action in Action::ALL {
        let m = meta(action);
        if m.tab != Some(tab) || m.kind == ActionKind::Gesture {
            continue;
        }
        let Some(label) = m.group else { continue };
        match out.iter_mut().find(|g| g.label == label) {
            Some(group) => group.actions.push(action),
            None => out.push(Group {
                label,
                actions: vec![action],
            }),
        }
    }
    // Em Exibir, os Painéis abrem a aba (como o "Mostrar" do Word): são o que mais se usa ali.
    if let Some(i) = out.iter().position(|g| g.label == "ribbon-group-panels") {
        out[..=i].rotate_right(1);
    }
    out
}

/// As Ações de uma aba (todas as dos grupos), na ordem.
pub fn actions_of(view: RibbonView) -> Vec<Action> {
    groups(view).into_iter().flat_map(|g| g.actions).collect()
}

// --- Modo simples: o que cabe na linha ---

/// Resultado do corte da linha única do modo simples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverflowSplit {
    /// Quantos grupos (do começo) ficam visíveis na linha.
    pub visible: usize,
    /// Índices dos grupos que vão para o menu "⋯", na ordem original.
    pub overflow: Vec<usize>,
}

impl OverflowSplit {
    pub fn has_overflow(&self) -> bool {
        !self.overflow.is_empty()
    }
}

/// Decide quais grupos cabem em `available` pontos de largura. `widths` são as larguras dos
/// grupos, `gap` o espaço entre eles e `more_width` a largura do botão "⋯". Se tudo cabe, não
/// há "⋯"; senão os grupos saem da direita para a esquerda até sobrar espaço para o "⋯".
pub fn split_overflow(widths: &[f32], gap: f32, available: f32, more_width: f32) -> OverflowSplit {
    let total = |count: usize| -> f32 {
        let sum: f32 = widths[..count].iter().sum();
        sum + gap * count.saturating_sub(1) as f32
    };
    let n = widths.len();
    if total(n) <= available {
        return OverflowSplit {
            visible: n,
            overflow: Vec::new(),
        };
    }
    let budget = available - more_width - gap;
    let visible = (0..=n)
        .rev()
        .find(|&count| total(count) <= budget)
        .unwrap_or(0);
    OverflowSplit {
        visible,
        overflow: (visible..n).collect(),
    }
}

// --- KeyTips ---

/// Em que nível dos KeyTips o usuário está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyTipLevel {
    /// Nada à mostra.
    #[default]
    Off,
    /// Letras sobre as abas.
    Tabs,
    /// Letras sobre as Ações da aba.
    Actions(RibbonView),
}

/// Entrada da máquina de estados dos KeyTips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyTipInput {
    /// Alt apertado e solto sozinho.
    AltTap,
    Escape,
    Char(char),
}

/// O que a interface deve fazer em resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyTipOutcome {
    None,
    /// Torna a aba a ativa (e abre o corpo, se o Ribbon estiver recolhido).
    OpenTab(RibbonView),
    /// A aba Arquivo: abre os bastidores.
    OpenBackstage,
    /// Escreve `ActionRequested` para a Ação.
    Perform(Action),
}

/// Máquina de estados dos KeyTips (Office): Alt -> letra da aba -> letras da Ação; Esc sobe
/// um nível. Letras são estáveis (definidas na Ação), nunca geradas pela posição.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyTipNav {
    level: KeyTipLevel,
    typed: String,
}

impl KeyTipNav {
    pub fn level(&self) -> KeyTipLevel {
        self.level
    }

    pub fn is_active(&self) -> bool {
        self.level != KeyTipLevel::Off
    }

    /// Letras já digitadas no nível das Ações (KeyTips de mais de uma letra).
    pub fn typed(&self) -> &str {
        &self.typed
    }

    pub fn close(&mut self) {
        self.level = KeyTipLevel::Off;
        self.typed.clear();
    }

    /// Processa uma entrada. `tabs` são as abas visíveis; `enabled` diz se a Ação pode rodar
    /// agora (uma desabilitada não dispara e o que foi digitado é descartado).
    pub fn handle(
        &mut self,
        input: KeyTipInput,
        tabs: &[RibbonView],
        enabled: impl Fn(Action) -> bool,
    ) -> KeyTipOutcome {
        match (input, self.level) {
            (KeyTipInput::AltTap, KeyTipLevel::Off) => {
                self.level = KeyTipLevel::Tabs;
                self.typed.clear();
                KeyTipOutcome::None
            }
            (KeyTipInput::AltTap, _) => {
                self.close();
                KeyTipOutcome::None
            }
            (KeyTipInput::Escape, KeyTipLevel::Off) => KeyTipOutcome::None,
            (KeyTipInput::Escape, KeyTipLevel::Tabs) => {
                self.close();
                KeyTipOutcome::None
            }
            (KeyTipInput::Escape, KeyTipLevel::Actions(_)) => {
                // Primeiro descarta o que foi digitado; depois sobe para as abas.
                if self.typed.is_empty() {
                    self.level = KeyTipLevel::Tabs;
                }
                self.typed.clear();
                KeyTipOutcome::None
            }
            (KeyTipInput::Char(_), KeyTipLevel::Off) => KeyTipOutcome::None,
            (KeyTipInput::Char(c), KeyTipLevel::Tabs) => {
                let c = c.to_ascii_uppercase();
                let Some(view) = tabs.iter().copied().find(|t| t.keytip().starts_with(c)) else {
                    return KeyTipOutcome::None;
                };
                if view.is_file() {
                    self.close();
                    KeyTipOutcome::OpenBackstage
                } else {
                    self.level = KeyTipLevel::Actions(view);
                    self.typed.clear();
                    KeyTipOutcome::OpenTab(view)
                }
            }
            (KeyTipInput::Char(c), KeyTipLevel::Actions(view)) => {
                self.typed.push(c.to_ascii_uppercase());
                let actions = actions_of(view);
                // Igualdade exata vence prefixo.
                if let Some(action) = actions
                    .iter()
                    .copied()
                    .find(|a| meta(*a).keytip == self.typed)
                {
                    self.typed.clear();
                    if enabled(action) {
                        self.close();
                        return KeyTipOutcome::Perform(action);
                    }
                    return KeyTipOutcome::None;
                }
                let is_prefix = actions
                    .iter()
                    .any(|a| meta(*a).keytip.starts_with(self.typed.as_str()));
                if !is_prefix {
                    self.typed.clear();
                }
                KeyTipOutcome::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn tabs() -> Vec<RibbonView> {
        visible_tabs(false)
    }

    fn always(_: Action) -> bool {
        true
    }

    // --- grupos ---

    #[test]
    fn groups_come_from_the_action_list_in_order() {
        let home = groups(RibbonView::Tab(RibbonTab::Home));
        let labels: Vec<_> = home.iter().map(|g| g.label).collect();
        assert!(labels.contains(&"ribbon-group-game") && labels.contains(&"ribbon-group-clock"));
        let game = home
            .iter()
            .find(|g| g.label == "ribbon-group-game")
            .unwrap();
        assert_eq!(game.actions[0], Action::Undo);
    }

    #[test]
    fn every_ribbon_action_lands_in_exactly_one_group_and_gestures_stay_out() {
        for action in Action::ALL {
            let m = meta(action);
            let found: Vec<_> = RibbonTab::ALL
                .into_iter()
                .flat_map(|t| actions_of(RibbonView::Tab(t)))
                .filter(|a| *a == action)
                .collect();
            if m.kind == ActionKind::Gesture || m.tab == Some(RibbonTab::File) {
                assert!(found.is_empty(), "{action:?}");
            } else {
                assert_eq!(found.len(), 1, "{action:?}");
            }
        }
    }

    #[test]
    fn view_tab_has_one_panels_group_with_every_panel_and_other_tabs_have_none() {
        let view = groups(RibbonView::Tab(RibbonTab::View));
        let panels = view
            .iter()
            .find(|g| g.label == "ribbon-group-panels")
            .expect("grupo Painéis em Exibir");
        assert_eq!(view[0].label, "ribbon-group-panels", "Painéis abre a aba");
        for action in Action::ALL {
            if action.panel().is_some() {
                assert!(panels.actions.contains(&action), "{action:?}");
            }
        }
        for tab in RibbonTab::ALL {
            if tab == RibbonTab::View {
                continue;
            }
            for action in actions_of(RibbonView::Tab(tab)) {
                assert!(action.panel().is_none(), "{action:?} repetido em {tab:?}");
            }
        }
    }

    #[test]
    fn tabs_without_actions_are_not_shown() {
        let tabs = visible_tabs(false);
        assert!(!tabs.contains(&RibbonView::Tab(RibbonTab::Library)));
        assert!(!tabs.contains(&RibbonView::Tab(RibbonTab::Training)));
        assert!(tabs.contains(&RibbonView::Tab(RibbonTab::View)));
    }

    #[test]
    fn the_scoresheet_tab_only_exists_with_the_panel_open() {
        assert_eq!(visible_tabs(false).len(), 4);
        assert_eq!(visible_tabs(true).last(), Some(&RibbonView::Scoresheet));
        assert_eq!(
            actions_of(RibbonView::Scoresheet),
            [Action::PanelScoresheet]
        );
    }

    #[test]
    fn tab_keytips_are_unique_and_action_keytips_are_unique_within_a_tab() {
        let all = visible_tabs(true);
        let letters: HashSet<_> = all.iter().map(|t| t.keytip()).collect();
        assert_eq!(letters.len(), all.len());
        for view in all {
            let mut seen = HashSet::new();
            for action in actions_of(view) {
                let tip = meta(action).keytip;
                assert!(!tip.is_empty(), "{action:?} sem KeyTip");
                assert!(seen.insert(tip), "KeyTip repetido na aba: {tip}");
            }
        }
    }

    // --- corte do modo simples ---

    #[test]
    fn everything_fits_without_the_more_menu() {
        let split = split_overflow(&[100.0, 80.0, 60.0], 10.0, 260.0, 40.0);
        assert_eq!(split.visible, 3);
        assert!(!split.has_overflow());
    }

    #[test]
    fn groups_overflow_from_right_to_left_leaving_room_for_the_more_button() {
        // Total: 100+80+60 + 2*10 = 260. Com 240: sai o último; 190 <= 240-40-10.
        let split = split_overflow(&[100.0, 80.0, 60.0], 10.0, 240.0, 40.0);
        assert_eq!(split.visible, 2);
        assert_eq!(split.overflow, [2]);
        // Com 190: 190 > 190-40-10 = 140, então sai mais um.
        let split = split_overflow(&[100.0, 80.0, 60.0], 10.0, 190.0, 40.0);
        assert_eq!(split.visible, 1);
        assert_eq!(split.overflow, [1, 2]);
    }

    #[test]
    fn nothing_visible_when_not_even_one_group_fits() {
        let split = split_overflow(&[100.0, 80.0], 10.0, 120.0, 40.0);
        assert_eq!(split.visible, 0);
        assert_eq!(split.overflow, [0, 1]);
    }

    #[test]
    fn exact_fit_and_empty_input_never_overflow() {
        assert!(!split_overflow(&[100.0, 80.0], 10.0, 190.0, 40.0).has_overflow());
        let empty = split_overflow(&[], 10.0, 0.0, 40.0);
        assert_eq!(empty.visible, 0);
        assert!(!empty.has_overflow());
    }

    // --- KeyTips ---

    #[test]
    fn alt_toggles_the_tab_badges() {
        let mut nav = KeyTipNav::default();
        assert!(!nav.is_active());
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        assert_eq!(nav.level(), KeyTipLevel::Tabs);
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        assert_eq!(nav.level(), KeyTipLevel::Off);
    }

    #[test]
    fn a_tab_letter_shows_that_tabs_actions_and_an_action_keytip_performs_it() {
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        let home = RibbonView::Tab(RibbonTab::Home);
        let out = nav.handle(KeyTipInput::Char('i'), &tabs(), always);
        assert_eq!(out, KeyTipOutcome::OpenTab(home));
        assert_eq!(nav.level(), KeyTipLevel::Actions(home));
        let out = nav.handle(KeyTipInput::Char('u'), &tabs(), always);
        assert_eq!(out, KeyTipOutcome::Perform(Action::Undo));
        assert!(!nav.is_active());
    }

    #[test]
    fn multi_letter_keytips_wait_for_the_second_letter() {
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        nav.handle(KeyTipInput::Char('X'), &tabs(), always);
        assert_eq!(
            nav.handle(KeyTipInput::Char('P'), &tabs(), always),
            KeyTipOutcome::None
        );
        assert_eq!(nav.typed(), "P");
        assert_eq!(
            nav.handle(KeyTipInput::Char('T'), &tabs(), always),
            KeyTipOutcome::Perform(Action::PresentTechnical)
        );
    }

    #[test]
    fn escape_goes_up_one_level() {
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        nav.handle(KeyTipInput::Char('X'), &tabs(), always);
        nav.handle(KeyTipInput::Char('P'), &tabs(), always);
        // Primeiro descarta o "P" digitado.
        nav.handle(KeyTipInput::Escape, &tabs(), always);
        assert!(matches!(nav.level(), KeyTipLevel::Actions(_)));
        assert_eq!(nav.typed(), "");
        nav.handle(KeyTipInput::Escape, &tabs(), always);
        assert_eq!(nav.level(), KeyTipLevel::Tabs);
        nav.handle(KeyTipInput::Escape, &tabs(), always);
        assert_eq!(nav.level(), KeyTipLevel::Off);
    }

    #[test]
    fn the_file_tab_letter_opens_the_backstage_and_ends_the_keytips() {
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        assert_eq!(
            nav.handle(KeyTipInput::Char('A'), &tabs(), always),
            KeyTipOutcome::OpenBackstage
        );
        assert!(!nav.is_active());
    }

    #[test]
    fn unknown_letters_and_disabled_actions_do_nothing() {
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        assert_eq!(
            nav.handle(KeyTipInput::Char('9'), &tabs(), always),
            KeyTipOutcome::None
        );
        assert_eq!(nav.level(), KeyTipLevel::Tabs);
        nav.handle(KeyTipInput::Char('I'), &tabs(), always);
        let out = nav.handle(KeyTipInput::Char('U'), &tabs(), |a| a != Action::Undo);
        assert_eq!(out, KeyTipOutcome::None);
        assert!(nav.is_active());
    }

    #[test]
    fn letters_without_alt_do_nothing_and_the_scoresheet_tab_is_reachable() {
        let mut nav = KeyTipNav::default();
        assert_eq!(
            nav.handle(KeyTipInput::Char('I'), &tabs(), always),
            KeyTipOutcome::None
        );
        let with_sheet = visible_tabs(true);
        nav.handle(KeyTipInput::AltTap, &with_sheet, always);
        assert_eq!(
            nav.handle(KeyTipInput::Char('s'), &with_sheet, always),
            KeyTipOutcome::OpenTab(RibbonView::Scoresheet)
        );
        // Sem a aba visível, a letra não faz nada.
        let mut nav = KeyTipNav::default();
        nav.handle(KeyTipInput::AltTap, &tabs(), always);
        assert_eq!(
            nav.handle(KeyTipInput::Char('S'), &tabs(), always),
            KeyTipOutcome::None
        );
    }
}
