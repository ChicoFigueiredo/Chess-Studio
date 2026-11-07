//! Mapa de controles (US-28, DEC-51): a tabela única **ação -> atalhos** consultada por todos
//! os sistemas de entrada do app. É dado puro (sem Bevy): quais gestos de mouse, trackpad
//! e teclado disparam cada [`Action`], os padrões da DEC-51, a detecção de conflito entre
//! ações e a (de)serialização em texto estável para gravar por perfil de usuário.
//!
//! Regras:
//! - Cada ação tem uma lista de [`Trigger`]s (a órbita, por exemplo, tem o botão do meio e o
//!   arrasto de dois dedos). Um mesmo [`Trigger`] não pode servir a duas ações (conflito).
//! - A busca ([`ControlMap::resolve`]) escolhe o atalho **mais específico** cujos
//!   modificadores estejam todos apertados: com Alt+Esquerdo apertado vale o Ajuste, não
//!   a seleção; com Shift+Esquerdo (que ninguém usa) vale a seleção.
//! - Os nomes de tecla são os de `bevy::KeyCode` (`Digit1`, `F11`, `Tab`...); este módulo
//!   não os interpreta.

use crate::Modifiers;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// O que o usuário pode fazer e que é remapeável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// Selecionar / tocar / arrastar peça.
    Select,
    /// Ajuste (j'adoube), só nas Políticas competitivas.
    Adjust,
    /// Órbita 360° da câmera.
    Orbit,
    /// Pan da câmera.
    Pan,
    /// Zoom da câmera (com limites).
    Zoom,
    /// Cancela o gesto não concluído.
    Cancel,
    /// Aciona o relógio.
    ClockPress,
    ViewTop,
    ViewOblique,
    ViewStandard,
    ViewInverted,
    /// Reset da câmera (Vista padrão).
    ResetCamera,
    /// Alterna a Apresentação 3D profissional / Mesa imersiva / 2D técnico.
    CyclePresentation,
    ToggleFullscreen,
    ToggleFocus,
    /// Liga e desliga o HUD inteiro.
    ToggleHud,
}

impl Action {
    pub const ALL: [Action; 16] = [
        Action::Select,
        Action::Adjust,
        Action::Orbit,
        Action::Pan,
        Action::Zoom,
        Action::Cancel,
        Action::ClockPress,
        Action::ViewTop,
        Action::ViewOblique,
        Action::ViewStandard,
        Action::ViewInverted,
        Action::ResetCamera,
        Action::CyclePresentation,
        Action::ToggleFullscreen,
        Action::ToggleFocus,
        Action::ToggleHud,
    ];

    /// Identificador estável (gravado no banco; nunca muda).
    pub fn id(self) -> &'static str {
        match self {
            Action::Select => "select",
            Action::Adjust => "adjust",
            Action::Orbit => "orbit",
            Action::Pan => "pan",
            Action::Zoom => "zoom",
            Action::Cancel => "cancel",
            Action::ClockPress => "clock-press",
            Action::ViewTop => "view-top",
            Action::ViewOblique => "view-oblique",
            Action::ViewStandard => "view-standard",
            Action::ViewInverted => "view-inverted",
            Action::ResetCamera => "reset-camera",
            Action::CyclePresentation => "cycle-presentation",
            Action::ToggleFullscreen => "toggle-fullscreen",
            Action::ToggleFocus => "toggle-focus",
            Action::ToggleHud => "toggle-hud",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Se o tipo de entrada serve a esta ação.
    pub fn accepts(self, input: &Input) -> bool {
        match self {
            Action::Select | Action::Adjust => matches!(input, Input::Mouse(_)),
            Action::Orbit | Action::Pan => matches!(
                input,
                Input::Mouse(_) | Input::Gesture(GestureKind::TwoFinger)
            ),
            Action::Zoom => matches!(input, Input::Wheel | Input::Gesture(GestureKind::Pinch)),
            _ => matches!(input, Input::Key(_)),
        }
    }

    /// A ação que o app não pode ficar sem: sem atalho, ninguém joga.
    fn needs_a_trigger(self) -> bool {
        self == Action::Select
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseBtn {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

impl MouseBtn {
    pub const ALL: [MouseBtn; 5] = [
        MouseBtn::Left,
        MouseBtn::Middle,
        MouseBtn::Right,
        MouseBtn::Back,
        MouseBtn::Forward,
    ];

    fn name(self) -> &'static str {
        match self {
            MouseBtn::Left => "Left",
            MouseBtn::Middle => "Middle",
            MouseBtn::Right => "Right",
            MouseBtn::Back => "Back",
            MouseBtn::Forward => "Forward",
        }
    }
}

/// Gestos de trackpad. Em Linux/Windows os dois dedos chegam como roda em pixels; o app
/// os trata como o mesmo [`GestureKind::TwoFinger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GestureKind {
    /// Dois dedos deslizando.
    TwoFinger,
    /// Pinça.
    Pinch,
}

/// A parte "sem modificador" de um atalho.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Input {
    /// Tecla, pelo nome do `KeyCode` (`Digit1`, `F11`, `Tab`...).
    Key(String),
    Mouse(MouseBtn),
    /// Roda do mouse (passos de linha).
    Wheel,
    Gesture(GestureKind),
}

/// Um atalho: entrada + modificadores que precisam estar apertados.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Trigger {
    pub input: Input,
    pub mods: ModSet,
}

