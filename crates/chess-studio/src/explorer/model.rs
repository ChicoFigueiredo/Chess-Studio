//! Modelos do Painel Explorador, sem Bevy nem egui: o que cada aba mostra a partir do que o
//! serviço entregou. A interface só desenha estes valores; os testes os conferem direto.

use chess_core::{CanonicalPosition, Color, Move, Piece};
use chess_net::{Freshness, NetError, ProviderId, Timestamp};
use chess_providers::{
    Answer, Category, CloudEval, ExplorerPosition, MAX_PIECES, ProviderError, Score, Tablebase,
};
use chess_store::OpeningMoveStats;
use std::time::Duration;

// --- Escolhas do usuário -----------------------------------------------------------------

/// As abas do Painel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Openings,
    Endgames,
}

/// De onde vêm as estatísticas da aba Aberturas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    /// A Biblioteca do Perfil local: offline, o padrão.
    #[default]
    Library,
    /// Partidas de mestres (Explorador do Lichess).
    Masters,
    /// Todas as Partidas do Lichess.
    Lichess,
    /// As Partidas de um jogador (Explorador do Lichess).
    Player,
}

impl Source {
    pub const ALL: [Source; 4] = [
        Source::Library,
        Source::Masters,
        Source::Lichess,
        Source::Player,
    ];

    /// O Provedor consultado; `None` para a Biblioteca local.
    pub fn provider(self) -> Option<ProviderId> {
        match self {
            Source::Library => None,
            Source::Masters | Source::Lichess | Source::Player => Some(ProviderId::Lichess),
        }
    }

    pub fn is_online(self) -> bool {
        self.provider().is_some()
    }

    /// Chave estável (cache e identificação).
    pub fn key(self) -> &'static str {
        match self {
            Source::Library => "library",
            Source::Masters => "masters",
            Source::Lichess => "lichess",
            Source::Player => "player",
        }
    }

    /// Chave Fluent do nome da fonte.
    pub fn label_key(self) -> &'static str {
        match self {
            Source::Library => "explorer-source-library",
            Source::Masters => "explorer-source-masters",
            Source::Lichess => "explorer-source-lichess",
            Source::Player => "explorer-source-player",
        }
    }
}

/// A Conexão autorizada que o Explorador online do Lichess exige. A autorização ainda não
/// existe no app (o Cofre e o fluxo de entrar na conta chegam com a issue #224): enquanto isso
/// o Painel explica e leva à Central em vez de pedir (um pedido sem credencial só recebe 401).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorization {
    NotAvailable,
    Granted,
}

impl Authorization {
    /// Há Conexão autorizada do Lichess (papel jogador) para o Perfil local ativo?
    pub fn lichess_now(service: &crate::net_service::NetService) -> Self {
        match service
            .authorized_connection(ProviderId::Lichess, chess_store::ConnectionRole::Player)
        {
            Some(_) => Authorization::Granted,
            None => Authorization::NotAvailable,
        }
    }
}

/// Por que o Painel não consulta nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// Partida competitiva em andamento (Trava de fair play).
    Competitive,
    /// Exercício de Treino na tela.
    Training,
}

// --- Números e textos --------------------------------------------------------------------

/// `1234567` -> `1.234.567` (separador de milhar do pt-BR).
pub fn fmt_count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Porcentagens inteiras `(brancas, empates, pretas)` das Partidas decididas, somando 100 (o
/// resto do arredondamento vai para a maior fatia). Sem Partidas decididas: zeros.
pub fn percentages(white: u64, draws: u64, black: u64) -> (u32, u32, u32) {
    let total = white + draws + black;
    if total == 0 {
        return (0, 0, 0);
    }
    let share = |n: u64| u32::try_from(n * 100 / total).unwrap_or(100);
    let mut parts = [share(white), share(draws), share(black)];
    let left = 100 - parts.iter().sum::<u32>();
    let biggest = [white, draws, black]
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| **n)
        .map_or(0, |(i, _)| i);
    parts[biggest] += left;
    (parts[0], parts[1], parts[2])
}

