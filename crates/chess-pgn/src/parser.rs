//! Parser recursivo do movetext sobre os tokens do [`crate::lexer`]. Cada Lance é
//! validado pelo `chess-core` na Posição em que aparece.

use crate::lexer::{LexError, Lexer, Tok, Token, annotation_nag};
use crate::{MoveNode, PgnError, PgnErrorKind, PgnGame, PgnResult};
use chess_core::CanonicalPosition;

/// Limite de aninhamento de variações (protege a pilha de entradas hostis).
const MAX_DEPTH: usize = 128;

/// Resultado do modo tolerante: o que deu para importar e o que falhou.
#[derive(Debug, Default)]
pub struct ParseReport {
    pub games: Vec<PgnGame>,
    pub errors: Vec<PgnError>,
}

/// Importa todas as partidas do texto; o primeiro erro interrompe a importação.
pub fn parse_pgn(text: &str) -> Result<Vec<PgnGame>, PgnError> {
    let mut parser = Parser::new(text);
    let mut games = Vec::new();
    while let Some(game) = parser.parse_game()? {
        games.push(game);
    }
    Ok(games)
}

/// Importa o que for válido; uma partida com erro é pulada (e relatada) e a
/// leitura recomeça na partida seguinte.
pub fn parse_pgn_lenient(text: &str) -> ParseReport {
    let mut parser = Parser::new(text);
    let mut report = ParseReport::default();
    loop {
        match parser.parse_game() {
            Ok(Some(game)) => report.games.push(game),
            Ok(None) => break,
            Err(e) => {
                report.errors.push(e);
                parser.recover();
            }
        }
    }
    report
}

struct Parser<'a> {
    lex: Lexer<'a>,
    peeked: Option<Token>,
    game_no: usize,
}