/// [`Modifiers`] ordenável/comparável (o tipo do intérprete não é `Ord`/`Hash`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModSet {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ModSet {
    pub const NONE: ModSet = ModSet {
        ctrl: false,
        shift: false,
        alt: false,
    };
    pub const SHIFT: ModSet = ModSet {
        ctrl: false,
        shift: true,
        alt: false,
    };
    pub const ALT: ModSet = ModSet {
        ctrl: false,
        shift: false,
        alt: true,
    };

    pub fn count(self) -> usize {
        usize::from(self.ctrl) + usize::from(self.shift) + usize::from(self.alt)
    }

    /// Todos os modificadores de `self` estão em `held`.
    pub fn is_held_by(self, held: ModSet) -> bool {
        (!self.ctrl || held.ctrl) && (!self.shift || held.shift) && (!self.alt || held.alt)
    }
}

impl From<Modifiers> for ModSet {
    fn from(m: Modifiers) -> Self {
        Self {
            ctrl: m.ctrl,
            shift: m.shift,
            alt: m.alt,
        }
    }
}

impl From<ModSet> for Modifiers {
    fn from(m: ModSet) -> Self {
        Self {
            ctrl: m.ctrl,
            shift: m.shift,
            alt: m.alt,
        }
    }
}

impl Trigger {
    pub fn key(name: &str) -> Self {
        Self::new(Input::Key(name.to_owned()), ModSet::NONE)
    }

    pub fn mouse(button: MouseBtn) -> Self {
        Self::new(Input::Mouse(button), ModSet::NONE)
    }

    pub fn new(input: Input, mods: ModSet) -> Self {
        Self { input, mods }
    }

    pub fn with_mods(mut self, mods: ModSet) -> Self {
        self.mods = mods;
        self
    }
}

impl fmt::Display for Trigger {
    /// Formato estável: `Ctrl+Shift+Alt+Key:Digit1`, `Mouse:Middle`, `Wheel`,
    /// `Shift+Gesture:TwoFinger`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mods.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.mods.shift {
            f.write_str("Shift+")?;
        }
        if self.mods.alt {
            f.write_str("Alt+")?;
        }
        match &self.input {
            Input::Key(name) => write!(f, "Key:{name}"),
            Input::Mouse(b) => write!(f, "Mouse:{}", b.name()),
            Input::Wheel => f.write_str("Wheel"),
            Input::Gesture(GestureKind::TwoFinger) => f.write_str("Gesture:TwoFinger"),
            Input::Gesture(GestureKind::Pinch) => f.write_str("Gesture:Pinch"),
        }
    }
}

/// Texto de atalho ilegível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTriggerError(pub String);

impl fmt::Display for ParseTriggerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "atalho ilegível: `{}`", self.0)
    }
}

impl std::error::Error for ParseTriggerError {}

