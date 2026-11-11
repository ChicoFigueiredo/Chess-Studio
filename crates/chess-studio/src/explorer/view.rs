//! Desenho do Painel Explorador e da linha "Em cache" da Análise. Só pinta o que `model` e
//! `driver` entregaram, com os tokens do Tema e os widgets do projeto (README do design
//! system); nada de cor escrita à mão, nada de rede.

use super::driver::CloudRow;
use super::model::{
    Age, CloudView, EndgameMove, EndgameTable, How, Notice, Nuance, OpeningsTable, OriginNote,
    Outlook, ReferenceRow, Source, Verdict, clock_local, day_month, fmt_count, format_eval,
    mate_in_moves, minutes_until, white_share,
};
use super::reference::Status;
use crate::i18n::I18n;
use crate::local_time::LocalClock;
use crate::net_service::NetService;
use crate::panel_chrome::radius;
use crate::ui_theme::{
    ButtonVariant, CsButton, Palette, Tabular, cs_button, icon_font, palette, strong_font,
};
use bevy_egui::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};
use chess_core::Color;
use chess_net::{ProviderId, Timestamp};
use egui_phosphor::regular as icon;

/// Altura de uma linha de tabela (grade de 4 px).
pub const ROW_H: f32 = 28.0;
/// Recuo lateral das linhas de tabela.
const PAD: f32 = 8.0;

/// O que a pessoa pediu ao Painel neste quadro (aplicado depois de desenhar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    /// Mostrar o Lance na análise (variante somente leitura).
    Play(String),
    /// Abrir o diálogo "Ligar <Provedor>?".
    TurnOn(ProviderId),
    /// Abrir a Central de Conexões.
    OpenConnections,
    /// Abrir o Painel Biblioteca.
    OpenLibrary,
    /// Abrir a Partida de referência exata (pelo id do Provedor): a da Biblioteca, se já
    /// está lá; senão baixada e gravada com a Origem.
    OpenReference { id: String },
    /// Tentar de novo a consulta que falhou.
    Retry,
    /// Voltar da variante para a Partida.
    BackToGame,
    /// Trocar para a fonte local (atalho do aviso de autorização).
    UseLibrary,
}

fn caption(ui: &Ui) -> FontId {
    egui::TextStyle::Small.resolve(ui.style())
}

fn body(ui: &Ui) -> FontId {
    egui::TextStyle::Body.resolve(ui.style())
}

/// Texto pequeno e discreto (`caption` em `ink-muted`).
pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    let p = palette(ui);
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(p.ink_muted)).wrap());
}

// --- Barra de resultado --------------------------------------------------------------

/// Cores do resultado: Brancas, empates e Pretas (as peças não mudam com o Tema).
struct BarColors {
    white: Color32,
    draws: Color32,
    black: Color32,
}

fn bar_colors(p: &Palette) -> BarColors {
    BarColors {
        white: p.eval_white,
        draws: p.eval_white.lerp_to_gamma(p.eval_black, 0.5),
        black: p.eval_black,
    }
}

/// Texto legível sobre `fill`: o de maior contraste entre as duas cores das peças.
fn ink_on(p: &Palette, fill: Color32) -> Color32 {
    // Luminância relativa (WCAG): o texto escuro vai sobre fundo claro e vice-versa.
    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }
    let ratio = |a: Color32, b: Color32| {
        let (hi, lo) = (
            luminance(a).max(luminance(b)),
            luminance(a).min(luminance(b)),
        );
        (hi + 0.05) / (lo + 0.05)
    };
    if ratio(p.eval_black, fill) >= ratio(p.eval_white, fill) {
        p.eval_black
    } else {
        p.eval_white
    }
}

/// Barra Brancas | Empates | Pretas com a porcentagem dentro de cada fatia que cabe.
fn result_bar(ui: &Ui, rect: Rect, shares: (f32, f32, f32), pct: (u32, u32, u32), decided: bool) {
    let p = palette(ui);
    let painter = ui.painter_at(rect);
    let corner = radius("radius-s");
    if !decided {
        painter.rect(
            rect,
            corner,
            p.surface_3,
            Stroke::new(1.0, p.border),
            StrokeKind::Inside,
        );
        return;
    }
    let colors = bar_colors(p);
    let font = FontId::proportional(12.0);
    let mut x = rect.left();
    let parts = [
        (shares.0, pct.0, colors.white),
        (shares.1, pct.1, colors.draws),
        (shares.2, pct.2, colors.black),
    ];
    let last = parts.len() - 1;
    for (i, (share, percent, fill)) in parts.into_iter().enumerate() {
        // A última fatia fecha a barra: nada sobra por arredondamento.
        let width = if i == last {
            rect.right() - x
        } else {
            rect.width() * share
        };
        if width <= 0.0 {
            continue;
        }
        let seg = Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, rect.height()));
        painter.rect_filled(seg, 0.0, fill);
        if width >= 30.0 {
            let text = Tabular::new(&painter, &format!("{percent}%"), &font, ink_on(p, fill));
            let pos = Align2::CENTER_CENTER
                .align_size_within_rect(text.size, seg)
                .min;
            text.paint(&painter, pos, ink_on(p, fill));
        }
        x += width;
    }
    // Contorno por cima, para a barra não sumir sobre uma superfície parecida.
    painter.rect_stroke(
        rect,
        corner,
        Stroke::new(1.0, p.border_strong),
        StrokeKind::Inside,
    );
}

