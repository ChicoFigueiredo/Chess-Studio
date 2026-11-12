//! Fim de Partida visível (B30, CORE-01): diálogo modal com resultado e motivo, e o rei
//! derrotado tombado na cena. Tudo aqui é **apresentação** (ADR-0001): lê o `snapshot()` e a
//! revisão da `Session` e nunca escreve na Partida; as ações do diálogo enviam intenções
//! (revisar, Nova Partida) pelos mesmos caminhos do painel.
//!
//! A lógica (que rei tomba, quando o diálogo abre, a pose da queda) é pura e testada sem o
//! Bevy; os sistemas só aplicam o resultado às entidades e ao egui.

use crate::i18n::I18n;
use crate::new_game::NewGameDialog;
use crate::pieces::{BoardOverride, KingPiece, Piece2d, Piece3d, PieceAt, PiecePlacementSet};
use crate::presentation::VisibleIn;
use crate::session::Session;
use crate::ui_theme::{ButtonVariant, cs_button};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::{Color as Side, Square};
use chess_game::{GameResult, Outcome};
use std::f32::consts::FRAC_PI_2;

/// Duração da queda do rei.
pub const FALL_SECS: f32 = 0.7;
/// Raio aproximado da base do rei, em casas (1 casa = 1 unidade).
const KING_BASE_RADIUS: f32 = 0.3;
/// Deslocamento horizontal do rei ao tombar, em casas.
const FALL_SHIFT: f32 = 0.12;

/// Rei que tomba num desfecho: o do lado derrotado. Empate não tomba ninguém. Vale para
/// mate, abandono, tempo, Infração e Eliminação (todos têm vencedor).
pub fn fallen_king(outcome: Option<Outcome>) -> Option<Side> {
    match outcome?.result {
        GameResult::WhiteWins => Some(Side::Black),
        GameResult::BlackWins => Some(Side::White),
        GameResult::Draw => None,
    }
}

/// Rei a mostrar tombado agora. A revisão de Lances anteriores, uma variante da análise e a
/// Posição visual provisória mostram o rei de pé; a revisão do último meio-lance mostra a
/// Posição final, com o rei tombado.
pub fn fall_target(session: &Session, board_overridden: bool) -> Option<Side> {
    let snapshot = session.snapshot();
    if board_overridden || snapshot.provisional.is_some() {
        return None;
    }
    if let Some(review) = session.review()
        && (session.showing_variation() || review.ply != snapshot.history.len())
    {
        return None;
    }
    fallen_king(snapshot.outcome)
}

/// Quando o diálogo de fim de Partida abre. Abre na transição "em andamento" -> "encerrada"
/// dentro da mesma Partida (época); Desfazer reabre a Partida e rearma o aviso. Uma Partida
/// que já nasce encerrada (aberta da biblioteca, retomada) não abre o diálogo sozinha.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EndDialogState {
    epoch: Option<u64>,
    ended: bool,
    open: bool,
}

