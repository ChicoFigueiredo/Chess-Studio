//! A Partida: Comando + instante injetado -> Eventos; `replay` reconstrói do log.

use crate::clock::flag_events;
use crate::command::{Command, CommandKind};
use crate::event::{Event, EventKind};
use crate::online::{OnlineRuntime, Opponent, SentMove};
use crate::policy::policy_for;
use crate::rejection::Rejection;
use crate::setup::GameSetup;
use crate::snapshot::Snapshot;
use crate::state::{GameState, ReplayError, apply};
use crate::time::Timestamp;
use chess_core::Move;

#[derive(Debug, Clone)]
pub struct Game {
    setup: GameSetup,
    state: GameState,
    log: Vec<Event>,
    /// Partida online: intenção enviada e dessincronização, fora do log (ADR-0007).
    online: OnlineRuntime,
}

impl Game {
    /// Partida nova, sem Eventos.
    pub fn new(setup: GameSetup) -> Self {
        let state = GameState::new(&setup);
        Self {
            setup,
            state,
            log: Vec::new(),
            online: OnlineRuntime::default(),
        }
    }

    /// Executa o Comando no instante `at`. Em caso de sucesso, devolve os Eventos
    /// gerados (já aplicados e acrescentados ao log); em caso de rejeição, a Partida
    /// não muda. Um Lance ilegal **não** é rejeição: gera `IllegalMoveRejected`.
    ///
    /// Numa Partida online só o [`Command::Provider`] vale (os demais são pedidos ao
    /// Provedor, feitos pela camada de serviço). Um retrato divergente volta como
    /// [`Rejection::Desynced`]: o log não muda, mas a Partida fica marcada como
    /// dessincronizada (fora do log) até um retrato coerente.
    pub fn handle(&mut self, command: Command, at: Timestamp) -> Result<Vec<Event>, Rejection> {
        if at.mono_ms < self.state.last_at_ms {
            return Err(Rejection::TimeWentBackwards {
                last_ms: self.state.last_at_ms,
                got_ms: at.mono_ms,
            });
        }
        let online = self.state.online;
        if !online
            && matches!(
                command.kind(),
                CommandKind::Provider | CommandKind::CloseWithoutResult
            )
        {
            return Err(Rejection::NotSupportedByPolicy(command.kind()));
        }
        // Bandeira (art. 6.9): um Comando que chega com o tempo esgotado só registra
        // a bandeira; o Comando em si não vale. Online, quem decide o tempo é o Provedor.
        let flag = if online { None } else { self.flag_fall(at) };
        let decided = match flag {
            Some(kinds) => Ok(kinds),
            None => policy_for(self.setup.policy()).decide(&self.state, &command),
        };
        let kinds = match decided {
            Ok(kinds) => kinds,
            Err(Rejection::Desynced(desync)) => {
                self.online.desync = Some(desync.clone());
                self.online.sent = None;
                return Err(Rejection::Desynced(desync));
            }
            Err(rejection) => return Err(rejection),
        };
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
        if online {
            self.after_provider();
        }
        Ok(events)
    }

    /// Depois de um retrato aplicado: a Partida está em sincronia e a intenção enviada se
    /// resolve (confirmada ou substituída) quando o meio-lance dela já existe.
    fn after_provider(&mut self) {
        self.online.desync = None;
        let resolved = self
            .online
            .sent
            .as_ref()
            .is_some_and(|s| self.state.played.len() >= s.ply || self.state.is_over());
        if resolved {
            self.online.sent = None;
        }
    }

