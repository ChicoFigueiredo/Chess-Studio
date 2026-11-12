//! Barra de avaliação (`docs/design/componentes/EvalBar.md`, DS-16, issue #165): vertical,
//! colada à esquerda do tabuleiro, visível só com a Análise ligada e a Engine consentida
//! (ADR-0003). É só Apresentação: lê a última avaliação da Análise e nunca decide nada da
//! Partida (ADR-0001). A matemática (curva, rótulo, visibilidade, geometria) é pura e testada;
//! o sistema só pinta com o `egui::Painter` na área do tabuleiro, sem alocar a cada quadro.

use crate::analysis::{Analysis, Status};
use crate::board2d_panel::{Orientation, orientation_for};
use crate::camera::{CameraRig, MainCamera};
use crate::dock::Dock;
use crate::presentation::Presentation;
use crate::presentation::TechnicalCamera;
use crate::ui_theme::{Palette, Tokens, strong_font};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_uci::Score;

/// Vantagem (em peões) a partir da qual a barra fica cheia.
pub const SATURATION_PAWNS: f32 = 8.0;
/// Inclinação da logística, por peão.
const STEEPNESS: f32 = 0.5;

/// Fração da barra ocupada pelas brancas (0 a 1): logística em peões, renormalizada para
/// valer exatamente 0,5 em 0 e 1 (ou 0) a partir de ±8 peões. Mate enche a barra.
pub fn eval_fill(score: Score) -> f32 {
    match score {
        Score::Mate(n) if n >= 0 => 1.0,
        Score::Mate(_) => 0.0,
        Score::Cp(cp) => {
            #[allow(clippy::cast_precision_loss)]
            let pawns = (cp as f32 / 100.0).clamp(-SATURATION_PAWNS, SATURATION_PAWNS);
            let logistic = |x: f32| 1.0 / (1.0 + (-STEEPNESS * x).exp());
            let top = logistic(SATURATION_PAWNS) - 0.5;
            (0.5 + 0.5 * (logistic(pawns) - 0.5) / top).clamp(0.0, 1.0)
        }
    }
}

/// Texto do valor, em pt-BR: `+0,4`, `−3,1` (sinal de menos U+2212), `0,0`, `M3`, `−M3`.
pub fn eval_label(score: Score) -> String {
    match score {
        Score::Mate(n) if n >= 0 => format!("M{n}"),
        Score::Mate(n) => format!("\u{2212}M{}", n.unsigned_abs()),
        Score::Cp(cp) => {
            // Décimos de peão arredondados com inteiros (sem surpresas de ponto flutuante).
            let tenths = (i64::from(cp).unsigned_abs() + 5) / 10;
            let (whole, frac) = (tenths / 10, tenths % 10);
            let sign = if tenths == 0 {
                ""
            } else if cp > 0 {
                "+"
            } else {
                "\u{2212}"
            };
            format!("{sign}{whole},{frac}")
        }
    }
}

/// A barra só aparece com a Análise ligada, uma Engine consentida (ADR-0003) e sem bloqueio
/// da Política competitiva ou do Treino.
pub fn is_visible(analysis_on: bool, engine_consented: bool, locked: bool) -> bool {
    analysis_on && engine_consented && !locked
}

/// Altura da parte das brancas e se ela cresce de baixo para cima: a barra vira junto com o
/// tabuleiro (brancas embaixo: sobem do pé; pretas embaixo: descem do topo).
pub fn white_extent(fill: f32, height: f32, orientation: Orientation) -> (f32, bool) {
    (
        fill.clamp(0.0, 1.0) * height,
        orientation == Orientation::WhiteBottom,
    )
}

/// Retângulo da barra (sem o rótulo) dentro da área do tabuleiro, colada à esquerda.
pub fn bar_rect(area: egui::Rect, width: f32, label_h: f32) -> egui::Rect {
    let total = (area.height() * 0.72).clamp(96.0, 520.0).min(area.height());
    let bar_h = (total - label_h).max(32.0);
    let top = area.center().y - total / 2.0;
    egui::Rect::from_min_size(
        egui::pos2(area.min.x + 12.0, top.max(area.min.y)),
        egui::vec2(width, bar_h),
    )
}

