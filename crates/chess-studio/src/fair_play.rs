//! Trava de fair play no app (ADR-0007, CX-08, PO-08 e PO-13): o **ponto único** que todo
//! serviço que calcula consulta antes de responder. A interface só explica o que este módulo
//! decide; nenhum Painel, Ação ou opção o contorna, e não existe chave de depuração.
//!
//! # A regra
//!
//! - **Tudo travado** enquanto a [`FairPlayGuard`] da camada de rede estiver ligada (uma
//!   Partida online contra pessoa com a sessão aberta, ou a Partida competitiva local) ou
//!   enquanto houver uma Partida online em **tempo real** contra pessoa sem fim confirmado.
//! - **Correspondência** (PO-13): com uma Partida por correspondência em andamento e a sessão
//!   dela fechada (o tabuleiro mostra outra coisa), a trava vale para
//!   1. a própria Partida, em qualquer meio-lance (revisão inclusive);
//!   2. toda Posição com a mesma **estrutura** (peões dos dois lados e material por tipo e
//!      cor) da Posição atual dela ou de qualquer Posição a até dois meios-lances dela, venha
//!      de onde vier (outra Partida, FEN, Biblioteca);
//!   3. tudo o que descende disso: se a Partida mostrada passou por uma Posição travada, as
//!      seguintes também ficam.
//!
//!   As Posições já jogadas da Partida, vistas em outra Partida, não travam: não decidem mais
//!   nada nela, e a abertura (a Posição inicial, sobretudo) é comum a milhares de Partidas.
//! - Contra a Engine do Provedor não há trava; a trava só cai quando o Provedor confirma o fim.
//!
//! Serviços que consultam: análise da Engine local (linhas e barra de avaliação), dica e setas
//! do HUD, Engine remota, avaliação em cache, finais, Explorador online e a estatística da
//! Biblioteca local (Explorador e Painel Aberturas). Os que falam com a rede também passam
//! pela trava do gate (`chess-net`), que recusa tudo o que calcula enquanto ela está ligada.

use crate::session::Session;
use bevy::prelude::Resource;
use chess_core::{CanonicalPosition, Color, Piece};
use chess_game::{Game, ReviewView};
use chess_net::FairPlayGuard;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, PoisonError, RwLock};

/// Até quantos meios-lances à frente da Posição atual da correspondência a trava alcança.
pub const NEAR_PLIES: usize = 2;

/// O que um serviço quer calcular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Computation {
    /// Análise da Engine local: linhas, avaliação, barra de avaliação, seta da Engine.
    EngineAnalysis,
    /// Dica do HUD, setas de sugestão, casas atacadas e peças ameaçadas.
    Hint,
    /// Engine remota (a posição sai para um Provedor).
    RemoteEngine,
    /// Avaliação em cache de um Provedor.
    CloudEval,
    /// Finais exatos.
    Endgames,
    /// Explorador online (Mestres, Lichess, Um jogador).
    Explorer,
    /// Estatística da Biblioteca local para a Posição (Explorador e Painel Aberturas).
    LibraryStats,
}

impl Computation {
    #[cfg(test)]
    pub const ALL: [Computation; 7] = [
        Computation::EngineAnalysis,
        Computation::Hint,
        Computation::RemoteEngine,
        Computation::CloudEval,
        Computation::Endgames,
        Computation::Explorer,
        Computation::LibraryStats,
    ];
}

/// Até onde a trava vale na recusa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockScope {
    /// Partida online em tempo real (ou a sessão aberta de uma Partida online, ou a Partida
    /// competitiva local): nada calcula.
    Everything,
    /// O que está em cálculo é a própria Partida online em andamento.
    ThisGame,
    /// A Posição é de uma Partida por correspondência em andamento (ou descende de uma).
    ThisPosition,
}

/// Recusa tipada de um serviço que calcula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FairPlayLocked {
    pub computation: Computation,
    pub scope: LockScope,
}

impl fmt::Display for FairPlayLocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scope = match self.scope {
            LockScope::Everything => "há uma Partida online contra pessoa em andamento",
            LockScope::ThisGame => "esta Partida online ainda está em andamento",
            LockScope::ThisPosition => {
                "esta Posição é de uma Partida por correspondência em andamento"
            }
        };
        write!(f, "Trava de fair play: {scope}")
    }
}

