//! A tela da Central de Conexões (egui): título, filtros, busca e os cartões por categoria.
//! Só desenha e junta pedidos ([`Op`]); quem decide é o modelo ([`super::model`]) e quem executa
//! é [`ConnectionsUi::apply`](super::ConnectionsUi::apply).

use super::model::{
    self, ActionSpec, Age, CardAction, CardState, CostFilter, FieldKind, SwitchMode,
};
use super::{CentralParams, ConnectionsUi, LinkForm, Op};
use crate::i18n::I18n;
use crate::ui_theme::{
    ButtonVariant, CsButton, Palette, cs_button, cs_choice_card, cs_switch, cs_text_field, palette,
    strong_font,
};
use bevy_egui::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Ui};
use chess_net::{Provider, ProviderId, ProviderState, Timestamp};
use chess_store::Connection;
use std::collections::HashMap;

/// Largura mínima, em pontos, a partir da qual os cartões vão em duas colunas.
const TWO_COLUMNS_FROM: f32 = 980.0;
const CARD_GAP: f32 = 12.0;

/// Desenha a página e executa os pedidos do quadro.
pub fn section(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    params: &mut CentralParams,
    profile_name: Option<&str>,
) {
    let handle = params.persistence.as_deref();
    let state: &mut ConnectionsUi = &mut params.ui;
    state.refresh(handle);
    let service = &*params.service;
    let status = service.net().status();
    let last = model::last_queries(&service.net().recent_operations());
    let now = crate::net_service::NetService::now();
    let mut ops = Vec::new();
    let p = palette(ui);

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
    ui.add_space(12.0);
    filter_row(ui, i18n, state);
    ui.add_space(8.0);
    ui.scope(|ui| {
        ui.spacing_mut().text_edit_width = 360.0;
        cs_text_field(
            ui,
            &mut state.query,
            &i18n.t("connections-search-hint"),
            None,
        );
    });
    ui.add_space(8.0);

    let groups = model::groups(state.filter, &state.query);
    if groups.is_empty() {
        ui.label(RichText::new(i18n.t("connections-empty")).color(p.ink_muted));
    }
    let mut view = View {
        ctx,
        i18n,
        status: &status,
        last: &last,
        now,
        list: &state.list,
        forms: &mut state.forms,
        notes: &state.notes,
        service,
        ops: &mut ops,
    };
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for group in &groups {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(i18n.t(model::category_key(group.category)))
                        .font(strong_font(ctx, 16.0))
                        .color(p.ink),
                );
                ui.add_space(4.0);
                view.cards(ui, &group.providers);
            }
            ui.add_space(16.0);
        });
    state.apply(ops, service, handle);
}

/// Os três filtros de custo, como cartões de escolha com a contagem.
fn filter_row(ui: &mut Ui, i18n: &I18n, state: &mut ConnectionsUi) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CARD_GAP;
        for filter in CostFilter::ALL {
            let n = model::count(filter);
            let detail = if n == 1 {
                i18n.t("connections-count-one")
            } else {
                i18n.t_with("connections-count-many", &[("count", &n.to_string())])
            };
            if cs_choice_card(
                ui,
                state.filter == filter,
                &i18n.t(filter.key()),
                Some(&detail),
            )
            .clicked()
            {
                state.filter = filter;
            }
        }
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
    service: &'a crate::net_service::NetService,
    ops: &'a mut Vec<Op>,
}