/// Só nas capturas (`--eval-demo 0.4`): avaliação falsa, em peões, que força a barra a
/// aparecer sem Engine consentida. Nunca existe fora do modo de captura.
pub fn demo_score(arg: &str) -> Option<Score> {
    let pawns: f32 = arg.trim().replace(',', ".").parse().ok()?;
    pawns
        .is_finite()
        .then(|| Score::Cp((pawns * 100.0).round().clamp(-100_000.0, 100_000.0) as i32))
}

/// Retângulo da barra colada ao tabuleiro: à esquerda da caixa `bbox` (o tabuleiro projetado
/// na tela), com `gap` de folga, da altura dela (o rótulo cabe no pé), sempre dentro de `area`.
pub fn glued_bar_rect(
    bbox: egui::Rect,
    area: egui::Rect,
    width: f32,
    label_h: f32,
    gap: f32,
) -> egui::Rect {
    let x = (bbox.min.x - gap - width).clamp(area.min.x, (area.max.x - width).max(area.min.x));
    let top = bbox.min.y.clamp(area.min.y, area.max.y);
    let bottom = (bbox.max.y - label_h).clamp(area.min.y, (area.max.y - label_h).max(area.min.y));
    let bottom = bottom.max(top + 32.0);
    egui::Rect::from_min_max(egui::pos2(x, top), egui::pos2(x + width, bottom))
}

/// As câmeras do tabuleiro (3D e 2D técnico); só a ativa conta.
pub(crate) type BoardCameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        &'static GlobalTransform,
        Has<TechnicalCamera>,
    ),
    Or<(With<MainCamera>, With<TechnicalCamera>)>,
>;

/// Meia-largura da moldura do tabuleiro (igual à de `framing`).
const FRAME_HALF: f32 = chess_input::BOARD_SIZE / 2.0 + 0.6;

/// Caixa, em pontos do egui, do tabuleiro (moldura) visto pela câmera ativa. Sem alocação.
pub(crate) fn projected_board(
    cameras: &BoardCameras<'_, '_>,
    scale_factor: f32,
    ppp: f32,
) -> Option<egui::Rect> {
    let (camera, transform, technical) = cameras.iter().find(|(c, _, _)| c.is_active)?;
    let offset = camera
        .viewport
        .as_ref()
        .map_or(Vec2::ZERO, |v| v.physical_position.as_vec2());
    let mut bbox: Option<egui::Rect> = None;
    // 3D: a moldura no plano XZ. 2D técnico: o tabuleiro no plano XY, com a faixa das
    // coordenadas (números à esquerda) incluída para a barra não cobri-las.
    let half = if technical {
        (chess_input::BOARD_SIZE / 2.0 + 0.7) * crate::board2d::UNIT
    } else {
        FRAME_HALF
    };
    for (x, z) in [(-half, -half), (half, -half), (half, half), (-half, half)] {
        let world = if technical {
            Vec3::new(x, z, 0.0)
        } else {
            Vec3::new(x, 0.0, z)
        };
        let at = camera.world_to_viewport(transform, world).ok()?;
        let point = (at * scale_factor + offset) / ppp;
        let point = egui::pos2(point.x, point.y);
        bbox = Some(match bbox {
            Some(b) => b.union(egui::Rect::from_min_max(point, point)),
            None => egui::Rect::from_min_max(point, point),
        });
    }
    bbox
}

pub struct EvalBarPlugin;

impl Plugin for EvalBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            eval_bar_ui.after(crate::dock::dock_ui),
        );
    }
}

/// Último valor formatado: o texto só é refeito quando a avaliação muda.
#[derive(Default)]
struct Cached {
    score: Option<Score>,
    label: String,
}

