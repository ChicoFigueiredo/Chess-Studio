//! Painel Peças capturadas: material perdido por cada lado e o saldo, **derivados** do
//! `Snapshot` (nunca guardados). Leitura pura, sem Comando de Partida (ADR-0001).
//!
//! A conta é exata mesmo com promoções: peças de um tipo capturadas =
//! (as da Posição inicial) + (promoções para esse tipo, lidas do histórico) − (as de agora);
//! peões capturados = (peões iniciais) − (promoções) − (peões de agora).

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::{CanonicalPosition, Color as Side, Piece};
use chess_game::HistoryEntry;
use chess_input::all_squares;

/// Tipos contados, do menor para o maior valor (o rei nunca é capturado).
pub const KINDS: [Piece; 5] = [
    Piece::Pawn,
    Piece::Knight,
    Piece::Bishop,
    Piece::Rook,
    Piece::Queen,
];

/// Valor de material de um tipo (convenção 1-3-3-5-9).
pub fn value(piece: Piece) -> u32 {
    match piece {
        Piece::Pawn => 1,
        Piece::Knight | Piece::Bishop => 3,
        Piece::Rook => 5,
        Piece::Queen => 9,
        Piece::King => 0,
    }
}

fn index(piece: Piece) -> Option<usize> {
    KINDS.iter().position(|k| *k == piece)
}

/// Contagem por tipo (`KINDS`) para cada lado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub white: [u8; 5],
    pub black: [u8; 5],
}

impl Counts {
    pub fn of(&self, side: Side) -> &[u8; 5] {
        match side {
            Side::White => &self.white,
            Side::Black => &self.black,
        }
    }

    fn of_mut(&mut self, side: Side) -> &mut [u8; 5] {
        match side {
            Side::White => &mut self.white,
            Side::Black => &mut self.black,
        }
    }

    /// Material (1-3-3-5-9) do lado.
    pub fn material(&self, side: Side) -> u32 {
        self.of(side)
            .iter()
            .zip(KINDS)
            .map(|(n, k)| u32::from(*n) * value(k))
            .sum()
    }
}

/// Peças no tabuleiro por tipo.
pub fn on_board(position: &CanonicalPosition) -> Counts {
    let mut counts = Counts::default();
    for square in all_squares() {
        if let Some((side, piece)) = position.piece_at(square)
            && let Some(i) = index(piece)
        {
            counts.of_mut(side)[i] += 1;
        }
    }
    counts
}

/// Peças que cada lado **perdeu** (foram capturadas) desde `start`, dadas as de agora e o
/// histórico até aqui (para contar as promoções).
pub fn captured(
    start: &CanonicalPosition,
    now: &CanonicalPosition,
    history: &[HistoryEntry],
) -> Counts {
    let (start, now) = (on_board(start), on_board(now));
    let mut promoted = Counts::default();
    for entry in history {
        if let Some(i) = entry.mv.promotion().and_then(index) {
            promoted.of_mut(entry.side)[i] += 1;
        }
    }
    let mut lost = Counts::default();
    for side in [Side::White, Side::Black] {
        let total_promoted: u8 = promoted.of(side).iter().sum();
        for (i, kind) in KINDS.iter().enumerate() {
            let before = i16::from(start.of(side)[i]);
            let after = i16::from(now.of(side)[i]);
            let gained = i16::from(promoted.of(side)[i]);
            let missing = if *kind == Piece::Pawn {
                before - i16::from(total_promoted) - after
            } else {
                before + gained - after
            };
            lost.of_mut(side)[i] = u8::try_from(missing.max(0)).unwrap_or(0);
        }
    }
    lost
}

/// Saldo de material no tabuleiro: positivo = brancas à frente.
pub fn balance(now: &Counts) -> i32 {
    i32::try_from(now.material(Side::White)).unwrap_or(0)
        - i32::try_from(now.material(Side::Black)).unwrap_or(0)
}