/// Legenda da barra: três quadrados com o nome de cada fatia.
fn legend(ui: &mut Ui, i18n: &I18n) {
    let p = palette(ui);
    let colors = bar_colors(p);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (key, fill) in [
            ("explorer-legend-white", colors.white),
            ("explorer-legend-draws", colors.draws),
            ("explorer-legend-black", colors.black),
        ] {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
            ui.painter().rect(
                rect.shrink(1.0),
                radius("radius-s"),
                fill,
                Stroke::new(1.0, p.border_strong),
                StrokeKind::Inside,
            );
            muted(ui, i18n.t(key));
            ui.add_space(8.0);
        }
    });
}

fn bar_tip(i18n: &I18n, pct: (u32, u32, u32), decided: u64) -> String {
    i18n.t_with(
        "explorer-bar-tip",
        &[
            ("white", &pct.0.to_string()),
            ("draw", &pct.1.to_string()),
            ("black", &pct.2.to_string()),
            ("decided", &fmt_count(decided)),
        ],
    )
}

// --- Aba Aberturas -------------------------------------------------------------------

fn games_text(i18n: &I18n, n: u64) -> String {
    let key = if n == 1 {
        "explorer-games-one"
    } else {
        "explorer-games-other"
    };
    i18n.t_with(key, &[("count", &fmt_count(n))])
}

/// Colunas da tabela: onde cada coisa fica dentro da linha.
struct Columns {
    san_x: f32,
    games_right: f32,
    bar: Rect,
    rating_right: Option<f32>,
}

fn columns(rect: Rect, has_rating: bool) -> Columns {
    let right = rect.right() - PAD;
    let rating_right = has_rating.then_some(right);
    let bar_right = if has_rating {
        right - 44.0 - 8.0
    } else {
        right
    };
    let san_x = rect.left() + PAD;
    let games_right = san_x + 52.0 + 56.0;
    let bar_left = games_right + 12.0;
    let bar = Rect::from_min_max(
        egui::pos2(bar_left, rect.center().y - 8.0),
        egui::pos2(bar_right.max(bar_left + 48.0), rect.center().y + 8.0),
    );
    Columns {
        san_x,
        games_right,
        bar,
        rating_right,
    }
}

fn header_row(ui: &mut Ui, i18n: &I18n, has_rating: bool) {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 24.0), Sense::hover());
    let cols = columns(rect, has_rating);
    let painter = ui.painter_at(rect);
    let font = caption(ui);
    let y = rect.center().y;
    painter.text(
        egui::pos2(cols.san_x, y),
        Align2::LEFT_CENTER,
        i18n.t("explorer-col-move"),
        font.clone(),
        p.ink_muted,
    );
    painter.text(
        egui::pos2(cols.games_right, y),
        Align2::RIGHT_CENTER,
        i18n.t("explorer-col-games"),
        font.clone(),
        p.ink_muted,
    );
    painter.text(
        egui::pos2(cols.bar.left(), y),
        Align2::LEFT_CENTER,
        i18n.t("explorer-col-result"),
        font.clone(),
        p.ink_muted,
    );
    if let Some(right) = cols.rating_right {
        painter.text(
            egui::pos2(right, y),
            Align2::RIGHT_CENTER,
            i18n.t("explorer-col-rating"),
            font,
            p.ink_muted,
        );
    }
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, p.border),
    );
}

