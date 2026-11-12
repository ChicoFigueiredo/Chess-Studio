//! Transporte ICS (#237, CX-18): a segunda saída de rede do app, além do HTTP. Uma conexão
//! TCP de texto, linha a linha, com um servidor do protocolo ICS (o FICS). Passa pelo mesmo
//! gate dos pedidos HTTP ([`crate::Net::connect_ics`]): Provedor ligado, catálogo (só os
//! endereços de [`ics_endpoints`]), Trava de fair play, limites, estado da rede,
//! cancelamento, thread própria e consulta por quadro ([`IcsLink::try_next`]).
//!
//! Este arquivo não abre socket. Tem os tipos públicos, os endereços ICS do catálogo e o
//! enquadramento das linhas ([`IcsReader`]): tira as sequências telnet, junta linhas partidas
//! entre leituras, descarta linha gigante (e se ressincroniza na seguinte), troca byte
//! inválido por `\u{FFFD}` e nunca entra em pânico. O socket está em `ics_transport.rs`, e só
//! o gate (`net/ics.rs`) o usa. Interpretar as mensagens (prompt, `style 12`…) é do
//! adaptador do Provedor.
//!
//! **Canal sem cifra.** O ICS do FICS é texto puro (o *timeseal* só mede atraso e embaralha
//! com uma chave pública: não é criptografia). Por isso [`IcsEndpoint::encrypted`] é falso e
//! nenhuma senha pode sair por este transporte (CX-18, `docs/decisoes/2026-11-11-fics.md`).

use crate::catalog::{Capability, ProviderId};
use crate::error::NetError;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// Endereço ICS permitido: host e porta exatos, com a capacidade e a licença dos dados. Faz
/// parte do catálogo de Provedores (os endereços HTTP estão em [`crate::Endpoint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcsEndpoint {
    pub provider: ProviderId,
    pub host: &'static str,
    pub port: u16,
    pub capability: Capability,
    pub license: &'static str,
    /// O canal é cifrado ponta a ponta (TLS validado). Falso: tudo trafega em texto puro, e
    /// o adaptador recusa login com senha antes de abrir a conexão (CX-18).
    pub encrypted: bool,
}

/// Os endereços ICS do catálogo (revisão de 11/11/2026). A porta 5000 é a padrão documentada
/// pelos clientes do FICS; a 23 (telnet) fica de fora: é o mesmo serviço, sem ganho algum.
static ICS_ENDPOINTS: [IcsEndpoint; 1] = [IcsEndpoint {
    provider: ProviderId::Fics,
    host: "freechess.org",
    port: 5000,
    capability: Capability::Play,
    license: "Termos do FICS",
    encrypted: false,
}];

/// Todos os endereços ICS do catálogo.
pub fn ics_endpoints() -> &'static [IcsEndpoint] {
    &ICS_ENDPOINTS
}

/// O endereço ICS do catálogo para `provider`, `host` e `port`, se houver.
pub fn ics_endpoint(provider: ProviderId, host: &str, port: u16) -> Option<&'static IcsEndpoint> {
    ICS_ENDPOINTS
        .iter()
        .find(|e| e.provider == provider && e.host.eq_ignore_ascii_case(host) && e.port == port)
}

/// Linha máxima que o leitor aceita (bytes). Uma linha do `style 12` tem uns 200; as maiores
/// mensagens do FICS ficam bem abaixo disto.
pub const DEFAULT_ICS_MAX_LINE: usize = 16 * 1024;

/// Comando máximo que o app envia (bytes, sem o fim de linha).
pub const MAX_ICS_COMMAND: usize = 1024;

/// Eventos de uma conexão ICS à espera de quem lê, no máximo (#273). O canal não é cifrado:
/// um servidor hostil, ou qualquer um no caminho, pode despejar linhas sem parar. Com a fila
/// cheia, a leitura do socket espera (o TCP segura o resto do outro lado), então a memória
/// fica em `MAX_ICS_PENDING` linhas de até [`DEFAULT_ICS_MAX_LINE`] bytes, qualquer que seja
/// a entrada.
pub const MAX_ICS_PENDING: usize = 1024;

/// Pedido de conexão ICS: Provedor + endereço oficial do catálogo. Nos testes o host vai
/// para o servidor falso por [`crate::NetConfig::route`] (só loopback).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcsRequest {
    pub(crate) provider: ProviderId,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) max_line: Option<usize>,
}

impl IcsRequest {
    pub fn new(provider: ProviderId, host: impl Into<String>, port: u16) -> Self {
        Self {
            provider,
            host: host.into().trim().to_ascii_lowercase(),
            port,
            connect_timeout: None,
            max_line: None,
        }
    }

