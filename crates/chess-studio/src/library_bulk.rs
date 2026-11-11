//! Aba "Importações" da Biblioteca (CX-19, #242): selecionar e apagar em lote as Partidas
//! importadas de um Provedor ou de uma importação (por nome e data), com confirmação e
//! contagem. As Partidas jogadas no app **nunca** entram aqui: a lista vem de
//! `imported_games`, uma tabela que não tem Partida própria.
//!
//! O modelo da seleção (quem cobre quem, quantas Partidas saem) é puro e testado; a tela só
//! desenha e junta pedidos. Só Apresentação (ADR-0001).

use crate::i18n::I18n;
use crate::library_ui::{mode_key, source_key};
use crate::persistence::PersistenceHandle;
use crate::short_date::date_text;
use crate::training_ui::with_store;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, StatusTone, cs_card_frame, cs_checkbox, cs_status_badge,
    icon_font, palette, strong_font,
};
use bevy_egui::egui::{self, RichText, Ui};
use chess_store::{BulkCount, BulkDeleted, ImportGroup, ImportScope, ImportSource, Store};
use egui_phosphor::regular as icon;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Releitura automática da lista, para o que outra aba importou aparecer sem trocar de aba.
const AUTO_REFRESH: Duration = Duration::from_secs(3);

/// Uma importação, identificada por Provedor, nome e instante.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BatchKey {
    pub provider: Option<String>,
    pub source: Option<String>,
    pub retrieved_at_ms: i64,
}

impl BatchKey {
    pub(crate) fn of(group: &ImportGroup) -> Self {
        Self {
            provider: group.provider.clone(),
            source: group.source.clone(),
            retrieved_at_ms: group.retrieved_at_ms,
        }
    }
}

/// O que a pessoa marcou: Provedores inteiros e importações avulsas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    providers: BTreeSet<Option<String>>,
    batches: BTreeSet<BatchKey>,
}

impl Selection {
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.providers.is_empty() && self.batches.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Marcar um Provedor cobre todas as importações dele: as marcas avulsas dessas somem.
    pub(crate) fn set_provider(&mut self, provider: &Option<String>, on: bool) {
        if on {
            self.providers.insert(provider.clone());
            self.batches.retain(|b| &b.provider != provider);
        } else {
            self.providers.remove(provider);
        }
    }

    pub(crate) fn set_batch(&mut self, key: &BatchKey, on: bool) {
        if on {
            self.batches.insert(key.clone());
        } else {
            self.batches.remove(key);
        }
    }

    pub(crate) fn provider_on(&self, provider: &Option<String>) -> bool {
        self.providers.contains(provider)
    }

    /// Uma importação conta como marcada se ela ou o Provedor dela está marcado.
    pub(crate) fn batch_on(&self, key: &BatchKey) -> bool {
        self.providers.contains(&key.provider) || self.batches.contains(key)
    }

    /// Os alvos a apagar, sem sobreposição: cada Provedor marcado e as importações avulsas.
    pub(crate) fn scopes(&self) -> Vec<ImportScope> {
        let mut scopes: Vec<ImportScope> = self
            .providers
            .iter()
            .map(|p| ImportScope::Provider(p.clone()))
            .collect();
        scopes.extend(self.batches.iter().map(|b| ImportScope::Batch {
            provider: b.provider.clone(),
            source: b.source.clone(),
            retrieved_at_ms: b.retrieved_at_ms,
        }));
        scopes
    }

    /// Descarta marcas de importações que já não existem (apagadas, ou de outro Perfil local).
    fn retain_existing(&mut self, groups: &[ImportGroup]) {
        self.batches
            .retain(|b| groups.iter().any(|g| &BatchKey::of(g) == b));
        self.providers
            .retain(|p| groups.iter().any(|g| &g.provider == p));
    }
}

