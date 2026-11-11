//! Servidor HTTP/1.1 falso, mínimo, em `127.0.0.1` porta efêmera: o suficiente para o
//! `ureq`. Uma thread por conexão (uma resposta parada não segura as outras), toda resposta
//! fecha a conexão, e cada pedido é registrado (com o instante do relógio injetado, se
//! houver) antes da resposta sair.

use crate::clock::{Clock, Timestamp};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_HEAD: usize = 64 * 1024;
const STALL_LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Normal,
    /// Lê o pedido e não responde (até o cliente desistir ou o servidor cair).
    Stall,
    /// Corpo em pedaços, um a cada 20 ms, e fecha.
    Stream(Vec<Vec<u8>>),
    /// Repete o pedaço a cada 50 ms até o cliente fechar.
    StreamForever(Vec<u8>),
}

/// Resposta do servidor falso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    kind: Kind,
}

impl FakeResponse {
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            kind: Kind::Normal,
        }
    }

    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::status(200).with_body(body.into())
    }

    pub fn json(body: &str) -> Self {
        Self::ok(body.as_bytes().to_vec()).header("content-type", "application/json")
    }

    pub fn stall() -> Self {
        Self {
            kind: Kind::Stall,
            ..Self::status(200)
        }
    }

    /// Fluxo NDJSON que entrega `parts` e termina.
    pub fn stream(parts: &[&[u8]]) -> Self {
        Self {
            kind: Kind::Stream(parts.iter().map(|p| p.to_vec()).collect()),
            ..Self::status(200).header("content-type", "application/x-ndjson")
        }
    }

    /// Fluxo que nunca termina sozinho (heartbeat repetido).
    pub fn stream_forever(heartbeat: &[u8]) -> Self {
        Self {
            kind: Kind::StreamForever(heartbeat.to_vec()),
            ..Self::status(200).header("content-type", "application/x-ndjson")
        }
    }

    /// Resposta gravada em arquivo (formato do `curl -i`; ver `testing::recorded`).
    pub fn recorded(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::parse_recording(&text)
    }

    pub fn parse_recording(text: &str) -> Result<Self, String> {
        super::recorded::parse(text)
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_owned()));
        self
    }

    pub(crate) fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn status_code(&self) -> u16 {
        self.status
    }

    pub fn header_value(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn body_bytes(&self) -> &[u8] {
        &self.body
    }
}

/// Um pedido que chegou ao servidor falso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedRequest {
    pub method: String,
    pub path: String,
    /// Parâmetros de consulta já decodificados, na ordem recebida.
    pub query: Vec<(String, String)>,
    /// Cabeçalhos com o nome em minúsculas.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Instante do relógio injetado na chegada (`None` sem relógio).
    pub at: Option<Timestamp>,
}

impl ReceivedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn query_value(&self, key: &str) -> Option<String> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
}

#[derive(Debug, Default)]
struct Route {
    responses: Vec<FakeResponse>,
    next: usize,
}

#[derive(Debug)]
struct State {
    routes: Mutex<HashMap<String, Route>>,
    requests: Mutex<Vec<ReceivedRequest>>,
    connections: AtomicUsize,
    shutdown: AtomicBool,
    clock: Option<Arc<dyn Clock>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Servidor HTTP falso. Cai (e libera a porta) quando é descartado.
#[derive(Debug)]
pub struct FakeServer {
    addr: SocketAddr,
    state: Arc<State>,
    accept: Option<JoinHandle<()>>,
}

impl FakeServer {
    pub fn start() -> Self {
        Self::launch(None)
    }

    /// Carimba cada pedido com o relógio dado (o falso, nos testes de limite).
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self::launch(Some(clock))
    }

