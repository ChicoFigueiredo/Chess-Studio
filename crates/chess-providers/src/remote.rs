//! Engine remota (CX-11): o Stockfish do Chess-API.com como uma Engine da lista, pelo
//! **mesmo contrato de análise** que a interface já consome das Engines locais
//! (`AnalysisEvent`/`AnalysisUpdate` do `chess-uci`).
//!
//! - [`ChessApi`]: o cliente da API REST pública (`POST https://chess-api.com/v1`): a posição
//!   vai como **FEN e nada mais** (mais a profundidade e o número de variantes).
//! - [`RemoteEngine`]: o adaptador com a forma do `AnalysisController` (`analyze`, `stop`,
//!   `poll`), para aparecer como uma Engine na lista com o selo do Provedor
//!   ([`RemoteEngine::badge`]). **Sem troca silenciosa**: só consulta o serviço quando o
//!   usuário escolheu esta Engine e chamou `analyze`; nenhuma outra Engine recai nela.
//! - Capacidade "calcula": a Trava de fair play recusa durante uma Partida online contra
//!   pessoa; Provedor desligado: nada sai; `429`: a pausa é respeitada pela camada de rede.
//! - Todo Lance devolvido é validado pelo núcleo de regras (ADR-0003): o Lance ilegal vira
//!   erro e nada é mostrado; a variante principal é cortada no primeiro Lance ilegal.
//!
//! ```no_run
//! use chess_providers::{ChessApi, RemoteEngine};
//! use chess_uci::{AnalysisEvent, PositionSpec};
//! # fn demo(api: ChessApi) {
//! let mut engine = RemoteEngine::new(api);
//! let id = engine.analyze(PositionSpec::startpos(), 1);
//! // A cada quadro:
//! while let Some(event) = engine.poll() {
//!     if let AnalysisEvent::Update(update) = event {
//!         println!("{id}: {:?}", update.lines.first().map(|l| l.score));
//!     }
//! }
//! # }
//! ```

use crate::answer::{Answer, Query, fetch};
use crate::error::{Missing, ProviderError};
use chess_core::{CanonicalPosition, Move};
use chess_net::{Clock, Fetched, Net, ProviderId, Request};
use chess_uci::{
    AnalysisEvent, AnalysisLine, AnalysisUpdate, Failure, FailureKind, MAX_MULTIPV, PositionSpec,
    Score,
};
use serde_json::Value;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

const PROVIDER: ProviderId = ProviderId::ChessApi;
const URL: &str = "https://chess-api.com/v1";
/// Profundidade padrão do serviço.
pub const DEFAULT_DEPTH: u32 = 12;
/// Profundidade máxima do acesso comum (documentação de 11/11/2026).
pub const MAX_DEPTH: u32 = 18;
/// Comprimento máximo da linha principal lida de uma resposta (Lance principal mais a
/// continuação): o serviço manda poucas dezenas; mais que isso é resposta hostil e o excesso
/// é cortado **antes** de validar e gerar a notação (SEC-07).
const MAX_LINE_PLIES: usize = 64;
/// Nome da Engine na lista.
pub const ENGINE_NAME: &str = "Stockfish (Chess-API.com)";

/// Cliente da API de análise do Chess-API.com. Barato de clonar.
#[derive(Clone)]
pub struct ChessApi {
    net: Net,
    clock: Arc<dyn Clock>,
    depth: u32,
}

impl fmt::Debug for ChessApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChessApi")
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}

impl ChessApi {
    /// `clock` é o relógio injetado (o mesmo da `Net`).
    pub fn new(net: Net, clock: Arc<dyn Clock>) -> Self {
        Self {
            net,
            clock,
            depth: DEFAULT_DEPTH,
        }
    }

    /// Profundidade pedida (1 a [`MAX_DEPTH`]).
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = depth.clamp(1, MAX_DEPTH);
        self
    }

    /// Analisa `fen` com até `multipv` variantes (1 a [`MAX_MULTIPV`]). O pedido leva **só** o
    /// FEN, a profundidade e o número de variantes. As linhas vêm no ponto de vista das
    /// brancas, como no contrato de análise local; cada Lance foi validado pelo núcleo de
    /// regras.
    pub fn analyze_fen(&self, fen: &str, multipv: u8) -> Query<Vec<AnalysisLine>> {
        let position = match CanonicalPosition::from_fen(fen.trim()) {
            Ok(position) => position,
            Err(e) => {
                return Query::failed(ProviderError::invalid(format!("FEN inválido ({e})")));
            }
        };
        self.analyze_position(&position, multipv)
    }

    fn analyze_position(
        &self,
        position: &CanonicalPosition,
        multipv: u8,
    ) -> Query<Vec<AnalysisLine>> {
        let variants = multipv.clamp(1, MAX_MULTIPV);
        let body = serde_json::json!({
            "fen": position.to_fen(),
            "depth": self.depth,
            "variants": variants,
        });
        let request = Request::post(PROVIDER, URL)
            .accept("application/json")
            .body("application/json", body.to_string().into_bytes());
        let position = position.clone();
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            decode(&position, fetched)
        })
    }

    /// Instante atual do relógio injetado.
    pub fn now(&self) -> chess_net::Timestamp {
        self.clock.now()
    }
}

