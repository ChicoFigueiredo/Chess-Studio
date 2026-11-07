//! Painel Anotações de estudo: comentário do usuário por Lance, só **fora** de Partida
//! competitiva (UI-08). As anotações não são parte da Partida (ADR-0001): ficam num recurso
//! próprio, gravadas pelo `PersistencePlugin` como preferência por Partida e exportadas no
//! PGN como comentários `{...}` (`PgnOptions::study_notes`).

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::notation;
use crate::session::Session;
use crate::training_ui::TrainingUi;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::BTreeMap;

/// Anotações da Partida atual, por meio-lance (`0`: antes do primeiro Lance).
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct StudyNotes {
    /// Época da `Session` a que as anotações pertencem (muda com Nova Partida/FEN).
    epoch: u64,
    notes: BTreeMap<usize, String>,
    dirty: bool,
}

impl StudyNotes {
    /// Anotações lidas do banco para a Partida de `epoch`.
    pub fn loaded(epoch: u64, notes: BTreeMap<usize, String>) -> Self {
        Self {
            epoch,
            notes,
            dirty: false,
        }
    }

    pub fn get(&self, ply: usize) -> Option<&str> {
        self.notes.get(&ply).map(String::as_str)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Grava (ou apaga, se vazia) a anotação do Lance.
    pub fn set(&mut self, ply: usize, text: &str) {
        let changed = if text.trim().is_empty() {
            self.notes.remove(&ply).is_some()
        } else if self.notes.get(&ply).map(String::as_str) == Some(text) {
            false
        } else {
            self.notes.insert(ply, text.to_owned());
            true
        };
        self.dirty |= changed;
    }

    /// Anotações para o PGN da Partida de `epoch` (vazio se forem de outra Partida).
    pub fn export(&self, epoch: u64) -> Vec<(usize, String)> {
        if epoch != self.epoch {
            return Vec::new();
        }
        self.notes.iter().map(|(p, t)| (*p, t.clone())).collect()
    }

    /// Acompanha a Partida: Nova Partida/FEN começam sem anotações.
    pub fn follow(&mut self, epoch: u64) {
        if epoch != self.epoch {
            self.epoch = epoch;
            self.notes.clear();
            self.dirty = false;
        }
    }

    /// Anotações a gravar (se mudaram desde a última gravação) para a Partida de `epoch`.
    pub fn take_dirty(&mut self, epoch: u64) -> Option<&BTreeMap<usize, String>> {
        let dirty = std::mem::take(&mut self.dirty);
        (dirty && epoch == self.epoch).then_some(&self.notes)
    }
}

/// Anotações em JSON (`{"3": "texto"}`) e de volta; ilegível vira vazio.
pub fn notes_to_json(notes: &BTreeMap<usize, String>) -> serde_json::Value {
    serde_json::Value::Object(
        notes
            .iter()
            .map(|(p, t)| (p.to_string(), serde_json::Value::String(t.clone())))
            .collect(),
    )
}

pub fn notes_from_json(value: Option<serde_json::Value>) -> BTreeMap<usize, String> {
    let Some(serde_json::Value::Object(map)) = value else {
        return BTreeMap::new();
    };
    map.into_iter()
        .filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_owned())))
        .filter(|(_, t)| !t.trim().is_empty())
        .collect()
}

fn ply_title(i18n: &I18n, session: &Session, ply: usize) -> String {
    match ply
        .checked_sub(1)
        .and_then(|i| session.snapshot().history.get(i))
    {
        Some(entry) => i18n.t_with(
            "study-ply",
            &[("ply", &ply.to_string()), ("san", &entry.san)],
        ),
        None => i18n.t("study-start"),
    }
}

