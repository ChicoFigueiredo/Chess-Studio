//! Inventário do painel esquerdo (issue #164): para cada item que ele tinha, o destino na
//! interface nova. O painel foi removido na troca única (#167); o inventário ficou como o registro
//! do que foi migrado, conferido por testes: nenhum item sem destino e toda Ação alcançável no
//! Ribbon e na Paleta. (As `origin` abaixo são os nomes das funções do painel removido.)
//!
//! Destinos:
//! - [`Destination::Option`]: uma linha do diálogo Opções (preferência persistente).
//! - [`Destination::Actions`]: Ações da lista única (`chess_input::actions`), no Ribbon, na Paleta
//!   e no atalho.
//! - [`Destination::Surface`]: outra superfície da interface nova (barra de status).

use super::OptionId;
use chess_input::controls::Action;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Option(OptionId),
    Actions(&'static [Action]),
    Surface(&'static str),
}

/// Um item do painel esquerdo (ou preferência espalhada) e para onde ele vai.
#[derive(Clone, Copy, Debug)]
pub struct InventoryItem {
    /// Nome curto do item.
    pub name: &'static str,
    /// Onde ele vive hoje (função ou arquivo).
    pub origin: &'static str,
    /// Chaves i18n que o item usa hoje (a conferência com o código-fonte usa estas).
    pub keys: &'static [&'static str],
    pub dests: &'static [Destination],
}

use Destination::{Actions, Option as Opt, Surface};

const STATUS_BAR: &str = "Barra de status (#163)";

