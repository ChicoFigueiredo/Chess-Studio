//! A parte da Central que trata de autorização (CX-11, #231): a faixa do cofre no topo, o diálogo
//! da senha-mestra, o bloco de autorização do cartão (espera, chave pessoal, como terminou), as
//! Conexões autorizadas e a entrada separada da Engine online. Só desenha e junta pedidos
//! ([`Op`]); quem decide é [`super::auth`].

use super::super::auth::{
    Check, Ending, Slot, VaultDialog, VaultMode, has_extra_permissions, permission_key,
};
use super::super::{Note, Op};
use super::{View, small, status_line};
use crate::net_service::Preload;
use crate::secrets::Standing;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, MenuItem, Palette, StatusTone, cs_card_frame,
    cs_checkbox, cs_overflow_menu, cs_secret_field, cs_status_badge, forget_secret_field,
    icon_font, palette, strong_font,
};
use bevy_egui::egui::{self, Color32, RichText, Ui};
use chess_net::Provider;
use chess_store::{Connection, ConnectionKind, ConnectionRole};
use chess_vault::VaultStatus;
use egui_phosphor::regular as icon;

/// A faixa do cofre: o que há em `Banner` decide ícone, textos e botões.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Banner {
    /// O cofre real não abriu (índice adulterado, disco).
    Broken,
    /// O cofre do sistema está sendo lido em fundo.
    Loading,
    /// O cofre do sistema não respondeu.
    SystemFailed,
    System,
    OwnOpen,
    OwnLocked,
    /// Sem cofre do sistema e sem cofre próprio: nada é guardado.
    NoVault,
}

/// Qual faixa mostrar, a partir do que o `NetService` sabe do Cofre.
pub(super) fn banner_of(service: &crate::net_service::NetService) -> Banner {
    if service.vault_error().is_some() {
        return Banner::Broken;
    }
    match service.vault().status() {
        VaultStatus::System => match service.preload() {
            Preload::Loading => Banner::Loading,
            Preload::Failed(_) => Banner::SystemFailed,
            Preload::Ready => Banner::System,
        },
        VaultStatus::OwnUnlocked => Banner::OwnOpen,
        VaultStatus::OwnLocked => Banner::OwnLocked,
        VaultStatus::Unavailable => Banner::NoVault,
    }
}