/// A tabela de Lances e a barra-resumo da Posição.
pub fn openings_table(
    ui: &mut Ui,
    i18n: &I18n,
    table: &OpeningsTable,
    selected: Option<&str>,
    reference: &Status,
    actions: &mut Vec<UiAction>,
) {
    let p = palette(ui);
    // Cabeçalho da Posição: abertura e total de Partidas.
    match &table.opening {
        Some((eco, name)) => {
            let label = ui.label(
                egui::RichText::new(opening_label(i18n, eco, name))
                    .font(strong_font(ui.ctx(), 14.0)),
            );
            // O nome original (em inglês) fica na dica, para quem quiser procurá-lo por aí.
            if chess_openings::name_pt_br(name) != *name {
                label.on_hover_text(i18n.t_with("explorer-opening-original", &[("name", name)]));
            }
        }
        None => muted(ui, i18n.t("explorer-opening-unnamed")),
    }
    muted(ui, games_text(i18n, table.total));
    ui.add_space(4.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
    let decided = table.white + table.draws + table.black;
    let shares = if decided == 0 {
        (0.0, 0.0, 0.0)
    } else {
        let f = |n: u64| n as f32 / decided as f32;
        (f(table.white), f(table.draws), f(table.black))
    };
    let pct = super::model::percentages(table.white, table.draws, table.black);
    result_bar(ui, rect, shares, pct, decided > 0);
    response.on_hover_text(bar_tip(i18n, pct, decided));
    ui.add_space(4.0);
    legend(ui, i18n);
    ui.add_space(8.0);

    let has_rating = table.has_ratings();
    header_row(ui, i18n, has_rating);
    for row in &table.rows {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW_H), Sense::click());
        let is_selected = selected == Some(row.san.as_str());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, row.san.clone())
        });
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let fill = if is_selected {
                p.accent_subtle
            } else if response.hovered() {
                p.surface_3
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(rect, radius("radius-s"), fill);
            let cols = columns(rect, has_rating);
            let y = rect.center().y;
            let ink = if is_selected { p.accent_ink } else { p.ink };
            painter.text(
                egui::pos2(cols.san_x, y),
                Align2::LEFT_CENTER,
                &row.san,
                strong_font(ui.ctx(), 14.0),
                ink,
            );
            let games = Tabular::new(&painter, &fmt_count(row.games), &body(ui), p.ink);
            games.paint(
                &painter,
                egui::pos2(cols.games_right - games.size.x, y - games.size.y / 2.0),
                p.ink,
            );
            let pct = row.percentages();
            result_bar(ui, cols.bar, row.shares(), pct, row.decided() > 0);
            if let (Some(right), Some(rating)) = (cols.rating_right, row.rating) {
                let text = Tabular::new(&painter, &rating.to_string(), &body(ui), p.ink_muted);
                text.paint(
                    &painter,
                    egui::pos2(right - text.size.x, y - text.size.y / 2.0),
                    p.ink_muted,
                );
            }
            if response.has_focus() {
                painter.rect_stroke(
                    rect.shrink(1.0),
                    radius("radius-s"),
                    Stroke::new(2.0, p.focus_ring),
                    StrokeKind::Inside,
                );
            }
        }
        if response.clicked() {
            actions.push(UiAction::Play(row.san.clone()));
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "{} · {}",
                row.san,
                bar_tip(i18n, row.percentages(), row.decided())
            ));
    }
    if table.still_loading {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spinner();
            muted(ui, i18n.t("explorer-more-coming"));
        });
    }
    if !table.references.is_empty() {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(i18n.t("explorer-references")).font(strong_font(ui.ctx(), 14.0)),
        );
        ui.add_space(4.0);
        for game in &table.references {
            reference_row(ui, i18n, game, reference, actions);
        }
    }
}

