//! A Posição que a Análise, as Aberturas e o Explorador seguem (#325, decisão 13 da #320).
//!
//! Sem Modo de estudo é a de sempre: a da Revisão (ou, sem Revisão, a ao vivo) da Partida, que
//! não muda quando uma variante da Análise está em exibição. Com o Modo de estudo é a Posição
//! da Linha de estudo no Lance mostrado. Os três serviços leem daqui, então seguem a linha, e
//! ao sair do modo voltam à Partida sem nenhum caso especial.
//!
//! Só leitura (ADR-0001): nada aqui escreve na Partida nem no log (ADR-0002).

use crate::notation;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::{CanonicalPosition, Move};
use chess_uci::PositionSpec;

/// A Posição seguida, com o caminho até ela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    /// A Posição.
    pub position: CanonicalPosition,
    /// Meio-lance da Partida de onde ela parte: o da Revisão ou, sem Revisão, o último.
    pub ply: usize,
    /// Meios-lances da Partida.
    pub total: usize,
    /// Os Lances da Linha de estudo até o Lance mostrado (vazio fora do estudo).
    pub study: Vec<Move>,
}

impl Followed {
    /// A Posição que os serviços seguem agora. `None` só se a Partida não se refaz.
    pub fn of(session: &Session) -> Option<Self> {
        let ply = notation::current_ply(session);
        let total = session.snapshot().history.len();
        match session.study_line() {
            Some(line) => Some(Self {
                position: line.position().clone(),
                ply,
                total,
                study: line.moves()[..line.cursor()].iter().map(|m| m.mv).collect(),
            }),
            None => Some(Self {
                position: session.game().review_at_ply(ply)?.position,
                ply,
                total,
                study: Vec::new(),
            }),
        }
    }

    /// O pedido UCI da Posição: a inicial da Partida, os Lances dela até `ply` e os da Linha de
    /// estudo (a Engine recebe o caminho, não só a Posição).
    pub fn spec(&self, session: &Session) -> Option<PositionSpec> {
        let start = session.game().review_at_ply(0)?.position;
        let game_moves = session
            .snapshot()
            .history
            .iter()
            .take(self.ply)
            .map(|h| h.mv);
        Some(
            PositionSpec::from_fen(start.to_fen())
                .with_moves(game_moves.chain(self.study.iter().copied())),
        )
    }
}

/// A Linha de estudo só existe enquanto o Modo de estudo é permitido (decisão 4 da #320): se a
/// recusa passa a valer com um estudo em curso, o estudo se encerra e os serviços voltam à
/// Partida. Roda antes de qualquer serviço que calcula, a cada quadro.
pub fn end_study_when_refused(mut session: ResMut<Session>) {
    if session.studying() && session.study_refusal().is_some() {
        session.leave_study();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};

    fn mv(uci: &str) -> Move {
        uci.parse().unwrap()
    }

    /// Partida Assistida com os Lances `uci`, mostrada na Revisão do meio-lance `review`.
    fn session_at(uci: &[&str], review: usize) -> Session {
        let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
        for (i, m) in uci.iter().enumerate() {
            let m = mv(m);
            game.handle(
                Command::Release {
                    from: m.from(),
                    to: m.to(),
                },
                Timestamp::mono(1000 * (i as u64 + 1)),
            )
            .unwrap();
        }
        let mut session = Session::from_game(game);
        session.set_review_ply(Some(review));
        session
    }

    #[test]
    fn without_study_the_followed_position_is_the_reviewed_one() {
        let session = session_at(&["e2e4", "e7e5", "g1f3"], 1);
        let followed = Followed::of(&session).unwrap();
        assert_eq!(followed.ply, 1);
        assert_eq!(followed.total, 3);
        assert!(followed.study.is_empty());
        assert_eq!(
            followed.position.to_fen(),
            session.game().review_at_ply(1).unwrap().position.to_fen()
        );
    }

    #[test]
    fn in_study_the_followed_position_is_the_study_line_and_the_request_carries_the_path() {
        let mut session = session_at(&["e2e4", "e7e5", "g1f3"], 1);
        assert!(session.study_play(mv("c7c5")));
        assert!(session.study_play(mv("g1f3")));
        let followed = Followed::of(&session).unwrap();
        assert_eq!(followed.ply, 1, "parte do meio-lance da Partida");
        assert_eq!(followed.study, vec![mv("c7c5"), mv("g1f3")]);
        assert_eq!(
            followed.position.to_fen(),
            "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );
        let spec = followed.spec(&session).unwrap();
        assert_eq!(
            spec.moves,
            vec![mv("e2e4"), mv("c7c5"), mv("g1f3")],
            "os Lances da Partida até o ponto e os da linha"
        );

        // Navegar na linha muda a Posição seguida; a Partida segue intacta.
        session.study_step_back();
        let back = Followed::of(&session).unwrap();
        assert_eq!(back.study, vec![mv("c7c5")]);
        session.study_to_start();
        assert_eq!(
            Followed::of(&session).unwrap().position.to_fen(),
            session.game().review_at_ply(1).unwrap().position.to_fen()
        );
        assert_eq!(session.game().snapshot().history.len(), 3);
    }

    #[test]
    fn leaving_the_study_brings_the_followed_position_back_to_the_game() {
        let mut session = session_at(&["e2e4", "e7e5", "g1f3"], 1);
        assert!(session.study_play(mv("c7c5")));
        assert_ne!(Followed::of(&session).unwrap().study, vec![]);
        session.leave_study();
        let after = Followed::of(&session).unwrap();
        assert!(after.study.is_empty());
        assert_eq!(after.ply, 3, "sem Revisão, o último meio-lance da Partida");
        assert_eq!(
            after.position.to_fen(),
            session.snapshot().position.to_fen()
        );
    }

    #[test]
    fn a_study_move_must_be_legal() {
        let mut session = session_at(&["e2e4", "e7e5"], 1);
        assert!(!session.study_play(mv("e7e4")), "Lance ilegal não entra");
        assert!(!session.studying());
        assert!(session.study_play(mv("e7e5")));
        assert!(session.studying());
        assert_eq!(session.game().snapshot().history.len(), 2);
    }
}
