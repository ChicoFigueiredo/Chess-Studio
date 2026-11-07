//! Política de visibilidade do HUD e dos Auxílios (UI-08, US-11/19): dada a Política de
//! arbitragem e o momento da Partida, diz o que **pode** aparecer. Pura: não conhece o
//! Bevy nem a `Session`. O Perfil e os liga/desliga do usuário só escolhem dentro do que a
//! política permite; nunca o ampliam.
//!
//! | Momento                              | Vedado                                      | Permitido (conforme o Perfil) |
//! |--------------------------------------|---------------------------------------------|-------------------------------|
//! | Assistido (qualquer momento)         | nada                                        | tudo                          |
//! | Competitiva em andamento             | destinos, fantasmas, setas de sugestão, casas atacadas, ameaças, linha da Engine, dicas | xeque, seleção, último Lance |
//! | Competitiva com resultado ou revisão | nada                                        | tudo                          |

use crate::session::Session;
use chess_game::PolicyKind;

/// Em que ponto está a Partida exibida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStage {
    /// Sem resultado: ainda se joga (inclui revisar Lances de uma Partida em curso).
    InProgress,
    /// Com resultado, olhando a posição atual.
    Finished,
    /// Com resultado, revisando Lance a Lance.
    Review,
}

impl GameStage {
    /// Momento a partir de `Partida tem resultado?` e `Partida em revisão?`. Revisar uma
    /// Partida ainda em andamento não libera nada: continua `InProgress`.
    pub fn of(has_outcome: bool, reviewing: bool) -> Self {
        match (has_outcome, reviewing) {
            (false, _) => GameStage::InProgress,
            (true, false) => GameStage::Finished,
            (true, true) => GameStage::Review,
        }
    }
}

/// O que a Política permite mostrar agora. `true` = permitido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub destinations: bool,
    pub ghosts: bool,
    /// Setas de sugestão (melhor Lance).
    pub suggestion_arrows: bool,
    pub attacked_squares: bool,
    /// Peças ameaçadas e indefesas.
    pub threats: bool,
    pub engine_line: bool,
    pub hints: bool,
    /// Indicação de xeque.
    pub check: bool,
    pub selection: bool,
    pub last_move: bool,
}

impl Allowance {
    pub const ALL: Allowance = Allowance {
        destinations: true,
        ghosts: true,
        suggestion_arrows: true,
        attacked_squares: true,
        threats: true,
        engine_line: true,
        hints: true,
        check: true,
        selection: true,
        last_move: true,
    };

    /// Competitiva em andamento: só o que existiria na mesa e o Perfil escolher.
    pub const COMPETITIVE_LIVE: Allowance = Allowance {
        destinations: false,
        ghosts: false,
        suggestion_arrows: false,
        attacked_squares: false,
        threats: false,
        engine_line: false,
        hints: false,
        check: true,
        selection: true,
        last_move: true,
    };

    pub fn for_game(policy: PolicyKind, stage: GameStage) -> Allowance {
        if policy == PolicyKind::Assisted || stage != GameStage::InProgress {
            Allowance::ALL
        } else {
            Allowance::COMPETITIVE_LIVE
        }
    }

    /// Algo está vedado agora? (Competitiva em andamento.)
    pub fn restricted(&self) -> bool {
        *self != Allowance::ALL
    }
}

/// O que a Política da Partida atual permite mostrar agora.
pub fn allowance(session: &Session) -> Allowance {
    let snapshot = session.snapshot();
    let stage = GameStage::of(snapshot.outcome.is_some(), session.review().is_some());
    Allowance::for_game(snapshot.policy, stage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use GameStage::{Finished, InProgress, Review};
    use PolicyKind::{Assisted, FideSimulation, SuddenElimination};

    /// UI-08: tabela completa Política x momento.
    #[test]
    fn visibility_table_by_policy_and_stage() {
        let cases = [
            (Assisted, InProgress, Allowance::ALL),
            (Assisted, Finished, Allowance::ALL),
            (Assisted, Review, Allowance::ALL),
            (FideSimulation, InProgress, Allowance::COMPETITIVE_LIVE),
            (FideSimulation, Finished, Allowance::ALL),
            (FideSimulation, Review, Allowance::ALL),
            (SuddenElimination, InProgress, Allowance::COMPETITIVE_LIVE),
            (SuddenElimination, Finished, Allowance::ALL),
            (SuddenElimination, Review, Allowance::ALL),
        ];
        for (policy, stage, expected) in cases {
            assert_eq!(
                Allowance::for_game(policy, stage),
                expected,
                "{policy:?} / {stage:?}"
            );
        }
    }

    #[test]
    fn competitive_live_forbids_every_hint_like_aid_and_keeps_the_table_ones() {
        let a = Allowance::for_game(FideSimulation, InProgress);
        assert!(
            !a.destinations
                && !a.ghosts
                && !a.suggestion_arrows
                && !a.attacked_squares
                && !a.threats
                && !a.engine_line
                && !a.hints
        );
        assert!(a.check && a.selection && a.last_move);
        assert!(a.restricted());
        assert!(!Allowance::ALL.restricted());
    }

    #[test]
    fn stage_comes_from_outcome_and_review() {
        assert_eq!(GameStage::of(false, false), InProgress);
        assert_eq!(
            GameStage::of(false, true),
            InProgress,
            "revisar uma Partida em curso não libera a política"
        );
        assert_eq!(GameStage::of(true, false), Finished);
        assert_eq!(GameStage::of(true, true), Review);
    }
}