/// Uma linha de texto que termina em reticências se não couber em `width`.
fn ellipsized(
    painter: &egui::Painter,
    text: String,
    font: FontId,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text, font, color, width.max(24.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    painter.layout_job(job)
}

/// "Brancas 2750 · Pretas 2700 · 2019": o que a Partida de referência tem de contexto.
fn reference_meta(i18n: &I18n, game: &ReferenceRow) -> String {
    let mut parts = Vec::new();
    if let Some(r) = game.white_rating {
        parts.push(i18n.t_with("explorer-reference-white", &[("rating", &r.to_string())]));
    }
    if let Some(r) = game.black_rating {
        parts.push(i18n.t_with("explorer-reference-black", &[("rating", &r.to_string())]));
    }
    if let Some(year) = game.year {
        parts.push(year.to_string());
    }
    parts.join(" \u{b7} ")
}

/// "C60 · Ruy Lopez: Defesa Morphy": o nome da abertura em português (regras em
/// `chess-openings`), com o código ECO.
pub fn opening_label(i18n: &I18n, eco: &str, name: &str) -> String {
    i18n.t_with(
        "explorer-opening",
        &[("eco", eco), ("name", &chess_openings::name_pt_br(name))],
    )
}

fn reference_row(
    ui: &mut Ui,
    i18n: &I18n,
    game: &ReferenceRow,
    reference: &Status,
    actions: &mut Vec<UiAction>,
) {
    let p = palette(ui);
    // Abrindo esta Partida, ou não deu certo: a linha de baixo diz, sem esconder os jogadores.
    let (meta_text, meta_color) = match reference {
        Status::Opening(id) if *id == game.id => (i18n.t("explorer-reference-opening"), p.accent),
        Status::Failed { id, key } if *id == game.id => (i18n.t(key), p.warning),
        _ => (reference_meta(i18n, game), p.ink_muted),
    };
    let busy = matches!(reference, Status::Opening(_));
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        if response.hovered() {
            painter.rect_filled(rect, radius("radius-s"), p.surface_3);
        }
        let left = rect.left() + PAD;
        let right = rect.right() - PAD;
        let top = rect.top() + 6.0;
        let result = match game.winner {
            Some(Color::White) => "1-0",
            Some(Color::Black) => "0-1",
            None => "\u{bd}-\u{bd}",
        };
        let result_text = Tabular::new(&painter, result, &strong_font(ui.ctx(), 14.0), p.ink);
        result_text.paint(&painter, egui::pos2(right - result_text.size.x, top), p.ink);
        // Os nomes ocupam o que sobra da primeira linha, com reticências.
        let names = format!("{} \u{2013} {}", game.white, game.black);
        let galley = ellipsized(
            &painter,
            names,
            body(ui),
            p.ink,
            right - left - result_text.size.x - 8.0,
        );
        painter.galley(egui::pos2(left, top), galley, p.ink);
        let meta = ellipsized(&painter, meta_text, caption(ui), meta_color, right - left);
        painter.galley(egui::pos2(left, top + 20.0), meta, meta_color);
        if response.has_focus() {
            painter.rect_stroke(
                rect.shrink(1.0),
                radius("radius-s"),
                Stroke::new(2.0, p.focus_ring),
                StrokeKind::Inside,
            );
        }
    }
    if response.clicked() && !busy {
        actions.push(UiAction::OpenReference {
            id: game.id.clone(),
        });
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(i18n.t("explorer-reference-tip"));
}

// --- Aba Finais ----------------------------------------------------------------------

fn outlook_style(p: &Palette, outlook: Outlook) -> (&'static str, Color32, &'static str) {
    match outlook {
        Outlook::Win => (icon::CHECK_CIRCLE, p.success, "explorer-end-result-win"),
        Outlook::Draw => (icon::EQUALS, p.ink_muted, "explorer-end-result-draw"),
        Outlook::Loss => (icon::X_CIRCLE, p.danger, "explorer-end-result-loss"),
    }
}

/// Ícone, cor e palavra do resultado de um Lance, com a nuance da regra dos 50 Lances: uma
/// vitória (ou derrota) que a regra transforma em empate aparece como empate.
fn move_style(p: &Palette, verdict: Verdict) -> (&'static str, Color32, &'static str) {
    match (verdict.outlook, verdict.nuance) {
        (Outlook::Win, Some(Nuance::CursedWin)) | (Outlook::Loss, Some(Nuance::BlessedLoss)) => {
            (icon::EQUALS, p.ink_muted, "explorer-end-result-fifty")
        }
        (Outlook::Win, Some(Nuance::Probable)) => (
            icon::CHECK_CIRCLE,
            p.success,
            "explorer-end-result-probable-win",
        ),
        (Outlook::Loss, Some(Nuance::Probable)) => (
            icon::X_CIRCLE,
            p.danger,
            "explorer-end-result-probable-loss",
        ),
        (outlook, _) => outlook_style(p, outlook),
    }
}

/// Texto do veredito para quem joga: "Vitória das Brancas", "Empate"…
fn verdict_headline(i18n: &I18n, verdict: Verdict, side: Color) -> String {
    let winner = |side: Color| match side {
        Color::White => i18n.t("explorer-end-win-white"),
        Color::Black => i18n.t("explorer-end-win-black"),
    };
    match verdict.outlook {
        Outlook::Win => winner(side),
        Outlook::Draw => i18n.t("explorer-end-draw"),
        Outlook::Loss => winner(side.opposite()),
    }
}

fn nuance_text(i18n: &I18n, nuance: Nuance) -> String {
    i18n.t(match nuance {
        Nuance::CursedWin => "explorer-end-nuance-cursed",
        Nuance::BlessedLoss => "explorer-end-nuance-blessed",
        Nuance::Probable => "explorer-end-nuance-probable",
    })
}

fn mate_text(i18n: &I18n, dtm: i32) -> String {
    let n = mate_in_moves(dtm);
    let key = if n == 1 {
        "explorer-end-mate-one"
    } else {
        "explorer-end-mate-other"
    };
    i18n.t_with(key, &[("count", &n.to_string())])
}

pub fn endgames_table(ui: &mut Ui, i18n: &I18n, table: &EndgameTable, actions: &mut Vec<UiAction>) {
    let p = palette(ui);
    let _ = actions;
    if table.checkmate || table.stalemate || table.insufficient_material {
        let key = if table.checkmate {
            "explorer-end-checkmate"
        } else if table.stalemate {
            "explorer-end-stalemate"
        } else {
            "explorer-end-insufficient"
        };
        ui.label(egui::RichText::new(i18n.t(key)).font(strong_font(ui.ctx(), 16.0)));
        return;
    }
    if let Some(verdict) = table.verdict {
        let (glyph, color, _) = outlook_style(p, verdict.outlook);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(glyph)
                    .font(icon_font(ui.ctx(), 20.0))
                    .color(color),
            );
            ui.label(
                egui::RichText::new(verdict_headline(i18n, verdict, table.side))
                    .font(strong_font(ui.ctx(), 20.0))
                    .color(color),
            );
        });
        if let Some(nuance) = verdict.nuance {
            muted(ui, nuance_text(i18n, nuance));
        }
    }
    // Distâncias: mate e zeramento, com a explicação na dica.
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if let Some(dtm) = table.dtm.filter(|d| *d != 0) {
            let r = ui.label(egui::RichText::new(mate_text(i18n, dtm)).strong());
            r.on_hover_text(i18n.t("explorer-end-dtm-tip"));
        }
        if let Some(dtz) = table.dtz.filter(|d| *d != 0) {
            let text = i18n.t_with(
                "explorer-end-dtz",
                &[("count", &dtz.unsigned_abs().to_string())],
            );
            ui.add(egui::Label::new(
                egui::RichText::new(text).small().color(p.ink_muted),
            ))
            .on_hover_text(i18n.t("explorer-end-dtz-tip"));
        }
    });
    ui.add_space(8.0);
    muted(ui, i18n.t("explorer-end-moves"));
    ui.add_space(4.0);
    for mv in &table.moves {
        endgame_row(ui, i18n, mv);
    }
}