impl FromStr for Trigger {
    type Err = ParseTriggerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || ParseTriggerError(text.to_owned());
        let mut rest = text;
        let mut mods = ModSet::NONE;
        loop {
            if let Some(r) = rest.strip_prefix("Ctrl+") {
                mods.ctrl = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("Shift+") {
                mods.shift = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("Alt+") {
                mods.alt = true;
                rest = r;
            } else {
                break;
            }
        }
        let input = if rest == "Wheel" {
            Input::Wheel
        } else if let Some(name) = rest.strip_prefix("Key:") {
            if name.is_empty() {
                return Err(bad());
            }
            Input::Key(name.to_owned())
        } else if let Some(name) = rest.strip_prefix("Mouse:") {
            Input::Mouse(
                MouseBtn::ALL
                    .into_iter()
                    .find(|b| b.name() == name)
                    .ok_or_else(bad)?,
            )
        } else if let Some(name) = rest.strip_prefix("Gesture:") {
            match name {
                "TwoFinger" => Input::Gesture(GestureKind::TwoFinger),
                "Pinch" => Input::Gesture(GestureKind::Pinch),
                _ => return Err(bad()),
            }
        } else {
            return Err(bad());
        };
        Ok(Trigger { input, mods })
    }
}

/// Por que um atalho não pôde ser atribuído.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignError {
    /// Este tipo de entrada não serve para a ação (tecla para órbita, por exemplo).
    Unsupported,
    /// O atalho já é de outra ação.
    Conflict(Action),
    /// A ação ficaria sem nenhum atalho e precisa de um.
    WouldLeaveEmpty,
}

/// Como o novo atalho entra na lista da ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignMode {
    /// Substitui todos os atalhos da ação.
    Replace,
    /// Acrescenta aos existentes.
    Add,
}

/// A tabela ação -> atalhos. Toda ação sempre está presente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlMap {
    bindings: BTreeMap<Action, Vec<Trigger>>,
}

fn defaults_for(action: Action) -> Vec<Trigger> {
    let key = Trigger::key;
    let digit_and_pad = |n: u8| vec![key(&format!("Digit{n}")), key(&format!("Numpad{n}"))];
    match action {
        // DEC-51: mouse.
        Action::Select => vec![Trigger::mouse(MouseBtn::Left)],
        Action::Adjust => vec![Trigger::mouse(MouseBtn::Left).with_mods(ModSet::ALT)],
        Action::Orbit => vec![
            Trigger::mouse(MouseBtn::Middle),
            Trigger::new(Input::Gesture(GestureKind::TwoFinger), ModSet::NONE),
        ],
        Action::Pan => vec![
            Trigger::mouse(MouseBtn::Middle).with_mods(ModSet::SHIFT),
            Trigger::mouse(MouseBtn::Right),
            Trigger::new(Input::Gesture(GestureKind::TwoFinger), ModSet::SHIFT),
        ],
        Action::Zoom => vec![
            Trigger::new(Input::Wheel, ModSet::NONE),
            Trigger::new(Input::Gesture(GestureKind::Pinch), ModSet::NONE),
        ],
        // DEC-51: teclado.
        Action::Cancel => vec![key("Escape")],
        Action::ClockPress => vec![key("Space")],
        Action::ViewTop => digit_and_pad(1),
        Action::ViewOblique => digit_and_pad(2),
        Action::ViewStandard => digit_and_pad(3),
        Action::ViewInverted => digit_and_pad(4),
        Action::ResetCamera => digit_and_pad(0),
        Action::CyclePresentation => vec![key("Tab")],
        Action::ToggleFullscreen => vec![key("F11")],
        Action::ToggleFocus => vec![key("F10")],
        Action::ToggleHud => vec![key("KeyH")],
    }
}

impl Default for ControlMap {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .into_iter()
                .map(|a| (a, defaults_for(a)))
                .collect(),
        }
    }
}

impl ControlMap {
    pub fn triggers(&self, action: Action) -> &[Trigger] {
        self.bindings.get(&action).map_or(&[], Vec::as_slice)
    }

