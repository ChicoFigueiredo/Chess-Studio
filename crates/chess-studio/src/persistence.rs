//! Liga o `chess-store` ao app (ADR-0002, DB-01, UX-08): o banco fica no diretório de
//! dados do usuário; ao abrir, a última Partida não terminada volta **pausada** (DEC-42);
//! cada Evento aceito é gravado (uma transação por lote); ao fechar a janela a sessão é
//! encerrada com `close_game`; as preferências são gravadas no ato, sem "Salvar".
//!
//! [`Persistence`] não depende do Bevy: observa a [`Session`] (`sync`) e as preferências
//! (`sync_prefs`), então a retomada é testável com um banco em memória. Se o banco não
//! abrir, o app segue jogando sem persistência (aviso no log).

use crate::aids::AidSettings;
use crate::dock::{Dock, DockLayout};
use crate::engine::{EngineUi, MAX_ELO, MIN_ELO};
use crate::focus::ViewState;
use crate::hud::HudSettings;
use crate::play::PlaySet;
use crate::presentation::{GraphicsQuality, Presentation};
use crate::profiles::{InterfaceProfiles, ProfilesState};
use crate::session::Session;
use crate::training_ui::TrainingUi;
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use chess_core::Color;
use chess_game::{Game, GameSetup, PolicyKind};
use chess_input::MoveMethod;
use chess_store::{GameId, GameMetadata, ProfileId, Store, StoreError};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEY_AIDS: &str = "aids";
/// HUD virtual (C5): elementos, transparência e paleta.
const KEY_HUD: &str = "hud";
const KEY_MOVE_METHOD: &str = "move_method";
const KEY_PRESENTATION: &str = "presentation";
const KEY_QUALITY: &str = "quality";
const KEY_ENGINE_ELO: &str = "engine_elo";
const KEY_ENGINE_USER_SIDE: &str = "engine_user_side";
/// Tela cheia e modo Foco (C3, UI-03/04): preferências de interface.
const KEY_FULLSCREEN: &str = "fullscreen";
const KEY_FOCUS: &str = "focus_mode";
/// Layout do dock de painéis (C1, UI-02): árvore, tamanhos, visibilidade e largura.
const KEY_DOCK_LAYOUT: &str = "dock_layout";
/// Perfis de interface (C4, UI-09): o ativo e os salvos pelo usuário.
const KEY_INTERFACE_PROFILES: &str = "interface_profiles";
/// Prefixo da chave das Anotações de estudo (C2): uma preferência por Partida.
const KEY_STUDY_NOTES: &str = "study_notes";

/// Preferências do usuário que sobrevivem ao fechamento do app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedPrefs {
    pub aids: AidSettings,
    pub hud: HudSettings,
    pub move_method: MoveMethod,
    pub presentation: Presentation,
    pub quality: GraphicsQuality,
    /// Elo escolhido para a Engine adversária (B9).
    pub engine_elo: u32,
    /// Lado do usuário contra a Engine (B9).
    pub engine_user_side: Color,
    /// Tela cheia (F11) e modo Foco (F10), independentes.
    pub view: ViewState,
}

impl Default for SavedPrefs {
    fn default() -> Self {
        Self {
            aids: AidSettings::default(),
            hud: HudSettings::default(),
            move_method: MoveMethod::default(),
            presentation: Presentation::default(),
            quality: GraphicsQuality::default(),
            engine_elo: 1500,
            engine_user_side: Color::White,
            view: ViewState::default(),
        }
    }
}

/// Resultado de abrir o app: a Partida a jogar e se ela veio do banco.
#[derive(Debug)]
pub struct Restored {
    pub session: Session,
    pub resumed: bool,
    /// O processo anterior terminou sem fechar a sessão (queda de energia, SIGKILL).
    pub abrupt_close: bool,
}

#[derive(Debug)]
pub struct Persistence {
    store: Store,
    profile: ProfileId,
    current: Option<GameId>,
    /// Quantos Eventos da Partida atual já estão gravados.
    persisted: usize,
    epoch: u64,
    was_paused: bool,
    saved: Option<SavedPrefs>,
    /// Último layout do dock gravado (ou carregado), para só gravar o que mudou.
    saved_layout: Option<Value>,
    /// Últimos Perfis de interface gravados (ou carregados).
    saved_profiles: Option<Value>,
    failing: bool,
}