#[allow(clippy::too_many_arguments)]
fn eval_bar_ui(
    mut contexts: EguiContexts,
    analysis: Res<Analysis>,
    dock: Res<Dock>,
    rig: Option<Res<CameraRig>>,
    presentation: Option<Res<Presentation>>,
    view: Option<Res<crate::focus::ViewState>>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut demo: Local<Option<Option<Score>>>,
    cameras: BoardCameras,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
    mut cache: Local<Cached>,
) -> Result {
    let forced = *demo.get_or_insert_with(|| {
        capture
            .and_then(|_| crate::capture::option_value("--eval-demo"))
            .and_then(|v| demo_score(&v))
    });
    let locked = matches!(analysis.status(), Status::Locked(_));
    if forced.is_none() && !is_visible(analysis.is_enabled(), analysis.engine_available(), locked) {
        return Ok(());
    }
    if view.is_some_and(|v| v.focus) {
        return Ok(());
    }
    let Some(score) = forced.or_else(|| {
        analysis
            .shown()
            .and_then(|s| s.update.lines.first())
            .map(|l| l.score)
    }) else {
        return Ok(());
    };
    let Some(area) = dock.board_area() else {
        return Ok(());
    };
    let three_d = presentation.is_none_or(|p| p.is_3d());
    let yaw = rig.map_or(0.0, |r| r.pose.yaw);
    let orientation = orientation_for(yaw, three_d);
    if cache.score != Some(score) {
        cache.score = Some(score);
        cache.label = eval_label(score);
    }
    let ctx = contexts.ctx_mut()?;
    let p = palette_of(ctx);
    let tokens = Tokens::embedded();
    let width = tokens.length("evalbar-w").unwrap_or(16.0);
    let radius = tokens.length("radius-s").unwrap_or(4.0) as u8;
    let font = strong_font(ctx, 12.0);
    let label_h = 20.0;
    let gap = tokens.length("space-2").unwrap_or(8.0);
    let scale_factor = window.map_or(1.0, |w| w.scale_factor());
    let bar = match projected_board(&cameras, scale_factor, ctx.pixels_per_point()) {
        Some(bbox) => glued_bar_rect(bbox, area, width, label_h, gap),
        None => bar_rect(area, width, label_h),
    };
    let (white_h, from_bottom) = white_extent(eval_fill(score), bar.height(), orientation);
    let label = cache.label.as_str();
    // Sem interação: o ponteiro continua do tabuleiro (órbita, picking).
    egui::Area::new(egui::Id::new("chess-studio-eval-bar"))
        .order(egui::Order::Foreground)
        .fixed_pos(bar.min)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();
            let corner = egui::CornerRadius::same(radius);
            painter.rect_filled(bar, corner, p.eval_black);
            let white = if from_bottom {
                egui::Rect::from_min_max(egui::pos2(bar.min.x, bar.max.y - white_h), bar.max)
            } else {
                egui::Rect::from_min_max(bar.min, egui::pos2(bar.max.x, bar.min.y + white_h))
            };
            painter.rect_filled(white, corner, p.eval_white);
            painter.hline(
                bar.x_range(),
                bar.center().y,
                egui::Stroke::new(1.0, p.accent),
            );
            painter.rect_stroke(
                bar,
                corner,
                egui::Stroke::new(1.0, p.border_strong),
                egui::StrokeKind::Outside,
            );
            // Números tabulares: o valor não muda de largura a cada atualização.
            let run = crate::ui_theme::Tabular::new(painter, label, &font, p.ink);
            let top_left = egui::pos2(bar.center().x - run.size.x / 2.0, bar.max.y + 4.0);
            run.paint(painter, top_left, p.ink);
        });
    Ok(())
}