/// Idade de um dado, em palavras que a interface sabe dizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    JustNow,
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

pub fn age_of(age: Duration) -> Age {
    let minutes = age.as_secs() / 60;
    match minutes {
        0 => Age::JustNow,
        1..=59 => Age::Minutes(minutes),
        60..=1439 => Age::Hours(minutes / 60),
        _ => Age::Days(minutes / 1440),
    }
}

/// `dd/mm` (UTC) de um instante, para "do cache, de 09/11".
pub fn day_month(at: Timestamp) -> String {
    let text = crate::openings::last_used_text(at.as_millis());
    text.get(..5).unwrap_or(&text).to_owned()
}

/// Quanto falta para `until`, arredondado para cima em minutos (mínimo 1).
pub fn minutes_until(now: Timestamp, until: Timestamp) -> u64 {
    let ms = (until.as_millis() - now.as_millis()).max(0);
    u64::try_from(ms).unwrap_or(0).div_ceil(60_000).max(1)
}

/// `HH:MM UTC` de um instante.
pub fn clock_utc(at: Timestamp) -> String {
    let secs = at.as_millis().div_euclid(1000).rem_euclid(86_400);
    format!("{:02}:{:02} UTC", secs / 3600, (secs / 60) % 60)
}

// --- Aba Aberturas -----------------------------------------------------------------------

/// Uma linha da tabela: o Lance e como as Partidas terminaram depois dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveRow {
    pub san: String,
    pub games: u64,
    pub white: u64,
    pub draws: u64,
    pub black: u64,
    /// Rating médio de quem jogou o Lance, quando o Provedor informa.
    pub rating: Option<u32>,
}

impl MoveRow {
    pub fn decided(&self) -> u64 {
        self.white + self.draws + self.black
    }

    pub fn percentages(&self) -> (u32, u32, u32) {
        percentages(self.white, self.draws, self.black)
    }

    /// Frações (0 a 1) para a barra; zeros sem Partidas decididas.
    pub fn shares(&self) -> (f32, f32, f32) {
        let total = self.decided();
        if total == 0 {
            return (0.0, 0.0, 0.0);
        }
        let f = |n: u64| n as f32 / total as f32;
        (f(self.white), f(self.draws), f(self.black))
    }
}

/// Partida de referência (as mais jogadas ou famosas naquela Posição).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceRow {
    pub id: String,
    pub white: String,
    pub white_rating: Option<u32>,
    pub black: String,
    pub black_rating: Option<u32>,
    /// `None`: empate.
    pub winner: Option<Color>,
    pub year: Option<u32>,
}

/// De onde vieram os números e como estão (rodapé do Painel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginNote {
    /// Nome do Provedor ("Lichess") ou da fonte local.
    pub provider: String,
    pub how: How,
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// Da Biblioteca do Perfil, sem rede.
    Local,
    /// Da rede, agora ou há pouco.
    Queried(Age),
    /// Do cache, dentro da validade, consultado em `queried_at`.
    Cached(Timestamp),
    /// Do cache vencido, porque a rede falhou: mostrar com a idade.
    Stale(Timestamp),
}

impl OriginNote {
    pub fn from_answer<T>(answer: &Answer<T>) -> Self {
        let queried_at = answer.provenance.queried_at;
        let how = match &answer.freshness {
            Freshness::Network | Freshness::Revalidated => How::Queried(age_of(answer.age)),
            Freshness::Cache => How::Cached(queried_at),
            Freshness::Stale { .. } => How::Stale(queried_at),
        };
        Self {
            provider: answer.provenance.provider.info().name.to_owned(),
            how,
            license: answer.provenance.license.clone(),
        }
    }

    #[cfg(test)]
    pub fn is_stale(&self) -> bool {
        matches!(self.how, How::Stale(_))
    }
}

/// A tabela pronta para desenhar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningsTable {
    /// Código ECO e nome da abertura, quando conhecidos.
    pub opening: Option<(String, String)>,
    /// Partidas que passaram pela Posição.
    pub total: u64,
    /// Total das Partidas por resultado, para a barra de cima.
    pub white: u64,
    pub draws: u64,
    pub black: u64,
    pub rows: Vec<MoveRow>,
    pub references: Vec<ReferenceRow>,
    pub origin: OriginNote,
    /// O fluxo do jogador ainda manda resultados melhores.
    pub still_loading: bool,
}

