//! Exportação PGN (DEC-44). O PGN é interoperabilidade, não fonte de verdade
//! (ADR-0002): é gerado do log de Eventos e só contém Lances legais (AC-08, DB-04).
//! Infrações, penalidades e Reclamações entram como comentários `{...}`, opcionais.
//! Implementação própria, sem dependências (ADR-0004).

use crate::event::{DrawBasis, EventKind, InfractionCause};
use crate::game::Game;
use crate::outcome::EndReason;
use crate::setup::PolicyKind;
use chess_core::{CanonicalPosition, Color};

/// Largura máxima das linhas do movetext.
const MAX_COLUMNS: usize = 80;

/// Metadados e opções da exportação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgnOptions {
    pub event: String,
    pub site: String,
    /// `AAAA.MM.DD`; `None` exporta `????.??.??`.
    pub date: Option<String>,
    pub round: String,
    pub white: String,
    pub black: String,
    /// Elo da Engine adversária, se houver (tag própria `EngineElo`).
    pub engine_elo: Option<u32>,
    /// Exporta Infrações, penalidades e Reclamações como comentários (DEC-44).
    pub include_comments: bool,
}

impl Default for PgnOptions {
    fn default() -> Self {
        Self {
            event: "?".into(),
            site: "?".into(),
            date: None,
            round: "-".into(),
            white: "?".into(),
            black: "?".into(),
            engine_elo: None,
            include_comments: true,
        }
    }
}

impl Game {
    /// PGN da Partida: Seven Tag Roster, tags extras e movetext em SAN. Não altera a Partida.
    pub fn to_pgn(&self, options: &PgnOptions) -> String {
        export(self, options)
    }
}

/// Data PGN (`AAAA.MM.DD`, UTC) a partir de milissegundos Unix.
pub fn pgn_date_from_unix_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // Dias desde 1970-01-01 para data civil (algoritmo de Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}.{month:02}.{day:02}")
}

/// Lances do histórico final, com os comentários que ocorreram depois de cada um.
/// `comments[k]` vem depois de `k` meios-lances (`comments[0]`: antes do primeiro).
struct Line {
    sans: Vec<String>,
    comments: Vec<Vec<String>>,
}

fn collect(game: &Game) -> Line {
    let mut line = Line {
        sans: Vec::new(),
        comments: vec![Vec::new()],
    };
    for event in game.events() {
        match &event.kind {
            EventKind::MoveCompleted { san, .. } | EventKind::MoveRedone { san, .. } => {
                line.sans.push(san.clone());
                line.comments.push(Vec::new());
            }
            EventKind::MoveUndone { .. } => {
                line.sans.pop();
                // Fatos já ocorridos não somem: passam para o ponto anterior.
                if line.comments.len() > 1 {
                    let orphans = line.comments.pop().unwrap_or_default();
                    if let Some(prev) = line.comments.last_mut() {
                        prev.extend(orphans);
                    }
                }
            }
            kind => {
                if let Some(text) = comment_for(kind) {
                    if let Some(slot) = line.comments.last_mut() {
                        slot.push(text);
                    }
                }
            }
        }
    }
    line
}

fn side_name(side: Color) -> &'static str {
    match side {
        Color::White => "brancas",
        Color::Black => "pretas",
    }
}

fn minutes_text(ms: u64) -> String {
    if ms % 60_000 == 0 {
        format!("{} min", ms / 60_000)
    } else {
        format!("{} s", ms / 1_000)
    }
}

fn comment_for(kind: &EventKind) -> Option<String> {
    match kind {
        EventKind::InfractionRecorded {
            side,
            cause,
            count,
            penalty,
            ..
        } => {
            let what = match cause {
                InfractionCause::IllegalMove { from, to, .. } => {
                    format!("Lance ilegal concluído ({from}{to})")
                }
                InfractionCause::ClockPressedWithoutMove => {
                    "relógio acionado sem Lance feito".to_owned()
                }
                InfractionCause::PromotionNotCompleted { from, to } => {
                    format!("promoção não concluída ({from}{to})")
                }
            };
            let mut text = format!("Infração {count} das {}: {what}.", side_name(*side));
            match penalty {
                Some(p) => text.push_str(&format!(
                    " Penalidade: +{} para as {}.",
                    minutes_text(p.ms),
                    side_name(p.to)
                )),
                None => text.push_str(" Perde a Partida."),
            }
            Some(text)
        }
        EventKind::DrawClaimAccepted { by, basis, .. } => Some(format!(
            "Reclamação de empate das {} aceita ({}).",
            side_name(*by),
            basis_name(*basis)
        )),
        EventKind::DrawClaimRejected { by, penalty, .. } => Some(format!(
            "Reclamação de empate das {} incorreta. Penalidade: +{} para as {}.",
            side_name(*by),
            minutes_text(penalty.ms),
            side_name(penalty.to)
        )),
        EventKind::FlagFell { side, .. } => {
            Some(format!("Bandeira das {} caiu.", side_name(*side)))
        }
        _ => None,
    }
}

