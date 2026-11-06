//! Intérprete de gestos: máquina de estados pura (sem Bevy) que traduz eventos abstratos
//! de ponteiro, teclado e foco em [`Command`]s da Partida e num estado visual.
//!
//! Estados (prompt mestre §7.3): `Idle -> Selected -> Held / Dragging -> ReleasedPending`.
//! O intérprete nunca muta a Partida (ADR-0001): só lê o [`Snapshot`] e emite intenções.
//! Um destino nunca é "corrigido": o que o usuário soltou é o que vai no `Release`.

use chess_core::Square;
use chess_game::{Command, Snapshot};
use glam::Vec3;

/// Método de movimento (preferência do usuário).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MoveMethod {
    /// Clique na origem, clique no destino.
    TwoTaps,
    /// Pressionar, arrastar e soltar.
    Drag,
    /// Os dois (padrão).
    #[default]
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Middle,
    Secondary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub alt: bool,
    pub shift: bool,
    pub ctrl: bool,
}

/// Evento abstrato de entrada. `square` é a casa sob o cursor (`None` fora do tabuleiro);
/// `world` é o ponto do plano do tabuleiro sob o cursor, usado só para desenhar o arrasto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureEvent {
    PointerDown {
        button: PointerButton,
        square: Option<Square>,
        modifiers: Modifiers,
    },
    PointerMove {
        square: Option<Square>,
        world: Vec3,
    },
    PointerUp {
        square: Option<Square>,
    },
    Escape,
    FocusLost,
    /// Ponteiro interagiu com um painel (egui).
    PanelInteraction,
    /// A câmera começou a orbitar (ou fazer pan).
    OrbitStarted,
    /// A Partida mudou por fora (desfazer, nova Partida...): revalida a seleção.
    Resync,
    /// O app terminou de tratar o `Release` emitido (sai de `ReleasedPending`).
    Resolved,
}

/// Estado da máquina.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureState {
    Idle,
    /// Peça escolhida por clique, esperando o destino.
    Selected { from: Square },
    /// Botão pressionado sobre a peça, ainda sem arrastar. `was_selected`: o clique
    /// fecha uma seleção anterior (alterna).
    Held { from: Square, was_selected: bool },
    Dragging { from: Square, world: Vec3 },
    /// `Release` enviado; a peça fica onde foi solta até o app resolver.
    ReleasedPending { from: Square, to: Square },
}

/// O que o desenho da cena precisa saber do gesto em curso.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GestureVisual {
    /// Casa de origem da peça selecionada, segurada ou arrastada.
    pub origin: Option<Square>,
    /// Peça elevada (selecionada, segurada ou arrastada).
    pub lifted: bool,
    /// Posição do arrasto no plano do tabuleiro; `Some` só durante o arrasto.
    pub drag_world: Option<Vec3>,
}

#[derive(Clone, Debug)]
pub struct GestureInterpreter {
    method: MoveMethod,
    state: GestureState,
}

impl GestureInterpreter {
    pub fn new(method: MoveMethod) -> Self {
        Self {
            method,
            state: GestureState::Idle,
        }
    }

    pub fn method(&self) -> MoveMethod {
        self.method
    }

    /// Troca o método; um gesto em curso é cancelado.
    pub fn set_method(&mut self, method: MoveMethod) {
        self.method = method;
        self.state = GestureState::Idle;
    }

    pub fn state(&self) -> GestureState {
        self.state
    }

    pub fn visual(&self) -> GestureVisual {
        GestureVisual::default()
    }

    /// Processa um evento e devolve os Comandos a enviar à Partida (em ordem).
    pub fn handle(&mut self, event: GestureEvent, snapshot: &Snapshot) -> Vec<Command> {
        let _ = (event, snapshot);
        Vec::new()
    }
}
