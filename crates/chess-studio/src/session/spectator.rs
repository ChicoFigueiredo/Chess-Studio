//! Modo espectador (#302): a Partida da Engine online no tabuleiro, só para ver.
//!
//! - Só a Partida da Engine online (`OnlineGame::is_engine_game`) entra; a de uma pessoa nunca.
//! - Só o que o Provedor confirmou: o tabuleiro mostra o log da Partida, nunca o Lance que a
//!   Engine enviou e o Lichess ainda não confirmou (a intenção enviada é volátil, PO-04).
//! - Nenhum gesto vira Lance nem intenção: a Partida do tabuleiro nunca fica ao vivo e
//!   [`Session::online_request`] não guarda nada.
//! - A Sessão do usuário fica guardada por baixo, intacta ([`Session::own`]): a persistência e
//!   a Partida online dela continuam a acompanhá-la, e ela volta ao tabuleiro ao sair. Trocar a
//!   Partida do tabuleiro (Nova Partida, Biblioteca, Partida online que começou) sai do modo
//!   espectador: a Sessão nova não guarda nada por baixo.

use super::Session;
use chess_game::{Game, OnlineGame};
use chess_input::GestureEvent;

/// Só o log: o que o Provedor confirmou.
fn confirmed(game: &Game) -> Game {
    Game::replay(game.setup().clone(), game.events())
}

fn is_engine_game(game: &Game) -> bool {
    game.setup()
        .online_game()
        .is_some_and(OnlineGame::is_engine_game)
}

impl Session {
    /// Põe no tabuleiro a Partida da Engine online `game`, só para ver. A Sessão do usuário
    /// fica guardada por baixo, como estava, e volta com [`Session::leave_spectator`]. Já no
    /// modo espectador, só troca a Partida vista. Devolve `false` (e nada muda) se `game` não
    /// é uma Partida da Engine online.
    pub fn watch(&mut self, game: &Game) -> bool {
        if !is_engine_game(game) {
            return false;
        }
        let mut fresh = Self::from_game(confirmed(game));
        fresh.interpreter.set_method(self.interpreter.method());
        // Época própria: nada que acompanha a Partida do usuário a confunde com esta.
        fresh.epoch = self.epoch.wrapping_add(1);
        fresh.revision = self.revision.wrapping_add(1);
        let mut previous = std::mem::replace(self, fresh);
        let own = match previous.spectating.take() {
            Some(own) => own,
            None => {
                // A peça na mão volta à casa: nenhum gesto em curso fica pela metade.
                previous
                    .interpreter
                    .handle(GestureEvent::Escape, &previous.snapshot);
                Box::new(previous)
            }
        };
        self.spectating = Some(own);
        true
    }

    /// Acompanha a Partida vista: copia os Lances que o Provedor confirmou desde a última vez.
    /// Fora do modo espectador, ou outra Partida, nada muda.
    pub fn follow_watched(&mut self, game: &Game) {
        if self.spectating.is_none() || game.events().len() == self.game.events().len() {
            return;
        }
        self.mirror_online(&confirmed(game), false);
    }

    /// Sai do modo espectador: a Sessão do usuário volta ao tabuleiro como estava (com o método
    /// de mover que ele usa agora). Devolve `false` fora do modo espectador.
    pub fn leave_spectator(&mut self) -> bool {
        let Some(own) = self.spectating.take() else {
            return false;
        };
        let mut own = *own;
        own.set_move_method(self.move_method());
        own.supersede(self);
        *self = own;
        true
    }

    /// O tabuleiro mostra a Partida da Engine online só para ver?
    pub fn is_spectator(&self) -> bool {
        self.spectating.is_some()
    }

    /// A Sessão do usuário: no modo espectador, a guardada por baixo; fora dele, esta. É a que a
    /// persistência grava, a que a Partida online do usuário espelha e a que segura a trava da
    /// Partida competitiva local.
    pub fn own(&self) -> &Session {
        self.spectating.as_deref().unwrap_or(self)
    }

    /// A Sessão do usuário, para o espelho da Partida online dele.
    pub fn own_mut(&mut self) -> &mut Session {
        match self.spectating {
            Some(ref mut own) => own,
            None => self,
        }
    }
}