impl std::error::Error for FairPlayLocked {}

/// Estrutura de uma Posição: onde estão os peões de cada lado e quanto material há de cada
/// tipo e cor. Duas Posições com a mesma estrutura diferem só por manobras de peças.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Structure(u64);

impl Structure {
    pub fn of(position: &CanonicalPosition) -> Self {
        let mut white_pawns = 0u64;
        let mut black_pawns = 0u64;
        let mut counts = [[0u8; 5]; 2];
        for square in chess_input::all_squares() {
            let Some((color, piece)) = position.piece_at(square) else {
                continue;
            };
            let bit = 1u64 << (square.rank() * 8 + square.file());
            let side = usize::from(color == Color::Black);
            match piece {
                Piece::Pawn if color == Color::White => white_pawns |= bit,
                Piece::Pawn => black_pawns |= bit,
                Piece::Knight => counts[side][0] += 1,
                Piece::Bishop => counts[side][1] += 1,
                Piece::Rook => counts[side][2] += 1,
                Piece::Queen => counts[side][3] += 1,
                Piece::King => counts[side][4] += 1,
            }
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (white_pawns, black_pawns, counts).hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// O que um serviço quer calcular: a Posição, a Partida de onde ela vem e o caminho até ela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    structure: Structure,
    /// As Posições da Partida mostrada antes desta, da inicial em diante.
    lineage: Vec<Structure>,
    /// A Partida é uma Partida online contra pessoa sem fim confirmado.
    online_in_progress: bool,
    /// `(Provedor, id remoto)` da Partida online, se for.
    online_id: Option<(String, String)>,
}

impl Subject {
    /// Uma Posição solta (FEN, Biblioteca), sem Partida.
    pub fn position(position: &CanonicalPosition) -> Self {
        Self {
            structure: Structure::of(position),
            lineage: Vec::new(),
            online_in_progress: false,
            online_id: None,
        }
    }

    /// A Posição de `game` depois de `ply` meios-lances (0 = inicial), com o caminho até ela.
    pub fn at(game: &Game, ply: usize) -> Option<Self> {
        let snapshot = game.snapshot();
        if ply > snapshot.history.len() {
            return None;
        }
        let mut position = game.setup().start().clone();
        let mut lineage = Vec::with_capacity(ply);
        for entry in snapshot.history.iter().take(ply) {
            lineage.push(Structure::of(&position));
            position = position.play(entry.mv).ok()?;
        }
        Some(Self {
            structure: Structure::of(&position),
            lineage,
            online_in_progress: game.online_human_in_progress(),
            online_id: game
                .setup()
                .online_game()
                .map(|o| (o.provider.clone(), o.remote_id.clone())),
        })
    }

    /// A Posição que o tabuleiro mostra: a da revisão, a da variante em exibição ou a ao vivo.
    pub fn shown(session: &Session) -> Option<Self> {
        let game = session.game();
        match session.review() {
            Some(view) if session.showing_variation() => Some(Self::variation(game, view)),
            Some(view) => Self::at(game, view.ply),
            None => Self::at(game, session.snapshot().history.len()),
        }
    }

    /// Uma variante que parte do meio-lance `view.ply` da Partida (Análise, Explorador).
    fn variation(game: &Game, view: &ReviewView) -> Self {
        let base = Self::at(game, view.ply.min(game.snapshot().history.len()));
        let mut subject = base.unwrap_or_else(|| Self::position(&view.position));
        subject.lineage.push(subject.structure);
        subject.structure = Structure::of(&view.position);
        subject
    }

    /// A Posição depois de `mv` a partir desta (o Lance que o Explorador mostraria).
    #[cfg(test)]
    fn then(mut self, next: &CanonicalPosition) -> Self {
        self.lineage.push(self.structure);
        self.structure = Structure::of(next);
        self
    }
}

/// Uma Partida online contra pessoa em andamento, como a trava a vê.
#[derive(Debug, Clone)]
struct Locked {
    realtime: bool,
    /// Meios-lances e fim no momento do cálculo: muda, recalcula.
    signature: (usize, bool),
    structures: HashSet<Structure>,
}

impl Locked {
    fn of(game: &Game) -> Option<Self> {
        let online = game.setup().online_game()?;
        if !game.online_human_in_progress() {
            return None;
        }
        let snapshot = game.snapshot();
        let mut structures = HashSet::new();
        let mut frontier = vec![snapshot.position.clone()];
        if let Some(sent) = snapshot.online.as_ref().and_then(|o| o.sent_move.as_ref()) {
            frontier.push(sent.position_after.clone());
        }
        for position in &frontier {
            structures.insert(Structure::of(position));
        }
        // As Posições a até `NEAR_PLIES` meios-lances da atual.
        let mut layer = frontier;
        for _ in 0..NEAR_PLIES {
            let mut next = Vec::new();
            for position in &layer {
                for mv in position.legal_moves() {
                    if let Ok(after) = position.play(mv) {
                        structures.insert(Structure::of(&after));
                        next.push(after);
                    }
                }
            }
            layer = next;
        }
        Some(Self {
            realtime: online.days_per_move.is_none(),
            signature: (snapshot.history.len(), snapshot.is_over()),
            structures,
        })
    }
}

#[derive(Debug, Default)]
struct State {
    /// Por `(Provedor, id remoto)`.
    games: BTreeMap<(String, String), Locked>,
}

/// O ponto único da Trava de fair play no app. Barato de clonar: os clones veem o mesmo
/// estado (o `NetService` cria o do app sobre a mesma [`FairPlayGuard`] da rede).
#[derive(Resource, Clone, Debug)]
pub struct FairPlay {
    guard: FairPlayGuard,
    state: Arc<RwLock<State>>,
}

impl FairPlay {
    /// A trava do app sobre a trava da camada de rede (a mesma que as sessões online seguram).
    pub fn new(guard: FairPlayGuard) -> Self {
        Self {
            guard,
            state: Arc::default(),
        }
    }

    /// A trava da camada de rede que esta reflete.
    #[cfg(test)]
    pub fn guard(&self) -> &FairPlayGuard {
        &self.guard
    }

    /// As Partidas online contra pessoa em andamento que o app conhece (do banco e das sessões
    /// abertas), por inteiro: as que não vierem somem. Partida local, contra a Engine do
    /// Provedor ou com o fim confirmado não entram.
    pub fn follow_games<'a>(&self, games: impl IntoIterator<Item = &'a Game>) {
        let Ok(mut state) = self.state.write() else {
            return;
        };
        let mut next = BTreeMap::new();
        for game in games {
            let Some(online) = game.setup().online_game() else {
                continue;
            };
            let id = (online.provider.clone(), online.remote_id.clone());
            let snapshot = game.snapshot();
            let signature = (snapshot.history.len(), snapshot.is_over());
            let reused = state
                .games
                .remove(&id)
                .filter(|l| l.signature == signature && !snapshot_has_sent_move(game));
            if let Some(locked) = reused.or_else(|| Locked::of(game)) {
                next.insert(id, locked);
            }
        }
        state.games = next;
    }

    /// Partidas online contra pessoa em andamento que a trava acompanha.
    pub fn followed(&self) -> usize {
        self.state.read().map_or(0, |s| s.games.len())
    }

    /// Nada calcula, em Posição nenhuma? (Trava da rede ligada ou Partida em tempo real.)
    pub fn everything_locked(&self) -> bool {
        self.guard.is_engaged()
            || self
                .state
                .read()
                .map_or(true, |s| s.games.values().any(|g| g.realtime))
    }

    /// Há alguma trava ligada (global ou por Partida)? Para a barra de status.
    pub fn any(&self) -> bool {
        self.everything_locked() || self.followed() > 0
    }

    /// A licença para calcular `computation` sobre `subject`, ou a recusa tipada.
    pub fn check(&self, computation: Computation, subject: &Subject) -> Result<(), FairPlayLocked> {
        let refuse = |scope| Err(FairPlayLocked { computation, scope });
        if self.guard.is_engaged() {
            return refuse(LockScope::Everything);
        }
        // Lock envenenado: na dúvida, trava.
        let Ok(state) = self.state.read().map_err(|_: PoisonError<_>| ()) else {
            return refuse(LockScope::Everything);
        };
        if state.games.values().any(|g| g.realtime) {
            return refuse(LockScope::Everything);
        }
        if subject.online_in_progress
            || subject
                .online_id
                .as_ref()
                .is_some_and(|id| state.games.contains_key(id))
        {
            return refuse(LockScope::ThisGame);
        }
        let hits = |s: &Structure| state.games.values().any(|g| g.structures.contains(s));
        if hits(&subject.structure) || subject.lineage.iter().any(hits) {
            return refuse(LockScope::ThisPosition);
        }
        Ok(())
    }

    /// Como [`FairPlay::check`], para quem só tem a Posição (FEN).
    pub fn check_fen(&self, computation: Computation, fen: &str) -> Result<(), FairPlayLocked> {
        match CanonicalPosition::from_fen(fen.trim()) {
            Ok(position) => self.check(computation, &Subject::position(&position)),
            // FEN ilegível: o serviço também o recusaria; aqui, na dúvida, trava se houver trava.
            Err(_) if self.any() => Err(FairPlayLocked {
                computation,
                scope: LockScope::Everything,
            }),
            Err(_) => Ok(()),
        }
    }
}

fn snapshot_has_sent_move(game: &Game) -> bool {
    game.sent_move().is_some()
}

// --- Serviços que calculam: a única porta de entrada no app ------------------------------

/// Estatística da Biblioteca local para a Posição (Explorador e Painel Aberturas), só com a
/// licença da trava. É a única chamada de `Store::opening_stats` no app (teste estático).
pub fn opening_stats(
    fair: &FairPlay,
    subject: &Subject,
    store: &chess_store::Store,
    profile: chess_store::ProfileId,
    fen: &str,
) -> Result<Result<Vec<chess_store::OpeningMoveStats>, chess_store::StoreError>, FairPlayLocked> {
    fair.check(Computation::LibraryStats, subject)?;
    Ok(store.opening_stats(profile, fen))
}

/// A Engine de análise atrás da trava: a segunda Engine local (`chess_uci::AnalysisController`)
/// ou a Engine remota (`chess_providers::RemoteEngine`). Só sobe e só recebe Posição com
/// licença. É o único lugar do app que as cria (teste estático).
#[derive(Debug)]
pub struct GuardedAnalysis {
    backend: Backend,
}

#[derive(Debug)]
enum Backend {
    Local(chess_uci::AnalysisController),
    /// Num `Mutex` só para o recurso ser `Sync` (as consultas da rede não são); nunca há disputa.
    Remote(std::sync::Mutex<chess_providers::RemoteEngine>),
}

fn unpoisoned(
    engine: &mut std::sync::Mutex<chess_providers::RemoteEngine>,
) -> &mut chess_providers::RemoteEngine {
    engine
        .get_mut()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl GuardedAnalysis {
    /// Sobe a Engine de análise para `subject`, se a trava deixar.
    pub fn start(
        fair: &FairPlay,
        subject: &Subject,
        config: chess_uci::AnalysisConfig,
    ) -> Result<Self, FairPlayLocked> {
        fair.check(Computation::EngineAnalysis, subject)?;
        Ok(Self {
            backend: Backend::Local(chess_uci::AnalysisController::start(config)),
        })
    }

    /// Prepara a Engine remota para `subject`, se a trava deixar. Nada sai da máquina aqui:
    /// o pedido só vai em [`GuardedAnalysis::analyze`].
    pub fn start_remote(
        fair: &FairPlay,
        subject: &Subject,
        api: chess_providers::ChessApi,
    ) -> Result<Self, FairPlayLocked> {
        fair.check(Computation::RemoteEngine, subject)?;
        Ok(Self {
            backend: Backend::Remote(std::sync::Mutex::new(chess_providers::RemoteEngine::new(
                api,
            ))),
        })
    }

    /// A Engine é a remota (a Posição sai da máquina)?
    #[cfg(test)]
    pub fn is_remote(&self) -> bool {
        matches!(self.backend, Backend::Remote(_))
    }

    /// O problema tipado do último pedido da Engine remota.
    pub fn remote_problem(&mut self) -> Option<&chess_providers::ProviderError> {
        match &mut self.backend {
            Backend::Remote(engine) => unpoisoned(engine).problem(),
            Backend::Local(_) => None,
        }
    }

    /// Pede a análise de uma Posição, se a trava deixar; recusada, a Engine para.
    pub fn analyze(
        &mut self,
        fair: &FairPlay,
        subject: &Subject,
        spec: chess_uci::PositionSpec,
        multipv: u8,
    ) -> Result<u64, FairPlayLocked> {
        let computation = match self.backend {
            Backend::Local(_) => Computation::EngineAnalysis,
            Backend::Remote(_) => Computation::RemoteEngine,
        };
        if let Err(locked) = fair.check(computation, subject) {
            self.stop();
            return Err(locked);
        }
        Ok(match &mut self.backend {
            Backend::Local(controller) => controller.analyze(spec, multipv),
            Backend::Remote(engine) => unpoisoned(engine).analyze(spec, multipv),
        })
    }

    pub fn stop(&mut self) {
        match &mut self.backend {
            Backend::Local(controller) => controller.stop(),
            Backend::Remote(engine) => unpoisoned(engine).stop(),
        }
    }

    pub fn poll(&mut self) -> Option<chess_uci::AnalysisEvent> {
        match &mut self.backend {
            Backend::Local(controller) => controller.poll(),
            Backend::Remote(engine) => unpoisoned(engine).poll(),
        }
    }

    /// Plano de carga do governador de FPS (C10): não muda o que se calcula, só quanto. A
    /// Engine remota não tem carga local.
    pub fn set_load(&mut self, plan: chess_uci::LoadPlan) {
        if let Backend::Local(controller) = &mut self.backend {
            controller.set_load(plan);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_core::Move;
    use chess_game::{
        Command, GameSetup, OnlineGame, OnlinePlayer, Opponent, ProviderSnapshot, Timestamp,
        Variant,
    };

    pub(crate) fn online_game(remote_id: &str, days: Option<u32>, opponent: Opponent) -> Game {
        let setup = GameSetup::online(
            OnlineGame {
                provider: "lichess".into(),
                remote_id: remote_id.into(),
                local_color: Color::White,
                opponent,
                white: OnlinePlayer {
                    name: "Ana".into(),
                    rating: Some(1500),
                },
                black: OnlinePlayer {
                    name: "Bruno".into(),
                    rating: Some(1510),
                },
                rated: false,
                days_per_move: days,
            },
            Variant::Standard,
            None,
        )
        .unwrap();
        Game::new(setup)
    }

    pub(crate) fn feed(game: &mut Game, moves: &[&str], end: Option<chess_game::ProviderEnd>) {
        let at = game.events().last().map_or(0, |e| e.at.mono_ms) + 1;
        game.handle(
            Command::Provider(ProviderSnapshot {
                moves: moves.iter().map(|m| (*m).to_owned()).collect(),
                clock: None,
                draw_offer: None,
                end,
            }),
            Timestamp::mono(at),
        )
        .unwrap();
    }

    fn after(moves: &[&str]) -> CanonicalPosition {
        moves.iter().fold(CanonicalPosition::start(), |p, m| {
            p.play(m.parse::<Move>().unwrap()).unwrap()
        })
    }

    fn local(moves: &[&str]) -> Game {
        let mut game = Game::new(GameSetup::standard(chess_game::PolicyKind::Assisted));
        for (i, m) in moves.iter().enumerate() {
            let mv: Move = m.parse().unwrap();
            game.handle(
                Command::Release {
                    from: mv.from(),
                    to: mv.to(),
                },
                Timestamp::mono(i as u64),
            )
            .unwrap();
        }
        game
    }

    fn fair() -> FairPlay {
        FairPlay::new(FairPlayGuard::new())
    }

    /// Correspondência na Escocesa: 1.e4 e5 2.Nf3 Nc6 3.d4 exd4 4.Nxd4 Nf6 5.Nxc6 bxc6.
    const SCOTCH: [&str; 10] = [
        "e2e4", "e7e5", "g1f3", "b8c6", "d2d4", "e5d4", "f3d4", "g8f6", "d4c6", "b7c6",
    ];

    #[test]
    fn without_online_games_everything_is_allowed() {
        let fair = fair();
        let subject = Subject::at(&local(&SCOTCH), 10).unwrap();
        for computation in Computation::ALL {
            assert_eq!(fair.check(computation, &subject), Ok(()));
        }
        assert!(!fair.any());
    }

    #[test]
    fn the_network_guard_locks_everything_for_every_service() {
        let fair = fair();
        let hold = fair.guard().hold("Partida online contra pessoa");
        let elsewhere = Subject::at(&local(&["d2d4"]), 1).unwrap();
        for computation in Computation::ALL {
            assert_eq!(
                fair.check(computation, &elsewhere),
                Err(FairPlayLocked {
                    computation,
                    scope: LockScope::Everything
                })
            );
        }
        drop(hold);
        assert_eq!(fair.check(Computation::EngineAnalysis, &elsewhere), Ok(()));
    }

    #[test]
    fn a_real_time_game_in_progress_locks_everything_even_without_its_session() {
        let fair = fair();
        let mut game = online_game("rt1", None, Opponent::Human);
        feed(&mut game, &["e2e4"], None);
        fair.follow_games([&game]);
        assert!(fair.everything_locked());
        let unrelated = Subject::position(&after(&["d2d4", "d7d5"]));
        assert_eq!(
            fair.check(Computation::CloudEval, &unrelated)
                .unwrap_err()
                .scope,
            LockScope::Everything
        );
    }

    #[test]
    fn the_correspondence_game_itself_is_locked_at_every_ply() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        assert!(!fair.everything_locked());
        for ply in 0..=SCOTCH.len() {
            let subject = Subject::at(&game, ply).unwrap();
            assert_eq!(
                fair.check(Computation::EngineAnalysis, &subject)
                    .unwrap_err()
                    .scope,
                LockScope::ThisGame,
                "meio-lance {ply}"
            );
        }
        // Mesmo antes de a trava ser informada da Partida, ela se reconhece pelo setup.
        let other = online_game("cor2", Some(3), Opponent::Human);
        assert_eq!(
            fair.check(Computation::Hint, &Subject::at(&other, 0).unwrap())
                .unwrap_err()
                .scope,
            LockScope::ThisGame
        );
    }

    #[test]
    fn correspondence_locks_its_current_position_and_what_is_near_it_anywhere() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        // A mesma Posição num tabuleiro de estudo, por FEN.
        let current = after(&SCOTCH);
        let by_fen = Subject::position(&current);
        for computation in Computation::ALL {
            assert_eq!(
                fair.check(computation, &by_fen).unwrap_err().scope,
                LockScope::ThisPosition
            );
        }
        // Dois meios-lances à frente (o Lance candidato e a resposta).
        let ahead = after(&[&SCOTCH[..], &["e4e5", "d8e7"]].concat());
        assert!(
            fair.check(Computation::Explorer, &Subject::position(&ahead))
                .is_err()
        );
        // Manobras de peças mais adiante mantêm a estrutura: continuam travadas.
        let maneuver = after(&[&SCOTCH[..], &["f1d3", "f8c5", "e1g1", "e8g8"]].concat());
        assert!(
            fair.check(Computation::Endgames, &Subject::position(&maneuver))
                .is_err()
        );
        // E tudo o que descende da Posição atual num estudo.
        let study = local(&[&SCOTCH[..], &["e4e5", "f6d5", "c2c4", "d5b6"]].concat());
        let deep = Subject::at(&study, 14).unwrap();
        assert_eq!(
            fair.check(Computation::EngineAnalysis, &deep)
                .unwrap_err()
                .scope,
            LockScope::ThisPosition
        );
    }

    #[test]
    fn correspondence_leaves_other_games_and_its_past_opening_free() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        // Outra abertura, outra Partida: livre.
        let other = local(&["d2d4", "d7d5", "c2c4", "e7e6", "b1c3", "g8f6"]);
        for ply in 0..=6 {
            let subject = Subject::at(&other, ply).unwrap();
            assert_eq!(fair.check(Computation::EngineAnalysis, &subject), Ok(()));
        }
        // As Posições já jogadas da Partida, vistas em outro tabuleiro, não travam.
        let opening = local(&SCOTCH[..4]);
        assert_eq!(
            fair.check(Computation::Explorer, &Subject::at(&opening, 4).unwrap()),
            Ok(())
        );
        assert_eq!(
            fair.check(
                Computation::LibraryStats,
                &Subject::position(&CanonicalPosition::start())
            ),
            Ok(())
        );
    }

    #[test]
    fn a_variation_shown_from_a_locked_position_stays_locked() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        let study = local(&SCOTCH);
        let base = Subject::at(&study, 10).unwrap();
        let far = after(&[&SCOTCH[..], &["e4e5", "f6d5", "c2c4", "d5b6", "c4c5"]].concat());
        assert!(
            fair.check(Computation::EngineAnalysis, &base.then(&far))
                .is_err()
        );
    }

