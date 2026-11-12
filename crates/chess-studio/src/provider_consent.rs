//! Diálogo padrão "Ligar <Provedor>?" (CX-09, #214): o primeiro uso de um recurso com o
//! Provedor desligado mostra o que será enviado e pergunta. É a API que os próximos tickets
//! (Explorador, importação, Engines remotas) chamam quando precisam de um Provedor desligado:
//!
//! ```ignore
//! fn open_explorer(net: Res<NetService>, mut dialog: ResMut<ConsentDialog>) {
//!     match net.ensure_enabled(ProviderId::Lichess, Some("O Explorador"), &mut dialog) {
//!         Readiness::Ready => { /* pode pedir: net.net().fetch(...) */ }
//!         Readiness::Asked => { /* espera ProviderConsentAnswered e tenta de novo */ }
//!         Readiness::NoNetworkService => { /* nada a ligar */ }
//!     }
//! }
//!
//! fn on_answer(mut answers: MessageReader<ProviderConsentAnswered>) {
//!     for answer in answers.read() {
//!         if answer.accepted && answer.provider == ProviderId::Lichess { /* tenta de novo */ }
//!     }
//! }
//! ```
//!
//! O texto do que será enviado vem do catálogo (`Provider::sends`); o diálogo grava a escolha
//! (`Net::set_enabled`) só se a pessoa clicar em "Ligar". Só Apresentação (ADR-0001).

use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::ribbon::RibbonState;
use crate::ui_theme::{ButtonVariant, cs_button, palette, strong_font};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_net::ProviderId;

/// A resposta da pessoa ao diálogo, para o recurso que o pediu tentar de novo.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderConsentAnswered {
    pub provider: ProviderId,
    /// `true`: o Provedor foi ligado. `false`: "Agora não" (ou Esc).
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Ask {
    provider: ProviderId,
    feature: Option<String>,
}

/// O diálogo "Ligar <Provedor>?": um pedido por vez (o primeiro vence; os seguintes do mesmo
/// Provedor são ignorados, os de outro Provedor esperam a vez no próximo quadro do recurso).
#[derive(Resource, Debug, Default)]
pub struct ConsentDialog {
    ask: Option<Ask>,
}

impl ConsentDialog {
    /// Abre o diálogo para `provider`. `feature` é o nome do recurso na frase ("O Explorador").
    #[allow(dead_code)] // API para os próximos tickets
    pub fn ask(&mut self, provider: ProviderId, feature: Option<String>) {
        if self.ask.is_none() {
            self.ask = Some(Ask { provider, feature });
        }
    }

    /// O Provedor que o diálogo aberto pergunta, se há um.
    #[allow(dead_code)]
    pub fn current(&self) -> Option<ProviderId> {
        self.ask.as_ref().map(|a| a.provider)
    }

    /// Fecha o diálogo sem resposta (a pessoa trocou de tela, por exemplo).
    pub fn dismiss(&mut self) {
        self.ask = None;
    }
}

pub struct ProviderConsentPlugin;

impl Plugin for ProviderConsentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsentDialog>()
            .add_message::<ProviderConsentAnswered>()
            .add_systems(EguiPrimaryContextPass, consent_dialog);
    }
}

/// O que a pessoa escolheu neste quadro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    TurnOn,
    NotNow,
    OpenCenter,
}

