//! Tokenizador do PGN: texto -> tokens com linha e coluna (1-based, em caracteres).
//! Não conhece xadrez; só a sintaxe do padrão (tags, símbolos, `{}`/`;`, `()`, NAG).

use crate::{PgnErrorKind, PgnResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Tok {
    LBracket,
    RBracket,
    /// Valor de tag, já sem escape.
    Str(String),
    /// Nome de tag, Lance (com `!`/`?` coladas), `0-0`, `--` ou símbolo solto.
    Symbol(String),
    /// `12.` ou `12...`; o número em si não é confiável e é ignorado.
    MoveNumber,
    /// Texto do comentário, com espaços normalizados.
    Comment(String),
    LParen,
    RParen,
    Nag(u8),
    Result(PgnResult),
    /// Qualquer outro caractere.
    Other(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
}

pub(crate) type LexError = (u32, u32, PgnErrorKind);

pub(crate) struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    line: u32,
    col: u32,
}

fn is_symbol_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '+' | '#' | '=' | ':' | '-' | '/' | '!' | '?' | '@')
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// NAG de uma sequência de `!`/`?`.
pub(crate) fn annotation_nag(marks: &str) -> Option<u8> {
    match marks {
        "!" => Some(1),
        "?" => Some(2),
        "!!" => Some(3),
        "??" => Some(4),
        "!?" => Some(5),
        "?!" => Some(6),
        _ => None,
    }
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        Self {
            src,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn take_while(&mut self, pred: impl Fn(char) -> bool) -> &'a str {
        let start = self.pos;
        while self.peek_char().is_some_and(&pred) {
            self.bump();
        }
        &self.src[start..self.pos]
    }

    /// Próximo token, ou `None` no fim do texto.
    pub fn next_token(&mut self) -> Result<Option<Token>, LexError> {
        loop {
            let Some(c) = self.peek_char() else {
                return Ok(None);
            };
            if c.is_whitespace() || c == '\u{feff}' {
                self.bump();
                continue;
            }
            if c == '%' && self.col == 1 {
                self.take_while(|c| c != '\n');
                continue;
            }
            let (line, col) = (self.line, self.col);
            let tok = match c {
                '{' => {
                    self.bump();
                    let rest = &self.src[self.pos..];
                    let Some(end) = rest.find('}') else {
                        self.pos = self.src.len();
                        return Err((line, col, PgnErrorKind::UnterminatedComment));
                    };
                    let text = normalize(&rest[..end]);
                    while self.pos < self.src.len() - rest.len() + end {
                        self.bump();
                    }
                    self.bump(); // `}`
                    Tok::Comment(text)
                }
                ';' => {
                    self.bump();
                    Tok::Comment(normalize(self.take_while(|c| c != '\n')))
                }
                '[' => {
                    self.bump();
                    Tok::LBracket
                }
                ']' => {
                    self.bump();
                    Tok::RBracket
                }
                '(' => {
                    self.bump();
                    Tok::LParen
                }
                ')' => {
                    self.bump();
                    Tok::RParen
                }
                '*' => {
                    self.bump();
                    Tok::Result(PgnResult::Unknown)
                }
                '"' => self.string(line, col)?,
                '$' => {
                    self.bump();
                    let digits = self.take_while(|c| c.is_ascii_digit());
                    match digits.parse::<u8>() {
                        Ok(n) => Tok::Nag(n),
                        Err(_) => {
                            return Err((
                                line,
                                col,
                                PgnErrorKind::InvalidNag(format!("${digits}")),
                            ));
                        }
                    }
                }
                '.' => {
                    // Pontos soltos (`...` depois do número, `e.p.`) não carregam informação.
                    self.take_while(|c| c == '.');
                    continue;
                }
                c if c.is_ascii_digit() => {
                    let digits = self.take_while(|c| c.is_ascii_digit());
                    if self.peek_char() == Some('.') {
                        self.take_while(|c| c == '.');
                        Tok::MoveNumber
                    } else if self.peek_char().is_some_and(is_symbol_char) {
                        let rest = self.take_while(is_symbol_char);
                        self.symbol(format!("{digits}{rest}"))
                    } else {
                        Tok::Symbol(digits.to_owned())
                    }
                }
                c if is_symbol_char(c) => {
                    let text = self.take_while(is_symbol_char).to_owned();
                    self.symbol(text)
                }
                c => {
                    self.bump();
                    Tok::Other(c)
                }
            };
            return Ok(Some(Token { tok, line, col }));
        }
    }

    fn symbol(&self, text: String) -> Tok {
        match PgnResult::from_token(&text) {
            Some(r) => Tok::Result(r),
            None => Tok::Symbol(text),
        }
    }

    fn string(&mut self, line: u32, col: u32) -> Result<Tok, LexError> {
        self.bump(); // abre aspas
        let mut out = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n' | '\r') => {
                    return Err((line, col, PgnErrorKind::UnterminatedTagValue));
                }
                Some('"') => {
                    self.bump();
                    return Ok(Tok::Str(out));
                }
                Some('\\') => {
                    self.bump();
                    match self.peek_char() {
                        Some(e @ ('"' | '\\')) => {
                            self.bump();
                            out.push(e);
                        }
                        _ => out.push('\\'),
                    }
                }
                Some(c) => {
                    self.bump();
                    out.push(c);
                }
            }
        }
    }

    /// Recuperação (modo tolerante): avança até o início da próxima partida, uma
    /// linha que começa em `[Event ` ou em `[` logo depois de uma linha em branco.
    pub fn skip_to_next_game(&mut self) {
        // Termina a linha atual.
        self.take_while(|c| c != '\n');
        self.bump();
        let mut prev_blank = false;
        while self.pos < self.src.len() {
            let rest = &self.src[self.pos..];
            let line_end = rest.find('\n').unwrap_or(rest.len());
            let text = rest[..line_end].trim();
            if text.starts_with("[Event ") || (prev_blank && text.starts_with('[')) {
                return;
            }
            prev_blank = text.is_empty();
            for _ in rest[..line_end].chars() {
                self.bump();
            }
            self.bump(); // '\n'
        }
    }
}
