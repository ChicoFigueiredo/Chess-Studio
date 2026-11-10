//! A lista única de Ações (DS-14): o que o usuário pode mandar o aplicativo fazer, com
//! metadados (rótulo, descrição, ícone, KeyTip, aba e grupo do Ribbon) e o **estado** de cada
//! uma (habilitada ou desabilitada com motivo; marcada ou não) calculado por função pura a
//! partir de um instantâneo do app ([`ActionContext`]).
//!
//! Ribbon, Barra de Acesso Rápido, KeyTips, Paleta de Ações e Mapa de controles consomem esta
//! mesma lista. O tipo é o [`Action`] do Mapa de controles; os atalhos vêm de
//! [`crate::controls::ControlMap`], não daqui. Textos são ids do Fluent (`action-<id>`,
//! `action-<id>-desc`, `ribbon-tab-*`, `ribbon-group-*`, `reason-*`) resolvidos pelo app;
//! ícones são nomes do Phosphor em kebab-case. Nada aqui depende do Bevy.

use crate::controls::Action;
use crate::view::CameraView;
use std::collections::BTreeSet;

/// Abas do Ribbon (DS-02), na ordem em que aparecem. Abas contextuais (Súmula…) entram depois.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RibbonTab {
    File,
    Home,
    Analysis,
    Library,
    Training,
    View,
}

impl RibbonTab {
    pub const ALL: [RibbonTab; 6] = [
        RibbonTab::File,
        RibbonTab::Home,
        RibbonTab::Analysis,
        RibbonTab::Library,
        RibbonTab::Training,
        RibbonTab::View,
    ];

    /// Id Fluent do nome da aba.
    pub fn label(self) -> &'static str {
        match self {
            RibbonTab::File => "ribbon-tab-file",
            RibbonTab::Home => "ribbon-tab-home",
            RibbonTab::Analysis => "ribbon-tab-analysis",
            RibbonTab::Library => "ribbon-tab-library",
            RibbonTab::Training => "ribbon-tab-training",
            RibbonTab::View => "ribbon-tab-view",
        }
    }
}

/// Como a Ação se comporta na interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Executa e pronto.
    Command,
    /// Liga e desliga (aparece marcada ou não).
    Toggle,
    /// Uma de várias opções excludentes (Apresentação, Vista): marcada se é a atual.
    Choice,
    /// Só existe como gesto de ponteiro ou de teclado (selecionar, órbita…): fica fora do
    /// Ribbon, dos KeyTips e da Paleta.
    Gesture,
}

/// Metadados estáticos de uma Ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionMeta {
    /// Id Fluent do rótulo.
    pub label: &'static str,
    /// Id Fluent da descrição (tooltip e Paleta).
    pub description: &'static str,
    /// Nome do ícone Phosphor (`"arrow-counter-clockwise"`).
    pub icon: &'static str,
    /// Letras do KeyTip, estáveis e únicas dentro da aba. Vazio fora do Ribbon.
    pub keytip: &'static str,
    /// Aba do Ribbon; `None` para Ações só de gesto.
    pub tab: Option<RibbonTab>,
    /// Id Fluent do grupo dentro da aba.
    pub group: Option<&'static str>,
    pub kind: ActionKind,
}

macro_rules! item {
    ($id:literal, $icon:literal, $kind:ident, $tab:ident, $group:literal, $keytip:literal) => {
        ActionMeta {
            label: concat!("action-", $id),
            description: concat!("action-", $id, "-desc"),
            icon: $icon,
            keytip: $keytip,
            tab: Some(RibbonTab::$tab),
            group: Some(concat!("ribbon-group-", $group)),
            kind: ActionKind::$kind,
        }
    };
}

macro_rules! gesture {
    ($id:literal, $icon:literal) => {
        ActionMeta {
            label: concat!("action-", $id),
            description: concat!("action-", $id, "-desc"),
            icon: $icon,
            keytip: "",
            tab: None,
            group: None,
            kind: ActionKind::Gesture,
        }
    };
}

