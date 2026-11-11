//! Eventos: a fonte de verdade da Partida (ADR-0002). Serializados com payload
//! versionado para o log do B5.
//!
//! Formato JSON: `{"version":1,"at":{...},"kind":{"type":"move_completed",...}}`.
//! Evolução: novas variantes entram em `EventKind` (`non_exhaustive`) sem mudar
//! as existentes; uma mudança incompatível de campo sobe [`PAYLOAD_VERSION`].
//! As variantes do B6 (relógio, Simulação FIDE, Reclamações) e as da Partida online
//! (#230: relógio do Provedor, proposta retirada, Partida abortada) são aditivas e
//! continuam na versão 1 do envelope: logs anteriores seguem legíveis
//! (docs/decisoes/2026-11-11-partida-online.md).

use crate::outcome::Outcome;
use crate::setup::InfractionStage;
use crate::time::Timestamp;
use crate::wire;
use chess_core::{Color, Move, Piece, Square};
use serde::{Deserialize, Serialize};

/// Versão atual do payload dos Eventos.
pub const PAYLOAD_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub version: u16,
    pub at: Timestamp,
    pub kind: EventKind,
}

impl Event {
    pub fn new(at: Timestamp, kind: EventKind) -> Self {
        Self {
            version: PAYLOAD_VERSION,
            at,
            kind,
        }
    }
}

/// Por que um Lance foi tratado como ilegal (para a explicação conforme as
/// preferências, AC-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum IllegalReason {
    EmptyOrigin,
    OpponentPiece,
    SameSquare,
    /// A peça não faz esse Lance ou ele deixaria o rei em xeque; `in_check` diz se
    /// o lado da vez estava em xeque.
    NotLegal {
        in_check: bool,
    },
}

/// Causa de uma Infração (art. 7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum InfractionCause {
    /// Lance ilegal concluído (art. 7.5.1): a peça foi solta numa casa ilegal.
    IllegalMove {
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
        reason: IllegalReason,
    },
    /// Relógio acionado sem Lance feito (art. 7.5.3).
    ClockPressedWithoutMove,
    /// Peão na última fileira sem a peça de promoção (art. 7.5.2).
    PromotionNotCompleted {
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
    },
}

/// Tempo somado ao relógio de `to` (arts. 7.5.5 e 9.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimePenalty {
    #[serde(with = "wire::color")]
    pub to: Color,
    pub ms: u64,
}

/// Fundamento de uma Reclamação de empate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DrawBasis {
    /// Art. 9.2.
    ThreefoldRepetition,
    /// Art. 9.3.
    FiftyMoveRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    /// Toque intencional (art. 4.3): cria Obrigação de peça tocada. `implicit` é
    /// verdadeiro quando o toque foi deduzido de um `Release` sem `Touch` prévio.
    PieceTouched {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        square: Square,
        /// Cor da peça tocada (pode ser a do adversário).
        #[serde(with = "wire::color")]
        owner: Color,
        #[serde(with = "wire::piece")]
        piece: Piece,
        implicit: bool,
    },
    /// Ajuste, j'adoube (art. 4.2): sem obrigação.
    PieceAdjusted {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        square: Square,
    },
    /// Lance legal feito (peça solta, art. 4.7), ainda não concluído pelo relógio.
    MoveMade {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
        san: String,
    },
    /// Peça solta numa casa ilegal: permanece lá (Posição visual provisória, DEC-32)
    /// até o relógio (Infração) ou a retratação.
    IllegalMoveReleased {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
        reason: IllegalReason,
    },
    /// A peça solta ilegalmente voltou à origem; a Obrigação de peça tocada fica.
    ReleaseRetracted {
        #[serde(with = "wire::color")]
        side: Color,
    },
    /// Infração registrada (art. 7.5). `count` é a ordem da Infração deste lado;
    /// `penalty` é o tempo dado ao adversário (a 2ª Infração encerra a Partida).
    InfractionRecorded {
        #[serde(with = "wire::color")]
        side: Color,
        cause: InfractionCause,
        count: u8,
        stage: InfractionStage,
        penalty: Option<TimePenalty>,
    },
    /// Pausa (DEC-42): os relógios param.
    Paused,
    Resumed,
    /// Bandeira caiu (art. 6.9) no instante `flag_at_ms` (mono). O resultado vem no
    /// `GameEnded` seguinte.
    FlagFell {
        #[serde(with = "wire::color")]
        side: Color,
        flag_at_ms: u64,
    },
    /// Reclamação de empate correta (arts. 9.2/9.3); `intended` é o Lance anunciado
    /// (arts. 9.2.1/9.3.1), que se completa com a Reclamação.
    DrawClaimAccepted {
        #[serde(with = "wire::color")]
        by: Color,
        basis: DrawBasis,
        #[serde(with = "wire::uci_opt")]
        intended: Option<Move>,
    },
    /// Reclamação incorreta (art. 9.5.3): tempo ao adversário; a Partida continua e,
    /// se havia Lance anunciado, ele passa a ser obrigatório.
    DrawClaimRejected {
        #[serde(with = "wire::color")]
        by: Color,
        #[serde(with = "wire::uci_opt")]
        intended: Option<Move>,
        penalty: TimePenalty,
    },
    /// Proposta da Engine recusada pelo núcleo de regras (ilegal ou malformada).
    EngineMoveRejected {
        #[serde(with = "wire::color")]
        side: Color,
        uci: String,
    },
    /// Lance legal concluído (no Assistido, ao soltar). Entra no histórico legal.
    MoveCompleted {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
        san: String,
    },
    /// Lance ilegal recusado: informativo, sem penalidade, fora do histórico legal (DB-04).
    IllegalMoveRejected {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
        reason: IllegalReason,
    },
    /// Peão chegou à última fileira; falta escolher a peça.
    PromotionPending {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
    },
    PromotionCancelled {
        #[serde(with = "wire::color")]
        side: Color,
    },
    DrawOffered {
        #[serde(with = "wire::color")]
        by: Color,
    },
    DrawDeclined {
        #[serde(with = "wire::color")]
        by: Color,
    },
    Resigned {
        #[serde(with = "wire::color")]
        side: Color,
    },
    /// Último Lance desfeito; marca a Partida "com desfazer" (DEC-39).
    MoveUndone {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
    },
    MoveRedone {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
        san: String,
    },
    /// Fim da Partida (resultado e motivo).
    GameEnded {
        outcome: Outcome,
    },
    /// Partida online: relógios informados pelo Provedor no instante do Evento (a chegada
    /// do retrato); `running` é quem corre a partir dali. Substitui o relógio derivado.
    ClockReported {
        white_ms: u64,
        black_ms: u64,
        #[serde(with = "wire::color_opt")]
        running: Option<Color>,
    },
    /// Partida online: a proposta de empate de `by` deixou de valer no Provedor (recusada
    /// ou retirada; o Provedor não distingue).
    DrawOfferWithdrawn {
        #[serde(with = "wire::color")]
        by: Color,
    },
    /// Partida online abortada (ou encerrada pelo Provedor sem resultado). Termina a
    /// Partida sem resultado.
    GameAborted,
}
