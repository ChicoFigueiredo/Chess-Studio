//! Temas oficiais dos problemas do Lichess: os identificadores fixos que aparecem na coluna
//! `Themes` do arquivo de problemas (`mateIn2`, `fork`, `pin`, `endgame`…), agrupados como o
//! próprio Lichess os apresenta. Os **nomes em português** vivem nos textos da interface
//! (Fluent, `puzzle-theme-<id>`); aqui ficam só a lista, os grupos e a normalização, que não
//! dependem de idioma. A lista segue `PuzzleTheme.categorized` do Lichess (lila).

/// Grupo de apresentação, na ordem em que o Lichess os mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeGroup {
    /// Fase da partida e tipo de final.
    Phases,
    /// Motivos táticos comuns.
    Motifs,
    /// Táticas mais avançadas.
    Advanced,
    /// Mate em tantos lances.
    Mates,
    /// Padrões de mate com nome próprio.
    MatePatterns,
    /// Roque, en passant, promoção.
    SpecialMoves,
    /// O que o problema busca alcançar.
    Goals,
    /// Tamanho da solução.
    Lengths,
    /// De onde vem a partida do problema.
    Origin,
}

impl ThemeGroup {
    pub const ALL: [ThemeGroup; 9] = [
        Self::Phases,
        Self::Motifs,
        Self::Advanced,
        Self::Mates,
        Self::MatePatterns,
        Self::SpecialMoves,
        Self::Goals,
        Self::Lengths,
        Self::Origin,
    ];

    /// Chave estável, usada nos textos da interface (`puzzle-theme-group-<chave>`).
    pub const fn key(self) -> &'static str {
        match self {
            Self::Phases => "phases",
            Self::Motifs => "motifs",
            Self::Advanced => "advanced",
            Self::Mates => "mates",
            Self::MatePatterns => "mate-patterns",
            Self::SpecialMoves => "special-moves",
            Self::Goals => "goals",
            Self::Lengths => "lengths",
            Self::Origin => "origin",
        }
    }
}

use ThemeGroup::{Advanced, Goals, Lengths, MatePatterns, Mates, Motifs, Origin, Phases};
use ThemeGroup::SpecialMoves;

/// Todos os temas oficiais visíveis do Lichess, por grupo. "Mate" aparece só em
/// [`ThemeGroup::Mates`] (o Lichess o repete em Objetivos).
pub const OFFICIAL_THEMES: [(&str, ThemeGroup); 73] = [
    ("opening", Phases),
    ("middlegame", Phases),
    ("endgame", Phases),
    ("rookEndgame", Phases),
    ("bishopEndgame", Phases),
    ("pawnEndgame", Phases),
    ("knightEndgame", Phases),
    ("queenEndgame", Phases),
    ("queenRookEndgame", Phases),
    ("advancedPawn", Motifs),
    ("attackingF2F7", Motifs),
    ("capturingDefender", Motifs),
    ("discoveredAttack", Motifs),
    ("doubleCheck", Motifs),
    ("exposedKing", Motifs),
    ("fork", Motifs),
    ("hangingPiece", Motifs),
    ("kingsideAttack", Motifs),
    ("pin", Motifs),
    ("queensideAttack", Motifs),
    ("sacrifice", Motifs),
    ("skewer", Motifs),
    ("trappedPiece", Motifs),
    ("attraction", Advanced),
    ("clearance", Advanced),
    ("collinearMove", Advanced),
    ("discoveredCheck", Advanced),
    ("defensiveMove", Advanced),
    ("deflection", Advanced),
    ("interference", Advanced),
    ("intermezzo", Advanced),
    ("quietMove", Advanced),
    ("xRayAttack", Advanced),
    ("zugzwang", Advanced),
    ("mate", Mates),
    ("mateIn1", Mates),
    ("mateIn2", Mates),
    ("mateIn3", Mates),
    ("mateIn4", Mates),
    ("mateIn5", Mates),
    ("anastasiaMate", MatePatterns),
    ("arabianMate", MatePatterns),
    ("backRankMate", MatePatterns),
    ("balestraMate", MatePatterns),
    ("blindSwineMate", MatePatterns),
    ("bodenMate", MatePatterns),
    ("cornerMate", MatePatterns),
    ("doubleBishopMate", MatePatterns),
    ("dovetailMate", MatePatterns),
    ("epauletteMate", MatePatterns),
    ("hookMate", MatePatterns),
    ("killBoxMate", MatePatterns),
    ("pillsburysMate", MatePatterns),
    ("morphysMate", MatePatterns),
    ("operaMate", MatePatterns),
    ("swallowstailMate", MatePatterns),
    ("triangleMate", MatePatterns),
    ("vukovicMate", MatePatterns),
    ("smotheredMate", MatePatterns),
    ("castling", SpecialMoves),
    ("enPassant", SpecialMoves),
    ("promotion", SpecialMoves),
    ("underPromotion", SpecialMoves),
    ("equality", Goals),
    ("advantage", Goals),
    ("crushing", Goals),
    ("oneMove", Lengths),
    ("short", Lengths),
    ("long", Lengths),
    ("veryLong", Lengths),
    ("master", Origin),
    ("masterVsMaster", Origin),
    ("superGM", Origin),
];

/// Temas que existem nos dados mas que o Lichess não oferece para escolha (precisam de
/// nome, para a lista de Exercícios, mas não aparecem no seletor).
pub const HIDDEN_THEMES: [&str; 1] = ["checkFirst"];

/// Identificadores oficiais de um grupo, na ordem do Lichess.
pub fn themes_in(group: ThemeGroup) -> impl Iterator<Item = &'static str> {
    OFFICIAL_THEMES
        .iter()
        .filter(move |(_, g)| *g == group)
        .map(|(id, _)| *id)
}

/// Todo identificador que o app sabe nomear (oficiais e ocultos).
pub fn known_themes() -> impl Iterator<Item = &'static str> {
    OFFICIAL_THEMES
        .iter()
        .map(|(id, _)| *id)
        .chain(HIDDEN_THEMES)
}

/// O identificador oficial equivalente a `text` (sem diferenciar maiúsculas), se houver.
pub fn canonical_theme(text: &str) -> Option<&'static str> {
    let text = text.trim();
    known_themes().find(|id| id.eq_ignore_ascii_case(text))
}
