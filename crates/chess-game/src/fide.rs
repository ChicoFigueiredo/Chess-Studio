//! Simulação FIDE (stub).
use crate::command::Command;
use crate::event::EventKind;
use crate::policy::ArbitrationPolicy;
use crate::rejection::Rejection;
use crate::state::GameState;

pub(crate) struct Competitive {
    pub configurable_stage: bool,
}

pub(crate) static FIDE_SIMULATION: Competitive = Competitive {
    configurable_stage: false,
};

impl ArbitrationPolicy for Competitive {
    fn decide(&self, _state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection> {
        Err(Rejection::NotYetImplemented(command.kind()))
    }
}
