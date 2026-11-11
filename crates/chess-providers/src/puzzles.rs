//! Pacotes de problemas do Lichess pela rede (CX-16, #238): a base pública CC0
//! (`database.lichess.org/lichess_db_puzzle.csv.zst`, centenas de MB, milhões de linhas)
//! baixada **em fluxo** pelo gate da camada de rede, descomprimida e lida linha a linha pelo
//! leitor do `chess-training` (o mesmo da importação de arquivo), e instalada como pacote.
//!
//! O que isso garante:
//!
//! - **nada é carregado por inteiro**: o corpo chega em pedaços pelo canal limitado de
//!   [`chess_net::Net::download`] e vai direto para o descompressor; só os Exercícios
//!   selecionados ficam na memória (no máximo [`MAX_PUZZLES`]);
//! - **a seleção encerra o download**: ao atingir a quantidade pedida, a leitura para e a
//!   conexão cai; o resto do arquivo nunca é baixado;
//! - **o corpo não vai para o cache** da rede, e tudo passa pelo gate (Provedor ligado,
//!   catálogo, Trava de fair play, limites);
//! - **cancelar não deixa pacote pela metade**: o pacote só é gravado no fim, de uma vez
//!   (arquivo temporário e troca), e só se não houve cancelamento nem falha.
//!
//! ```no_run
//! # use chess_providers::{Lichess, PuzzleEvent, PuzzleRequest};
//! # use chess_training::PuzzleFilter;
//! # fn demo(lichess: Lichess, dir: std::path::PathBuf) {
//! let mut job = lichess.download_puzzles(PuzzleRequest {
//!     filter: PuzzleFilter { themes: vec!["mateIn2".into()], max_count: 500, ..Default::default() },
//!     dir,
//!     id: "lichess-mate-em-2".into(),
//!     name: "Lichess: mate em 2".into(),
//! });
//! // A cada quadro (nunca bloqueia):
//! while let Some(event) = job.try_next() {
//!     match event {
//!         PuzzleEvent::Progress(p) => println!("{} lidos, {} selecionados", p.read, p.selected),
//!         PuzzleEvent::Finished(done) => println!("{:?}", done.pack),
//!         PuzzleEvent::Failed(error) => println!("{error}"),
//!     }
//! }
//! # }
//! ```

use crate::error::ProviderError;
use crate::lichess::{Lichess, PROVIDER};
use chess_net::{Download, DownloadEvent, ImportMode, NetError, Provenance, Request};
use chess_training::{
    ImportProgress, ImportReport, PackInfo, PuzzleFilter, civil_date, import_lichess_csv,
    install_pack_dated, open_puzzle_reader,
};
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Servidor da base pública de problemas.
pub const PUZZLE_HOST: &str = "database.lichess.org";
/// O arquivo de problemas (Zstandard), atualizado pelo Lichess.
pub const PUZZLE_PATH: &str = "/lichess_db_puzzle.csv.zst";
/// Teto de Exercícios por pacote baixado (a seleção fica na memória até instalar).
pub const MAX_PUZZLES: usize = 10_000;

/// O que baixar e onde instalar.
#[derive(Debug, Clone)]
pub struct PuzzleRequest {
    /// Temas (identificadores do Lichess), faixa de dificuldade e quantidade.
    pub filter: PuzzleFilter,
    /// Pasta dos pacotes do Perfil local.
    pub dir: PathBuf,
    /// Identificador do pacote (um slug livre; quem chama garante que não repete).
    pub id: String,
    /// Nome mostrado ao usuário.
    pub name: String,
}

/// Andamento: o que já veio da rede e o que a leitura já fez.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PuzzleProgress {
    /// Bytes recebidos da rede (comprimidos).
    pub downloaded: u64,
    /// Tamanho do arquivo, quando o servidor anuncia.
    pub total: Option<u64>,
    /// Problemas lidos.
    pub read: u64,
    /// Problemas que atendem à seleção até agora.
    pub selected: usize,
}

/// O fim de uma execução bem-sucedida (instalou, não achou nada ou foi cancelada).
#[derive(Debug, Clone, PartialEq)]
pub struct PuzzleOutcome {
    /// O pacote instalado; `None` se foi cancelado ou nenhum problema atendeu à seleção.
    pub pack: Option<PackInfo>,
    pub report: ImportReport,
    /// Cancelado por quem pediu: nada foi instalado.
    pub cancelled: bool,
    /// Origem do arquivo: Provedor, instante, licença e endereço.
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PuzzleEvent {
    Progress(PuzzleProgress),
    Finished(PuzzleOutcome),
    Failed(ProviderError),
}

/// Baixada em andamento. Descartar cancela (a conexão cai e nada é instalado).
#[derive(Debug)]
pub struct PuzzleDownload {
    rx: Receiver<PuzzleEvent>,
    cancel: Arc<AtomicBool>,
    last: PuzzleProgress,
    done: bool,
}

impl PuzzleDownload {
    /// Próximo evento, se já chegou (nunca bloqueia). Chame a cada quadro.
    pub fn try_next(&mut self) -> Option<PuzzleEvent> {
        if self.done {
            return None;
        }
        let event = match self.rx.try_recv() {
            Ok(event) => event,
            Err(mpsc::TryRecvError::Empty) => return None,
            // A thread sumiu sem avisar (pânico): trata como falha.
            Err(mpsc::TryRecvError::Disconnected) => PuzzleEvent::Failed(ProviderError::Net(
                NetError::Unreachable {
                    provider: PROVIDER,
                    detail: "a leitura dos problemas parou sem aviso".to_owned(),
                },
            )),
        };
        match &event {
            PuzzleEvent::Progress(p) => self.last = *p,
            PuzzleEvent::Finished(_) | PuzzleEvent::Failed(_) => self.done = true,
        }
        Some(event)
    }