fn endgame_row(ui: &mut Ui, i18n: &I18n, mv: &EndgameMove) {
    let p = palette(ui);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    if response.hovered() {
        painter.rect_filled(rect, radius("radius-s"), p.surface_3);
    }
    let y = rect.center().y;
    painter.text(
        egui::pos2(rect.left() + PAD, y),
        Align2::LEFT_CENTER,
        &mv.san,
        strong_font(ui.ctx(), 14.0),
        p.ink,
    );
    // Onde termina o texto do resultado: a distância só aparece se couber depois dele.
    let mut text_end = rect.left() + PAD + 76.0;
    if let Some(verdict) = mv.verdict {
        let (glyph, color, key) = move_style(p, verdict);
        let x = rect.left() + PAD + 76.0;
        painter.text(
            egui::pos2(x, y),
            Align2::LEFT_CENTER,
            glyph,
            icon_font(ui.ctx(), 16.0),
            color,
        );
        text_end = painter
            .text(
                egui::pos2(x + 24.0, y),
                Align2::LEFT_CENTER,
                i18n.t(key),
                body(ui),
                p.ink,
            )
            .right();
    }
    let distance = match (mv.dtm.filter(|d| *d != 0), mv.dtz.filter(|d| *d != 0)) {
        (Some(dtm), _) => Some(mate_text(i18n, dtm)),
        (None, Some(dtz)) => Some(i18n.t_with(
            "explorer-end-dtz",
            &[("count", &dtz.unsigned_abs().to_string())],
        )),
        _ => None,
    };
    if let Some(text) = distance {
        let run = Tabular::new(&painter, &text, &caption(ui), p.ink_muted);
        let x = rect.right() - PAD - run.size.x;
        if x >= text_end + 12.0 {
            run.paint(&painter, egui::pos2(x, y - run.size.y / 2.0), p.ink_muted);
        } else {
            // Sem espaço: a distância vai na dica da linha.
            response.on_hover_text(text);
        }
    }
}

// --- Origem (rodapé) -----------------------------------------------------------------

fn ago_text(i18n: &I18n, age: Age) -> String {
    match age {
        Age::JustNow => i18n.t("explorer-ago-now"),
        Age::Minutes(n) => i18n.t_with("explorer-ago-minutes", &[("count", &n.to_string())]),
        Age::Hours(n) => i18n.t_with("explorer-ago-hours", &[("count", &n.to_string())]),
        Age::Days(n) => i18n.t_with("explorer-ago-days", &[("count", &n.to_string())]),
    }
}

/// "Lichess · consultado às 14:32 · CC0", "do cache, de 09/11" ou "Sua Biblioteca, offline".
/// A hora é a local do computador; sem fuso do sistema, "consultado há 3 min" (#239).
pub fn origin_text(i18n: &I18n, origin: &OriginNote, now: Timestamp) -> String {
    let mut args: Vec<(&str, String)> = vec![
        ("provider", origin.provider.clone()),
        ("license", origin.license.clone()),
    ];
    let key = match &origin.how {
        How::Local => "explorer-origin-local",
        How::Queried(Age::JustNow, _) => "explorer-origin-queried-now",
        How::Queried(age, at) => {
            let local = LocalClock::current().when(i18n, now.as_millis(), at.as_millis());
            match (local, age) {
                (Some(when), _) => {
                    args.push(("when", when));
                    "explorer-origin-queried-at"
                }
                (None, Age::Minutes(n) | Age::Hours(n) | Age::Days(n)) => {
                    args.push(("count", n.to_string()));
                    match age {
                        Age::Minutes(_) => "explorer-origin-queried-minutes",
                        Age::Hours(_) => "explorer-origin-queried-hours",
                        _ => "explorer-origin-queried-days",
                    }
                }
                (None, Age::JustNow) => "explorer-origin-queried-now",
            }
        }
        How::Cached(at) | How::Stale(at) => {
            args.push(("date", day_month(*at)));
            "explorer-origin-cached"
        }
    };
    let list: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
    i18n.t_with(key, &list)
}

