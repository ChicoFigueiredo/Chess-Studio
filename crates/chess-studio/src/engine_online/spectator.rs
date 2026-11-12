//! "Ver no tabuleiro" (#302): a Partida da Engine online no tabuleiro, só para ver.
//!
//! Cada Partida em andamento na tela da Engine online abre o **modo espectador**: o tabuleiro
//! (3D e 2D) mostra a Posição que o Lichess confirmou e acompanha os Lances conforme o serviço
//! da Engine online os confirma. A Partida vista é uma cópia do log que o controlador escreveu
//! (ADR-0001): nenhum gesto vira Lance nem intenção, a Trava de fair play não liga (é a Engine
//! jogando, AD-05) e nada é gravado de novo (quem grava é o serviço). A Sessão do usuário fica
//! guardada por baixo ([`Session::own`]) e volta ao tabuleiro ao sair.
//!
//! Com uma Partida própria em andamento no tabuleiro, o app pergunta antes de trocar a vista;
//! a Partida dele não é encerrada (a local, com relógio correndo, fica em Pausa).

use super::service::EngineOnline;
use super::{EngineOnlineService, EngineOnlineUi};
use crate::i18n::I18n;
use crate::session::Session;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, StatusTone, cs_card_frame, cs_status_badge, palette,
    strong_font,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use chess_core::Color;
use chess_game::Game;
use egui_phosphor::regular as icon;

/// Estado da interface do modo espectador.
#[derive(Resource, Debug, Default)]
pub struct SpectatorUi {
    /// "Ver no tabuleiro" à espera da confirmação (a Partida do usuário está em andamento): o
    /// id da Partida da Engine no Lichess.
    pub confirm: Option<String>,
    /// Cena de demonstração das capturas: a pergunta aparece, os botões não agem.
    pub demo: bool,
}

/// O que um pedido de "Ver no tabuleiro" deu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    /// O tabuleiro mostra a Partida da Engine.
    Watching,
    /// A Partida do usuário está em andamento: a pergunta abriu e nada mudou ainda.
    Confirm,
    /// O serviço não tem (mais) essa Partida: nada muda.
    Gone,
}

/// A Partida do usuário no tabuleiro está em andamento (sem resultado e já começada, ou online)?
/// Já no modo espectador, trocar a Partida vista não pergunta nada.
pub fn needs_confirmation(session: &Session) -> bool {
    if session.is_spectator() {
        return false;
    }
    let snapshot = session.snapshot();
    !snapshot.is_over() && (session.is_online() || !snapshot.history.is_empty())
}

/// O relógio local do usuário está correndo (a Partida dele fica em Pausa enquanto ele assiste)?
/// Online o relógio é do Provedor e não para.
pub fn clock_running(session: &Session) -> bool {
    !session.is_online()
        && !session.snapshot().is_over()
        && session
            .clock_reading()
            .is_some_and(|r| r.running.is_some() && !r.paused)
}

/// "Ver no tabuleiro" da Partida `id`: pergunta antes se a Partida do usuário está em andamento.
pub fn request(ui: &mut SpectatorUi, session: &mut Session, run: &EngineOnline, id: &str) -> Watch {
    if run.game(id).is_none() {
        return Watch::Gone;
    }
    if needs_confirmation(session) {
        ui.confirm = Some(id.to_owned());
        return Watch::Confirm;
    }
    watch(session, run, id)
}

/// O usuário confirmou: o tabuleiro passa a mostrar a Partida da Engine.
pub fn confirm(ui: &mut SpectatorUi, session: &mut Session, run: &EngineOnline) -> Watch {
    match ui.confirm.take() {
        Some(id) => watch(session, run, &id),
        None => Watch::Gone,
    }
}

/// O usuário desistiu: nada muda.
pub fn cancel(ui: &mut SpectatorUi) {
    ui.confirm = None;
}

