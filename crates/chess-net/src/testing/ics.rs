//! Servidor ICS falso (TCP de texto) em `127.0.0.1`, porta efêmera, só para testes: o teste
//! empurra o que o "servidor" escreve e confere tudo o que o cliente mandou. Uma conexão por
//! vez (a nova substitui a anterior); tudo o que chega é guardado, de todas as conexões, para
//! o teste provar o que **não** saiu (nenhuma senha). As transcrições dos testes são
//! sintéticas, no formato documentado do FICS; nenhuma foi gravada de um servidor real.

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Teto do que o servidor falso guarda do cliente (um cliente doido não enche a memória).
const MAX_RECEIVED: usize = 4 * 1024 * 1024;

#[derive(Debug, Default)]
struct State {
    connections: usize,
    received: Vec<u8>,
    outbox: Vec<Vec<u8>>,
    /// A conexão atual; uma conexão que deixa de ser a atual (nova conexão ou
    /// `disconnect`) encerra.
    current: Option<usize>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Servidor ICS falso. Descartar encerra a escuta e a conexão.
#[derive(Debug)]
pub struct FakeIcsServer {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeIcsServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("porta efêmera de loopback");
        listener.set_nonblocking(true).expect("escuta sem bloquear");
        let addr = listener.local_addr().expect("endereço local");
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = state.clone();
            let stop = stop.clone();
            thread::spawn(move || accept_loop(&listener, &state, &stop))
        };
        Self {
            addr,
            state,
            stop,
            thread: Some(thread),
        }
    }

    /// A rota para `NetConfig::route(host, …)`: o host oficial vai para este servidor.
    pub fn route_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Quantas conexões foram abertas até agora.
    pub fn connections(&self) -> usize {
        lock(&self.state).connections
    }

    pub fn is_connected(&self) -> bool {
        lock(&self.state).current.is_some()
    }

    /// Espera (tempo real) até um cliente estar conectado.
    pub fn wait_connected(&self, timeout: Duration) -> bool {
        wait(timeout, || self.is_connected())
    }

    /// Espera (tempo real) até a conexão atual cair.
    pub fn wait_disconnected(&self, timeout: Duration) -> bool {
        wait(timeout, || !self.is_connected())
    }

    /// Escreve `text` como o FICS escreve: cada `\n` vira `\n\r`. Sem `\n` no fim, fica como
    /// prompt (`login: `).
    pub fn send(&self, text: &str) {
        self.send_raw(text.replace('\n', "\n\r").as_bytes());
    }

    /// Escreve bytes crus (sequências telnet, bytes inválidos, linhas gigantes).
    pub fn send_raw(&self, bytes: &[u8]) {
        lock(&self.state).outbox.push(bytes.to_vec());
    }

    /// Encerra a conexão atual (o cliente vê o fim do fluxo).
    pub fn disconnect(&self) {
        let mut state = lock(&self.state);
        state.current = None;
        state.outbox.clear();
    }

    /// Todos os bytes que os clientes mandaram, de todas as conexões.
    pub fn received(&self) -> Vec<u8> {
        lock(&self.state).received.clone()
    }

    /// As linhas que os clientes mandaram (sem `\r` e sem o `\n`).
    pub fn received_lines(&self) -> Vec<String> {
        let bytes = self.received();
        let text = String::from_utf8_lossy(&bytes);
        let mut lines: Vec<String> = text.split('\n').map(|l| l.replace('\r', "")).collect();
        // O que vem depois do último `\n` ainda não é linha.
        lines.pop();
        lines
    }

    /// Espera (tempo real) até o cliente ter mandado a linha `line`, exatamente.
    pub fn wait_line(&self, line: &str, timeout: Duration) -> bool {
        wait(timeout, || self.received_lines().iter().any(|l| l == line))
    }

    /// Espera (tempo real) até o cliente ter mandado `count` linhas.
    pub fn wait_lines(&self, count: usize, timeout: Duration) -> bool {
        wait(timeout, || self.received_lines().len() >= count)
    }
}

impl Drop for FakeIcsServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn wait(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    cond()
}

fn accept_loop(listener: &TcpListener, state: &Arc<Mutex<State>>, stop: &Arc<AtomicBool>) {
    let mut handlers: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let id = {
                    let mut s = lock(state);
                    s.connections += 1;
                    s.current = Some(s.connections);
                    s.connections
                };
                let state = state.clone();
                let stop = stop.clone();
                handlers.push(thread::spawn(move || {
                    serve(stream, &state, &stop, id);
                }));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    for handler in handlers {
        let _ = handler.join();
    }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>, stop: &AtomicBool, id: usize) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(5)));
    let _ = stream.set_nodelay(true);
    let mut buf = [0u8; 8192];
    loop {
        if stop.load(Ordering::Acquire) || lock(state).current != Some(id) {
            break;
        }
        let outgoing = std::mem::take(&mut lock(state).outbox);
        for chunk in outgoing {
            if stream.write_all(&chunk).is_err() {
                return finish(state, id);
            }
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut s = lock(state);
                let room = MAX_RECEIVED.saturating_sub(s.received.len());
                s.received.extend_from_slice(&buf[..n.min(room)]);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
    finish(state, id);
}

fn finish(state: &Mutex<State>, id: usize) {
    let mut s = lock(state);
    if s.current == Some(id) {
        s.current = None;
    }
}
