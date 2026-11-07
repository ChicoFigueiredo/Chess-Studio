//! Estado "cena pronta" (B22): assets carregados, entidades esperadas da Apresentação
//! instanciadas e alguns quadros estáveis. `--screenshot` e `--benchmark` esperam por ele
//! antes de capturar ou medir; sem isso a primeira foto saía com fundo escuro e peças
//! ausentes (a glTF e as texturas carregam de forma assíncrona) e o benchmark incluía o
//! carregamento na medição.
//!
//! A máquina de estados ([`ReadyTracker`]) é pura (sem Bevy) para ser testada; o sistema
//! `observe_scene` só colhe os fatos do mundo e os entrega a ela.

use crate::clock::ClockRoot;
use crate::pieces::{BoardOverride, BoardPieces, PieceFont, PieceTemplates, pieces_synced};
use crate::presentation::{Presentation, VisibleIn};
use crate::session::Session;
use bevy::prelude::*;
use std::time::Duration;

/// Quadros consecutivos com tudo pronto antes de declarar a cena pronta (a GPU ainda
/// prepara texturas e malhas nos quadros seguintes ao carregamento).
pub const STABLE_FRAMES: u32 = 30;
/// Prazo padrão, em segundos, para a cena ficar pronta (`--ready-timeout <s>` muda).
pub const DEFAULT_TIMEOUT_S: f32 = 60.0;

/// Fatos observados num quadro; cada um é uma condição de "pronto".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    /// Fonte, texturas das malhas e a cena glTF carregadas (`LoadState::Loaded`).
    pub assets_loaded: bool,
    /// As 12 peças-molde extraídas da glTF.
    pub templates: bool,
    /// Uma entidade por peça da Posição exibida (3D e 2D).
    pub pieces: bool,
    /// Tabuleiro (e mesa/piso) da Apresentação atual instanciados.
    pub board: bool,
    /// Relógio instanciado (só as Apresentações 3D o mostram; no 2D é sempre verdadeiro).
    pub clock: bool,
}