fn watch(session: &mut Session, run: &EngineOnline, id: &str) -> Watch {
    let Some(game) = run.game(id) else {
        return Watch::Gone;
    };
    // A Partida local do usuário não perde tempo enquanto ele assiste: Pausa, um Comando dele
    // gravado como sempre (a Partida não é encerrada).
    if clock_running(session) {
        session.toggle_pause();
    }
    if session.watch(game) {
        Watch::Watching
    } else {
        Watch::Gone
    }
}

/// A Partida vista, se ela andou desde a última cópia.
fn fresh_game(session: &Session, run: &EngineOnline) -> Option<Game> {
    if !session.is_spectator() {
        return None;
    }
    let id = session.online_remote_id()?;
    run.game(id)
        .filter(|g| g.events().len() != session.game().events().len())
        .cloned()
}

/// Um quadro do modo espectador: copia os Lances que o Lichess confirmou na Partida vista.
/// Devolve `true` se o tabuleiro mudou.
pub fn follow(session: &mut Session, run: &EngineOnline) -> bool {
    match fresh_game(session, run) {
        Some(game) => {
            session.follow_watched(&game);
            true
        }
        None => false,
    }
}

/// O quadro do modo espectador no app, depois do quadro da Engine online.
pub fn follow_spectator(
    service: Res<EngineOnlineService>,
    ui: Res<EngineOnlineUi>,
    mut session: ResMut<Session>,
) {
    if ui.demo.is_some() || !session.is_spectator() {
        return;
    }
    // Só marca a Sessão como mudada quando há Lance novo (a cena não se redesenha à toa).
    if service.with(|run| follow(session.bypass_change_detection(), run)) {
        session.set_changed();
    }
}

/// O que a faixa do modo espectador conta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    /// A Engine joga de brancas.
    pub engine_white: bool,
    pub opponent: String,
    pub rating: Option<u32>,
    /// A Partida terminou (o resultado, se o Lichess deu um).
    pub ended: Option<Option<String>>,
    /// O serviço não acompanha mais a Partida (sem fim confirmado): o tabuleiro fica no último
    /// Lance confirmado.
    pub gone: bool,
}

impl Banner {
    /// A faixa, no modo espectador. `run` é o serviço (nas capturas, nenhum).
    pub fn of(session: &Session, run: Option<&EngineOnline>) -> Option<Self> {
        if !session.is_spectator() {
            return None;
        }
        let online = session.game().setup().online_game()?;
        let engine_white = online.local_color == Color::White;
        let opponent = if engine_white {
            &online.black
        } else {
            &online.white
        };
        let snapshot = session.snapshot();
        let ended = snapshot.is_over().then(|| {
            snapshot
                .outcome
                .as_ref()
                .map(|o| o.result.pgn_token().to_owned())
        });
        let gone = ended.is_none() && run.is_some_and(|r| r.game(&online.remote_id).is_none());
        Some(Self {
            engine_white,
            opponent: crate::display_text::clean_display(&opponent.name),
            rating: opponent.rating,
            ended,
            gone,
        })
    }

    /// "A Engine joga de Brancas contra Salma (1662) no Lichess."
    pub fn line(&self, i18n: &I18n) -> String {
        let side = if self.engine_white {
            i18n.t("engine-online-side-white")
        } else {
            i18n.t("engine-online-side-black")
        };
        let opponent = match self.rating {
            Some(rating) => format!("{} ({rating})", self.opponent),
            None => self.opponent.clone(),
        };
        i18n.t_with(
            "engine-online-spectator-line",
            &[("side", &side), ("opponent", &opponent)],
        )
    }
}

/// Respiro entre a faixa, a borda da área do tabuleiro e o tabuleiro.
const BANNER_MARGIN: f32 = 16.0;
/// Larguras do cartão ao lado do tabuleiro: a menor que ainda cabe e a maior.
const BANNER_MIN_W: f32 = 232.0;
const BANNER_MAX_W: f32 = 320.0;

