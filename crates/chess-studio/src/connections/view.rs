//! A tela da Central de Conexões (egui): título, filtros, busca e os cartões por categoria.
//! Só desenha e junta pedidos ([`Op`]); quem decide é o modelo ([`super::model`]) e quem executa
//! é [`ConnectionsUi::apply`](super::ConnectionsUi::apply).
//!
//! Hierarquia (#225): cada cartão mostra nome, estado, "O que você pode fazer agora", o Switch
//! e uma ação principal; o resto (o que é enviado, custo por serviço, o que vem depois, última
//! consulta, Conexões vinculadas) fica em "Detalhes". Os Provedores que ainda não funcionam
//! vão em cartões menores, depois dos que funcionam.

use super::model::{
    self, ActionSpec, Age, CardAction, CardState, CostFilter, Face, FieldKind, Tier,
};
use super::{CentralParams, ConnectionsUi, LinkForm, Op, RenameForm};
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::persistence::PersistenceHandle;
use crate::provider_consent::ConsentDialog;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, MenuItem, Palette, StatusTone, cs_card_frame,
    cs_detail_row, cs_disclosure, cs_overflow_menu, cs_status_badge, cs_switch, cs_text_field,
    cs_toggle_button, palette, strong_font,
};
use bevy_egui::egui::{self, Color32, RichText, Ui, vec2};
use chess_net::{Provider, ProviderId, ProviderState, Timestamp};
use chess_store::{Connection, ConnectionId};
use egui_phosphor::regular as icon;
use std::collections::{HashMap, HashSet};

/// Largura mínima, em pontos, a partir da qual os cartões vão em duas colunas.
pub(super) const TWO_COLUMNS_FROM: f32 = 980.0;
const CARD_GAP: f32 = 16.0;
/// Folga à direita do conteúdo, para a barra de rolagem.
const SCROLLBAR_GUTTER: f32 = 16.0;
/// Linhas reservadas para "O que você pode fazer agora": todos os cartões completos de uma
/// faixa ficam com a mesma altura e os rodapés se alinham.
const FACE_LINES: f32 = 2.0;
/// Largura do cartão a partir da qual a frase cabe numa linha e só uma é reservada.
const ONE_LINE_FROM: f32 = 540.0;

/// Desenha a página e executa os pedidos do quadro.
pub fn section(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    params: &mut CentralParams,
    profile_name: Option<&str>,
) {
    let handle = params.persistence.as_deref();
    let service: &NetService = &params.service;
    page(
        ui,
        ctx,
        i18n,
        &mut params.ui,
        service,
        &mut params.dialog,
        handle,
        profile_name,
    );
}

/// A página, sem depender dos recursos do Bevy (os testes a desenham direto).
#[allow(clippy::too_many_arguments)]
pub(super) fn page(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    state: &mut ConnectionsUi,
    service: &NetService,
    dialog: &mut ConsentDialog,
    handle: Option<&PersistenceHandle>,
    profile_name: Option<&str>,
) {
    state.refresh(handle);
    let status = service.net().status();
    let last = model::last_queries(&service.net().recent_operations());
    let now = NetService::now();
    let mut ops = Vec::new();
    let p = palette(ui);
    let groups = model::groups(state.filter, &state.query);
    // A rolagem inicial vale por alguns quadros: no primeiro, o egui ainda não sabe a altura.
    let scroll = match state.scroll {
        Some((pixels, frames)) if frames > 0 => {
            state.scroll = Some((pixels, frames - 1));
            Some(pixels)
        }
        _ => None,
    };
    let ConnectionsUi {
        filter,
        query,
        forms,
        notes,
        tests,
        expanded,
        renaming,
        confirm_remove,
        authorized,
        list,
        ..
    } = &mut *state;
    let mut view = View {
        ctx,
        i18n,
        status: &status,
        last: &last,
        now,
        list,
        forms,
        notes,
        tests,
        expanded,
        renaming,
        confirm_remove,
        authorized,
        service,
        ops: &mut ops,
    };
    // A página inteira rola, filtros e busca junto.
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(pixels) = scroll {
        area = area.vertical_scroll_offset(pixels);
    }
    area.show(ui, |ui| {
        // Uma folga à direita: a barra de rolagem flutua sobre o conteúdo e não pode cobrir
        // a borda dos cartões da segunda coluna.
        ui.set_width(ui.available_width() - SCROLLBAR_GUTTER);
        ui.label(
            RichText::new(i18n.t("connections-intro"))
                .color(p.ink_muted)
                .text_style(egui::TextStyle::Body),
        );
        if let Some(name) = profile_name {
            ui.label(
                RichText::new(i18n.t_with("connections-profile", &[("name", name)]))
                    .color(p.ink_muted)
                    .text_style(egui::TextStyle::Small),
            );
        }
        ui.add_space(16.0);
        toolbar(ui, i18n, filter, query);
        if groups.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new(i18n.t("connections-empty")).color(p.ink_muted));
        }
        for group in &groups {
            ui.add_space(24.0);
            view.section_header(ui, group, p);
            ui.add_space(12.0);
            for tier in [Tier::Working, Tier::Soon, Tier::Approval] {
                let providers: Vec<&'static Provider> = group
                    .providers
                    .iter()
                    .copied()
                    .filter(|provider| model::tier_of(provider) == tier)
                    .collect();
                if providers.is_empty() {
                    continue;
                }
                if let Some(key) = tier.key() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(i18n.t(key))
                            .font(strong_font(ctx, 14.0))
                            .color(p.ink_muted),
                    );
                    ui.add_space(8.0);
                }
                view.cards(ui, &providers);
            }
        }
        ui.add_space(24.0);
    });
    state.apply(ops, service, dialog, handle);
}