    /// Está exatamente como a DEC-51 manda.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn is_action_default(&self, action: Action) -> bool {
        self.triggers(action) == defaults_for(action).as_slice()
    }

    /// A ação que já usa exatamente este atalho (fora `except`).
    pub fn conflict(&self, except: Action, trigger: &Trigger) -> Option<Action> {
        Action::ALL
            .into_iter()
            .filter(|a| *a != except)
            .find(|a| self.triggers(*a).contains(trigger))
    }

    /// Atribui `trigger` a `action`. Falha, sem alterar nada, se o tipo de entrada não
    /// serve ou se o atalho é de outra ação (veja [`ControlMap::assign_stealing`]).
    pub fn assign(
        &mut self,
        action: Action,
        trigger: Trigger,
        mode: AssignMode,
    ) -> Result<(), AssignError> {
        if !action.accepts(&trigger.input) {
            return Err(AssignError::Unsupported);
        }
        if let Some(other) = self.conflict(action, &trigger) {
            return Err(AssignError::Conflict(other));
        }
        let list = self.bindings.entry(action).or_default();
        if mode == AssignMode::Replace {
            list.clear();
        }
        if !list.contains(&trigger) {
            list.push(trigger);
        }
        Ok(())
    }

    /// Como [`ControlMap::assign`], mas tira o atalho da ação em conflito. Se isso a deixaria
    /// sem atalho e ela precisa de um, nada muda e devolve `WouldLeaveEmpty`.
    pub fn assign_stealing(
        &mut self,
        action: Action,
        trigger: Trigger,
        mode: AssignMode,
    ) -> Result<Option<Action>, AssignError> {
        if !action.accepts(&trigger.input) {
            return Err(AssignError::Unsupported);
        }
        let Some(other) = self.conflict(action, &trigger) else {
            self.assign(action, trigger, mode)?;
            return Ok(None);
        };
        if other.needs_a_trigger() && self.triggers(other).len() <= 1 {
            return Err(AssignError::WouldLeaveEmpty);
        }
        self.bindings
            .entry(other)
            .or_default()
            .retain(|t| *t != trigger);
        self.assign(action, trigger, mode)?;
        Ok(Some(other))
    }

    /// Remove o `index`-ésimo atalho da ação.
    pub fn remove(&mut self, action: Action, index: usize) -> Result<(), AssignError> {
        let list = self.bindings.entry(action).or_default();
        if index >= list.len() {
            return Ok(());
        }
        if action.needs_a_trigger() && list.len() == 1 {
            return Err(AssignError::WouldLeaveEmpty);
        }
        list.remove(index);
        Ok(())
    }

    /// Volta uma ação ao padrão. Atalhos padrão que outra ação tomou são devolvidos a esta
    /// (a outra ação os perde): o padrão é sempre restaurado por inteiro.
    pub fn reset_action(&mut self, action: Action) {
        let defaults = defaults_for(action);
        for other in Action::ALL.into_iter().filter(|a| *a != action) {
            if let Some(list) = self.bindings.get_mut(&other) {
                list.retain(|t| !defaults.contains(t));
            }
        }
        self.bindings.insert(action, defaults);
        // Ação que precisa de atalho e o perdeu: volta ao padrão dela.
        for other in Action::ALL {
            if other.needs_a_trigger() && self.triggers(other).is_empty() {
                self.bindings.insert(other, defaults_for(other));
            }
        }
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    /// Qual ação responde a esta entrada com estes modificadores apertados: o atalho mais
    /// específico (mais modificadores) cujos modificadores estão todos apertados.
    pub fn resolve(&self, input: &Input, held: ModSet) -> Option<Action> {
        let mut best: Option<(usize, Action)> = None;
        for action in Action::ALL {
            for t in self.triggers(action) {
                if t.input == *input && t.mods.is_held_by(held) {
                    let score = t.mods.count();
                    if best.is_none_or(|(s, _)| score > s) {
                        best = Some((score, action));
                    }
                }
            }
        }
        best.map(|(_, a)| a)
    }

    /// Entradas `(id da ação, atalhos em texto)` para gravar.
    pub fn to_entries(&self) -> Vec<(String, Vec<String>)> {
        Action::ALL
            .into_iter()
            .map(|a| {
                (
                    a.id().to_owned(),
                    self.triggers(a).iter().map(Trigger::to_string).collect(),
                )
            })
            .collect()
    }

    /// Lê o que [`ControlMap::to_entries`] gravou. Tolerante: ação ausente ou desconhecida
    /// fica no padrão; atalho ilegível ou de tipo errado descarta a ação (padrão); e um
    /// mapa com conflito é recusado por inteiro (volta ao padrão).
    pub fn from_entries<'a>(entries: impl IntoIterator<Item = (&'a str, Vec<String>)>) -> Self {
        let mut map = Self::default();
        for (id, texts) in entries {
            let Some(action) = Action::from_id(id) else {
                continue;
            };
            let parsed: Result<Vec<Trigger>, _> = texts.iter().map(|t| t.parse()).collect();
            let Ok(triggers) = parsed else { continue };
            if triggers.iter().any(|t| !action.accepts(&t.input)) {
                continue;
            }
            if triggers.is_empty() && action.needs_a_trigger() {
                continue;
            }
            map.bindings.insert(action, triggers);
        }
        let conflicting = Action::ALL
            .into_iter()
            .any(|a| map.triggers(a).iter().any(|t| map.conflict(a, t).is_some()));
        if conflicting { Self::default() } else { map }
    }
}
