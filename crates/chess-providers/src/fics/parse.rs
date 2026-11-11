//! As mensagens do FICS que o adaptador lê (#237), separadas do transporte: o `chess-net`
//! entrega linhas limpas (sem telnet, sem controle), este módulo as classifica. Nada aqui
//! entra em pânico nem guarda estado; o que não se reconhece é [`Message::Other`].
//!
//! Formatos: o `style 12` segue a ajuda oficial do FICS (`help style12`: 31 campos separados
//! por espaço, novos campos só no fim, lidos da esquerda para a direita). As demais mensagens
//! (prompt de login, início e fim de Partida, desafio, proposta de empate) seguem o texto que
//! os clientes de código aberto do FICS reconhecem; a ajuda do FICS não as transcreve.
//! Detalhes e riscos em `docs/decisoes/2026-11-11-fics.md`.

use chess_core::Color;
use chess_game::{EndReason, GameResult, Outcome, ProviderEnd};

/// O prompt de comando padrão do FICS.
const PROMPT: &str = "fics%";
/// Nome de jogador do FICS: letras (visitantes têm `Guest` + letras); folga para o resto.
const MAX_NAME: usize = 32;
/// Número de lance acima do qual o `style 12` é recusado (nenhuma Partida chega perto).
const MAX_MOVE_NUMBER: u32 = 10_000;
/// Tempo restante absurdo (segundos): recusado.
const MAX_SECONDS: i64 = 1_000_000_000;

/// Relação do jogador local com a Partida do `style 12` (campo 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relation {
    /// Posição isolada (`-3`).
    Isolated,
    /// Observando uma Partida em exame (`-2`).
    ObservingExamined,
    /// Examinando (`2`).
    Examining,
    /// Jogando; vez do adversário (`-1`).
    OpponentToMove,
    /// Jogando; minha vez (`1`).
    MyMove,
    /// Observando uma Partida em andamento (`0`).
    Observing,
}

/// Uma linha `style 12`: o tabuleiro completo de uma Partida depois de cada Lance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Style12 {
    /// Colocação das peças em FEN (fileira 8 primeiro).
    pub placement: String,
    pub side: Color,
    /// Coluna (0 = a) do avanço duplo de peão do último Lance.
    pub double_push: Option<u8>,
    /// Brancas pequeno, brancas grande, pretas pequeno, pretas grande.
    pub castling: [bool; 4],
    /// Meio-lances desde o último Lance irreversível.
    pub halfmove_clock: u32,
    pub game: u32,
    pub white: String,
    pub black: String,
    pub relation: Relation,
    /// Tempo inicial (minutos, como no exemplo da ajuda) e incremento (segundos).
    pub initial_minutes: u32,
    pub increment_s: u32,
    /// Tempo restante em segundos (pode ser negativo: bandeira caída).
    pub white_s: i64,
    pub black_s: i64,
    /// Número do lance que vai ser feito (os dois lados começam em 1).
    pub move_number: u32,
    /// O relógio está correndo (campo que o FICS acrescenta depois dos 31; nem sempre há).
    pub ticking: Option<bool>,
}

impl Style12 {
    /// Meio-lances jogados desde a Posição inicial padrão.
    pub fn ply(&self) -> u64 {
        u64::from(self.move_number - 1) * 2 + u64::from(self.side == Color::Black)
    }

    /// Os quatro primeiros campos do FEN: colocação, vez, roque e en passant.
    pub fn key(&self) -> String {
        let side = match self.side {
            Color::White => "w",
            Color::Black => "b",
        };
        let mut castling: String = ['K', 'Q', 'k', 'q']
            .iter()
            .zip(self.castling)
            .filter(|(_, can)| *can)
            .map(|(c, _)| *c)
            .collect();
        if castling.is_empty() {
            castling.push('-');
        }
        let ep = match self.double_push {
            Some(file) => {
                let rank = if self.side == Color::White { '6' } else { '3' };
                format!("{}{rank}", char::from(b'a' + file))
            }
            None => "-".to_owned(),
        };
        format!("{} {side} {castling} {ep}", self.placement)
    }

    /// FEN completo (para a Posição em que a observação começa).
    pub fn fen(&self) -> String {
        format!(
            "{} {} {}",
            self.key(),
            self.halfmove_clock.min(9_999),
            self.move_number
        )
    }

