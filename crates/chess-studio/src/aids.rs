//! Auxílios visuais e Perfis de auxílios (UX-01, UX-03, UX-04, DEC-49).
//!
//! Um Auxílio é marcação que não existiria na mesa física: destinos legais, peças
//! fantasmas, destaque da peça selecionada, último Lance e indicação de xeque, mais a
//! política de explicação de erro. Tudo aqui é **só leitura** da Partida (ADR-0001): os
//! Auxílios nunca mudam legalidade, penalidades nem Eventos (UX-03). A lógica é pura
//! ([`AidSettings`], [`compute_marks`]); os sistemas Bevy apenas desenham o resultado.

use crate::hud_policy::Allowance;
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::{CanonicalPosition, Color as Side, Piece, Square};
use chess_game::IllegalReason;
use serde::{Deserialize, Serialize};

/// Quando explicar por que um Lance foi ilegal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorExplanation {
    /// O motivo aparece assim que o Lance ilegal é solto.
    #[default]
    Immediate,
    /// A peça volta e o aviso oferece "Por quê?"; o motivo só aparece se pedido.
    AfterAttempt,
    /// Nada durante a Partida; as ocorrências ilegais ficam na revisão.
    ReviewOnly,
}

impl ErrorExplanation {
    pub const ALL: [ErrorExplanation; 3] = [
        ErrorExplanation::Immediate,
        ErrorExplanation::AfterAttempt,
        ErrorExplanation::ReviewOnly,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            ErrorExplanation::Immediate => "aid-explanation-immediate",
            ErrorExplanation::AfterAttempt => "aid-explanation-after-attempt",
            ErrorExplanation::ReviewOnly => "aid-explanation-review-only",
        }
    }
}

/// Perfil de auxílios nomeado; `Custom` é qualquer ajuste individual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AidProfile {
    PhysicalGame,
    Training,
    Custom,
}

impl AidProfile {
    pub fn label_key(self) -> &'static str {
        match self {
            AidProfile::PhysicalGame => "aid-profile-physical",
            AidProfile::Training => "aid-profile-training",
            AidProfile::Custom => "aid-profile-custom",
        }
    }
}

/// Cada Auxílio liga e desliga separadamente. O Perfil é derivado destes valores, então
/// mexer em qualquer um transforma o Perfil em "Personalizado" sem estado extra.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AidSettings {
    /// Pontos nos destinos legais da peça selecionada.
    pub destinations: bool,
    /// Peças fantasmas (a peça selecionada, translúcida) nos destinos livres.
    pub ghosts: bool,
    /// Destaque da peça selecionada (e da casa sob o cursor).
    pub selected: bool,
    /// Destaque do último Lance.
    pub last_move: bool,
    /// Indicação do rei em xeque.
    pub check: bool,
    pub explanation: ErrorExplanation,
}

impl Default for AidSettings {
    /// Padrão da primeira execução: Treinamento (DEC-49).
    fn default() -> Self {
        Self::training()
    }
}

impl AidSettings {
    /// "Jogo físico": nenhum Auxílio; o erro só aparece na revisão.
    pub const fn physical() -> Self {
        Self {
            destinations: false,
            ghosts: false,
            selected: false,
            last_move: false,
            check: false,
            explanation: ErrorExplanation::ReviewOnly,
        }
    }

    /// "Treinamento": destinos, seleção, último Lance, xeque e explicação imediata.
    pub const fn training() -> Self {
        Self {
            destinations: true,
            ghosts: false,
            selected: true,
            last_move: true,
            check: true,
            explanation: ErrorExplanation::Immediate,
        }
    }

    pub fn for_profile(profile: AidProfile) -> Option<Self> {
        match profile {
            AidProfile::PhysicalGame => Some(Self::physical()),
            AidProfile::Training => Some(Self::training()),
            AidProfile::Custom => None,
        }
    }

    /// Perfil que estes ajustes representam.
    pub fn profile(&self) -> AidProfile {
        if *self == Self::physical() {
            AidProfile::PhysicalGame
        } else if *self == Self::training() {
            AidProfile::Training
        } else {
            AidProfile::Custom
        }
    }