// ---- Decodificação --------------------------------------------------------------------------

fn malformed(detail: impl fmt::Display) -> ProviderError {
    ProviderError::malformed(PROVIDER, detail)
}

/// Número que o serviço manda ora como texto, ora como número (`"33"`, `33`, `"1"`, `-9`).
fn number(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| {
            // `33.0`: aceita só se for inteiro.
            n.as_f64()
                .filter(|f| f.fract() == 0.0 && f.abs() < 1e9)
                .map(|f| f as i64)
        }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn score(item: &Value) -> Result<Score, ProviderError> {
    if let Some(mate) = item.get("mate").and_then(number)
        && mate != 0
    {
        return i32::try_from(mate)
            .map(Score::Mate)
            .map_err(|_| malformed("mate fora da faixa"));
    }
    let cp = item
        .get("centipawns")
        .and_then(number)
        .or_else(|| {
            item.get("eval")
                .and_then(Value::as_f64)
                .map(|e| (e * 100.0).round() as i64)
        })
        .ok_or_else(|| malformed("avaliação ausente"))?;
    i32::try_from(cp)
        .map(Score::Cp)
        .map_err(|_| malformed("avaliação fora da faixa"))
}

/// A variante principal: o Lance e a continuação, cada um validado pelo núcleo de regras. O
/// Lance principal ilegal é erro; a continuação é cortada no primeiro Lance ilegal.
fn line(
    position: &CanonicalPosition,
    item: &Value,
    multipv: usize,
) -> Result<AnalysisLine, ProviderError> {
    let text = item
        .get("lan")
        .or_else(|| item.get("move"))
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("resposta sem Lance"))?;
    let best: Move = text
        .parse()
        .map_err(|_| malformed(format!("Lance «{text}» ilegível")))?;
    if !position.is_legal(best) {
        return Err(malformed(format!(
            "o serviço devolveu o Lance ilegal «{text}»; nada foi usado"
        )));
    }
    let mut pv = vec![best];
    let mut san = vec![
        position
            .san(best)
            .map_err(|_| malformed("Lance sem notação"))?,
    ];
    let mut current = position
        .play(best)
        .map_err(|_| malformed("Lance recusado pelas regras"))?;
    let continuation = item
        .get("continuationArr")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    // Alguns exemplos repetem o Lance principal no começo da continuação.
    let skip = usize::from(continuation.first().and_then(Value::as_str) == Some(text));
    for next in continuation.iter().skip(skip).take(MAX_LINE_PLIES - 1) {
        let Some(mv) = next.as_str().and_then(|t| t.parse::<Move>().ok()) else {
            break;
        };
        if !current.is_legal(mv) {
            break;
        }
        let (Ok(notation), Ok(after)) = (current.san(mv), current.play(mv)) else {
            break;
        };
        pv.push(mv);
        san.push(notation);
        current = after;
    }
    Ok(AnalysisLine {
        multipv,
        depth: item
            .get("depth")
            .and_then(number)
            .and_then(|d| u32::try_from(d).ok())
            .unwrap_or(0),
        seldepth: None,
        score: score(item)?,
        nodes: None,
        nps: None,
        time_ms: item
            .get("time")
            .and_then(number)
            .and_then(|t| u64::try_from(t).ok()),
        pv,
        san,
    })
}

fn decode(
    position: &CanonicalPosition,
    fetched: &Fetched,
) -> Result<Vec<AnalysisLine>, ProviderError> {
    let value: Value =
        serde_json::from_slice(&fetched.body).map_err(|_| malformed("a resposta não é JSON"))?;
    let items: Vec<&Value> = match &value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![&value],
        _ => return Err(malformed("formato inesperado")),
    };
    let mut lines = Vec::new();
    for item in items {
        // Só as variantes que a interface pode mostrar: o resto nem é validado.
        if lines.len() >= usize::from(MAX_MULTIPV) {
            break;
        }
        match item.get("type").and_then(Value::as_str) {
            // O serviço responde `200` com `type: "error"` (FEN recusado, por exemplo).
            Some("error") => {
                let code = item
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return Err(if code.contains("FEN") {
                    ProviderError::invalid("o serviço recusou a Posição (FEN)")
                } else {
                    malformed("o serviço respondeu com um erro")
                });
            }
            Some("move" | "bestmove") | None => {
                lines.push(line(position, item, lines.len() + 1)?);
            }
            // `info`: andamento, sem Lance.
            Some(_) => {}
        }
    }
    if lines.is_empty() {
        return Err(malformed("a resposta não traz Lance"));
    }
    lines.truncate(usize::from(MAX_MULTIPV));
    Ok(lines)
}

// ---- A Engine remota no contrato de análise ---------------------------------------------------