/// O aviso de que não há internet e o que está na tela foi salvo antes.
fn stale_banner(ui: &mut Ui, i18n: &I18n, origin: &OriginNote, now: Timestamp) {
    let How::Stale(at) = origin.how else {
        return;
    };
    let p = palette(ui);
    let age = super::model::age_of(std::time::Duration::from_millis(
        u64::try_from(now.as_millis() - at.as_millis()).unwrap_or(0),
    ));
    egui::Frame::new()
        .fill(p.surface_2)
        .stroke(Stroke::new(1.0, p.warning))
        .corner_radius(radius("radius-m"))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.label(
                    egui::RichText::new(icon::WIFI_SLASH)
                        .font(icon_font(ui.ctx(), 16.0))
                        .color(p.warning),
                );
                ui.add(
                    egui::Label::new(i18n.t_with(
                        "explorer-stale",
                        &[("date", &day_month(at)), ("ago", &ago_text(i18n, age))],
                    ))
                    .wrap(),
                );
            });
        });
    ui.add_space(8.0);
}

pub fn stale_notice(ui: &mut Ui, i18n: &I18n, origin: &OriginNote, now: Timestamp) {
    stale_banner(ui, i18n, origin, now);
}

/// Rodapé com a Origem: Provedor, idade do dado e licença.
pub fn footer(ui: &mut Ui, i18n: &I18n, origin: &OriginNote, now: Timestamp) {
    let p = palette(ui);
    ui.add_space(12.0);
    let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(line.x_range(), line.center().y, Stroke::new(1.0, p.border));
    ui.add_space(8.0);
    muted(ui, origin_text(i18n, origin, now));
}

// --- Avisos --------------------------------------------------------------------------

/// Bloco de estado: ícone, título, explicação e, abaixo, os botões que `buttons` desenha.
fn notice_block(
    ui: &mut Ui,
    glyph: &str,
    tint: Color32,
    title: &str,
    text: &str,
    buttons: impl FnOnce(&mut Ui),
) {
    let p = palette(ui);
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(glyph)
                .font(icon_font(ui.ctx(), 32.0))
                .color(tint),
        );
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(egui::RichText::new(title).font(strong_font(ui.ctx(), 16.0))).wrap(),
        );
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new(text).color(p.ink_muted)).wrap());
        ui.add_space(12.0);
        buttons(ui);
    });
}