    /// Algum Auxílio de marcação no tabuleiro está ligado?
    #[cfg(test)]
    pub fn any_mark(&self) -> bool {
        self.destinations || self.ghosts || self.selected || self.last_move || self.check
    }
}

/// Peça fantasma: a peça selecionada, translúcida, num destino livre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ghost {
    pub square: Square,
    pub side: Side,
    pub piece: Piece,
}

/// O que desenhar no tabuleiro, já filtrado pelos Auxílios ligados.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub destinations: Vec<Square>,
    pub ghosts: Vec<Ghost>,
    pub selected: Option<Square>,
    pub last_move: Option<(Square, Square)>,
    pub check: Option<Square>,
}

/// Posição exibida (a atual ou a de uma revisão) com o que os Auxílios precisam dela.
#[derive(Clone, Copy, Debug)]
pub struct ShownPosition<'a> {
    pub position: &'a CanonicalPosition,
    pub side_to_move: Side,
    pub in_check: bool,
    pub last_move: Option<(Square, Square)>,
}

/// Marcações para a posição exibida. `origin` é a peça selecionada/arrastada e
/// `legal_destinations` os destinos legais dela (vazio em revisão). Função pura: não
/// toca a Partida.
pub fn compute_marks(
    settings: &AidSettings,
    shown: ShownPosition<'_>,
    origin: Option<Square>,
    legal_destinations: &[Square],
) -> Marks {
    let mut marks = Marks::default();
    if settings.destinations {
        marks.destinations = legal_destinations.to_vec();
    }
    if settings.ghosts
        && let Some((side, piece)) = origin.and_then(|o| shown.position.piece_at(o))
    {
        marks.ghosts = legal_destinations
            .iter()
            .filter(|sq| shown.position.piece_at(**sq).is_none())
            .map(|&square| Ghost {
                square,
                side,
                piece,
            })
            .collect();
    }
    if settings.selected {
        marks.selected = origin;
    }
    if settings.last_move {
        marks.last_move = shown.last_move;
    }
    if settings.check && shown.in_check {
        marks.check = king_square(shown.position, shown.side_to_move);
    }
    marks
}

fn king_square(position: &CanonicalPosition, side: Side) -> Option<Square> {
    chess_input::all_squares().find(|s| position.piece_at(*s) == Some((side, Piece::King)))
}

/// Chave i18n do motivo de um Lance ilegal (UX-04).
pub fn reason_key(reason: IllegalReason) -> &'static str {
    match reason {
        IllegalReason::EmptyOrigin => "aid-reason-empty-origin",
        IllegalReason::OpponentPiece => "aid-reason-opponent-piece",
        IllegalReason::SameSquare => "aid-reason-same-square",
        IllegalReason::NotLegal { in_check: true } => "aid-reason-in-check",
        _ => "aid-reason-not-legal",
    }
}

/// O aviso de Lance ilegal deve aparecer agora, e com ou sem motivo?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeView {
    Hidden,
    /// Só "Lance ilegal", com o botão "Por quê?".
    Brief,
    WithReason,
}

pub fn notice_view(explanation: ErrorExplanation, revealed: bool) -> NoticeView {
    match explanation {
        ErrorExplanation::Immediate => NoticeView::WithReason,
        ErrorExplanation::AfterAttempt if revealed => NoticeView::WithReason,
        ErrorExplanation::AfterAttempt => NoticeView::Brief,
        ErrorExplanation::ReviewOnly => NoticeView::Hidden,
    }
}

/// Conjunto do sistema que calcula [`Marks`]; os desenhistas rodam depois dele.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarksSet;

pub struct AidsPlugin;

impl Plugin for AidsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AidSettings>()
            .init_resource::<Marks>()
            .add_systems(Update, update_marks.in_set(MarksSet).after(PlaySet));
    }
}

/// Chave de cache de [`update_marks`]: revisão, seleção, Auxílios efetivos.
type MarksKey = (u64, Option<Square>, AidSettings);

