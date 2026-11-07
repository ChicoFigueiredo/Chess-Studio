//! Exportação PGN da árvore: Seven Tag Roster, tags extras e movetext com
//! comentários `{}`, NAGs `$n` e variações `( )`, em linhas de até 80 colunas.
//! Reimportar o texto dá a mesma árvore.

use crate::{MoveNode, PgnGame};

const MAX_COLUMNS: usize = 80;
const STANDARD_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const ROSTER: [(&str, &str); 7] = [
    ("Event", "?"),
    ("Site", "?"),
    ("Date", "????.??.??"),
    ("Round", "?"),
    ("White", "?"),
    ("Black", "?"),
    ("Result", "*"),
];

impl PgnGame {
    /// Texto PGN da partida (tags, linha principal, variações, comentários e NAGs).
    pub fn to_pgn(&self) -> String {
        let mut out = String::new();
        self.write_tags(&mut out);
        out.push('\n');

        let mut fields = self.start_fen.split_whitespace().skip(1);
        let white = fields.next() != Some("b");
        let number = fields
            .nth(3)
            .and_then(|n| n.parse::<u32>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(1);

        let mut tokens: Vec<String> = Vec::new();
        for text in &self.comments {
            push_comment(&mut tokens, text);
        }
        write_line(&mut tokens, &self.moves, white, number);
        tokens.push(self.result.token().to_owned());
        wrap(&mut out, &tokens);
        out
    }

    fn write_tags(&self, out: &mut String) {
        for (name, default) in ROSTER {
            let value = if name == "Result" {
                self.result.token()
            } else {
                self.tag(name).unwrap_or(default)
            };
            push_tag(out, name, value);
        }
        let skip = |n: &str| ROSTER.iter().any(|(r, _)| *r == n);
        for (name, value) in self.tags.iter().filter(|(n, _)| !skip(n)) {
            push_tag(out, name, value);
        }
        if self.start_fen != STANDARD_FEN && self.tag("FEN").is_none() {
            push_tag(out, "SetUp", "1");
            push_tag(out, "FEN", &self.start_fen);
        }
    }
}

fn push_tag(out: &mut String, name: &str, value: &str) {
    let flat: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let escaped = flat.replace('\\', "\\\\").replace('"', "\\\"");
    out.push_str(&format!("[{name} \"{escaped}\"]\n"));
}

/// Comentário `{...}`: o PGN não tem escape para `}`, então ele vira `)`.
fn push_comment(tokens: &mut Vec<String>, raw: &str) {
    let flat: String = raw
        .chars()
        .map(|c| match c {
            '}' => ')',
            '{' => '(',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let words: Vec<&str> = flat.split_whitespace().collect();
    if words.is_empty() {
        return;
    }
    let last = words.len() - 1;
    for (i, word) in words.iter().enumerate() {
        let open = if i == 0 { "{" } else { "" };
        let close = if i == last { "}" } else { "" };
        tokens.push(format!("{open}{word}{close}"));
    }
}

fn write_line(tokens: &mut Vec<String>, nodes: &[MoveNode], mut white: bool, mut number: u32) {
    let mut need_number = true;
    for node in nodes {
        for text in &node.pre_comments {
            push_comment(tokens, text);
        }
        if white {
            tokens.push(format!("{number}."));
        } else if need_number {
            tokens.push(format!("{number}..."));
        }
        tokens.push(node.san.clone());
        tokens.extend(node.nags.iter().map(|n| format!("${n}")));
        for text in &node.comments {
            push_comment(tokens, text);
        }
        let mut interrupted = !node.comments.is_empty();
        for variation in node.variations.iter().filter(|v| !v.is_empty()) {
            let first = tokens.len();
            write_line(tokens, variation, white, number);
            tokens[first].insert(0, '(');
            if let Some(last) = tokens.last_mut() {
                last.push(')');
            }
            interrupted = true;
        }
        need_number = interrupted;
        if !white {
            number += 1;
        }
        white = !white;
    }
}

fn wrap(out: &mut String, tokens: &[String]) {
    let mut current = String::new();
    for token in tokens {
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
}