    /// Relógios em milissegundos (negativo vira zero).
    pub fn clock_ms(&self) -> (u64, u64) {
        let ms = |s: i64| u64::try_from(s.max(0)).unwrap_or(0).saturating_mul(1000);
        (ms(self.white_s), ms(self.black_s))
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.relation, Relation::MyMove | Relation::OpponentToMove)
    }
}

/// Um desafio (`match`) recebido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicsChallenge {
    /// Quem desafia.
    pub challenger: String,
    pub rating: Option<u32>,
    /// Quem é desafiado (o jogador local).
    pub receiver: String,
    pub rated: bool,
    /// Tipo de Partida no FICS (`blitz`, `lightning`, `standard`, `untimed`, `wild/fr`…).
    pub kind: String,
    pub minutes: u32,
    pub increment_s: u32,
}

/// Tipo e nomes de uma Partida, como o FICS anuncia ao criar (ou ao observar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GameHeader {
    pub game: u32,
    pub white: String,
    pub black: String,
    pub rated: bool,
    pub kind: String,
}

/// Uma linha (ou prompt) do FICS, classificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Message {
    LoginPrompt,
    PasswordPrompt,
    /// `Press return to enter the server as "GuestXXXX":`
    GuestName(String),
    /// `**** Starting FICS session as GuestXXXX(U) ****`
    SessionStarted(String),
    Board(Box<Style12>),
    /// `{Game N (W vs. B) Creating unrated blitz match.}`
    GameCreated(GameHeader),
    /// `{Game N (W vs. B) motivo} resultado`
    GameEnded {
        game: u32,
        end: ProviderEnd,
    },
    /// `You are now observing game N.`
    ObservationStarted(u32),
    /// `Game N: W (r) B (r) unrated blitz 5 0`
    ObservedHeader(GameHeader),
    /// `Removing game N from observation list.`
    ObservationRemoved(u32),
    /// `Challenge: A (r) B (r) unrated blitz 5 0.`
    Challenge(FicsChallenge),
    /// `X offers you a draw.`
    DrawOffered {
        by: String,
    },
    /// Proposta de empate recusada ou retirada por `by` (`You` ou o nome de quem retira).
    /// Recados de outros jogadores (`X tells you: …`) têm `:` e nunca chegam aqui.
    DrawCancelled {
        by: String,
    },
    /// `X would like to abort the game; …`
    AbortRequested {
        by: String,
    },
    /// `Your seek has been posted with index N.`
    SeekPosted,
    /// `Illegal move (…).` ou `It is not your move.`
    MoveRefused,
    Other,
}

/// Tira os prompts `fics% ` do começo da linha (o FICS escreve a resposta logo depois do
/// prompt anterior, na mesma linha).
pub(crate) fn strip_prompt(mut line: &str) -> &str {
    loop {
        let trimmed = line.trim_start();
        match trimmed.strip_prefix(PROMPT) {
            Some(rest) => line = rest,
            None => return trimmed,
        }
    }
}

/// O que um prompt (texto parado sem fim de linha) pede.
pub(crate) fn parse_prompt(text: &str) -> Message {
    let text = strip_prompt(text).trim_end();
    if text == "login:" {
        Message::LoginPrompt
    } else if text == "password:" {
        Message::PasswordPrompt
    } else if let Some(name) = guest_name(text) {
        Message::GuestName(name)
    } else {
        Message::Other
    }
}