/// Auxílios efetivos: o que o usuário ligou, limitado pelo que a Política permite agora
/// (`hud_policy`). Nas competitivas em andamento sobram xeque, seleção e último Lance.
pub fn effective_settings(settings: AidSettings, allowed: &Allowance) -> AidSettings {
    AidSettings {
        destinations: settings.destinations && allowed.destinations,
        ghosts: settings.ghosts && allowed.ghosts,
        selected: settings.selected && allowed.selection,
        last_move: settings.last_move && allowed.last_move,
        check: settings.check && allowed.check,
        explanation: settings.explanation,
    }
}

/// Recalcula as marcações só quando algo relevante muda (revisão, seleção, ajustes).
fn update_marks(
    session: Res<Session>,
    settings: Res<AidSettings>,
    mut marks: ResMut<Marks>,
    mut key: Local<Option<MarksKey>>,
) {
    let origin = session.gesture_visual().origin;
    // Destinos legais e fantasmas são vedados nas competitivas em andamento (DEC-32,
    // UI-08), qualquer que seja o Perfil; "Xeque!", seleção e último Lance seguem o Perfil.
    let effective = effective_settings(*settings, &crate::hud_policy::allowance(&session));
    let now = (session.revision(), origin, effective);
    if *key == Some(now) {
        return;
    }
    *key = Some(now);
    let snapshot = session.snapshot();
    let next = match session.review() {
        Some(view) => compute_marks(
            &effective,
            ShownPosition {
                position: &view.position,
                side_to_move: view.side_to_move,
                in_check: view.in_check,
                last_move: view.last_move.as_ref().map(|h| (h.mv.from(), h.mv.to())),
            },
            None,
            &[],
        ),
        None => {
            let dests = origin.map_or_else(Vec::new, |o| snapshot.legal_destinations(o));
            compute_marks(
                &effective,
                ShownPosition {
                    position: &snapshot.position,
                    side_to_move: snapshot.side_to_move,
                    in_check: snapshot.in_check,
                    // Partida online: a intenção enviada fica marcada como o último Lance
                    // (a peça já está no destino) até o Provedor confirmar ou recusar.
                    last_move: snapshot
                        .made_move
                        .as_ref()
                        .filter(|_| session.is_online())
                        .map(|m| (m.mv.from(), m.mv.to()))
                        .or_else(|| snapshot.history.last().map(|h| (h.mv.from(), h.mv.to()))),
                },
                origin,
                &dests,
            )
        }
    };
    if *marks != next {
        *marks = next;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn shown<'a>(pos: &'a CanonicalPosition, last: Option<(&str, &str)>) -> ShownPosition<'a> {
        ShownPosition {
            position: pos,
            side_to_move: pos.side_to_move(),
            in_check: pos.is_check(),
            last_move: last.map(|(a, b)| (sq(a), sq(b))),
        }
    }

    #[test]
    fn profiles_resolve_to_the_expected_aids() {
        let physical = AidSettings::for_profile(AidProfile::PhysicalGame).unwrap();
        assert!(!physical.any_mark());
        assert_eq!(physical.explanation, ErrorExplanation::ReviewOnly);
        let training = AidSettings::for_profile(AidProfile::Training).unwrap();
        assert!(training.destinations && training.selected && training.last_move && training.check);
        assert!(!training.ghosts, "fantasmas são opcionais no Treinamento");
        assert_eq!(training.explanation, ErrorExplanation::Immediate);
        assert!(AidSettings::for_profile(AidProfile::Custom).is_none());
    }

    #[test]
    fn competitive_policies_never_show_destinations_or_ghosts() {
        let mut all = AidSettings::training();
        all.ghosts = true;
        let eff = effective_settings(all, &Allowance::COMPETITIVE_LIVE);
        assert!(!eff.destinations && !eff.ghosts);
        assert!(
            eff.check && eff.selected && eff.last_move,
            "o resto segue o Perfil"
        );
        assert_eq!(effective_settings(all, &Allowance::ALL), all);
        let pos = CanonicalPosition::start();
        let dests = [sq("e3"), sq("e4")];
        let marks = compute_marks(&eff, shown(&pos, None), Some(sq("e2")), &dests);
        assert!(marks.destinations.is_empty() && marks.ghosts.is_empty());
        assert_eq!(marks.selected, Some(sq("e2")));
    }

    #[test]
    fn first_run_default_is_training() {
        assert_eq!(AidSettings::default().profile(), AidProfile::Training);
    }

    #[test]
    fn touching_any_single_aid_makes_the_profile_custom() {
        for base in [AidSettings::physical(), AidSettings::training()] {
            let toggles: [fn(&mut AidSettings); 5] = [
                |s| s.destinations = !s.destinations,
                |s| s.ghosts = !s.ghosts,
                |s| s.selected = !s.selected,
                |s| s.last_move = !s.last_move,
                |s| s.check = !s.check,
            ];
            for toggle in toggles {
                let mut s = base;
                toggle(&mut s);
                assert_eq!(s.profile(), AidProfile::Custom);
            }
            let mut s = base;
            s.explanation = if base.explanation == ErrorExplanation::AfterAttempt {
                ErrorExplanation::Immediate
            } else {
                ErrorExplanation::AfterAttempt
            };
            assert_eq!(s.profile(), AidProfile::Custom);
        }
    }

    #[test]
    fn physical_profile_removes_every_mark() {
        // UX-01: com tudo desligado, nenhuma marcação, mesmo com seleção, Lance e xeque.
        let pos = CanonicalPosition::from_fen("4k3/8/8/8/8/8/4q3/4K3 w - - 0 1").unwrap();
        assert!(pos.is_check());
        let dests = [sq("d1"), sq("e2")];
        let marks = compute_marks(
            &AidSettings::physical(),
            shown(&pos, Some(("e3", "e2"))),
            Some(sq("e1")),
            &dests,
        );
        assert_eq!(marks, Marks::default());
    }

    #[test]
    fn training_profile_marks_destinations_selection_last_move_and_check() {
        let pos = CanonicalPosition::from_fen("4k3/8/8/8/8/8/4q3/4K3 w - - 0 1").unwrap();
        let dests = [sq("d1"), sq("e2")];
        let marks = compute_marks(
            &AidSettings::training(),
            shown(&pos, Some(("e3", "e2"))),
            Some(sq("e1")),
            &dests,
        );
        assert_eq!(marks.destinations, dests);
        assert_eq!(marks.selected, Some(sq("e1")));
        assert_eq!(marks.last_move, Some((sq("e3"), sq("e2"))));
        assert_eq!(marks.check, Some(sq("e1")));
        assert!(marks.ghosts.is_empty());
    }

    #[test]
    fn each_aid_works_independently() {
        let pos = CanonicalPosition::start();
        let dests = [sq("e3"), sq("e4")];
        let only = |f: fn(&mut AidSettings)| {
            let mut s = AidSettings::physical();
            f(&mut s);
            compute_marks(&s, shown(&pos, Some(("a2", "a3"))), Some(sq("e2")), &dests)
        };
        let m = only(|s| s.destinations = true);
        assert_eq!(m.destinations.len(), 2);
        assert!(m.selected.is_none() && m.last_move.is_none() && m.ghosts.is_empty());
        let m = only(|s| s.selected = true);
        assert_eq!(m.selected, Some(sq("e2")));
        assert!(m.destinations.is_empty());
        let m = only(|s| s.last_move = true);
        assert_eq!(m.last_move, Some((sq("a2"), sq("a3"))));
        let m = only(|s| s.ghosts = true);
        assert_eq!(m.ghosts.len(), 2);
        assert!(m.destinations.is_empty());
    }

    #[test]
    fn ghosts_show_the_selected_piece_only_on_free_destinations() {
        let pos = CanonicalPosition::from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let mut s = AidSettings::physical();
        s.ghosts = true;
        let dests = [sq("e5"), sq("d5")]; // e5 livre, d5 captura
        let marks = compute_marks(&s, shown(&pos, None), Some(sq("e4")), &dests);
        assert_eq!(
            marks.ghosts,
            [Ghost {
                square: sq("e5"),
                side: Side::White,
                piece: Piece::Pawn
            }]
        );
    }

    #[test]
    fn check_is_not_marked_when_not_in_check() {
        let pos = CanonicalPosition::start();
        let marks = compute_marks(&AidSettings::training(), shown(&pos, None), None, &[]);
        assert_eq!(marks.check, None);
    }

    #[test]
    fn notice_follows_the_explanation_policy() {
        use ErrorExplanation::*;
        assert_eq!(notice_view(Immediate, false), NoticeView::WithReason);
        assert_eq!(notice_view(AfterAttempt, false), NoticeView::Brief);
        assert_eq!(notice_view(AfterAttempt, true), NoticeView::WithReason);
        assert_eq!(notice_view(ReviewOnly, true), NoticeView::Hidden);
    }

    /// UX-03: os Auxílios só leem. A mesma sequência de Comandos gera exatamente os mesmos
    /// Eventos com todos os Auxílios ligados ou desligados.
    #[test]
    fn aids_never_change_the_events_of_a_command_sequence() {
        let script = |settings: AidSettings| {
            let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
            let moves = [
                ("e2", "e4"),
                ("e7", "e5"),
                ("e1", "e5"), // ilegal
                ("f1", "c4"),
                ("b8", "c6"),
                ("d1", "h5"),
                ("g8", "f6"),
                ("h5", "f7"),
            ];
            for (i, (from, to)) in moves.iter().enumerate() {
                // Antes de cada Comando o app calcula as marcações (como no jogo real).
                let snap = game.snapshot();
                let dests = snap.legal_destinations(sq(from));
                let _ = compute_marks(
                    &settings,
                    ShownPosition {
                        position: &snap.position,
                        side_to_move: snap.side_to_move,
                        in_check: snap.in_check,
                        last_move: snap.history.last().map(|h| (h.mv.from(), h.mv.to())),
                    },
                    Some(sq(from)),
                    &dests,
                );
                game.handle(
                    Command::Release {
                        from: sq(from),
                        to: sq(to),
                    },
                    Timestamp::mono(1_000 * (i as u64 + 1)),
                )
                .unwrap();
            }
            game.events().to_vec()
        };
        let all_on = AidSettings {
            ghosts: true,
            ..AidSettings::training()
        };
        let on = script(all_on);
        let off = script(AidSettings::physical());
        assert_eq!(on, off);
        assert!(!on.is_empty());
    }

    fn tap(session: &mut Session, name: &str) {
        let square = Some(sq(name));
        session.gesture(chess_input::GestureEvent::PointerDown {
            button: chess_input::PointerButton::Primary,
            square,
            modifiers: chess_input::Modifiers::default(),
        });
        session.gesture(chess_input::GestureEvent::PointerUp { square });
    }

    /// BDD-01 e UX-04: sem destinos e fantasmas, selecionar e2 não mostra e3 nem e4, e
    /// concluir em e5 é Lance ilegal que não entra no histórico legal.
    #[test]
    fn bdd_01_physical_game_shows_no_destinations_and_e2_e5_is_illegal() {
        let mut s = Session::new();
        let aids = AidSettings::physical();
        tap(&mut s, "e2");
        assert_eq!(s.gesture_visual().origin, Some(sq("e2")));
        let snap = s.snapshot();
        let legal = snap.legal_destinations(sq("e2"));
        assert!(legal.contains(&sq("e3")) && legal.contains(&sq("e4")));
        let marks = compute_marks(
            &aids,
            ShownPosition {
                position: &snap.position,
                side_to_move: snap.side_to_move,
                in_check: snap.in_check,
                last_move: None,
            },
            s.gesture_visual().origin,
            &legal,
        );
        assert_eq!(marks, Marks::default(), "nada em e3, e4 nem em e2");

        tap(&mut s, "e5");
        assert!(s.snapshot().history.is_empty(), "e2-e5 não é Lance legal");
        let notice = s.illegal_notice().unwrap();
        assert_eq!((notice.from, notice.to), (sq("e2"), sq("e5")));
        assert_eq!(notice.reason, IllegalReason::NotLegal { in_check: false });
        // Em Jogo físico o aviso não aparece durante a Partida; fica para a revisão.
        assert_eq!(
            notice_view(aids.explanation, s.illegal_revealed()),
            NoticeView::Hidden
        );
        assert_eq!(s.illegal_incidents().len(), 1);
    }
}