impl EndDialogState {
    /// Informa o estado deste quadro (época da Partida, se há desfecho).
    pub fn observe(&mut self, epoch: u64, ended: bool) {
        if self.epoch != Some(epoch) {
            self.epoch = Some(epoch);
            self.ended = ended;
            self.open = false;
            return;
        }
        if !ended {
            self.open = false;
        } else if !self.ended {
            self.open = true;
        }
        self.ended = ended;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Pose do rei durante a queda: rotação em torno da base, elevação e deslocamento.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FallPose {
    /// Ângulo de tombamento (0 = de pé, 90 graus = deitado).
    pub angle: f32,
    /// Quanto a base sobe para a borda não atravessar a mesa.
    pub lift: f32,
    /// Deslocamento horizontal na direção da queda.
    pub shift: f32,
}

pub fn fall_pose(progress: f32) -> FallPose {
    let p = progress.clamp(0.0, 1.0);
    let eased = p * p * (3.0 - 2.0 * p);
    let angle = eased * FRAC_PI_2;
    FallPose {
        angle,
        // A borda da base desce `raio * sen(ângulo)` ao girar: a elevação a mantém na mesa.
        lift: KING_BASE_RADIUS * angle.sin(),
        shift: FALL_SHIFT * eased,
    }
}

/// Andamento da queda. Mudar o alvo (Desfazer, Nova Partida, revisão) reinicia a animação.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FallAnim {
    pub side: Option<Side>,
    pub progress: f32,
}

impl FallAnim {
    pub fn update(&mut self, target: Option<Side>, dt: f32) {
        if target != self.side {
            self.side = target;
            self.progress = 0.0;
        }
        if self.side.is_some() {
            self.progress = (self.progress + dt / FALL_SECS).min(1.0);
        }
    }
}

/// Direção da queda como passo (arquivo, fileira): em direção ao centro do tabuleiro, de
/// preferência sobre a casa vizinha vazia, para o rei não deitar em cima de outra peça.
pub fn fall_direction(square: Square, occupied: impl Fn(Square) -> bool) -> (i8, i8) {
    let file = i8::try_from(square.file()).unwrap_or(0);
    let rank = i8::try_from(square.rank()).unwrap_or(0);
    let toward_rank = if rank >= 4 { -1 } else { 1 };
    let toward_file = if file >= 4 { -1 } else { 1 };
    let candidates = [
        (0, toward_rank),
        (toward_file, 0),
        (0, -toward_rank),
        (-toward_file, 0),
    ];
    let neighbour = |(df, dr): (i8, i8)| {
        let f = u8::try_from(file + df).ok()?;
        let r = u8::try_from(rank + dr).ok()?;
        Square::new(f, r)
    };
    candidates
        .into_iter()
        .find(|&step| neighbour(step).is_some_and(|sq| !occupied(sq)))
        .or_else(|| {
            candidates
                .into_iter()
                .find(|&step| neighbour(step).is_some())
        })
        .unwrap_or((0, toward_rank))
}

/// Estado da queda na cena.
#[derive(Resource, Debug, Default)]
pub struct KingFall {
    anim: FallAnim,
    dir: (i8, i8),
}

/// Estado do diálogo e se ele está suprimido (capturas de evidência sem o diálogo).
#[derive(Resource, Debug, Default)]
pub struct EndDialog {
    pub state: EndDialogState,
    pub suppressed: bool,
}

/// Marca vermelha sob o rei derrotado no 2D técnico.
#[derive(Component)]
struct DefeatMark;

pub struct EndGamePlugin;

impl Plugin for EndGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KingFall>()
            .init_resource::<EndDialog>()
            .add_systems(Startup, spawn_mark)
            .add_systems(Update, apply_fall.after(PiecePlacementSet))
            .add_systems(EguiPrimaryContextPass, end_dialog);
    }
}

fn spawn_mark(mut commands: Commands) {
    commands.spawn((
        DefeatMark,
        Sprite::from_color(
            Color::srgba(0.85, 0.12, 0.10, 0.55),
            Vec2::splat(crate::board2d::UNIT),
        ),
        Transform::from_xyz(0.0, 0.0, 0.6),
        Visibility::Hidden,
        VisibleIn::TECHNICAL,
    ));
}

