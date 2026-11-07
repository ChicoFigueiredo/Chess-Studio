//! A Engine como processo filho: uma thread lê o stdout e entrega as linhas por
//! canal; toda espera usa `recv_timeout`, então nada bloqueia sem prazo.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use chess_core::{CanonicalPosition, Move};

use crate::{
    BestMove, EngineConfig, EngineError, EngineInfo, EngineOption, GoParams, LogEntry, LogSource,
    PositionSpec, Stage, Timeouts,
};

/// Resultado de [`Engine::search_legal_move`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineMove {
    /// Lance legal e a Posição resultante (a original não é alterada).
    Played { mv: Move, next: CanonicalPosition },
    /// A Engine disse `bestmove (none)`.
    NoMove,
}

/// Como o processo terminou em [`Engine::quit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitOutcome {
    /// Saiu sozinho dentro do prazo (ou já tinha saído).
    Graceful,
    /// Ignorou `quit` e foi morto depois do prazo.
    Killed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Searching { budget: Duration },
    Dead,
}

type SharedLog = Arc<Mutex<VecDeque<LogEntry>>>;

enum Event {
    Line(String),
    Eof,
}

fn push_log(log: &SharedLog, capacity: usize, source: LogSource, line: &str) {
    let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
    if capacity == 0 {
        return;
    }
    while log.len() >= capacity {
        log.pop_front();
    }
    log.push_back(LogEntry {
        source,
        line: line.to_owned(),
    });
}

/// Lê linhas (tolerando bytes inválidos) até o EOF. Só o stdout manda eventos.
fn spawn_reader(
    stream: impl Read + Send + 'static,
    source: LogSource,
    tx: Option<Sender<Event>>,
    log: SharedLog,
    capacity: usize,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buf);
                    let line = text.trim_end_matches(['\r', '\n']).to_owned();
                    push_log(&log, capacity, source, &line);
                    if let Some(tx) = &tx
                        && tx.send(Event::Line(line)).is_err()
                    {
                        return;
                    }
                }
            }
        }
        if let Some(tx) = tx {
            let _ = tx.send(Event::Eof);
        }
    });
}

/// Uma Engine UCI rodando como processo filho.
///
/// Os métodos que esperam resposta têm prazo (ver [`crate::Timeouts`]). Se a
/// Engine travar ou morrer, o erro é tipado e a instância passa a responder
/// [`EngineError::Terminated`]; quem chama decide se cria outra.
#[derive(Debug)]
pub struct Engine {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Event>,
    log: SharedLog,
    capacity: usize,
    info: EngineInfo,
    timeouts: Timeouts,
    state: State,
}

impl Engine {
    /// Inicia o processo, faz o handshake `uci`/`uciok` e confirma `isready`/`readyok`.
    pub fn spawn(config: EngineConfig) -> Result<Self, EngineError> {
        let program = config.program.display().to_string();
        let mut child = Command::new(&config.program)
            .args(&config.args)
            .envs(config.envs.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| EngineError::Spawn { program, source })?;

        let log: SharedLog = Arc::default();
        let (tx, rx) = mpsc::channel();
        let capacity = config.log_capacity;
        if let Some(out) = child.stdout.take() {
            spawn_reader(
                out,
                LogSource::FromEngine,
                Some(tx),
                Arc::clone(&log),
                capacity,
            );
        }
        if let Some(err) = child.stderr.take() {
            spawn_reader(err, LogSource::Stderr, None, Arc::clone(&log), capacity);
        }
        let stdin = child.stdin.take();
        let mut engine = Self {
            child,
            stdin,
            rx,
            log,
            capacity,
            info: EngineInfo::default(),
            timeouts: config.timeouts,
            state: State::Idle,
        };
        engine.handshake()?;
        for (name, value) in &config.options {
            engine.set_option(name, Some(value))?;
        }
        engine.is_ready()?;
        Ok(engine)
    }

    fn handshake(&mut self) -> Result<(), EngineError> {
        self.send("uci")?;
        let mut info = EngineInfo::default();
        let timeout = self.timeouts.handshake;
        let result = self.wait_line(Stage::Handshake, timeout, |line| {
            if let Some(name) = line.strip_prefix("id name ") {
                info.name = Some(name.trim().to_owned());
            } else if let Some(author) = line.strip_prefix("id author ") {
                info.author = Some(author.trim().to_owned());
            } else if let Some(opt) = EngineOption::parse(line) {
                info.options.push(opt);
            }
            (line.trim() == "uciok").then_some(())
        });
        self.fatal_on_timeout(result)?;
        self.info = info;
        Ok(())
    }

