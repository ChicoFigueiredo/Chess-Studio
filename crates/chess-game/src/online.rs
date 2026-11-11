//! Partida online (ADR-0007, CX-07, CX-19): uma Partida comum cuja autoridade é o Provedor.
//!
//! O controlador continua sendo o único que escreve (ADR-0001), mas só registra o que o
//! Provedor confirmou. O Provedor fala por **retratos** ([`ProviderSnapshot`]): a lista de
//! Lances desde o início, os relógios, a proposta de empate e o fim. Cada retrato é aplicado
//! pelo [`crate::Command::Provider`]: entra só o que falta; retrato repetido ou atrasado não
//! gera Evento e nunca regride a Partida; retrato com outro prefixo (ou com Lance ilegal) é
//! [`Desync`], tipado, sem tocar no log.
//!
//! O Lance do jogador local é uma **intenção enviada** ([`SentMove`]): fica fora do log, no
//! máximo uma, até o Provedor confirmar o mesmo Lance (vira `MoveCompleted`), confirmar outro
//! ou recusar (é descartada). As Políticas de arbitragem locais não se aplicam: abandonar,
//! propor empate e abortar são pedidos ao Provedor, feitos pela camada de serviço.

use crate::command::Command;
use crate::event::EventKind;
use crate::outcome::Outcome;
use crate::policy::ArbitrationPolicy;
use crate::rejection::Rejection;
use crate::state::GameState;
use crate::wire;
use chess_core::{CanonicalPosition, Color, FenError, Move, Piece, Square};
use serde::{Deserialize, Serialize};
use std::fmt;

/// O que o Provedor declarou ao começar a Partida online. Imutável, gravado no
/// [`crate::GameSetup`] (e, com ele, no log da Partida).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OnlineGame {
    /// Chave estável do Provedor (`"lichess"`), a mesma do catálogo da camada de rede.
    pub provider: String,
    /// Identificador da Partida no Provedor.
    pub remote_id: String,
    /// Lado do jogador local.
    #[serde(with = "wire::color")]
    pub local_color: Color,
    pub opponent: Opponent,
    pub white: OnlinePlayer,
    pub black: OnlinePlayer,
    /// Vale rating no Provedor.
    pub rated: bool,
    /// Correspondência: dias por Lance (sem relógio de xadrez). `None` em tempo real.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_per_move: Option<u32>,
}

/// Sufixo da chave do Provedor (`OnlineGame::provider`) numa Partida da **Engine online**:
/// quem joga do lado local é uma Engine declarada (Bot API), nunca uma pessoa. A chave
/// `"lichess-bot"` separa para sempre essa Partida das de pessoa (`"lichess"`).
pub const ENGINE_GAME_SUFFIX: &str = "-bot";

impl OnlineGame {
    /// A Partida é da Engine online (Conexão de Engine, Bot API): não conta como Partida de
    /// pessoa e nunca liga a Trava de fair play.
    pub fn is_engine_game(&self) -> bool {
        self.provider.ends_with(ENGINE_GAME_SUFFIX)
    }
}

/// Um jogador como o Provedor o apresenta.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OnlinePlayer {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u32>,
}

/// Quem joga do outro lado. Contra a Engine do Provedor não há Trava de fair play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Opponent {
    /// Uma pessoa (inclui correspondência).
    Human,
    /// A Engine do próprio Provedor (Lichess: níveis 1 a 8).
    ProviderEngine { level: u8 },
}

/// Variante declarada pelo Provedor. O app só joga xadrez padrão.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Xadrez padrão (no Lichess, `standard` e `fromPosition`).
    Standard,
    /// Qualquer outra, com o nome que o Provedor usa: recusada.
    Other(String),
}

/// Por que a Partida online não pôde ser criada.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OnlineSetupError {
    /// Só xadrez padrão (CX-15): nada foi criado.
    UnsupportedVariant(String),
    /// A Posição inicial do Provedor não é um FEN válido.
    InvalidFen(FenError),
}

impl fmt::Display for OnlineSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVariant(v) => {
                write!(f, "a variante «{v}» não é aceita: só xadrez padrão")
            }
            Self::InvalidFen(e) => write!(f, "Posição inicial inválida: {e}"),
        }
    }
}

impl std::error::Error for OnlineSetupError {}

/// Relógios do Provedor no instante em que o retrato chegou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderClock {
    pub white_ms: u64,
    pub black_ms: u64,
    /// O relógio do lado da vez está correndo (no Lichess, só depois do primeiro Lance de
    /// cada lado).
    pub running: bool,
}

/// Como o Provedor encerrou a Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderEnd {
    /// Com resultado (mate, tempo, abandono, empate, desconexão…).
    Decided(Outcome),
    /// Abortada, ou encerrada pelo Provedor sem resultado.
    Aborted,
}

/// Retrato: o estado completo da Partida no Provedor (no Lichess, `gameFull`/`gameState`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderSnapshot {
    /// Todos os Lances desde a Posição inicial, em UCI. O roque pode vir como "rei para a
    /// torre" (`e1h1`).
    pub moves: Vec<String>,
    /// `None` sem relógio de xadrez (correspondência).
    pub clock: Option<ProviderClock>,
    /// Proposta de empate em aberto e de quem.
    pub draw_offer: Option<Color>,
    /// Presente quando o Provedor já encerrou a Partida.
    pub end: Option<ProviderEnd>,
}

/// Por que a Partida local e o Provedor não concordam. O log não muda; um retrato coerente
/// desfaz a marca.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Desync {
    /// O retrato tem outro Lance no meio-lance `ply` (a partir de 1) já registrado.
    Diverged { ply: usize },
    /// O Lance `uci` do meio-lance `ply` é ilegal (ou ilegível) na Posição local.
    IllegalMove { ply: usize, uci: String },
}