fn debug_id<T: std::fmt::Debug>(value: &T) -> Value {
    Value::String(format!("{value:?}"))
}

fn from_debug_id<T: std::fmt::Debug + Copy>(all: &[T], value: Option<Value>) -> Option<T> {
    let text = value?;
    let text = text.as_str()?;
    all.iter().copied().find(|v| format!("{v:?}") == text)
}

impl Persistence {
    /// Abre (criando, se preciso) o banco em `path`.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            // Falha aqui reaparece como erro do SQLite ao abrir o arquivo.
            let _ = std::fs::create_dir_all(parent);
        }
        Ok(Self::new(Store::open(path)?))
    }

    pub fn new(store: Store) -> Self {
        let profile = store.default_profile().unwrap_or(ProfileId(1));
        Self {
            store,
            profile,
            current: None,
            persisted: 0,
            epoch: 0,
            was_paused: false,
            saved: None,
            saved_layout: None,
            saved_profiles: None,
            failing: false,
        }
    }

    /// O banco e o perfil, para as telas que têm API própria (Treino).
    pub(crate) fn store_and_profile(&mut self) -> (&mut Store, ProfileId) {
        (&mut self.store, self.profile)
    }

    /// Devolve o banco (testes: reabrir sobre os mesmos dados).
    #[cfg(test)]
    pub fn into_store(self) -> Store {
        self.store
    }

    /// Partida a jogar: a última não terminada, reaberta **pausada**, ou uma nova.
    pub fn restore(&mut self) -> Restored {
        match self.try_resume() {
            Ok(Some((session, abrupt_close))) => Restored {
                session,
                resumed: true,
                abrupt_close,
            },
            Ok(None) => Restored {
                session: self.fresh(),
                resumed: false,
                abrupt_close: false,
            },
            Err(err) => {
                warn!("não foi possível retomar a última Partida: {err}");
                Restored {
                    session: self.fresh(),
                    resumed: false,
                    abrupt_close: false,
                }
            }
        }
    }

    fn try_resume(&mut self) -> Result<Option<(Session, bool)>, StoreError> {
        let Some(id) = self.store.latest_unfinished_game()? else {
            return Ok(None);
        };
        let opened = self.store.open_game(id)?;
        let mut game = opened.game;
        // Congela o relógio no último Evento gravado (DEC-42) e registra a Pausa no log.
        let events = game.reopen_paused().unwrap_or_default();
        self.store.append_events(id, &events)?;
        self.current = Some(id);
        self.persisted = game.events().len();
        let session = Session::from_game(game);
        self.epoch = session.epoch();
        self.was_paused = session.snapshot().paused;
        Ok(Some((session, opened.abrupt_close)))
    }

    /// A Partida atual no banco (origem das Posições favoritas).
    pub(crate) fn current_game(&self) -> Option<GameId> {
        self.current
    }

    /// Abre uma Partida do banco (Biblioteca, #49) como a Partida atual: encerra a sessão da
    /// atual (ela continua no banco, retomável), reabre `id` **pausada** e passa a gravar nela.
    /// Depois de `Session::open_game(game)` chame [`Persistence::track`], senão a persistência
    /// trataria a troca como uma Partida nova e duplicaria o log.
    pub(crate) fn switch_to_game(&mut self, id: GameId) -> Result<Game, StoreError> {
        let opened = self.store.open_game(id)?;
        if self.current != Some(id) {
            self.close();
        }
        let mut game = opened.game;
        let events = game.reopen_paused().unwrap_or_default();
        self.store.append_events(id, &events)?;
        self.current = Some(id);
        self.persisted = game.events().len();
        Ok(game)
    }

    /// Acompanha a `Session` que acabou de receber a Partida de [`Self::switch_to_game`].
    pub(crate) fn track(&mut self, session: &Session) {
        self.epoch = session.epoch();
        self.was_paused = session.snapshot().paused;
    }

    fn fresh(&mut self) -> Session {
        let session = Session::from_game(Game::new(GameSetup::standard(PolicyKind::Assisted)));
        self.epoch = session.epoch();
        self.start_game(&session);
        session
    }

    fn start_game(&mut self, session: &Session) {
        self.persisted = 0;
        self.was_paused = false;
        self.current = match self.store.create_game_for(
            self.profile,
            session.game().setup(),
            &GameMetadata::default(),
        ) {
            Ok(id) => Some(id),
            Err(err) => {
                self.report(&err);
                None
            }
        };
    }

    fn report(&mut self, err: &StoreError) {
        if !self.failing {
            warn!("persistência: {err}");
        }
        self.failing = true;
    }

    /// Grava o que a [`Session`] fez desde a última chamada: troca de Partida (Nova
    /// Partida, FEN) e Eventos novos. Barato quando nada mudou.
    pub fn sync(&mut self, session: &Session) {
        if session.epoch() != self.epoch {
            self.close();
            self.epoch = session.epoch();
            self.start_game(session);
        }
        if self.current.is_none() {
            self.start_game(session);
        }
        let events = session.game().events();
        if let Some(id) = self.current
            && events.len() > self.persisted
        {
            match self.store.append_events(id, &events[self.persisted..]) {
                Ok(()) => {
                    self.persisted = events.len();
                    self.failing = false;
                }
                Err(err) => self.report(&err),
            }
        }
        let paused = session.snapshot().paused;
        if self.was_paused
            && !paused
            && let Some(id) = self.current
            && let Err(err) = self.store.resume_game(id)
        {
            self.report(&err);
        }
        self.was_paused = paused;
    }

    /// Fecha a sessão da Partida atual (janela fechada normalmente, nunca "Salvar").
    pub fn close(&mut self) {
        if let Some(id) = self.current
            && let Err(err) = self.store.close_game(id)
        {
            self.report(&err);
        }
    }

    /// Preferências gravadas; o que faltar ou estiver ilegível volta ao padrão.
    pub fn load_prefs(&self) -> SavedPrefs {
        let get = |key: &str| self.store.get_pref(self.profile, key).ok().flatten();
        let defaults = SavedPrefs::default();
        SavedPrefs {
            aids: get(KEY_AIDS)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or(defaults.aids),
            hud: get(KEY_HUD)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or(defaults.hud),
            move_method: from_debug_id(
                &[MoveMethod::Both, MoveMethod::TwoTaps, MoveMethod::Drag],
                get(KEY_MOVE_METHOD),
            )
            .unwrap_or(defaults.move_method),
            presentation: from_debug_id(&Presentation::ALL, get(KEY_PRESENTATION))
                .unwrap_or(defaults.presentation),
            quality: from_debug_id(&GraphicsQuality::ALL, get(KEY_QUALITY))
                .unwrap_or(defaults.quality),
            engine_elo: get(KEY_ENGINE_ELO)
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok())
                .map_or(defaults.engine_elo, |v| v.clamp(MIN_ELO, MAX_ELO)),
            engine_user_side: from_debug_id(
                &[Color::White, Color::Black],
                get(KEY_ENGINE_USER_SIDE),
            )
            .unwrap_or(defaults.engine_user_side),
            view: ViewState {
                fullscreen: get(KEY_FULLSCREEN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(defaults.view.fullscreen),
                focus: get(KEY_FOCUS)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(defaults.view.focus),
            },
        }
    }

    /// Layout do dock gravado; ausente ou ilegível volta ao padrão.
    pub fn load_layout(&mut self) -> DockLayout {
        let stored = self
            .store
            .get_pref(self.profile, KEY_DOCK_LAYOUT)
            .ok()
            .flatten();
        let layout = DockLayout::from_json(stored);
        self.saved_layout = Some(layout.to_json());
        layout
    }

    /// Grava o layout do dock se mudou desde a última chamada (no ato, sem "Salvar").
    pub fn sync_layout(&mut self, layout: &DockLayout) {
        let json = layout.to_json();
        if self.saved_layout.as_ref() == Some(&json) {
            return;
        }
        match self.store.set_pref(self.profile, KEY_DOCK_LAYOUT, &json) {
            Ok(()) => self.saved_layout = Some(json),
            Err(err) => self.report(&err),
        }
    }

    /// Perfis de interface gravados; ausente ou ilegível volta ao padrão.
    pub fn load_profiles(&mut self) -> ProfilesState {
        let stored = self
            .store
            .get_pref(self.profile, KEY_INTERFACE_PROFILES)
            .ok()
            .flatten();
        let profiles = ProfilesState::from_json(stored);
        self.saved_profiles = Some(profiles.to_json());
        profiles
    }

    /// Grava os Perfis de interface se mudaram desde a última chamada.
    pub fn sync_profiles(&mut self, profiles: &ProfilesState) {
        let json = profiles.to_json();
        if self.saved_profiles.as_ref() == Some(&json) {
            return;
        }
        match self
            .store
            .set_pref(self.profile, KEY_INTERFACE_PROFILES, &json)
        {
            Ok(()) => self.saved_profiles = Some(json),
            Err(err) => self.report(&err),
        }
    }

    fn notes_key(&self) -> Option<String> {
        self.current.map(|id| format!("{KEY_STUDY_NOTES}.{}", id.0))
    }

    /// Anotações de estudo da Partida atual (vazio se não há ou estão ilegíveis).
    pub fn load_notes(&self) -> BTreeMap<usize, String> {
        let stored = self
            .notes_key()
            .and_then(|key| self.store.get_pref(self.profile, &key).ok().flatten());
        crate::study::notes_from_json(stored)
    }

    /// Grava as anotações de estudo da Partida atual (no ato, sem "Salvar").
    pub fn save_notes(&mut self, notes: &BTreeMap<usize, String>) {
        let Some(key) = self.notes_key() else { return };
        if let Err(err) =
            self.store
                .set_pref(self.profile, &key, &crate::study::notes_to_json(notes))
        {
            self.report(&err);
        }
    }

    /// Grava só o que mudou desde a última chamada (ou desde `load_prefs`).
    pub fn sync_prefs(&mut self, now: SavedPrefs) {
        let before = match self.saved {
            Some(saved) => saved,
            None => self.load_prefs(),
        };
        if before == now {
            return;
        }
        let mut writes: Vec<(&str, Value)> = Vec::new();
        if before.aids != now.aids {
            writes.push((KEY_AIDS, json!(now.aids)));
        }
        if before.hud != now.hud {
            writes.push((KEY_HUD, json!(now.hud)));
        }
        if before.move_method != now.move_method {
            writes.push((KEY_MOVE_METHOD, debug_id(&now.move_method)));
        }
        if before.presentation != now.presentation {
            writes.push((KEY_PRESENTATION, debug_id(&now.presentation)));
        }
        if before.quality != now.quality {
            writes.push((KEY_QUALITY, debug_id(&now.quality)));
        }
        if before.engine_elo != now.engine_elo {
            writes.push((KEY_ENGINE_ELO, json!(now.engine_elo)));
        }
        if before.engine_user_side != now.engine_user_side {
            writes.push((KEY_ENGINE_USER_SIDE, debug_id(&now.engine_user_side)));
        }
        if before.view.fullscreen != now.view.fullscreen {
            writes.push((KEY_FULLSCREEN, json!(now.view.fullscreen)));
        }
        if before.view.focus != now.view.focus {
            writes.push((KEY_FOCUS, json!(now.view.focus)));
        }
        for (key, value) in writes {
            if let Err(err) = self.store.set_pref(self.profile, key, &value) {
                self.report(&err);
                return;
            }
        }
        self.saved = Some(now);
    }
}