    /// Nome, autor e opções anunciados no handshake.
    pub fn info(&self) -> &EngineInfo {
        &self.info
    }

    /// `isready` → `readyok`. Estourar o prazo mata o processo.
    pub fn is_ready(&mut self) -> Result<(), EngineError> {
        self.ensure_idle()?;
        self.send("isready")?;
        let timeout = self.timeouts.ready;
        let result = self.wait_line(Stage::Ready, timeout, |l| {
            (l.trim() == "readyok").then_some(())
        });
        self.fatal_on_timeout(result)
    }

    /// `setoption name <nome> [value <valor>]`.
    pub fn set_option(&mut self, name: &str, value: Option<&str>) -> Result<(), EngineError> {
        // Quebra de linha ou controle em nome/valor injetaria outro comando UCI.
        let hostile = |t: &str| t.chars().any(char::is_control);
        if name.is_empty() || hostile(name) || value.is_some_and(hostile) {
            return Err(EngineError::InvalidInput(format!(
                "opção UCI inválida: {name:?}"
            )));
        }
        self.ensure_idle()?;
        match value {
            Some(v) => self.send(&format!("setoption name {name} value {v}")),
            None => self.send(&format!("setoption name {name}")),
        }
    }

    /// `ucinewgame` seguido de `isready`, como manda o protocolo.
    pub fn new_game(&mut self) -> Result<(), EngineError> {
        self.ensure_idle()?;
        self.send("ucinewgame")?;
        self.is_ready()
    }

    pub fn set_position(&mut self, spec: &PositionSpec) -> Result<(), EngineError> {
        self.ensure_idle()?;
        self.send(&spec.command())
    }

    /// Inicia a busca sem esperar a resposta; use [`Engine::wait_bestmove`].
    pub fn go(&mut self, params: &GoParams) -> Result<(), EngineError> {
        self.ensure_idle()?;
        self.send(&params.command())?;
        self.state = State::Searching {
            budget: self.budget(params),
        };
        Ok(())
    }

    /// Espera o `bestmove` por até `timeout`. Se o prazo estourar a busca continua
    /// (erro recuperável): chame [`Engine::stop`] e espere de novo.
    pub fn wait_bestmove(&mut self, timeout: Duration) -> Result<BestMove, EngineError> {
        match self.state {
            State::Dead => return Err(EngineError::Terminated),
            State::Idle => return Err(EngineError::NotSearching),
            State::Searching { .. } => {}
        }
        let best = self.wait_line(Stage::Search, timeout, BestMove::parse)?;
        self.state = State::Idle;
        Ok(best)
    }

    /// Envia `stop`. A Engine ainda deve responder `bestmove`.
    pub fn stop(&mut self) -> Result<(), EngineError> {
        match self.state {
            State::Dead => Err(EngineError::Terminated),
            State::Idle => Err(EngineError::NotSearching),
            State::Searching { .. } => self.send("stop"),
        }
    }

    /// `go` + espera com prazo derivado dos parâmetros (`movetime` + folga). Passado
    /// o prazo envia `stop` e dá `stop_grace`; sem resposta o processo é morto e o
    /// erro é [`EngineError::Timeout`] com `killed: true`.
    pub fn search(&mut self, params: &GoParams) -> Result<BestMove, EngineError> {
        self.go(params)?;
        let State::Searching { budget } = self.state else {
            return Err(EngineError::NotSearching);
        };
        match self.wait_bestmove(budget) {
            Err(EngineError::Timeout { killed: false, .. }) => {}
            other => return other,
        }
        self.stop()?;
        let grace = self.timeouts.stop_grace;
        match self.wait_bestmove(grace) {
            Err(EngineError::Timeout { .. }) => {
                self.kill();
                Err(EngineError::Timeout {
                    stage: Stage::Search,
                    after: budget + grace,
                    killed: true,
                })
            }
            other => other,
        }
    }