/// O estado do Painel que não é uma tabela, sem susto e sem jargão.
pub fn notice(
    ui: &mut Ui,
    i18n: &I18n,
    notice: &Notice,
    on_endgames: bool,
    now: Timestamp,
    actions: &mut Vec<UiAction>,
) {
    let p = palette(ui);
    match notice {
        Notice::FairPlay => notice_block(
            ui,
            icon::SHIELD_CHECK,
            p.accent,
            &i18n.t("explorer-fairplay-title"),
            &i18n.t("explorer-fairplay-text"),
            |_| {},
        ),
        Notice::Training => notice_block(
            ui,
            icon::BARBELL,
            p.ink_muted,
            &i18n.t("explorer-training-title"),
            &i18n.t("explorer-training-text"),
            |_| {},
        ),
        Notice::ProviderOff(provider) => {
            let name = provider.info().name;
            notice_block(
                ui,
                icon::PLUGS,
                p.ink_muted,
                &i18n.t_with("explorer-off-title", &[("provider", name)]),
                &i18n.t_with("explorer-off-text", &[("provider", name)]),
                |ui| {
                    let label = i18n.t_with("explorer-off-button", &[("provider", name)]);
                    if ui
                        .add(CsButton::new(&label).variant(ButtonVariant::Primary))
                        .clicked()
                    {
                        actions.push(UiAction::TurnOn(*provider));
                    }
                },
            );
        }
        Notice::SignInRequired(provider) => {
            let name = provider.info().name;
            notice_block(
                ui,
                icon::LOCK_KEY,
                p.ink_muted,
                &i18n.t_with("explorer-signin-title", &[("provider", name)]),
                &i18n.t_with("explorer-signin-text", &[("provider", name)]),
                |ui| {
                    if cs_button(
                        ui,
                        &i18n.t("explorer-signin-center"),
                        ButtonVariant::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::OpenConnections);
                    }
                    ui.add_space(4.0);
                    if ui
                        .add(
                            CsButton::new(&i18n.t("explorer-use-library"))
                                .variant(ButtonVariant::Subtle),
                        )
                        .clicked()
                    {
                        actions.push(UiAction::UseLibrary);
                    }
                },
            );
        }
        Notice::NeedsPlayerName => notice_block(
            ui,
            icon::USER,
            p.ink_muted,
            &i18n.t("explorer-player-title"),
            &i18n.t("explorer-player-text"),
            |_| {},
        ),
        Notice::Loading => {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(8.0);
                muted(ui, i18n.t("explorer-loading"));
            });
        }
        Notice::NoData => notice_block(
            ui,
            icon::MAGNIFYING_GLASS,
            p.ink_muted,
            &i18n.t("explorer-nodata-title"),
            &i18n.t(if on_endgames {
                "explorer-nodata-endgames"
            } else {
                "explorer-nodata-openings"
            }),
            |_| {},
        ),
        Notice::Limited { until } => {
            let minutes = minutes_until(now, *until);
            notice_block(
                ui,
                icon::CLOCK_COUNTDOWN,
                p.warning,
                &i18n.t("explorer-limited-title"),
                &i18n.t_with(
                    "explorer-limited-text",
                    &[("minutes", &minutes.to_string())],
                ),
                |ui| {
                    // Sem fuso do sistema, o "em cerca de N min" de cima já diz tudo.
                    if let Some(time) = clock_local(*until) {
                        muted(
                            ui,
                            i18n.t_with("explorer-limited-until", &[("time", &time)]),
                        );
                    }
                },
            );
        }
        Notice::Offline => notice_block(
            ui,
            icon::WIFI_SLASH,
            p.warning,
            &i18n.t("explorer-offline-title"),
            &i18n.t("explorer-offline-text"),
            |ui| retry_button(ui, i18n, actions),
        ),
        Notice::Failed => notice_block(
            ui,
            icon::WARNING,
            p.warning,
            &i18n.t("explorer-failed-title"),
            &i18n.t("explorer-failed-text"),
            |ui| retry_button(ui, i18n, actions),
        ),
        Notice::TooManyPieces { pieces } => notice_block(
            ui,
            icon::CROWN_SIMPLE,
            p.ink_muted,
            &i18n.t("explorer-pieces-title"),
            &i18n.t_with("explorer-pieces-text", &[("pieces", &pieces.to_string())]),
            |_| {},
        ),
        Notice::EmptyLibrary => notice_block(
            ui,
            icon::BOOKS,
            p.ink_muted,
            &i18n.t("explorer-empty-title"),
            &i18n.t("explorer-empty-text"),
            |ui| {
                if cs_button(
                    ui,
                    &i18n.t("explorer-empty-button"),
                    ButtonVariant::Secondary,
                )
                .clicked()
                {
                    actions.push(UiAction::OpenLibrary);
                }
            },
        ),
    }
}

fn retry_button(ui: &mut Ui, i18n: &I18n, actions: &mut Vec<UiAction>) {
    if ui
        .add(
            CsButton::new(&i18n.t("explorer-retry"))
                .icon(icon::ARROW_CLOCKWISE)
                .variant(ButtonVariant::Secondary),
        )
        .clicked()
    {
        actions.push(UiAction::Retry);
    }
}

// --- Fonte (cabeçalho) ---------------------------------------------------------------

/// Nome da fonte na lista (com o selo "online" nas que usam o Lichess).
pub fn source_label(i18n: &I18n, source: Source) -> String {
    i18n.t(source.label_key())
}

// --- Avaliação em cache (Painel Análise) ---------------------------------------------

/// A linha "Em cache (Lichess)": separada da Engine local, nunca misturada a ela.
pub fn cloud_row(
    ui: &mut Ui,
    i18n: &I18n,
    row: &CloudRow,
    base_white_to_move: bool,
    actions: &mut Vec<UiAction>,
) {
    let _ = base_white_to_move;
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.surface_2)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(radius("radius-m"))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(icon::CLOUD)
                        .font(icon_font(ui.ctx(), 16.0))
                        .color(p.ink_muted),
                );
                ui.label(
                    egui::RichText::new(i18n.t("explorer-cloud-title"))
                        .font(strong_font(ui.ctx(), 14.0)),
                );
            });
            ui.add_space(4.0);
            match row {
                CloudRow::Ready(view) => cloud_ready(ui, i18n, view),
                CloudRow::NoEvaluation => muted(ui, i18n.t("explorer-cloud-none")),
                CloudRow::Notice(Notice::Loading) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        muted(ui, i18n.t("explorer-cloud-loading"));
                    });
                }
                CloudRow::Notice(Notice::ProviderOff(provider)) => {
                    muted(ui, i18n.t("explorer-cloud-off"));
                    ui.add_space(4.0);
                    let name = provider.info().name;
                    let label = i18n.t_with("explorer-off-button", &[("provider", name)]);
                    if ui
                        .add(CsButton::new(&label).variant(ButtonVariant::Secondary))
                        .clicked()
                    {
                        actions.push(UiAction::TurnOn(*provider));
                    }
                }
                CloudRow::Notice(Notice::FairPlay) => {
                    muted(ui, i18n.t("explorer-cloud-fairplay"));
                }
                CloudRow::Notice(Notice::Limited { .. }) => {
                    muted(ui, i18n.t("explorer-cloud-limited"));
                }
                CloudRow::Notice(_) => {
                    muted(ui, i18n.t("explorer-cloud-failed"));
                    ui.add_space(4.0);
                    retry_button(ui, i18n, actions);
                }
            }
        });
}