impl View<'_> {
    /// A faixa discreta do cofre, no topo da Central: onde as autorizações ficam guardadas.
    pub(super) fn vault_banner(&mut self, ui: &mut Ui, p: &Palette) {
        let i18n = self.i18n;
        let banner = banner_of(self.service);
        let has_authorizations = self.list.iter().any(Connection::is_authorized);
        let (glyph, color, title, text) = match banner {
            Banner::Broken => (
                icon::WARNING_CIRCLE,
                p.danger,
                "vault-broken-title",
                "vault-broken-text",
            ),
            Banner::Loading => (
                icon::HOURGLASS_MEDIUM,
                p.ink_muted,
                "vault-loading-title",
                "vault-loading-text",
            ),
            Banner::SystemFailed => (
                icon::WARNING,
                p.warning,
                "vault-system-failed-title",
                "vault-system-failed-text",
            ),
            Banner::System => (
                icon::SHIELD_CHECK,
                p.success,
                "vault-system-title",
                "vault-system-text",
            ),
            Banner::OwnOpen => (
                icon::LOCK_SIMPLE_OPEN,
                p.success,
                "vault-own-open-title",
                "vault-own-open-text",
            ),
            Banner::OwnLocked => (
                icon::LOCK_SIMPLE,
                p.warning,
                "vault-own-locked-title",
                if has_authorizations {
                    "vault-own-locked-text"
                } else {
                    "vault-own-locked-text-empty"
                },
            ),
            Banner::NoVault => (
                icon::SHIELD_WARNING,
                p.warning,
                "vault-none-title",
                "vault-none-text",
            ),
        };
        let mut ops = Vec::new();
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Sides::new().shrink_left().show(
                ui,
                |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.label(
                            RichText::new(glyph)
                                .font(icon_font(self.ctx, 20.0))
                                .color(color),
                        );
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.label(
                                RichText::new(i18n.t(title))
                                    .font(strong_font(self.ctx, 14.0))
                                    .color(p.ink),
                            );
                            ui.label(small(i18n.t(text), p.ink_muted));
                        });
                    });
                },
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // Da direita para a esquerda: o último botão adicionado fica mais à esquerda.
                    match banner {
                        Banner::NoVault => {
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("vault-act-create"))
                                        .icon(icon::KEY)
                                        .variant(ButtonVariant::Secondary),
                                )
                                .clicked()
                            {
                                ops.push(Op::OpenVaultDialog(VaultMode::Create));
                            }
                        }
                        Banner::OwnLocked => {
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("vault-act-unlock"))
                                        .icon(icon::LOCK_SIMPLE_OPEN)
                                        .variant(ButtonVariant::Secondary),
                                )
                                .clicked()
                            {
                                ops.push(Op::OpenVaultDialog(VaultMode::Unlock));
                            }
                        }
                        Banner::OwnOpen => {
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("vault-act-lock"))
                                        .icon(icon::LOCK_SIMPLE)
                                        .variant(ButtonVariant::Secondary),
                                )
                                .clicked()
                            {
                                ops.push(Op::LockVault);
                            }
                            if ui
                                .add(
                                    CsButton::new(&i18n.t("vault-act-change"))
                                        .variant(ButtonVariant::Subtle),
                                )
                                .clicked()
                            {
                                ops.push(Op::OpenVaultDialog(VaultMode::Change));
                            }
                        }
                        _ => {}
                    }
                },
            );
        });
        self.ops.extend(ops);
    }

    /// O diálogo da senha-mestra (modal), se aberto.
    pub(super) fn vault_dialog(&mut self) {
        let Some(dialog) = self.auth.vault_dialog.as_mut() else {
            return;
        };
        let ctx = self.ctx;
        let i18n = self.i18n;
        // Escurece o resto: o diálogo é modal.
        egui::Area::new(egui::Id::new("vault-dialog-dim"))
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.viewport_rect();
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
            });
        let title = match dialog.mode {
            VaultMode::Create => "vault-dialog-create-title",
            VaultMode::Unlock => "vault-dialog-unlock-title",
            VaultMode::Change => "vault-dialog-change-title",
        };
        let mut ops = Vec::new();
        egui::Window::new(i18n.t(title))
            .id(egui::Id::new("vault-dialog"))
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                vault_dialog_body(ui, ctx, i18n, dialog, &mut ops);
            });
        if !dialog.is_busy() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ops.push(Op::CloseVaultDialog);
        }
        self.ops.extend(ops);
    }

    // --- Bloco de autorização do cartão --------------------------------------------------------

    /// O que está acontecendo com a autorização deste Provedor, logo abaixo do "O que você pode
    /// fazer agora": a espera, a chave pessoal, como terminou. Não ocupa lugar quando não há nada.
    pub(super) fn authorization_block(
        &mut self,
        ui: &mut Ui,
        provider: &'static chess_net::Provider,
        state: super::CardState,
        p: &Palette,
    ) {
        if !super::model::can_authorize(provider) {
            return;
        }
        let id = provider.id;
        let player = Slot::player(id);
        if self.auth.is_busy(id) {
            self.waiting_block(ui, p);
            ui.add_space(8.0);
            return;
        }
        if self.auth.key_forms.contains_key(&player) {
            self.key_form(ui, player, p);
            ui.add_space(8.0);
            return;
        }
        if let Some(ending) = self.auth.endings.get(&player).cloned() {
            self.ending_block(ui, player, &ending, p);
            ui.add_space(8.0);
            return;
        }
        let line = match state {
            super::CardState::Expired => {
                Some((icon::CLOCK_COUNTDOWN, p.warning, "auth-expired-text"))
            }
            super::CardState::Locked => Some((icon::LOCK_SIMPLE, p.warning, "auth-locked-text")),
            _ => None,
        };
        if let Some((glyph, color, key)) = line {
            let text = self.i18n.t(key);
            ui.label(status_line(self.ctx, glyph, color, &text, p.ink));
            ui.add_space(8.0);
        }
    }

    /// Esperando o navegador (ou a conferência da chave pessoal), com "Cancelar".
    fn waiting_block(&mut self, ui: &mut Ui, p: &Palette) {
        let i18n = self.i18n;
        let Some(attempt) = self.auth.attempt.as_ref() else {
            return;
        };
        let browser = attempt.is_browser();
        let engine = attempt.slot.is_engine();
        let (title, text) = if browser {
            (
                if engine {
                    "auth-waiting-title-engine"
                } else {
                    "auth-waiting-title"
                },
                "auth-waiting-text",
            )
        } else {
            ("auth-checking-title", "auth-checking-text")
        };
        let left = attempt.minutes_left(self.now);
        let failed = attempt.open_failed;
        let url = attempt.url().map(str::to_owned);
        let mut ops = Vec::new();
        let ctx = self.ctx;
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                ui.label(
                    RichText::new(icon::HOURGLASS_MEDIUM)
                        .font(icon_font(ctx, 20.0))
                        .color(p.info),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(
                        RichText::new(i18n.t(title))
                            .font(strong_font(ctx, 14.0))
                            .color(p.ink),
                    );
                    ui.label(RichText::new(i18n.t(text)).color(p.ink));
                    if browser {
                        let when = self.minutes_text(left);
                        ui.label(small(
                            i18n.t_with("auth-waiting-left", &[("when", &when)]),
                            p.ink_muted,
                        ));
                    }
                    if failed {
                        ui.label(status_line(
                            ctx,
                            icon::WARNING,
                            p.warning,
                            &i18n.t("auth-waiting-open-failed"),
                            p.ink,
                        ));
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if browser
                    && ui
                        .add(
                            CsButton::new(&i18n.t("auth-act-reopen"))
                                .icon(icon::ARROW_SQUARE_OUT)
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                {
                    ops.push(Op::ReopenBrowser);
                }
                if failed
                    && let Some(url) = &url
                    && ui
                        .add(
                            CsButton::new(&i18n.t("auth-act-copy"))
                                .icon(icon::COPY)
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                {
                    // Cópia pedida pela pessoa, e só do endereço da autorização: a senha-mestra
                    // e a chave pessoal nunca vão para a área de transferência.
                    ui.ctx().copy_text(url.clone());
                }
                if ui
                    .add(CsButton::new(&i18n.t("auth-act-cancel")).variant(ButtonVariant::Subtle))
                    .clicked()
                {
                    ops.push(Op::CancelAuthorization);
                }
            });
        });
        self.ops.extend(ops);
    }

    /// Como terminou a última autorização da entrada, com a próxima ação.
    pub(super) fn ending_block(&mut self, ui: &mut Ui, slot: Slot, ending: &Ending, p: &Palette) {
        let i18n = self.i18n;
        let ctx = self.ctx;
        let (glyph, color, title): (&str, Color32, String) = match ending {
            Ending::Authorized { name, .. } => (
                icon::SEAL_CHECK,
                p.success,
                i18n.t_with("auth-done-title", &[("name", name)]),
            ),
            Ending::Refused => (icon::PROHIBIT, p.warning, i18n.t("auth-refused-title")),
            Ending::TimedOut => (
                icon::CLOCK_COUNTDOWN,
                p.warning,
                i18n.t("auth-timeout-title"),
            ),
            Ending::Declined => (icon::POWER, p.warning, i18n.t("auth-declined-title")),
            Ending::Problem(_) => (icon::WARNING_CIRCLE, p.danger, i18n.t("auth-problem-title")),
        };
        let mut ops = Vec::new();
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                ui.label(RichText::new(glyph).font(icon_font(ctx, 20.0)).color(color));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(
                        RichText::new(title)
                            .font(strong_font(ctx, 14.0))
                            .color(p.ink),
                    );
                    match ending {
                        Ending::Authorized { scopes, kind, .. } => {
                            let how = if *kind == ConnectionKind::AuthorizedKey {
                                "auth-done-key"
                            } else {
                                "auth-done-browser"
                            };
                            ui.label(RichText::new(i18n.t(how)).color(p.ink));
                            self.permissions(ui, scopes, slot.role, p);
                        }
                        Ending::Refused => {
                            ui.label(RichText::new(i18n.t("auth-refused-text")).color(p.ink));
                        }
                        Ending::TimedOut => {
                            ui.label(RichText::new(i18n.t("auth-timeout-text")).color(p.ink));
                        }
                        Ending::Declined => {
                            ui.label(RichText::new(i18n.t("auth-declined-text")).color(p.ink));
                        }
                        Ending::Problem(note) => {
                            ui.label(RichText::new(self.note_text(note)).color(p.ink));
                        }
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let retry = !matches!(ending, Ending::Authorized { .. });
                if retry
                    && ui
                        .add(
                            CsButton::new(&i18n.t("auth-act-retry"))
                                .icon(icon::ARROW_COUNTER_CLOCKWISE)
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                {
                    ops.push(Op::Retry(slot));
                }
                if ui
                    .add(CsButton::new(&i18n.t("auth-act-dismiss")).variant(ButtonVariant::Subtle))
                    .clicked()
                {
                    ops.push(Op::DismissEnding(slot));
                }
            });
        });
        self.ops.extend(ops);
    }

    /// O texto de uma mensagem; `auth-problem-net` traz a frase curta da falha de rede.
    fn note_text(&self, note: &Note) -> String {
        match &note.arg {
            Some((name, value)) if note.key == "auth-problem-net" => {
                let detail = self.i18n.t(value);
                self.i18n.t_with(note.key, &[(name, &detail)])
            }
            Some((name, value)) => self.i18n.t_with(note.key, &[(name, value)]),
            None => self.i18n.t(note.key),
        }
    }

    /// "O que esta autorização permite": uma linha por permissão, em linguagem de usuário.
    pub(super) fn permissions(
        &self,
        ui: &mut Ui,
        scopes: &[String],
        role: ConnectionRole,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        ui.add_space(4.0);
        ui.label(small(i18n.t("auth-permits"), p.ink_muted));
        for scope in scopes {
            let text = match permission_key(scope) {
                Some(key) => i18n.t(key),
                None => i18n.t_with("perm-other", &[("name", scope)]),
            };
            ui.label(status_line(self.ctx, icon::CHECK, p.success, &text, p.ink));
        }
        if has_extra_permissions(scopes, role) {
            ui.label(status_line(
                self.ctx,
                icon::INFO,
                p.info,
                &i18n.t("auth-permits-extra"),
                p.ink_muted,
            ));
        }
    }

    /// O formulário da chave pessoal: campo mascarado, nunca em log nem na área de transferência.
    pub(super) fn key_form(&mut self, ui: &mut Ui, slot: Slot, p: &Palette) {
        // Só o Provedor que aceita chave pessoal ganha o campo de segredo.
        let fields = super::model::key_fields(slot.provider.info(), super::CardState::On);
        if !fields.contains(&super::FieldKind::Secret) {
            return;
        }
        let i18n = self.i18n;
        let ctx = self.ctx;
        let (title, intro) = if slot.is_engine() {
            ("auth-key-title-engine", "auth-key-intro-engine")
        } else {
            ("auth-key-title", "auth-key-intro")
        };
        let mut ops = Vec::new();
        let field_id = egui::Id::new(("auth-key-field", slot.provider.key(), slot.is_engine()));
        let Some(form) = self.auth.key_forms.get_mut(&slot) else {
            return;
        };
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(
                    RichText::new(icon::KEY)
                        .font(icon_font(ctx, 16.0))
                        .color(p.ink_muted),
                );
                ui.label(
                    RichText::new(i18n.t(title))
                        .font(strong_font(ctx, 14.0))
                        .color(p.ink),
                );
            });
            ui.label(RichText::new(i18n.t(intro)).color(p.ink));
            ui.add_space(4.0);
            ui.label(small(i18n.t("auth-key-label"), p.ink_muted));
            let error = form.error.map(|key| i18n.t(key));
            let response = cs_secret_field(
                ui,
                field_id,
                &mut form.secret,
                &i18n.t("auth-key-hint"),
                error.as_deref(),
            );
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.label(small(i18n.t("auth-key-note"), p.ink_muted));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let save = ui
                    .add(
                        CsButton::new(&i18n.t("auth-key-save"))
                            .icon(icon::SEAL_CHECK)
                            .variant(ButtonVariant::Primary),
                    )
                    .clicked();
                if save || enter {
                    ops.push(Op::SubmitKey(slot));
                    // O egui guarda cópias do texto no histórico de desfazer: esquece agora.
                    forget_secret_field(ctx, field_id);
                }
                if ui
                    .add(CsButton::new(&i18n.t("auth-key-cancel")).variant(ButtonVariant::Subtle))
                    .clicked()
                {
                    ops.push(Op::CloseKeyForm(slot));
                    forget_secret_field(ctx, field_id);
                }
            });
        });
        self.ops.extend(ops);
    }

    // --- Conexão autorizada --------------------------------------------------------------------

    /// A linha de uma Conexão autorizada (jogador ou Engine online).
    pub(super) fn authorized_inner(
        &mut self,
        ui: &mut Ui,
        provider: &Provider,
        connection: &Connection,
        p: &Palette,
    ) {
        let i18n = self.i18n;
        let ctx = self.ctx;
        let standing = self.auth.standings.get(&connection.id).copied();
        let slot = Slot {
            provider: provider.id,
            role: connection.role,
        };
        if self
            .renaming
            .as_ref()
            .is_some_and(|form| form.id == connection.id)
        {
            self.rename_row(ui, connection, p);
            return;
        }
        if self.auth.confirm_disconnect == Some(connection.id) {
            ui.label(
                RichText::new(i18n.t_with(
                    "auth-disconnect-confirm",
                    &[("name", connection.display_name())],
                ))
                .color(p.ink),
            );
            ui.horizontal(|ui| {
                if ui
                    .add(
                        CsButton::new(&i18n.t("auth-disconnect-yes"))
                            .variant(ButtonVariant::Danger),
                    )
                    .clicked()
                {
                    self.ops.push(Op::Disconnect(connection.id));
                }
                if ui
                    .add(
                        CsButton::new(&i18n.t("auth-disconnect-no"))
                            .variant(ButtonVariant::Secondary),
                    )
                    .clicked()
                {
                    self.auth.confirm_disconnect = None;
                }
            });
            return;
        }
        let disconnecting = self.auth.is_disconnecting(connection.id);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(
                RichText::new(connection.display_name())
                    .font(strong_font(ctx, 14.0))
                    .color(p.ink),
            );
            let (tone, glyph, word) = match standing {
                Some(Standing::Active) => (
                    StatusTone::Success,
                    icon::SEAL_CHECK,
                    "auth-badge-authorized",
                ),
                Some(Standing::Locked) => {
                    (StatusTone::Warning, icon::LOCK_SIMPLE, "auth-badge-locked")
                }
                _ => (
                    StatusTone::Warning,
                    icon::CLOCK_COUNTDOWN,
                    "auth-badge-expired",
                ),
            };
            cs_status_badge(ui, tone, glyph, &i18n.t(word));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if disconnecting {
                    ui.label(small(i18n.t("auth-disconnecting"), p.ink_muted));
                    return;
                }
                self.authorized_menu(ui, slot, connection, standing);
            });
        });
        let who = i18n.t_with("auth-row-user", &[("name", &connection.identifier)]);
        ui.label(small(who, p.ink_muted));
        if let Some(at) = connection.authorized_at_ms {
            let when = self.when_text(chess_net::Timestamp::from_millis(at));
            let key = if connection.kind == ConnectionKind::AuthorizedKey {
                "auth-row-key"
            } else {
                "auth-row-browser"
            };
            ui.label(small(i18n.t_with(key, &[("when", &when)]), p.ink_muted));
        }
        match standing {
            Some(Standing::Active) => {
                self.permissions(ui, &connection.scopes, connection.role, p);
                self.check_line(ui, connection, p);
            }
            Some(Standing::Locked) => {
                ui.label(status_line(
                    ctx,
                    icon::LOCK_SIMPLE,
                    p.warning,
                    &i18n.t("auth-locked-text"),
                    p.ink,
                ));
            }
            _ => {
                ui.label(status_line(
                    ctx,
                    icon::CLOCK_COUNTDOWN,
                    p.warning,
                    &i18n.t("auth-expired-text"),
                    p.ink,
                ));
            }
        }
    }

    fn authorized_menu(
        &mut self,
        ui: &mut Ui,
        slot: Slot,
        connection: &Connection,
        standing: Option<Standing>,
    ) {
        let i18n = self.i18n;
        let active = standing == Some(Standing::Active);
        let test = i18n.t("auth-act-test");
        let reauth = i18n.t("auth-act-reauth");
        let rename = i18n.t("connections-rename");
        let disconnect = i18n.t("auth-act-disconnect");
        let locked_reason = i18n.t("auth-locked-text");
        let first = MenuItem::new(&test).icon(icon::PLAY);
        let mut items = vec![if active {
            first
        } else {
            first.disabled(&locked_reason)
        }];
        items.push(MenuItem::new(&reauth).icon(icon::SEAL_CHECK));
        items.push(MenuItem::new(&rename).icon(icon::PENCIL_SIMPLE));
        items.push(MenuItem::new(&disconnect).icon(icon::PLUGS).danger());
        let chosen = cs_overflow_menu(
            ui,
            egui::Id::new(("connection-menu", connection.id.0)),
            &i18n.t("connections-conn-more"),
            &items,
        );
        match chosen {
            Some(0) => self.ops.push(Op::TestAuthorized(connection.id)),
            Some(1) => {
                if matches!(connection.kind, ConnectionKind::AuthorizedKey) {
                    self.ops.push(Op::OpenKeyForm {
                        slot,
                        renew: Some(connection.id),
                    });
                } else {
                    self.ops.push(Op::Reauthorize(connection.id));
                }
            }
            Some(2) => {
                *self.renaming = Some(super::RenameForm {
                    id: connection.id,
                    text: connection.nickname.clone(),
                    error: None,
                });
                *self.confirm_remove = None;
            }
            Some(3) => {
                self.auth.confirm_disconnect = Some(connection.id);
                *self.renaming = None;
            }
            _ => {}
        }
    }

    /// A última conferência com o Lichess ("Testar"), com "há N min".
    fn check_line(&self, ui: &mut Ui, connection: &Connection, p: &Palette) {
        let i18n = self.i18n;
        let Some(check) = self.auth.checks.get(&connection.id) else {
            return;
        };
        let (glyph, color, text) = match check {
            Check::Running(_) => (
                icon::HOURGLASS_MEDIUM,
                p.ink_muted,
                i18n.t("auth-check-running"),
            ),
            Check::Done {
                result: Ok(name),
                at,
            } => {
                let when = self.when_text(*at);
                (
                    icon::CHECK_CIRCLE,
                    p.success,
                    i18n.t_with("auth-check-ok", &[("when", &when), ("name", name)]),
                )
            }
            Check::Done {
                result: Err(key), ..
            } => {
                let text = if key.starts_with("auth-") {
                    i18n.t(key)
                } else {
                    i18n.t_with(
                        "connections-test-failed",
                        &[("when", ""), ("detail", &i18n.t(key))],
                    )
                };
                (icon::WARNING_CIRCLE, p.danger, text)
            }
        };
        ui.label(status_line(self.ctx, glyph, color, &text, p.ink_muted));
    }

    // --- Engine online ---------------------------------------------------------------------------

    /// A entrada **separada** da Engine online: outra autorização, outra permissão, nunca
    /// misturada com a do jogador.
    pub(super) fn engine_section(&mut self, ui: &mut Ui, provider: &Provider, p: &Palette) {
        let i18n = self.i18n;
        let ctx = self.ctx;
        let id = provider.id;
        let slot = Slot::engine(id);
        let engines: Vec<Connection> = self
            .list
            .iter()
            .filter(|c| c.provider == id.key() && c.role == ConnectionRole::Engine)
            .filter(|c| !c.is_pending())
            .cloned()
            .collect();
        let open = self.auth.engine_open.contains(&id)
            || !engines.is_empty()
            || self.auth.key_forms.contains_key(&slot)
            || self.auth.endings.contains_key(&slot);
        if !open {
            return;
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(
            RichText::new(i18n.t("engine-section-title"))
                .font(strong_font(ctx, 14.0))
                .color(p.ink),
        );
        ui.label(small(i18n.t("engine-section-text"), p.ink_muted));
        for connection in &engines {
            ui.add_space(4.0);
            cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                self.authorized_inner(ui, provider, connection, p);
            });
        }
        let busy = self.auth.is_busy(id);
        if self.auth.key_forms.contains_key(&slot) {
            ui.add_space(4.0);
            self.key_form(ui, slot, p);
        } else if let Some(ending) = self.auth.endings.get(&slot).cloned() {
            ui.add_space(4.0);
            self.ending_block(ui, slot, &ending, p);
        }
        let has_active = engines.iter().any(Connection::is_authorized);
        if !has_active && !busy && !self.auth.key_forms.contains_key(&slot) {
            if engines.is_empty() {
                ui.add_space(4.0);
                ui.label(small(i18n.t("engine-none"), p.ink_muted));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-act-authorize"))
                            .icon(icon::SEAL_CHECK)
                            .variant(ButtonVariant::Secondary),
                    )
                    .clicked()
                {
                    self.ops.push(Op::Authorize(slot));
                }
                if ui
                    .add(
                        CsButton::new(&i18n.t("engine-act-key"))
                            .icon(icon::KEY)
                            .variant(ButtonVariant::Subtle),
                    )
                    .clicked()
                {
                    self.ops.push(Op::OpenKeyForm { slot, renew: None });
                }
            });
        }
    }
}

