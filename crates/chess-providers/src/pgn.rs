//! Leitura mínima de PGN para filtrar Partidas **localmente** (Chess.com não filtra no
//! servidor): separa as Partidas de um arquivo e lê as tags. Não interpreta Lances; quem
//! interpreta é o `chess-pgn`, na importação.

use crate::dates::Day;
use crate::games::GamesFilter;
use chess_core::Color;
use chess_store::Pace;

/// Separa um texto PGN em Partidas: cada uma começa numa linha `[Event `.
pub(crate) fn split_games(text: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("[Event ") {
            starts.push(offset);
        }
        offset += line.len();
    }
    let mut games = Vec::with_capacity(starts.len());
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        games.push(&text[start..end]);
    }
    games
}

/// Valor da tag `name` (`[White "Fulano"]`), lendo só o cabeçalho da Partida.
pub(crate) fn tag<'a>(game: &'a str, name: &str) -> Option<&'a str> {
    for line in game.lines() {
        let line = line.trim_end();
        if !line.starts_with('[') {
            if line.is_empty() {
                continue;
            }
            break;
        }
        let Some(rest) = line.strip_prefix('[').and_then(|l| l.strip_prefix(name)) else {
            continue;
        };
        if let Some(value) = rest.strip_prefix(" \"") {
            return value.strip_suffix("\"]");
        }
    }
    None
}

/// Resultado do filtro local de uma Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Keep,
    /// Fora do filtro (período, ritmo ou cor).
    Filtered,
    /// Variante que o app recusa.
    Variant,
}

/// Aplica variante, período, ritmo e cor (do ponto de vista de `user`).
pub(crate) fn judge(game: &str, user: &str, filter: &GamesFilter) -> Verdict {
    if let Some(variant) = tag(game, "Variant") {
        let key = variant.to_ascii_lowercase();
        if !matches!(key.as_str(), "standard" | "chess" | "from position") {
            return Verdict::Variant;
        }
    }
    if filter.since.is_some() || filter.until.is_some() {
        let date = tag(game, "UTCDate")
            .or_else(|| tag(game, "Date"))
            .and_then(Day::parse_pgn);
        match date {
            Some(day) => {
                if filter.since.is_some_and(|s| day < s) || filter.until.is_some_and(|u| day > u) {
                    return Verdict::Filtered;
                }
            }
            // Sem data legível não dá para provar que está no período.
            None => return Verdict::Filtered,
        }
    }
    if !filter.paces.is_empty() {
        let pace = Pace::derive(tag(game, "TimeControl"), tag(game, "Event"));
        if !pace.is_some_and(|p| filter.paces.contains(&p)) {
            return Verdict::Filtered;
        }
    }
    if let Some(color) = filter.color {
        let wanted = match color {
            Color::White => tag(game, "White"),
            Color::Black => tag(game, "Black"),
        };
        if !wanted.is_some_and(|name| name.eq_ignore_ascii_case(user)) {
            return Verdict::Filtered;
        }
    }
    Verdict::Keep
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "[Event \"A\"]\n[White \"ana\"]\n[Black \"beto\"]\n[Date \"2026.11.07\"]\n[TimeControl \"300+3\"]\n\n1. e4 e5 1-0\n\n[Event \"B\"]\n[White \"beto\"]\n[Black \"ana\"]\n[Date \"2026.10.01\"]\n[TimeControl \"1/259200\"]\n[Variant \"Chess960\"]\n\n1. e4 e5 0-1\n";

    #[test]
    fn splits_games_at_event_lines() {
        let games = split_games(TWO);
        assert_eq!(games.len(), 2);
        assert!(games[0].starts_with("[Event \"A\"]"));
        assert!(games[1].starts_with("[Event \"B\"]"));
        assert_eq!(tag(games[0], "White"), Some("ana"));
        assert_eq!(tag(games[1], "Variant"), Some("Chess960"));
        assert_eq!(tag(games[0], "Variant"), None);
    }

    #[test]
    fn filters_by_variant_period_pace_and_color() {
        let games = split_games(TWO);
        let all = GamesFilter {
            max: None,
            ..GamesFilter::default()
        };
        assert_eq!(judge(games[0], "ana", &all), Verdict::Keep);
        assert_eq!(judge(games[1], "ana", &all), Verdict::Variant);

        let november = GamesFilter {
            since: Day::new(2026, 11, 1).ok(),
            ..all.clone()
        };
        assert_eq!(judge(games[0], "ana", &november), Verdict::Keep);

        let before = GamesFilter {
            until: Day::new(2026, 10, 31).ok(),
            ..all.clone()
        };
        assert_eq!(judge(games[0], "ana", &before), Verdict::Filtered);

        let rapid = GamesFilter {
            paces: vec![Pace::Rapid],
            ..all.clone()
        };
        assert_eq!(judge(games[0], "ana", &rapid), Verdict::Filtered);

        let black = GamesFilter {
            color: Some(Color::Black),
            ..all
        };
        assert_eq!(judge(games[0], "ANA", &black), Verdict::Filtered);
        assert_eq!(judge(games[0], "beto", &black), Verdict::Keep);
    }
}
