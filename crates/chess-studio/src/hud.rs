//! HUD virtual (UI-06, US-9..11): sobreposição desenhada **na Apresentação**, presa às 64
//! casas, nunca uma câmera física (GLOSSARY). Mostra setas (último Lance, melhor Lance da
//! análise, dica), casas atacadas por um lado e peças ameaçadas ou indefesas.
//!
//! O que aparece sai de [`compute_hud`], função pura sobre a Posição exibida, os ajustes do
//! usuário ([`HudSettings`]) e o que a Política permite agora ([`Allowance`], `hud_policy`):
//! as escolhas do usuário nunca ampliam a política. Cálculos de ataque vivem no
//! `chess-core`; a forma das setas, em `chess-input`. Aqui nada toca a Partida (ADR-0001).

use crate::hud_policy::{Allowance, effective};
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::{CanonicalPosition, Color as Side, Square};
use serde::{Deserialize, Serialize};

/// De qual lado destacar as casas atacadas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackedSide {
    #[default]
    Off,
    White,
    Black,
}

impl AttackedSide {
    pub const ALL: [AttackedSide; 3] =
        [AttackedSide::Off, AttackedSide::White, AttackedSide::Black];

    pub fn label_key(self) -> &'static str {
        match self {
            AttackedSide::Off => "hud-attacked-off",
            AttackedSide::White => "hud-attacked-white",
            AttackedSide::Black => "hud-attacked-black",
        }
    }

    pub fn side(self) -> Option<Side> {
        match self {
            AttackedSide::Off => None,
            AttackedSide::White => Some(Side::White),
            AttackedSide::Black => Some(Side::Black),
        }
    }
}

/// Paleta do HUD: todas discretas, só mudam o contraste e o daltonismo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HudPalette {
    #[default]
    Discreet,
    Contrast,
    /// Azul e laranja, distinguíveis no daltonismo vermelho-verde.
    ColorBlind,
}

impl HudPalette {
    pub const ALL: [HudPalette; 3] = [
        HudPalette::Discreet,
        HudPalette::Contrast,
        HudPalette::ColorBlind,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            HudPalette::Discreet => "hud-palette-discreet",
            HudPalette::Contrast => "hud-palette-contrast",
            HudPalette::ColorBlind => "hud-palette-colorblind",
        }
    }

    /// Cor RGB (0..=1) de cada elemento, sem transparência.
    pub fn rgb(self, element: HudElement) -> [f32; 3] {
        use HudElement::*;
        match (self, element) {
            (HudPalette::Discreet, LastMoveArrow) => [0.85, 0.72, 0.25],
            (HudPalette::Discreet, EngineArrow) => [0.30, 0.55, 0.85],
            (HudPalette::Discreet, HintArrow) => [0.35, 0.75, 0.45],
            (HudPalette::Discreet, Attacked) => [0.25, 0.55, 0.85],
            (HudPalette::Discreet, Threatened) => [0.85, 0.25, 0.25],
            (HudPalette::Discreet, Undefended) => [0.70, 0.45, 0.80],
            (HudPalette::Contrast, LastMoveArrow) => [1.00, 0.90, 0.10],
            (HudPalette::Contrast, EngineArrow) => [0.10, 0.45, 1.00],
            (HudPalette::Contrast, HintArrow) => [0.10, 0.90, 0.30],
            (HudPalette::Contrast, Attacked) => [1.00, 0.45, 0.10],
            (HudPalette::Contrast, Threatened) => [1.00, 0.10, 0.10],
            (HudPalette::Contrast, Undefended) => [0.85, 0.20, 1.00],
            (HudPalette::ColorBlind, LastMoveArrow) => [0.95, 0.90, 0.25],
            (HudPalette::ColorBlind, EngineArrow) => [0.00, 0.45, 0.70],
            (HudPalette::ColorBlind, HintArrow) => [0.35, 0.70, 0.90],
            (HudPalette::ColorBlind, Attacked) => [0.90, 0.60, 0.00],
            (HudPalette::ColorBlind, Threatened) => [0.84, 0.37, 0.00],
            (HudPalette::ColorBlind, Undefended) => [0.80, 0.47, 0.65],
        }
    }
}