/// Caminho do banco: `--db <arquivo>`, o padrão do usuário, ou nenhum (sem persistência).
pub fn database_location(override_path: Option<PathBuf>) -> Option<PathBuf> {
    override_path.or_else(|| {
        crate::paths::database_path(
            crate::paths::Platform::current(),
            &crate::paths::process_env,
        )
    })
}

#[derive(Resource, Debug)]
pub(crate) struct PersistenceHandle(pub(crate) Mutex<Persistence>);

/// Liga a persistência. `db_path = None` mantém o app sem banco (capturas, benchmark).
#[derive(Debug)]
pub struct PersistencePlugin {
    pub db_path: Option<PathBuf>,
}

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = &self.db_path else { return };
        let mut persistence = match Persistence::open(path) {
            Ok(p) => p,
            Err(err) => {
                warn!(
                    "banco {} indisponível, o app segue sem persistência: {err}",
                    path.display()
                );
                return;
            }
        };
        let prefs = persistence.load_prefs();
        let mut restored = persistence.restore();
        restored.session.set_move_method(prefs.move_method);
        if restored.resumed {
            info!(
                "Partida retomada em Pausa{}",
                if restored.abrupt_close {
                    " (a sessão anterior não foi fechada)"
                } else {
                    ""
                }
            );
        }
        persistence.saved = Some(prefs);
        let layout = persistence.load_layout();
        let profiles = persistence.load_profiles();
        let notes =
            crate::study::StudyNotes::loaded(restored.session.epoch(), persistence.load_notes());
        // Os plugins seguintes usam `init_resource`, que preserva estes valores.
        app.insert_resource(restored.session)
            .insert_resource(prefs.aids)
            .insert_resource(prefs.hud)
            .insert_resource(prefs.presentation)
            .insert_resource(prefs.quality)
            .insert_resource(prefs.view)
            .insert_resource(Dock::with_layout(layout))
            .insert_resource(InterfaceProfiles::with_state(profiles))
            .insert_resource(notes)
            .insert_resource(
                EngineUi::default().with_saved(prefs.engine_user_side, prefs.engine_elo),
            )
            .insert_resource(crate::engine_manager::DbLocation(path.clone()))
            .insert_resource(PersistenceHandle(Mutex::new(persistence)))
            .add_systems(
                Update,
                (
                    persist_session,
                    persist_prefs,
                    persist_layout,
                    persist_profiles,
                )
                    .after(PlaySet),
            )
            .add_systems(Update, persist_notes.after(persist_session).after(PlaySet))
            .add_systems(Last, close_on_exit);
    }
}