impl OpeningsTable {
    /// Alguma linha tem rating médio: a coluna aparece.
    pub fn has_ratings(&self) -> bool {
        self.rows.iter().any(|r| r.rating.is_some())
    }

    /// Da resposta do Explorador do Lichess.
    pub fn from_explorer(answer: &Answer<ExplorerPosition>, still_loading: bool) -> Self {
        let position = &answer.value;
        Self {
            opening: position
                .opening
                .as_ref()
                .map(|o| (o.eco.clone(), o.name.clone())),
            total: position.outcomes.total(),
            white: position.outcomes.white,
            draws: position.outcomes.draws,
            black: position.outcomes.black,
            rows: position
                .moves
                .iter()
                .map(|m| MoveRow {
                    san: m.san.clone(),
                    games: m.outcomes.total(),
                    white: m.outcomes.white,
                    draws: m.outcomes.draws,
                    black: m.outcomes.black,
                    rating: m.average_rating,
                })
                .collect(),
            references: position
                .games
                .iter()
                .map(|g| ReferenceRow {
                    id: g.id.clone(),
                    white: g.white.name.clone(),
                    white_rating: g.white.rating,
                    black: g.black.name.clone(),
                    black_rating: g.black.rating,
                    winner: g.winner,
                    year: g.year,
                })
                .collect(),
            origin: OriginNote::from_answer(answer),
            still_loading,
        }
    }

    /// Da estatística da Biblioteca local (`Store::opening_stats`) e do livro de aberturas.
    pub fn from_library(stats: &[OpeningMoveStats], opening: Option<(String, String)>) -> Self {
        let rows: Vec<MoveRow> = stats
            .iter()
            .map(|s| MoveRow {
                san: s.san.clone(),
                games: u64::from(s.games),
                white: u64::from(s.white_wins),
                draws: u64::from(s.draws),
                black: u64::from(s.black_wins),
                rating: None,
            })
            .collect();
        Self {
            opening,
            total: rows.iter().map(|r| r.games).sum(),
            white: rows.iter().map(|r| r.white).sum(),
            draws: rows.iter().map(|r| r.draws).sum(),
            black: rows.iter().map(|r| r.black).sum(),
            rows,
            references: Vec::new(),
            origin: OriginNote {
                provider: String::new(),
                how: How::Local,
                license: String::new(),
            },
            still_loading: false,
        }
    }
}

// --- Avisos (estados sem susto) ----------------------------------------------------------

/// Um estado normal do Painel que não é uma tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// Partida competitiva em andamento (a Trava de fair play está ligada).
    FairPlay,
    /// Exercício de Treino na tela.
    Training,
    /// O Provedor está desligado: oferece ligar.
    ProviderOff(ProviderId),
    /// O Explorador online exige uma Conexão autorizada: leva à Central.
    SignInRequired(ProviderId),
    /// Fonte "Um jogador" sem nome.
    NeedsPlayerName,
    /// A Posição ainda está mudando ou o pedido está a caminho.
    Loading,
    /// O Provedor não tem dados desta Posição.
    NoData,
    /// O Provedor pediu uma pausa.
    Limited { until: Timestamp },
    /// Sem internet e sem nada salvo.
    Offline,
    /// Falha que passa sozinha: "Tentar de novo".
    Failed,
    /// Finais exatos só existem até 7 peças.
    TooManyPieces { pieces: usize },
    /// Nada a mostrar: a Biblioteca não tem Partidas nesta Posição.
    EmptyLibrary,
}

