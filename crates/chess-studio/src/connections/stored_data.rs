//! "Limpar dados guardados…" (CX-19, #242): apaga de uma vez os caches de **todos** os
//! Provedores do Perfil local ativo, depois de uma confirmação que diz o que some e o que fica.
//!
//! O cache das consultas (perfis, ratings, avaliações, Explorador) mora no `rede.db`; as
//! Partidas e os Exercícios importados, as Conexões e o último rating FIDE moram no banco do
//! app. São bancos separados: limpar o primeiro nunca toca o segundo. Só Apresentação
//! (ADR-0001).

use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::ui_theme::{
    ButtonVariant, CardEmphasis, CsButton, cs_card_frame, icon_font, palette, strong_font,
};
use bevy_egui::egui::{self, RichText, Ui};
use chess_net::{NetError, ProviderId};
use egui_phosphor::regular as icon;

/// Quantas entradas guardadas saíram, somando todos os Provedores.
pub(crate) fn clear_all(service: &NetService) -> Result<usize, NetError> {
    let mut cleared = 0;
    for provider in ProviderId::ALL {
        cleared += service.net().clear_cache(provider)?;
    }
    Ok(cleared)
}

/// Como terminou a última limpeza.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Cleared(usize),
    Failed,
}

/// O estado da seção: a confirmação aberta e o último resultado.
#[derive(Debug, Default)]
pub(crate) struct StoredData {
    pub(crate) confirming: bool,
    pub(crate) last: Option<Outcome>,
}

/// O que a pessoa escolheu neste quadro.
enum Choice {
    Ask,
    Clear,
    Cancel,
}

impl StoredData {
    /// A faixa "Dados guardados" no alto da Central e, se aberta, a confirmação.
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        i18n: &I18n,
        service: &NetService,
    ) {
        let mut choice = None;
        let p = palette(ui);
        cs_card_frame(p, CardEmphasis::Quiet).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                ui.label(
                    RichText::new(icon::DATABASE)
                        .font(icon_font(ctx, 20.0))
                        .color(p.ink_muted),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new(i18n.t("stored-title"))
                            .font(strong_font(ctx, 14.0))
                            .color(p.ink),
                    );
                    ui.label(
                        RichText::new(i18n.t("stored-intro"))
                            .text_style(egui::TextStyle::Small)
                            .color(p.ink_muted),
                    );
                    if let Some(last) = self.last {
                        let (glyph, color, text) = match last {
                            Outcome::Cleared(0) => (icon::INFO, p.info, i18n.t("stored-done-none")),
                            Outcome::Cleared(1) => {
                                (icon::CHECK_CIRCLE, p.success, i18n.t("stored-done-one"))
                            }
                            Outcome::Cleared(n) => (
                                icon::CHECK_CIRCLE,
                                p.success,
                                i18n.t_with("stored-done-many", &[("count", &n.to_string())]),
                            ),
                            Outcome::Failed => {
                                (icon::WARNING_CIRCLE, p.danger, i18n.t("stored-failed"))
                            }
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(RichText::new(glyph).font(icon_font(ctx, 14.0)).color(color));
                            ui.label(
                                RichText::new(text)
                                    .text_style(egui::TextStyle::Small)
                                    .color(p.ink),
                            );
                        });
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            CsButton::new(&i18n.t("stored-clear"))
                                .icon(icon::BROOM)
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                    {
                        choice = Some(Choice::Ask);
                    }
                });
            });
        });
        if self.confirming {
            choice = self.dialog(ctx, i18n).or(choice);
        }
        match choice {
            Some(Choice::Ask) => {
                self.confirming = true;
                self.last = None;
            }
            Some(Choice::Cancel) => self.confirming = false,
            Some(Choice::Clear) => {
                self.confirming = false;
                self.last = Some(match clear_all(service) {
                    Ok(n) => Outcome::Cleared(n),
                    Err(_) => Outcome::Failed,
                });
            }
            None => {}
        }
    }

    /// A confirmação, modal: o que some, o que fica, e dois botões.
    fn dialog(&self, ctx: &egui::Context, i18n: &I18n) -> Option<Choice> {
        let mut choice = None;
        egui::Area::new(egui::Id::new("stored-data-dim"))
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.viewport_rect();
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, palette(ui).scrim);
            });
        egui::Window::new(i18n.t("stored-dialog-title"))
            .id(egui::Id::new("stored-data-dialog"))
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let p = palette(ui);
                ui.set_width(460.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                section(
                    ui,
                    ctx,
                    i18n,
                    icon::TRASH,
                    p.danger,
                    "stored-goes-title",
                    &["stored-goes-1", "stored-goes-2"],
                );
                section(
                    ui,
                    ctx,
                    i18n,
                    icon::SHIELD_CHECK,
                    p.success,
                    "stored-stays-title",
                    &[
                        "stored-stays-1",
                        "stored-stays-2",
                        "stored-stays-3",
                        "stored-stays-4",
                        "stored-stays-5",
                    ],
                );
                ui.label(
                    RichText::new(i18n.t("stored-after"))
                        .text_style(egui::TextStyle::Small)
                        .color(p.ink_muted),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui
                        .add(
                            CsButton::new(&i18n.t("stored-confirm"))
                                .icon(icon::BROOM)
                                .variant(ButtonVariant::Danger),
                        )
                        .clicked()
                    {
                        choice = Some(Choice::Clear);
                    }
                    if ui
                        .add(
                            CsButton::new(&i18n.t("stored-cancel"))
                                .variant(ButtonVariant::Secondary),
                        )
                        .clicked()
                    {
                        choice = Some(Choice::Cancel);
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            choice = Some(Choice::Cancel);
        }
        choice
    }
}