/// Provedores presentes na lista, com o total de Partidas de cada um (os mais cheios primeiro).
pub(crate) fn provider_totals(groups: &[ImportGroup]) -> Vec<(Option<String>, usize)> {
    let mut totals: Vec<(Option<String>, usize)> = Vec::new();
    for group in groups {
        match totals.iter_mut().find(|(p, _)| p == &group.provider) {
            Some((_, n)) => *n += group.games,
            None => totals.push((group.provider.clone(), group.games)),
        }
    }
    totals.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    totals
}

/// A confirmação aberta: o que será apagado, já contado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Confirm {
    pub scopes: Vec<ImportScope>,
    pub count: BulkCount,
}

/// Como terminou o último apagamento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Deleted(BulkDeleted),
    Failed,
}

/// O que a tela pede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Ask,
    Confirm,
    Cancel,
    ClearSelection,
}

/// Estado da aba.
#[derive(Debug, Default)]
pub struct BulkUi {
    groups: Vec<ImportGroup>,
    selection: Selection,
    confirm: Option<Confirm>,
    last: Option<Outcome>,
    loaded_at: Option<Instant>,
    dirty: bool,
    ops: Vec<Op>,
}

impl BulkUi {
    /// A lista vem do banco do Perfil local ativo; relê quando mudou ou envelheceu.
    pub(crate) fn refresh(
        &mut self,
        local: &mut Option<Mutex<Store>>,
        handle: Option<&PersistenceHandle>,
    ) {
        let due = self.loaded_at.is_none_or(|at| at.elapsed() >= AUTO_REFRESH);
        if !self.dirty && !due {
            return;
        }
        let loaded = with_store(local, handle, |store, profile| store.import_groups(profile));
        if let Some(Ok(groups)) = loaded {
            self.selection.retain_existing(&groups);
            self.groups = groups;
        }
        self.loaded_at = Some(Instant::now());
        self.dirty = false;
    }

    /// Executa os pedidos do quadro. Devolve `true` se apagou Partidas (a Biblioteca relê).
    pub(crate) fn apply(
        &mut self,
        local: &mut Option<Mutex<Store>>,
        handle: Option<&PersistenceHandle>,
    ) -> bool {
        let mut changed = false;
        for op in std::mem::take(&mut self.ops) {
            match op {
                Op::ClearSelection => self.selection.clear(),
                Op::Cancel => self.confirm = None,
                Op::Ask => {
                    let scopes = self.selection.scopes();
                    let counted = with_store(local, handle, |store, profile| {
                        let mut total = BulkCount::default();
                        for scope in &scopes {
                            let one = store.count_imported_in(profile, scope)?;
                            total.to_delete += one.to_delete;
                            total.kept_other_origin += one.kept_other_origin;
                        }
                        Ok::<_, chess_store::StoreError>(total)
                    });
                    self.last = None;
                    if let Some(Ok(count)) = counted {
                        self.confirm = Some(Confirm { scopes, count });
                    }
                }
                Op::Confirm => {
                    let Some(confirm) = self.confirm.take() else {
                        continue;
                    };
                    let done = with_store(local, handle, |store, profile| {
                        let mut total = BulkDeleted::default();
                        for scope in &confirm.scopes {
                            let one = store.delete_imported_in(profile, scope)?;
                            total.deleted += one.deleted;
                            total.kept_other_origin += one.kept_other_origin;
                        }
                        Ok::<_, chess_store::StoreError>(total)
                    });
                    self.last = Some(match done {
                        Some(Ok(total)) => Outcome::Deleted(total),
                        _ => Outcome::Failed,
                    });
                    self.selection.clear();
                    self.dirty = true;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Abre a confirmação já contada (capturas de evidência).
    pub(crate) fn demo_confirm(
        &mut self,
        local: &mut Option<Mutex<Store>>,
        select: impl FnOnce(&[ImportGroup], &mut Selection),
    ) {
        self.refresh(local, None);
        let groups = self.groups.clone();
        select(&groups, &mut self.selection);
        self.ops.push(Op::Ask);
        self.apply(local, None);
    }

    // --- Tela ---------------------------------------------------------------------------

    pub(crate) fn draw(&mut self, ui: &mut Ui, ctx: &egui::Context, i18n: &I18n) {
        let p = palette(ui);
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(RichText::new(i18n.t("bulk-intro")).color(p.ink_muted));
        if let Some(last) = self.last {
            self.result_line(ui, ctx, i18n, last);
        }
        if self.groups.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(i18n.t("bulk-empty")).color(p.ink));
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width() - 28.0);
                self.provider_rows(ui, ctx, i18n);
                self.batch_rows(ui, ctx, i18n);
                ui.add_space(8.0);
                self.footer(ui, ctx, i18n);
            });
        if self.confirm.is_some() {
            self.dialog(ctx, i18n);
        }
    }