impl fmt::Display for Desync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diverged { ply } => write!(
                f,
                "o Provedor tem outro Lance no meio-lance {ply}: a Partida local não é reescrita"
            ),
            Self::IllegalMove { ply, uci } => write!(
                f,
                "o Lance «{uci}» do Provedor (meio-lance {ply}) é ilegal na Posição local"
            ),
        }
    }
}

/// Intenção enviada: o Lance do jogador local à espera do Provedor. Nunca é a Posição
/// canônica; `position_after` é o que a interface desenha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMove {
    /// Meio-lance que o Lance ocuparia, a partir de 1.
    pub ply: usize,
    pub side: Color,
    pub mv: Move,
    pub san: String,
    pub position_after: CanonicalPosition,
}

/// A parte online do [`crate::Snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineView {
    pub sent_move: Option<SentMove>,
    pub desync: Option<Desync>,
    /// O Provedor abortou a Partida (sem resultado).
    pub aborted: bool,
}

/// Estado volátil da Partida online: fora do log (ADR-0007), reconstruído pela sessão.
#[derive(Debug, Clone, Default)]
pub(crate) struct OnlineRuntime {
    pub sent: Option<SentMove>,
    pub desync: Option<Desync>,
}

/// "Regras do Provedor": nenhuma Política local; só o retrato muda a Partida (CX-19).
pub(crate) struct ProviderRules;

pub(crate) const PROVIDER_RULES: ProviderRules = ProviderRules;

impl ArbitrationPolicy for ProviderRules {
    fn decide(&self, state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection> {
        match command {
            Command::Provider(snapshot) => reconcile(state, snapshot),
            other => Err(Rejection::NotSupportedByPolicy(other.kind())),
        }
    }
}

/// Lance do Provedor validado pelo núcleo de regras, aceitando o roque como "rei para a
/// torre" (notação compatível com Chess960).
pub(crate) fn provider_move(position: &CanonicalPosition, uci: &str) -> Option<Move> {
    let mv: Move = uci.trim().parse().ok()?;
    if position.is_legal(mv) {
        return Some(mv);
    }
    let side = position.side_to_move();
    let king = position.piece_at(mv.from()) == Some((side, Piece::King));
    let rook = position.piece_at(mv.to()) == Some((side, Piece::Rook));
    if !(king && rook && mv.from().rank() == mv.to().rank() && mv.promotion().is_none()) {
        return None;
    }
    let file = if mv.to().file() > mv.from().file() {
        6
    } else {
        2
    };
    let castle = Move::new(mv.from(), Square::new(file, mv.from().rank())?, None);
    position.is_legal(castle).then_some(castle)
}

/// Os Eventos que levam o estado local ao retrato, ou o motivo da dessincronização.
/// Tudo ou nada: se um Lance falha, nenhum Evento é devolvido.
fn reconcile(state: &GameState, snapshot: &ProviderSnapshot) -> Result<Vec<EventKind>, Rejection> {
    // Fim confirmado é final: retratos depois dele (repetidos ou atrasados) não mudam nada.
    if state.is_over() {
        return Ok(Vec::new());
    }
    let have = state.played.len();
    for (i, uci) in snapshot.moves.iter().take(have).enumerate() {
        let before = if i == 0 {
            &state.start
        } else {
            &state.played[i - 1].position
        };
        if provider_move(before, uci) != Some(state.played[i].mv) {
            return Err(Rejection::Desynced(Desync::Diverged { ply: i + 1 }));
        }
    }
    // Retrato atrasado (prefixo do nosso): nada a fazer, nada regride.
    if snapshot.moves.len() < have {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    let mut position = state.position().clone();
    for (i, uci) in snapshot.moves.iter().enumerate().skip(have) {
        let illegal = || {
            Rejection::Desynced(Desync::IllegalMove {
                ply: i + 1,
                uci: uci.clone(),
            })
        };
        let mv = provider_move(&position, uci).ok_or_else(illegal)?;
        let side = position.side_to_move();
        let san = position.san(mv).map_err(|_| illegal())?;
        position = position.play(mv).map_err(|_| illegal())?;
        events.push(EventKind::MoveCompleted { side, mv, san });
    }
    let moved = snapshot.moves.len() > have;
    if let (Some(clock), Some(_)) = (snapshot.clock, &state.clock) {
        let running = (clock.running && snapshot.end.is_none()).then(|| position.side_to_move());
        let changed = match state.last_clock_report {
            None => true,
            // No fim, o relógio para de qualquer jeito: só os tempos contam.
            Some((w, b, _)) if snapshot.end.is_some() => (w, b) != (clock.white_ms, clock.black_ms),
            Some(last) => last != (clock.white_ms, clock.black_ms, running),
        };
        if moved || changed {
            events.push(EventKind::ClockReported {
                white_ms: clock.white_ms,
                black_ms: clock.black_ms,
                running,
            });
        }
    }
    // Um Lance encerra a proposta anterior (fim da vez); o retrato diz se há outra.
    let offer_now = if moved { None } else { state.draw_offer };
    if snapshot.end.is_none() && offer_now != snapshot.draw_offer {
        if let Some(by) = offer_now {
            events.push(EventKind::DrawOfferWithdrawn { by });
        }
        if let Some(by) = snapshot.draw_offer {
            events.push(EventKind::DrawOffered { by });
        }
    }
    match snapshot.end {
        Some(ProviderEnd::Decided(outcome)) => events.push(EventKind::GameEnded { outcome }),
        Some(ProviderEnd::Aborted) => events.push(EventKind::GameAborted),
        None => {}
    }
    Ok(events)
}
