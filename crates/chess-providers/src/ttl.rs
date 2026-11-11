//! Validade do cache por tipo de consulta (a camada de rede guarda a resposta `200` por
//! este tempo e revalida com `ETag` quando o Provedor oferece). O critério: quanto o dado
//! muda e quanto custa errar.

use std::time::Duration;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// Explorador de mestres: o corpus cresce devagar (dias).
pub const MASTERS: Duration = Duration::from_secs(7 * DAY);
/// Explorador do Lichess: milhões de Partidas novas por dia, mas estatística de aberturas
/// pouco se move numa tarde (horas).
pub const LICHESS_EXPLORER: Duration = Duration::from_secs(6 * HOUR);
/// Avaliação em cache da nuvem do Lichess: longa; o Provedor devolve a mais funda que tem.
pub const CLOUD_EVAL: Duration = Duration::from_secs(7 * DAY);
/// "Sem avaliação" (404) da nuvem: curta, porque outro usuário pode avaliar a Posição logo.
pub const NO_EVAL: Duration = Duration::from_secs(10 * MINUTE);
/// Finais exatos: determinísticos (até sete peças, com o relógio de 50 Lances no FEN).
pub const TABLEBASE: Duration = Duration::from_secs(30 * DAY);
/// Perfil público do Lichess: curto (ratings e "visto por último" mudam).
pub const LICHESS_PROFILE: Duration = Duration::from_secs(10 * MINUTE);
/// Rating FIDE reproduzido pelo Lichess: a FIDE atualiza uma vez por mês.
pub const FIDE: Duration = Duration::from_secs(DAY);
/// Perfil público do Chess.com: curto.
pub const CHESSCOM_PROFILE: Duration = Duration::from_secs(10 * MINUTE);
/// Lista de meses com Partidas do Chess.com: o mês corrente entra a cada virada.
pub const CHESSCOM_ARCHIVES: Duration = Duration::from_secs(HOUR);
/// PGN de um mês já encerrado do Chess.com: não muda (salvo Partidas banidas depois).
pub const CHESSCOM_PAST_MONTH: Duration = Duration::from_secs(7 * DAY);
/// PGN do mês corrente do Chess.com: ganha Partidas o dia todo.
pub const CHESSCOM_CURRENT_MONTH: Duration = Duration::from_secs(10 * MINUTE);