fn palette_of(ctx: &egui::Context) -> &'static Palette {
    Palette::from_visuals(&ctx.global_style().visuals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_even_position_is_half_and_saturation_fills_the_bar() {
        assert!((eval_fill(Score::Cp(0)) - 0.5).abs() < 1e-6);
        assert!((eval_fill(Score::Cp(800)) - 1.0).abs() < 1e-6);
        assert!(eval_fill(Score::Cp(-800)).abs() < 1e-6);
        // Além de ±8 peões não muda mais.
        assert_eq!(eval_fill(Score::Cp(5_000)), eval_fill(Score::Cp(800)));
        assert_eq!(eval_fill(Score::Cp(-5_000)), eval_fill(Score::Cp(-800)));
    }

    #[test]
    fn the_curve_grows_with_the_advantage_and_is_symmetric() {
        let mut last = -1.0;
        for cp in (-800..=800).step_by(50) {
            let f = eval_fill(Score::Cp(cp));
            assert!(f >= last, "a curva não decresce em {cp}");
            last = f;
            let mirror = eval_fill(Score::Cp(-cp));
            assert!((f + mirror - 1.0).abs() < 1e-5, "simétrica em {cp}");
        }
        // Um peão já pende claramente, mas longe de encher.
        let one = eval_fill(Score::Cp(100));
        assert!(one > 0.55 && one < 0.75, "{one}");
    }

    #[test]
    fn mate_fills_the_bar_for_the_side_that_mates() {
        assert_eq!(eval_fill(Score::Mate(3)), 1.0);
        assert_eq!(eval_fill(Score::Mate(1)), 1.0);
        assert_eq!(eval_fill(Score::Mate(-3)), 0.0);
    }

    #[test]
    fn labels_use_the_decimal_comma_and_the_true_minus_sign() {
        assert_eq!(eval_label(Score::Cp(40)), "+0,4");
        assert_eq!(eval_label(Score::Cp(-310)), "\u{2212}3,1");
        assert_eq!(eval_label(Score::Cp(0)), "0,0");
        assert_eq!(eval_label(Score::Cp(3)), "0,0");
        assert_eq!(eval_label(Score::Cp(-4)), "0,0");
        assert_eq!(eval_label(Score::Cp(35)), "+0,4");
        assert_eq!(eval_label(Score::Cp(1_250)), "+12,5");
        assert_eq!(eval_label(Score::Mate(3)), "M3");
        assert_eq!(eval_label(Score::Mate(-2)), "\u{2212}M2");
    }

    #[test]
    fn the_bar_needs_the_analysis_on_and_a_consented_engine() {
        assert!(is_visible(true, true, false));
        assert!(!is_visible(false, true, false), "análise desligada");
        assert!(!is_visible(true, false, false), "Engine sem consentimento");
        assert!(
            !is_visible(true, true, true),
            "bloqueada por Política ou Treino"
        );
        assert!(!is_visible(false, false, false));
    }

    #[test]
    fn the_capture_demo_value_is_read_in_pawns() {
        assert_eq!(demo_score("0.4"), Some(Score::Cp(40)));
        assert_eq!(demo_score("-3,1"), Some(Score::Cp(-310)));
        assert_eq!(demo_score("abc"), None);
        assert_eq!(demo_score("inf"), None);
    }

    #[test]
    fn the_bar_flips_with_the_board() {
        let (h, bottom) = white_extent(0.75, 200.0, Orientation::WhiteBottom);
        assert_eq!((h, bottom), (150.0, true));
        let (h, bottom) = white_extent(0.75, 200.0, Orientation::BlackBottom);
        assert_eq!((h, bottom), (150.0, false));
    }

    #[test]
    fn the_bar_is_glued_to_the_left_of_the_projected_board() {
        let area = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1600.0, 1000.0));
        let bbox = egui::Rect::from_min_max(egui::pos2(400.0, 200.0), egui::pos2(1200.0, 900.0));
        let bar = glued_bar_rect(bbox, area, 16.0, 20.0, 8.0);
        assert_eq!(bar.max.x + 8.0, bbox.min.x);
        assert_eq!(bar.width(), 16.0);
        assert_eq!(bar.min.y, 200.0);
        assert_eq!(bar.max.y + 20.0, 900.0);
        assert!(area.contains_rect(bar));
    }

    #[test]
    fn a_board_against_the_edge_keeps_the_bar_inside_the_area() {
        let area = egui::Rect::from_min_max(egui::pos2(0.0, 40.0), egui::pos2(1000.0, 700.0));
        let bbox = egui::Rect::from_min_max(egui::pos2(5.0, -50.0), egui::pos2(900.0, 800.0));
        let bar = glued_bar_rect(bbox, area, 16.0, 20.0, 8.0);
        assert!(area.contains_rect(bar), "{bar:?}");
        assert!(bar.height() >= 32.0);
    }

    #[test]
    fn the_bar_hugs_the_left_edge_inside_the_board_area() {
        let area = egui::Rect::from_min_max(egui::pos2(10.0, 50.0), egui::pos2(810.0, 650.0));
        let bar = bar_rect(area, 16.0, 20.0);
        assert_eq!(bar.min.x, 22.0);
        assert_eq!(bar.width(), 16.0);
        assert!(area.contains_rect(bar));
    }
}