/// Filtros por custo (botões de alternância, um por vez) e a busca, numa linha que quebra.
fn toolbar(ui: &mut Ui, i18n: &I18n, filter: &mut CostFilter, query: &mut String) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for option in CostFilter::ALL {
            let label = i18n.t_with(
                "connections-filter-chip",
                &[
                    ("name", &i18n.t(option.key())),
                    ("count", &model::count(option).to_string()),
                ],
            );
            let mut on = *filter == option;
            if cs_toggle_button(ui, &mut on, &label).clicked() {
                *filter = option;
            }
        }
        ui.add_space(8.0);
        ui.scope(|ui| {
            ui.spacing_mut().text_edit_width = 280.0;
            cs_text_field(ui, query, &i18n.t("connections-search-hint"), None);
        });
    });
}

struct View<'a> {
    ctx: &'a egui::Context,
    i18n: &'a I18n,
    status: &'a chess_net::NetworkStatus,
    last: &'a HashMap<ProviderId, Timestamp>,
    now: Timestamp,
    list: &'a [Connection],
    forms: &'a mut HashMap<ProviderId, LinkForm>,
    notes: &'a HashMap<ProviderId, super::Note>,
    tests: &'a HashMap<ProviderId, super::probe::TestState>,
    expanded: &'a mut HashSet<ProviderId>,
    renaming: &'a mut Option<RenameForm>,
    confirm_remove: &'a mut Option<ConnectionId>,
    authorized: &'a HashSet<ProviderId>,
    service: &'a NetService,
    ops: &'a mut Vec<Op>,
}