fn persist_session(
    session: Res<Session>,
    handle: Res<PersistenceHandle>,
    training: Res<TrainingUi>,
) {
    // Durante o Treino a `Session` é a Partida de um Exercício: não vira Partida do histórico.
    if training.session_swapped() {
        return;
    }
    if let Ok(mut p) = handle.0.lock() {
        p.sync(&session);
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_prefs(
    session: Res<Session>,
    aids: Res<AidSettings>,
    hud: Res<HudSettings>,
    presentation: Res<Presentation>,
    quality: Res<GraphicsQuality>,
    engine_ui: Res<EngineUi>,
    view: Res<ViewState>,
    handle: Res<PersistenceHandle>,
) {
    if let Ok(mut p) = handle.0.lock() {
        p.sync_prefs(SavedPrefs {
            aids: *aids,
            hud: *hud,
            move_method: session.move_method(),
            presentation: *presentation,
            quality: *quality,
            engine_elo: engine_ui.elo(),
            engine_user_side: engine_ui.user_side(),
            view: *view,
        });
    }
}

/// Grava as Anotações de estudo quando mudam. Depois de `persist_session` (que cria a
/// Partida no banco); durante o Treino a `Session` não é a Partida do usuário.
fn persist_notes(
    session: Res<Session>,
    training: Res<TrainingUi>,
    mut notes: ResMut<crate::study::StudyNotes>,
    handle: Res<PersistenceHandle>,
) {
    if training.session_swapped() || !notes.is_dirty() {
        return;
    }
    if let Some(dirty) = notes.take_dirty(session.epoch()).cloned()
        && let Ok(mut p) = handle.0.lock()
    {
        p.save_notes(&dirty);
    }
}

/// Grava o layout do dock. Enquanto o botão do mouse está apertado (arrastando um divisor
/// ou uma aba) espera: grava ao soltar, não a cada quadro.
fn persist_layout(
    dock: Res<Dock>,
    buttons: Res<ButtonInput<MouseButton>>,
    handle: Res<PersistenceHandle>,
) {
    if buttons.pressed(MouseButton::Left) {
        return;
    }
    if let Ok(mut p) = handle.0.lock() {
        p.sync_layout(&dock.layout);
    }
}

/// Grava os Perfis de interface quando mudam.
fn persist_profiles(profiles: Res<InterfaceProfiles>, handle: Res<PersistenceHandle>) {
    if let Ok(mut p) = handle.0.lock() {
        p.sync_profiles(&profiles.state);
    }
}

/// Janela fechada ou saída do app: encerra a sessão da Partida e grava o que faltar.
fn close_on_exit(
    mut close_requested: MessageReader<WindowCloseRequested>,
    mut exits: MessageReader<AppExit>,
    session: Res<Session>,
    handle: Res<PersistenceHandle>,
    training: Res<TrainingUi>,
    dock: Res<Dock>,
    profiles: Res<InterfaceProfiles>,
) {
    let closing = close_requested.read().count() > 0 || exits.read().count() > 0;
    if closing && let Ok(mut p) = handle.0.lock() {
        if !training.session_swapped() {
            p.sync(&session);
        }
        p.sync_layout(&dock.layout);
        p.sync_profiles(&profiles.state);
        p.close();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_core::Square;
    use chess_game::{Command, EventKind};

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn play(session: &mut Session, from: &str, to: &str) {
        session.command(Command::Release {
            from: sq(from),
            to: sq(to),
        });
    }

    fn new_persistence() -> Persistence {
        Persistence::new(Store::open_in_memory().unwrap())
    }

    #[test]
    fn closing_and_reopening_resumes_the_game_paused_at_the_same_move() {
        // DB-01 / BDD-07 no app.
        let mut p = new_persistence();
        let Restored {
            mut session,
            resumed,
            ..
        } = p.restore();
        assert!(!resumed);
        play(&mut session, "e2", "e4");
        p.sync(&session);
        play(&mut session, "e7", "e5");
        play(&mut session, "e1", "e5"); // ilegal: fica no log, fora do histórico
        p.sync(&session);
        let before = session.snapshot().clone();
        p.close();

        let mut p2 = Persistence::new(p.into_store());
        let restored = p2.restore();
        assert!(restored.resumed);
        assert!(!restored.abrupt_close, "fechou normalmente");
        let after = restored.session.snapshot();
        assert_eq!(after.position, before.position);
        assert_eq!(after.history, before.history);
        assert!(after.paused, "volta pausada (DEC-42)");
        assert_eq!(restored.session.illegal_incidents().len(), 1);
    }

    #[test]
    fn moves_after_resume_continue_the_same_log_with_monotonic_time() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "e2", "e4");
        p.sync(&session);
        p.close();

        let mut p2 = Persistence::new(p.into_store());
        let mut session = p2.restore().session;
        session.command(Command::Resume);
        play(&mut session, "e7", "e5");
        p2.sync(&session);
        assert_eq!(session.snapshot().history.len(), 2);
        p2.close();

        let mut p3 = Persistence::new(p2.into_store());
        let restored = p3.restore();
        let sans: Vec<&str> = restored
            .session
            .snapshot()
            .history
            .iter()
            .map(|h| h.san.as_str())
            .collect();
        assert_eq!(sans, ["e4", "e5"]);
        let kinds: Vec<&EventKind> = restored
            .session
            .game()
            .events()
            .iter()
            .map(|e| &e.kind)
            .collect();
        assert!(kinds.iter().any(|k| matches!(k, EventKind::Resumed)));
    }

    #[test]
    fn unclean_exit_is_reported_as_abrupt_close() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "d2", "d4");
        p.sync(&session);
        // Sem close(): o processo "morreu".
        let mut p2 = Persistence::new(p.into_store());
        let restored = p2.restore();
        assert!(restored.resumed && restored.abrupt_close);
        assert_eq!(restored.session.snapshot().history.len(), 1);
    }

    #[test]
    fn finished_games_are_not_resumed() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "e2", "e4");
        session.resign_side_to_move();
        p.sync(&session);
        p.close();
        let mut p2 = Persistence::new(p.into_store());
        assert!(!p2.restore().resumed);
    }

    #[test]
    fn new_game_and_fen_start_new_rows_and_the_discarded_game_is_not_resumed() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "e2", "e4");
        p.sync(&session);
        session.new_game();
        p.sync(&session);
        session.load_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
        p.sync(&session);
        play(&mut session, "e2", "e4");
        p.sync(&session);
        p.close();

        let mut p2 = Persistence::new(p.into_store());
        let restored = p2.restore();
        assert!(restored.resumed);
        // Retoma a Partida do FEN, não a antiga.
        let sans: Vec<&str> = restored
            .session
            .snapshot()
            .history
            .iter()
            .map(|h| h.san.as_str())
            .collect();
        assert_eq!(sans, ["e4"]);
        assert_eq!(restored.session.game().setup().start().to_fen().len(), 31);
    }

    /// C7 (#49): reabrir uma Partida da Biblioteca troca a Partida atual sem duplicar o log;
    /// a anterior continua guardada e o que se jogar depois vai para a reaberta.
    #[test]
    fn opening_a_library_game_switches_without_duplicating_the_log() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "e2", "e4");
        p.sync(&session);
        let first = p.current_game().unwrap();
        session.new_game();
        p.sync(&session);
        play(&mut session, "d2", "d4");
        p.sync(&session);
        let second = p.current_game().unwrap();
        assert_ne!(first, second);
        let games = p.store.list_games(&Default::default()).unwrap().len();

        let game = p.switch_to_game(first).unwrap();
        session.open_game(game);
        p.track(&session);
        p.sync(&session);
        assert_eq!(p.current_game(), Some(first));
        assert_eq!(
            p.store.list_games(&Default::default()).unwrap().len(),
            games,
            "nenhuma Partida nova no banco"
        );
        assert_eq!(session.snapshot().history[0].san, "e4");
        assert!(session.snapshot().paused, "volta pausada (DEC-42)");

        // Retoma e joga: os Eventos novos vão para a Partida reaberta, não para a outra.
        session.command(Command::Resume);
        play(&mut session, "e7", "e5");
        p.sync(&session);
        let reopened = p.store.load_game(first).unwrap();
        assert_eq!(reopened.snapshot().history.len(), 2);
        let other = p.store.load_game(second).unwrap();
        assert_eq!(other.snapshot().history.len(), 1);
    }

    #[test]
    fn invalid_fen_changes_nothing_in_session_or_database() {
        // DB-02: erro sem corromper nada.
        let mut p = new_persistence();
        let mut session = p.restore().session;
        play(&mut session, "e2", "e4");
        p.sync(&session);
        let games_before = p.store.list_games(&Default::default()).unwrap().len();
        let epoch = session.epoch();
        let snapshot = session.snapshot().clone();
        assert!(session.load_fen("isto não é um FEN").is_err());
        assert!(session.load_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_err());
        p.sync(&session);
        assert_eq!(session.epoch(), epoch);
        assert_eq!(session.snapshot(), &snapshot);
        assert_eq!(
            p.store.list_games(&Default::default()).unwrap().len(),
            games_before
        );
    }

    #[test]
    fn preferences_round_trip_and_default_to_training() {
        let mut p = new_persistence();
        assert_eq!(p.load_prefs(), SavedPrefs::default());
        assert_eq!(
            p.load_prefs().aids.profile(),
            crate::aids::AidProfile::Training
        );
        let custom = SavedPrefs {
            aids: AidSettings {
                ghosts: true,
                ..AidSettings::physical()
            },
            hud: HudSettings {
                palette: crate::hud::HudPalette::ColorBlind,
                opacity_pct: 25,
                ..HudSettings::analysis()
            },
            move_method: MoveMethod::Drag,
            presentation: Presentation::Technical2d,
            quality: GraphicsQuality::Economic,
            engine_elo: 2100,
            engine_user_side: Color::Black,
            view: ViewState {
                focus: true,
                fullscreen: true,
            },
        };
        p.sync_prefs(custom);
        let p2 = Persistence::new(p.into_store());
        assert_eq!(p2.load_prefs(), custom);
    }

    /// UI-02: layout, tamanhos e visibilidade persistem no SQLite e voltam ao reabrir.
    #[test]
    fn dock_layout_round_trips_through_sqlite() {
        use crate::dock::ids;
        let mut p = new_persistence();
        assert_eq!(p.load_layout(), DockLayout::default(), "banco novo: padrão");
        let mut layout = DockLayout {
            width: 360.0,
            ..DockLayout::default()
        };
        layout.set_visible(ids::NOTATION, false);
        layout.set_visible(ids::CLOCK, false);
        layout.set_visible(ids::STUDY, false);
        p.sync_layout(&layout);
        let mut p2 = Persistence::new(p.into_store());
        let back = p2.load_layout();
        assert_eq!(back, layout);
        assert_eq!(back.width, 360.0);
        assert!(!back.is_visible(ids::NOTATION));
        assert!(!back.is_visible(ids::CLOCK));
        assert!(!back.is_visible(ids::STUDY));
        assert!(back.is_visible(ids::CAPTURED));
        // Restaurar o padrão também é gravado.
        p2.sync_layout(&DockLayout::default());
        let mut p3 = Persistence::new(p2.into_store());
        assert_eq!(p3.load_layout(), DockLayout::default());
    }

    /// UI-09: o perfil ativo e os perfis do usuário persistem e voltam ao reabrir.
    #[test]
    fn interface_profiles_round_trip_through_sqlite() {
        use crate::profiles::{
            FactoryProfile, InterfaceProfile, InterfaceState, ProfileKey, ProfilesState,
        };
        let mut p = new_persistence();
        assert_eq!(p.load_profiles(), ProfilesState::default(), "banco novo");
        let mut profiles = ProfilesState {
            active: ProfileKey::Factory(FactoryProfile::StudyAnalysis),
            ..ProfilesState::default()
        };
        let state = InterfaceState {
            presentation: Presentation::Technical2d,
            dock: DockLayout {
                width: 280.0,
                ..DockLayout::default()
            },
            aids: AidSettings::physical(),
            focus: true,
        };
        profiles
            .save_user("Aula", InterfaceProfile::capture(&state, Some("Top")))
            .unwrap();
        p.sync_profiles(&profiles);
        let mut p2 = Persistence::new(p.into_store());
        assert_eq!(p2.load_profiles(), profiles);
    }

    /// C2: as Anotações de estudo ficam gravadas por Partida e voltam ao retomar.
    #[test]
    fn study_notes_round_trip_per_game() {
        let mut p = new_persistence();
        let mut session = p.restore().session;
        assert!(p.load_notes().is_empty());
        play(&mut session, "e2", "e4");
        p.sync(&session);
        let notes = BTreeMap::from([(0, "Início".to_owned()), (1, "Peão do rei".to_owned())]);
        p.save_notes(&notes);
        assert_eq!(p.load_notes(), notes);
        p.close();

        let mut p2 = Persistence::new(p.into_store());
        let restored = p2.restore();
        assert!(restored.resumed);
        assert_eq!(p2.load_notes(), notes, "mesma Partida, mesmas anotações");

        // Outra Partida começa sem anotações e não herda as da anterior.
        let mut next = restored.session;
        next.new_game();
        p2.sync(&next);
        assert!(p2.load_notes().is_empty());
    }

    #[test]
    fn unreadable_dock_layout_falls_back_to_the_default() {
        let mut p = new_persistence();
        p.store
            .set_pref(p.profile, KEY_DOCK_LAYOUT, &json!({"width": "x"}))
            .unwrap();
        assert_eq!(p.load_layout(), DockLayout::default());
    }

    #[test]
    fn unreadable_preferences_fall_back_to_defaults() {
        let p = new_persistence();
        p.store
            .set_pref(p.profile, KEY_AIDS, &json!("lixo"))
            .unwrap();
        p.store
            .set_pref(p.profile, KEY_PRESENTATION, &json!("Inexistente"))
            .unwrap();
        assert_eq!(p.load_prefs(), SavedPrefs::default());
    }

    #[test]
    fn a_new_game_screen_fide_game_with_engine_is_saved_and_resumes_paused() {
        use crate::new_game::{NewGameConfig, Opponent};
        let mut p = new_persistence();
        let mut session = p.restore().session;
        let config = NewGameConfig {
            opponent: Opponent::Engine,
            engine_elo: 1700,
            ..NewGameConfig::for_policy(PolicyKind::FideSimulation)
        };
        session.start(&config, Some(chess_uci::EngineConfig::new("/nao/existe")));
        assert!(session.engine_active());
        play(&mut session, "e2", "e4");
        session.press_clock();
        p.sync(&session);
        p.close();

        let mut p2 = Persistence::new(p.into_store());
        let restored = p2.restore();
        assert!(restored.resumed);
        let s = restored.session;
        assert!(s.snapshot().paused, "volta pausada");
        assert_eq!(s.snapshot().policy, PolicyKind::FideSimulation);
        assert_eq!(s.snapshot().history.len(), 1);
        assert_eq!(s.config().opponent, Opponent::Engine);
        assert_eq!(s.config().engine_elo, 1700);
        let clock = s.snapshot().clock.as_ref().expect("relógio da Partida");
        assert_eq!(clock.increment_ms, 30_000);
    }
}
