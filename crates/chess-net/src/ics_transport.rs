//! Transporte ICS: TCP de texto, sem cifra (#237). É o único arquivo de produção, além do
//! loopback da autorização, que usa socket, e só abre conexão (nunca escuta). Só o gate do
//! ICS (`net/ics.rs`) o chama: nenhum caminho chega aqui sem passar pelo consentimento, pelo
//! catálogo dos endereços ICS, pela Trava de fair play e pelos limites.
//!
//! Não interpreta nada: lê bytes com prazo curto (para o gate atender ao cancelamento e aos
//! comandos) e escreve linhas já validadas.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Para onde conectar: o host oficial do catálogo ou, nos testes, o servidor falso em
/// loopback (rota da configuração).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IcsAddress {
    Host(&'static str, u16),
    Loopback(SocketAddr),
}

impl IcsAddress {
    /// `127.0.0.1:porta`, `localhost:porta` ou `[::1]:porta`; qualquer outro endereço não é
    /// loopback e fica de fora.
    pub fn loopback(authority: &str) -> Option<Self> {
        let authority = authority
            .strip_prefix("localhost:")
            .map_or_else(|| authority.to_owned(), |port| format!("127.0.0.1:{port}"));
        authority
            .parse::<SocketAddr>()
            .ok()
            .filter(|addr| addr.ip().is_loopback())
            .map(Self::Loopback)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IcsFailure {
    /// Prazo de conexão esgotado (ou, na leitura, só o fim da fatia de espera).
    Timeout,
    /// Sem rota até o servidor (DNS, recusada, rede fora).
    Unreachable(&'static str),
    /// A conexão caiu no meio.
    Interrupted(&'static str),
    Cancelled,
}

#[derive(Debug)]
pub(crate) struct IcsSocket {
    stream: TcpStream,
}

impl IcsSocket {
    /// Abre a conexão (cada endereço do host com o prazo `timeout`); a leitura passa a
    /// esperar no máximo `poll`.
    pub fn connect(
        address: &IcsAddress,
        timeout: Duration,
        poll: Duration,
        cancel: &AtomicBool,
    ) -> Result<Self, IcsFailure> {
        let addresses: Vec<SocketAddr> = match address {
            IcsAddress::Loopback(addr) => vec![*addr],
            IcsAddress::Host(host, port) => (*host, *port)
                .to_socket_addrs()
                .map_err(|_| IcsFailure::Unreachable("host não encontrado"))?
                .collect(),
        };
        let mut last = IcsFailure::Unreachable("host não encontrado");
        for addr in addresses {
            if cancel.load(Ordering::Acquire) {
                return Err(IcsFailure::Cancelled);
            }
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(stream) => {
                    let configured = stream
                        .set_read_timeout(Some(poll))
                        .and_then(|()| stream.set_write_timeout(Some(timeout)))
                        .and_then(|()| stream.set_nodelay(true));
                    return match configured {
                        Ok(()) => Ok(Self { stream }),
                        Err(e) => Err(failure(e.kind())),
                    };
                }
                Err(e) => last = failure(e.kind()),
            }
        }
        if cancel.load(Ordering::Acquire) {
            return Err(IcsFailure::Cancelled);
        }
        Err(last)
    }

    /// Lê o que houver; [`IcsFailure::Timeout`] quando a fatia de espera acaba sem dados.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IcsFailure> {
        loop {
            match self.stream.read(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(failure(e.kind())),
            }
        }
    }

    /// Escreve uma linha já validada pelo gate, com o fim de linha.
    pub fn write_line(&mut self, line: &str) -> Result<(), IcsFailure> {
        let mut bytes = Vec::with_capacity(line.len() + 1);
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        self.stream
            .write_all(&bytes)
            .and_then(|()| self.stream.flush())
            .map_err(|e| match failure(e.kind()) {
                IcsFailure::Timeout => IcsFailure::Interrupted("envio sem resposta"),
                other => other,
            })
    }

    pub fn shutdown(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl Drop for IcsSocket {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Mensagens fixas: o erro do sistema pode citar o endereço, que não vai para o diagnóstico.
fn failure(kind: ErrorKind) -> IcsFailure {
    match kind {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => IcsFailure::Timeout,
        ErrorKind::ConnectionRefused => IcsFailure::Unreachable("conexão recusada"),
        ErrorKind::NotConnected
        | ErrorKind::AddrNotAvailable
        | ErrorKind::NetworkUnreachable
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkDown => IcsFailure::Unreachable("rede indisponível"),
        _ => IcsFailure::Interrupted("conexão interrompida"),
    }
}