    /// O endereço ICS do catálogo do Provedor (o primeiro, se houver vários).
    pub fn to_catalog(provider: ProviderId) -> Option<Self> {
        ICS_ENDPOINTS
            .iter()
            .find(|e| e.provider == provider)
            .map(|e| Self::new(provider, e.host, e.port))
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    /// Prazo para abrir a conexão (padrão: o da camada de rede).
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Linha máxima aceita (padrão: [`DEFAULT_ICS_MAX_LINE`]).
    pub fn max_line(mut self, bytes: usize) -> Self {
        self.max_line = Some(bytes.max(1));
        self
    }

    /// `ics://host:porta`, curto e sem nada além do endereço (para erro e diagnóstico).
    pub(crate) fn display(&self) -> String {
        let host: String = self
            .host
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
            .take(80)
            .collect();
        format!("ics://{host}:{}", self.port)
    }
}

/// O que chegou de uma conexão ICS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcsEvent {
    /// A conexão TCP abriu.
    Connected,
    /// Uma linha completa, sem fim de linha, sem sequências telnet e sem caracteres de
    /// controle (exceto tabulação). Linhas vazias não são entregues.
    Line(String),
    /// O servidor parou no meio de uma linha: é o prompt (`login: `, `fics% `). Pode também
    /// ser o começo de uma linha que ainda não terminou; nesse caso a linha completa chega
    /// depois como [`IcsEvent::Line`].
    Prompt(String),
    /// Uma linha passou de `limit` bytes e foi descartada até o próximo fim de linha.
    LineTooLong { limit: usize },
    /// O servidor fechou a conexão.
    Ended,
    /// A conexão não abriu ou caiu (Provedor desligado, fora do catálogo, sem rede…).
    Failed(NetError),
}

/// Uma conexão ICS aberta por [`crate::Net::connect_ics`]. Consulte [`IcsLink::try_next`] a
/// cada quadro (nunca bloqueia). Descartar ou [`IcsLink::close`] fecha a conexão.
pub struct IcsLink {
    provider: ProviderId,
    events: Receiver<IcsEvent>,
    commands: Sender<String>,
    cancel: Arc<AtomicBool>,
}

impl fmt::Debug for IcsLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IcsLink")
            .field("provider", &self.provider)
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

impl IcsLink {
    pub(crate) fn new(
        provider: ProviderId,
        events: Receiver<IcsEvent>,
        commands: Sender<String>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            provider,
            events,
            commands,
            cancel,
        }
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    /// Próximo evento, se já chegou (nunca bloqueia).
    pub fn try_next(&self) -> Option<IcsEvent> {
        self.events.try_recv().ok()
    }

    /// Espera até `timeout` (tempo real) pelo próximo evento. Para testes e threads de
    /// fundo, nunca para a interface.
    pub fn next_timeout(&self, timeout: Duration) -> Option<IcsEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Envia uma linha de comando (o fim de linha vai junto). Só ASCII visível e espaço, até
    /// [`MAX_ICS_COMMAND`] bytes: um comando nunca carrega outra linha escondida nem byte de
    /// controle do telnet. Linha vazia é permitida (o "aperte Enter" do login).
    pub fn send_line(&self, line: &str) -> Result<(), NetError> {
        validate_command(line)?;
        if self.is_closed() {
            return Err(NetError::Cancelled);
        }
        self.commands
            .send(line.to_owned())
            .map_err(|_| NetError::Unreachable {
                provider: self.provider,
                detail: "conexão ICS encerrada".to_owned(),
            })
    }

    /// Fecha a conexão: a thread para na próxima volta (no máximo uma fração de segundo).
    pub fn close(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

impl Drop for IcsLink {
    fn drop(&mut self) {
        self.close();
    }
}

/// Comando aceitável para o servidor ICS.
pub(crate) fn validate_command(line: &str) -> Result<(), NetError> {
    if line.len() > MAX_ICS_COMMAND {
        return Err(NetError::invalid(format!(
            "comando ICS acima de {MAX_ICS_COMMAND} bytes"
        )));
    }
    if !line.bytes().all(|b| (b' '..=b'~').contains(&b)) {
        return Err(NetError::invalid(
            "comando ICS com quebra de linha, byte de controle ou caractere fora do ASCII",
        ));
    }
    Ok(())
}

/// Item do enquadramento: o que o leitor tirou dos bytes recebidos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcsItem {
    Line(String),
    Prompt(String),
    LineTooLong { limit: usize },
}

impl From<IcsItem> for IcsEvent {
    fn from(item: IcsItem) -> Self {
        match item {
            IcsItem::Line(line) => Self::Line(line),
            IcsItem::Prompt(prompt) => Self::Prompt(prompt),
            IcsItem::LineTooLong { limit } => Self::LineTooLong { limit },
        }
    }
}

const IAC: u8 = 255;
const SB: u8 = 250;
const SE: u8 = 240;
const WILL: u8 = 251;
const DONT: u8 = 254;

/// Estado do filtro telnet (RFC 854): comandos e subnegociações são descartados; `IAC IAC`
/// é o byte 255 como dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Telnet {
    Data,
    Iac,
    /// Depois de `IAC WILL/WONT/DO/DONT`: falta o byte da opção.
    Option,
    /// Dentro de `IAC SB … IAC SE`.
    Sub,
    SubIac,
}