/// Como a faixa aparece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BannerLayout {
    /// Cartão ao lado do tabuleiro, com esta largura (a do cartão inteiro): selos, quem joga, o
    /// aviso de só leitura e "Sair do modo espectador".
    Card { width: f32 },
    /// Sem espaço ao lado: uma linha só no alto da área (selos e "Sair do modo espectador"),
    /// acima do tabuleiro; quem joga fica na dica dos selos e no Painel Partida online.
    Chip,
}

/// Onde a faixa fica e como: no canto de cima da área do tabuleiro, ao lado dele quando há
/// espaço (no 2D técnico e nas janelas largas, sem cobrir nenhuma casa); sem espaço, numa
/// linha só, acima do tabuleiro. `board` é a caixa do tabuleiro visto pela câmera, se conhecida.
pub fn banner_place(area: egui::Rect, board: Option<egui::Rect>) -> (egui::Pos2, BannerLayout) {
    let at = egui::pos2(area.left() + BANNER_MARGIN, area.top() + BANNER_MARGIN);
    let room = board.map_or(0.0, |b| b.left() - area.left() - 2.0 * BANNER_MARGIN);
    if room >= BANNER_MIN_W {
        let width = room.min(BANNER_MAX_W);
        (at, BannerLayout::Card { width })
    } else {
        (egui::pos2(at.x, area.top() + 8.0), BannerLayout::Chip)
    }
}

/// Os dois selos: é a Engine online, e só para ver. A dica conta quem joga.
fn badges(ui: &mut egui::Ui, i18n: &I18n, banner: &Banner) {
    let line = banner.line(i18n);
    cs_status_badge(
        ui,
        StatusTone::Accent,
        icon::ROBOT,
        &i18n.t("engine-online-spectator-engine"),
    )
    .on_hover_text(&line);
    cs_status_badge(
        ui,
        StatusTone::Info,
        icon::EYE,
        &i18n.t("engine-online-spectator-badge"),
    )
    .on_hover_text(&line);
}

/// "Partida terminada: 1-0." ou "A Engine online não acompanha mais…", quando é o caso.
fn ended_text(i18n: &I18n, banner: &Banner) -> Option<(String, bool)> {
    match &banner.ended {
        Some(Some(result)) => Some((
            i18n.t_with("engine-online-spectator-ended", &[("result", result)]),
            false,
        )),
        Some(None) => Some((i18n.t("engine-online-spectator-ended-plain"), false)),
        None if banner.gone => Some((i18n.t("engine-online-spectator-gone"), true)),
        None => None,
    }
}

fn leave_button(ui: &mut egui::Ui, i18n: &I18n) -> bool {
    ui.add(
        CsButton::new(&i18n.t("engine-online-watch-leave"))
            .icon(icon::ARROW_U_UP_LEFT)
            .variant(ButtonVariant::Secondary),
    )
    .clicked()
}

/// O cartão: selos, quem joga, o fim (se houve), o aviso de só leitura e "Sair do modo
/// espectador". Devolve `true` no clique em sair.
pub fn banner_body(ui: &mut egui::Ui, i18n: &I18n, banner: &Banner) -> bool {
    let p = palette(ui);
    ui.spacing_mut().item_spacing.y = crate::panel_chrome::token("space-1", 4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        badges(ui, i18n, banner);
    });
    ui.label(
        egui::RichText::new(banner.line(i18n))
            .font(strong_font(ui.ctx(), 14.0))
            .color(p.ink),
    );
    if let Some((text, warning)) = ended_text(i18n, banner) {
        ui.label(egui::RichText::new(text).color(if warning { p.warning } else { p.ink }));
    }
    ui.label(
        egui::RichText::new(i18n.t("engine-online-spectator-readonly"))
            .text_style(egui::TextStyle::Small)
            .color(p.ink_muted),
    );
    ui.add_space(4.0);
    leave_button(ui, i18n)
}