    /// O último andamento recebido.
    pub fn progress(&self) -> PuzzleProgress {
        self.last
    }

    /// Pede o cancelamento; o resultado chega como `Finished` com `cancelled`.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn is_finished(&self) -> bool {
        self.done
    }
}

impl Drop for PuzzleDownload {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Lichess {
    /// Baixa os problemas do Lichess, seleciona e instala o pacote em segundo plano. O pedido
    /// passa pelo gate: com o Provedor desligado, nada sai e o primeiro evento é
    /// `Failed(Disabled)`.
    pub fn download_puzzles(&self, request: PuzzleRequest) -> PuzzleDownload {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let job = PuzzleDownload {
            rx,
            cancel: cancel.clone(),
            last: PuzzleProgress::default(),
            done: false,
        };
        let net_request = Request::get(PROVIDER, format!("https://{PUZZLE_HOST}{PUZZLE_PATH}"));
        let download = self.net.download(net_request);
        let provenance = Provenance {
            provider: PROVIDER,
            queried_at: self.clock.now(),
            license: crate::answer::license_of(PROVIDER, PUZZLE_HOST, PUZZLE_PATH),
            mode: ImportMode::Download,
            source: format!("https://{PUZZLE_HOST}{PUZZLE_PATH}"),
        };
        let day = day_text(self.clock.now().as_millis());
        let spawned = thread::Builder::new()
            .name("chess-providers-problemas".to_owned())
            .spawn({
                let tx = tx.clone();
                move || {
                    let event = match run(download, &request, &cancel, &tx, provenance, &day) {
                        Ok(outcome) => PuzzleEvent::Finished(outcome),
                        Err(error) => PuzzleEvent::Failed(error),
                    };
                    let _ = tx.send(event);
                }
            });
        if spawned.is_err() {
            // O `tx` do closure caiu com a thread que não nasceu: o `Disconnected` acima avisa.
            drop(tx);
        }
        job
    }
}

/// `AAAA-MM-DD` de um instante (UTC).
fn day_text(millis: i64) -> String {
    let (year, month, day) = civil_date(millis.div_euclid(86_400_000));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Guarda o erro da rede que derrubou a leitura (o leitor de CSV só o vê como texto).
type Slot = Arc<Mutex<Option<NetError>>>;

struct Tap<R> {
    inner: R,
    slot: Slot,
}

impl<R: Read> Read for Tap<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner.read(buf);
        if let Err(e) = &result
            && let Some(net) = e.get_ref().and_then(|s| s.downcast_ref::<NetError>())
            && let Ok(mut slot) = self.slot.lock()
        {
            *slot = Some(net.clone());
        }
        result
    }
}

fn run(
    download: Download,
    request: &PuzzleRequest,
    cancel: &Arc<AtomicBool>,
    tx: &Sender<PuzzleEvent>,
    provenance: Provenance,
    day: &str,
) -> Result<PuzzleOutcome, ProviderError> {
    let meter = download.meter();
    // O gate responde antes do primeiro byte: desligado, fora do catálogo, limitado…
    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(cancelled(provenance));
        }
        match download.next_timeout(Duration::from_millis(100)) {
            Some(DownloadEvent::Started { .. }) => break,
            Some(DownloadEvent::Failed(error)) => return Err(error.into()),
            Some(DownloadEvent::Ended) => break,
            Some(DownloadEvent::Chunk(_)) | None => {}
        }
    }
    let slot: Slot = Slot::default();
    let source = Tap {
        inner: download.into_reader(),
        slot: slot.clone(),
    };
    let net_failure = |fallback: String| {
        slot.lock()
            .ok()
            .and_then(|mut s| s.take())
            .map_or_else(
                || ProviderError::malformed(PROVIDER, fallback),
                ProviderError::from,
            )
    };
    let reader = match open_puzzle_reader(source, true) {
        Ok(reader) => reader,
        Err(_) if cancel.load(Ordering::Acquire) => return Ok(cancelled(provenance)),
        Err(e) => return Err(net_failure(e.to_string())),
    };
    let mut filter = request.filter.clone();
    filter.max_count = filter.max_count.clamp(1, MAX_PUZZLES);
    let (exercises, report) = match import_lichess_csv(
        reader,
        &filter,
        cancel,
        &mut |p: &ImportProgress| {
            let _ = tx.send(PuzzleEvent::Progress(PuzzleProgress {
                downloaded: meter.received(),
                total: meter.total(),
                read: p.lines,
                selected: p.imported,
            }));
        },
    ) {
        Ok(done) => done,
        Err(_) if cancel.load(Ordering::Acquire) => return Ok(cancelled(provenance)),
        Err(e) => return Err(net_failure(e.to_string())),
    };
    // Fim do arquivo antes de atender a seleção é normal (seleção rara); fim por
    // cancelamento não instala nada.
    if report.cancelled || cancel.load(Ordering::Acquire) {
        return Ok(PuzzleOutcome {
            pack: None,
            report,
            cancelled: true,
            provenance,
        });
    }
    let pack = if exercises.is_empty() {
        None
    } else {
        let info = install_pack_dated(
            &request.dir,
            &request.id,
            &request.name,
            Some(day),
            &exercises,
        )
        .map_err(|e| ProviderError::invalid(e.to_string()))?;
        Some(info)
    };
    Ok(PuzzleOutcome {
        pack,
        report,
        cancelled: false,
        provenance,
    })
}

fn cancelled(provenance: Provenance) -> PuzzleOutcome {
    PuzzleOutcome {
        pack: None,
        report: ImportReport {
            cancelled: true,
            ..ImportReport::default()
        },
        cancelled: true,
        provenance,
    }
}