/// Linha de Lances lida até o `)` / resultado / fim.
struct Line {
    nodes: Vec<MoveNode>,
    /// Comentários antes do primeiro Lance (e únicos, se não houver Lance).
    loose: Vec<String>,
    result: Option<PgnResult>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lex: Lexer::new(text),
            peeked: None,
            game_no: 0,
        }
    }

    fn error(&self, line: u32, column: u32, kind: PgnErrorKind) -> PgnError {
        PgnError {
            game: self.game_no,
            line,
            column,
            kind,
        }
    }

    fn lex_error(&self, (line, column, kind): LexError) -> PgnError {
        self.error(line, column, kind)
    }

    fn peek(&mut self) -> Result<Option<&Token>, PgnError> {
        if self.peeked.is_none() {
            self.peeked = self.lex.next_token().map_err(|e| self.lex_error(e))?;
        }
        Ok(self.peeked.as_ref())
    }

    fn next(&mut self) -> Result<Option<Token>, PgnError> {
        if let Some(t) = self.peeked.take() {
            return Ok(Some(t));
        }
        self.lex.next_token().map_err(|e| self.lex_error(e))
    }

    /// Depois de um erro: se o token já lido abre a partida seguinte, segue dele;
    /// senão pula até o começo da próxima partida.
    fn recover(&mut self) {
        if matches!(
            self.peeked,
            Some(Token {
                tok: Tok::LBracket,
                ..
            })
        ) {
            return;
        }
        self.peeked = None;
        self.lex.skip_to_next_game();
    }

    fn parse_game(&mut self) -> Result<Option<PgnGame>, PgnError> {
        if self.peek()?.is_none() {
            return Ok(None);
        }
        self.game_no += 1;
        let mut tags: Vec<(String, String)> = Vec::new();
        let mut fen_at = (1, 1);
        while matches!(
            self.peek()?,
            Some(Token {
                tok: Tok::LBracket,
                ..
            })
        ) {
            let (name, value, at) = self.parse_tag()?;
            if name == "FEN" {
                fen_at = at;
            }
            tags.push((name, value));
        }
        let tag = |n: &str| tags.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        let start = match tag("FEN").filter(|_| tag("SetUp") != Some("0")) {
            Some(fen) => CanonicalPosition::from_fen(fen).map_err(|source| {
                self.error(
                    fen_at.0,
                    fen_at.1,
                    PgnErrorKind::InvalidFen {
                        fen: fen.to_owned(),
                        source,
                    },
                )
            })?,
            None => CanonicalPosition::start(),
        };
        let tag_result = tag("Result").and_then(PgnResult::from_token);

        let line = self.parse_line(&start, 0, (0, 0))?;
        if tags.is_empty() && line.nodes.is_empty() && line.result.is_none() {
            // Só comentários soltos (por exemplo depois do último resultado).
            self.game_no -= 1;
            return self.parse_game();
        }
        Ok(Some(PgnGame {
            start_fen: start.to_fen(),
            tags,
            moves: line.nodes,
            result: line.result.or(tag_result).unwrap_or(PgnResult::Unknown),
            comments: line.loose,
        }))
    }

    /// `[Nome "valor"]`; devolve também onde o valor começa.
    fn parse_tag(&mut self) -> Result<(String, String, (u32, u32)), PgnError> {
        let open = self.next()?.map_or((1, 1), |t| (t.line, t.col));
        let bad = |p: &Self, t: Option<Token>, why: &str| {
            let (l, c) = t.map_or(open, |t| (t.line, t.col));
            p.error(l, c, PgnErrorKind::MalformedTag(why.to_owned()))
        };
        let name = match self.next()? {
            Some(Token {
                tok: Tok::Symbol(n),
                ..
            }) => n,
            other => return Err(bad(self, other, "falta o nome da tag")),
        };
        let (value, at) = match self.next()? {
            Some(Token {
                tok: Tok::Str(v),
                line,
                col,
            }) => (v, (line, col)),
            other => return Err(bad(self, other, "o valor precisa estar entre aspas")),
        };
        match self.next()? {
            Some(Token {
                tok: Tok::RBracket, ..
            }) => Ok((name, value, at)),
            other => Err(bad(self, other, "falta `]`")),
        }
    }

    /// Lê uma linha de Lances a partir de `start`. `open` é onde o `(` desta linha
    /// está (para o erro de variação sem fecho); `depth` 0 é a linha principal.
    fn parse_line(
        &mut self,
        start: &CanonicalPosition,
        depth: usize,
        open: (u32, u32),
    ) -> Result<Line, PgnError> {
        let mut line = Line {
            nodes: Vec::new(),
            loose: Vec::new(),
            result: None,
        };
        let mut pos = start.clone();
        let mut before_last: Option<CanonicalPosition> = None;
        let mut pending_pre: Vec<String> = Vec::new();

        loop {
            let Some(token) = self.peek()?.cloned() else {
                if depth > 0 {
                    return Err(self.error(open.0, open.1, PgnErrorKind::UnclosedVariation));
                }
                break;
            };
            let at = (token.line, token.col);
            if token.tok == Tok::LBracket {
                if depth > 0 {
                    return Err(self.error(open.0, open.1, PgnErrorKind::UnclosedVariation));
                }
                break; // começa a próxima partida; o token fica para ela
            }
            self.next()?;
            match token.tok {
                Tok::MoveNumber => {}
                Tok::Comment(text) => {
                    if !text.is_empty() {
                        match line.nodes.last_mut() {
                            Some(last) => last.comments.push(text),
                            None => pending_pre.push(text),
                        }
                    }
                }
                Tok::Nag(n) => self.annotate(&mut line, n, at)?,
                Tok::Result(r) => {
                    if depth == 0 {
                        line.result = Some(r);
                        break;
                    }
                }
                Tok::LParen => {
                    if depth + 1 > MAX_DEPTH {
                        return Err(self.error(at.0, at.1, PgnErrorKind::NestingTooDeep));
                    }
                    let Some(base) = before_last.as_ref() else {
                        return Err(self.error(at.0, at.1, PgnErrorKind::VariationWithoutMove));
                    };
                    let inner = self.parse_line(base, depth + 1, at)?;
                    if inner.nodes.is_empty() {
                        return Err(self.error(at.0, at.1, PgnErrorKind::EmptyVariation));
                    }
                    if let Some(last) = line.nodes.last_mut() {
                        last.variations.push(inner.nodes);
                    }
                }
                Tok::RParen => {
                    if depth == 0 {
                        return Err(self.error(at.0, at.1, PgnErrorKind::UnbalancedCloseParen));
                    }
                    break;
                }
                Tok::Symbol(text) => {
                    self.symbol(
                        &text,
                        at,
                        &mut line,
                        &mut pos,
                        &mut before_last,
                        &mut pending_pre,
                    )?;
                }
                Tok::Str(s) => {
                    return Err(self.error(
                        at.0,
                        at.1,
                        PgnErrorKind::UnexpectedToken(format!("{s:?}")),
                    ));
                }
                Tok::RBracket => {
                    return Err(self.error(at.0, at.1, PgnErrorKind::UnexpectedToken("]".into())));
                }
                Tok::Other(c) => {
                    return Err(self.error(
                        at.0,
                        at.1,
                        PgnErrorKind::UnexpectedToken(c.to_string()),
                    ));
                }
                Tok::LBracket => {}
            }
        }
        // Comentários antes de um Lance que não veio (partida sem Lances).
        line.loose = pending_pre;
        Ok(line)
    }

    fn annotate(&self, line: &mut Line, nag: u8, at: (u32, u32)) -> Result<(), PgnError> {
        match line.nodes.last_mut() {
            Some(last) => {
                last.nags.push(nag);
                Ok(())
            }
            None => Err(self.error(at.0, at.1, PgnErrorKind::AnnotationWithoutMove)),
        }
    }

    fn symbol(
        &self,
        text: &str,
        at: (u32, u32),
        line: &mut Line,
        pos: &mut CanonicalPosition,
        before_last: &mut Option<CanonicalPosition>,
        pending_pre: &mut Vec<String>,
    ) -> Result<(), PgnError> {
        // Número sem ponto (`12 e4`): sem informação.
        if text.chars().all(|c| c.is_ascii_digit()) {
            return Ok(());
        }
        // Anotação solta: `!`, `?!`, `=`, `+-`...
        if let Some(nag) = symbol_nag(text) {
            return self.annotate(line, nag, at);
        }
        if matches!(text, "--" | "Z0" | "@@@@" | "0000") {
            return Err(self.error(at.0, at.1, PgnErrorKind::NullMove(text.to_owned())));
        }
        let core = text.trim_end_matches(['!', '?']);
        let marks = &text[core.len()..];
        let illegal = |source| {
            self.error(
                at.0,
                at.1,
                PgnErrorKind::IllegalMove {
                    san: text.to_owned(),
                    ply_in_line: line.nodes.len() + 1,
                    fen: pos.to_fen(),
                    source,
                },
            )
        };
        let mv = pos.parse_san(core).map_err(illegal)?;
        // Já validado como legal; as falhas abaixo não ocorrem na prática.
        let san = pos
            .san(mv)
            .map_err(|_| illegal(chess_core::SanError::NoMatchingMove(core.to_owned())))?;
        let next = pos
            .play(mv)
            .map_err(|_| illegal(chess_core::SanError::NoMatchingMove(core.to_owned())))?;
        let mut node = MoveNode::new(san);
        node.pre_comments = std::mem::take(pending_pre);
        if !marks.is_empty() {
            match annotation_nag(marks) {
                Some(n) => node.nags.push(n),
                None => {
                    return Err(self.error(at.0, at.1, PgnErrorKind::InvalidNag(marks.to_owned())));
                }
            }
        }
        line.nodes.push(node);
        *before_last = Some(std::mem::replace(pos, next));
        Ok(())
    }
}

/// Símbolos de anotação que não são Lance: `!`/`?` isolados e os glifos `=`, `+-`...
fn symbol_nag(text: &str) -> Option<u8> {
    if text.chars().all(|c| matches!(c, '!' | '?')) {
        return annotation_nag(text);
    }
    match text {
        "=" => Some(10),
        "+/=" => Some(14),
        "=/+" => Some(15),
        "+/-" => Some(16),
        "-/+" => Some(17),
        "+-" => Some(18),
        "-+" => Some(19),
        _ => None,
    }
}