/// Classifica uma linha completa.
pub(crate) fn parse_line(line: &str) -> Message {
    let line = strip_prompt(line).trim_end();
    if line.starts_with("<12> ") {
        return parse_style12(line).map_or(Message::Other, |s| Message::Board(Box::new(s)));
    }
    if line == "login:" {
        return Message::LoginPrompt;
    }
    if line == "password:" {
        return Message::PasswordPrompt;
    }
    if let Some(name) = guest_name(line) {
        return Message::GuestName(name);
    }
    if let Some(rest) = line.strip_prefix("**** Starting FICS session as ") {
        let name = rest.trim_end_matches('*').trim();
        let name = name.split('(').next().unwrap_or_default();
        return valid_name(name).map_or(Message::Other, Message::SessionStarted);
    }
    if let Some(rest) = line.strip_prefix("{Game ") {
        return parse_game_line(rest).unwrap_or(Message::Other);
    }
    if let Some(rest) = line.strip_prefix("You are now observing game ") {
        return number(rest.trim_end_matches('.'))
            .map_or(Message::Other, |n| Message::ObservationStarted(n));
    }
    if let Some(rest) = line.strip_prefix("Removing game ") {
        return rest
            .strip_suffix(" from observation list.")
            .and_then(number)
            .map_or(Message::Other, Message::ObservationRemoved);
    }
    if let Some(rest) = line.strip_prefix("Game ")
        && let Some(header) = parse_observed_header(rest)
    {
        return Message::ObservedHeader(header);
    }
    if let Some(rest) = line.strip_prefix("Challenge: ") {
        return parse_challenge(rest).map_or(Message::Other, Message::Challenge);
    }
    if let Some(by) = line.strip_suffix(" offers you a draw.") {
        return valid_name(by).map_or(Message::Other, |by| Message::DrawOffered { by });
    }
    let lower = line.to_ascii_lowercase();
    if !line.contains(':')
        && lower.contains("draw")
        && (lower.contains("declin") || lower.contains("withdr"))
        && let Some(by) = line.split_whitespace().next().and_then(valid_name)
    {
        return Message::DrawCancelled { by };
    }
    if let Some((by, _)) = line.split_once(" would like to abort the game")
        && let Some(by) = valid_name(by)
    {
        return Message::AbortRequested { by };
    }
    if line.starts_with("Your seek has been posted with index ") {
        return Message::SeekPosted;
    }
    if line.starts_with("Illegal move (") || line == "It is not your move." {
        return Message::MoveRefused;
    }
    Message::Other
}

fn guest_name(text: &str) -> Option<String> {
    let rest = text.strip_prefix("Press return to enter the server as \"")?;
    let (name, _) = rest.split_once('"')?;
    valid_name(name)
}

fn valid_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    .then(|| name.to_owned())
}