/// Metadados da Ação: a tabela única.
pub fn meta(action: Action) -> ActionMeta {
    use Action as A;
    match action {
        // Só gesto.
        A::Select => gesture!("select", "cursor"),
        A::Adjust => gesture!("adjust", "hand-pointing"),
        A::Orbit => gesture!("orbit", "arrows-clockwise"),
        A::Pan => gesture!("pan", "hand-grabbing"),
        A::Zoom => gesture!("zoom", "magnifying-glass-plus"),
        A::Cancel => gesture!("cancel", "x-circle"),
        // Arquivo.
        A::NewGame => item!("new-game", "file-plus", Command, File, "game", "N"),
        A::OpenControls => item!(
            "open-controls",
            "keyboard",
            Command,
            File,
            "preferences",
            "C"
        ),
        A::OpenOptions => item!("open-options", "gear", Command, File, "preferences", "O"),
        // Início.
        A::Undo => item!(
            "undo",
            "arrow-counter-clockwise",
            Command,
            Home,
            "game",
            "U"
        ),
        A::Redo => item!("redo", "arrow-clockwise", Command, Home, "game", "R"),
        A::Resign => item!("resign", "flag", Command, Home, "game", "A"),
        A::OfferDraw => item!("offer-draw", "handshake", Command, Home, "game", "E"),
        A::ClaimDraw => item!("claim-draw", "scales", Command, Home, "game", "L"),
        A::ClockPress => item!("clock-press", "timer", Command, Home, "clock", "K"),
        A::PauseClock => item!("pause-clock", "pause", Toggle, Home, "clock", "P"),
        // Análise.
        A::ToggleAnalysis => item!(
            "toggle-analysis",
            "chart-line",
            Toggle,
            Analysis,
            "engine",
            "A"
        ),
        A::PanelAnalysis => item!(
            "panel-analysis",
            "chart-bar",
            Toggle,
            Analysis,
            "panels",
            "N"
        ),
        A::PanelStudy => item!("panel-study", "student", Toggle, Analysis, "panels", "E"),
        A::PanelEngines => item!("panel-engines", "cpu", Toggle, Analysis, "panels", "M"),
        // Biblioteca.
        A::PanelLibrary => item!("panel-library", "books", Toggle, Library, "panels", "B"),
        A::PanelOpenings => item!(
            "panel-openings",
            "tree-structure",
            Toggle,
            Library,
            "panels",
            "A"
        ),
        // Treino.
        A::PanelTraining => item!("panel-training", "barbell", Toggle, Training, "panels", "T"),
        // Exibir: Apresentação.
        A::PresentProfessional => item!(
            "present-professional",
            "cube",
            Choice,
            View,
            "presentation",
            "PP"
        ),
        A::PresentImmersive => item!(
            "present-immersive",
            "table",
            Choice,
            View,
            "presentation",
            "PI"
        ),
        A::PresentTechnical => item!(
            "present-technical",
            "grid-four",
            Choice,
            View,
            "presentation",
            "PT"
        ),
        A::CyclePresentation => item!(
            "cycle-presentation",
            "arrows-left-right",
            Command,
            View,
            "presentation",
            "PC"
        ),
        // Exibir: câmera.
        A::ViewTop => item!("view-top", "square", Choice, View, "camera", "VT"),
        A::ViewOblique => item!("view-oblique", "cube-focus", Choice, View, "camera", "VO"),
        A::ViewStandard => item!("view-standard", "camera", Choice, View, "camera", "VP"),
        A::ViewInverted => item!(
            "view-inverted",
            "camera-rotate",
            Choice,
            View,
            "camera",
            "VI"
        ),
        A::ResetCamera => item!("reset-camera", "crosshair", Command, View, "camera", "VR"),
        A::FrameBoard => item!(
            "frame-board",
            "frame-corners",
            Command,
            View,
            "camera",
            "VE"
        ),
        // Exibir: janela.
        A::ToggleFullscreen => item!(
            "toggle-fullscreen",
            "corners-out",
            Toggle,
            View,
            "window",
            "TC"
        ),
        A::ToggleFocus => item!("toggle-focus", "eye", Toggle, View, "window", "TF"),
        A::ToggleHud => item!("toggle-hud", "chart-polar", Toggle, View, "window", "TH"),
        // Exibir: tabuleiro.
        A::ToggleCoordinates => item!("toggle-coordinates", "text-aa", Toggle, View, "board", "BC"),
        A::ToggleHoverHighlight => item!(
            "toggle-hover-highlight",
            "cursor-click",
            Toggle,
            View,
            "board",
            "BR"
        ),
        A::FlipBoard => item!("flip-board", "arrows-down-up", Toggle, View, "board", "BV"),
        // Exibir: painéis.
        A::PanelNotation => item!(
            "panel-notation",
            "list-numbers",
            Toggle,
            View,
            "panels",
            "NN"
        ),
        A::PanelClock => item!("panel-clock", "clock", Toggle, View, "panels", "NR"),
        A::PanelCaptured => item!("panel-captured", "stack", Toggle, View, "panels", "NC"),
        A::PanelAids => item!("panel-aids", "lifebuoy", Toggle, View, "panels", "NA"),
        A::PanelReview => item!(
            "panel-review",
            "clock-counter-clockwise",
            Toggle,
            View,
            "panels",
            "NV"
        ),
        A::PanelArbitration => item!("panel-arbitration", "gavel", Toggle, View, "panels", "NB"),
        A::PanelScoresheet => item!("panel-scoresheet", "notepad", Toggle, View, "panels", "NS"),
        A::PanelBoard2d => item!(
            "panel-board2d",
            "checkerboard",
            Toggle,
            View,
            "panels",
            "N2"
        ),
        // Exibir: janelas dos Painéis.
        A::FloatPanel => item!(
            "float-panel",
            "picture-in-picture",
            Command,
            View,
            "windows",
            "JF"
        ),
        A::DockPanel => item!("dock-panel", "squares-four", Command, View, "windows", "JE"),
        A::DetachPanel => item!(
            "detach-panel",
            "arrow-square-out",
            Command,
            View,
            "windows",
            "JD"
        ),
        A::ResetLayout => item!("reset-layout", "layout", Command, View, "windows", "JR"),
        // Exibir: interface.
        A::OpenPalette => item!("open-palette", "command", Command, View, "interface", "IP"),
        A::ToggleRibbonCollapse => item!(
            "toggle-ribbon-collapse",
            "caret-line-up",
            Toggle,
            View,
            "interface",
            "IR"
        ),
    }
}

