//! Política de arbitragem: decide, para um Comando no estado atual, quais Eventos
//! acontecem. Pura: lê o estado, não o muda (quem aplica é `Game::handle`).

use crate::command::Command;
use crate::event::EventKind;
use crate::rejection::Rejection;
use crate::setup::PolicyKind;
use crate::state::GameState;

pub(crate) trait ArbitrationPolicy {
    fn decide(&self, state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection>;
}

pub(crate) fn policy_for(kind: PolicyKind) -> &'static dyn ArbitrationPolicy {
    match kind {
        PolicyKind::Assisted => &Assisted,
    }
}

struct Assisted;

impl ArbitrationPolicy for Assisted {
    fn decide(&self, _state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection> {
        Err(Rejection::NotYetImplemented(command.kind()))
    }
}