/// O corpo do diálogo da senha-mestra.
fn vault_dialog_body(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &crate::i18n::I18n,
    dialog: &mut VaultDialog,
    ops: &mut Vec<Op>,
) {
    let p = palette(ui);
    ui.set_width(440.0);
    ui.spacing_mut().item_spacing.y = 8.0;
    let (intro, submit_label, working) = match dialog.mode {
        VaultMode::Create => (
            "vault-dialog-create-intro",
            "vault-submit-create",
            "vault-working-create",
        ),
        VaultMode::Unlock => (
            "vault-dialog-unlock-intro",
            "vault-submit-unlock",
            "vault-working-unlock",
        ),
        VaultMode::Change => (
            "vault-dialog-change-intro",
            "vault-submit-change",
            "vault-working-create",
        ),
    };
    ui.label(RichText::new(i18n.t(intro)).color(p.ink));
    if dialog.mode == VaultMode::Create {
        egui::Frame::new()
            .fill(p.surface_2)
            .stroke(egui::Stroke::new(1.0, p.warning))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(
                        RichText::new(icon::WARNING)
                            .font(icon_font(ctx, 16.0))
                            .color(p.warning),
                    );
                    ui.label(
                        RichText::new(i18n.t("vault-dialog-create-warning-title"))
                            .font(strong_font(ctx, 14.0))
                            .color(p.ink),
                    );
                });
                ui.label(RichText::new(i18n.t("vault-dialog-create-warning")).color(p.ink));
            });
    }
    let busy = dialog.is_busy();
    let unlocking = dialog.mode == VaultMode::Unlock;
    let error = dialog.error.map(|key| i18n.t(key));
    let mut enter = false;
    ui.add_enabled_ui(!busy, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let field = |ui: &mut Ui, label: &str, id: &str, text: &mut String, err: Option<&str>| {
            ui.add_space(4.0);
            ui.label(small(i18n.t(label), p.ink_muted));
            let response = cs_secret_field(
                ui,
                egui::Id::new(("vault-field", id)),
                text,
                &i18n.t(if id == "confirm" {
                    "vault-hint-confirm"
                } else if unlocking || id == "current" {
                    "vault-hint-unlock"
                } else {
                    "vault-hint-password"
                }),
                err,
            );
            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
        };
        match dialog.mode {
            VaultMode::Unlock => {
                enter |= field(
                    ui,
                    "vault-field-password",
                    "password",
                    &mut dialog.password,
                    error.as_deref(),
                );
            }
            VaultMode::Create => {
                field(
                    ui,
                    "vault-field-password",
                    "password",
                    &mut dialog.password,
                    None,
                );
                enter |= field(
                    ui,
                    "vault-field-confirm",
                    "confirm",
                    &mut dialog.confirm,
                    error.as_deref(),
                );
            }
            VaultMode::Change => {
                field(
                    ui,
                    "vault-field-current",
                    "current",
                    &mut dialog.current,
                    None,
                );
                field(
                    ui,
                    "vault-field-new",
                    "password",
                    &mut dialog.password,
                    None,
                );
                enter |= field(
                    ui,
                    "vault-field-confirm",
                    "confirm",
                    &mut dialog.confirm,
                    error.as_deref(),
                );
            }
        }
        if dialog.mode == VaultMode::Create {
            ui.add_space(4.0);
            cs_checkbox(ui, &mut dialog.understood, &i18n.t("vault-understood"));
        }
    });
    if busy {
        ui.label(status_line(
            ctx,
            icon::HOURGLASS_MEDIUM,
            p.info,
            &i18n.t(working),
            p.ink_muted,
        ));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let ready = !busy && (dialog.mode != VaultMode::Create || dialog.understood);
        let submit = ui
            .add(
                CsButton::new(&i18n.t(submit_label))
                    .variant(ButtonVariant::Primary)
                    .enabled(ready),
            )
            .clicked();
        if (submit || (enter && ready)) && ready {
            ops.push(Op::SubmitVaultDialog);
            for id in ["password", "confirm", "current"] {
                forget_secret_field(ctx, egui::Id::new(("vault-field", id)));
            }
        }
        if ui
            .add(
                CsButton::new(&i18n.t("vault-cancel"))
                    .variant(ButtonVariant::Secondary)
                    .enabled(!busy),
            )
            .clicked()
        {
            ops.push(Op::CloseVaultDialog);
            for id in ["password", "confirm", "current"] {
                forget_secret_field(ctx, egui::Id::new(("vault-field", id)));
            }
        }
    });
}
