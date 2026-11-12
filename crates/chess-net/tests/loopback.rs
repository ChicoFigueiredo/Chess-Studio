//! Servidor de loopback do retorno da autorização: só `127.0.0.1`, um único retorno, `Host`
//! conferido, prazo pelo relógio injetado, cancelamento e página mínima.

mod common;

use chess_net::{CallbackError, CallbackParams, FakeClock, LoopbackCallback};
use common::*;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const TEN_MINUTES: Duration = Duration::from_secs(600);

type Waiting = JoinHandle<Result<CallbackParams, CallbackError>>;

/// Abre o servidor e espera numa thread, com prazo de 10 minutos no relógio falso.
fn start(clock: &Arc<FakeClock>) -> (u16, Arc<AtomicBool>, Waiting) {
    let callback = LoopbackCallback::bind("/retorno").expect("porta de loopback");
    let port = callback.port();
    assert_eq!(
        callback.redirect_uri(),
        format!("http://127.0.0.1:{port}/retorno")
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let deadline = T0.plus(TEN_MINUTES);
    let (c, k) = (clock.clone(), cancel.clone());
    let handle = thread::spawn(move || callback.wait(c.as_ref(), deadline, &k));
    (port, cancel, handle)
}

/// Manda uma requisição crua e devolve a resposta inteira.
fn send(port: u16, request: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn get(port: u16, target: &str) -> String {
    send(
        port,
        &format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    )
    .expect("resposta")
}

#[test]
fn the_single_callback_is_received_answered_and_the_port_closes() {
    let clock = clock();
    let (port, _cancel, handle) = start(&clock);

    let response = get(port, "/retorno?code=CODIGO&state=ESTADO%2Bx");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(
        response.contains("Pode voltar ao Chess Studio"),
        "{response}"
    );
    assert!(response.contains("cache-control: no-store"), "{response}");
    assert!(
        response.contains("referrer-policy: no-referrer"),
        "{response}"
    );
    assert!(!response.contains("<script"), "{response}");

    let params = handle.join().expect("thread").expect("retorno");
    assert_eq!(params.get("code"), Some("CODIGO"));
    assert_eq!(params.get("state"), Some("ESTADO+x"));
    assert!(
        !format!("{params:?}").contains("CODIGO"),
        "o código não vai para o Debug"
    );

    // Segunda requisição: ninguém escuta mais.
    assert!(
        TcpStream::connect(("127.0.0.1", port)).is_err(),
        "a porta continuou aberta depois do retorno"
    );
}

#[test]
fn other_paths_methods_and_hosts_do_not_count_as_the_callback() {
    let clock = clock();
    let (port, _cancel, handle) = start(&clock);

    let favicon = get(port, "/favicon.ico");
    assert!(favicon.starts_with("HTTP/1.1 404"), "{favicon}");
    let post = send(
        port,
        &format!("POST /retorno?code=X&state=Y HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    )
    .expect("resposta");
    assert!(post.starts_with("HTTP/1.1 404"), "{post}");
    // DNS rebinding: o navegador de uma página maliciosa manda o Host do atacante.
    let rebinding = send(
        port,
        &format!("GET /retorno?code=X&state=Y HTTP/1.1\r\nHost: evil.example:{port}\r\n\r\n"),
    )
    .expect("resposta");
    assert!(rebinding.starts_with("HTTP/1.1 404"), "{rebinding}");
    // Conexão aberta e abandonada (o navegador às vezes abre uma de reserva).
    let idle = TcpStream::connect(("127.0.0.1", port)).expect("conecta");
    drop(idle);
    assert!(!handle.is_finished(), "nada disso é o retorno");

    get(port, "/retorno?code=BOM&state=S");
    let params = handle.join().expect("thread").expect("retorno");
    assert_eq!(params.get("code"), Some("BOM"));
}

#[test]
fn the_deadline_comes_from_the_injected_clock() {
    let clock = clock();
    let (port, _cancel, handle) = start(&clock);

    clock.advance(TEN_MINUTES);

    assert_eq!(
        handle.join().expect("thread").unwrap_err(),
        CallbackError::Expired
    );
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
fn cancelling_stops_waiting_and_closes_the_port() {
    let clock = clock();
    let (port, cancel, handle) = start(&clock);

    cancel.store(true, Ordering::Release);

    assert_eq!(
        handle.join().expect("thread").unwrap_err(),
        CallbackError::Cancelled
    );
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

/// Cliente local que manda um byte a cada 500 ms, sem nunca terminar o cabeçalho (SEC-04).
fn slow_client(port: u16) -> JoinHandle<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("conexão lenta");
    thread::spawn(move || {
        for _ in 0..40 {
            if stream.write_all(b"G").is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
    })
}

#[test]
fn a_slow_local_client_does_not_hold_back_the_legitimate_callback() {
    let clock = clock();
    let (port, _cancel, handle) = start(&clock);
    let _slow = slow_client(port);
    thread::sleep(Duration::from_millis(600));

    let started = std::time::Instant::now();
    let response = get(port, "/retorno?code=BOM&state=S");

    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
    let params = handle.join().expect("thread").expect("retorno");
    assert_eq!(params.get("code"), Some("BOM"));
}

#[test]
fn cancelling_during_a_slow_client_answers_in_under_a_second() {
    let clock = clock();
    let (port, cancel, handle) = start(&clock);
    let _slow = slow_client(port);
    thread::sleep(Duration::from_millis(700));

    let started = std::time::Instant::now();
    cancel.store(true, Ordering::Release);

    assert_eq!(
        handle.join().expect("thread").unwrap_err(),
        CallbackError::Cancelled
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn the_deadline_of_the_injected_clock_holds_during_a_slow_client() {
    let clock = clock();
    let (port, _cancel, handle) = start(&clock);
    let _slow = slow_client(port);
    thread::sleep(Duration::from_millis(700));

    clock.advance(TEN_MINUTES);

    assert_eq!(
        handle.join().expect("thread").unwrap_err(),
        CallbackError::Expired
    );
}

/// Um endereço da máquina que não é de loopback, se houver (nada sai pela rede: `connect`
/// de UDP só escolhe a interface).
fn non_loopback_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[test]
fn the_server_is_not_reachable_from_another_interface() {
    let clock = clock();
    let (port, cancel, handle) = start(&clock);

    if let Some(ip) = non_loopback_address() {
        let outside = SocketAddr::new(ip, port);
        assert!(
            TcpStream::connect_timeout(&outside, Duration::from_millis(500)).is_err(),
            "o servidor respondeu em {outside}"
        );
    }

    cancel.store(true, Ordering::Release);
    let _ = handle.join();
}