/// Elementos desenhados pelo HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HudElement {
    LastMoveArrow,
    EngineArrow,
    HintArrow,
    Attacked,
    Threatened,
    Undefended,
}

/// Perfil de HUD nomeado; `Custom` é qualquer ajuste individual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudProfile {
    Off,
    Study,
    Analysis,
    Custom,
}

impl HudProfile {
    pub fn label_key(self) -> &'static str {
        match self {
            HudProfile::Off => "hud-profile-off",
            HudProfile::Study => "hud-profile-study",
            HudProfile::Analysis => "hud-profile-analysis",
            HudProfile::Custom => "hud-profile-custom",
        }
    }
}

/// Faixa permitida da opacidade (percentual): nunca invisível nem opaco demais.
pub const OPACITY_RANGE: std::ops::RangeInclusive<u8> = 10..=90;

/// Ajustes do HUD: cada elemento liga e desliga sozinho. O perfil é derivado destes
/// valores, como nos Auxílios. Persistidos como preferência do usuário.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HudSettings {
    /// Chave geral (tecla H): desligada, sobram só mesa, tabuleiro e peças (UI-05).
    pub enabled: bool,
    pub last_move_arrow: bool,
    /// Seta do melhor Lance da análise da Engine (quando houver).
    pub engine_arrow: bool,
    pub attacked: AttackedSide,
    pub threatened: bool,
    pub undefended: bool,
    /// Dica de Lance (seta verde).
    pub hint: bool,
    /// Opacidade em percentual, dentro de [`OPACITY_RANGE`].
    pub opacity_pct: u8,
    pub palette: HudPalette,
}

impl Default for HudSettings {
    /// Primeira execução: HUD desligado (a mesa física não tem sobreposição).
    fn default() -> Self {
        Self::off()
    }
}

impl HudSettings {
    const fn base() -> Self {
        Self {
            enabled: true,
            last_move_arrow: false,
            engine_arrow: false,
            attacked: AttackedSide::Off,
            threatened: false,
            undefended: false,
            hint: false,
            opacity_pct: 45,
            palette: HudPalette::Discreet,
        }
    }

    /// Nenhum elemento ligado.
    pub const fn off() -> Self {
        Self::base()
    }

    /// "Estudo": seta do último Lance, peças ameaçadas e dica.
    pub const fn study() -> Self {
        Self {
            last_move_arrow: true,
            threatened: true,
            hint: true,
            ..Self::base()
        }
    }

    /// "Análise": tudo, com as casas atacadas pelas brancas.
    pub const fn analysis() -> Self {
        Self {
            last_move_arrow: true,
            engine_arrow: true,
            attacked: AttackedSide::White,
            threatened: true,
            undefended: true,
            hint: true,
            ..Self::base()
        }
    }

    pub fn for_profile(profile: HudProfile) -> Option<Self> {
        match profile {
            HudProfile::Off => Some(Self::off()),
            HudProfile::Study => Some(Self::study()),
            HudProfile::Analysis => Some(Self::analysis()),
            HudProfile::Custom => None,
        }
    }

    /// Perfil que estes ajustes representam. Transparência, paleta e a chave geral são
    /// aparência: não tiram o perfil do lugar.
    pub fn profile(&self) -> HudProfile {
        let same = |p: Self| {
            Self {
                enabled: p.enabled,
                opacity_pct: p.opacity_pct,
                palette: p.palette,
                ..*self
            } == p
        };
        if same(Self::off()) {
            HudProfile::Off
        } else if same(Self::study()) {
            HudProfile::Study
        } else if same(Self::analysis()) {
            HudProfile::Analysis
        } else {
            HudProfile::Custom
        }
    }

    pub fn opacity(&self) -> f32 {
        f32::from(
            self.opacity_pct
                .clamp(*OPACITY_RANGE.start(), *OPACITY_RANGE.end()),
        ) / 100.0
    }
}

