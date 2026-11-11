//! Tela de Nova Partida (AC-01): janela egui aberta antes da Partida, onde o usuário escolhe
//! a Política de arbitragem, a etapa da Eliminação Imediata, o controle de tempo, o lado, o
//! adversário e o Perfil de auxílios. A configuração é um dado puro ([`NewGameConfig`]) que
//! vira o `GameSetup` do domínio; a janela só a edita e a entrega à `Session`.
//!
//! Integrações:
//! - **Engine (B9/B10):** "Engine" está sempre selecionável. Sem Engine configurada, escolhê-la
//!   (ou iniciar assim) abre o assistente de Engine (`engine_wizard`) em vez de só desabilitar;
//!   com o caminho escolhido, `Session::start` entrega a Engine à Partida, inclusive nas
//!   Políticas competitivas (o relógio dela é acionado pelo `EngineMove`).
//! - **Perfil de auxílios (B4):** "Jogo físico"/"Treinamento" gravam as `AidSettings` (que a
//!   persistência guarda); "Manter os atuais" ([`AidProfile::Custom`]) não as altera.

use crate::aids::{AidProfile, AidSettings};
use crate::camera::CameraRig;
use crate::engine::{EngineUi, MAX_ELO, MIN_ELO};
use crate::fair_play::{Computation, FairPlay, FairPlayLocked, LockScope};
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;
use chess_game::{
    CLASSICAL_MIN_BASE_MS, EngineSeat, GameSetup, InfractionStage, PolicyKind, TimeControl,
};
use chess_input::CameraView;

/// Contra quem o usuário joga.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Opponent {
    /// Dois jogadores no mesmo computador.
    #[default]
    TwoLocal,
    /// Engine UCI (B9); sem executável configurado, a tela abre o assistente (B10).
    Engine,
}

/// Tudo o que a tela de Nova Partida define.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewGameConfig {
    pub policy: PolicyKind,
    /// Só vale na Eliminação Imediata (DEC-36).
    pub infraction_stage: InfractionStage,
    /// Controle de tempo livre (base + incremento); `None` = sem relógio.
    pub time: Option<TimeControl>,
    /// Lado do usuário (decide a Vista inicial e de que lado fica o relógio).
    pub user_side: Side,
    pub opponent: Opponent,
    /// Elo pedido à Engine (só vale com `Opponent::Engine`).
    pub engine_elo: u32,
    /// Perfil de auxílios; `Custom` mantém os Auxílios atuais.
    pub aid_profile: AidProfile,
}

impl Default for NewGameConfig {
    fn default() -> Self {
        Self {
            policy: PolicyKind::Assisted,
            infraction_stage: InfractionStage::Clock,
            time: None,
            user_side: Side::White,
            opponent: Opponent::TwoLocal,
            engine_elo: 1500,
            aid_profile: AidProfile::Training,
        }
    }
}

impl NewGameConfig {
    /// Configuração padrão de uma Política (o controle de tempo clássico nas competitivas).
    pub fn for_policy(policy: PolicyKind) -> Self {
        Self {
            policy,
            time: (policy == PolicyKind::FideSimulation).then_some(CLASSIC),
            ..Self::default()
        }
    }

    /// Configuração da Partida no domínio.
    pub fn to_setup(&self) -> GameSetup {
        let mut setup = GameSetup::standard(self.policy);
        if let Some(time) = self.time {
            setup = setup.with_time_control(time);
        }
        if self.policy == PolicyKind::SuddenElimination {
            setup = setup.with_infraction_stage(self.infraction_stage);
        }
        if self.opponent == Opponent::Engine {
            setup = setup.with_engine(EngineSeat {
                color: self.user_side.opposite(),
                elo: self.engine_elo,
            });
        }
        setup
    }