/// Presentação atual, espelho neutro do tipo do app (este crate não conhece o Bevy).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PresentationId {
    #[default]
    Professional3d,
    ImmersiveTable,
    Technical2d,
}

/// Instantâneo do app de que o estado das Ações depende. O app o monta a cada quadro.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionContext {
    /// A Política da Partida não é a Assistido (Simulação FIDE, Eliminação Imediata).
    pub competitive: bool,
    /// Partida competitiva ainda sem resultado: dicas e análise travadas (UI-08).
    pub competitive_in_progress: bool,
    /// Há Partida em andamento (sem resultado).
    pub game_running: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_clock: bool,
    pub paused: bool,
    pub draw_offer_pending: bool,
    pub claimable_draw: bool,
    /// Há Engine liberada pelo consentimento (ADR-0003).
    pub engine_consented: bool,
    pub analysis_on: bool,
    /// Um Exercício de Treino ocupa a tela (a Análise fica travada).
    pub training_on_screen: bool,
    pub presentation: PresentationId,
    /// Vista da câmera atual; `None` com a câmera livre.
    pub camera_view: Option<CameraView>,
    pub fullscreen: bool,
    pub focus: bool,
    pub hud_enabled: bool,
    pub coordinates: bool,
    pub hover_highlight: bool,
    pub board2d_flipped: bool,
    pub ribbon_collapsed: bool,
    /// Ids dos Painéis visíveis (encaixados ou flutuantes).
    pub visible_panels: BTreeSet<String>,
    pub has_docked_panel: bool,
    pub has_floating_panel: bool,
}

/// Estado de uma Ação num dado [`ActionContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionState {
    /// Id Fluent (`reason-*`) do motivo de estar desabilitada; `None` = habilitada.
    pub disabled_reason: Option<&'static str>,
    /// `Some` para alternâncias e escolhas; `None` para comandos.
    pub checked: Option<bool>,
}

impl ActionState {
    pub fn is_enabled(self) -> bool {
        self.disabled_reason.is_none()
    }
}

impl Action {
    /// A Ação só existe como gesto (fora do Ribbon, dos KeyTips e da Paleta).
    pub fn is_gesture_only(self) -> bool {
        meta(self).kind == ActionKind::Gesture
    }

    /// O atalho vale mesmo com um campo de texto em foco (teclas de função e Esc não digitam).
    pub fn fires_while_typing(self) -> bool {
        matches!(
            self,
            Action::Cancel | Action::ToggleFullscreen | Action::ToggleFocus
        )
    }

    /// Para as Ações que mostram ou ocultam um Painel: `(id do Painel, só fora da Partida
    /// competitiva em andamento)`. Os ids são os de `dock::ids` do app.
    pub fn panel(self) -> Option<(&'static str, bool)> {
        Some(match self {
            Action::PanelNotation => ("Notation", false),
            Action::PanelClock => ("Clock", false),
            Action::PanelCaptured => ("Captured", false),
            Action::PanelStudy => ("Study", true),
            Action::PanelAids => ("Aids", false),
            Action::PanelReview => ("Review", false),
            Action::PanelArbitration => ("Arbitration", false),
            Action::PanelTraining => ("Training", false),
            Action::PanelAnalysis => ("Analysis", true),
            Action::PanelLibrary => ("Library", true),
            Action::PanelOpenings => ("Openings", true),
            Action::PanelScoresheet => ("Scoresheet", false),
            Action::PanelBoard2d => ("Board2d", false),
            Action::PanelEngines => ("Engines", false),
            _ => return None,
        })
    }
}