#[allow(clippy::type_complexity)]
fn apply_fall(
    time: Res<Time>,
    session: Res<Session>,
    over: Res<BoardOverride>,
    mut fall: ResMut<KingFall>,
    mut kings: Query<
        (
            &KingPiece,
            &PieceAt,
            &mut Transform,
            Has<Piece3d>,
            Has<Piece2d>,
        ),
        Without<DefeatMark>,
    >,
    mark: Single<(&mut Transform, &mut Visibility), With<DefeatMark>>,
) {
    let target = fall_target(&session, over.is_active());
    let previous = fall.anim.side;
    fall.anim.update(target, time.delta_secs());
    if target.is_some() && previous != target {
        let pieces = crate::pieces::visible_pieces(&session, &over);
        let at = kings
            .iter()
            .find(|(k, _, _, is3d, _)| Some(k.0) == target && *is3d)
            .map(|(_, at, _, _, _)| at.0);
        if let Some(square) = at {
            fall.dir = fall_direction(square, |sq| pieces.contains_key(&sq));
        }
    }
    let pose = fall_pose(fall.anim.progress);
    let (df, dr) = fall.dir;
    let dir = Vec3::new(f32::from(df), 0.0, -f32::from(dr));
    let (mut mark_transform, mut mark_visibility) = mark.into_inner();
    let mut mark_square = None;
    for (king, at, mut transform, is_3d, is_2d) in &mut kings {
        let fallen = fall.anim.side == Some(king.0);
        if is_3d {
            let facing = Quat::from_rotation_y(if king.0 == Side::White {
                std::f32::consts::PI
            } else {
                0.0
            });
            let rotation = if fallen && dir != Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Y.cross(dir), pose.angle) * facing
            } else {
                facing
            };
            if transform.rotation != rotation {
                transform.rotation = rotation;
            }
            if fallen {
                transform.translation += Vec3::new(0.0, pose.lift, 0.0) + dir * pose.shift;
            }
        } else if is_2d {
            let rotation = if fallen {
                Quat::from_rotation_z(-pose.angle)
            } else {
                Quat::IDENTITY
            };
            if transform.rotation != rotation {
                transform.rotation = rotation;
            }
            if fallen {
                mark_square = Some(at.0);
            }
        }
    }
    match mark_square {
        Some(square) => {
            let p = crate::board2d::square_pos(square);
            mark_transform.translation.x = p.x;
            mark_transform.translation.y = p.y;
            *mark_visibility = Visibility::Visible;
        }
        None => *mark_visibility = Visibility::Hidden,
    }
}