fn basis_name(basis: DrawBasis) -> &'static str {
    match basis {
        DrawBasis::ThreefoldRepetition => "tripla repetição, art. 9.2",
        DrawBasis::FiftyMoveRule => "regra dos 50 lances, art. 9.3",
    }
}

fn termination(reason: Option<EndReason>) -> &'static str {
    match reason {
        None => "unterminated",
        Some(EndReason::Timeout | EndReason::TimeoutDraw) => "time forfeit",
        Some(EndReason::SecondIllegalMove | EndReason::SecondIllegalMoveDraw) => "rules infraction",
        Some(_) => "normal",
    }
}

fn time_control(game: &Game) -> String {
    match game.setup().time_control() {
        None => "-".to_owned(),
        Some(tc) if tc.increment_ms == 0 => format!("{}", tc.base_ms / 1_000),
        Some(tc) => format!("{}+{}", tc.base_ms / 1_000, tc.increment_ms / 1_000),
    }
}

fn arbitration(game: &Game) -> String {
    match game.setup().policy() {
        PolicyKind::FideSimulation => format!(
            "FIDE Simulation ({})",
            game.setup().rules_version().unwrap_or("FIDE Laws of Chess")
        ),
        _ => "Assisted".to_owned(),
    }
}

/// Valor de tag: sem quebras de linha, `\` e `"` escapados; vazio vira `?`.
fn tag_value(raw: &str) -> String {
    let flat: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.is_empty() {
        return "?".to_owned();
    }
    flat.replace('\\', "\\\\").replace('"', "\\\"")
}

fn tag(out: &mut String, name: &str, value: &str) {
    out.push_str(&format!("[{name} \"{}\"]\n", tag_value(value)));
}

/// Comentário `{...}`: o PGN não tem escape para `}`, então ele vira `)`.
fn comment_text(raw: &str) -> String {
    let flat: String = raw
        .chars()
        .map(|c| match c {
            '}' => ')',
            '{' => '(',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    format!(
        "{{{}}}",
        flat.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn export(game: &Game, options: &PgnOptions) -> String {
    let snapshot = game.snapshot();
    let start = game.setup().start();
    let result = snapshot.outcome.map_or("*", |o| o.result.pgn_token());
    let reason = snapshot.outcome.map(|o| o.reason);

    let mut out = String::new();
    tag(&mut out, "Event", &options.event);
    tag(&mut out, "Site", &options.site);
    tag(
        &mut out,
        "Date",
        options.date.as_deref().unwrap_or("????.??.??"),
    );
    tag(&mut out, "Round", &options.round);
    tag(&mut out, "White", &options.white);
    tag(&mut out, "Black", &options.black);
    tag(&mut out, "Result", result);
    if start.to_fen() != CanonicalPosition::start().to_fen() {
        tag(&mut out, "SetUp", "1");
        tag(&mut out, "FEN", &start.to_fen());
    }
    tag(&mut out, "TimeControl", &time_control(game));
    tag(&mut out, "Termination", termination(reason));
    tag(&mut out, "Arbitration", &arbitration(game));
    if let Some(elo) = options.engine_elo {
        tag(&mut out, "EngineElo", &elo.to_string());
    }
    out.push('\n');

    let line = collect(game);
    let mut tokens: Vec<String> = Vec::new();
    let mut side = start.side_to_move();
    let mut number = u32::from(start.fullmove_number());
    let mut after_comment = false;
    let push_comments = |tokens: &mut Vec<String>, slot: &[String], after: &mut bool| {
        if options.include_comments {
            for text in slot {
                tokens.extend(comment_text(text).split(' ').map(str::to_owned));
                *after = true;
            }
        }
    };
    push_comments(&mut tokens, &line.comments[0], &mut after_comment);
    for (i, san) in line.sans.iter().enumerate() {
        if side == Color::White {
            tokens.push(format!("{number}."));
        } else if i == 0 || after_comment {
            tokens.push(format!("{number}..."));
        }
        after_comment = false;
        tokens.push(san.clone());
        push_comments(&mut tokens, &line.comments[i + 1], &mut after_comment);
        if side == Color::Black {
            number += 1;
        }
        side = side.opposite();
    }
    tokens.push(result.to_owned());

    let mut current = String::new();
    for token in &tokens {
        if !current.is_empty() && current.chars().count() + 1 + token.chars().count() > MAX_COLUMNS
        {
            out.push_str(&current);
            out.push('\n');
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(token);
    }
    out.push_str(&current);
    out.push('\n');
    out
}
