//! Perfis de treino na tela de Treino (C11, US-27): escolher um Perfil, ver o avanço
//! passo a passo e continuar do próximo Exercício. O avanço fica no SQLite
//! (`Store::record_profile_step`); o catálogo de Perfis vem de arquivo versionado.

use super::{Action, TrainingUi, View, with_store};
use crate::i18n::I18n;
use crate::persistence::PersistenceHandle;
use bevy::log::warn;
use bevy_egui::egui;
use chess_training::{Difficulty, RunSummary, TrainingProfile};
use std::collections::BTreeSet;

pub(super) fn difficulty_key(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Easy => "profiles-difficulty-easy",
        Difficulty::Medium => "profiles-difficulty-medium",
        Difficulty::Hard => "profiles-difficulty-hard",
    }
}

impl TrainingUi {
    /// Conjunto de Exercícios já cumpridos no Perfil de treino.
    pub(super) fn done_in(&self, profile: &TrainingProfile) -> BTreeSet<String> {
        self.progress.get(&profile.id).cloned().unwrap_or_default()
    }

    /// Relê do banco o avanço de cada Perfil de treino e qual está ativo.
    pub(super) fn refresh_profiles(&mut self, handle: Option<&PersistenceHandle>) {
        let ids: Vec<String> = self.profiles.iter().map(|p| p.id.clone()).collect();
        let loaded = with_store(&mut self.local, handle, |store, player| {
            let mut progress = std::collections::BTreeMap::new();
            for id in &ids {
                if let Ok(done) = store.profile_progress(player, id) {
                    progress.insert(id.clone(), done);
                }
            }
            (
                progress,
                store.active_training_profile(player).ok().flatten(),
            )
        });
        if let Some((progress, active)) = loaded {
            self.progress = progress;
            self.active_profile = active;
        }
    }

    /// Relê as Resoluções datadas e recalcula a aba Evolução.
    pub(super) fn refresh_evolution(&mut self, handle: Option<&PersistenceHandle>) {
        let runs = with_store(&mut self.local, handle, |store, player| {
            store.dated_runs(&chess_store::RunQuery {
                profile: Some(player),
                ..chess_store::RunQuery::default()
            })
        });
        if let Some(Ok(runs)) = runs {
            self.evolution_data = super::evolution::EvolutionData::compute(&runs, &self.profiles);
        }
    }

    /// Uma Resolução acertada cumpre o passo nos Perfis que incluem o Exercício.
    pub(super) fn advance_profiles(
        &mut self,
        summary: &RunSummary,
        handle: Option<&PersistenceHandle>,
    ) {
        let profiles = self.profiles.clone();
        let done = with_store(&mut self.local, handle, |store, player| {
            store.advance_profiles(player, &profiles, summary)
        });
        if let Some(Err(err)) = done {
            warn!("Treino: não foi possível gravar o avanço do Perfil: {err}");
        }
    }

    /// Escolhe (ou larga) o Perfil de treino que está sendo seguido.
    pub(super) fn follow_profile(
        &mut self,
        id: Option<String>,
        handle: Option<&PersistenceHandle>,
    ) {
        let chosen = id.clone();
        let saved = with_store(&mut self.local, handle, |store, player| {
            store.set_active_training_profile(player, chosen.as_deref())
        });
        if let Some(Err(err)) = saved {
            warn!("Treino: não foi possível gravar o Perfil ativo: {err}");
        }
        self.active_profile = id;
    }

    /// O próximo Exercício do Perfil ativo (para o atalho "Continuar").
    pub(super) fn next_in_active(&self) -> Option<(&TrainingProfile, String)> {
        let profile = self.profiles.get(self.active_profile.as_deref()?)?;
        let next = profile.next_step(&self.done_in(profile))?;
        Some((profile, next.exercise.clone()))
    }
}

/// Atalho no topo da lista: "Continuar <Perfil>: <Exercício>".
pub(super) fn continue_bar(ui: &mut egui::Ui, i18n: &I18n, state: &TrainingUi) -> Option<Action> {
    let mut action = None;
    if let Some((profile, next)) = state.next_in_active() {
        let title = state
            .catalog
            .get(&next)
            .map_or(next.as_str(), |e| e.title.as_str());
        ui.horizontal(|ui| {
            if ui
                .button(i18n.t_with("profiles-continue", &[("name", &profile.name)]))
                .clicked()
            {
                action = Some(Action::Start(next.clone()));
            }
            ui.small(title);
        });
    }
    action
}

/// Tela de Perfis: lista com avanço, passos com dificuldade e os botões de seguir/continuar.
pub(super) fn view(ui: &mut egui::Ui, i18n: &I18n, state: &TrainingUi) -> Option<Action> {
    let mut action = None;
    ui.strong(i18n.t("profiles-heading"));
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for profile in state.profiles.iter() {
                let done = state.done_in(profile);
                let active = state.active_profile.as_deref() == Some(profile.id.as_str());
                let percent = profile.percent(&done);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&profile.name);
                        if active {
                            ui.label(
                                egui::RichText::new(i18n.t("profiles-active"))
                                    .color(egui::Color32::from_rgb(54, 122, 196)),
                            );
                        }
                    });
                    ui.small(&profile.description);
                    ui.add(
                        egui::ProgressBar::new(percent as f32 / 100.0).text(i18n.t_with(
                            "profiles-progress",
                            &[
                                ("done", &profile.completed(&done).to_string()),
                                ("total", &profile.steps.len().to_string()),
                                ("pct", &percent.to_string()),
                            ],
                        )),
                    );
                    for (i, step) in profile.steps.iter().enumerate() {
                        let title = state
                            .catalog
                            .get(&step.exercise)
                            .map_or(step.exercise.as_str(), |e| e.title.as_str());
                        let mark = if done.contains(&step.exercise) {
                            "[x]"
                        } else {
                            "[ ]"
                        };
                        ui.small(format!(
                            "{mark} {}. {title} — {}",
                            i + 1,
                            i18n.t(difficulty_key(step.difficulty))
                        ));
                    }
                    ui.horizontal(|ui| {
                        if let Some(next) = profile.next_step(&done) {
                            let label = if done.is_empty() {
                                "profiles-start"
                            } else {
                                "profiles-resume"
                            };
                            if ui.button(i18n.t(label)).clicked() {
                                action = Some(Action::FollowAndStart(
                                    profile.id.clone(),
                                    next.exercise.clone(),
                                ));
                            }
                        } else {
                            ui.label(i18n.t("profiles-complete"));
                        }
                        if active && ui.small_button(i18n.t("profiles-unfollow")).clicked() {
                            action = Some(Action::Unfollow);
                        }
                    });
                });
            }
        });
    if ui.button(i18n.t("training-back")).clicked() {
        action = Some(Action::Show(View::List));
    }
    action
}
