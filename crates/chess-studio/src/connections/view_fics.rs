//! O aviso do FICS no cartão da Central (#254): conexão sem criptografia, só visitante, sem
//! senha e sem reconexão. Não há campo de senha nem autorização neste cartão; ligar e desligar
//! é o Switch de sempre (consentimento por Provedor). Jogar fica na tela da Partida online.

use super::View;
use crate::ui_theme::Palette;
use bevy_egui::egui::{RichText, Ui};

impl View<'_> {
    /// O bloco do aviso, logo abaixo de "O que você pode fazer agora" do cartão do FICS.
    pub(super) fn fics_block(&mut self, ui: &mut Ui, p: &Palette) {
        crate::online::fics_view::warning_lines(ui, self.i18n);
        ui.add_space(2.0);
        ui.label(
            RichText::new(self.i18n.t("fics-card-how"))
                .small()
                .color(p.ink_muted),
        );
        ui.add_space(8.0);
    }
}
