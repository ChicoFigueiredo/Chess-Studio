//! Conferência da Súmula manual contra os Lances reais (V2-SUM-02).
//!
//! Compara, meio-lance a meio-lance, o que o jogador escreveu na [`ManualScoresheet`]
//! com o Lance que de fato foi jogado. É **só leitura**: recebe a Posição inicial e os
//! Lances (ou uma [`Game`] emprestada) e devolve um [`ScoresheetCheck`] à parte. Nunca
//! muda a Partida, o log de eventos nem o PGN (ADR-0001/0002), e não usa relógio: o
//! instante da conferência entra por parâmetro.
//!
//! # Classes do resultado ([`Verdict`])
//!
//! | Classe | Quando |
//! |---|---|
//! | `Correct` | o texto é exatamente o SAN canônico do `chess-core` (`Nf3` sim; `Cf3` não) |
//! | `AcceptedWithCaveat` | o texto, lido pela regra abaixo, é o Lance real, mas difere do SAN canônico |
//! | `WrongNotation` | o texto não identifica um Lance legal único nesta Posição (ilegal, ilegível ou ambíguo) |
//! | `Swapped` | o texto é um Lance legal, mas **outro** que não o jogado |
//! | `Missing` | há Lance real e a Súmula está em branco nesse meio-lance |
//! | `Extra` | há anotação além do último Lance jogado |
//!
//! # Regra de normalização (o que é aceito "com ressalva")
//!
//! O texto é lido contra a Posição **antes** do Lance, com a leitura tolerante do
//! `chess-core` (`parse_san`) sobre duas leituras do alfabeto das peças; vale a união:
//!
//! 1. **Inglesa/FIDE**: `K Q R B N` (a que o SAN canônico usa).
//! 2. **pt-BR**: `R`=Rei, `D`=Dama, `T`=Torre, `B`=Bispo, `C`=Cavalo, traduzida para a
//!    inglesa antes de ler (`Cf3` vira `Nf3`, `Dd1` vira `Qd1`). Vale também na promoção
//!    (`e8=D`, `e8D`, `e8=T`). Peão não tem letra.
//!
//!    `R` é ambígua entre as duas (Torre em inglês, Rei em pt-BR): a conferência admite
//!    as duas leituras e aceita o texto se **alguma** delas dá o Lance real. Se nenhuma
//!    dá, mas alguma dá outro Lance legal, é `Swapped`.
//!
//! Além do alfabeto, não geram erro (só ressalva): sufixo `+`/`#` ausente, a mais ou
//! trocado; anotações `!`/`?`; `x` de captura omitido (`ed5` por `exd5`, `Cf3` por `Cxf3`);
//! roque com zeros (`0-0`, `0-0-0`) ou `o` minúsculo; promoção com ou sem `=`; `e.p.`.
//! Letra de peça **minúscula** não é aceita (`nf3` é lido como lance de peão e falha),
//! porque colidiria com colunas (`b`).
//!
//! **Desambiguação**: a *desnecessária* (`Nbd2` quando só um cavalo chega) é aceita com
//! ressalva; a *faltante* (`Cd2` quando dois cavalos chegam a `d2`) é `WrongNotation`,
//! porque numa competição a anotação ambígua não identifica o Lance.
//!
//! O resultado é independente da Partida e guarda, por meio-lance, o texto escrito e o
//! SAN real, pronto para persistir e para o histórico (E3).

use crate::scoresheet::ManualScoresheet;
use chess_core::{CanonicalPosition, Move};
use chess_game::Game;
use std::collections::BTreeMap;

/// Como uma anotação se compara com o Lance real.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verdict {
    Correct,
    AcceptedWithCaveat,
    WrongNotation,
    Swapped,
    Missing,
    Extra,
}

impl Verdict {
    /// Todas as classes, na ordem de gravação/exibição.
    pub const ALL: [Verdict; 6] = [
        Self::Correct,
        Self::AcceptedWithCaveat,
        Self::WrongNotation,
        Self::Swapped,
        Self::Missing,
        Self::Extra,
    ];

    /// Chave estável (persistência).
    pub fn key(self) -> &'static str {
        match self {
            Self::Correct => "correct",
            Self::AcceptedWithCaveat => "accepted",
            Self::WrongNotation => "wrong_notation",
            Self::Swapped => "swapped",
            Self::Missing => "missing",
            Self::Extra => "extra",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.key() == key)
    }

    /// Conta como acerto (Lance identificado corretamente).
    pub fn is_right(self) -> bool {
        matches!(self, Self::Correct | Self::AcceptedWithCaveat)
    }
}

/// A conferência de um meio-lance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryCheck {
    /// Meio-lance, a partir de 1.
    pub ply: u16,
    /// O que o jogador escreveu (`None` em `Missing`).
    pub written: Option<String>,
    /// O SAN canônico do Lance real (`None` em `Extra`).
    pub expected: Option<String>,
    pub verdict: Verdict,
}

/// Quantas entradas há em cada classe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub correct: usize,
    pub accepted: usize,
    pub wrong_notation: usize,
    pub swapped: usize,
    pub missing: usize,
    pub extra: usize,
}

/// Resultado da conferência de uma Súmula entregue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoresheetCheck {
    checked_at_ms: i64,
    entries: Vec<EntryCheck>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckError {
    /// Só se confere uma Súmula entregue (em anotação, ela ainda está sendo escrita).
    NotDelivered,
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a Súmula ainda não foi entregue")
    }
}

impl std::error::Error for CheckError {}