    fn flag_fall(&self, at: Timestamp) -> Option<Vec<EventKind>> {
        if self.state.is_over() {
            return None;
        }
        let clock = self.state.clock.as_ref()?;
        flag_events(
            clock,
            self.state.paused,
            self.state.effective_position(),
            at.mono_ms,
        )
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
            online: OnlineRuntime::default(),
        })
    }

    /// Leitura do estado atual.
    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = Snapshot::from_state(&self.setup, &self.state);
        if let Some(view) = &mut snapshot.online {
            view.sent_move = self.online.sent.clone();
            view.desync = self.online.desync.clone();
        }
        snapshot
    }

    /// Revisão: a Posição após `ply` meios-lances do histórico legal (0 = inicial),
    /// também numa Partida terminada. Só lê: não gera Evento nem muda o resultado,
    /// e por isso vale em qualquer Política (revisar não é desfazer, DEC-39).
    pub fn review_at_ply(&self, ply: usize) -> Option<crate::ReviewView> {
        crate::ReviewView::at(&self.state, ply)
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

impl Game {
    /// Ao reabrir uma Partida do log, congela o relógio no último Evento gravado
    /// (DEC-42). Idempotente: já pausada ou terminada, não gera Evento.
    /// Partida online não pausa: o relógio é do Provedor.
    pub fn reopen_paused(&mut self) -> Result<Vec<Event>, Rejection> {
        if self.state.paused || self.state.is_over() || self.state.online {
            return Ok(Vec::new());
        }
        let at = Timestamp::mono(self.state.last_at_ms);
        self.handle(Command::Pause, at)
    }

    /// Relógio em `at` (tempo restante, quem corre, bandeira). `None` sem controle de tempo.
    pub fn clock_at(&self, at: Timestamp) -> Option<crate::ClockReading> {
        self.snapshot().clock_at(at)
    }
}

/// Partida online (ADR-0007): intenção enviada e trava de fair play.
impl Game {
    /// Registra o Lance do jogador local como **intenção enviada**: nada vai para o log; a
    /// camada de serviço o envia ao Provedor uma vez, e ele só vira Lance quando um retrato
    /// o confirma. No máximo uma; só na vez do jogador local; só Lance legal.
    pub fn send_move(&mut self, mv: Move) -> Result<(), Rejection> {
        let info = self.setup.online_game().ok_or(Rejection::NotOnline)?;
        if self.state.is_over() {
            return Err(Rejection::GameOver);
        }
        if let Some(desync) = &self.online.desync {
            return Err(Rejection::Desynced(desync.clone()));
        }
        if self.online.sent.is_some() {
            return Err(Rejection::MoveAlreadySent);
        }
        let position = self.state.position();
        let side = position.side_to_move();
        if side != info.local_color {
            return Err(Rejection::NotLocalTurn);
        }
        if !position.is_legal(mv) {
            return Err(Rejection::MoveNotLegal(mv));
        }
        let san = position.san(mv).map_err(|_| Rejection::MoveNotLegal(mv))?;
        let position_after = position.play(mv).map_err(|_| Rejection::MoveNotLegal(mv))?;
        self.online.sent = Some(SentMove {
            ply: self.state.played.len() + 1,
            side,
            mv,
            san,
            position_after,
        });
        Ok(())
    }

    /// A intenção enviada, se houver.
    pub fn sent_move(&self) -> Option<&SentMove> {
        self.online.sent.as_ref()
    }

    /// Descarta a intenção enviada (o Provedor recusou o Lance, ou o envio se perdeu e o
    /// retrato não o trouxe). Devolve a intenção descartada.
    pub fn drop_sent_move(&mut self) -> Option<SentMove> {
        self.online.sent.take()
    }

    /// "Partida online contra pessoa em andamento" (ADR-0007, CX-08): enquanto for
    /// verdade, a camada de serviço mantém a Trava de fair play ligada. Inclui
    /// correspondência e Partida dessincronizada; exclui a Engine do Provedor e a Partida
    /// cujo fim o Provedor confirmou. Pedir abandono não muda isto: só o retrato do fim.
    pub fn online_human_in_progress(&self) -> bool {
        self.setup
            .online_game()
            .is_some_and(|o| o.opponent == Opponent::Human && !o.is_engine_game())
            && !self.state.is_over()
    }
}