const BOARD2D: &str = "Board2d";

fn view_of(action: Action) -> Option<CameraView> {
    match action {
        Action::ViewTop => Some(CameraView::Top),
        Action::ViewOblique => Some(CameraView::Oblique),
        Action::ViewStandard => Some(CameraView::Standard),
        Action::ViewInverted => Some(CameraView::Inverted),
        _ => None,
    }
}

/// O estado da Ação: habilitada ou desabilitada com motivo, e marcada ou não.
pub fn state(action: Action, ctx: &ActionContext) -> ActionState {
    ActionState {
        disabled_reason: disabled_reason(action, ctx),
        checked: checked(action, ctx),
    }
}

fn checked(action: Action, ctx: &ActionContext) -> Option<bool> {
    use Action as A;
    if let Some((id, _)) = action.panel() {
        return Some(ctx.visible_panels.contains(id));
    }
    if let Some(view) = view_of(action) {
        return Some(ctx.camera_view == Some(view));
    }
    Some(match action {
        A::PauseClock => ctx.paused,
        A::ToggleAnalysis => ctx.analysis_on,
        A::PresentProfessional => ctx.presentation == PresentationId::Professional3d,
        A::PresentImmersive => ctx.presentation == PresentationId::ImmersiveTable,
        A::PresentTechnical => ctx.presentation == PresentationId::Technical2d,
        A::ToggleFullscreen => ctx.fullscreen,
        A::ToggleFocus => ctx.focus,
        A::ToggleHud => ctx.hud_enabled,
        A::ToggleCoordinates => ctx.coordinates,
        A::ToggleHoverHighlight => ctx.hover_highlight,
        A::FlipBoard => ctx.board2d_flipped,
        A::ToggleRibbonCollapse => ctx.ribbon_collapsed,
        _ => return None,
    })
}

fn disabled_reason(action: Action, ctx: &ActionContext) -> Option<&'static str> {
    use Action as A;
    if let Some((_, outside_competitive)) = action.panel() {
        return (outside_competitive && ctx.competitive_in_progress)
            .then_some("reason-competitive-lock");
    }
    match action {
        A::Undo if ctx.competitive => Some("reason-assisted-only"),
        A::Undo if !ctx.can_undo => Some("reason-nothing-to-undo"),
        A::Redo if ctx.competitive => Some("reason-assisted-only"),
        A::Redo if !ctx.can_redo => Some("reason-nothing-to-redo"),
        A::Resign | A::OfferDraw | A::ClaimDraw | A::PauseClock | A::ClockPress
            if !ctx.game_running =>
        {
            Some("reason-no-game-running")
        }
        A::OfferDraw if ctx.draw_offer_pending => Some("reason-draw-offer-pending"),
        A::ClaimDraw if !ctx.claimable_draw => Some("reason-no-claimable-draw"),
        A::PauseClock | A::ClockPress if !ctx.has_clock => Some("reason-no-clock"),
        A::ToggleAnalysis if ctx.training_on_screen => Some("reason-training-lock"),
        A::ToggleAnalysis if ctx.competitive_in_progress => Some("reason-competitive-lock"),
        A::ToggleAnalysis if !ctx.engine_consented => Some("reason-engine-consent"),
        A::ViewTop
        | A::ViewOblique
        | A::ViewStandard
        | A::ViewInverted
        | A::ResetCamera
        | A::FrameBoard
            if ctx.presentation == PresentationId::Technical2d =>
        {
            Some("reason-3d-only")
        }
        A::FlipBoard if !ctx.visible_panels.contains(BOARD2D) => Some("reason-board2d-hidden"),
        A::FloatPanel if !ctx.has_docked_panel => Some("reason-no-docked-panel"),
        A::DockPanel if !ctx.has_floating_panel => Some("reason-no-floating-panel"),
        // Superfícies que os tickets seguintes da DS-0x entregam.
        A::DetachPanel | A::OpenPalette | A::OpenOptions | A::ToggleRibbonCollapse => {
            Some("reason-coming-soon")
        }
        _ => None,
    }
}