    /// Configuração que descreve um `GameSetup` (Partida reaberta do log). O Perfil de
    /// auxílios é preferência do usuário, não da Partida: fica "Manter os atuais".
    pub fn from_setup(setup: &GameSetup) -> Self {
        let engine = setup.engine();
        Self {
            policy: setup.policy(),
            infraction_stage: setup.infraction_stage(),
            time: setup.time_control(),
            // Online, o lado do usuário é o que o Provedor deu a ele.
            user_side: match (engine, setup.online_game()) {
                (_, Some(online)) => online.local_color,
                (Some(seat), None) => seat.color.opposite(),
                (None, None) => Side::White,
            },
            opponent: if engine.is_some() {
                Opponent::Engine
            } else {
                Opponent::TwoLocal
            },
            engine_elo: engine.map_or(1500, |seat| seat.elo),
            aid_profile: AidProfile::Custom,
        }
    }

    /// Aviso RULE-05: Simulação FIDE com menos de 60 min por jogador.
    pub fn non_classical_notice(&self) -> bool {
        self.to_setup().non_classical_notice()
    }

    /// Vista inicial: o usuário sempre vê o tabuleiro do seu lado.
    pub fn initial_view(&self) -> CameraView {
        match self.user_side {
            Side::White => CameraView::Standard,
            Side::Black => CameraView::Inverted,
        }
    }
}

/// 90 min + 30 s por lance: o controle clássico (Leis da FIDE, partidas clássicas).
pub const CLASSIC: TimeControl = TimeControl {
    base_ms: 90 * 60_000,
    increment_ms: 30_000,
};

/// Atalhos de controle de tempo: (chave i18n, valor).
const PRESETS: [(&str, Option<TimeControl>); 4] = [
    ("newgame-preset-classic", Some(CLASSIC)),
    (
        "newgame-preset-rapid",
        Some(TimeControl {
            base_ms: 15 * 60_000,
            increment_ms: 10_000,
        }),
    ),
    (
        "newgame-preset-blitz",
        Some(TimeControl {
            base_ms: 5 * 60_000,
            increment_ms: 3_000,
        }),
    ),
    ("newgame-preset-none", None),
];

/// Estado da janela: aberta ou não, e o rascunho que o usuário edita.
#[derive(Resource, Debug)]
pub struct NewGameDialog {
    pub open: bool,
    pub draft: NewGameConfig,
    /// O rascunho já recebeu as preferências gravadas (Elo, lado, Perfil de auxílios).
    pub seeded: bool,
}

impl NewGameDialog {
    /// Reabre a tela partindo da Partida atual e dos Auxílios em uso.
    pub fn open_from(&mut self, config: &NewGameConfig, aids: &AidSettings) {
        self.draft = config.clone();
        self.draft.aid_profile = aids.profile();
        self.seeded = true;
        self.open = true;
    }
}

impl Default for NewGameDialog {
    fn default() -> Self {
        // O app abre na tela de Nova Partida: a Partida só começa depois de "Iniciar".
        Self {
            open: true,
            draft: NewGameConfig::default(),
            seeded: false,
        }
    }
}

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NewGameDialog>()
            .add_systems(EguiPrimaryContextPass, new_game_window);
    }
}

/// A Trava de fair play recusa a Engine adversária desta Nova Partida (FP-01)? Sem o ponto
/// único da Trava, recusa.
fn engine_refusal(fair: Option<&FairPlay>, draft: &NewGameConfig) -> Option<FairPlayLocked> {
    if draft.opponent != Opponent::Engine {
        return None;
    }
    match fair {
        Some(fair) => fair
            .check_game(
                Computation::OpponentEngine,
                &chess_game::Game::new(draft.to_setup()),
            )
            .err(),
        None => Some(FairPlayLocked {
            computation: Computation::OpponentEngine,
            scope: LockScope::Everything,
        }),
    }
}

fn policy_key(policy: PolicyKind) -> &'static str {
    match policy {
        PolicyKind::FideSimulation => "newgame-policy-fide",
        PolicyKind::SuddenElimination => "newgame-policy-elimination",
        _ => "newgame-policy-assisted",
    }
}

fn policy_hint_key(policy: PolicyKind) -> &'static str {
    match policy {
        PolicyKind::FideSimulation => "newgame-policy-fide-hint",
        PolicyKind::SuddenElimination => "newgame-policy-elimination-hint",
        _ => "newgame-policy-assisted-hint",
    }
}