impl View<'_> {
    /// Os cartões de uma categoria, em uma ou duas colunas conforme a largura.
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
        let width = (ui.available_width() - CARD_GAP) / 2.0 - 1.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            for column in 0..2 {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 0.0),
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

    fn card(&mut self, ui: &mut Ui, provider: &Provider) {
        let p = palette(ui);
        let id = provider.id;
        let linked = self.connections_of(id).len();
        let net = self.status.state(id).clone();
        let state = model::card_state(provider, &net, linked > 0);
        egui::Frame::new()
            .fill(p.surface_2)
            .stroke(Stroke::new(1.0, p.border_strong))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                self.header(ui, provider, state, p);
                self.facts(ui, provider, state, &net, p);
                ui.add_space(4.0);
                self.switch_and_sends(ui, provider, state, p);
                ui.add_space(4.0);
                self.actions(ui, provider, state);
                self.note(ui, id, p);
                self.connections(ui, provider, state, p);
            });
    }

    fn header(&self, ui: &mut Ui, provider: &Provider, state: CardState, p: &Palette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(provider.name)
                    .font(strong_font(self.ctx, 16.0))
                    .color(p.ink),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.chip(ui, state, p);
            });
        });
    }

    /// Ponto colorido e palavra do estado (nunca só a cor).
    fn chip(&self, ui: &mut Ui, state: CardState, p: &Palette) {
        let color = state_color(state, p);
        ui.label(
            RichText::new(self.i18n.t(state.key()))
                .font(strong_font(self.ctx, 13.0))
                .color(p.ink),
        );
        let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 5.0, color);
    }

    fn facts(
        &self,
        ui: &mut Ui,
        provider: &Provider,
        state: CardState,
        net: &ProviderState,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let list = |keys: Vec<&str>| {
            keys.iter()
                .map(|key| i18n.t(key))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let now = model::now_keys(provider, state);
        let line = if now.is_empty() {
            i18n.t("connections-now-nothing")
        } else {
            i18n.t_with("connections-now", &[("list", &list(now))])
        };
        ui.label(RichText::new(line).color(p.ink));
        if state == CardState::ManualImport {
            ui.label(RichText::new(i18n.t("connections-manual-phrase")).color(p.ink));
        }
        for (stage, keys) in model::planned(provider) {
            let text = i18n.t_with(
                "connections-planned",
                &[("stage", &stage.to_string()), ("list", &list(keys))],
            );
            ui.label(small(text, p.ink_muted));
        }
        let cost = i18n.t_with(
            "connections-cost",
            &[("cost", &i18n.t(model::cost_key(provider.cost())))],
        );
        ui.label(small(cost, p.ink_muted));
        let auth = i18n.t_with(
            "connections-identifies",
            &[("list", &list(model::auth_keys(provider)))],
        );
        ui.label(small(auth, p.ink_muted));
        if provider.uses_network() {
            let last = match self.last.get(&provider.id) {
                None => i18n.t("connections-last-never"),
                Some(at) => i18n.t_with(
                    "connections-last-ago",
                    &[("ago", &self.age_text(model::age(self.now, *at)))],
                ),
            };
            ui.label(small(last, p.ink_muted));
        }
        match net {
            ProviderState::Failing { error, .. } => {
                let detail = i18n.t(model::error_key(error));
                ui.label(small(
                    i18n.t_with("connections-failing", &[("detail", &detail)]),
                    p.danger,
                ));
            }
            ProviderState::Limited { .. } => {
                ui.label(small(i18n.t("connections-limited-until"), p.warning));
            }
            _ => {}
        }
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

    /// O Switch "Ligado" e, ao lado do estado, o que será (ou é) enviado ao Provedor.
    fn switch_and_sends(
        &mut self,
        ui: &mut Ui,
        provider: &Provider,
        state: CardState,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let sends = |key: &str| i18n.t_with(key, &[("text", provider.sends)]);
        match model::switch_mode(provider, state) {
            SwitchMode::Hidden => {
                ui.label(small(sends("connections-sends-never"), p.ink_muted));
            }
            SwitchMode::Locked { stage } => {
                ui.add_enabled_ui(false, |ui| {
                    let mut off = false;
                    cs_switch(ui, &mut off, &i18n.t("connections-switch"));
                });
                ui.label(small(
                    i18n.t_with("connections-switch-soon", &[("stage", &stage.to_string())]),
                    p.ink_muted,
                ));
                ui.label(small(sends("connections-sends-off"), p.ink_muted));
            }
            SwitchMode::Usable => {
                let mut on = self.service.is_enabled(provider.id);
                if cs_switch(ui, &mut on, &i18n.t("connections-switch")).changed() {
                    self.ops.push(Op::SetEnabled(provider.id, on));
                }
                let key = if on {
                    "connections-sends-on"
                } else {
                    "connections-sends-off"
                };
                ui.label(small(sends(key), p.ink_muted));
            }
        }
    }

    fn actions(&mut self, ui: &mut Ui, provider: &Provider, state: CardState) {
        let specs = model::card_actions(provider, state);
        if specs.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
            for spec in specs {
                self.action_button(ui, provider, spec);
            }
        });
    }

    fn action_button(&mut self, ui: &mut Ui, provider: &Provider, spec: ActionSpec) {
        let i18n = self.i18n;
        let stage = spec.soon_stage.unwrap_or(2).to_string();
        let label = match spec.action {
            CardAction::Link => i18n.t("connections-act-link"),
            CardAction::Test => i18n.t_with("connections-act-test", &[("stage", &stage)]),
            CardAction::ClearCache => i18n.t("connections-act-clear-cache"),
            CardAction::Authorize => i18n.t("connections-act-authorize"),
            CardAction::EnterKey => i18n.t("connections-act-key"),
            CardAction::OpenDocs => i18n.t("connections-act-docs"),
            CardAction::OpenTerms => i18n.t("connections-act-terms"),
        };
        let variant = match spec.action {
            CardAction::Link => ButtonVariant::Primary,
            _ => ButtonVariant::Secondary,
        };
        let icon = match spec.action {
            CardAction::OpenDocs | CardAction::OpenTerms => {
                Some(crate::action_icons::glyph("arrow-square-out"))
            }
            _ => None,
        };
        let mut button = CsButton::new(&label).variant(variant).enabled(spec.enabled);
        if let Some(icon) = icon {
            button = button.icon(icon);
        }
        let mut response = ui.add(button);
        if let Some(stage) = spec.soon_stage {
            response = response.on_hover_text(
                i18n.t_with("connections-soon-stage", &[("stage", &stage.to_string())]),
            );
        }
        if !(spec.enabled && response.clicked()) {
            return;
        }
        let id = provider.id;
        match spec.action {
            CardAction::Link => {
                if self.forms.remove(&id).is_none() {
                    self.forms.insert(id, LinkForm::default());
                }
            }
            CardAction::ClearCache => self.ops.push(Op::ClearCache(id)),
            CardAction::OpenDocs => self.ops.push(Op::OpenDocs(id)),
            CardAction::OpenTerms => self.ops.push(Op::OpenTerms(id)),
            CardAction::Test | CardAction::Authorize | CardAction::EnterKey => {}
        }
    }

    fn note(&self, ui: &mut Ui, id: ProviderId, p: &Palette) {
        let Some(note) = self.notes.get(&id) else {
            return;
        };
        let text = match &note.arg {
            Some((name, value)) => self.i18n.t_with(note.key, &[(name, value)]),
            None => self.i18n.t(note.key),
        };
        ui.label(small(text, p.ink));
    }

    /// As Conexões vinculadas e o formulário de vincular.
    fn connections(&mut self, ui: &mut Ui, provider: &Provider, state: CardState, p: &Palette) {
        let i18n = self.i18n;
        let id = provider.id;
        let mine: Vec<Connection> = self.connections_of(id).into_iter().cloned().collect();
        if !mine.is_empty() {
            ui.add_space(4.0);
            ui.separator();
            ui.label(small(i18n.t("connections-list-title"), p.ink_muted));
            for connection in &mine {
                self.connection_row(ui, connection, p);
            }
        }
        if self.forms.contains_key(&id) {
            ui.add_space(4.0);
            ui.separator();
            self.link_form(ui, provider, state, p);
        }
    }

    fn connection_row(&mut self, ui: &mut Ui, connection: &Connection, p: &Palette) {
        let i18n = self.i18n;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(
                RichText::new(connection.display_name())
                    .font(strong_font(self.ctx, 14.0))
                    .color(p.ink),
            );
            if !connection.nickname.is_empty() {
                ui.label(small(connection.identifier.clone(), p.ink_muted));
            }
            if connection.is_default {
                let badge = egui::Frame::new()
                    .fill(p.accent_subtle)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(Margin::symmetric(6, 1))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(i18n.t("connections-default-badge"))
                                .text_style(egui::TextStyle::Small)
                                .color(p.accent_ink),
                        );
                    });
                let _ = badge;
            } else if cs_button(
                ui,
                &i18n.t("connections-make-default"),
                ButtonVariant::Subtle,
            )
            .clicked()
            {
                self.ops.push(Op::SetDefault(connection.id));
            }
            if cs_button(ui, &i18n.t("connections-remove"), ButtonVariant::Subtle).clicked() {
                self.ops.push(Op::Remove(connection.id));
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
                // Sem campo de segredo na etapa 1 (o Cofre chega na etapa 3).
                FieldKind::Secret => {}
            }
        }
        ui.label(small(i18n.t("connections-link-note"), p.ink_muted));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if cs_button(ui, &i18n.t("connections-link-save"), ButtonVariant::Primary).clicked() {
                submit = true;
            }
            if cs_button(
                ui,
                &i18n.t("connections-link-cancel"),
                ButtonVariant::Secondary,
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

/// Cor do ponto do estado: sinal, sempre acompanhado da palavra.
fn state_color(state: CardState, p: &Palette) -> Color32 {
    match state {
        CardState::Available | CardState::Connected => p.success,
        CardState::Limited => p.warning,
        CardState::Failing => p.danger,
        CardState::ManualImport => p.info,
        CardState::Off | CardState::NeedsApproval | CardState::Planned | CardState::Unconfirmed => {
            p.ink_muted
        }
    }
}