    fn result_line(&self, ui: &mut Ui, ctx: &egui::Context, i18n: &I18n, last: Outcome) {
        let p = palette(ui);
        let (glyph, color, text) = match last {
            Outcome::Deleted(done) => {
                let mut text = count_text(i18n, "bulk-done", done.deleted);
                if done.kept_other_origin > 0 {
                    text.push(' ');
                    text.push_str(&count_text(i18n, "bulk-done-kept", done.kept_other_origin));
                }
                (icon::CHECK_CIRCLE, p.success, text)
            }
            Outcome::Failed => (icon::WARNING_CIRCLE, p.danger, i18n.t("bulk-failed")),
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(glyph).font(icon_font(ctx, 14.0)).color(color));
            ui.label(RichText::new(text).color(p.ink));
        });
    }

    fn provider_rows(&mut self, ui: &mut Ui, ctx: &egui::Context, i18n: &I18n) {
        let p = palette(ui);
        ui.add_space(4.0);
        ui.label(
            RichText::new(i18n.t("bulk-by-provider"))
                .font(strong_font(ctx, 14.0))
                .color(p.ink),
        );
        for (provider, games) in provider_totals(&self.groups) {
            let mut on = self.selection.provider_on(&provider);
            let name = provider_name(i18n, provider.as_deref());
            cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if cs_checkbox(ui, &mut on, &name).changed() {
                        self.selection.set_provider(&provider, on);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(count_text(i18n, "bulk-games", games)).color(p.ink));
                    });
                });
                ui.label(
                    RichText::new(i18n.t("bulk-provider-hint"))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                );
            });
        }
    }

    fn batch_rows(&mut self, ui: &mut Ui, ctx: &egui::Context, i18n: &I18n) {
        let p = palette(ui);
        ui.add_space(12.0);
        ui.label(
            RichText::new(i18n.t("bulk-by-import"))
                .font(strong_font(ctx, 14.0))
                .color(p.ink),
        );
        let groups = self.groups.clone();
        for group in &groups {
            let key = BatchKey::of(group);
            let covered = self.selection.provider_on(&key.provider);
            let mut on = self.selection.batch_on(&key);
            let title = group
                .source
                .clone()
                .unwrap_or_else(|| i18n.t("bulk-unnamed"));
            cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!covered, |ui| {
                        if cs_checkbox(ui, &mut on, &title).changed() {
                            self.selection.set_batch(&key, on);
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(count_text(i18n, "bulk-games", group.games)).color(p.ink),
                        );
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let provider = provider_name(i18n, group.provider.as_deref());
                    cs_status_badge(ui, StatusTone::Neutral, icon::FILE_ARROW_DOWN, &provider);
                    ui.label(
                        RichText::new(i18n.t_with(
                            "bulk-import-line",
                            &[
                                ("mode", &i18n.t(mode_key(group.mode))),
                                ("date", &date_text(group.retrieved_at_ms)),
                            ],
                        ))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                    );
                });
            });
        }
    }

    /// "N marcadas" e os botões "Apagar..." e "Limpar seleção".
    fn footer(&mut self, ui: &mut Ui, ctx: &egui::Context, i18n: &I18n) {
        let p = palette(ui);
        let marked = self.selection.scopes().len();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ui
                .add(
                    CsButton::new(&i18n.t("bulk-delete"))
                        .icon(icon::TRASH)
                        .variant(ButtonVariant::Danger)
                        .enabled(marked > 0),
                )
                .clicked()
            {
                self.ops.push(Op::Ask);
            }
            if marked > 0
                && ui
                    .add(
                        CsButton::new(&i18n.t("bulk-clear-selection"))
                            .variant(ButtonVariant::Subtle),
                    )
                    .clicked()
            {
                self.ops.push(Op::ClearSelection);
            }
            if marked == 0 {
                ui.label(
                    RichText::new(i18n.t("bulk-nothing-marked"))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                );
            }
        });
        let _ = ctx;
    }

    /// A confirmação, modal: quantas Partidas saem, de onde, e o que fica.
    fn dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        egui::Area::new(egui::Id::new("bulk-delete-dim"))
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.viewport_rect();
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
            });
        let nothing = confirm.count.to_delete == 0;
        egui::Window::new(i18n.t("bulk-dialog-title"))
            .id(egui::Id::new("bulk-delete-dialog"))
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let p = palette(ui);
                ui.set_width(440.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                let headline = if nothing {
                    i18n.t("bulk-dialog-nothing")
                } else {
                    count_text(i18n, "bulk-dialog-count", confirm.count.to_delete)
                };
                ui.label(
                    RichText::new(headline)
                        .font(strong_font(ctx, 16.0))
                        .color(p.ink),
                );
                for scope in &confirm.scopes {
                    ui.horizontal_top(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!("•  {}", self.scope_label(i18n, scope)))
                                .color(p.ink),
                        );
                    });
                }
                if confirm.count.kept_other_origin > 0 {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            RichText::new(icon::INFO)
                                .font(icon_font(ctx, 14.0))
                                .color(p.info),
                        );
                        ui.label(
                            RichText::new(count_text(
                                i18n,
                                "bulk-dialog-kept",
                                confirm.count.kept_other_origin,
                            ))
                            .color(p.ink),
                        );
                    });
                }
                ui.horizontal_top(|ui| {
                    ui.label(
                        RichText::new(icon::SHIELD_CHECK)
                            .font(icon_font(ctx, 14.0))
                            .color(p.success),
                    );
                    ui.label(RichText::new(i18n.t("bulk-dialog-safe")).color(p.ink));
                });
                ui.label(
                    RichText::new(i18n.t("bulk-dialog-undo"))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if !nothing
                        && ui
                            .add(
                                CsButton::new(&count_text(
                                    i18n,
                                    "bulk-dialog-confirm",
                                    confirm.count.to_delete,
                                ))
                                .icon(icon::TRASH)
                                .variant(ButtonVariant::Danger),
                            )
                            .clicked()
                    {
                        self.ops.push(Op::Confirm);
                    }
                    if ui
                        .add(
                            CsButton::new(&i18n.t("bulk-dialog-cancel"))
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                    {
                        self.ops.push(Op::Cancel);
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.ops.push(Op::Cancel);
        }
    }

    /// Uma linha da confirmação por alvo: o Provedor inteiro ou a importação com nome e data.
    fn scope_label(&self, i18n: &I18n, scope: &ImportScope) -> String {
        match scope {
            ImportScope::Provider(provider) => {
                let games = self
                    .groups
                    .iter()
                    .filter(|g| &g.provider == provider)
                    .map(|g| g.games)
                    .sum();
                i18n.t_with(
                    "bulk-scope-provider",
                    &[
                        ("provider", &provider_name(i18n, provider.as_deref())),
                        ("games", &count_text(i18n, "bulk-games", games)),
                    ],
                )
            }
            ImportScope::Batch {
                provider,
                source,
                retrieved_at_ms,
            } => {
                let games = self
                    .groups
                    .iter()
                    .find(|g| {
                        &g.provider == provider
                            && &g.source == source
                            && g.retrieved_at_ms == *retrieved_at_ms
                    })
                    .map_or(0, |g| g.games);
                i18n.t_with(
                    "bulk-scope-batch",
                    &[
                        ("name", source.as_deref().unwrap_or(&i18n.t("bulk-unnamed"))),
                        ("date", &date_text(*retrieved_at_ms)),
                        ("games", &count_text(i18n, "bulk-games", games)),
                    ],
                )
            }
        }
    }
}