/// Enquadramento incremental das linhas ICS. Puro (sem socket): o gate o alimenta com o que
/// o socket leu. Memória limitada a `max_line` bytes, qualquer que seja a entrada.
#[derive(Debug, Clone)]
pub struct IcsReader {
    max_line: usize,
    line: Vec<u8>,
    discarding: bool,
    telnet: Telnet,
    /// O último prompt entregue (não se repete enquanto a linha não muda).
    last_prompt: Option<String>,
}

impl IcsReader {
    pub fn new(max_line: usize) -> Self {
        Self {
            max_line: max_line.max(1),
            line: Vec::new(),
            discarding: false,
            telnet: Telnet::Data,
            last_prompt: None,
        }
    }

    /// Processa os bytes recebidos e devolve as linhas completas, as linhas descartadas e, se
    /// o que chegou termina no meio de uma linha, o prompt.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<IcsItem> {
        let mut out = Vec::new();
        for &byte in bytes {
            let Some(data) = self.telnet_filter(byte) else {
                continue;
            };
            self.data(data, &mut out);
        }
        if !self.discarding && !self.line.is_empty() {
            let tail = clean(&self.line);
            if !tail.trim().is_empty() && self.last_prompt.as_deref() != Some(tail.as_str()) {
                self.last_prompt = Some(tail.clone());
                out.push(IcsItem::Prompt(tail));
            }
        }
        out
    }

    fn telnet_filter(&mut self, byte: u8) -> Option<u8> {
        match self.telnet {
            Telnet::Data if byte == IAC => {
                self.telnet = Telnet::Iac;
                None
            }
            Telnet::Data => Some(byte),
            Telnet::Iac if byte == IAC => {
                self.telnet = Telnet::Data;
                Some(IAC)
            }
            Telnet::Iac => {
                self.telnet = match byte {
                    WILL..=DONT => Telnet::Option,
                    SB => Telnet::Sub,
                    _ => Telnet::Data,
                };
                None
            }
            Telnet::Option => {
                self.telnet = Telnet::Data;
                None
            }
            Telnet::Sub => {
                if byte == IAC {
                    self.telnet = Telnet::SubIac;
                }
                None
            }
            Telnet::SubIac => {
                self.telnet = if byte == SE {
                    Telnet::Data
                } else {
                    Telnet::Sub
                };
                None
            }
        }
    }

    fn data(&mut self, byte: u8, out: &mut Vec<IcsItem>) {
        if byte == b'\n' {
            if self.discarding {
                self.discarding = false;
            } else {
                let line = clean(&self.line);
                if !line.trim().is_empty() {
                    out.push(IcsItem::Line(line));
                }
            }
            self.line.clear();
            self.last_prompt = None;
            return;
        }
        if self.discarding {
            return;
        }
        if self.line.len() >= self.max_line {
            self.line.clear();
            self.discarding = true;
            self.last_prompt = None;
            out.push(IcsItem::LineTooLong {
                limit: self.max_line,
            });
            return;
        }
        self.line.push(byte);
    }
}

/// Texto da linha: UTF-8 com troca do que for inválido, sem `\r` e sem caractere de controle
/// (exceto tabulação).
fn clean(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| *c == '\t' || !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_prompt_parado_e_entregue_uma_vez_e_a_linha_completa_depois() {
        let mut reader = IcsReader::new(64);
        assert_eq!(
            reader.feed(b"Welcome\n\rlogin: "),
            vec![
                IcsItem::Line("Welcome".into()),
                IcsItem::Prompt("login: ".into())
            ]
        );
        assert_eq!(reader.feed(b""), vec![]);
        assert_eq!(
            reader.feed(b"guest\n\r"),
            vec![IcsItem::Line("login: guest".into())]
        );
    }

    #[test]
    fn telnet_some_e_iac_iac_vira_um_byte_invalido() {
        let mut reader = IcsReader::new(64);
        let input = [
            b'a', IAC, WILL, 1, b'b', IAC, SB, 24, 1, b'x', IAC, SE, b'c', IAC, IAC, b'\n',
        ];
        assert_eq!(
            reader.feed(&input),
            vec![IcsItem::Line("abc\u{FFFD}".into())]
        );
    }
}