#[allow(clippy::too_many_arguments)]
fn new_game_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut engine_ui: ResMut<EngineUi>,
    mut aids: ResMut<AidSettings>,
    mut dialog: ResMut<NewGameDialog>,
    mut session: ResMut<Session>,
    mut rig: ResMut<CameraRig>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    online: Option<ResMut<crate::online::OnlineUi>>,
    fair: Option<Res<FairPlay>>,
) -> Result {
    let fair = fair.as_deref();
    if !dialog.open || capture.as_ref().is_some_and(|c| !c.show_new_game) {
        return Ok(());
    }
    if !dialog.seeded {
        // Primeira abertura: o Elo/lado gravados e o Perfil de auxílios em uso.
        dialog.seeded = true;
        dialog.draft.engine_elo = engine_ui.elo();
        dialog.draft.user_side = engine_ui.user_side();
        dialog.draft.aid_profile = aids.profile();
    }
    let mut draft = dialog.draft.clone();
    let refusal = engine_refusal(fair, &draft);
    let mut start = false;
    let mut cancel = false;
    let mut go_online = false;
    egui::Window::new(i18n.t("newgame-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
            policy_section(ui, &i18n, &mut draft);
            ui.separator();
            time_section(ui, &i18n, &mut draft);
            ui.separator();
            players_section(ui, &i18n, &mut draft, &mut engine_ui);
            ui.separator();
            // Início > Nova Partida > Online (Lichess): a tela própria da Partida online.
            if ui
                .add(
                    crate::ui_theme::CsButton::new(&i18n.t("action-new-game-online"))
                        .icon(egui_phosphor::regular::GLOBE)
                        .variant(crate::ui_theme::ButtonVariant::Subtle),
                )
                .on_hover_text(i18n.t("action-new-game-online-desc"))
                .clicked()
            {
                go_online = true;
            }
            ui.separator();
            // Trava de fair play (FP-01): contra a Engine, só com a licença.
            if let Some(locked) = refusal {
                let p = crate::ui_theme::palette(ui);
                ui.colored_label(p.warning, i18n.t("engine-fairplay-title"));
                ui.label(i18n.t(locked.message_key()));
            }
            ui.horizontal(|ui| {
                let button = ui.add_enabled(
                    refusal.is_none(),
                    egui::Button::new(i18n.t("newgame-start")),
                );
                if button.clicked() {
                    start = true;
                }
                if ui.button(i18n.t("newgame-cancel")).clicked() {
                    cancel = true;
                }
            });
        });
    if start {
        let engine_config = engine_ui
            .engine_config()
            .filter(|_| draft.opponent == Opponent::Engine);
        if draft.opponent == Opponent::Engine && engine_config.is_none() {
            // Sem Engine não há Partida contra ela: abre o assistente (B10) e fica na tela.
            engine_ui.request_wizard();
        } else {
            // Contra a Engine, o serviço pede a licença da Trava de fair play (FP-01):
            // recusada, nada muda e a tela continua aberta, com o motivo.
            let started = match (engine_config, fair) {
                (Some(engine), Some(fair)) => session.start_vs_engine(&draft, engine, fair).is_ok(),
                (Some(_), None) => false,
                (None, _) => {
                    session.start(&draft);
                    true
                }
            };
            if started {
                let view = draft.initial_view();
                if draft.opponent == Opponent::Engine {
                    engine_ui.set_choice(draft.user_side, draft.engine_elo);
                }
                if let Some(profile) = AidSettings::for_profile(draft.aid_profile) {
                    // Ajuste explícito do usuário: a persistência grava a preferência.
                    aids.set_if_neq(profile);
                }
                rig.pose = view.pose();
                rig.view = Some(view);
                dialog.open = false;
            }
        }
    } else if cancel {
        dialog.open = false;
    } else if go_online && let Some(mut online) = online {
        dialog.open = false;
        online.open_at(crate::online::Tab::Seek);
    }
    if dialog.draft != draft {
        dialog.draft = draft;
    }
    Ok(())
}

