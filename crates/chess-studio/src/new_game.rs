//! Tela de Nova Partida (AC-01): janela egui aberta antes da Partida, onde o usuário escolhe
//! a Política de arbitragem, a etapa da Eliminação Imediata, o controle de tempo, o lado, o
//! adversário e o Perfil de auxílios. A configuração é um dado puro ([`NewGameConfig`]) que
//! vira o `GameSetup` do domínio; a janela só a edita e a entrega à `Session`.
//!
//! Pontos de extensão:
//! - **Engine (B9):** [`Opponent::Engine`] existe, mas só é selecionável quando o recurso
//!   [`EngineAvailability`] diz que há Engine configurada. O B9 liga isso ao inserir
//!   `EngineAvailability(true)` e ao consumir `Session::opponent()` para pedir os Lances.
//! - **Perfil de auxílios (B4):** [`AidProfile`] é guardado na configuração, mas ainda não
//!   altera nada na cena (TODO(B4): ligar ao store de preferências e aos marcadores).

use crate::camera::CameraRig;
use crate::i18n::I18n;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;
use chess_game::{CLASSICAL_MIN_BASE_MS, GameSetup, InfractionStage, PolicyKind, TimeControl};
use chess_input::CameraView;

/// Contra quem o usuário joga.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Opponent {
    /// Dois jogadores no mesmo computador.
    #[default]
    TwoLocal,
    /// Engine UCI (B9). Só selecionável com [`EngineAvailability`] ligado.
    Engine,
}

/// Perfil de auxílios escolhido na Nova Partida (DEC-49).
/// TODO(B4): trocar por `chess_store::AidProfile` e aplicar os Auxílios na cena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AidProfile {
    /// Todos os Auxílios desligados.
    PhysicalGame,
    /// Destinos, seleção, último lance, xeque e explicação imediata (padrão).
    #[default]
    Training,
}

/// Há Engine configurada? O B9 insere `EngineAvailability(true)` quando houver.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EngineAvailability(pub bool);

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
        setup
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
}

impl Default for NewGameDialog {
    fn default() -> Self {
        // O app abre na tela de Nova Partida: a Partida só começa depois de "Iniciar".
        Self {
            open: true,
            draft: NewGameConfig::default(),
        }
    }
}

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NewGameDialog>()
            .init_resource::<EngineAvailability>()
            .add_systems(EguiPrimaryContextPass, new_game_window);
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
    engine: Res<EngineAvailability>,
    mut dialog: ResMut<NewGameDialog>,
    mut session: ResMut<Session>,
    mut rig: ResMut<CameraRig>,
    capture: Option<Res<crate::capture::CaptureMode>>,
) -> Result {
    if !dialog.open || capture.as_ref().is_some_and(|c| !c.show_new_game) {
        return Ok(());
    }
    let mut draft = dialog.draft.clone();
    let mut start = false;
    let mut cancel = false;
    egui::Window::new(i18n.t("newgame-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
            policy_section(ui, &i18n, &mut draft);
            ui.separator();
            time_section(ui, &i18n, &mut draft);
            ui.separator();
            players_section(ui, &i18n, &mut draft, engine.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(i18n.t("newgame-start")).clicked() {
                    start = true;
                }
                if ui.button(i18n.t("newgame-cancel")).clicked() {
                    cancel = true;
                }
            });
        });
    if start {
        let view = draft.initial_view();
        session.start(&draft);
        rig.pose = view.pose();
        rig.view = Some(view);
        dialog.open = false;
    } else if cancel {
        dialog.open = false;
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

fn players_section(ui: &mut egui::Ui, i18n: &I18n, draft: &mut NewGameConfig, engine: bool) {
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
        ui.add_enabled_ui(engine, |ui| {
            ui.radio_value(
                &mut draft.opponent,
                Opponent::Engine,
                i18n.t("newgame-opponent-engine"),
            );
        });
    });
    if !engine {
        ui.small(i18n.t("newgame-engine-unavailable"));
        if draft.opponent == Opponent::Engine {
            draft.opponent = Opponent::TwoLocal;
        }
    }
    ui.strong(i18n.t("newgame-aids-label"));
    ui.horizontal(|ui| {
        ui.radio_value(
            &mut draft.aid_profile,
            AidProfile::PhysicalGame,
            i18n.t("newgame-aids-physical"),
        );
        ui.radio_value(
            &mut draft.aid_profile,
            AidProfile::Training,
            i18n.t("newgame-aids-training"),
        );
    });
    ui.small(i18n.t("newgame-aids-pending"));
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
    fn engine_is_off_by_default_and_local_play_is_the_default_opponent() {
        assert!(!EngineAvailability::default().0);
        assert_eq!(NewGameConfig::default().opponent, Opponent::TwoLocal);
    }
}