fn consent_dialog(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut dialog: ResMut<ConsentDialog>,
    service: Res<NetService>,
    ribbon: Option<ResMut<RibbonState>>,
    mut answers: MessageWriter<ProviderConsentAnswered>,
) -> Result {
    let Some(ask) = dialog.ask.clone() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let mut choice = None;
    // Escurece o resto: o diálogo é modal.
    egui::Area::new(egui::Id::new("provider-consent-dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(&ctx, |ui| {
            let rect = ctx.viewport_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
        });
    egui::Window::new(i18n.t_with("consent-title", &[("provider", ask.provider.info().name)]))
        .id(egui::Id::new("provider-consent"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            choice = body(ui, &ctx, &i18n, &ask);
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(Choice::NotNow);
    }
    let Some(choice) = choice else {
        return Ok(());
    };
    let provider = ask.provider;
    match choice {
        Choice::TurnOn => {
            let accepted = service.set_enabled(provider, true).is_ok();
            answers.write(ProviderConsentAnswered { provider, accepted });
        }
        Choice::NotNow => {
            answers.write(ProviderConsentAnswered {
                provider,
                accepted: false,
            });
        }
        Choice::OpenCenter => {
            if let Some(mut ribbon) = ribbon {
                ribbon.backstage = Some(crate::ribbon::BackstagePage::Connections);
            }
            answers.write(ProviderConsentAnswered {
                provider,
                accepted: false,
            });
        }
    }
    dialog.dismiss();
    Ok(())
}

fn body(ui: &mut egui::Ui, ctx: &egui::Context, i18n: &I18n, ask: &Ask) -> Option<Choice> {
    ui.set_max_width(460.0);
    // Em janela baixa o texto rola e os botões ficam sempre visíveis embaixo (desconta o Ribbon, a
    // barra de status, o título e a fileira de botões).
    let text_height = (ctx.viewport_rect().height() - 480.0).max(120.0);
    egui::ScrollArea::vertical()
        .max_height(text_height)
        .auto_shrink([false, true])
        .show(ui, |ui| body_text(ui, ctx, i18n, ask));
    ui.add_space(16.0);
    let mut choice = None;
    ui.horizontal(|ui| {
        if cs_button(ui, &i18n.t("consent-turn-on"), ButtonVariant::Primary).clicked() {
            choice = Some(Choice::TurnOn);
        }
        if cs_button(ui, &i18n.t("consent-not-now"), ButtonVariant::Secondary).clicked() {
            choice = Some(Choice::NotNow);
        }
        if cs_button(ui, &i18n.t("consent-open-center"), ButtonVariant::Subtle).clicked() {
            choice = Some(Choice::OpenCenter);
        }
    });
    choice
}

fn body_text(ui: &mut egui::Ui, ctx: &egui::Context, i18n: &I18n, ask: &Ask) {
    let p = palette(ui);
    let provider = ask.provider.info();
    let intro = match &ask.feature {
        Some(feature) => i18n.t_with(
            "consent-intro-feature",
            &[("feature", feature), ("provider", provider.name)],
        ),
        None => i18n.t_with("consent-intro", &[("provider", provider.name)]),
    };
    ui.label(intro);
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(i18n.t("consent-sends-label"))
            .font(strong_font(ctx, 14.0))
            .color(p.ink),
    );
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(p.surface_2)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let state = crate::connections::model::CardState::Off;
            ui.label(crate::connections::model::sends_text(i18n, provider, state));
        });
    let hosts = crate::connections::model::hosts(provider);
    if !hosts.is_empty() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(i18n.t("consent-where-label"))
                .font(strong_font(ctx, 14.0))
                .color(p.ink),
        );
        ui.label(
            egui::RichText::new(hosts.join(", "))
                .text_style(egui::TextStyle::Small)
                .color(p.ink_muted),
        );
    }
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(i18n.t("consent-footer"))
            .color(p.ink_muted)
            .text_style(egui::TextStyle::Small),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_ask_wins_and_dismiss_clears_it() {
        let mut dialog = ConsentDialog::default();
        assert_eq!(dialog.current(), None);
        dialog.ask(ProviderId::Lichess, Some("O Explorador".into()));
        dialog.ask(ProviderId::ChessCom, None);
        assert_eq!(dialog.current(), Some(ProviderId::Lichess));
        dialog.dismiss();
        assert_eq!(dialog.current(), None);
        dialog.ask(ProviderId::ChessCom, None);
        assert_eq!(dialog.current(), Some(ProviderId::ChessCom));
    }

    #[test]
    fn every_dialog_text_exists_in_pt_br_and_names_no_account_login_or_credential() {
        let i18n = I18n::pt_br();
        for key in [
            "consent-sends-label",
            "consent-footer",
            "consent-turn-on",
            "consent-not-now",
            "consent-open-center",
        ] {
            assert_ne!(i18n.t(key), key, "{key}");
        }
        let title = i18n.t_with("consent-title", &[("provider", "Lichess")]);
        assert_eq!(title, "Ligar Lichess?");
        let all = [
            title,
            i18n.t_with("consent-intro", &[("provider", "Lichess")]),
            i18n.t_with(
                "consent-intro-feature",
                &[("feature", "O Explorador"), ("provider", "Lichess")],
            ),
            i18n.t("consent-footer"),
        ]
        .join(" ")
        .to_lowercase();
        for banned in ["conta", "login", "credencial"] {
            assert!(!all.contains(banned), "{banned}");
        }
    }
}