fn policy_section(ui: &mut egui::Ui, i18n: &I18n, draft: &mut NewGameConfig) {
    ui.strong(i18n.t("newgame-policy-label"));
    for policy in [
        PolicyKind::Assisted,
        PolicyKind::FideSimulation,
        PolicyKind::SuddenElimination,
    ] {
        if ui
            .radio(draft.policy == policy, i18n.t(policy_key(policy)))
            .clicked()
            && draft.policy != policy
        {
            // Trocar de Política sugere o controle de tempo dela; o usuário pode mudar.
            draft.time = NewGameConfig::for_policy(policy).time;
            draft.policy = policy;
        }
    }
    ui.small(i18n.t(policy_hint_key(draft.policy)));
    if draft.policy == PolicyKind::SuddenElimination {
        ui.colored_label(egui::Color32::LIGHT_YELLOW, i18n.t("newgame-custom-rule"));
        ui.label(i18n.t("newgame-stage-label"));
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut draft.infraction_stage,
                InfractionStage::Clock,
                i18n.t("newgame-stage-clock"),
            );
            ui.radio_value(
                &mut draft.infraction_stage,
                InfractionStage::Release,
                i18n.t("newgame-stage-release"),
            );
        });
    }
}

fn time_section(ui: &mut egui::Ui, i18n: &I18n, draft: &mut NewGameConfig) {
    ui.strong(i18n.t("newgame-time-label"));
    ui.horizontal(|ui| {
        for (key, preset) in PRESETS {
            if ui
                .selectable_label(draft.time == preset, i18n.t(key))
                .clicked()
            {
                draft.time = preset;
            }
        }
    });
    if let Some(time) = draft.time.as_mut() {
        let mut minutes = u32::try_from(time.base_ms / 60_000).unwrap_or(u32::MAX);
        let mut seconds = u32::try_from(time.increment_ms / 1000).unwrap_or(u32::MAX);
        ui.horizontal(|ui| {
            ui.label(i18n.t("newgame-base-minutes"));
            ui.add(egui::DragValue::new(&mut minutes).range(1..=600));
            ui.label(i18n.t("newgame-increment-seconds"));
            ui.add(egui::DragValue::new(&mut seconds).range(0..=300));
        });
        time.base_ms = u64::from(minutes) * 60_000;
        time.increment_ms = u64::from(seconds) * 1000;
    }
    if draft.non_classical_notice() {
        ui.colored_label(
            egui::Color32::LIGHT_YELLOW,
            i18n.t_with(
                "newgame-rule05-notice",
                &[("minutes", &(CLASSICAL_MIN_BASE_MS / 60_000).to_string())],
            ),
        );
    }
}