/// Melhor Lance de uma análise, válido para uma Posição (gancho para o painel Análise,
/// C8). O HUD só desenha se o FEN confere com a posição exibida.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct HudAnalysis {
    pub best: Option<AnalysisBest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisBest {
    /// FEN da posição analisada (`CanonicalPosition::to_fen`).
    pub fen: String,
    pub from: Square,
    pub to: Square,
}

impl HudAnalysis {
    fn best_for(&self, position: &CanonicalPosition) -> Option<(Square, Square)> {
        let best = self.best.as_ref()?;
        (best.fen == position.to_fen()).then_some((best.from, best.to))
    }
}

/// Seta do HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudArrow {
    pub from: Square,
    pub to: Square,
    pub element: HudElement,
}

/// O que desenhar, já filtrado pelos ajustes e pela política.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct HudMarks {
    pub arrows: Vec<HudArrow>,
    pub attacked: Vec<Square>,
    pub threatened: Vec<Square>,
    pub undefended: Vec<Square>,
}

impl HudMarks {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        *self == HudMarks::default()
    }
}

/// Posição exibida (a atual ou a de uma revisão) e o último Lance dela.
#[derive(Clone, Copy, Debug)]
pub struct HudShown<'a> {
    pub position: &'a CanonicalPosition,
    pub last_move: Option<(Square, Square)>,
}

/// Marcações do HUD para a posição exibida. Função pura: quem decide o que é permitido é
/// `allowed`, e `settings` só escolhe dentro dele.
pub fn compute_hud(
    settings: &HudSettings,
    allowed: &Allowance,
    shown: HudShown<'_>,
    analysis: &HudAnalysis,
) -> HudMarks {
    let mut marks = HudMarks::default();
    if !settings.enabled {
        return marks;
    }
    let position = shown.position;
    if settings.last_move_arrow
        && allowed.last_move
        && let Some((from, to)) = shown.last_move
    {
        marks.arrows.push(HudArrow {
            from,
            to,
            element: HudElement::LastMoveArrow,
        });
    }
    if settings.engine_arrow
        && allowed.engine_line
        && allowed.suggestion_arrows
        && let Some((from, to)) = analysis.best_for(position)
    {
        marks.arrows.push(HudArrow {
            from,
            to,
            element: HudElement::EngineArrow,
        });
    }
    if settings.hint
        && allowed.hints
        && allowed.suggestion_arrows
        && let Some(mv) = position.suggest_hint()
    {
        marks.arrows.push(HudArrow {
            from: mv.from(),
            to: mv.to(),
            element: HudElement::HintArrow,
        });
    }
    if let Some(side) = settings.attacked.side()
        && allowed.attacked_squares
    {
        marks.attacked = position.attacked_squares(side);
    }
    if allowed.threats {
        if settings.threatened {
            for side in [Side::White, Side::Black] {
                marks.threatened.extend(position.threatened_pieces(side));
            }
        }
        if settings.undefended {
            for side in [Side::White, Side::Black] {
                marks.undefended.extend(position.undefended_pieces(side));
            }
        }
    }
    marks
}

/// Conjunto do sistema que calcula [`HudMarks`]; o desenho roda depois dele.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct HudSet;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudSettings>()
            .init_resource::<HudAnalysis>()
            .init_resource::<HudMarks>()
            .add_systems(Update, update_hud.in_set(HudSet).chain().after(PlaySet));
    }
}

/// Chave de cache de [`update_hud`].
type HudKey = (u64, HudSettings, Allowance, HudAnalysis);

