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
    Selected {
        from: Square,
    },
    /// Botão pressionado sobre a peça, ainda sem arrastar. `was_selected`: o clique
    /// fecha uma seleção anterior (alterna).
    Held {
        from: Square,
        was_selected: bool,
    },
    Dragging {
        from: Square,
        world: Vec3,
    },
    /// `Release` enviado; a peça fica onde foi solta até o app resolver.
    ReleasedPending {
        from: Square,
        to: Square,
    },
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
        match self.state {
            GestureState::Selected { from } | GestureState::Held { from, .. } => GestureVisual {
                origin: Some(from),
                lifted: true,
                drag_world: None,
            },
            GestureState::Dragging { from, world } => GestureVisual {
                origin: Some(from),
                lifted: true,
                drag_world: Some(world),
            },
            GestureState::Idle | GestureState::ReleasedPending { .. } => GestureVisual::default(),
        }
    }

    /// Processa um evento e devolve os Comandos a enviar à Partida (em ordem).
    pub fn handle(&mut self, event: GestureEvent, snapshot: &Snapshot) -> Vec<Command> {
        if event == GestureEvent::Resolved {
            if matches!(self.state, GestureState::ReleasedPending { .. }) {
                self.state = GestureState::Idle;
            }
            return Vec::new();
        }
        // Seletor de promoção aberto (modal): só Esc age, e retrata o Lance (DEC-40).
        if snapshot.provisional.is_some() {
            self.state = GestureState::Idle;
            return match event {
                GestureEvent::Escape => vec![Command::RetractMove],
                _ => Vec::new(),
            };
        }
        // A seleção só vale enquanto a peça continua sendo do lado da vez.
        if let GestureState::Selected { from }
        | GestureState::Held { from, .. }
        | GestureState::Dragging { from, .. } = self.state
            && !own_piece(snapshot, from)
        {
            self.state = GestureState::Idle;
        }

        match event {
            GestureEvent::Escape
            | GestureEvent::FocusLost
            | GestureEvent::PanelInteraction
            | GestureEvent::OrbitStarted => {
                // Um Release já enviado não se cancela aqui: a Partida decide.
                if !matches!(self.state, GestureState::ReleasedPending { .. }) {
                    self.state = GestureState::Idle;
                }
                Vec::new()
            }
            GestureEvent::Resync | GestureEvent::Resolved => Vec::new(),
            GestureEvent::PointerDown { button, square, .. } => {
                self.pointer_down(button, square, snapshot)
            }
            GestureEvent::PointerMove { world, square } => {
                self.pointer_move(square, world);
                Vec::new()
            }
            GestureEvent::PointerUp { square } => self.pointer_up(square),
        }
    }

    fn pointer_down(
        &mut self,
        button: PointerButton,
        square: Option<Square>,
        snapshot: &Snapshot,
    ) -> Vec<Command> {
        if matches!(self.state, GestureState::ReleasedPending { .. }) {
            return Vec::new();
        }
        if button != PointerButton::Primary {
            // Botão do meio/direito nunca move peça; no meio de um arrasto é órbita/pan.
            if matches!(
                self.state,
                GestureState::Held { .. } | GestureState::Dragging { .. }
            ) {
                self.state = GestureState::Idle;
            }
            return Vec::new();
        }
        let Some(target) = square else {
            self.state = GestureState::Idle;
            return Vec::new();
        };
        let own = own_piece(snapshot, target);
        let taps = self.method == MoveMethod::TwoTaps;
        match self.state {
            GestureState::Selected { from } if from == target => {
                self.state = if taps {
                    GestureState::Idle
                } else {
                    GestureState::Held {
                        from,
                        was_selected: true,
                    }
                };
                Vec::new()
            }
            GestureState::Selected { from } if !own => {
                // O destino é o que o usuário apontou, mesmo ilegal (INT-02): a Partida decide.
                self.state = GestureState::ReleasedPending { from, to: target };
                vec![Command::Release { from, to: target }]
            }
            _ if own => {
                self.state = if taps {
                    GestureState::Selected { from: target }
                } else {
                    GestureState::Held {
                        from: target,
                        was_selected: false,
                    }
                };
                Vec::new()
            }
            _ => {
                self.state = GestureState::Idle;
                Vec::new()
            }
        }
    }

    fn pointer_move(&mut self, square: Option<Square>, world: Vec3) {
        match self.state {
            GestureState::Held { from, .. } if square != Some(from) => {
                self.state = GestureState::Dragging { from, world };
            }
            GestureState::Dragging { from, .. } => {
                self.state = GestureState::Dragging { from, world };
            }
            _ => {}
        }
    }

    fn pointer_up(&mut self, square: Option<Square>) -> Vec<Command> {
        match self.state {
            GestureState::Held { from, was_selected } => {
                // Clique sem arrastar: seleciona (ou fecha a seleção) só se o método admite.
                self.state = if self.method == MoveMethod::Both && !was_selected {
                    GestureState::Selected { from }
                } else {
                    GestureState::Idle
                };
                Vec::new()
            }
            GestureState::Dragging { from, .. } => match square {
                Some(to) if to != from => {
                    self.state = GestureState::ReleasedPending { from, to };
                    vec![Command::Release { from, to }]
                }
                // Soltou na origem ou fora do tabuleiro: cancela, sem comando.
                _ => {
                    self.state = GestureState::Idle;
                    Vec::new()
                }
            },
            _ => Vec::new(),
        }
    }
}

/// Casa com peça do lado da vez, numa Partida em andamento.
fn own_piece(snapshot: &Snapshot, square: Square) -> bool {
    snapshot.outcome.is_none()
        && snapshot
            .position
            .piece_at(square)
            .is_some_and(|(color, _)| color == snapshot.side_to_move)
}