fn players_section(
    ui: &mut egui::Ui,
    i18n: &I18n,
    draft: &mut NewGameConfig,
    engine: &mut EngineUi,
) {
    ui.strong(i18n.t("newgame-side-label"));
    ui.horizontal(|ui| {
        ui.radio_value(
            &mut draft.user_side,
            Side::White,
            i18n.t("newgame-side-white"),
        );
        ui.radio_value(
            &mut draft.user_side,
            Side::Black,
            i18n.t("newgame-side-black"),
        );
    });
    ui.strong(i18n.t("newgame-opponent-label"));
    ui.horizontal(|ui| {
        ui.radio_value(
            &mut draft.opponent,
            Opponent::TwoLocal,
            i18n.t("newgame-opponent-local"),
        );
        let was_local = draft.opponent == Opponent::TwoLocal;
        ui.radio_value(
            &mut draft.opponent,
            Opponent::Engine,
            i18n.t("newgame-opponent-engine"),
        );
        if was_local && draft.opponent == Opponent::Engine && !engine.has_engine() {
            // Sem Engine disponível: em vez de só desabilitar, abre o assistente (B10).
            engine.request_wizard();
        }
    });
    if draft.opponent == Opponent::Engine {
        ui.add(
            egui::Slider::new(&mut draft.engine_elo, MIN_ELO..=MAX_ELO)
                .text(i18n.t("engine-elo-label")),
        );
        if engine.has_engine() {
            ui.small(i18n.t("newgame-engine-ready"));
        } else {
            ui.colored_label(egui::Color32::YELLOW, i18n.t("newgame-engine-unavailable"));
            if ui.button(i18n.t("wizard-open")).clicked() {
                engine.request_wizard();
            }
        }
    }
    ui.strong(i18n.t("newgame-aids-label"));
    ui.horizontal(|ui| {
        for (profile, key) in [
            (AidProfile::PhysicalGame, "newgame-aids-physical"),
            (AidProfile::Training, "newgame-aids-training"),
            (AidProfile::Custom, "newgame-aids-keep"),
        ] {
            ui.radio_value(&mut draft.aid_profile, profile, i18n.t(key));
        }
    });
    ui.small(i18n.t("newgame-aids-hint"));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn each_policy_is_selectable_and_reaches_the_setup() {
        for policy in [
            PolicyKind::Assisted,
            PolicyKind::FideSimulation,
            PolicyKind::SuddenElimination,
        ] {
            assert_eq!(
                NewGameConfig::for_policy(policy).to_setup().policy(),
                policy
            );
        }
    }

    #[test]
    fn elimination_stage_is_recorded_in_the_setup() {
        for stage in [InfractionStage::Clock, InfractionStage::Release] {
            let config = NewGameConfig {
                policy: PolicyKind::SuddenElimination,
                infraction_stage: stage,
                ..NewGameConfig::default()
            };
            assert_eq!(config.to_setup().infraction_stage(), stage);
        }
        // Em outras Políticas a etapa escolhida não vaza para o setup.
        let fide = NewGameConfig {
            policy: PolicyKind::FideSimulation,
            infraction_stage: InfractionStage::Release,
            ..NewGameConfig::default()
        };
        assert_eq!(fide.to_setup().infraction_stage(), InfractionStage::Clock);
    }

    #[test]
    fn free_time_control_reaches_the_setup() {
        let config = NewGameConfig {
            time: Some(TimeControl {
                base_ms: 7 * 60_000,
                increment_ms: 2_000,
            }),
            ..NewGameConfig::default()
        };
        let tc = config.to_setup().time_control().unwrap();
        assert_eq!((tc.base_ms, tc.increment_ms), (420_000, 2_000));
        assert!(NewGameConfig::default().to_setup().time_control().is_none());
    }

    #[test]
    fn rule_05_notice_only_below_sixty_minutes_in_fide_simulation() {
        let fide = |base_min: u64| NewGameConfig {
            policy: PolicyKind::FideSimulation,
            time: Some(TimeControl {
                base_ms: base_min * 60_000,
                increment_ms: 0,
            }),
            ..NewGameConfig::default()
        };
        assert!(fide(59).non_classical_notice());
        assert!(!fide(60).non_classical_notice());
        assert!(!NewGameConfig::for_policy(PolicyKind::FideSimulation).non_classical_notice());
        let assisted = NewGameConfig {
            time: fide(5).time,
            ..NewGameConfig::default()
        };
        assert!(!assisted.non_classical_notice());
    }

    #[test]
    fn user_side_picks_the_initial_view() {
        let black = NewGameConfig {
            user_side: Side::Black,
            ..NewGameConfig::default()
        };
        assert_eq!(black.initial_view(), CameraView::Inverted);
        assert_eq!(
            NewGameConfig::default().initial_view(),
            CameraView::Standard
        );
    }

    #[test]
    fn local_play_is_the_default_opponent() {
        assert_eq!(NewGameConfig::default().opponent, Opponent::TwoLocal);
    }

    #[test]
    fn engine_seat_goes_to_the_setup_and_back_for_every_policy() {
        for policy in [
            PolicyKind::Assisted,
            PolicyKind::FideSimulation,
            PolicyKind::SuddenElimination,
        ] {
            let config = NewGameConfig {
                opponent: Opponent::Engine,
                user_side: Side::Black,
                engine_elo: 1800,
                aid_profile: AidProfile::Custom,
                ..NewGameConfig::for_policy(policy)
            };
            let setup = config.to_setup();
            let seat = setup.engine().unwrap();
            assert_eq!((seat.color, seat.elo), (Side::White, 1800));
            assert_eq!(NewGameConfig::from_setup(&setup), config);
        }
    }
}