pub const INVENTORY: &[InventoryItem] = &[
    // --- ui.rs::info_panel ---
    InventoryItem {
        name: "Apresentação (3D, Mesa imersiva, 2D técnico)",
        origin: "ui.rs::info_panel",
        keys: &[],
        dests: &[Actions(&[
            Action::PresentProfessional,
            Action::PresentImmersive,
            Action::PresentTechnical,
            Action::CyclePresentation,
        ])],
    },
    InventoryItem {
        name: "Qualidade gráfica",
        origin: "ui.rs::info_panel",
        keys: &["quality-label"],
        dests: &[Opt(OptionId::Quality)],
    },
    InventoryItem {
        name: "Efeitos extras (bloom)",
        origin: "ui.rs::info_panel",
        keys: &[
            "quality-effects",
            "quality-effects-hint",
            "quality-effects-unavailable",
        ],
        dests: &[Opt(OptionId::Effects)],
    },
    InventoryItem {
        name: "Realce ao passar o mouse",
        origin: "ui.rs::info_panel",
        keys: &["hover-highlight", "hover-highlight-hint"],
        dests: &[
            Opt(OptionId::Hover),
            Actions(&[Action::ToggleHoverHighlight]),
        ],
    },
    InventoryItem {
        name: "Coordenadas no tabuleiro",
        origin: "ui.rs::info_panel",
        keys: &["board-coordinates", "board-coordinates-hint"],
        dests: &[
            Opt(OptionId::Coordinates),
            Actions(&[Action::ToggleCoordinates]),
        ],
    },
    InventoryItem {
        name: "Casa sob o cursor",
        origin: "ui.rs::info_panel",
        keys: &["square-hover"],
        dests: &[Surface(STATUS_BAR)],
    },
    InventoryItem {
        name: "Vista da câmera atual",
        origin: "ui.rs::info_panel",
        keys: &["view-free", "view-label"],
        dests: &[Surface(STATUS_BAR), Opt(OptionId::CameraView)],
    },
    InventoryItem {
        name: "Resumo de atalhos (help_lines)",
        origin: "ui.rs::info_panel",
        keys: &[],
        dests: &[Opt(OptionId::ControlsSummary)],
    },
    InventoryItem {
        name: "Título da janela",
        origin: "ui.rs::info_panel",
        keys: &["app-title"],
        dests: &[Surface("Barra de título e Ribbon (#162)")],
    },
    // --- ui.rs::game_section ---
    InventoryItem {
        name: "Estado da Partida (vez, resultado, último Lance)",
        origin: "ui.rs::game_section",
        keys: &[
            "game-heading",
            "game-over",
            "game-turn-white",
            "game-turn-black",
            "scoresheet-last-move-hidden",
            "game-last-move",
            "game-no-moves",
        ],
        dests: &[Surface(STATUS_BAR), Actions(&[Action::PanelNotation])],
    },
    InventoryItem {
        name: "Desfazer, Refazer e Abandonar",
        origin: "ui.rs::game_section",
        keys: &["button-undo", "button-redo", "button-resign"],
        dests: &[Actions(&[Action::Undo, Action::Redo, Action::Resign])],
    },
    InventoryItem {
        name: "Nova Partida",
        origin: "ui.rs::game_section",
        keys: &["button-new-game"],
        dests: &[Actions(&[Action::NewGame])],
    },
    InventoryItem {
        name: "Método de movimento",
        origin: "ui.rs::game_section",
        keys: &["move-method-label"],
        dests: &[Opt(OptionId::MoveMethod)],
    },
    // --- dock.rs::panels_section ---
    InventoryItem {
        name: "Painéis visíveis (caixas de seleção)",
        origin: "dock.rs::panels_section",
        keys: &["dock-heading"],
        dests: &[Actions(&[
            Action::PanelNotation,
            Action::PanelClock,
            Action::PanelCaptured,
            Action::PanelStudy,
            Action::PanelAids,
            Action::PanelReview,
            Action::PanelArbitration,
            Action::PanelTraining,
            Action::PanelAnalysis,
            Action::PanelLibrary,
            Action::PanelOpenings,
            Action::PanelExplorer,
            Action::PanelScoresheet,
            Action::PanelBoard2d,
            Action::PanelEngines,
        ])],
    },
    InventoryItem {
        name: "Ocultar todos os Painéis (Ctrl+Shift+F12)",
        origin: "actions.rs::toggle_hide_all",
        keys: &[],
        dests: &[Actions(&[Action::HideAllPanels])],
    },
    InventoryItem {
        name: "Restaurar layout padrão",
        origin: "dock.rs::panels_section",
        keys: &["dock-reset"],
        dests: &[Actions(&[Action::ResetLayout])],
    },
    // --- focus.rs::view_section ---
    InventoryItem {
        name: "Tela cheia e modo Foco",
        origin: "focus.rs::view_section",
        keys: &["view-heading", "view-fullscreen", "view-focus"],
        dests: &[Actions(&[Action::ToggleFullscreen, Action::ToggleFocus])],
    },
    // --- profiles.rs::profiles_section ---
    InventoryItem {
        name: "Perfil de interface (escolher, restaurar, salvar, excluir)",
        origin: "profiles.rs::profiles_section",
        keys: &[
            "iprofile-heading",
            "iprofile-custom",
            "iprofile-restore",
            "iprofile-restore-hint",
            "iprofile-delete",
            "iprofile-name-hint",
            "iprofile-save",
            "iprofile-error-empty",
            "iprofile-error-long",
        ],
        dests: &[Opt(OptionId::InterfaceProfile)],
    },
    // --- local_profiles.rs::section ---
    InventoryItem {
        name: "Perfil local (trocar e gerenciar)",
        origin: "local_profiles.rs::section",
        keys: &["lp-heading", "lp-current", "lp-busy", "lp-manage"],
        dests: &[
            Opt(OptionId::LocalProfile),
            Opt(OptionId::LocalProfileManage),
        ],
    },
    // --- controls.rs::controls_section ---
    InventoryItem {
        name: "Controles…",
        origin: "controls.rs::controls_section",
        keys: &["ctl-open"],
        dests: &[
            Opt(OptionId::ControlsOpen),
            Actions(&[Action::OpenControls]),
        ],
    },
    // --- engine.rs::engine_section ---
    InventoryItem {
        name: "Engine adversária: lado e Elo",
        origin: "engine.rs::engine_section",
        keys: &[
            "engine-side-label",
            "engine-side-white",
            "engine-side-black",
            "engine-elo-label",
        ],
        dests: &[Opt(OptionId::EngineSide), Opt(OptionId::EngineElo)],
    },
    InventoryItem {
        name: "Engine adversária: caminho do executável e assistente",
        origin: "engine.rs::engine_section",
        keys: &[
            "engine-path-label",
            "engine-path-hint",
            "engine-not-found",
            "wizard-open",
        ],
        dests: &[Opt(OptionId::EngineManager), Opt(OptionId::EngineWizard)],
    },
    InventoryItem {
        name: "Engine adversária: Iniciar e Retomar",
        origin: "engine.rs::engine_section",
        keys: &[
            "engine-start",
            "engine-resume",
            "engine-pending-title",
            "engine-pending-hint",
        ],
        dests: &[Actions(&[Action::NewGame])],
    },
    InventoryItem {
        name: "Engine adversária: título da seção",
        origin: "engine.rs::engine_section",
        keys: &["engine-heading"],
        dests: &[Surface(STATUS_BAR)],
    },
    InventoryItem {
        name: "Botão Opções… (novo, abre este diálogo)",
        origin: "ui.rs::info_panel",
        keys: &["options-open-button"],
        dests: &[Actions(&[Action::OpenOptions])],
    },
    // --- preferências espalhadas fora do painel esquerdo ---
    InventoryItem {
        name: "Tema, Densidade e Escala",
        origin: "ui_theme (só na galeria até agora)",
        keys: &[],
        dests: &[
            Opt(OptionId::Theme),
            Opt(OptionId::Density),
            Opt(OptionId::Scale),
            Actions(&[Action::UiZoomIn, Action::UiZoomOut]),
        ],
    },
    InventoryItem {
        name: "Perfil de Auxílios, Auxílios visuais e Explicação de erro",
        origin: "aids_panel.rs (painel Auxílios do dock)",
        keys: &[],
        dests: &[
            Opt(OptionId::AidProfile),
            Opt(OptionId::AidDestinations),
            Opt(OptionId::AidGhosts),
            Opt(OptionId::AidSelected),
            Opt(OptionId::AidLastMove),
            Opt(OptionId::AidCheck),
            Opt(OptionId::AidExplanation),
        ],
    },
    InventoryItem {
        name: "HUD: chave geral e Perfil de HUD",
        origin: "aids_panel.rs (painel Auxílios do dock)",
        keys: &[],
        dests: &[
            Opt(OptionId::HudEnabled),
            Opt(OptionId::HudProfile),
            Actions(&[Action::ToggleHud]),
        ],
    },
    InventoryItem {
        name: "Promover sempre a Dama",
        origin: "ui.rs::promotion_selector (janela de promoção)",
        keys: &[],
        dests: &[Opt(OptionId::AlwaysQueen)],
    },
    InventoryItem {
        name: "Girar o tabuleiro 2D",
        origin: "board2d_panel.rs",
        keys: &[],
        dests: &[Actions(&[Action::FlipBoard])],
    },
];