fn study_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    if world
        .get_resource::<TrainingUi>()
        .is_some_and(TrainingUi::session_swapped)
    {
        ui.weak(i18n.t("study-unavailable-training"));
        return;
    }
    world.resource_scope(|world, mut session: Mut<Session>| {
        world.resource_scope(|_, mut notes: Mut<StudyNotes>| {
            let ply = notation::current_ply(&session);
            ui.strong(ply_title(i18n, &session, ply));
            let mut text = notes.get(ply).unwrap_or_default().to_owned();
            let edited = ui
                .add(
                    egui::TextEdit::multiline(&mut text)
                        .id_salt(("study-note", ply))
                        .hint_text(i18n.t("study-hint"))
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            if edited {
                notes.set(ply, &text);
            }
            ui.small(i18n.t("study-note-no-effect"));
            if notes.len() == 0 {
                return;
            }
            ui.separator();
            ui.strong(i18n.t_with("study-list", &[("count", &notes.len().to_string())]));
            let mut go_to = None;
            for (p, note) in notes.export(notes.epoch) {
                let first_line = note.lines().next().unwrap_or_default();
                let label = format!("{} — {first_line}", ply_title(i18n, &session, p));
                if ui.selectable_label(p == ply, label).clicked() {
                    go_to = Some(p);
                }
            }
            if let Some(p) = go_to {
                notation::navigate(&mut session, p);
            }
        });
    });
}

/// Mantém as anotações presas à Partida atual (Treino à parte: a `Session` trocada é a de
/// um Exercício, então não se mexe nas anotações da Partida guardada).
fn follow_game(session: Res<Session>, training: Res<TrainingUi>, mut notes: ResMut<StudyNotes>) {
    if !training.session_swapped() && session.epoch() != notes.epoch {
        notes.follow(session.epoch());
    }
}

/// Evidência (`--screenshot ... --note "texto"`): anota o Lance atual uma vez.
fn capture_note(
    session: Res<Session>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut notes: ResMut<StudyNotes>,
    mut frame: Local<u32>,
) {
    if capture.is_none() {
        return;
    }
    *frame += 1;
    if *frame == 20
        && let Some(text) = crate::capture::option_value("--note")
    {
        notes.set(notation::current_ply(&session), &text);
    }
}

pub struct StudyPlugin;

impl Plugin for StudyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StudyNotes>().add_systems(
            Update,
            (follow_game.before(crate::play::PlaySet), capture_note),
        );
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::STUDY,
                label_key: "dock-panel-study",
                access: PanelAccess::OutsideCompetitive,
                ui: study_panel_ui,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_set_cleared_and_marked_dirty_only_when_they_change() {
        let mut notes = StudyNotes::default();
        assert!(notes.take_dirty(0).is_none());
        notes.set(3, "Ataca e5");
        assert_eq!(notes.get(3), Some("Ataca e5"));
        assert_eq!(notes.take_dirty(0).map(BTreeMap::len), Some(1));
        assert!(notes.take_dirty(0).is_none(), "gravou: não está mais suja");
        notes.set(3, "Ataca e5");
        assert!(notes.take_dirty(0).is_none(), "igual: nada a gravar");
        notes.set(3, "   ");
        assert_eq!(notes.get(3), None, "vazia apaga");
        assert_eq!(notes.take_dirty(0).map(BTreeMap::len), Some(0));
    }

    #[test]
    fn a_new_game_starts_without_notes_and_never_exports_the_old_ones() {
        let mut notes = StudyNotes::default();
        notes.set(1, "e4 central");
        assert_eq!(notes.export(0), vec![(1, "e4 central".to_owned())]);
        assert!(notes.export(1).is_empty(), "outra Partida");
        notes.follow(1);
        assert_eq!(notes.len(), 0);
        assert!(notes.take_dirty(1).is_none());
    }

    #[test]
    fn notes_survive_the_json_round_trip_and_bad_json_is_empty() {
        let mut map = BTreeMap::new();
        map.insert(0, "Posição inicial".to_owned());
        map.insert(12, "Sacrifício".to_owned());
        assert_eq!(notes_from_json(Some(notes_to_json(&map))), map);
        assert!(notes_from_json(None).is_empty());
        assert!(notes_from_json(Some(serde_json::json!([1, 2]))).is_empty());
        assert!(notes_from_json(Some(serde_json::json!({"x": "a", "2": 7}))).is_empty());
    }
}