/// Traduz o erro de um adaptador no aviso que o Painel mostra.
pub fn notice_for(error: &ProviderError) -> Notice {
    match error {
        ProviderError::Disabled { provider } => Notice::ProviderOff(*provider),
        ProviderError::RateLimited { until, .. } => Notice::Limited { until: *until },
        ProviderError::FairPlayLocked { .. } => Notice::FairPlay,
        ProviderError::SignInRequired { provider } => Notice::SignInRequired(*provider),
        ProviderError::Net(NetError::NotAuthorized { provider, .. }) => {
            Notice::SignInRequired(*provider)
        }
        // O Provedor recusou a autorização (token vencido ou revogado): autorizar de novo.
        ProviderError::ConnectionRejected { provider, .. } => Notice::SignInRequired(*provider),
        ProviderError::NoData => Notice::NoData,
        ProviderError::TooManyPieces { pieces, .. } => Notice::TooManyPieces { pieces: *pieces },
        ProviderError::Net(NetError::Unreachable { .. } | NetError::Timeout { .. }) => {
            Notice::Offline
        }
        _ => Notice::Failed,
    }
}

// --- Aba Finais --------------------------------------------------------------------------

/// Peças no tabuleiro (Reis incluídos) a partir do FEN.
pub fn pieces_in_fen(fen: &str) -> usize {
    fen.split_whitespace().next().map_or(0, |board| {
        board.chars().filter(char::is_ascii_alphabetic).count()
    })
}

/// Os finais exatos cobrem a Posição?
pub fn covered_by_tablebase(fen: &str) -> bool {
    pieces_in_fen(fen) <= MAX_PIECES
}

/// Resultado teórico em palavras, do ponto de vista de quem joga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outlook {
    Win,
    Draw,
    Loss,
}

/// O veredito: o resultado e a nuance da regra dos 50 Lances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub outlook: Outlook,
    pub nuance: Option<Nuance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nuance {
    /// Vitória que a regra dos 50 Lances torna empate.
    CursedWin,
    /// Derrota que a regra dos 50 Lances torna empate.
    BlessedLoss,
    /// Provável vitória ou derrota (as bases usadas não confirmam).
    Probable,
}

impl Verdict {
    pub fn of(category: Category) -> Option<Self> {
        let (outlook, nuance) = match category {
            Category::Win => (Outlook::Win, None),
            Category::CursedWin => (Outlook::Win, Some(Nuance::CursedWin)),
            Category::MaybeWin => (Outlook::Win, Some(Nuance::Probable)),
            Category::Draw => (Outlook::Draw, None),
            Category::BlessedLoss => (Outlook::Loss, Some(Nuance::BlessedLoss)),
            Category::MaybeLoss => (Outlook::Loss, Some(Nuance::Probable)),
            Category::Loss => (Outlook::Loss, None),
            Category::Unknown => return None,
        };
        Some(Self { outlook, nuance })
    }
}

/// Um Lance dos finais exatos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndgameMove {
    pub san: String,
    /// Resultado para quem joga o Lance.
    pub verdict: Option<Verdict>,
    pub dtz: Option<i32>,
    pub dtm: Option<i32>,
}

/// A aba Finais pronta para desenhar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndgameTable {
    /// De quem é a vez (o veredito é dele).
    pub side: Color,
    pub verdict: Option<Verdict>,
    pub dtz: Option<i32>,
    pub dtm: Option<i32>,
    pub checkmate: bool,
    pub stalemate: bool,
    pub insufficient_material: bool,
    /// Do melhor para o pior.
    pub moves: Vec<EndgameMove>,
    pub origin: OriginNote,
}

impl EndgameTable {
    pub fn new(answer: &Answer<Tablebase>, side: Color) -> Self {
        let table = &answer.value;
        Self {
            side,
            verdict: Verdict::of(table.category),
            dtz: table.dtz,
            dtm: table.dtm,
            checkmate: table.checkmate,
            stalemate: table.stalemate,
            insufficient_material: table.insufficient_material,
            moves: table
                .moves
                .iter()
                .map(|m| EndgameMove {
                    san: m.san.clone(),
                    verdict: Verdict::of(m.category_for_mover()),
                    dtz: m.dtz,
                    dtm: m.dtm,
                })
                .collect(),
            origin: OriginNote::from_answer(answer),
        }
    }
}