fn number(text: &str) -> Option<u32> {
    let text = text.trim();
    (!text.is_empty() && text.len() <= 9 && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

fn flag(text: &str) -> Option<bool> {
    match text {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// Uma fileira do `style 12` (`rnbqkb-r`) em FEN (`rnbqkb1r`).
fn rank_fen(rank: &str) -> Option<String> {
    if rank.len() != 8 {
        return None;
    }
    let mut out = String::new();
    let mut empty = 0u8;
    for c in rank.chars() {
        if c == '-' {
            empty += 1;
            continue;
        }
        if !"PNBRQKpnbrqk".contains(c) {
            return None;
        }
        if empty > 0 {
            out.push(char::from(b'0' + empty));
            empty = 0;
        }
        out.push(c);
    }
    if empty > 0 {
        out.push(char::from(b'0' + empty));
    }
    Some(out)
}

pub(crate) fn parse_style12(line: &str) -> Option<Style12> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 31 || fields[0] != "<12>" {
        return None;
    }
    let ranks: Vec<String> = fields[1..9]
        .iter()
        .map(|r| rank_fen(r))
        .collect::<Option<_>>()?;
    let side = match fields[9] {
        "W" => Color::White,
        "B" => Color::Black,
        _ => return None,
    };
    let double_push = match fields[10] {
        "-1" => None,
        f => Some(u8::try_from(number(f)?).ok().filter(|f| *f < 8)?),
    };
    let castling = [
        flag(fields[11])?,
        flag(fields[12])?,
        flag(fields[13])?,
        flag(fields[14])?,
    ];
    let relation = match fields[19] {
        "-3" => Relation::Isolated,
        "-2" => Relation::ObservingExamined,
        "2" => Relation::Examining,
        "-1" => Relation::OpponentToMove,
        "1" => Relation::MyMove,
        "0" => Relation::Observing,
        _ => return None,
    };
    let seconds = |f: &str| f.parse::<i64>().ok().filter(|s| s.abs() <= MAX_SECONDS);
    let move_number = number(fields[26]).filter(|n| (1..=MAX_MOVE_NUMBER).contains(n))?;
    // Os campos 27 a 29 (Lance anterior, tempo gasto, notação) não decidem nada: o Lance é
    // deduzido do tabuleiro pelo núcleo de regras. O 30 é o giro do tabuleiro.
    flag(fields[30])?;
    Some(Style12 {
        placement: ranks.join("/"),
        side,
        double_push,
        castling,
        halfmove_clock: number(fields[15])?,
        game: number(fields[16]).filter(|n| *n > 0)?,
        white: valid_name(fields[17])?,
        black: valid_name(fields[18])?,
        relation,
        initial_minutes: number(fields[20])?,
        increment_s: number(fields[21])?,
        white_s: seconds(fields[24])?,
        black_s: seconds(fields[25])?,
        move_number,
        ticking: fields.get(31).and_then(|f| flag(f)),
    })
}

/// `N (W vs. B) texto} resultado` (o `{Game ` já saiu).
fn parse_game_line(rest: &str) -> Option<Message> {
    let (number_text, rest) = rest.split_once(" (")?;
    let game = number(number_text)?;
    let (players, rest) = rest.split_once(") ")?;
    let (white, black) = players.split_once(" vs. ")?;
    let (white, black) = (valid_name(white)?, valid_name(black)?);
    let (text, result) = rest.split_once('}')?;
    let result = result.trim();
    if let Some(created) = text.strip_prefix("Creating ") {
        let mut words = created.split_whitespace();
        let rated = match words.next()? {
            "rated" => true,
            "unrated" => false,
            _ => return None,
        };
        let kind = words.next()?.to_owned();
        return Some(Message::GameCreated(GameHeader {
            game,
            white,
            black,
            rated,
            kind,
        }));
    }
    ending(text, result).map(|end| Message::GameEnded { game, end })
}

/// Como o FICS encerrou a Partida: o resultado vem do token final; o motivo, do texto.
pub(crate) fn ending(text: &str, result: &str) -> Option<ProviderEnd> {
    let result = match result {
        "1-0" => GameResult::WhiteWins,
        "0-1" => GameResult::BlackWins,
        "1/2-1/2" => GameResult::Draw,
        "*" => return Some(ProviderEnd::Aborted),
        _ => return None,
    };
    let text = text.to_ascii_lowercase();
    let draw = result == GameResult::Draw;
    let reason = if text.contains("checkmated") {
        EndReason::Checkmate
    } else if text.contains("resigns") {
        EndReason::Resignation
    } else if text.contains("forfeits by disconnection") {
        EndReason::Disconnection
    } else if text.contains("forfeits on time") {
        EndReason::Timeout
    } else if text.contains("ran out of time") && draw {
        EndReason::TimeoutDraw
    } else if text.contains("stalemate") {
        EndReason::Stalemate
    } else if text.contains("repetition") {
        EndReason::ThreefoldRepetition
    } else if text.contains("50 move rule") {
        EndReason::FiftyMoveRule
    } else if text.contains("mating material") {
        EndReason::InsufficientMaterial
    } else if text.contains("mutual agreement") && draw {
        EndReason::DrawAgreement
    } else {
        EndReason::EndedByProvider
    };
    Some(ProviderEnd::Decided(Outcome { result, reason }))
}

/// `N: W (r) B (r) unrated blitz 5 0` (o `Game ` já saiu).
fn parse_observed_header(rest: &str) -> Option<GameHeader> {
    let (number_text, rest) = rest.split_once(": ")?;
    let game = number(number_text)?;
    let words: Vec<&str> = rest.split_whitespace().collect();
    if words.len() < 6 {
        return None;
    }
    let rated = match words[4] {
        "rated" => true,
        "unrated" => false,
        _ => return None,
    };
    Some(GameHeader {
        game,
        white: valid_name(words[0])?,
        black: valid_name(words[2])?,
        rated,
        kind: words[5].to_owned(),
    })
}

fn rating(text: &str) -> Option<u32> {
    let digits: String = text
        .trim_matches(|c| c == '(' || c == ')')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    number(&digits)
}

/// `A (r) B (r) unrated blitz 5 0.` (o `Challenge: ` já saiu).
fn parse_challenge(rest: &str) -> Option<FicsChallenge> {
    let words: Vec<&str> = rest.trim_end_matches('.').split_whitespace().collect();
    let at = words
        .iter()
        .position(|w| *w == "rated" || *w == "unrated")?;
    if at < 3 || words.len() < at + 4 {
        return None;
    }
    Some(FicsChallenge {
        challenger: valid_name(words[0])?,
        rating: rating(words[1]),
        receiver: valid_name(words[2])?,
        rated: words[at] == "rated",
        kind: words[at + 1].to_owned(),
        minutes: number(words[at + 2])?,
        increment_s: number(words[at + 3])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O exemplo da ajuda oficial (`help style12`), numa linha só.
    const HELP_EXAMPLE: &str = "<12> rnbqkb-r pppppppp -----n-- -------- ----P--- -------- PPPPKPPP RNBQ-BNR B -1 0 0 1 1 0 7 Newton Einstein 1 2 12 39 39 119 122 2 K/e1-e2 (0:06) Ke2 0";

    #[test]
    fn o_exemplo_da_ajuda_do_style12() {
        let s = parse_style12(HELP_EXAMPLE).expect("exemplo da ajuda");
        assert_eq!(
            s.key(),
            "rnbqkb1r/pppppppp/5n2/8/4P3/8/PPPPKPPP/RNBQ1BNR b kq -"
        );
        assert_eq!(s.game, 7);
        assert_eq!((s.white.as_str(), s.black.as_str()), ("Newton", "Einstein"));
        assert_eq!(s.relation, Relation::MyMove);
        assert_eq!((s.initial_minutes, s.increment_s), (2, 12));
        assert_eq!((s.white_s, s.black_s), (119, 122));
        assert_eq!(s.ply(), 3);
        assert_eq!(s.ticking, None);
    }

    #[test]
    fn mensagens_de_inicio_e_fim_e_desafio() {
        assert_eq!(
            parse_line("fics% {Game 42 (GuestA vs. GuestB) Creating unrated blitz match.}"),
            Message::GameCreated(GameHeader {
                game: 42,
                white: "GuestA".into(),
                black: "GuestB".into(),
                rated: false,
                kind: "blitz".into(),
            })
        );
        let ended = |line: &str| match parse_line(line) {
            Message::GameEnded { end, .. } => Some(end),
            _ => None,
        };
        let decided = |result, reason| Some(ProviderEnd::Decided(Outcome { result, reason }));
        assert_eq!(
            ended("{Game 42 (GuestA vs. GuestB) GuestA forfeits on time} 0-1"),
            decided(GameResult::BlackWins, EndReason::Timeout)
        );
        assert_eq!(
            ended("{Game 42 (GuestA vs. GuestB) Game drawn by repetition} 1/2-1/2"),
            decided(GameResult::Draw, EndReason::ThreefoldRepetition)
        );
        assert_eq!(
            ended("{Game 42 (GuestA vs. GuestB) Game aborted on move 1} *"),
            Some(ProviderEnd::Aborted)
        );
        assert_eq!(
            ended("{Game 42 (GuestA vs. GuestB) GuestA resigns} 2-0"),
            None
        );
        match parse_line("Challenge: GuestB (1450) GuestA (----) unrated lightning 1 0.") {
            Message::Challenge(c) => {
                assert_eq!(c.challenger, "GuestB");
                assert_eq!(c.rating, Some(1450));
                assert_eq!(
                    (c.kind.as_str(), c.minutes, c.increment_s),
                    ("lightning", 1, 0)
                );
            }
            other => panic!("{other:?}"),
        }
        // Recado de outro jogador nunca vira mensagem do servidor.
        assert_eq!(
            parse_line("GuestZ tells you: GuestB offers you a draw."),
            Message::Other
        );
    }

    /// Propriedade: linhas parecidas com as do FICS, embaralhadas e cortadas, nunca fazem o
    /// parser entrar em pânico (xorshift determinístico).
    #[test]
    fn linhas_embaralhadas_nunca_derrubam_o_parser() {
        let seeds = [
            HELP_EXAMPLE,
            "{Game 42 (GuestA vs. GuestB) GuestA checkmated} 0-1",
            "Challenge: GuestB (----) GuestA (----) unrated blitz 5 0.",
            "Game 7: GuestA (++++) GuestB (++++) unrated blitz 3 0",
            "Press return to enter the server as \"GuestA\":",
            "**** Starting FICS session as GuestA(U) ****",
        ];
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..5000 {
            let base = seeds[(next() % seeds.len() as u64) as usize];
            let mut words: Vec<&str> = base.split(' ').collect();
            for _ in 0..(next() % 4) {
                let i = (next() % words.len() as u64) as usize;
                let j = (next() % words.len() as u64) as usize;
                words.swap(i, j);
            }
            let take = (next() % (words.len() as u64 + 1)) as usize;
            let line = words[..take].join(" ");
            let _ = parse_line(&line);
            let _ = parse_prompt(&line);
        }
    }
}