/// O nome do Provedor da Origem; sem Provedor, "Arquivo sem origem conhecida".
fn provider_name(i18n: &I18n, provider: Option<&str>) -> String {
    provider
        .and_then(ImportSource::from_provider_id)
        .map_or_else(
            || i18n.t("library-origin-unknown"),
            |s| i18n.t(source_key(s)),
        )
}

/// "1 Partida" / "N Partidas": a chave termina em `-one` ou `-many`.
pub(crate) fn count_text(i18n: &I18n, base: &str, n: usize) -> String {
    let key = if n == 1 {
        format!("{base}-one")
    } else {
        format!("{base}-many")
    };
    i18n.t_with(&key, &[("count", &n.to_string())])
}

/// Importações fictícias para as capturas: dois Provedores, um arquivo e uma Partida repetida.
/// Os nomes são inventados (`--library imports`): nunca apelidos de pessoas reais.
pub(crate) fn seed_demo(store: &mut Store) {
    use chess_store::{ImportMode, ImportOptions, Provenance};
    let pgn = |tag: &str, n: u32| {
        (1..=n).fold(String::new(), |mut out, i| {
            out.push_str(&format!(
                "[Event \"Partida {tag} {i}\"]\n[Site \"https://exemplo.test/{tag}{i}\"]\n\
                 [White \"{tag}\"]\n[Black \"adversario{i}\"]\n[Date \"2026.10.{i:02}\"]\n\
                 [Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 {i} 1-0\n\n"
            ));
            out
        })
    };
    let day = 86_400_000_i64;
    let t0 = 1_794_398_400_000_i64;
    let runs = [
        (
            "ana_exemplo",
            "Lichess: ana_exemplo",
            ImportSource::Lichess,
            ImportMode::Api,
            t0 - 6 * day,
            12,
        ),
        (
            "ana_exemplo_b",
            "Lichess: ana_exemplo",
            ImportSource::Lichess,
            ImportMode::Api,
            t0 - day,
            5,
        ),
        (
            "carla_exemplo",
            "Chess.com: carla_exemplo",
            ImportSource::ChessCom,
            ImportMode::Api,
            t0 - 3 * day,
            8,
        ),
        (
            "estudos",
            "estudos-de-aberturas.pgn",
            ImportSource::Other,
            ImportMode::File,
            t0 - 20 * day,
            7,
        ),
    ];
    for (tag, name, source, mode, at, n) in runs {
        let options = ImportOptions::new(Some(name), Provenance::from_source(source, mode, at));
        let _ = store.import_pgn_with(
            store.default_profile().unwrap_or(chess_store::ProfileId(1)),
            &pgn(tag, n),
            &options,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_store::{ImportMode, ImportOptions, ImportedQuery, Provenance};

    fn group(provider: Option<&str>, source: &str, at: i64, games: usize) -> ImportGroup {
        ImportGroup {
            provider: provider.map(str::to_owned),
            source: Some(source.to_owned()),
            mode: ImportMode::Api,
            retrieved_at_ms: at,
            games,
        }
    }

    fn sample() -> Vec<ImportGroup> {
        vec![
            group(Some("lichess"), "a", 3, 5),
            group(Some("lichess"), "a", 1, 12),
            group(Some("chess-com"), "b", 2, 8),
        ]
    }

    #[test]
    fn provider_totals_sum_every_import_of_the_provider_biggest_first() {
        assert_eq!(
            provider_totals(&sample()),
            [
                (Some("lichess".to_owned()), 17),
                (Some("chess-com".to_owned()), 8)
            ]
        );
    }

    #[test]
    fn marking_a_provider_covers_its_imports_and_drops_their_own_marks() {
        let groups = sample();
        let first = BatchKey::of(&groups[0]);
        let third = BatchKey::of(&groups[2]);
        let mut selection = Selection::default();
        selection.set_batch(&first, true);
        selection.set_batch(&third, true);
        assert_eq!(selection.scopes().len(), 2);

        selection.set_provider(&Some("lichess".into()), true);

        assert!(selection.batch_on(&first), "coberta pelo Provedor");
        assert!(selection.batch_on(&third));
        // Sem sobreposição: o Provedor + a importação do outro Provedor.
        let scopes = selection.scopes();
        assert_eq!(scopes.len(), 2);
        assert!(scopes.contains(&ImportScope::Provider(Some("lichess".into()))));
        selection.set_provider(&Some("lichess".into()), false);
        assert!(!selection.batch_on(&first));
        assert!(selection.batch_on(&third));
    }

    #[test]
    fn marks_of_imports_that_no_longer_exist_are_dropped() {
        let mut groups = sample();
        let gone = BatchKey::of(&groups[1]);
        let mut selection = Selection::default();
        selection.set_batch(&gone, true);
        selection.set_provider(&Some("chess-com".into()), true);
        groups.remove(1);
        groups.remove(1);
        selection.retain_existing(&groups);
        assert!(!selection.batch_on(&gone));
        assert!(!selection.provider_on(&Some("chess-com".into())));
    }

    #[test]
    fn counts_read_one_and_many_in_plain_portuguese() {
        let i18n = I18n::pt_br();
        assert_eq!(count_text(&i18n, "bulk-games", 1), "1 Partida");
        assert_eq!(count_text(&i18n, "bulk-games", 12), "12 Partidas");
        assert_eq!(count_text(&i18n, "bulk-games", 0), "0 Partidas");
    }

    /// O fluxo da tela inteiro, sem janela: marcar, contar, confirmar e apagar.
    #[test]
    fn asking_counts_first_and_only_confirming_deletes_and_never_touches_played_games() {
        let mut local = Some(Mutex::new(Store::open_in_memory().unwrap()));
        with_store(&mut local, None, |store, _| seed_demo(store));
        let mut ui = BulkUi::default();
        ui.refresh(&mut local, None);
        assert_eq!(ui.groups.len(), 4);

        ui.selection.set_provider(&Some("lichess".into()), true);
        ui.ops.push(Op::Ask);
        assert!(!ui.apply(&mut local, None), "perguntar não apaga");
        let confirm = ui.confirm.clone().expect("confirmação aberta");
        assert_eq!(confirm.count.to_delete, 17);
        let left = |local: &mut Option<Mutex<Store>>| {
            with_store(local, None, |store, profile| {
                store
                    .list_imported(&ImportedQuery {
                        profile: Some(profile),
                        ..Default::default()
                    })
                    .unwrap()
                    .len()
            })
            .unwrap()
        };
        assert_eq!(left(&mut local), 32);

        ui.ops.push(Op::Cancel);
        ui.apply(&mut local, None);
        assert_eq!(left(&mut local), 32, "cancelar não apaga");

        ui.ops.push(Op::Ask);
        ui.apply(&mut local, None);
        ui.ops.push(Op::Confirm);
        assert!(ui.apply(&mut local, None));
        assert_eq!(left(&mut local), 15);
        assert_eq!(
            ui.last,
            Some(Outcome::Deleted(BulkDeleted {
                deleted: 17,
                kept_other_origin: 0
            }))
        );
        assert!(ui.selection.is_empty());
        ui.refresh(&mut local, None);
        assert!(
            ui.groups
                .iter()
                .all(|g| g.provider.as_deref() != Some("lichess"))
        );
    }

    #[test]
    fn a_failed_import_name_is_never_invented() {
        let mut local = Some(Mutex::new(Store::open_in_memory().unwrap()));
        with_store(&mut local, None, |store, profile| {
            let _ = store.import_pgn_with(
                profile,
                "[Event \"x\"]\n[White \"a\"]\n[Black \"b\"]\n[Result \"1-0\"]\n\n1. e4 1-0\n",
                &ImportOptions::new(None, Provenance::unknown(ImportMode::File, 5)),
            );
        });
        let mut ui = BulkUi::default();
        ui.refresh(&mut local, None);
        assert_eq!(ui.groups[0].source, None);
        assert_eq!(ui.groups[0].provider, None);
    }
}
