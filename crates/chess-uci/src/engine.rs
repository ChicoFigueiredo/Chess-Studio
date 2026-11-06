//! Esqueleto da API pública (implementação vem no próximo commit).

use std::time::Duration;

use chess_core::{CanonicalPosition, Move};

use crate::{
    BestMove, EngineConfig, EngineError, EngineInfo, GoParams, LogEntry, PositionSpec,
};

/// Resultado de [`Engine::search_legal_move`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineMove {
    /// Lance legal e a Posição resultante (a original não é alterada).
    Played { mv: Move, next: CanonicalPosition },
    /// A Engine disse `bestmove (none)`.
    NoMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitOutcome {
    Graceful,
    Killed,
}

#[derive(Debug)]
pub struct Engine {}

impl Engine {
    pub fn spawn(_config: EngineConfig) -> Result<Self, EngineError> {
        unimplemented!()
    }
    pub fn info(&self) -> &EngineInfo {
        unimplemented!()
    }
    pub fn is_ready(&mut self) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn set_option(&mut self, _name: &str, _value: Option<&str>) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn new_game(&mut self) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn set_position(&mut self, _spec: &PositionSpec) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn go(&mut self, _params: &GoParams) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn wait_bestmove(&mut self, _timeout: Duration) -> Result<BestMove, EngineError> {
        unimplemented!()
    }
    pub fn stop(&mut self) -> Result<(), EngineError> {
        unimplemented!()
    }
    pub fn search(&mut self, _params: &GoParams) -> Result<BestMove, EngineError> {
        unimplemented!()
    }
    pub fn search_legal_move(
        &mut self,
        _position: &CanonicalPosition,
        _params: &GoParams,
    ) -> Result<EngineMove, EngineError> {
        unimplemented!()
    }
    pub fn log(&self) -> Vec<LogEntry> {
        unimplemented!()
    }
    pub fn is_alive(&mut self) -> bool {
        unimplemented!()
    }
    pub fn quit(self) -> QuitOutcome {
        unimplemented!()
    }
}