/// Peças de uma contagem (`KINDS`), da de maior valor para a de menor: é a ordem em que o
/// Painel as desenha. Entre as de mesmo valor, o Bispo vem antes do Cavalo.
pub fn pieces_by_value(counts: &[u8; 5]) -> Vec<Piece> {
    let mut out = Vec::new();
    for (i, kind) in KINDS.iter().enumerate().rev() {
        out.extend(std::iter::repeat_n(*kind, usize::from(counts[i])));
    }
    out
}

/// A linha de quem capturou: as peças que levou e a vantagem de material (0 se não lidera).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedRow {
    pub captor: Side,
    /// Peças do **adversário** que `captor` capturou, da mais valiosa para a menos.
    pub pieces: Vec<Piece>,
    pub lead: u32,
}

/// As duas linhas do Painel (Brancas, Pretas): peças capturadas por lado, em ordem de valor,
/// e o saldo (+N) só na linha de quem está à frente no tabuleiro.
pub fn rows(lost: &Counts, board: &Counts) -> [CapturedRow; 2] {
    let diff = balance(board);
    let lead = |side: Side| {
        let n = if side == Side::White { diff } else { -diff };
        u32::try_from(n.max(0)).unwrap_or(0)
    };
    [Side::White, Side::Black].map(|captor| CapturedRow {
        captor,
        pieces: pieces_by_value(lost.of(captor.opposite())),
        lead: lead(captor),
    })
}

/// Como o Painel mostra as peças capturadas (Opções > Tabuleiro).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapturedStyle {
    /// Figuras das peças (padrão).
    #[default]
    Pieces,
    /// O texto "Torre ×1 · Bispo ×1…".
    Text,
}

impl CapturedStyle {
    pub fn is_pieces(self) -> bool {
        self == Self::Pieces
    }
}

fn piece_key(piece: Piece) -> &'static str {
    match piece {
        Piece::Pawn => "captured-pawn",
        Piece::Knight => "captured-knight",
        Piece::Bishop => "captured-bishop",
        Piece::Rook => "captured-rook",
        Piece::Queen => "captured-queen",
        Piece::King => "captured-king",
    }
}

fn list_text(i18n: &I18n, lost: &[u8; 5]) -> String {
    let parts: Vec<String> = KINDS
        .iter()
        .enumerate()
        .rev()
        .filter(|(i, _)| lost[*i] > 0)
        .map(|(i, k)| format!("{} ×{}", i18n.t(piece_key(*k)), lost[i]))
        .collect();
    if parts.is_empty() {
        i18n.t("captured-none")
    } else {
        parts.join(" · ")
    }
}

const GLYPH: f32 = 30.0;
const STEP: f32 = 22.0;
/// Largura reservada ao saldo "+N" ao lado das figuras.
const LEAD_WIDTH: f32 = 36.0;

/// Desenha as figuras de uma linha (as mesmas do Tabuleiro 2D), quebrando quando falta
/// largura, com o saldo "+N" ao lado para quem lidera.
fn paint_row(ui: &mut egui::Ui, i18n: &I18n, row: &CapturedRow) {
    if row.pieces.is_empty() {
        ui.small(i18n.t("captured-none"));
        return;
    }
    let victim = row.captor.opposite();
    let reserved = if row.lead > 0 { LEAD_WIDTH } else { 0.0 };
    let per_line = (((ui.available_width() - reserved) / STEP) as usize).max(1);
    let lines = row.pieces.len().div_ceil(per_line);
    let width = STEP * per_line.min(row.pieces.len()) as f32 + GLYPH - STEP;
    ui.horizontal_top(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(width, GLYPH * lines as f32),
            egui::Sense::hover(),
        );
        for (i, piece) in row.pieces.iter().enumerate() {
            let (col, line) = (i % per_line, i / per_line);
            let min = response.rect.min + egui::vec2(STEP * col as f32, GLYPH * line as f32);
            let cell = egui::Rect::from_min_size(min, egui::vec2(GLYPH, GLYPH));
            crate::board2d_panel::paint_piece(&painter, cell, victim, *piece, 1.0, None);
        }
        if row.lead > 0 {
            ui.label(egui::RichText::new(format!("+{}", row.lead)).strong());
        }
    });
}

