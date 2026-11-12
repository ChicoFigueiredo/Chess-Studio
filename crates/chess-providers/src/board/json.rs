//! Formato das mensagens da Board API do Lichess (documentação oficial, lichess-org/api:
//! `GameFullEvent`, `GameStateEvent`, `GameEventInfo`, `ChallengeJson`; conferida em
//! 11/11/2026) e a tradução para o domínio. Campos fora do documentado são ignorados.

use super::{Challenge, ChallengePlayer, GameInfo, OnlineTime, OpponentInfo};
use crate::family::ProviderProfile;
use chess_core::{CanonicalPosition, Color};
use chess_game::{
    EndReason, GameResult, GameSetup, OnlineGame, OnlinePlayer, OnlineSetupError, Opponent,
    Outcome, ProviderClock, ProviderEnd, ProviderSnapshot, TimeControl, Variant,
};
use serde::Deserialize;

fn color(text: &str) -> Option<Color> {
    match text {
        "white" => Some(Color::White),
        "black" => Some(Color::Black),
        _ => None,
    }
}

fn millis(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VariantJson {
    pub key: String,
}

/// Xadrez padrão para o app: as variantes que o catálogo aceita para o Provedor (no Lichess,
/// `standard` e `fromPosition`: regras padrão, outra Posição).
pub(crate) fn variant(profile: &ProviderProfile, key: &str) -> Variant {
    if profile.accepts_variant(key) {
        Variant::Standard
    } else {
        Variant::Other(key.to_owned())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ClockJson {
    pub initial: i64,
    pub increment: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerJson {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub rating: Option<u32>,
    #[serde(default)]
    pub ai_level: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct StateJson {
    #[serde(default)]
    pub moves: String,
    #[serde(default)]
    pub wtime: i64,
    #[serde(default)]
    pub btime: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub winner: Option<String>,
    #[serde(default)]
    pub wdraw: bool,
    #[serde(default)]
    pub bdraw: bool,
    #[serde(default)]
    pub wtakeback: bool,
    #[serde(default)]
    pub btakeback: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameFull {
    pub id: String,
    pub variant: VariantJson,
    #[serde(default)]
    pub clock: Option<ClockJson>,
    #[serde(default)]
    pub rated: bool,
    #[serde(default)]
    pub white: PlayerJson,
    #[serde(default)]
    pub black: PlayerJson,
    #[serde(default)]
    pub initial_fen: Option<String>,
    pub state: StateJson,
    #[serde(default)]
    pub days_per_turn: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpponentGoneJson {
    pub gone: bool,
    #[serde(default)]
    pub claim_win_in_seconds: Option<u64>,
}

fn online_player(profile: &ProviderProfile, player: &PlayerJson) -> OnlinePlayer {
    let name = match (&player.name, player.ai_level) {
        (Some(name), _) => name.clone(),
        (None, Some(level)) => format!("{} (nível {level})", profile.provider_engine_name),
        (None, None) => "?".to_owned(),
    };
    OnlinePlayer {
        name,
        rating: player.rating,
    }
}

/// O setup da Partida online a partir do `gameFull`. Variante não padrão é recusada aqui,
/// antes de criar a Partida.
pub(crate) fn setup(
    profile: &ProviderProfile,
    engine_game: bool,
    full: &GameFull,
    local: Color,
) -> Result<GameSetup, OnlineSetupError> {
    let opponent = match local {
        Color::White => &full.black,
        Color::Black => &full.white,
    };
    let game = OnlineGame {
        provider: profile.game_key(engine_game),
        remote_id: full.id.clone(),
        local_color: local,
        opponent: match opponent.ai_level {
            Some(level) => Opponent::ProviderEngine { level },
            None => Opponent::Human,
        },
        white: online_player(profile, &full.white),
        black: online_player(profile, &full.black),
        rated: full.rated,
        days_per_move: full.days_per_turn,
    };
    let fen = full
        .initial_fen
        .as_deref()
        .filter(|f| !f.is_empty() && *f != "startpos");
    let setup = GameSetup::online(game, variant(profile, &full.variant.key), fen)?;
    Ok(match &full.clock {
        Some(clock) => setup.with_time_control(TimeControl {
            base_ms: millis(clock.initial),
            increment_ms: millis(clock.increment),
        }),
        None => setup,
    })
}

/// Como o Lichess encerrou a Partida, pelo `status` e o `winner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
    Playing,
    End(ProviderEnd),
    /// `draw`: o Lichess não diz o motivo; vem da Posição (ver [`draw_reason`]).
    Draw,
}

pub(crate) fn ending(state: &StateJson) -> Ending {
    let winner = state.winner.as_deref().and_then(color);
    let decided = |result, reason| Ending::End(ProviderEnd::Decided(Outcome { result, reason }));
    let won = |reason| match winner {
        Some(Color::White) => Some(decided(GameResult::WhiteWins, reason)),
        Some(Color::Black) => Some(decided(GameResult::BlackWins, reason)),
        None => None,
    };
    match state.status.as_str() {
        "created" | "started" | "" => Ending::Playing,
        "aborted" => Ending::End(ProviderEnd::Aborted),
        "mate" => won(EndReason::Checkmate).unwrap_or(Ending::End(ProviderEnd::Aborted)),
        "resign" => won(EndReason::Resignation).unwrap_or(Ending::End(ProviderEnd::Aborted)),
        "stalemate" => decided(GameResult::Draw, EndReason::Stalemate),
        "outoftime" => {
            won(EndReason::Timeout).unwrap_or(decided(GameResult::Draw, EndReason::TimeoutDraw))
        }
        // `timeout` no Lichess: o adversário saiu e quem ficou reclamou.
        "timeout" => won(EndReason::Disconnection)
            .unwrap_or(decided(GameResult::Draw, EndReason::Disconnection)),
        "insufficientMaterialClaim" => decided(GameResult::Draw, EndReason::InsufficientMaterial),
        "draw" => Ending::Draw,
        // cheat, noStart, unknownFinish, variantEnd e o que vier: encerrada pelo servidor.
        _ => won(EndReason::EndedByProvider).unwrap_or(Ending::End(ProviderEnd::Aborted)),
    }
}

/// Motivo de um `draw` do Lichess, pela Posição final e o histórico: material insuficiente,
/// repetição tripla, 50 lances; senão, acordo.
pub(crate) fn draw_reason(start: &CanonicalPosition, history: &[chess_core::Move]) -> EndReason {
    let mut position = start.clone();
    let mut keys = vec![position.repetition_key()];
    for mv in history {
        match position.play(*mv) {
            Ok(next) => position = next,
            Err(_) => return EndReason::DrawAgreement,
        }
        keys.push(position.repetition_key());
    }
    let last = position.repetition_key();
    if position.is_insufficient_material() {
        EndReason::InsufficientMaterial
    } else if keys.iter().filter(|k| **k == last).count() >= 3 {
        EndReason::ThreefoldRepetition
    } else if position.can_claim_fifty_move() {
        EndReason::FiftyMoveRule
    } else {
        EndReason::DrawAgreement
    }
}

/// O retrato do domínio a partir do `gameState`. O relógio só vale com controle de tempo
/// (na correspondência o Lichess manda números sem sentido de relógio de xadrez) e, como no
/// Lichess, só corre depois do primeiro Lance de cada lado.
pub(crate) fn snapshot(
    state: &StateJson,
    has_clock: bool,
    end: Option<ProviderEnd>,
) -> ProviderSnapshot {
    let moves: Vec<String> = state.moves.split_whitespace().map(str::to_owned).collect();
    let clock = has_clock.then(|| ProviderClock {
        white_ms: millis(state.wtime),
        black_ms: millis(state.btime),
        running: moves.len() >= 2 && state.status == "started",
    });
    let draw_offer = if state.wdraw {
        Some(Color::White)
    } else if state.bdraw {
        Some(Color::Black)
    } else {
        None
    };
    ProviderSnapshot {
        moves,
        clock,
        draw_offer,
        end,
    }
}

/// Quem pede a volta de Lance, se alguém pede.
pub(crate) fn takeback_by(state: &StateJson) -> Option<Color> {
    if state.wtakeback {
        Some(Color::White)
    } else if state.btakeback {
        Some(Color::Black)
    } else {
        None
    }
}

// ---- GameEventInfo e desafios ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StatusJson {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct EventOpponentJson {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub rating: Option<u32>,
    #[serde(default)]
    pub ai: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameInfoJson {
    pub game_id: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub fen: Option<String>,
    #[serde(default)]
    pub last_move: Option<String>,
    #[serde(default)]
    pub status: Option<StatusJson>,
    #[serde(default)]
    pub variant: Option<VariantJson>,
    #[serde(default)]
    pub speed: Option<String>,
    #[serde(default)]
    pub rated: bool,
    #[serde(default)]
    pub opponent: EventOpponentJson,
    #[serde(default)]
    pub is_my_turn: bool,
    #[serde(default)]
    pub seconds_left: Option<u64>,
    #[serde(default)]
    pub winner: Option<String>,
}

impl TryFrom<GameInfoJson> for GameInfo {
    type Error = String;

    fn try_from(j: GameInfoJson) -> Result<Self, String> {
        let local = j
            .color
            .as_deref()
            .and_then(color)
            .ok_or_else(|| "Partida sem a cor do jogador".to_owned())?;
        // O id entra no caminho de várias URLs (fluxo, ações, Lances): só o alfabeto e o
        // comprimento de um id do Lichess, nunca `/`, `..`, `?`, `#` ou `%` (SEC-06).
        if !super::is_id(&j.game_id) {
            return Err("Partida com identificador inválido".to_owned());
        }
        Ok(Self {
            game_id: j.game_id,
            color: local,
            opponent: OpponentInfo {
                name: j.opponent.username.unwrap_or_else(|| "?".to_owned()),
                rating: j.opponent.rating,
                ai_level: j.opponent.ai,
            },
            variant: j.variant.map_or_else(|| "standard".to_owned(), |v| v.key),
            speed: j.speed.unwrap_or_default(),
            rated: j.rated,
            is_my_turn: j.is_my_turn,
            status: j.status.and_then(|s| s.name),
            winner: j.winner.as_deref().and_then(color),
            fen: j.fen,
            last_move: j.last_move.filter(|m| !m.is_empty()),
            seconds_left: j.seconds_left,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChallengePlayerJson {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub rating: Option<u32>,
}

impl From<ChallengePlayerJson> for ChallengePlayer {
    fn from(j: ChallengePlayerJson) -> Self {
        Self {
            name: j.name.unwrap_or_else(|| j.id.clone()),
            id: j.id,
            title: j.title,
            rating: j.rating,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeControlJson {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub increment: Option<u32>,
    #[serde(default)]
    pub days_per_turn: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChallengeJson {
    pub id: String,
    pub challenger: ChallengePlayerJson,
    #[serde(default)]
    pub dest_user: Option<ChallengePlayerJson>,
    pub variant: VariantJson,
    #[serde(default)]
    pub rated: bool,
    #[serde(default)]
    pub speed: Option<String>,
    pub time_control: TimeControlJson,
    #[serde(default)]
    pub color: Option<String>,
}

impl TryFrom<ChallengeJson> for Challenge {
    type Error = String;

    fn try_from(j: ChallengeJson) -> Result<Self, String> {
        let time = match j.time_control.kind.as_str() {
            "clock" => OnlineTime::RealTime {
                base_s: j.time_control.limit.unwrap_or(0),
                increment_s: j.time_control.increment.unwrap_or(0),
            },
            "correspondence" => OnlineTime::Correspondence {
                days: j.time_control.days_per_turn.unwrap_or(0),
            },
            "unlimited" => OnlineTime::Unlimited,
            other => return Err(format!("ritmo desconhecido: {other}")),
        };
        Ok(Self {
            id: j.id,
            challenger: j.challenger.into(),
            dest: j.dest_user.map(Into::into),
            variant: j.variant.key,
            rated: j.rated,
            speed: j.speed.unwrap_or_default(),
            time,
            color: j.color.unwrap_or_else(|| "random".to_owned()),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IdJson {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclinedJson {
    pub id: String,
    #[serde(default)]
    pub decline_reason_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayingJson {
    #[serde(default)]
    pub now_playing: Vec<GameInfo>,
}