impl Facts {
    /// Nomes (para a mensagem de erro) do que ainda falta.
    pub fn missing(&self) -> Vec<&'static str> {
        [
            (self.assets_loaded, "assets (glTF, texturas ou fonte)"),
            (self.templates, "moldes das 12 peças"),
            (self.pieces, "entidades das peças"),
            (self.board, "tabuleiro da Apresentação"),
            (self.clock, "relógio"),
        ]
        .into_iter()
        .filter_map(|(ok, name)| (!ok).then_some(name))
        .collect()
    }

    fn all(&self) -> bool {
        self.missing().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyState {
    /// Ainda carregando ou instanciando.
    Loading,
    /// Tudo presente; contando quadros estáveis.
    Stabilizing { stable: u32 },
    /// Pronta; `after` é o tempo desde o início até aqui.
    Ready { after: Duration },
    /// Prazo esgotado; guarda o que faltava.
    TimedOut {
        after: Duration,
        missing: Vec<&'static str>,
    },
}

/// Máquina de estados da prontidão: alimentada uma vez por quadro com os [`Facts`].
#[derive(Debug, Clone)]
pub struct ReadyTracker {
    stable_needed: u32,
    timeout: Duration,
    elapsed: Duration,
    state: ReadyState,
}

impl ReadyTracker {
    pub fn new(stable_needed: u32, timeout: Duration) -> Self {
        Self {
            stable_needed,
            timeout,
            elapsed: Duration::ZERO,
            state: ReadyState::Loading,
        }
    }

    pub fn state(&self) -> &ReadyState {
        &self.state
    }

    /// Avança um quadro de duração `dt`. Pronta e esgotada são estados finais; perder um
    /// fato durante a estabilização zera a contagem.
    pub fn update(&mut self, facts: Facts, dt: Duration) -> &ReadyState {
        if matches!(
            self.state,
            ReadyState::Ready { .. } | ReadyState::TimedOut { .. }
        ) {
            return &self.state;
        }
        self.elapsed += dt;
        let stable = match self.state {
            ReadyState::Stabilizing { stable } if facts.all() => stable + 1,
            _ if facts.all() => 1,
            _ => 0,
        };
        self.state = if stable >= self.stable_needed {
            ReadyState::Ready {
                after: self.elapsed,
            }
        } else if facts.all() {
            ReadyState::Stabilizing { stable }
        } else if self.elapsed >= self.timeout {
            ReadyState::TimedOut {
                after: self.elapsed,
                missing: facts.missing(),
            }
        } else {
            ReadyState::Loading
        };
        &self.state
    }
}

/// Recurso com o estado de prontidão da cena.
#[derive(Resource, Debug)]
pub struct SceneReady {
    tracker: ReadyTracker,
}

impl SceneReady {
    pub fn is_ready(&self) -> bool {
        matches!(self.tracker.state(), ReadyState::Ready { .. })
    }

    /// Tempo de carregamento (do início do app até a cena pronta), se já ficou pronta.
    pub fn load_time(&self) -> Option<Duration> {
        match self.tracker.state() {
            ReadyState::Ready { after } => Some(*after),
            _ => None,
        }
    }

    /// Mensagem de erro clara se o prazo esgotou.
    pub fn timeout_error(&self) -> Option<String> {
        match self.tracker.state() {
            ReadyState::TimedOut { after, missing } => Some(format!(
                "a cena não ficou pronta em {:.0} s; faltando: {}. Verifique a pasta de assets \
                 (variável {}) ou aumente o prazo com --ready-timeout <segundos>.",
                after.as_secs_f32(),
                missing.join(", "),
                crate::asset_dir::ENV_OVERRIDE,
            )),
            _ => None,
        }
    }
}

/// Prazo pedido por `--ready-timeout <segundos>` (inválido ou ausente = padrão).
pub fn timeout_from_args(args: &[String]) -> Duration {
    let secs = args
        .iter()
        .position(|a| a == "--ready-timeout")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(DEFAULT_TIMEOUT_S);
    Duration::from_secs_f32(secs)
}

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        app.insert_resource(SceneReady {
            tracker: ReadyTracker::new(STABLE_FRAMES, timeout_from_args(&args)),
        })
        .add_systems(
            Update,
            observe_scene
                .after(crate::play::PlaySet)
                .run_if(|ready: Res<SceneReady>| {
                    !ready.is_ready() && ready.timeout_error().is_none()
                }),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_scene(
    mut ready: ResMut<SceneReady>,
    time: Res<Time<Real>>,
    assets: Res<AssetServer>,
    materials: Res<Assets<StandardMaterial>>,
    font: Option<Res<PieceFont>>,
    templates: Res<PieceTemplates>,
    board_pieces: Res<BoardPieces>,
    over: Res<BoardOverride>,
    session: Res<Session>,
    presentation: Res<Presentation>,
    visible: Query<&VisibleIn>,
    clock: Query<(), With<ClockRoot>>,
) {
    let texture_ok = |tex: &Option<Handle<Image>>| {
        tex.as_ref()
            .is_none_or(|h| assets.is_loaded_with_dependencies(h.id()))
    };
    let textures_loaded = materials.iter().all(|(_, m)| {
        texture_ok(&m.base_color_texture)
            && texture_ok(&m.normal_map_texture)
            && texture_ok(&m.metallic_roughness_texture)
            && texture_ok(&m.occlusion_texture)
            && texture_ok(&m.emissive_texture)
    });
    let font_loaded = font
        .as_ref()
        .is_some_and(|f| assets.is_loaded_with_dependencies(f.0.id()));
    let facts = Facts {
        assets_loaded: font_loaded && textures_loaded && templates.is_complete(),
        templates: templates.is_complete(),
        pieces: pieces_synced(&board_pieces, &session, &over),
        board: visible.iter().any(|rule| rule.shows(*presentation)),
        clock: !presentation.is_3d() || !clock.is_empty(),
    };
    ready.tracker.update(facts, time.delta());
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: Duration = Duration::from_millis(16);

    fn all_ready() -> Facts {
        Facts {
            assets_loaded: true,
            templates: true,
            pieces: true,
            board: true,
            clock: true,
        }
    }

    fn tracker() -> ReadyTracker {
        ReadyTracker::new(3, Duration::from_secs(1))
    }

    #[test]
    fn starts_loading_and_stays_while_something_is_missing() {
        let mut t = tracker();
        let facts = Facts {
            clock: false,
            ..all_ready()
        };
        assert_eq!(t.update(facts, DT), &ReadyState::Loading);
    }

    #[test]
    fn becomes_ready_only_after_the_stable_frames() {
        let mut t = tracker();
        assert_eq!(
            t.update(all_ready(), DT),
            &ReadyState::Stabilizing { stable: 1 }
        );
        assert_eq!(
            t.update(all_ready(), DT),
            &ReadyState::Stabilizing { stable: 2 }
        );
        assert!(matches!(
            t.update(all_ready(), DT),
            ReadyState::Ready { .. }
        ));
    }

    #[test]
    fn losing_a_fact_resets_the_stability_count() {
        let mut t = tracker();
        t.update(all_ready(), DT);
        t.update(all_ready(), DT);
        let lost = Facts {
            pieces: false,
            ..all_ready()
        };
        assert_eq!(t.update(lost, DT), &ReadyState::Loading);
        assert_eq!(
            t.update(all_ready(), DT),
            &ReadyState::Stabilizing { stable: 1 }
        );
    }

    #[test]
    fn ready_records_load_time_and_is_final() {
        let mut t = ReadyTracker::new(2, Duration::from_secs(1));
        t.update(Facts::default(), Duration::from_millis(100));
        t.update(all_ready(), Duration::from_millis(100));
        t.update(all_ready(), Duration::from_millis(100));
        assert_eq!(
            t.state(),
            &ReadyState::Ready {
                after: Duration::from_millis(300)
            }
        );
        t.update(Facts::default(), Duration::from_secs(5));
        assert!(matches!(t.state(), ReadyState::Ready { .. }));
    }

    #[test]
    fn times_out_naming_what_is_missing() {
        let mut t = tracker();
        let facts = Facts {
            assets_loaded: false,
            templates: false,
            ..all_ready()
        };
        t.update(facts, Duration::from_millis(600));
        let state = t.update(facts, Duration::from_millis(600)).clone();
        let ReadyState::TimedOut { missing, .. } = state else {
            panic!("deveria esgotar o prazo");
        };
        assert_eq!(
            missing,
            vec!["assets (glTF, texturas ou fonte)", "moldes das 12 peças"]
        );
    }

    #[test]
    fn timeout_option_is_parsed_with_a_safe_default() {
        let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            timeout_from_args(&args(&["--ready-timeout", "12.5"])),
            Duration::from_secs_f32(12.5)
        );
        for bad in [
            args(&[]),
            args(&["--ready-timeout", "x"]),
            args(&["--ready-timeout", "0"]),
        ] {
            assert_eq!(
                timeout_from_args(&bad),
                Duration::from_secs_f32(DEFAULT_TIMEOUT_S)
            );
        }
    }
}