fn captured_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let Some(session) = world.get_resource::<Session>() else {
        return;
    };
    // Na revisão, a posição e o histórico são os do Lance escolhido.
    let (now, plies) = match session.review() {
        Some(r) => (r.position.clone(), r.ply),
        None => (
            session.snapshot().position.clone(),
            session.snapshot().history.len(),
        ),
    };
    let start = session
        .game()
        .review_at_ply(0)
        .map_or_else(CanonicalPosition::start, |v| v.position);
    let history = &session.snapshot().history[..plies.min(session.snapshot().history.len())];
    let lost = captured(&start, &now, history);
    let board = on_board(&now);

    let style = world
        .get_resource::<CapturedStyle>()
        .copied()
        .unwrap_or_default();
    match style {
        CapturedStyle::Text => {
            ui.strong(i18n.t("captured-by-white"));
            ui.label(list_text(i18n, &lost.black));
            ui.strong(i18n.t("captured-by-black"));
            ui.label(list_text(i18n, &lost.white));
        }
        CapturedStyle::Pieces => {
            for row in rows(&lost, &board) {
                let key = if row.captor == Side::White {
                    "captured-by-white"
                } else {
                    "captured-by-black"
                };
                ui.strong(i18n.t(key));
                paint_row(ui, i18n, &row);
            }
        }
    }
    ui.separator();
    let diff = balance(&board);
    let text = match diff.cmp(&0) {
        std::cmp::Ordering::Greater => {
            i18n.t_with("captured-balance-white", &[("n", &diff.to_string())])
        }
        std::cmp::Ordering::Less => {
            i18n.t_with("captured-balance-black", &[("n", &(-diff).to_string())])
        }
        std::cmp::Ordering::Equal => i18n.t("captured-balance-even"),
    };
    ui.label(text);
    ui.small(i18n.t_with(
        "captured-material",
        &[
            ("white", &board.material(Side::White).to_string()),
            ("black", &board.material(Side::Black).to_string()),
        ],
    ));
}

pub struct CapturedPlugin;