fn update_hud(
    session: Res<Session>,
    settings: Res<HudSettings>,
    analysis: Res<HudAnalysis>,
    fair: Option<Res<crate::fair_play::FairPlay>>,
    mut marks: ResMut<HudMarks>,
    mut key: Local<Option<HudKey>>,
) {
    // Sem o ponto único da Trava (apps parciais de teste), nada que calcula aparece.
    let allowed = match fair.as_deref() {
        Some(fair) => effective(&session, fair),
        None => crate::hud_policy::allowance(&session).without_computation(),
    };
    let now = (session.revision(), *settings, allowed, analysis.clone());
    if key.as_ref() == Some(&now) {
        return;
    }
    *key = Some(now);
    let snapshot = session.snapshot();
    let next = match session.review() {
        Some(view) => compute_hud(
            &settings,
            &allowed,
            HudShown {
                position: &view.position,
                last_move: view.last_move.as_ref().map(|h| (h.mv.from(), h.mv.to())),
            },
            &analysis,
        ),
        None => compute_hud(
            &settings,
            &allowed,
            HudShown {
                position: &snapshot.position,
                last_move: snapshot.history.last().map(|h| (h.mv.from(), h.mv.to())),
            },
            &analysis,
        ),
    };
    if *marks != next {
        *marks = next;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::focus::ViewState;
    use chess_game::PolicyKind;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn shown<'a>(pos: &'a CanonicalPosition, last: Option<(&str, &str)>) -> HudShown<'a> {
        HudShown {
            position: pos,
            last_move: last.map(|(a, b)| (sq(a), sq(b))),
        }
    }

    fn all_on() -> HudSettings {
        HudSettings::analysis()
    }

    /// Posição com cavalo preto atacando a torre e o peão brancos, e a dama preta livre
    /// para ser capturada: dá algo a mostrar em cada elemento.
    fn busy() -> CanonicalPosition {
        CanonicalPosition::from_fen("4k3/8/8/3q4/8/2n5/R3P3/3RK3 w - - 0 1").unwrap()
    }

    fn analysis_for(pos: &CanonicalPosition) -> HudAnalysis {
        HudAnalysis {
            best: Some(AnalysisBest {
                fen: pos.to_fen(),
                from: sq("d1"),
                to: sq("d5"),
            }),
        }
    }

    #[test]
    fn default_hud_shows_nothing() {
        let pos = busy();
        let marks = compute_hud(
            &HudSettings::default(),
            &Allowance::ALL,
            shown(&pos, Some(("e3", "e2"))),
            &analysis_for(&pos),
        );
        assert!(marks.is_empty());
    }

    #[test]
    fn assisted_with_everything_on_draws_every_element() {
        let pos = busy();
        let marks = compute_hud(
            &all_on(),
            &Allowance::ALL,
            shown(&pos, Some(("e3", "e2"))),
            &analysis_for(&pos),
        );
        let kinds: Vec<HudElement> = marks.arrows.iter().map(|a| a.element).collect();
        assert_eq!(
            kinds,
            [
                HudElement::LastMoveArrow,
                HudElement::EngineArrow,
                HudElement::HintArrow
            ]
        );
        assert!(!marks.attacked.is_empty());
        assert!(marks.threatened.contains(&sq("a2")));
        assert!(marks.undefended.contains(&sq("c3")));
    }

    /// UI-08, US-11: nas competitivas em andamento, nenhum perfil de HUD ativo mostra algo
    /// vedado; o último Lance (permitido) segue o ajuste.
    #[test]
    fn competitive_live_blocks_everything_but_the_last_move_arrow() {
        let pos = busy();
        let marks = compute_hud(
            &all_on(),
            &Allowance::for_game(
                PolicyKind::FideSimulation,
                crate::hud_policy::GameStage::InProgress,
            ),
            shown(&pos, Some(("e3", "e2"))),
            &analysis_for(&pos),
        );
        assert_eq!(
            marks.arrows,
            [HudArrow {
                from: sq("e3"),
                to: sq("e2"),
                element: HudElement::LastMoveArrow
            }]
        );
        assert!(marks.attacked.is_empty());
        assert!(marks.threatened.is_empty() && marks.undefended.is_empty());
    }

    #[test]
    fn competitive_finished_or_review_releases_everything() {
        use crate::hud_policy::GameStage::{Finished, Review};
        let pos = busy();
        let free = compute_hud(
            &all_on(),
            &Allowance::ALL,
            shown(&pos, Some(("e3", "e2"))),
            &analysis_for(&pos),
        );
        for stage in [Finished, Review] {
            let allowed = Allowance::for_game(PolicyKind::SuddenElimination, stage);
            let marks = compute_hud(
                &all_on(),
                &allowed,
                shown(&pos, Some(("e3", "e2"))),
                &analysis_for(&pos),
            );
            assert_eq!(marks, free, "{stage:?}");
        }
    }

    #[test]
    fn each_element_is_independent() {
        let pos = busy();
        let analysis = analysis_for(&pos);
        let only = |f: fn(&mut HudSettings)| {
            let mut s = HudSettings::off();
            f(&mut s);
            compute_hud(
                &s,
                &Allowance::ALL,
                shown(&pos, Some(("e3", "e2"))),
                &analysis,
            )
        };
        let m = only(|s| s.last_move_arrow = true);
        assert_eq!(m.arrows.len(), 1);
        assert!(m.attacked.is_empty() && m.threatened.is_empty());
        let m = only(|s| s.engine_arrow = true);
        assert_eq!(m.arrows[0].element, HudElement::EngineArrow);
        let m = only(|s| s.hint = true);
        assert_eq!(m.arrows[0].element, HudElement::HintArrow);
        assert_eq!(
            (m.arrows[0].from, m.arrows[0].to),
            (sq("d1"), sq("d5")),
            "captura da dama livre"
        );
        let m = only(|s| s.attacked = AttackedSide::Black);
        assert_eq!(m.attacked, pos.attacked_squares(Side::Black));
        assert!(m.arrows.is_empty());
        let m = only(|s| s.threatened = true);
        assert!(!m.threatened.is_empty() && m.undefended.is_empty());
        let m = only(|s| s.undefended = true);
        assert!(!m.undefended.is_empty() && m.threatened.is_empty());
    }

    #[test]
    fn engine_arrow_needs_an_analysis_for_the_same_position() {
        let pos = busy();
        let other = CanonicalPosition::start();
        let mut s = HudSettings::off();
        s.engine_arrow = true;
        let marks = compute_hud(
            &s,
            &Allowance::ALL,
            shown(&pos, None),
            &analysis_for(&other),
        );
        assert!(
            marks.arrows.is_empty(),
            "análise de outra posição não aparece"
        );
        let marks = compute_hud(
            &s,
            &Allowance::ALL,
            shown(&pos, None),
            &HudAnalysis::default(),
        );
        assert!(marks.arrows.is_empty(), "sem análise não há seta");
    }

    #[test]
    fn master_switch_hides_everything_without_losing_the_settings() {
        let pos = busy();
        let mut s = all_on();
        s.enabled = false;
        let marks = compute_hud(
            &s,
            &Allowance::ALL,
            shown(&pos, Some(("e3", "e2"))),
            &analysis_for(&pos),
        );
        assert!(marks.is_empty());
        assert_eq!(
            s.profile(),
            HudProfile::Analysis,
            "a chave geral não muda o perfil"
        );
    }

    #[test]
    fn profiles_resolve_and_touching_one_element_makes_it_custom() {
        assert_eq!(HudSettings::default().profile(), HudProfile::Off);
        assert_eq!(HudSettings::study().profile(), HudProfile::Study);
        assert_eq!(HudSettings::analysis().profile(), HudProfile::Analysis);
        assert!(HudSettings::for_profile(HudProfile::Custom).is_none());
        for base in [
            HudSettings::off(),
            HudSettings::study(),
            HudSettings::analysis(),
        ] {
            let toggles: [fn(&mut HudSettings); 6] = [
                |s| s.last_move_arrow = !s.last_move_arrow,
                |s| s.engine_arrow = !s.engine_arrow,
                |s| s.threatened = !s.threatened,
                |s| s.undefended = !s.undefended,
                |s| s.hint = !s.hint,
                |s| {
                    s.attacked = if s.attacked == AttackedSide::Off {
                        AttackedSide::Black
                    } else {
                        AttackedSide::Off
                    }
                },
            ];
            for toggle in toggles {
                let mut s = base;
                toggle(&mut s);
                assert_eq!(s.profile(), HudProfile::Custom);
            }
            let mut s = base;
            s.opacity_pct = 20;
            s.palette = HudPalette::ColorBlind;
            assert_eq!(s.profile(), base.profile(), "aparência não muda o perfil");
        }
    }

    #[test]
    fn opacity_is_clamped_to_the_discreet_range() {
        let mut s = HudSettings::off();
        s.opacity_pct = 0;
        assert!((s.opacity() - 0.10).abs() < 1e-6);
        s.opacity_pct = 255;
        assert!((s.opacity() - 0.90).abs() < 1e-6);
        s.opacity_pct = 45;
        assert!((s.opacity() - 0.45).abs() < 1e-6);
    }

    #[test]
    fn settings_survive_a_json_round_trip_and_missing_fields_use_defaults() {
        let s = HudSettings {
            palette: HudPalette::Contrast,
            opacity_pct: 30,
            ..HudSettings::analysis()
        };
        let back: HudSettings = serde_json::from_value(serde_json::json!(s)).unwrap();
        assert_eq!(back, s);
        let partial: HudSettings = serde_json::from_str(r#"{"hint": true}"#).unwrap();
        assert!(partial.hint && partial.enabled && !partial.last_move_arrow);
    }

    #[test]
    fn palettes_give_each_element_a_distinct_color() {
        use HudElement::*;
        for palette in HudPalette::ALL {
            let colors: Vec<[f32; 3]> = [
                LastMoveArrow,
                EngineArrow,
                HintArrow,
                Attacked,
                Threatened,
                Undefended,
            ]
            .iter()
            .map(|e| palette.rgb(*e))
            .collect();
            for (i, a) in colors.iter().enumerate() {
                for b in &colors[i + 1..] {
                    assert_ne!(a, b, "{palette:?}");
                }
            }
        }
    }

    /// Sistema real: o HUD acompanha a Partida, a política e a análise.
    fn hud_app(policy: PolicyKind) -> App {
        let mut app = App::new();
        app.init_resource::<HudSettings>()
            .init_resource::<HudAnalysis>()
            .init_resource::<HudMarks>()
            .insert_resource(crate::fair_play::FairPlay::new(
                chess_net::FairPlayGuard::new(),
            ))
            .add_systems(Update, update_hud);
        let config = crate::new_game::NewGameConfig::for_policy(policy);
        let mut session = Session::new();
        session.start(&config, None);
        app.insert_resource(session);
        app
    }

    #[test]
    fn system_hides_forbidden_elements_in_a_live_competitive_game() {
        let mut app = hud_app(PolicyKind::FideSimulation);
        *app.world_mut().resource_mut::<HudSettings>() = HudSettings::analysis();
        app.update();
        let marks = app.world().resource::<HudMarks>().clone();
        assert!(marks.attacked.is_empty() && marks.threatened.is_empty());
        assert!(
            marks
                .arrows
                .iter()
                .all(|a| a.element == HudElement::LastMoveArrow)
        );
    }

    #[test]
    fn system_shows_attacked_squares_in_an_assisted_game() {
        let mut app = hud_app(PolicyKind::Assisted);
        *app.world_mut().resource_mut::<HudSettings>() = HudSettings::analysis();
        app.update();
        assert!(!app.world().resource::<HudMarks>().attacked.is_empty());
    }

    /// UI-05: o Foco não esconde o HUD permitido, e a chave geral deixa só mesa,
    /// tabuleiro e peças.
    #[test]
    fn focus_keeps_the_allowed_hud_and_the_master_switch_clears_it() {
        let mut app = hud_app(PolicyKind::Assisted);
        app.insert_resource(ViewState {
            focus: true,
            fullscreen: false,
        });
        *app.world_mut().resource_mut::<HudSettings>() = HudSettings::analysis();
        app.update();
        assert!(
            !app.world().resource::<HudMarks>().attacked.is_empty(),
            "o Foco não esconde o HUD permitido"
        );
        app.world_mut().resource_mut::<HudSettings>().enabled = false;
        app.update();
        assert!(app.world().resource::<HudMarks>().is_empty());
    }

    #[test]
    fn h_key_toggles_the_master_switch() {
        let mut app = crate::actions::test_app();
        for expected in [false, true] {
            crate::actions::press(&mut app, KeyCode::KeyH);
            assert_eq!(app.world().resource::<HudSettings>().enabled, expected);
        }
    }
}