impl View<'_> {
    fn section_header(&self, ui: &mut Ui, group: &model::Group, p: &Palette) {
        let n = group.providers.len();
        let count = if n == 1 {
            self.i18n.t("connections-section-count-one")
        } else {
            self.i18n.t_with(
                "connections-section-count-many",
                &[("count", &n.to_string())],
            )
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.i18n.t(model::category_key(group.category)))
                    .font(strong_font(self.ctx, 20.0))
                    .color(p.ink),
            );
            ui.label(small(count, p.ink_muted));
        });
    }

    /// Os cartões de uma faixa, em uma ou duas colunas conforme a largura.
    fn cards(&mut self, ui: &mut Ui, providers: &[&'static Provider]) {
        let columns = if ui.available_width() >= TWO_COLUMNS_FROM {
            2
        } else {
            1
        };
        if columns == 1 {
            for provider in providers {
                self.card(ui, provider);
                ui.add_space(CARD_GAP);
            }
            return;
        }
        let width = ((ui.available_width() - CARD_GAP) / 2.0).floor();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            for column in 0..2 {
                ui.allocate_ui_with_layout(
                    vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        for provider in providers.iter().skip(column).step_by(2) {
                            self.card(ui, provider);
                            ui.add_space(CARD_GAP);
                        }
                    },
                );
            }
        });
    }

    fn connections_of(&self, provider: ProviderId) -> Vec<&Connection> {
        let mut found: Vec<&Connection> = self
            .list
            .iter()
            .filter(|c| c.provider == provider.key())
            .filter(|c| c.role == chess_store::ConnectionRole::Player)
            .collect();
        found.sort_by_key(|c| !c.is_default);
        found
    }

    fn card(&mut self, ui: &mut Ui, provider: &'static Provider) {
        let p = palette(ui);
        let id = provider.id;
        let linked = !self.connections_of(id).is_empty();
        let net = self.status.state(id).clone();
        let authorized = self.authorized.contains(&id);
        let state = model::card_state(provider, &net, linked, authorized);
        let emphasis = if state.is_compact() {
            CardEmphasis::Quiet
        } else {
            CardEmphasis::Full
        };
        ui.push_id(("provider-card", id.key()), |ui| {
            cs_card_frame(p, emphasis).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                if state.is_compact() {
                    self.compact_body(ui, provider, state, p);
                } else {
                    self.full_body(ui, provider, state, &net, linked, p);
                }
            });
        });
    }

    /// Nome e selo de estado.
    fn header(&self, ui: &mut Ui, provider: &Provider, state: CardState, net: &ProviderState) {
        let p = palette(ui);
        let (tone, glyph) = state_look(state);
        let word = match (state, net) {
            (CardState::Limited, ProviderState::Limited { until }) => self.i18n.t_with(
                "connections-state-limited-in",
                &[(
                    "when",
                    &self.minutes_text(model::minutes_left(self.now, *until)),
                )],
            ),
            _ => self.i18n.t(state.key()),
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(provider.name)
                    .font(strong_font(self.ctx, 16.0))
                    .color(p.ink),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cs_status_badge(ui, tone, glyph, &word)
                    .on_hover_text(self.i18n.t(state.hint_key()));
            });
        });
    }

    /// Cartão menor: nome, estado, uma frase e, se há, o menu "⋯" com instruções e termos.
    fn compact_body(
        &mut self,
        ui: &mut Ui,
        provider: &'static Provider,
        state: CardState,
        p: &Palette,
    ) {
        let (tone, glyph) = state_look(state);
        let word = self.i18n.t(state.key());
        let actions = model::card_actions(provider, state, false);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(provider.name)
                    .font(strong_font(self.ctx, 14.0))
                    .color(p.ink),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.menu(ui, provider, &actions);
                cs_status_badge(ui, tone, glyph, &word)
                    .on_hover_text(self.i18n.t(state.hint_key()));
            });
        });
        let line = match model::face(provider, state) {
            Face::Note(key) => {
                let later = model::later(provider);
                if state == CardState::Planned && !later.is_empty() {
                    self.i18n.t_with(
                        "connections-face-planned-list",
                        &[("list", &self.verbs(&later))],
                    )
                } else {
                    self.i18n.t(key)
                }
            }
            Face::Can(list) => self
                .i18n
                .t_with("connections-face-list", &[("list", &self.verbs(&list))]),
        };
        let lines = if ui.available_width() >= ONE_LINE_FROM {
            1.0
        } else {
            2.0
        };
        reserve_lines(ui, lines, egui::TextStyle::Small, |ui| {
            ui.label(small(line, p.ink_muted));
        });
    }

    fn full_body(
        &mut self,
        ui: &mut Ui,
        provider: &'static Provider,
        state: CardState,
        net: &ProviderState,
        linked: bool,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let id = provider.id;
        self.header(ui, provider, state, net);
        ui.add_space(8.0);
        ui.label(small(i18n.t("connections-face-label"), p.ink_muted));
        let phrase = match model::face(provider, state) {
            Face::Can(list) => {
                i18n.t_with("connections-face-list", &[("list", &self.verbs(&list))])
            }
            Face::Note(key) => i18n.t(key),
        };
        let lines = if ui.available_width() >= ONE_LINE_FROM {
            1.0
        } else {
            FACE_LINES
        };
        reserve_lines(ui, lines, egui::TextStyle::Body, |ui| {
            ui.label(RichText::new(phrase).color(p.ink));
        });
        ui.add_space(8.0);

        let actions = model::card_actions(provider, state, linked);
        let mut open = self.expanded.contains(&id);
        let was_open = open;
        ui.horizontal(|ui| {
            if model::has_switch(provider, state) {
                let mut on = self.service.is_enabled(id);
                if cs_switch(ui, &mut on, &i18n.t("connections-switch")).changed() {
                    self.ops.push(Op::SetEnabled(id, on));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if let Some(primary) = actions.primary {
                    self.action_button(ui, provider, primary);
                }
                self.menu(ui, provider, &actions);
                let hint = if open {
                    i18n.t("connections-details-hide")
                } else {
                    i18n.t("connections-details-show")
                };
                cs_disclosure(ui, &mut open, &i18n.t("connections-details"), &hint);
            });
        });
        if open != was_open {
            if open {
                self.expanded.insert(id);
            } else {
                self.expanded.remove(&id);
            }
        }
        if self.expanded.contains(&id) {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            self.details(ui, provider, state, net, p);
        }
    }

    /// "Detalhes": tudo o que o cartão resumido não mostra.
    fn details(
        &mut self,
        ui: &mut Ui,
        provider: &'static Provider,
        state: CardState,
        net: &ProviderState,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let id = provider.id;
        ui.spacing_mut().item_spacing.y = 8.0;

        cs_detail_row(ui, &i18n.t("connections-detail-sends"), |ui| {
            let text = i18n.t(model::sends_key(provider, state));
            if provider.uses_network() && !self.service.is_enabled(id) {
                ui.label(RichText::new(i18n.t("connections-sends-off-lead")).color(p.ink));
                ui.label(
                    RichText::new(i18n.t_with("connections-sends-when-on", &[("text", &text)]))
                        .color(p.ink_muted),
                );
            } else {
                ui.label(RichText::new(text).color(p.ink));
            }
        });
        cs_detail_row(ui, &i18n.t("connections-detail-identifies"), |ui| {
            let keys = model::auth_keys(provider);
            let items: Vec<String> = keys.iter().map(|key| i18n.t(key)).collect();
            ui.label(RichText::new(items.join(" · ")).color(p.ink));
        });
        cs_detail_row(ui, &i18n.t("connections-detail-cost"), |ui| {
            for (cost, services) in model::cost_groups(provider) {
                let cost_text = i18n.t(model::cost_key(cost));
                let line = if services.is_empty() {
                    cost_text
                } else {
                    let names: Vec<String> = services.iter().map(|key| i18n.t(key)).collect();
                    i18n.t_with(
                        "connections-cost-line",
                        &[("cost", &cost_text), ("services", &self.join(&names))],
                    )
                };
                ui.label(RichText::new(line).color(p.ink));
            }
        });
        let later = model::later(provider);
        if !later.is_empty() {
            let needs_authorization = !model::requirements(provider).is_empty();
            cs_detail_row(ui, &i18n.t("connections-detail-later"), |ui| {
                ui.label(
                    RichText::new(i18n.t_with(
                        "connections-detail-later-list",
                        &[("list", &self.verbs(&later))],
                    ))
                    .color(p.ink),
                );
                if needs_authorization {
                    let text = i18n.t_with(
                        "connections-requires-authorize",
                        &[("provider", provider.name)],
                    );
                    ui.label(status_line(
                        self.ctx,
                        icon::LOCK_KEY,
                        p.ink_muted,
                        &text,
                        p.ink_muted,
                    ));
                }
            });
        }
        if provider.uses_network() {
            cs_detail_row(ui, &i18n.t("connections-detail-last"), |ui| {
                let text = match self.last.get(&id) {
                    None => i18n.t("connections-last-never"),
                    Some(at) => self.age_text(model::age(self.now, *at)),
                };
                ui.label(RichText::new(text).color(p.ink));
            });
        }
        match net {
            ProviderState::Failing { error, .. } => {
                cs_detail_row(ui, &i18n.t("connections-detail-status"), |ui| {
                    let detail = i18n.t(model::error_key(error));
                    let text = i18n.t_with("connections-failing", &[("detail", &detail)]);
                    ui.label(status_line(
                        self.ctx,
                        icon::WARNING_CIRCLE,
                        p.danger,
                        &text,
                        p.ink,
                    ));
                });
            }
            ProviderState::Limited { until } => {
                cs_detail_row(ui, &i18n.t("connections-detail-status"), |ui| {
                    let when = self.minutes_text(model::minutes_left(self.now, *until));
                    let text = i18n.t_with("connections-limited-until", &[("when", &when)]);
                    ui.label(status_line(self.ctx, icon::TIMER, p.warning, &text, p.ink));
                });
            }
            _ => {}
        }
        if provider.docs_url.len() + provider.terms_url.len() > 0 {
            cs_detail_row(ui, &i18n.t("connections-detail-links"), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (action, label, url) in [
                        (
                            CardAction::OpenDocs,
                            "connections-act-docs",
                            provider.docs_url,
                        ),
                        (
                            CardAction::OpenTerms,
                            "connections-act-terms",
                            provider.terms_url,
                        ),
                    ] {
                        if url.is_empty() {
                            continue;
                        }
                        let text = i18n.t(label);
                        let button = CsButton::new(&text)
                            .icon(icon::ARROW_SQUARE_OUT)
                            .variant(ButtonVariant::Secondary);
                        if ui.add(button).clicked() {
                            self.perform(provider, action);
                        }
                    }
                });
            });
        }
        if let Some(note) = self.notes.get(&id) {
            let text = match &note.arg {
                Some((name, value)) => i18n.t_with(note.key, &[(name, value)]),
                None => i18n.t(note.key),
            };
            ui.label(status_line(self.ctx, icon::INFO, p.info, &text, p.ink));
        }
        self.connections(ui, provider, state, p);
    }

    fn join(&self, items: &[String]) -> String {
        match items {
            [] => String::new(),
            [one] => one.clone(),
            [head @ .., last] => self.i18n.t_with(
                "connections-list-and",
                &[("head", &head.join(", ")), ("last", last)],
            ),
        }
    }

    /// Verbos (ids Fluent) numa frase: "vincular seu nome público e testar a Conexão", com a
    /// primeira letra em maiúscula.
    fn verbs(&self, keys: &[&str]) -> String {
        let items: Vec<String> = keys.iter().map(|key| self.i18n.t(key)).collect();
        capitalize(&self.join(&items))
    }

    fn minutes_text(&self, minutes: u64) -> String {
        self.i18n
            .t_with("connections-minutes", &[("count", &minutes.to_string())])
    }

    fn age_text(&self, age: Age) -> String {
        let count = |n: u64| n.to_string();
        match age {
            Age::JustNow => self.i18n.t("connections-ago-now"),
            Age::Minutes(n) => self
                .i18n
                .t_with("connections-ago-minutes", &[("count", &count(n))]),
            Age::Hours(n) => self
                .i18n
                .t_with("connections-ago-hours", &[("count", &count(n))]),
            Age::Days(n) => self
                .i18n
                .t_with("connections-ago-days", &[("count", &count(n))]),
        }
    }

    fn action_label(
        &self,
        provider: &Provider,
        action: CardAction,
        has_connection: bool,
    ) -> String {
        let i18n = self.i18n;
        let id = model::connection_kind(provider) == chess_store::ConnectionKind::PublicId;
        match (action, id, has_connection) {
            (CardAction::Link, false, true) => i18n.t("connections-act-link-another"),
            (CardAction::Link, true, true) => i18n.t("connections-act-link-another-id"),
            (CardAction::Link, false, false) => i18n.t("connections-act-link"),
            (CardAction::Link, true, false) => i18n.t("connections-act-link-id"),
            (CardAction::Test, ..) => i18n.t("connections-act-test"),
            (CardAction::ClearCache, ..) => i18n.t("connections-act-clear-cache"),
            (CardAction::Authorize, ..) => i18n.t("connections-act-authorize"),
            (CardAction::EnterKey, ..) => i18n.t("connections-act-key"),
            (CardAction::OpenDocs, ..) => i18n.t("connections-act-docs"),
            (CardAction::OpenTerms, ..) => i18n.t("connections-act-terms"),
        }
    }

    fn action_icon(action: CardAction) -> &'static str {
        match action {
            CardAction::Link => icon::LINK_SIMPLE,
            CardAction::Test => icon::PLAY,
            CardAction::ClearCache => icon::BROOM,
            CardAction::Authorize => icon::SEAL_CHECK,
            CardAction::EnterKey => icon::KEY,
            CardAction::OpenDocs | CardAction::OpenTerms => icon::ARROW_SQUARE_OUT,
        }
    }

    /// A ação principal do cartão: um botão `Secondary` (o `Primary` é um por tela).
    fn action_button(&mut self, ui: &mut Ui, provider: &Provider, spec: ActionSpec) {
        let has_connection = !self.connections_of(provider.id).is_empty();
        let label = self.action_label(provider, spec.action, has_connection);
        let button = CsButton::new(&label)
            .icon(Self::action_icon(spec.action))
            .variant(ButtonVariant::Secondary)
            .enabled(spec.enabled);
        if ui.add(button).clicked() && spec.enabled {
            self.perform(provider, spec.action);
        }
    }

    /// O menu "⋯" com as demais ações; some se não há nenhuma.
    fn menu(&mut self, ui: &mut Ui, provider: &Provider, actions: &model::Actions) {
        if actions.menu.is_empty() {
            return;
        }
        let i18n = self.i18n;
        let has_connection = !self.connections_of(provider.id).is_empty();
        let labels: Vec<String> = actions
            .menu
            .iter()
            .map(|spec| self.action_label(provider, spec.action, has_connection))
            .collect();
        let reasons: Vec<Option<String>> = actions
            .menu
            .iter()
            .map(|spec| {
                spec.reason_key
                    .map(|key| i18n.t_with(key, &[("provider", provider.name)]))
            })
            .collect();
        let items: Vec<MenuItem<'_>> = actions
            .menu
            .iter()
            .zip(&labels)
            .zip(&reasons)
            .map(|((spec, label), reason)| {
                let item = MenuItem::new(label).icon(Self::action_icon(spec.action));
                match (spec.enabled, reason) {
                    (false, Some(reason)) => item.disabled(reason),
                    _ => item,
                }
            })
            .collect();
        let chosen = cs_overflow_menu(
            ui,
            egui::Id::new(("provider-menu", provider.id.key())),
            &i18n.t("connections-more"),
            &items,
        );
        if let Some(index) = chosen {
            let spec = actions.menu[index];
            if spec.enabled {
                self.perform(provider, spec.action);
            }
        }
    }

    /// Faz o que a Ação pede: abre o formulário, junta o pedido ao quadro.
    fn perform(&mut self, provider: &Provider, action: CardAction) {
        let id = provider.id;
        match action {
            CardAction::Link => {
                if self.forms.remove(&id).is_none() {
                    self.forms.insert(id, LinkForm::default());
                }
                self.expanded.insert(id);
            }
            CardAction::Test => {
                self.expanded.insert(id);
                self.ops.push(Op::Test {
                    provider: id,
                    feature: self.i18n.t("connections-test-feature"),
                });
            }
            CardAction::ClearCache => {
                self.expanded.insert(id);
                self.ops.push(Op::ClearCache(id));
            }
            CardAction::OpenDocs => self.ops.push(Op::OpenDocs(id)),
            CardAction::OpenTerms => self.ops.push(Op::OpenTerms(id)),
            // Chegam com o Cofre (#224): hoje o menu os mostra desabilitados.
            CardAction::Authorize | CardAction::EnterKey => {}
        }
    }

    /// A linha do resultado do último "Testar": confirmado, não encontrado, limitado ou falha,
    /// com "há N min" (o app não lê o fuso do sistema).
    fn test_line(&self, ui: &mut Ui, id: ProviderId, p: &Palette) {
        use super::probe::{Outcome, TestState};
        let i18n = self.i18n;
        let Some(state) = self.tests.get(&id) else {
            ui.label(small(i18n.t("connections-not-tested"), p.ink_muted));
            return;
        };
        let (glyph, color, text) = match state {
            TestState::AwaitingConsent { .. } => (
                icon::HOURGLASS_MEDIUM,
                p.ink_muted,
                i18n.t("connections-test-waiting"),
            ),
            TestState::Running { .. } => (
                icon::HOURGLASS_MEDIUM,
                p.ink_muted,
                i18n.t("connections-test-running"),
            ),
            TestState::Done(report) => {
                let when = self.age_text(model::age(self.now, report.at));
                let source = report.source.info().name;
                let fide = id == ProviderId::Fide;
                let base = [("when", when.as_str()), ("source", source)];
                match &report.outcome {
                    Outcome::Confirmed(name) => {
                        let key = if fide {
                            "connections-test-confirmed-id"
                        } else {
                            "connections-test-confirmed"
                        };
                        let args = [base[0], base[1], ("name", name.as_str())];
                        (icon::CHECK_CIRCLE, p.success, i18n.t_with(key, &args))
                    }
                    Outcome::NotFound => {
                        let key = if fide {
                            "connections-test-not-found-id"
                        } else {
                            "connections-test-not-found"
                        };
                        (icon::WARNING, p.warning, i18n.t_with(key, &base))
                    }
                    Outcome::Limited(until) => {
                        let until = self.minutes_text(model::minutes_left(self.now, *until));
                        let args = [base[0], base[1], ("until", until.as_str())];
                        (
                            icon::TIMER,
                            p.warning,
                            i18n.t_with("connections-test-limited", &args),
                        )
                    }
                    Outcome::InvalidId => (
                        icon::WARNING,
                        p.warning,
                        i18n.t_with("connections-test-invalid", &base[..1]),
                    ),
                    Outcome::Failed(detail_key) => {
                        let detail = i18n.t(detail_key);
                        let args = [base[0], ("detail", detail.as_str())];
                        (
                            icon::WARNING_CIRCLE,
                            p.danger,
                            i18n.t_with("connections-test-failed", &args),
                        )
                    }
                }
            }
        };
        ui.label(status_line(self.ctx, glyph, color, &text, p.ink_muted));
    }

    /// As Conexões vinculadas e o formulário de vincular.
    fn connections(&mut self, ui: &mut Ui, provider: &Provider, state: CardState, p: &Palette) {
        let i18n = self.i18n;
        let id = provider.id;
        let mine: Vec<Connection> = self.connections_of(id).into_iter().cloned().collect();
        if !mine.is_empty() {
            ui.add_space(4.0);
            ui.label(
                RichText::new(i18n.t("connections-list-title"))
                    .font(strong_font(self.ctx, 14.0))
                    .color(p.ink),
            );
            for connection in &mine {
                self.connection_row(ui, provider, connection, p);
            }
        }
        if self.forms.contains_key(&id) {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            self.link_form(ui, provider, state, p);
        }
    }

    fn connection_row(
        &mut self,
        ui: &mut Ui,
        provider: &Provider,
        connection: &Connection,
        p: &Palette,
    ) {
        ui.add_space(4.0);
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            self.connection_inner(ui, provider, connection, p);
        });
    }

    fn connection_inner(
        &mut self,
        ui: &mut Ui,
        provider: &Provider,
        connection: &Connection,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let editing = self
            .renaming
            .as_ref()
            .is_some_and(|form| form.id == connection.id);
        if editing {
            self.rename_row(ui, connection, p);
            return;
        }
        if *self.confirm_remove == Some(connection.id) {
            self.confirm_row(ui, connection, p);
            return;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(
                RichText::new(connection.display_name())
                    .font(strong_font(self.ctx, 14.0))
                    .color(p.ink),
            );
            if connection.is_default {
                cs_status_badge(
                    ui,
                    StatusTone::Accent,
                    "",
                    &i18n.t("connections-default-badge"),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let rename = i18n.t("connections-rename");
                let remove = i18n.t("connections-remove");
                let items = [
                    MenuItem::new(&rename).icon(icon::PENCIL_SIMPLE),
                    MenuItem::new(&remove).icon(icon::TRASH).danger(),
                ];
                match cs_overflow_menu(
                    ui,
                    egui::Id::new(("connection-menu", connection.id.0)),
                    &i18n.t("connections-conn-more"),
                    &items,
                ) {
                    Some(0) => {
                        *self.renaming = Some(RenameForm {
                            id: connection.id,
                            text: connection.nickname.clone(),
                            error: None,
                        });
                        *self.confirm_remove = None;
                    }
                    Some(1) => {
                        *self.confirm_remove = Some(connection.id);
                        *self.renaming = None;
                    }
                    _ => {}
                }
                if !connection.is_default
                    && ui
                        .add(
                            CsButton::new(&i18n.t("connections-make-default"))
                                .variant(ButtonVariant::Subtle),
                        )
                        .clicked()
                {
                    self.ops.push(Op::SetDefault(connection.id));
                }
            });
        });
        let who = if connection.kind == chess_store::ConnectionKind::PublicId {
            i18n.t_with("connections-id-in", &[("name", &connection.identifier)])
        } else {
            i18n.t_with(
                "connections-name-in",
                &[
                    ("provider", provider.name),
                    ("name", &connection.identifier),
                ],
            )
        };
        ui.label(small(who, p.ink_muted));
        if connection.is_default {
            self.test_line(ui, provider.id, p);
        }
    }

    fn rename_row(&mut self, ui: &mut Ui, connection: &Connection, p: &Palette) {
        let i18n = self.i18n;
        let Some(form) = self.renaming.as_mut() else {
            return;
        };
        ui.label(small(i18n.t("connections-rename-label"), p.ink_muted));
        let error = form.error.map(|key| i18n.t(key));
        cs_text_field(
            ui,
            &mut form.text,
            &i18n.t("connections-rename-hint"),
            error.as_deref(),
        );
        let (mut save, mut cancel) = (false, false);
        ui.horizontal(|ui| {
            save = ui
                .add(
                    CsButton::new(&i18n.t("connections-rename-save"))
                        .variant(ButtonVariant::Secondary),
                )
                .clicked();
            cancel = ui
                .add(
                    CsButton::new(&i18n.t("connections-rename-cancel"))
                        .variant(ButtonVariant::Subtle),
                )
                .clicked();
        });
        if save {
            self.ops.push(Op::Rename(connection.id, form.text.clone()));
        } else if cancel {
            *self.renaming = None;
        }
    }

    /// Remover pede confirmação na própria linha: nada some sem perguntar.
    fn confirm_row(&mut self, ui: &mut Ui, connection: &Connection, p: &Palette) {
        let i18n = self.i18n;
        ui.label(
            RichText::new(i18n.t_with(
                "connections-remove-confirm",
                &[("name", connection.display_name())],
            ))
            .color(p.ink),
        );
        ui.horizontal(|ui| {
            if ui
                .add(
                    CsButton::new(&i18n.t("connections-remove-yes")).variant(ButtonVariant::Danger),
                )
                .clicked()
            {
                self.ops.push(Op::Remove(connection.id));
            }
            if ui
                .add(
                    CsButton::new(&i18n.t("connections-remove-no"))
                        .variant(ButtonVariant::Secondary),
                )
                .clicked()
            {
                *self.confirm_remove = None;
            }
        });
    }

    fn link_form(&mut self, ui: &mut Ui, provider: &Provider, state: CardState, p: &Palette) {
        let i18n = self.i18n;
        let id = provider.id;
        let fields = model::card_fields(provider, state);
        let kind = model::connection_kind(provider);
        let hint_key = match id {
            ProviderId::Lichess => "connections-link-hint-lichess",
            ProviderId::ChessCom => "connections-link-hint-chesscom",
            ProviderId::Fide => "connections-link-hint-fide",
            _ => "connections-link-hint-default",
        };
        let mut submit = false;
        let mut cancel = false;
        let Some(form) = self.forms.get_mut(&id) else {
            return;
        };
        for field in fields {
            match field {
                FieldKind::PublicName | FieldKind::PublicId => {
                    let label = if field == FieldKind::PublicId {
                        "connections-link-id-label"
                    } else {
                        "connections-link-name-label"
                    };
                    ui.label(small(i18n.t(label), p.ink_muted));
                    let error = form.error.map(|key| i18n.t(key));
                    cs_text_field(
                        ui,
                        &mut form.identifier,
                        &i18n.t(hint_key),
                        error.as_deref(),
                    );
                }
                FieldKind::Nickname => {
                    ui.label(small(i18n.t("connections-link-nick-label"), p.ink_muted));
                    cs_text_field(
                        ui,
                        &mut form.nickname,
                        &i18n.t("connections-link-nick-hint"),
                        None,
                    );
                }
                // Sem campo de segredo até o Cofre (#224).
                FieldKind::Secret => {}
            }
        }
        ui.label(small(i18n.t("connections-link-note"), p.ink_muted));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    CsButton::new(&i18n.t("connections-link-save")).variant(ButtonVariant::Primary),
                )
                .clicked()
            {
                submit = true;
            }
            if ui
                .add(
                    CsButton::new(&i18n.t("connections-link-cancel"))
                        .variant(ButtonVariant::Secondary),
                )
                .clicked()
            {
                cancel = true;
            }
        });
        if cancel {
            self.forms.remove(&id);
        } else if submit {
            if form.identifier.trim().is_empty() {
                form.error = Some("connections-link-error-empty");
            } else {
                form.error = None;
                self.ops.push(Op::Link {
                    provider: id,
                    kind,
                    identifier: form.identifier.clone(),
                    nickname: form.nickname.clone(),
                });
            }
        }
    }
}