/// Mate em quantos Lances (completos) a partir da distância em meios-Lances.
pub fn mate_in_moves(dtm: i32) -> u32 {
    dtm.unsigned_abs().div_ceil(2)
}

// --- Avaliação em cache (Painel Análise) -------------------------------------------------

/// A linha principal da avaliação em cache, validada pelo núcleo de regras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudView {
    pub depth: u32,
    /// Do ponto de vista das Brancas (como o Provedor entrega).
    pub score_white: Score,
    /// Lances da linha principal em SAN, só os que o núcleo de regras aceita.
    pub san: Vec<String>,
    /// Número do lance de cada Lance da linha, para escrever "12. Cf3 Cc6".
    pub first_number: u16,
    pub white_to_move: bool,
    pub origin: OriginNote,
}

/// Converte um Lance em UCI para `Move` da Posição, aceitando o roque como "Rei captura Torre"
/// (`e1h1`), que o Lichess usa em algumas linhas. A legalidade fica com o núcleo de regras.
pub fn uci_move(position: &CanonicalPosition, uci: &str) -> Option<Move> {
    let parsed: Move = uci.parse().ok()?;
    let castle = |m: Move| -> Option<Move> {
        let (color, piece) = position.piece_at(m.from())?;
        let (target_color, target) = position.piece_at(m.to())?;
        if piece != Piece::King || target != Piece::Rook || color != target_color {
            return None;
        }
        let file = if m.to().file() > m.from().file() {
            6
        } else {
            2
        };
        let to = chess_core::Square::new(file, m.from().rank())?;
        Some(Move::new(m.from(), to, None))
    };
    [Some(parsed), castle(parsed)]
        .into_iter()
        .flatten()
        .find(|m| position.is_legal(*m))
}

/// Joga a linha UCI a partir de `base`, validando cada Lance. Para no primeiro Lance que o
/// núcleo recusa e devolve o que foi aceito, em SAN.
pub fn line_to_san(base: &CanonicalPosition, uci_moves: &[String]) -> Vec<String> {
    let mut position = base.clone();
    let mut out = Vec::new();
    for uci in uci_moves {
        let Some(mv) = uci_move(&position, uci) else {
            break;
        };
        let (Ok(san), Ok(next)) = (position.san(mv), position.play(mv)) else {
            break;
        };
        out.push(san);
        position = next;
    }
    out
}

impl CloudView {
    /// A melhor linha do `eval` para a Posição `base`.
    pub fn new(answer: &Answer<Option<CloudEval>>, base: &CanonicalPosition) -> Option<Self> {
        let eval = answer.value.as_ref()?;
        let line = eval.lines.first()?;
        Some(Self {
            depth: eval.depth,
            score_white: line.score_white,
            san: line_to_san(base, &line.moves),
            first_number: base.fullmove_number(),
            white_to_move: base.side_to_move() == Color::White,
            origin: OriginNote::from_answer(answer),
        })
    }
}

/// `+0,19`, `−1,20` (sinal de menos de verdade), `M3` (Brancas dão mate) ou `−M3`.
pub fn format_eval(score: Score) -> String {
    match score {
        Score::Centipawns(cp) => {
            let sign = match cp {
                0 => "",
                c if c > 0 => "+",
                _ => "\u{2212}",
            };
            format!(
                "{sign}{},{:02}",
                cp.unsigned_abs() / 100,
                cp.unsigned_abs() % 100
            )
        }
        Score::Mate(n) if n >= 0 => format!("M{n}"),
        Score::Mate(n) => format!("\u{2212}M{}", n.unsigned_abs()),
    }
}