    #[test]
    fn against_the_providers_engine_nothing_is_locked() {
        let fair = fair();
        let mut game = online_game("ai1", None, Opponent::ProviderEngine { level: 3 });
        feed(&mut game, &["e2e4"], None);
        fair.follow_games([&game]);
        assert_eq!(fair.followed(), 0);
        let subject = Subject::at(&game, 1).unwrap();
        for computation in Computation::ALL {
            assert_eq!(fair.check(computation, &subject), Ok(()));
        }
    }

    #[test]
    fn the_lock_only_falls_with_the_confirmed_end() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        let subject = Subject::position(&after(&SCOTCH));
        assert!(fair.check(Computation::CloudEval, &subject).is_err());
        // Fechar o Painel, abrir outra Partida: a trava continua (o app segue informando).
        fair.follow_games([&game]);
        assert!(fair.check(Computation::CloudEval, &subject).is_err());
        feed(
            &mut game,
            &SCOTCH,
            Some(chess_game::ProviderEnd::Decided(chess_game::Outcome {
                result: chess_game::GameResult::BlackWins,
                reason: chess_game::EndReason::Resignation,
            })),
        );
        fair.follow_games([&game]);
        assert_eq!(fair.check(Computation::CloudEval, &subject), Ok(()));
        let review = Subject::at(&game, 4).unwrap();
        assert_eq!(fair.check(Computation::EngineAnalysis, &review), Ok(()));
    }

    #[test]
    fn a_move_in_the_correspondence_moves_the_lock_along() {
        let fair = fair();
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH[..2], None);
        fair.follow_games([&game]);
        assert!(
            fair.check(
                Computation::Explorer,
                &Subject::position(&after(&SCOTCH[..2]))
            )
            .is_err()
        );
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        assert!(
            fair.check(Computation::Explorer, &Subject::position(&after(&SCOTCH)))
                .is_err()
        );
    }

    /// Sem atalho: no app, a Engine de análise só nasce e só recebe Posição por
    /// [`GuardedAnalysis`], e a estatística da Biblioteca só sai por [`opening_stats`]. Teste
    /// estático: varre as fontes do app (fora deste arquivo).
    #[test]
    fn computing_services_are_only_reachable_through_the_lock() {
        fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    sources(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&root, &mut files);
        let forbidden = [
            "AnalysisController::start(",
            ".opening_stats(",
            "AnalysisController>",
            "RemoteEngine::new(",
        ];
        for file in files {
            if file.ends_with("fair_play.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            // A dica só no cálculo do HUD, que recebe a permissão já com a trava (`effective`).
            if !file.ends_with("hud.rs") {
                assert!(
                    !text.contains("suggest_hint("),
                    "{} calcula dica fora do HUD",
                    file.display()
                );
            }
            for needle in forbidden {
                assert!(
                    !text.contains(needle),
                    "{} usa `{needle}` sem passar pela Trava de fair play",
                    file.display()
                );
            }
        }
    }

    #[test]
    fn an_unreadable_fen_is_refused_while_any_lock_is_on() {
        let fair = fair();
        assert_eq!(fair.check_fen(Computation::LibraryStats, "lixo"), Ok(()));
        let mut game = online_game("cor1", Some(3), Opponent::Human);
        feed(&mut game, &SCOTCH, None);
        fair.follow_games([&game]);
        assert!(fair.check_fen(Computation::LibraryStats, "lixo").is_err());
        assert!(
            fair.check_fen(Computation::LibraryStats, &after(&SCOTCH).to_fen())
                .is_err()
        );
    }
}
