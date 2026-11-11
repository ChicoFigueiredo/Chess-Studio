//! Download em fluxo de corpo binário (arquivos grandes de bases públicas, como os problemas
//! do Lichess em `.csv.zst`), pelo mesmo gate dos demais pedidos: consentimento, catálogo,
//! Trava de fair play, limites e diário de operações.
//!
//! Diferente de [`Net::fetch`], o corpo **nunca** vai para o cache nem para a memória inteira:
//! chega em pedaços de até 64 KiB por um canal limitado, então quem lê devagar (ou para)
//! faz a conexão esperar (ou cair), em vez de a camada baixar o arquivo todo à frente.
//! Descartar o [`Download`] (ou o [`DownloadReader`]) encerra a conexão na hora.

use super::{Net, Shared};
use crate::LocalProfileId;
use crate::catalog::Capability;
use crate::error::NetError;
use crate::request::Request;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

/// Tamanho máximo de cada pedaço entregue.
const CHUNK: usize = 64 * 1024;
/// Pedaços em trânsito: o teto de memória do download (8 x 64 KiB).
const IN_FLIGHT: usize = 8;

/// O que o download entrega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEvent {
    /// A resposta abriu. `total` é o tamanho anunciado do corpo, se o Provedor disse.
    Started { total: Option<u64> },
    /// Bytes do corpo, em ordem.
    Chunk(Vec<u8>),
    /// O corpo acabou por inteiro.
    Ended,
    /// Não abriu, caiu ou veio cortado.
    Failed(NetError),
}

/// Contadores de andamento, baratos de clonar: a interface os lê por quadro enquanto outra
/// thread consome o corpo.
#[derive(Debug, Clone, Default)]
pub struct DownloadMeter {
    received: Arc<AtomicU64>,
    /// `0` = tamanho desconhecido.
    total: Arc<AtomicU64>,
}

impl DownloadMeter {
    /// Bytes já entregues a quem lê.
    pub fn received(&self) -> u64 {
        self.received.load(Ordering::Relaxed)
    }

    /// Tamanho anunciado do corpo, quando se sabe.
    pub fn total(&self) -> Option<u64> {
        Some(self.total.load(Ordering::Relaxed)).filter(|t| *t > 0)
    }

    fn note(&self, event: &DownloadEvent) {
        match event {
            DownloadEvent::Started { total } => {
                self.total.store(total.unwrap_or(0), Ordering::Relaxed);
            }
            DownloadEvent::Chunk(bytes) => {
                self.received
                    .fetch_add(bytes.len() as u64, Ordering::Relaxed);
            }
            DownloadEvent::Ended | DownloadEvent::Failed(_) => {}
        }
    }
}

/// Download aberto por [`Net::download`]. Descartar ou [`Download::close`] encerra a conexão.
#[derive(Debug)]
pub struct Download {
    rx: Receiver<DownloadEvent>,
    cancel: Arc<AtomicBool>,
    meter: DownloadMeter,
}

impl Download {
    /// Próximo evento, se já chegou (nunca bloqueia).
    pub fn try_next(&self) -> Option<DownloadEvent> {
        self.next_timeout(Duration::ZERO)
    }

    /// Espera até `timeout` (tempo real) pelo próximo evento.
    pub fn next_timeout(&self, timeout: Duration) -> Option<DownloadEvent> {
        let event = match self.rx.recv_timeout(timeout) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
        };
        self.meter.note(&event);
        Some(event)
    }

    pub fn meter(&self) -> DownloadMeter {
        self.meter.clone()
    }

    /// Encerramento controlado: a conexão cai na próxima leitura.
    pub fn close(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Entrega o corpo como um `Read` **bloqueante**, para um decodificador (descompressão)
    /// rodando em segundo plano; nunca use na thread da interface. O fim limpo é `Ok(0)`; um
    /// download cortado ou recusado é um erro de leitura que carrega o [`NetError`].
    pub fn into_reader(self) -> DownloadReader {
        DownloadReader {
            download: self,
            current: Vec::new(),
            at: 0,
            finished: false,
        }
    }
}

impl Drop for Download {
    fn drop(&mut self) {
        self.close();
    }
}

/// O corpo do download como `Read`. Veja [`Download::into_reader`].
#[derive(Debug)]
pub struct DownloadReader {
    download: Download,
    current: Vec<u8>,
    at: usize,
    finished: bool,
}

impl DownloadReader {
    pub fn meter(&self) -> DownloadMeter {
        self.download.meter()
    }
}