/// Texto para o usuário de um problema da Engine remota (sem código HTTP nem detalhe técnico).
pub fn user_message(error: &ProviderError) -> String {
    match error {
        ProviderError::Net(_) => "A análise remota não respondeu agora. Tente de novo em \
                                  instantes; a análise local continua disponível."
            .to_owned(),
        other => other.to_string(),
    }
}

struct Current {
    id: u64,
    query: Query<Vec<AnalysisLine>>,
}

/// A Engine remota com a forma do `AnalysisController`: `analyze`, `stop`, `poll`.
pub struct RemoteEngine {
    api: ChessApi,
    next_id: u64,
    current: Option<Current>,
    events: VecDeque<AnalysisEvent>,
    announced: bool,
    problem: Option<ProviderError>,
    last_origin: Option<Answer<()>>,
}

impl fmt::Debug for RemoteEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteEngine")
            .field("analyzing", &self.is_analyzing())
            .finish_non_exhaustive()
    }
}

impl RemoteEngine {
    pub fn new(api: ChessApi) -> Self {
        Self {
            api,
            next_id: 1,
            current: None,
            events: VecDeque::new(),
            announced: false,
            problem: None,
            last_origin: None,
        }
    }

    /// Nome na lista de Engines.
    pub fn name(&self) -> &'static str {
        ENGINE_NAME
    }

    /// Selo do Provedor (CX-11): a interface mostra junto do nome para ficar claro que a
    /// Posição sai do computador.
    pub fn badge(&self) -> &'static str {
        "Remota · Chess-API.com"
    }

    pub fn provider(&self) -> ProviderId {
        PROVIDER
    }

    /// Analisa `position` com `multipv` variantes (1 a [`MAX_MULTIPV`]); substitui a análise
    /// anterior e devolve o `id` do pedido. Só o FEN da Posição final sai (a Engine remota não
    /// vê o histórico: a contagem de repetições se perde).
    pub fn analyze(&mut self, position: PositionSpec, multipv: u8) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.problem = None;
        self.current = None;
        let start = match &position.fen {
            None => Ok(CanonicalPosition::start()),
            Some(fen) => CanonicalPosition::from_fen(fen)
                .map_err(|e| ProviderError::invalid(format!("FEN inválido ({e})"))),
        };
        let resolved = start.and_then(|start| {
            position.moves.iter().try_fold(start, |p, mv| {
                p.play(*mv)
                    .map_err(|_| ProviderError::invalid("Lance ilegal na sequência da Posição"))
            })
        });
        match resolved {
            Ok(position) => {
                self.current = Some(Current {
                    id,
                    query: self.api.analyze_position(&position, multipv),
                });
            }
            Err(error) => self.fail(error),
        }
        id
    }

    /// Para a análise: a resposta que ainda chegar é descartada e o pedido é cancelado.
    pub fn stop(&mut self) {
        self.current = None;
    }

    pub fn is_analyzing(&self) -> bool {
        self.current.is_some()
    }

    /// A Engine remota nunca "morre": um problema vale para o pedido, não para a Engine.
    pub fn is_dead(&self) -> bool {
        false
    }

    /// O problema tipado do último pedido (Provedor desligado, Trava de fair play, limite
    /// com o prazo, resposta malformada…); some quando um novo `analyze` começa.
    pub fn problem(&self) -> Option<&ProviderError> {
        self.problem.as_ref()
    }

    /// Origem da última resposta (Provedor, instante, licença).
    pub fn last_origin(&self) -> Option<&Answer<()>> {
        self.last_origin.as_ref()
    }

    fn fail(&mut self, error: ProviderError) {
        let kind = if error.is_transient() {
            FailureKind::Timeout
        } else {
            FailureKind::Protocol
        };
        self.events.push_back(AnalysisEvent::Failed(Failure::new(
            kind,
            user_message(&error),
        )));
        self.problem = Some(error);
    }

    /// Próximo evento, sem bloquear. O primeiro é `Ready`; depois, `Update` com as linhas
    /// (ponto de vista das brancas) ou `Failed` com o texto para o usuário (e o problema
    /// tipado em [`RemoteEngine::problem`]). Resposta de pedido já substituído é descartada.
    pub fn poll(&mut self) -> Option<AnalysisEvent> {
        if !self.announced {
            self.announced = true;
            self.events.push_front(AnalysisEvent::Ready {
                name: Some(ENGINE_NAME.to_owned()),
                threads: None,
                hash_mb: None,
                backend: None,
                backends: Vec::new(),
            });
        }
        if let Some(current) = &mut self.current
            && let Some(result) = current.query.try_take()
        {
            let id = current.id;
            self.current = None;
            match result {
                Ok(answer) => {
                    self.last_origin = Some(Answer {
                        value: (),
                        provenance: answer.provenance,
                        freshness: answer.freshness,
                        age: answer.age,
                    });
                    self.events.push_back(AnalysisEvent::Update(AnalysisUpdate {
                        id,
                        lines: answer.value,
                    }));
                }
                Err(error) => self.fail(error),
            }
        }
        self.events.pop_front()
    }
}