fn cloud_ready(ui: &mut Ui, i18n: &I18n, view: &CloudView) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        // Valor do ponto de vista das Brancas, com a barra pequena ao lado.
        let text = format_eval(view.score_white);
        let font = strong_font(ui.ctx(), 20.0);
        let run = Tabular::new(ui.painter(), &text, &font, p.ink);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(run.size.x.max(56.0), 28.0), Sense::hover());
        run.paint(
            ui.painter(),
            egui::pos2(rect.left(), rect.center().y - run.size.y / 2.0),
            p.ink,
        );
        let (bar, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(120.0), 8.0),
            Sense::hover(),
        );
        let colors = bar_colors(p);
        let share = white_share(view.score_white);
        let painter = ui.painter_at(bar);
        let corner = radius("radius-s");
        painter.rect_filled(bar, corner, colors.black);
        painter.rect_filled(
            Rect::from_min_size(bar.min, egui::vec2(bar.width() * share, bar.height())),
            corner,
            colors.white,
        );
        painter.rect_stroke(
            bar,
            corner,
            Stroke::new(1.0, p.border_strong),
            StrokeKind::Inside,
        );
        response.on_hover_text(i18n.t("explorer-cloud-pov"));
    });
    muted(
        ui,
        i18n.t_with(
            "explorer-cloud-depth",
            &[("depth", &view.depth.to_string())],
        ),
    );
    if !view.san.is_empty() {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for token in
                crate::analysis::line_tokens(view.first_number, view.white_to_move, view.san.len())
            {
                if let Some(number) = &token.number {
                    ui.label(egui::RichText::new(number).color(p.ink_muted));
                }
                ui.label(&view.san[token.index]);
            }
        });
    }
    ui.add_space(4.0);
    muted(ui, origin_text(i18n, &view.origin, NetService::now()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_time::{LocalClock, testing};

    /// 2026-11-11 15:30 UTC (12:30 em Brasília).
    const NOW: i64 = 1_794_411_000_000;

    fn text(how: How) -> String {
        let origin = OriginNote {
            provider: "Lichess".into(),
            how,
            license: "CC0".into(),
        };
        origin_text(&I18n::pt_br(), &origin, Timestamp::from_millis(NOW))
    }

    #[test]
    fn the_footer_says_the_local_time_of_the_query() {
        let queried = Timestamp::from_millis(NOW - 3 * 60_000);
        let line = testing::with(LocalClock::fixed(-180), || {
            text(How::Queried(Age::Minutes(3), queried))
        });
        assert_eq!(line, "Lichess \u{b7} consultado às 12:27 \u{b7} CC0");
        let yesterday = Timestamp::from_millis(NOW - 24 * 3_600_000);
        let line = testing::with(LocalClock::fixed(-180), || {
            text(How::Queried(Age::Days(1), yesterday))
        });
        assert_eq!(line, "Lichess \u{b7} consultado ontem às 12:30 \u{b7} CC0");
    }

    #[test]
    fn without_a_time_zone_the_footer_falls_back_to_the_relative_age() {
        let queried = Timestamp::from_millis(NOW - 3 * 60_000);
        let line = testing::with(LocalClock::unknown(), || {
            text(How::Queried(Age::Minutes(3), queried))
        });
        assert_eq!(line, "Lichess \u{b7} consultado há 3 min \u{b7} CC0");
        let line = testing::with(LocalClock::unknown(), || {
            text(How::Queried(Age::JustNow, queried))
        });
        assert_eq!(line, "Lichess \u{b7} consultado agora \u{b7} CC0");
    }

    #[test]
    fn the_cache_date_is_the_local_day() {
        // 01:30 UTC de 12/11 ainda é 11/11 em Brasília.
        let at = Timestamp::from_millis(NOW + 10 * 3_600_000);
        let line = testing::with(LocalClock::fixed(-180), || text(How::Cached(at)));
        assert_eq!(line, "Lichess \u{b7} do cache, de 11/11 \u{b7} CC0");
    }

    #[test]
    fn the_opening_name_reads_in_portuguese() {
        let i18n = I18n::pt_br();
        assert_eq!(
            opening_label(&i18n, "C60", "Ruy Lopez: Morphy Defense"),
            "C60 \u{b7} Ruy Lopez: Defesa Morphy"
        );
    }
}