impl Read for DownloadReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.at < self.current.len() {
                let n = buf.len().min(self.current.len() - self.at);
                buf[..n].copy_from_slice(&self.current[self.at..self.at + n]);
                self.at += n;
                return Ok(n);
            }
            if self.finished {
                return Ok(0);
            }
            // Acorda de tempos em tempos só para notar o cancelamento ou uma thread que sumiu.
            match self.download.rx.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => {
                    self.download.meter.note(&event);
                    match event {
                        DownloadEvent::Started { .. } => {}
                        DownloadEvent::Chunk(bytes) => {
                            self.current = bytes;
                            self.at = 0;
                        }
                        DownloadEvent::Ended => self.finished = true,
                        DownloadEvent::Failed(error) => {
                            self.finished = true;
                            return Err(io::Error::other(error));
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.download.cancel.load(Ordering::Acquire) {
                        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.finished = true;
                    if self.download.cancel.load(Ordering::Acquire) {
                        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "o download terminou sem aviso",
                    ));
                }
            }
        }
    }
}

impl Net {
    /// Baixa o corpo de `request` em fluxo, numa thread própria, pelo mesmo gate. Sem cache e
    /// sem repetição: o corpo vai direto a quem lê (ver [`Download`]). O endereço precisa ser
    /// do catálogo com a capacidade [`Capability::Download`]; qualquer outro volta como
    /// [`NetError::EndpointNotAllowed`] (aqui não se baixa o que é consulta).
    pub fn download(&self, request: Request) -> Download {
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        let (tx, rx) = mpsc::sync_channel(IN_FLIGHT);
        let cancel = Arc::new(AtomicBool::new(false));
        let meter = DownloadMeter::default();
        let download = Download {
            rx,
            cancel: cancel.clone(),
            meter: meter.clone(),
        };
        let started = shared.clock.now();
        let admitted = shared.admit(&request, profile).and_then(|a| {
            if a.endpoint.capability == Capability::Download {
                Ok(a)
            } else {
                Err(NetError::EndpointNotAllowed {
                    provider: request.provider,
                    url: crate::request::sanitized(&request.url),
                })
            }
        });
        if let Err(err) = admitted {
            shared.record(&request, None, started, &Err(err.clone()));
            let _ = tx.try_send(DownloadEvent::Failed(err));
            return download;
        }
        shared.active_requests.fetch_add(1, Ordering::AcqRel);
        let worker = shared.clone();
        let name = format!("chess-net-download-{}", request.provider.key());
        let spawned = thread::Builder::new().name(name).spawn(move || {
            worker.run_download(&request, profile, &cancel, &tx);
            worker.active_requests.fetch_sub(1, Ordering::AcqRel);
        });
        if spawned.is_err() {
            shared.active_requests.fetch_sub(1, Ordering::AcqRel);
        }
        download
    }
}

/// Como terminou o download do lado de cá.
enum End {
    Whole,
    Closed,
}

impl Shared {
    fn run_download(
        &self,
        request: &Request,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &SyncSender<DownloadEvent>,
    ) {
        let started = self.clock.now();
        let result = self.download_inner(request, profile, cancel, events);
        let endpoint = self.admit(request, profile).ok().map(|a| a.endpoint);
        let outcome = match &result {
            Ok(_) => Ok(super::empty_fetched(request, started)),
            Err(e) => Err(e.clone()),
        };
        self.record(request, endpoint, started, &outcome);
        match result {
            Ok(End::Whole) => {
                send(events, DownloadEvent::Ended, cancel);
            }
            Ok(End::Closed) | Err(NetError::Cancelled) => {}
            Err(err) => {
                send(events, DownloadEvent::Failed(err), cancel);
            }
        }
    }

    fn download_inner(
        &self,
        request: &Request,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &SyncSender<DownloadEvent>,
    ) -> Result<End, NetError> {
        let provider = request.provider;
        let (head, mut body) = self.open_body(request, profile, cancel)?;
        let total = head.content_length;
        if !send(events, DownloadEvent::Started { total }, cancel) {
            return Ok(End::Closed);
        }
        let mut received = 0u64;
        let mut buf = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Ok(End::Closed);
            }
            match body.read(&mut buf) {
                Ok(0) => {
                    return match total {
                        Some(total) if received < total => Err(NetError::Unreachable {
                            provider,
                            detail: "download interrompido".to_owned(),
                        }),
                        _ => Ok(End::Whole),
                    };
                }
                Ok(n) => {
                    received += n as u64;
                    if !send(events, DownloadEvent::Chunk(buf[..n].to_vec()), cancel) {
                        return Ok(End::Closed);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(NetError::Timeout { provider });
                }
                Err(_) => {
                    if cancel.load(Ordering::Acquire) {
                        return Ok(End::Closed);
                    }
                    return Err(NetError::Unreachable {
                        provider,
                        detail: "download interrompido".to_owned(),
                    });
                }
            }
        }
    }
}

/// Entrega o evento esperando espaço no canal; `false` se quem lê saiu ou cancelou.
fn send(events: &SyncSender<DownloadEvent>, mut event: DownloadEvent, cancel: &AtomicBool) -> bool {
    loop {
        match events.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(back)) => {
                if cancel.load(Ordering::Acquire) {
                    return false;
                }
                event = back;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