#[allow(clippy::too_many_arguments)]
fn end_dialog(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
    mut dialog: ResMut<EndDialog>,
    mut new_game: ResMut<NewGameDialog>,
    aids: Res<crate::aids::AidSettings>,
    mut tools: ResMut<crate::fen_pgn::FileTools>,
    scoresheet: Option<Res<crate::scoresheet::ScoresheetUi>>,
) -> Result {
    let outcome = session.snapshot().outcome;
    dialog.state.observe(session.epoch(), outcome.is_some());
    // No modo espectador (#302) a Partida é da Engine online: o fim aparece na faixa e no
    // Painel, sem diálogo (Nova Partida, Revisar e Exportar são da Partida do usuário).
    let shown = dialog.state.is_open() && !dialog.suppressed && !session.is_spectator();
    let (true, Some(outcome)) = (shown, outcome) else {
        return Ok(());
    };
    let hide_notation = scoresheet
        .as_deref()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation);
    let mut close = false;
    let mut review = false;
    let mut start_new = false;
    let mut export = false;
    let mut saved_as = None;
    let modal =
        egui::Modal::new(egui::Id::new("end-game-dialog")).show(contexts.ctx_mut()?, |ui| {
            ui.set_min_width(300.0);
            ui.heading(i18n.t("endgame-title"));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(i18n.t(crate::ui::result_key(outcome.result))).size(20.0));
            ui.label(i18n.t(crate::ui::reason_key(outcome.reason)));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                review = cs_button(ui, &i18n.t("endgame-review"), ButtonVariant::Primary).clicked();
                start_new =
                    cs_button(ui, &i18n.t("button-new-game"), ButtonVariant::Secondary).clicked();
                export = ui
                    .add_enabled_ui(!hide_notation, |ui| {
                        cs_button(ui, &i18n.t("endgame-export"), ButtonVariant::Secondary)
                    })
                    .inner
                    .clicked();
                if !tools.pgn_pick.is_unavailable() {
                    ui.add_enabled_ui(!hide_notation, |ui| {
                        saved_as = tools.pgn_pick.show_save(ui, &i18n, || {
                            crate::file_picker::pgn_save_request(
                                &i18n,
                                crate::fen_pgn::FileTools::suggested_file_name(),
                            )
                        });
                    });
                }
                close = cs_button(ui, &i18n.t("endgame-close"), ButtonVariant::Subtle).clicked();
            });
            match &tools.pgn_result {
                Some(Ok(path)) => {
                    ui.small(i18n.t_with("pgn-saved", &[("path", &path.display().to_string())]));
                }
                Some(Err(message)) => {
                    ui.small(i18n.t_with("pgn-error", &[("detail", message)]));
                }
                None => {}
            }
        });
    if export {
        let dir = crate::paths::documents_dir(
            crate::paths::Platform::current(),
            &crate::paths::process_env,
        );
        tools.export(&session, &[], &dir);
    }
    if let Some(path) = saved_as {
        tools.export_to(&session, &[], &path);
    }
    if review {
        session.set_review_ply(Some(0));
        close = true;
    }
    if start_new {
        new_game.open_from(session.config(), &aids);
        close = true;
    }
    if close || modal.should_close() {
        dialog.state.close();
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, EndReason};

    fn outcome(result: GameResult, reason: EndReason) -> Option<Outcome> {
        Some(Outcome { result, reason })
    }

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn play(session: &mut Session, moves: &[&str]) {
        for m in moves {
            session.command(Command::Release {
                from: sq(&m[0..2]),
                to: sq(&m[2..4]),
            });
        }
    }

    const FOOLS_MATE: [&str; 4] = ["f2f3", "e7e5", "g2g4", "d8h4"];
    const SCHOLARS_MATE: [&str; 7] = ["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"];

    #[test]
    fn the_loser_king_falls_in_every_decisive_ending() {
        for reason in [
            EndReason::Checkmate,
            EndReason::Resignation,
            EndReason::Timeout,
            EndReason::SecondIllegalMove,
            EndReason::EliminatedByInfraction,
        ] {
            assert_eq!(
                fallen_king(outcome(GameResult::WhiteWins, reason)),
                Some(Side::Black),
                "{reason:?}"
            );
            assert_eq!(
                fallen_king(outcome(GameResult::BlackWins, reason)),
                Some(Side::White),
                "{reason:?}"
            );
        }
    }

    #[test]
    fn no_king_falls_in_draws_or_while_playing() {
        for reason in [
            EndReason::Stalemate,
            EndReason::InsufficientMaterial,
            EndReason::DeadPosition,
            EndReason::ThreefoldRepetition,
            EndReason::FivefoldRepetition,
            EndReason::FiftyMoveRule,
            EndReason::SeventyFiveMoveRule,
            EndReason::DrawAgreement,
            EndReason::TimeoutDraw,
        ] {
            assert_eq!(fallen_king(outcome(GameResult::Draw, reason)), None);
        }
        assert_eq!(fallen_king(None), None);
    }

    #[test]
    fn dialog_opens_when_the_game_ends_and_stays_closed_after_closing() {
        let mut s = EndDialogState::default();
        s.observe(1, false);
        assert!(!s.is_open());
        s.observe(1, true);
        assert!(s.is_open());
        s.close();
        for _ in 0..100 {
            s.observe(1, true);
        }
        assert!(!s.is_open(), "não reaparece a cada quadro");
    }

    #[test]
    fn dialog_reappears_when_the_game_ends_again_after_undo() {
        let mut s = EndDialogState::default();
        s.observe(1, false);
        s.observe(1, true);
        s.close();
        s.observe(1, false); // Desfazer reabriu a Partida
        s.observe(1, true); // outro fim
        assert!(s.is_open());
    }

    #[test]
    fn undo_closes_an_open_dialog() {
        let mut s = EndDialogState::default();
        s.observe(1, false);
        s.observe(1, true);
        s.observe(1, false);
        assert!(!s.is_open());
    }

    #[test]
    fn a_game_that_starts_ended_or_a_new_game_does_not_open_the_dialog() {
        let mut s = EndDialogState::default();
        s.observe(3, true); // Partida aberta já encerrada
        assert!(!s.is_open());
        s.observe(3, false);
        s.observe(3, true);
        assert!(s.is_open());
        s.observe(4, false); // Nova Partida
        assert!(!s.is_open());
    }

    #[test]
    fn fall_pose_goes_from_upright_to_lying_without_sinking_into_the_table() {
        assert_eq!(fall_pose(0.0).angle, 0.0);
        assert_eq!(fall_pose(0.0).lift, 0.0);
        let end = fall_pose(1.0);
        assert!((end.angle - FRAC_PI_2).abs() < 1e-6, "~90 graus");
        assert!(end.shift > 0.0 && end.shift < 0.5, "leve deslocamento");
        let mut last = -1.0;
        for i in 0..=100 {
            let pose = fall_pose(i as f32 / 100.0);
            assert!(pose.angle >= last, "rotação monotônica");
            last = pose.angle;
            // A borda mais baixa da base fica em `lift - raio * sen(ângulo)`: nunca abaixo da mesa.
            assert!(pose.lift - KING_BASE_RADIUS * pose.angle.sin() >= -1e-6);
        }
    }

    #[test]
    fn fall_animation_restarts_when_the_target_changes_and_clears_on_undo() {
        let mut a = FallAnim::default();
        a.update(Some(Side::Black), FALL_SECS / 2.0);
        assert!((a.progress - 0.5).abs() < 1e-6);
        a.update(Some(Side::Black), FALL_SECS);
        assert_eq!(a.progress, 1.0);
        a.update(None, 0.016);
        assert_eq!(a, FallAnim::default(), "rei de pé de novo");
        a.update(Some(Side::White), 0.0);
        assert_eq!(a.progress, 0.0, "recomeça do zero");
    }

    #[test]
    fn fall_direction_heads_to_the_centre_over_an_empty_neighbour() {
        // Rei preto em e8: cai para a fileira 7 (e7 vazia).
        assert_eq!(fall_direction(sq("e8"), |_| false), (0, -1));
        // Com e7 ocupada, vai para o lado do centro (d8 e f8 contam).
        assert_eq!(fall_direction(sq("e8"), |s| s == sq("e7")), (-1, 0));
        // Rei branco em e1 cai para a fileira 2.
        assert_eq!(fall_direction(sq("e1"), |_| false), (0, 1));
        // Cercado: ainda cai para dentro do tabuleiro.
        let (df, dr) = fall_direction(sq("a1"), |_| true);
        assert!(df >= 0 && dr >= 0 && (df, dr) != (0, 0));
    }

    #[test]
    fn scholars_mate_drops_the_black_king_and_undo_lifts_it() {
        let mut session = Session::new();
        play(&mut session, &SCHOLARS_MATE);
        assert_eq!(fall_target(&session, false), Some(Side::Black));
        assert_eq!(fall_target(&session, true), None, "desenho alternativo");
        session.undo();
        assert_eq!(fall_target(&session, false), None);
    }

    #[test]
    fn fools_mate_drops_the_white_king() {
        let mut session = Session::new();
        play(&mut session, &FOOLS_MATE);
        assert_eq!(fall_target(&session, false), Some(Side::White));
    }

    #[test]
    fn reviewing_earlier_plies_shows_the_king_upright() {
        let mut session = Session::new();
        play(&mut session, &SCHOLARS_MATE);
        let total = session.snapshot().history.len();
        session.set_review_ply(Some(2));
        assert_eq!(fall_target(&session, false), None);
        session.set_review_ply(Some(total));
        assert_eq!(fall_target(&session, false), Some(Side::Black));
        session.set_review_ply(None);
        assert_eq!(fall_target(&session, false), Some(Side::Black));
    }

    #[test]
    fn resignation_drops_the_king_of_the_side_to_move_and_a_new_game_resets() {
        let mut session = Session::new();
        play(&mut session, &["e2e4"]);
        session.resign_side_to_move(); // as pretas abandonam: as brancas vencem
        assert_eq!(fall_target(&session, false), Some(Side::Black));
        session.new_game();
        assert_eq!(fall_target(&session, false), None);
    }

    #[test]
    fn dialog_follows_the_session_through_end_undo_and_second_end() {
        let mut session = Session::new();
        let mut dialog = EndDialogState::default();
        let frame = |session: &Session, d: &mut EndDialogState| {
            d.observe(session.epoch(), session.snapshot().outcome.is_some());
        };
        frame(&session, &mut dialog);
        play(&mut session, &SCHOLARS_MATE);
        frame(&session, &mut dialog);
        assert!(dialog.is_open());
        dialog.close();
        frame(&session, &mut dialog);
        assert!(!dialog.is_open());
        session.undo();
        frame(&session, &mut dialog);
        play(&mut session, &["h5f7"]);
        frame(&session, &mut dialog);
        assert!(dialog.is_open(), "novo fim reabre o diálogo");
    }
}