/// Fração da barra ocupada pelas Brancas (0 a 1), pela mesma curva da barra de avaliação.
pub fn white_share(score: Score) -> f32 {
    match score {
        Score::Centipawns(cp) => 1.0 / (1.0 + (-(cp as f32) / 400.0).exp()),
        Score::Mate(n) if n > 0 => 1.0,
        Score::Mate(_) => 0.0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_net::{ImportMode, Provenance};

    fn answer<T>(value: T, freshness: Freshness, age_secs: u64) -> Answer<T> {
        Answer {
            value,
            provenance: Provenance {
                provider: ProviderId::Lichess,
                queried_at: Timestamp::from_millis(1_794_398_400_000),
                license: "CC0".into(),
                mode: ImportMode::Api,
                source: "https://explorer.lichess.org/masters".into(),
            },
            freshness,
            age: Duration::from_secs(age_secs),
        }
    }

    #[test]
    fn counts_use_the_brazilian_thousands_separator() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(1_000), "1.000");
        assert_eq!(fmt_count(1_234_567), "1.234.567");
    }

    #[test]
    fn percentages_sum_to_one_hundred() {
        assert_eq!(percentages(1, 1, 1), (33, 33, 34));
        assert_eq!(percentages(2, 0, 0), (100, 0, 0));
        assert_eq!(percentages(0, 0, 0), (0, 0, 0));
        let (w, d, b) = percentages(7, 2, 5);
        assert_eq!(w + d + b, 100);
    }

    #[test]
    fn ages_are_bucketed_in_words() {
        assert_eq!(age_of(Duration::from_secs(20)), Age::JustNow);
        assert_eq!(age_of(Duration::from_secs(5 * 60 + 3)), Age::Minutes(5));
        assert_eq!(age_of(Duration::from_secs(3 * 3600)), Age::Hours(3));
        assert_eq!(age_of(Duration::from_secs(2 * 86_400)), Age::Days(2));
    }

    #[test]
    fn dates_and_waits_are_readable() {
        let at = Timestamp::from_millis(1_794_398_400_000); // 2026-11-11T12:00Z
        assert_eq!(day_month(at), "11/11");
        assert_eq!(clock_utc(at), "12:00 UTC");
        let until = Timestamp::from_millis(at.as_millis() + 90_000);
        assert_eq!(minutes_until(at, until), 2);
        assert_eq!(minutes_until(until, at), 1);
    }

    #[test]
    fn pieces_are_counted_from_the_board_field() {
        assert_eq!(
            pieces_in_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            32
        );
        assert_eq!(pieces_in_fen("8/8/4k3/8/8/4KP2/6R1/7r w - - 0 1"), 5);
        assert!(covered_by_tablebase("8/8/4k3/8/8/4KP2/6R1/7r w - - 0 1"));
        // Sete peças ainda cabem; oito não.
        assert!(covered_by_tablebase(
            "8/2k5/3p4/8/3pP3/8/5K2/R5R1 w - - 0 1"
        ));
        assert!(!covered_by_tablebase(
            "8/2k5/3p4/8/3pP3/5n2/5K2/R5R1 w - - 0 1"
        ));
    }

    #[test]
    fn verdicts_carry_the_fifty_move_nuance() {
        assert_eq!(
            Verdict::of(Category::CursedWin),
            Some(Verdict {
                outlook: Outlook::Win,
                nuance: Some(Nuance::CursedWin)
            })
        );
        assert_eq!(
            Verdict::of(Category::BlessedLoss).unwrap().outlook,
            Outlook::Loss
        );
        assert_eq!(Verdict::of(Category::Unknown), None);
        assert_eq!(mate_in_moves(17), 9);
        assert_eq!(mate_in_moves(-16), 8);
    }

    #[test]
    fn errors_become_calm_notices() {
        let lichess = ProviderId::Lichess;
        assert_eq!(
            notice_for(&ProviderError::SignInRequired { provider: lichess }),
            Notice::SignInRequired(lichess)
        );
        assert_eq!(
            notice_for(&ProviderError::Net(NetError::NotAuthorized {
                provider: lichess,
                capability: chess_net::Capability::Explore
            })),
            Notice::SignInRequired(lichess)
        );
        assert_eq!(notice_for(&ProviderError::NoData), Notice::NoData);
        assert_eq!(
            notice_for(&ProviderError::Disabled { provider: lichess }),
            Notice::ProviderOff(lichess)
        );
        assert_eq!(
            notice_for(&ProviderError::FairPlayLocked { provider: lichess }),
            Notice::FairPlay
        );
        assert_eq!(
            notice_for(&ProviderError::Net(NetError::Unreachable {
                provider: lichess,
                detail: String::new()
            })),
            Notice::Offline
        );
    }

    #[test]
    fn origin_says_how_fresh_the_data_is() {
        let fresh = OriginNote::from_answer(&answer((), Freshness::Network, 0));
        assert_eq!(fresh.how, How::Queried(Age::JustNow));
        assert_eq!(fresh.provider, "Lichess");
        let cached = OriginNote::from_answer(&answer((), Freshness::Cache, 3600));
        assert!(matches!(cached.how, How::Cached(_)));
        let stale = OriginNote::from_answer(&answer(
            (),
            Freshness::Stale {
                cause: NetError::Cancelled,
            },
            9000,
        ));
        assert!(stale.is_stale());
    }

    #[test]
    fn the_library_table_sums_the_moves() {
        let stats = [
            OpeningMoveStats {
                san: "e4".into(),
                games: 6,
                white_wins: 3,
                draws: 2,
                black_wins: 1,
                unfinished: 0,
                last_used_ms: 0,
            },
            OpeningMoveStats {
                san: "d4".into(),
                games: 4,
                white_wins: 1,
                draws: 1,
                black_wins: 2,
                unfinished: 0,
                last_used_ms: 0,
            },
        ];
        let table = OpeningsTable::from_library(&stats, None);
        assert_eq!(
            (table.total, table.white, table.draws, table.black),
            (10, 4, 3, 3)
        );
        assert_eq!(table.origin.how, How::Local);
        assert!(!table.has_ratings());
        assert_eq!(table.rows[0].percentages(), (51, 33, 16));
    }

    #[test]
    fn evaluations_are_formatted_with_a_comma_and_a_real_minus() {
        assert_eq!(format_eval(Score::Centipawns(19)), "+0,19");
        assert_eq!(format_eval(Score::Centipawns(-310)), "\u{2212}3,10");
        assert_eq!(format_eval(Score::Centipawns(0)), "0,00");
        assert_eq!(format_eval(Score::Mate(3)), "M3");
        assert_eq!(format_eval(Score::Mate(-2)), "\u{2212}M2");
        assert!(white_share(Score::Centipawns(300)) > 0.6);
        assert_eq!(white_share(Score::Mate(-1)), 0.0);
    }

    #[test]
    fn a_cloud_line_is_validated_by_the_rules_core_and_accepts_king_takes_rook_castling() {
        let start = CanonicalPosition::start();
        // O Lichess escreveu o roque curto das Brancas como e1h1 (Rei captura Torre).
        let line: Vec<String> = "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 d2d3 f8c5 e1h1 d7d6"
            .split(' ')
            .map(str::to_owned)
            .collect();
        let san = line_to_san(&start, &line);
        assert_eq!(san.len(), 10, "{san:?}");
        assert_eq!(san[8], "O-O");
        // Um Lance ilegal corta a linha ali, sem pânico.
        let broken: Vec<String> = ["e2e4", "e7e5", "e1e8", "g1f3"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(line_to_san(&start, &broken), ["e4", "e5"]);
        assert!(line_to_san(&start, &["a1a8".to_owned()]).is_empty());
    }

    #[test]
    fn black_to_move_numbers_continue_the_game_numbering() {
        let base = CanonicalPosition::from_fen(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        )
        .unwrap();
        let eval = CloudEval {
            fen: base.to_fen(),
            depth: 40,
            nodes: 0,
            lines: vec![chess_providers::EvalLine {
                moves: vec!["c7c5".into(), "g1f3".into()],
                score_white: Score::Centipawns(24),
            }],
        };
        let view = CloudView::new(&answer(Some(eval), Freshness::Network, 0), &base).unwrap();
        assert_eq!(view.san, ["c5", "Nf3"]);
        assert!(!view.white_to_move);
        assert_eq!(view.first_number, 1);
        // «Sem avaliação em cache» não produz linha.
        assert!(CloudView::new(&answer(None, Freshness::Network, 0), &base).is_none());
    }
}