    /// Envia a Posição, busca e valida a resposta pelo núcleo de regras. Lance
    /// ilegal ou malformado vira erro recuperável; a `position` não é alterada.
    pub fn search_legal_move(
        &mut self,
        position: &CanonicalPosition,
        params: &GoParams,
    ) -> Result<EngineMove, EngineError> {
        self.set_position(&PositionSpec::from_position(position))?;
        match self.search(params)? {
            BestMove::None => Ok(EngineMove::NoMove),
            BestMove::Move { text, .. } => {
                let mv: Move = text.parse().map_err(|source| EngineError::MalformedMove {
                    text: text.clone(),
                    source,
                })?;
                if !position.is_legal(mv) {
                    return Err(EngineError::IllegalMove {
                        text,
                        fen: position.to_fen(),
                    });
                }
                let next = position.play(mv).map_err(|_| EngineError::IllegalMove {
                    text,
                    fen: position.to_fen(),
                })?;
                Ok(EngineMove::Played { mv, next })
            }
        }
    }

    /// As últimas linhas trocadas (limitado por `log_capacity`), da mais antiga à mais nova.
    pub fn log(&self) -> Vec<LogEntry> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// O processo ainda está vivo e utilizável?
    pub fn is_alive(&mut self) -> bool {
        if self.state == State::Dead {
            return false;
        }
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            self.state = State::Dead;
            return false;
        }
        true
    }

    /// `quit` e espera até `timeouts.quit`; depois disso mata o processo.
    pub fn quit(mut self) -> QuitOutcome {
        if self.state != State::Dead {
            let _ = self.send("quit");
        }
        self.stdin = None;
        let deadline = Instant::now() + self.timeouts.quit;
        loop {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                self.state = State::Dead;
                return QuitOutcome::Graceful;
            }
            if Instant::now() >= deadline {
                self.kill();
                return QuitOutcome::Killed;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    // --- internos ------------------------------------------------------------

    fn budget(&self, params: &GoParams) -> Duration {
        let slack = self.timeouts.search_slack;
        if let Some(t) = params.movetime {
            t + slack
        } else if let Some(c) = &params.clock {
            c.wtime.max(c.btime) + slack
        } else {
            self.timeouts.search_default
        }
    }

    fn ensure_idle(&self) -> Result<(), EngineError> {
        match self.state {
            State::Dead => Err(EngineError::Terminated),
            State::Searching { .. } => Err(EngineError::SearchInProgress),
            State::Idle => Ok(()),
        }
    }

    fn send(&mut self, line: &str) -> Result<(), EngineError> {
        if self.state == State::Dead {
            return Err(EngineError::Terminated);
        }
        if line.contains(['\n', '\r']) {
            return Err(EngineError::InvalidInput(format!(
                "comando com quebra de linha: {line:?}"
            )));
        }
        push_log(&self.log, self.capacity, LogSource::ToEngine, line);
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(EngineError::Terminated);
        };
        let written = writeln!(stdin, "{line}").and_then(|()| stdin.flush());
        match written {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(self.crashed()),
            Err(e) => Err(EngineError::Io(e)),
        }
    }

    /// Lê linhas até `f` devolver `Some`, no máximo por `timeout`. Linhas que `f`
    /// não reconhece (lixo, `info`) são ignoradas; ficam só no log.
    fn wait_line<T>(
        &mut self,
        stage: Stage,
        timeout: Duration,
        mut f: impl FnMut(&str) -> Option<T>,
    ) -> Result<T, EngineError> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(remaining) {
                Ok(Event::Line(line)) => {
                    if let Some(value) = f(&line) {
                        return Ok(value);
                    }
                }
                Ok(Event::Eof) | Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.crashed());
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(EngineError::Timeout {
                        stage,
                        after: timeout,
                        killed: false,
                    });
                }
            }
        }
    }

    /// Timeout de handshake/ready é fatal: a Engine não fala UCI direito.
    fn fatal_on_timeout<T>(&mut self, result: Result<T, EngineError>) -> Result<T, EngineError> {
        match result {
            Err(EngineError::Timeout { stage, after, .. }) => {
                self.kill();
                Err(EngineError::Timeout {
                    stage,
                    after,
                    killed: true,
                })
            }
            other => other,
        }
    }

    /// Marca a instância como morta e descobre o status de saída (espera curta).
    fn crashed(&mut self) -> EngineError {
        self.state = State::Dead;
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return EngineError::Crashed {
                        exit: Some(status.to_string()),
                    };
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                _ => {
                    self.kill();
                    return EngineError::Crashed { exit: None };
                }
            }
        }
    }

    fn kill(&mut self) {
        self.state = State::Dead;
        self.stdin = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