impl Plugin for CapturedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CapturedStyle>();
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::CAPTURED,
                label_key: "dock-panel-captured",
                access: PanelAccess::Always,
                ui: captured_panel_ui,
            },
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn play(game: &mut Game, uci: &str, at: u64) {
        let from = uci[0..2].parse().unwrap();
        let to = uci[2..4].parse().unwrap();
        game.handle(Command::Release { from, to }, Timestamp::mono(at))
            .unwrap();
        if let Some(p) = uci.chars().nth(4) {
            let piece = match p {
                'q' => Piece::Queen,
                'r' => Piece::Rook,
                'b' => Piece::Bishop,
                _ => Piece::Knight,
            };
            game.handle(Command::ChoosePromotion { piece }, Timestamp::mono(at + 1))
                .unwrap();
        }
    }

    fn lost_after(moves: &[&str]) -> (Counts, i32) {
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        for (i, m) in moves.iter().enumerate() {
            play(&mut game, m, 10 * (i as u64 + 1));
        }
        let snapshot = game.snapshot();
        let start = CanonicalPosition::start();
        (
            captured(&start, &snapshot.position, &snapshot.history),
            balance(&on_board(&snapshot.position)),
        )
    }

    #[test]
    fn pieces_are_listed_by_value_from_the_most_valuable() {
        let lost = [3, 1, 2, 1, 1];
        assert_eq!(
            pieces_by_value(&lost),
            [
                Piece::Queen,
                Piece::Rook,
                Piece::Bishop,
                Piece::Bishop,
                Piece::Knight,
                Piece::Pawn,
                Piece::Pawn,
                Piece::Pawn
            ]
        );
        assert!(pieces_by_value(&[0; 5]).is_empty());
    }

    #[test]
    fn each_row_shows_what_its_side_took_and_only_the_leader_gets_the_balance() {
        // 1. e4 d5 2. exd5 Qxd5: cada lado levou um peão; brancas +0.
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        for (i, m) in ["e2e4", "d7d5", "e4d5", "d8d5"].iter().enumerate() {
            play(&mut game, m, 10 * (i as u64 + 1));
        }
        let snapshot = game.snapshot();
        let start = CanonicalPosition::start();
        let lost = captured(&start, &snapshot.position, &snapshot.history);
        let board = on_board(&snapshot.position);
        let [white, black] = rows(&lost, &board);
        assert_eq!(white.captor, Side::White);
        assert_eq!(white.pieces, [Piece::Pawn]);
        assert_eq!(black.pieces, [Piece::Pawn]);
        assert_eq!((white.lead, black.lead), (0, 0), "material igual");

        // Mais uma captura das brancas: Nxd5 vira... aqui, Nc3 e Nxd5 tomam a dama.
        play(&mut game, "b1c3", 50);
        play(&mut game, "c8e6", 60);
        play(&mut game, "c3d5", 70);
        let snapshot = game.snapshot();
        let lost = captured(&start, &snapshot.position, &snapshot.history);
        let board = on_board(&snapshot.position);
        let [white, black] = rows(&lost, &board);
        assert_eq!(white.pieces, [Piece::Queen, Piece::Pawn]);
        assert_eq!(white.lead, 9);
        assert_eq!(black.lead, 0, "só quem está à frente mostra o +N");
    }

    #[test]
    fn the_start_position_has_nothing_captured_and_even_material() {
        let (lost, diff) = lost_after(&[]);
        assert_eq!(lost, Counts::default());
        assert_eq!(diff, 0);
        assert_eq!(
            on_board(&CanonicalPosition::start()).material(Side::White),
            39
        );
    }

    #[test]
    fn a_capture_shows_the_lost_piece_and_the_balance() {
        // 1. e4 d5 2. exd5: as pretas perderam um peão.
        let (lost, diff) = lost_after(&["e2e4", "d7d5", "e4d5"]);
        assert_eq!(lost.black, [1, 0, 0, 0, 0]);
        assert_eq!(lost.white, [0; 5]);
        assert_eq!(diff, 1, "brancas +1");
    }

    #[test]
    fn en_passant_counts_the_captured_pawn() {
        let (lost, diff) = lost_after(&["e2e4", "a7a6", "e4e5", "d7d5", "e5d6"]);
        assert_eq!(lost.black, [1, 0, 0, 0, 0]);
        assert_eq!(diff, 1);
    }

    #[test]
    fn a_promotion_does_not_read_as_a_captured_pawn() {
        // Brancas promovem a dama sem capturar: nada capturado, mas o saldo cresce.
        let mut game = Game::new(
            GameSetup::from_fen(PolicyKind::Assisted, "4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap(),
        );
        let start = game.snapshot().position.clone();
        play(&mut game, "a7a8q", 10);
        let snapshot = game.snapshot();
        let lost = captured(&start, &snapshot.position, &snapshot.history);
        assert_eq!(lost, Counts::default());
        assert_eq!(balance(&on_board(&snapshot.position)), 9);
    }

    #[test]
    fn capturing_a_promoted_piece_counts_as_that_piece() {
        // axb8=Q+ toma a torre e promove; o rei preto toma a dama promovida.
        let mut game = Game::new(
            GameSetup::from_fen(PolicyKind::Assisted, "1rk5/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap(),
        );
        let start = game.snapshot().position.clone();
        play(&mut game, "a7b8q", 10);
        play(&mut game, "c8b8", 20);
        let snapshot = game.snapshot();
        let lost = captured(&start, &snapshot.position, &snapshot.history);
        assert_eq!(lost.black, [0, 0, 0, 1, 0], "a torre");
        assert_eq!(lost.white, [0, 0, 0, 0, 1], "a dama promovida, não um peão");
    }
}