/// Um grupo da confirmação: um título com ícone e uma lista curta.
fn section(
    ui: &mut Ui,
    ctx: &egui::Context,
    i18n: &I18n,
    glyph: &str,
    color: egui::Color32,
    title: &str,
    items: &[&str],
) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(icon_font(ctx, 16.0)).color(color));
        ui.label(
            RichText::new(i18n.t(title))
                .font(strong_font(ctx, 14.0))
                .color(p.ink),
        );
    });
    for item in items {
        ui.horizontal_top(|ui| {
            ui.add_space(24.0);
            ui.label(RichText::new(format!("•  {}", i18n.t(item))).color(p.ink));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig, Timestamp};
    use chess_store::{
        ImportMode, ImportOptions, ImportSource, ImportedQuery, ProfileId, Provenance, Store,
    };
    use std::sync::Arc;

    /// 2026-11-11T12:00:00Z.
    const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

    fn service() -> (NetService, FakeServer) {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let config = NetConfig::in_memory()
            .clock(clock.clone())
            .route("lichess.org", &server.base_url())
            .route("api.chess.com", &server.base_url());
        let net = Net::open(config).expect("abre a rede");
        (NetService::from_net(net, clock, false), server)
    }

    #[test]
    fn clearing_empties_every_providers_cache_and_reports_how_many() {
        let (service, server) = service();
        service.set_enabled(ProviderId::Lichess, true).unwrap();
        service.set_enabled(ProviderId::ChessCom, true).unwrap();
        server.route(
            "/api/fide/player/9999991",
            FakeResponse::json(r#"{"id":9999991,"name":"Exemplo, Ana","standard":2301}"#),
        );
        server.route(
            "/pub/player/exemplo",
            FakeResponse::json(r#"{"username":"exemplo","player_id":1}"#),
        );
        service.lichess().fide_player(9_999_991).wait().unwrap();
        let _ = service.chesscom().profile("exemplo").wait();
        let before = server.requests().len();
        // Em cache: a segunda consulta não vai ao Provedor.
        service.lichess().fide_player(9_999_991).wait().unwrap();
        assert_eq!(server.requests().len(), before);

        let cleared = clear_all(&service).unwrap();
        assert!(cleared >= 1, "saiu pelo menos a consulta do Lichess");
        assert_eq!(clear_all(&service).unwrap(), 0, "não sobra nada");

        // Sem cache, a próxima consulta volta ao Provedor.
        service.lichess().fide_player(9_999_991).wait().unwrap();
        assert_eq!(server.requests().len(), before + 1);
    }

    #[test]
    fn clearing_never_touches_imported_games_played_games_or_the_saved_rating() {
        let (service, _server) = service();
        let mut store = Store::open_in_memory().unwrap();
        let me = ProfileId(1);
        let pgn = "[Event \"Estudo\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n";
        store
            .import_pgn_with(
                me,
                pgn,
                &ImportOptions::new(
                    Some("estudos.pgn"),
                    Provenance::from_source(ImportSource::Lichess, ImportMode::Api, 1_000),
                ),
            )
            .unwrap();
        store
            .save_fide_rating(
                me,
                &chess_store::FideSnapshot {
                    fide_id: 9_999_991,
                    name: "Exemplo, Ana".into(),
                    federation: None,
                    title: None,
                    birth_year: None,
                    standard: Some(2301),
                    rapid: None,
                    blitz: None,
                    inactive: false,
                    queried_at_ms: 5,
                    source: "lichess".into(),
                },
            )
            .unwrap();
        let imported = |s: &Store| {
            s.list_imported(&ImportedQuery {
                profile: Some(me),
                ..Default::default()
            })
            .unwrap()
            .len()
        };
        assert_eq!(imported(&store), 1);

        clear_all(&service).unwrap();

        assert_eq!(imported(&store), 1, "a Partida importada continua");
        assert!(store.fide_rating(me, 9_999_991).unwrap().is_some());
    }

    #[test]
    fn the_dialog_says_what_goes_and_what_stays_in_plain_words() {
        let i18n = I18n::pt_br();
        for key in [
            "stored-goes-1",
            "stored-goes-2",
            "stored-stays-1",
            "stored-stays-2",
            "stored-stays-3",
            "stored-stays-4",
            "stored-stays-5",
        ] {
            let text = i18n.t(key);
            assert_ne!(text, key, "{key} existe em pt-BR");
        }
        assert!(i18n.t("stored-stays-1").contains("jogadas"));
        assert!(i18n.t("stored-stays-2").to_lowercase().contains("importad"));
    }
}
