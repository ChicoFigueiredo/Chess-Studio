//! O gate do transporte ICS (#237): a mesma porta única dos pedidos HTTP, para a conexão TCP
//! de texto do FICS. Ordem, como no HTTP: consentimento do Provedor, catálogo (só os
//! endereços de [`crate::ics_endpoints`]), serviço liberado, Trava de fair play; depois o
//! token bucket do Provedor, a mesma conferência de novo logo antes de abrir o socket, e o
//! estado da rede (conexões ativas, Provedor falhando).
//!
//! A conexão vive numa thread própria, que a cada fatia curta confere o cancelamento e o
//! consentimento: desligar o Provedor fecha a conexão aberta. Os eventos esperam numa fila
//! de no máximo [`MAX_ICS_PENDING`]; com ela cheia, a thread para de ler o socket até haver
//! espaço (#273: o canal não é cifrado, e um despejo de linhas não enche a memória). Nada
//! aqui interpreta o protocolo; o adaptador lê as linhas de [`IcsLink`].

use super::{Net, Shared};
use crate::LocalProfileId;
use crate::error::NetError;
use crate::ics::{DEFAULT_ICS_MAX_LINE, MAX_ICS_PENDING, validate_command};
use crate::ics::{IcsEndpoint, IcsEvent, IcsLink, IcsReader, IcsRequest, ics_endpoint};
use crate::ics_transport::{IcsAddress, IcsFailure, IcsSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::Duration;

/// Fatia de espera da leitura: o atraso máximo para atender a um cancelamento ou comando.
const POLL: Duration = Duration::from_millis(20);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_CHUNK: usize = 8 * 1024;

/// Como a conexão terminou do lado de cá.
enum IcsEnd {
    /// O servidor fechou.
    Ended,
    /// Quem abriu fechou (ou descartou o [`IcsLink`]).
    Closed,
}

impl Net {
    /// Abre uma conexão ICS (TCP de texto) numa thread própria, pelo mesmo gate dos pedidos
    /// HTTP. Nunca bloqueia: uma conexão recusada pelo gate (Provedor desligado, endereço
    /// fora do catálogo) já volta com [`IcsEvent::Failed`], sem thread e sem socket.
    pub fn connect_ics(&self, request: IcsRequest) -> IcsLink {
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        let provider = request.provider;
        let (events, events_rx) = mpsc::sync_channel(MAX_ICS_PENDING);
        let (commands_tx, commands) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let link = IcsLink::new(provider, events_rx, commands_tx, cancel.clone());
        if let Err(err) = shared.admit_ics(&request, profile) {
            let _ = events.try_send(IcsEvent::Failed(err));
            return link;
        }
        shared.active_requests.fetch_add(1, Ordering::AcqRel);
        let worker = shared.clone();
        let failed = events.clone();
        let name = format!("chess-net-ics-{}", provider.key());
        let spawned = thread::Builder::new().name(name).spawn(move || {
            worker.run_ics(&request, profile, &cancel, &events, &commands);
            worker.active_requests.fetch_sub(1, Ordering::AcqRel);
        });
        if spawned.is_err() {
            shared.active_requests.fetch_sub(1, Ordering::AcqRel);
            let _ = failed.try_send(IcsEvent::Failed(NetError::Unreachable {
                provider,
                detail: "thread da conexão ICS indisponível".to_owned(),
            }));
        }
        link
    }
}

impl Shared {
    /// O gate do ICS: consentimento, catálogo, serviço liberado e Trava de fair play.
    fn admit_ics(
        &self,
        request: &IcsRequest,
        profile: LocalProfileId,
    ) -> Result<&'static IcsEndpoint, NetError> {
        let provider = request.provider;
        if !self.is_enabled(profile, provider) {
            return Err(NetError::ProviderDisabled { provider });
        }
        let endpoint = ics_endpoint(provider, &request.host, request.port).ok_or_else(|| {
            NetError::EndpointNotAllowed {
                provider,
                url: request.display(),
            }
        })?;
        let capability = endpoint.capability;
        provider
            .info()
            .service(capability)
            .filter(|s| s.status.allows_network())
            .ok_or(NetError::NotOffered {
                provider,
                capability,
            })?;
        self.guard.check(provider, capability)?;
        Ok(endpoint)
    }

    /// O host oficial ou, se a configuração o rotear (servidor falso dos testes), o endereço
    /// de loopback da rota. Rota que não é loopback é recusada: o ICS não fala TLS.
    fn ics_address(&self, endpoint: &'static IcsEndpoint) -> Result<IcsAddress, NetError> {
        let Some(base) = self.routes.get(endpoint.host) else {
            return Ok(IcsAddress::Host(endpoint.host, endpoint.port));
        };
        base.strip_prefix("http://")
            .and_then(IcsAddress::loopback)
            .ok_or_else(|| {
                NetError::invalid(format!(
                    "rota ICS para {} precisa ser de loopback",
                    endpoint.host
                ))
            })
    }

    fn run_ics(
        &self,
        request: &IcsRequest,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &SyncSender<IcsEvent>,
        commands: &Receiver<String>,
    ) {
        match self.ics_inner(request, profile, cancel, events, commands) {
            Ok(IcsEnd::Ended) => {
                deliver(events, IcsEvent::Ended, cancel, || true);
            }
            Ok(IcsEnd::Closed) | Err(NetError::Cancelled) => {}
            Err(err) => {
                deliver(events, IcsEvent::Failed(err), cancel, || true);
            }
        }
    }

    fn ics_inner(
        &self,
        request: &IcsRequest,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &SyncSender<IcsEvent>,
        commands: &Receiver<String>,
    ) -> Result<IcsEnd, NetError> {
        let provider = request.provider;
        let enabled = || self.is_enabled(profile, provider);
        // Fila cheia: espera espaço; para se quem lê fechou ou se o Provedor foi desligado.
        let push = |event: IcsEvent| -> Result<bool, NetError> {
            if deliver(events, event, cancel, &enabled) {
                Ok(true)
            } else if cancel.load(Ordering::Acquire) || enabled() {
                Ok(false)
            } else {
                Err(NetError::ProviderDisabled { provider })
            }
        };
        if !self.take_token(provider, cancel) {
            return Err(NetError::Cancelled);
        }
        // O consentimento ou a trava podem ter mudado durante a espera.
        let endpoint = self.admit_ics(request, profile)?;
        let address = self.ics_address(endpoint)?;
        let timeout = request.connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT);
        let mut socket = match IcsSocket::connect(&address, timeout, POLL, cancel) {
            Ok(socket) => socket,
            Err(failure) => return Err(self.ics_failure(provider, failure)),
        };
        self.mark_ok(provider);
        if !push(IcsEvent::Connected)? {
            return Ok(IcsEnd::Closed);
        }
        let mut reader = IcsReader::new(request.max_line.unwrap_or(DEFAULT_ICS_MAX_LINE));
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            if cancel.load(Ordering::Acquire) {
                socket.shutdown();
                return Ok(IcsEnd::Closed);
            }
            // Desligar o Provedor fecha a conexão aberta (ADR-0006).
            if !self.is_enabled(profile, provider) {
                socket.shutdown();
                return Err(NetError::ProviderDisabled { provider });
            }
            loop {
                match commands.try_recv() {
                    Ok(line) => {
                        // O `IcsLink` já validou; conferir de novo custa nada.
                        validate_command(&line)?;
                        if let Err(failure) = socket.write_line(&line) {
                            return self.ics_dropped(provider, failure, cancel);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        socket.shutdown();
                        return Ok(IcsEnd::Closed);
                    }
                }
            }
            match socket.read(&mut buf) {
                Ok(0) => return Ok(IcsEnd::Ended),
                Ok(n) => {
                    for item in reader.feed(&buf[..n]) {
                        if !push(item.into())? {
                            socket.shutdown();
                            return Ok(IcsEnd::Closed);
                        }
                    }
                }
                Err(IcsFailure::Timeout) => {}
                Err(failure) => return self.ics_dropped(provider, failure, cancel),
            }
        }
    }

    fn ics_dropped(
        &self,
        provider: crate::ProviderId,
        failure: IcsFailure,
        cancel: &AtomicBool,
    ) -> Result<IcsEnd, NetError> {
        if cancel.load(Ordering::Acquire) {
            return Ok(IcsEnd::Closed);
        }
        Err(self.ics_failure(provider, failure))
    }

    fn ics_failure(&self, provider: crate::ProviderId, failure: IcsFailure) -> NetError {
        let error = match failure {
            IcsFailure::Cancelled => return NetError::Cancelled,
            IcsFailure::Timeout => NetError::Timeout { provider },
            IcsFailure::Unreachable(detail) | IcsFailure::Interrupted(detail) => {
                NetError::Unreachable {
                    provider,
                    detail: detail.to_owned(),
                }
            }
        };
        self.mark_failing(provider, self.clock.now(), &error);
        error
    }
}

/// Entrega `event` esperando espaço na fila (no máximo [`MAX_ICS_PENDING`] eventos), em fatias
/// de [`POLL`]. `false`, sem entregar, se quem lê fechou ou descartou o [`IcsLink`] ou se
/// `keep_going` deixou de valer (Provedor desligado).
fn deliver(
    events: &SyncSender<IcsEvent>,
    mut event: IcsEvent,
    cancel: &AtomicBool,
    keep_going: impl Fn() -> bool,
) -> bool {
    loop {
        if cancel.load(Ordering::Acquire) || !keep_going() {
            return false;
        }
        match events.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(back)) => {
                event = back;
                thread::sleep(POLL);
            }
        }
    }
}
