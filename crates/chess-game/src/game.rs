//! A Partida: Comando + instante injetado -> Eventos; `replay` reconstrói do log.

use crate::command::Command;
use crate::event::Event;
use crate::policy::policy_for;
use crate::rejection::Rejection;
use crate::setup::GameSetup;
use crate::snapshot::Snapshot;
use crate::state::{GameState, ReplayError, apply};
use crate::time::Timestamp;

#[derive(Debug, Clone)]
pub struct Game {
    setup: GameSetup,
    state: GameState,
    log: Vec<Event>,
}

impl Game {
    /// Partida nova, sem Eventos.
    pub fn new(setup: GameSetup) -> Self {
        let state = GameState::new(&setup);
        Self {
            setup,
            state,
            log: Vec::new(),
        }
    }

    /// Executa o Comando no instante `at`. Em caso de sucesso, devolve os Eventos
    /// gerados (já aplicados e acrescentados ao log); em caso de rejeição, a Partida
    /// não muda. Um Lance ilegal **não** é rejeição: gera `IllegalMoveRejected`.
    pub fn handle(&mut self, command: Command, at: Timestamp) -> Result<Vec<Event>, Rejection> {
        if at.mono_ms < self.state.last_at_ms {
            return Err(Rejection::TimeWentBackwards {
                last_ms: self.state.last_at_ms,
                got_ms: at.mono_ms,
            });
        }
        let kinds = policy_for(self.setup.policy()).decide(&self.state, &command)?;
        let events: Vec<Event> = kinds.into_iter().map(|k| Event::new(at, k)).collect();
        for event in &events {
            // As Políticas só emitem Eventos coerentes com o estado; se não for
            // assim, é bug: a Partida é restaurada e o Comando é recusado.
            if apply(&mut self.state, event).is_err() {
                self.state =
                    rebuild(&self.setup, &self.log).map_err(|_| Rejection::Inconsistent)?;
                return Err(Rejection::Inconsistent);
            }
        }
        self.log.extend(events.iter().cloned());
        Ok(events)
    }

    /// Reconstrói a Partida dos Eventos. Tolerante: para no primeiro Evento que não
    /// se aplica (log truncado ou corrompido) e mantém o estado até ali. Use
    /// [`Game::try_replay`] para detectar o problema.
    pub fn replay(setup: GameSetup, events: &[Event]) -> Self {
        let mut game = Self::new(setup);
        for event in events {
            if apply(&mut game.state, event).is_err() {
                break;
            }
            game.log.push(event.clone());
        }
        game
    }

    /// Como [`Game::replay`], mas falha apontando o Evento que não se aplica.
    pub fn try_replay(setup: GameSetup, events: &[Event]) -> Result<Self, ReplayError> {
        let state = rebuild(&setup, events)?;
        Ok(Self {
            setup,
            state,
            log: events.to_vec(),
        })
    }

    /// Leitura do estado atual.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot::from_state(self.setup.policy(), &self.state)
    }

    /// Log de Eventos desta Partida (a fonte de verdade).
    pub fn events(&self) -> &[Event] {
        &self.log
    }

    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }
}

fn rebuild(setup: &GameSetup, events: &[Event]) -> Result<GameState, ReplayError> {
    let mut state = GameState::new(setup);
    for (index, event) in events.iter().enumerate() {
        apply(&mut state, event).map_err(|reason| ReplayError { index, reason })?;
    }
    Ok(state)
}