impl ScoresheetCheck {
    /// Reconstrói um resultado gravado (ordena por meio-lance).
    pub fn from_parts(checked_at_ms: i64, mut entries: Vec<EntryCheck>) -> Self {
        entries.sort_by_key(|e| e.ply);
        Self {
            checked_at_ms,
            entries,
        }
    }

    /// Instante (injetado) em que a conferência foi feita.
    pub fn checked_at_ms(&self) -> i64 {
        self.checked_at_ms
    }

    /// Uma entrada por meio-lance, em ordem: os Lances reais (anotados ou não) e as
    /// anotações além deles.
    pub fn entries(&self) -> &[EntryCheck] {
        &self.entries
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for e in &self.entries {
            *match e.verdict {
                Verdict::Correct => &mut c.correct,
                Verdict::AcceptedWithCaveat => &mut c.accepted,
                Verdict::WrongNotation => &mut c.wrong_notation,
                Verdict::Swapped => &mut c.swapped,
                Verdict::Missing => &mut c.missing,
                Verdict::Extra => &mut c.extra,
            } += 1;
        }
        c
    }

    /// Todos os Lances reais identificados e nada a mais.
    pub fn is_clean(&self) -> bool {
        self.entries.iter().all(|e| e.verdict.is_right())
    }
}

/// Confere a Súmula entregue contra a Partida (só lê o histórico atual).
pub fn check_against_game(
    sheet: &ManualScoresheet,
    game: &Game,
    at_ms: i64,
) -> Result<ScoresheetCheck, CheckError> {
    let start = game
        .review_at_ply(0)
        .map_or_else(CanonicalPosition::start, |v| v.position);
    let moves: Vec<Move> = game.snapshot().history.iter().map(|h| h.mv).collect();
    check_scoresheet(sheet, &start, &moves, at_ms)
}

/// Confere a Súmula entregue contra os Lances reais jogados a partir de `start`.
pub fn check_scoresheet(
    sheet: &ManualScoresheet,
    start: &CanonicalPosition,
    real_moves: &[Move],
    at_ms: i64,
) -> Result<ScoresheetCheck, CheckError> {
    if !sheet.is_delivered() {
        return Err(CheckError::NotDelivered);
    }
    let written: BTreeMap<u16, &str> = sheet.entries().collect();
    let mut entries = Vec::new();
    let mut pos = start.clone();
    let mut ply = 0u16;
    for mv in real_moves {
        let Ok(san) = pos.san(*mv) else { break };
        ply = ply.saturating_add(1);
        let (text, verdict) = match written.get(&ply) {
            None => (None, Verdict::Missing),
            Some(text) => (Some((*text).to_owned()), classify(&pos, *mv, &san, text)),
        };
        entries.push(EntryCheck {
            ply,
            written: text,
            expected: Some(san),
            verdict,
        });
        match pos.play(*mv) {
            Ok(next) => pos = next,
            Err(_) => break,
        }
    }
    for (p, text) in written.range(ply.saturating_add(1)..) {
        entries.push(EntryCheck {
            ply: *p,
            written: Some((*text).to_owned()),
            expected: None,
            verdict: Verdict::Extra,
        });
    }
    Ok(ScoresheetCheck {
        checked_at_ms: at_ms,
        entries,
    })
}

fn classify(pos: &CanonicalPosition, real: Move, real_san: &str, text: &str) -> Verdict {
    if text == real_san {
        return Verdict::Correct;
    }
    let mut other_legal = false;
    for reading in readings(text) {
        match pos.parse_san(&reading) {
            Ok(m) if m == real => return Verdict::AcceptedWithCaveat,
            Ok(_) => other_legal = true,
            Err(_) => {}
        }
    }
    if other_legal {
        Verdict::Swapped
    } else {
        Verdict::WrongNotation
    }
}

/// As leituras do texto: a inglesa e a pt-BR (já traduzida), sem repetir.
fn readings(text: &str) -> Vec<String> {
    let base = normalize_castling(text.trim());
    let pt = translate_pt(&base);
    if pt == base {
        vec![base]
    } else {
        vec![base, pt]
    }
}

/// `o-o` e `0-0` viram `O-O` (o `parse_san` já cobre zeros; falta o `o` minúsculo).
fn normalize_castling(text: &str) -> String {
    let core = text.trim_end_matches(['+', '#', '!', '?']);
    if !core.is_empty() && core.chars().all(|c| matches!(c, 'o' | 'O' | '0' | '-')) {
        text.replace(['o', '0'], "O")
    } else {
        text.to_owned()
    }
}

fn pt_piece(c: char) -> Option<char> {
    match c {
        'R' => Some('K'),
        'D' => Some('Q'),
        'T' => Some('R'),
        'C' => Some('N'),
        _ => None,
    }
}

/// Traduz as letras pt-BR (peça inicial e peça de promoção) para as inglesas.
fn translate_pt(text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    if let Some(first) = chars.first_mut()
        && let Some(en) = pt_piece(*first)
    {
        *first = en;
    }
    // Promoção: a letra depois de `=` ou, sem `=`, a colada a uma casa da fileira 1 ou 8
    // (`e8D`), antes de sufixos e anotações.
    let end = chars
        .iter()
        .rposition(|c| !matches!(c, '+' | '#' | '!' | '?'))
        .map_or(0, |i| i + 1);
    if end >= 3 {
        let last = end - 1;
        if matches!(chars[last - 1], '=' | '1' | '8')
            && let Some(en) = pt_piece(chars[last])
        {
            chars[last] = en;
        }
    }
    chars.into_iter().collect()
}