/// A linha só: selos, o fim (se houve) e "Sair do modo espectador".
pub fn chip_body(ui: &mut egui::Ui, i18n: &I18n, banner: &Banner) -> bool {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        badges(ui, i18n, banner);
        if let Some((text, warning)) = ended_text(i18n, banner) {
            ui.label(
                egui::RichText::new(text)
                    .text_style(egui::TextStyle::Small)
                    .color(if warning { p.warning } else { p.ink }),
            );
        }
        leave_button(ui, i18n)
    })
    .inner
}

/// A faixa do modo espectador, no canto de cima da área do tabuleiro (os avisos do Lichess
/// ficam no outro canto).
#[allow(clippy::too_many_arguments)]
pub fn spectator_banner(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    service: Res<EngineOnlineService>,
    ui_state: Res<EngineOnlineUi>,
    mut session: ResMut<Session>,
    dock: Option<Res<crate::dock::Dock>>,
    cameras: crate::eval_bar::BoardCameras,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
) -> Result {
    if !session.is_spectator() {
        return Ok(());
    }
    let banner = if ui_state.demo.is_some() {
        Banner::of(&session, None)
    } else {
        service.with(|run| Banner::of(&session, Some(run)))
    };
    let Some(banner) = banner else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let scale_factor = window.map_or(1.0, |w| w.scale_factor());
    let board = crate::eval_bar::projected_board(&cameras, scale_factor, ctx.pixels_per_point());
    let (at, layout) = dock
        .as_deref()
        .and_then(crate::dock::Dock::board_area)
        .map_or((egui::pos2(16.0, 140.0), BannerLayout::Chip), |area| {
            banner_place(area, board)
        });
    let mut leave = false;
    egui::Area::new(egui::Id::new("engine-online-spectator"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::LEFT_TOP)
        .fixed_pos(at)
        .show(&ctx, |ui| {
            let p = palette(ui);
            let frame = cs_card_frame(p, CardEmphasis::Full).shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: p.scrim,
            });
            match layout {
                BannerLayout::Card { width } => {
                    let content = width - frame.total_margin().sum().x;
                    frame.show(ui, |ui| {
                        ui.set_width(content);
                        leave = banner_body(ui, &i18n, &banner);
                    });
                }
                BannerLayout::Chip => {
                    frame
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| leave = chip_body(ui, &i18n, &banner));
                }
            }
        });
    if leave && ui_state.demo.is_none() {
        session.leave_spectator();
    }
    Ok(())
}

/// A pergunta antes de trocar a vista, com a Partida do usuário em andamento.
pub fn confirm_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut spectator: ResMut<SpectatorUi>,
    mut ui_state: ResMut<EngineOnlineUi>,
    service: Res<EngineOnlineService>,
    mut session: ResMut<Session>,
) -> Result {
    if spectator.confirm.is_none() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut choice = None;
    let body = if session.is_online() {
        i18n.t("engine-online-confirm-online")
    } else if clock_running(&session) {
        format!(
            "{} {}",
            i18n.t("engine-online-confirm-local"),
            i18n.t("engine-online-confirm-paused")
        )
    } else {
        i18n.t("engine-online-confirm-local")
    };
    egui::Window::new(i18n.t("engine-online-confirm-title"))
        .id(egui::Id::new("engine-online-spectator-confirm"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.set_width(super::view::WINDOW_W * 0.8);
            ui.label(body);
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-online-confirm-act"))
                            .icon(icon::EYE)
                            .variant(ButtonVariant::Primary),
                    )
                    .clicked()
                {
                    choice = Some(true);
                }
                if ui
                    .add(CsButton::new(&i18n.t("engine-online-cancel")))
                    .clicked()
                {
                    choice = Some(false);
                }
            });
        });
    if choice.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(false);
    }
    if spectator.demo {
        return Ok(());
    }
    match choice {
        Some(true) => {
            let watched = service.with(|run| confirm(&mut spectator, &mut session, run));
            if watched == Watch::Watching {
                // O tabuleiro é o que importa agora: a tela da Engine sai da frente dele.
                ui_state.open = false;
            }
        }
        Some(false) => cancel(&mut spectator),
        None => {}
    }
    Ok(())
}
