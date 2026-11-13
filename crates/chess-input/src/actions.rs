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
        // Início: grupo Nova Partida, o primeiro da aba (a ordem segue `Action::ALL`).
        A::NewGame => item!("new-game", "file-plus", Command, Home, "new-game", "N"),
        A::NewGameVsEngine => item!("new-game-vs-engine", "cpu", Command, Home, "new-game", "NE"),
        A::NewGameTwoPlayers => item!(
            "new-game-two-players",
            "users",
            Command,
            Home,
            "new-game",
            "ND"
        ),
        A::ResumeGame => item!(
            "resume-game",
            "clock-counter-clockwise",
            Command,
            Home,
            "new-game",
            "NR"
        ),
        A::NewGameOnline => item!("new-game-online", "globe", Command, Home, "new-game", "NO"),
        A::OpenControls => item!(
            "open-controls",
            "keyboard",
            Command,
            File,
            "preferences",
            "C"
        ),
        A::OpenOptions => item!("open-options", "gear", Command, File, "preferences", "O"),
        A::OpenConnections => item!(
            "open-connections",
            "plugs-connected",
            Command,
            File,
            "connections",
            "X"
        ),
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
        // Início: grupo Online (Lichess), depois do relógio.
        A::SeekOnline => item!(
            "seek-online",
            "magnifying-glass",
            Command,
            Home,
            "online",
            "OB"
        ),
        A::ChallengeOnline => item!(
            "challenge-online",
            "user-plus",
            Command,
            Home,
            "online",
            "OD"
        ),
        A::PlayLichessEngine => item!("play-lichess-engine", "cpu", Command, Home, "online", "OE"),
        A::OnlineGames => item!(
            "online-games",
            "list-bullets",
            Command,
            Home,
            "online",
            "OP"
        ),
        A::OpenEngineOnline => item!("open-engine-online", "robot", Command, Home, "online", "OO"),
        // Análise.
        A::ToggleAnalysis => item!(
            "toggle-analysis",
            "chart-line",
            Toggle,
            Analysis,
            "engine",
            "A"
        ),
        // Explorador: aberturas e finais, no grupo próprio da aba Análise.
        A::OpenExplorer => item!(
            "open-explorer",
            "compass",
            Command,
            Analysis,
            "explorer",
            "X"
        ),
        // Biblioteca e Treino: Ações de abrir o que já existe (o Painel é alternado em Exibir).
        A::OpenLibrary => item!("open-library", "books", Command, Library, "library", "B"),
        A::OpenOpenings => item!(
            "open-openings",
            "tree-structure",
            Command,
            Library,
            "library",
            "A"
        ),
        A::ImportOnline => item!(
            "import-online",
            "cloud-arrow-down",
            Command,
            Library,
            "library",
            "I"
        ),
        A::ExportPgn => item!("export-pgn", "export", Command, Library, "library", "E"),
        A::OpenTraining => item!(
            "open-training",
            "barbell",
            Command,
            Training,
            "training",
            "T"
        ),
        // Os Painéis (Análise, Estudo, Engines, Biblioteca, Aberturas, Treino…) não repetem
        // botões nas abas de domínio: moram só no grupo Painéis da aba Exibir (#183).
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
        A::ToggleTableClock => item!("toggle-table-clock", "clock", Toggle, View, "board", "BL"),
        A::ToggleTableCaptured => item!(
            "toggle-table-captured",
            "stack",
            Toggle,
            View,
            "board",
            "BT"
        ),
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
        A::PanelAnalysis => item!("panel-analysis", "chart-bar", Toggle, View, "panels", "NY"),
        A::PanelStudy => item!("panel-study", "student", Toggle, View, "panels", "NE"),
        A::PanelEngines => item!("panel-engines", "cpu", Toggle, View, "panels", "NM"),
        A::PanelLibrary => item!("panel-library", "books", Toggle, View, "panels", "NL"),
        A::PanelOpenings => item!(
            "panel-openings",
            "tree-structure",
            Toggle,
            View,
            "panels",
            "NO"
        ),
        A::PanelExplorer => item!("panel-explorer", "compass", Toggle, View, "panels", "NX"),
        A::PanelTraining => item!("panel-training", "barbell", Toggle, View, "panels", "NT"),
        A::PanelOnline => item!("panel-online", "globe", Toggle, View, "panels", "NI"),
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
        A::DetachGroup => item!(
            "detach-group",
            "arrow-square-out",
            Command,
            View,
            "windows",
            "JG"
        ),
        A::ResetLayout => item!("reset-layout", "layout", Command, View, "windows", "JR"),
        A::HideAllPanels => item!(
            "hide-all-panels",
            "eye-slash",
            Toggle,
            View,
            "windows",
            "JH"
        ),
        // Exibir: interface.
        A::OpenPalette => item!("open-palette", "command", Command, View, "interface", "IP"),
        A::UiZoomIn => item!(
            "ui-zoom-in",
            "magnifying-glass-plus",
            Command,
            View,
            "interface",
            "IA"
        ),
        A::UiZoomOut => item!(
            "ui-zoom-out",
            "magnifying-glass-minus",
            Command,
            View,
            "interface",
            "ID"
        ),
        A::UiZoomReset => item!(
            "ui-zoom-reset",
            "magnifying-glass",
            Command,
            View,
            "interface",
            "IZ"
        ),
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
    /// O relógio 3D está sobre a mesa (aba Tabuleiro).
    pub table_clock: bool,
    /// As peças tomadas aparecem em 3D ao lado do tabuleiro (aba Tabuleiro).
    pub table_captured: bool,
    pub hover_highlight: bool,
    /// O tabuleiro está virado (Ação `FlipBoard`: Vista 3D, 2D técnico e Tabuleiro 2D).
    pub board_flipped: bool,
    pub ribbon_collapsed: bool,
    /// O Ribbon está na tela; sem ele (modo Foco), recolher não faz sentido.
    pub ribbon_visible: bool,
    /// Ids dos Painéis visíveis (encaixados ou flutuantes).
    pub visible_panels: BTreeSet<String>,
    pub has_docked_panel: bool,
    /// Há Painel flutuante ou janela destacada para encaixar.
    pub has_floating_panel: bool,
    /// "Ocultar todos os Painéis" está ativo (há um conjunto guardado para restaurar).
    pub panels_hidden: bool,
    /// Há outra Partida não terminada no banco para retomar.
    pub resumable_game: bool,
    /// A Súmula está aberta e esconde a notação: exportar PGN a revelaria.
    pub pgn_blocked: bool,
    /// A Partida no tabuleiro é uma Partida online (o Provedor decide relógio e resultado).
    pub online: bool,
    /// Há uma Partida online em tempo real contra pessoa em andamento (outra não começa).
    pub online_realtime_in_progress: bool,
    /// A Trava de fair play recusa a Engine adversária agora (Partida online contra pessoa em
    /// andamento, no tabuleiro ou fora dele): "Contra a Engine" espera.
    pub fair_play_engine_lock: bool,
    /// Modo espectador: o tabuleiro mostra a Partida da Engine online só para ver; as Ações de
    /// jogo (abandonar, empate, relógio, desfazer) não valem nela.
    pub spectator: bool,
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
            Action::Cancel
                | Action::ToggleFullscreen
                | Action::ToggleFocus
                // Ctrl+Shift+F12 também não digita.
                | Action::HideAllPanels
                // Ctrl+, não digita nada: vale também de dentro da Paleta (campo de busca).
                | Action::OpenOptions
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
            // Sempre acessível: numa Partida competitiva o Painel explica a Trava de fair play.
            Action::PanelExplorer => ("Explorer", false),
            Action::PanelScoresheet => ("Scoresheet", false),
            Action::PanelBoard2d => ("Board2d", false),
            Action::PanelEngines => ("Engines", false),
            Action::PanelOnline => ("Online", false),
            _ => return None,
        })
    }
}

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
        A::HideAllPanels => ctx.panels_hidden,
        A::ToggleHud => ctx.hud_enabled,
        A::ToggleCoordinates => ctx.coordinates,
        A::ToggleTableClock => ctx.table_clock,
        A::ToggleTableCaptured => ctx.table_captured,
        A::ToggleHoverHighlight => ctx.hover_highlight,
        A::FlipBoard => ctx.board_flipped,
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
        A::OpenLibrary | A::OpenOpenings | A::ImportOnline if ctx.competitive_in_progress => {
            Some("reason-competitive-lock")
        }
        A::ResumeGame if !ctx.resumable_game => Some("reason-no-resumable-game"),
        A::Resign
        | A::OfferDraw
        | A::ClaimDraw
        | A::PauseClock
        | A::ClockPress
        | A::Undo
        | A::Redo
            if ctx.spectator =>
        {
            Some("reason-spectator")
        }
        // A Engine adversária calcula: com a Trava de fair play ligada, não começa (FP-01).
        A::NewGameVsEngine if ctx.online_realtime_in_progress || ctx.fair_play_engine_lock => {
            Some("reason-fair-play-engine")
        }
        A::NewGameOnline | A::SeekOnline | A::ChallengeOnline | A::PlayLichessEngine
            if ctx.online_realtime_in_progress =>
        {
            Some("reason-online-in-progress")
        }
        A::NewGameOnline | A::SeekOnline | A::ChallengeOnline | A::PlayLichessEngine
            if ctx.competitive_in_progress && !ctx.online =>
        {
            Some("reason-competitive-lock")
        }
        // Online, relógio e Pausa são do Provedor (PO-07).
        A::PauseClock | A::ClockPress if ctx.online => Some("reason-online-provider"),
        A::ExportPgn if ctx.pgn_blocked => Some("reason-pgn-scoresheet"),
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
        A::FloatPanel | A::DetachPanel | A::DetachGroup if !ctx.has_docked_panel => {
            Some("reason-no-docked-panel")
        }
        A::DockPanel if !ctx.has_floating_panel => Some("reason-no-floating-panel"),
        // Superfícies que os tickets seguintes da DS-0x entregam.
        A::ToggleRibbonCollapse if ctx.ribbon_visible => None,
        A::ToggleRibbonCollapse => Some("reason-coming-soon"),
        _ => None,
    }
}