    fn launch(clock: Option<Arc<dyn Clock>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("porta efêmera de loopback");
        listener.set_nonblocking(true).expect("accept sem bloqueio");
        let addr = listener.local_addr().expect("endereço local");
        let state = Arc::new(State {
            routes: Mutex::new(HashMap::new()),
            requests: Mutex::new(Vec::new()),
            connections: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            clock,
        });
        let shared = state.clone();
        let accept = thread::spawn(move || accept_loop(&listener, &shared));
        Self {
            addr,
            state,
            accept: Some(accept),
        }
    }

    /// `http://127.0.0.1:<porta>`, para `NetConfig::route`.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Responde sempre `response` em `path` (sem a consulta).
    pub fn route(&self, path: &str, response: FakeResponse) {
        self.route_seq(path, vec![response]);
    }

    /// Responde em sequência; depois da última, repete a última.
    pub fn route_seq(&self, path: &str, responses: Vec<FakeResponse>) {
        lock(&self.state.routes).insert(path.to_owned(), Route { responses, next: 0 });
    }

    pub fn requests(&self) -> Vec<ReceivedRequest> {
        lock(&self.state.requests).clone()
    }

    /// Conexões aceitas (inclui as que não chegaram a mandar um pedido completo).
    pub fn connections(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    /// Espera (tempo real) até chegarem `n` pedidos.
    pub fn wait_for_requests(&self, n: usize, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if lock(&self.state.requests).len() >= n {
                return true;
            }
            if start.elapsed() > timeout {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.state.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.accept.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(listener: &TcpListener, state: &Arc<State>) {
    while !state.shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                state.connections.fetch_add(1, Ordering::SeqCst);
                let state = state.clone();
                thread::spawn(move || serve(stream, &state));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(_) => return,
        }
    }
}

fn serve(mut stream: TcpStream, state: &State) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(request) = read_request(&mut stream, state) else {
        return;
    };
    let response = {
        let mut routes = lock(&state.routes);
        match routes.get_mut(&request.path) {
            Some(route) if !route.responses.is_empty() => {
                let i = route.next.min(route.responses.len() - 1);
                route.next += 1;
                route.responses[i].clone()
            }
            _ => FakeResponse::status(404),
        }
    };
    lock(&state.requests).push(request);
    match &response.kind {
        Kind::Normal => {
            let _ = write_head(&mut stream, &response, Some(response.body.len()));
            let _ = stream.write_all(&response.body);
            let _ = stream.flush();
        }
        Kind::Stall => stall(&mut stream, state),
        Kind::Stream(parts) => {
            if write_head(&mut stream, &response, None).is_ok() {
                for part in parts {
                    if stream
                        .write_all(part)
                        .and_then(|()| stream.flush())
                        .is_err()
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
        Kind::StreamForever(beat) => {
            if write_head(&mut stream, &response, None).is_ok() {
                while !state.shutdown.load(Ordering::SeqCst) {
                    if stream
                        .write_all(beat)
                        .and_then(|()| stream.flush())
                        .is_err()
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// Segura a conexão sem responder até o cliente fechar, o servidor cair ou 30 s.
fn stall(stream: &mut TcpStream, state: &State) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
    let start = Instant::now();
    let mut buf = [0u8; 256];
    while !state.shutdown.load(Ordering::SeqCst) && start.elapsed() < STALL_LIMIT {
        match stream.read(&mut buf) {
            Ok(0) => return,
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        304 => "Not Modified",
        302 => "Found",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn write_head(
    stream: &mut TcpStream,
    response: &FakeResponse,
    length: Option<usize>,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(n) = length {
        head.push_str(&format!("content-length: {n}\r\n"));
    }
    head.push_str("connection: close\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

fn read_request(stream: &mut TcpStream, state: &State) -> Option<ReceivedRequest> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if data.len() > MAX_HEAD {
            return None;
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let target = first.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
        }
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_owned(), parse_query(q)),
        None => (target, Vec::new()),
    };
    Some(ReceivedRequest {
        method,
        path,
        query,
        headers,
        body,
        at: state.clock.as_ref().map(|c| c.now()),
    })
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