/// Texto pequeno (`caption`) na cor dada.
fn small(text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text)
        .text_style(egui::TextStyle::Small)
        .color(color)
}

/// Uma linha com um ícone colorido (o sinal) e o texto em `text` (o sinal nunca é só a cor).
fn status_line(
    ctx: &egui::Context,
    glyph: &str,
    glyph_color: Color32,
    text: &str,
    text_color: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        glyph,
        0.0,
        egui::TextFormat {
            font_id: crate::ui_theme::icon_font(ctx, 14.0),
            color: glyph_color,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job.append(
        text,
        6.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(12.0),
            color: text_color,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job.wrap.max_width = f32::INFINITY;
    job
}

/// Reserva a altura de `lines` linhas do estilo, para que cartões de uma faixa tenham a mesma
/// altura mesmo quando a frase tem uma linha só.
fn reserve_lines(ui: &mut Ui, lines: f32, style: egui::TextStyle, add: impl FnOnce(&mut Ui)) {
    let height = lines * ui.text_style_height(&style);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_height(height);
            add(ui);
        },
    );
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Tom e ícone do selo de cada estado: palavra e ícone, nunca só a cor.
pub(super) fn state_look(state: CardState) -> (StatusTone, &'static str) {
    match state {
        CardState::Off => (StatusTone::Neutral, icon::POWER),
        CardState::On => (StatusTone::Success, icon::CHECK_CIRCLE),
        CardState::Linked => (StatusTone::Success, icon::LINK_SIMPLE),
        CardState::Authorized => (StatusTone::Success, icon::SEAL_CHECK),
        CardState::Limited => (StatusTone::Warning, icon::TIMER),
        CardState::Failing => (StatusTone::Danger, icon::WARNING_CIRCLE),
        CardState::ManualImport => (StatusTone::Info, icon::FILE_ARROW_DOWN),
        CardState::NeedsApproval => (StatusTone::Neutral, icon::LOCK_KEY),
        CardState::Planned | CardState::Unconfirmed => {
            (StatusTone::Neutral, icon::HOURGLASS_MEDIUM)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_theme::{Density, ThemeId, test_context};
    use bevy_egui::egui::Shape;

    /// Todo texto desenhado no quadro, com o canto direito de cada um.
    fn texts(shape: &Shape, out: &mut Vec<(String, f32)>) {
        match shape {
            Shape::Text(text) => {
                let right = text.pos.x + text.galley.size().x;
                out.push((text.galley.text().to_owned(), right));
            }
            Shape::Vec(list) => list.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }

    /// Desenha a Central inteira (todos os cartões abertos) e devolve os textos visíveis.
    fn render(
        theme: &str,
        density: Density,
        width: f32,
        open_all: bool,
        customize: impl FnOnce(&mut ConnectionsUi, &NetService),
    ) -> Vec<(String, f32)> {
        let theme = ThemeId::from_key(theme).expect("tema");
        let ctx = test_context(theme, density);
        let i18n = I18n::pt_br();
        let service = NetService::in_memory();
        service.set_enabled(ProviderId::Lichess, true).unwrap();
        service.set_enabled(ProviderId::ChessCom, true).unwrap();
        let mut state = ConnectionsUi::demo();
        if open_all {
            state.expanded.extend(ProviderId::ALL);
        }
        customize(&mut state, &service);
        let mut dialog = ConsentDialog::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(width, 6000.0),
            )),
            ..Default::default()
        };
        let mut found = Vec::new();
        // Dois quadros: o egui só acerta alturas e famílias de fonte depois do primeiro.
        for _ in 0..2 {
            found.clear();
            let mut out = ctx.run_ui(raw.clone(), |ui| {
                let ctx = ui.ctx().clone();
                page(
                    ui,
                    &ctx,
                    &i18n,
                    &mut state,
                    &service,
                    &mut dialog,
                    None,
                    Some("Perfil de teste"),
                );
            });
            out.textures_delta.clear();
            for clipped in &out.shapes {
                texts(&clipped.shape, &mut found);
            }
        }
        found
    }

    const BANNED: [&str; 8] = [
        "etapa",
        "401",
        "token",
        "conta",
        "login",
        "credencial",
        "pkce",
        "planejad",
    ];

    #[test]
    fn no_visible_text_uses_jargon_in_any_card_state_theme_or_density() {
        for theme in ["dark", "light", "pretas", "brancas"] {
            for density in Density::ALL {
                for open_all in [false, true] {
                    let found = render(theme, density, 1600.0, open_all, |_, _| {});
                    assert!(found.len() > 50, "a página desenhou ({})", found.len());
                    for (text, _) in &found {
                        let lower = text.to_lowercase();
                        for banned in BANNED {
                            assert!(
                                !lower.contains(banned),
                                "{banned:?} em {text:?} ({theme}, {density:?}, aberto={open_all})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_linked_name_is_never_shown_as_connected_or_authorized() {
        let found = render("dark", Density::Comfortable, 1600.0, true, |_, _| {});
        let all: Vec<&str> = found.iter().map(|(t, _)| t.as_str()).collect();
        assert!(all.iter().any(|t| t.contains("Ligado · nome vinculado")));
        for text in &all {
            assert!(!text.contains("Conectado"), "{text}");
            assert!(!text.contains("Autorizado"), "{text}");
        }
        // Com uma Conexão autorizada de verdade, e só então, aparece "Autorizado".
        let found = render("dark", Density::Comfortable, 1600.0, false, |state, _| {
            state.authorized.insert(ProviderId::Lichess);
        });
        assert!(found.iter().any(|(t, _)| t.contains("Autorizado")));
    }

    #[test]
    fn nothing_runs_past_the_right_edge_in_one_or_two_columns() {
        for (width, theme, density) in [
            (1600.0, "dark", Density::Comfortable),
            (1100.0, "light", Density::Compact),
            (900.0, "pretas", Density::Comfortable),
            (700.0, "brancas", Density::Compact),
            (520.0, "dark", Density::Comfortable),
        ] {
            let found = render(theme, density, width, true, |_, _| {});
            for (text, right) in &found {
                assert!(
                    *right <= width + 1.0,
                    "{text:?} termina em {right} numa tela de {width} ({theme})"
                );
            }
        }
    }

    #[test]
    fn states_use_a_word_and_an_icon_not_only_a_colour() {
        for state in [
            CardState::Off,
            CardState::On,
            CardState::Linked,
            CardState::Authorized,
            CardState::Limited,
            CardState::Failing,
            CardState::ManualImport,
            CardState::NeedsApproval,
            CardState::Planned,
            CardState::Unconfirmed,
        ] {
            let (_, glyph) = state_look(state);
            assert!(!glyph.is_empty(), "{state:?} sem ícone");
            let word = I18n::pt_br().t(state.key());
            assert_ne!(word, state.key(), "{state:?} sem palavra");
        }
    }

    #[test]
    fn the_page_scrolls_as_a_whole_and_keeps_every_card() {
        let found = render("dark", Density::Comfortable, 1600.0, false, |_, _| {});
        let all: Vec<&str> = found.iter().map(|(t, _)| t.as_str()).collect();
        for title in ["Jogar online", "Dados e consultas", "Engines"] {
            assert!(all.contains(&title), "{title}");
        }
        // Os filtros e a busca fazem parte da área que rola.
        assert!(all.iter().any(|t| t.starts_with("Todos")));
    }

    #[test]
    fn minutes_and_ages_read_in_plain_portuguese() {
        let found = render("dark", Density::Comfortable, 1600.0, true, |_, _| {});
        let all: Vec<&str> = found.iter().map(|(t, _)| t.as_str()).collect();
        assert!(
            all.iter().any(|t| t.contains("há 3 min")),
            "o teste do demo foi feito há 3 min, sem fuso"
        );
        assert!(
            all.iter().all(|t| !t.contains("UTC")),
            "nada de UTC na tela"
        );
    }

    #[test]
    fn capitalize_only_touches_the_first_letter() {
        assert_eq!(capitalize("vincular seu nome"), "Vincular seu nome");
        assert_eq!(capitalize("éfora"), "Éfora");
        assert_eq!(capitalize(""), "");
    }
}
