//! Galeria de Temas de Opções > Aparência (issue #206): "Seguir o sistema" no topo e, abaixo, os
//! cartões agrupados por família, na ordem do catálogo. Clicar num cartão fixa o Tema na hora;
//! marcar "Seguir o sistema" volta a alternar entre Escuro e Claro.

use crate::i18n::I18n;
use crate::ui_theme::{
    ThemeCardMark, ThemeId, ThemeMode, cs_checkbox, cs_theme_card, palette, strong,
    themes_by_family,
};
use bevy_egui::egui;

/// Como o cartão do Tema `id` aparece, dado o modo e o Tema efetivo.
pub fn mark_for(mode: ThemeMode, id: ThemeId, effective: ThemeId) -> ThemeCardMark {
    match mode {
        ThemeMode::Fixed(chosen) if chosen == id => ThemeCardMark::Selected,
        ThemeMode::System if effective == id => ThemeCardMark::Effective,
        _ => ThemeCardMark::None,
    }
}

/// O modo depois de mexer em "Seguir o sistema": ligado volta ao sistema; desligado fixa o Tema
/// que estava valendo, sem mudar nada na tela.
pub fn after_follow_system(on: bool, effective: ThemeId) -> ThemeMode {
    if on {
        ThemeMode::System
    } else {
        ThemeMode::Fixed(effective)
    }
}

/// Desenha a galeria e devolve o novo modo, se o usuário mudou algo. `effective` é o Tema que
/// está valendo agora (o do sistema, quando se segue o sistema).
pub fn theme_gallery(
    ui: &mut egui::Ui,
    i18n: &I18n,
    mode: ThemeMode,
    effective: ThemeId,
) -> Option<ThemeMode> {
    let mut result = None;
    let mut follow = mode == ThemeMode::System;
    if cs_checkbox(ui, &mut follow, &i18n.t(ThemeMode::SYSTEM_LABEL_KEY)).changed() {
        result = Some(after_follow_system(follow, effective));
    }
    if follow {
        let muted = palette(ui).ink_muted;
        ui.label(
            egui::RichText::new(i18n.t_with("opt-theme-following", &[("theme", effective.name())]))
                .small()
                .color(muted),
        );
    }
    for (family, swatches) in themes_by_family() {
        ui.add_space(6.0);
        ui.label(strong(ui.ctx(), family));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            for swatch in &swatches {
                let mark = mark_for(mode, swatch.id, effective);
                if cs_theme_card(ui, swatch, mark).clicked() && mark != ThemeCardMark::Selected {
                    result = Some(ThemeMode::Fixed(swatch.id));
                }
            }
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(key: &str) -> ThemeId {
        ThemeId::from_key(key).expect("tema")
    }

    #[test]
    fn families_follow_the_catalog_order_and_hold_all_22_themes() {
        let groups = themes_by_family();
        let names: Vec<_> = groups.iter().map(|(f, _)| *f).collect();
        assert_eq!(
            names,
            ["Padrão", "Materiais", "Jogo", "Aberturas", "Alto contraste"]
        );
        let total: usize = groups.iter().map(|(_, l)| l.len()).sum();
        assert_eq!(total, 22);
        // Dentro da família, a ordem do catálogo; no conjunto, todos os Temas uma vez só.
        let flat: Vec<_> = groups
            .iter()
            .flat_map(|(_, l)| l.iter().map(|s| s.id))
            .collect();
        let mut sorted = flat.clone();
        sorted.sort_by_key(|t| t.key());
        sorted.dedup();
        assert_eq!(sorted.len(), 22);
        assert_eq!(groups[0].1[0].id, ThemeId::DARK);
    }

    #[test]
    fn the_chosen_card_is_selected_and_follow_system_marks_only_the_effective_one() {
        let feltro = id("feltro");
        assert_eq!(
            mark_for(ThemeMode::Fixed(feltro), feltro, feltro),
            ThemeCardMark::Selected
        );
        assert_eq!(
            mark_for(ThemeMode::Fixed(feltro), ThemeId::DARK, feltro),
            ThemeCardMark::None
        );
        assert_eq!(
            mark_for(ThemeMode::System, ThemeId::DARK, ThemeId::DARK),
            ThemeCardMark::Effective
        );
        assert_eq!(
            mark_for(ThemeMode::System, feltro, ThemeId::DARK),
            ThemeCardMark::None
        );
    }

    #[test]
    fn follow_the_system_toggles_between_system_and_the_current_theme() {
        assert_eq!(after_follow_system(true, ThemeId::LIGHT), ThemeMode::System);
        assert_eq!(
            after_follow_system(false, ThemeId::LIGHT),
            ThemeMode::Fixed(ThemeId::LIGHT)
        );
    }

    /// Um clique de verdade num cartão devolve o Tema fixo clicado (o clique na caixa "Seguir o
    /// sistema" devolve `System`, então não se confunde).
    #[test]
    fn clicking_a_card_fixes_that_theme() {
        let ctx = egui::Context::default();
        let i18n = I18n::pt_br();
        let run = |events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 2000.0),
                )),
                ..egui::RawInput::default()
            };
            let mut out = None;
            let mut full = ctx.run_ui(raw, |ui| {
                out = Some(theme_gallery(ui, &i18n, ThemeMode::DARK, ThemeId::DARK));
            });
            full.textures_delta.clear();
            out.expect("quadro")
        };
        assert_eq!(run(vec![]), None, "sem interação, nada muda");
        let mut picked = Vec::new();
        for y in (60..1500).step_by(20) {
            for x in (20..800).step_by(20) {
                let at = egui::pos2(x as f32, y as f32);
                let ev = |pressed| egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                run(vec![egui::Event::PointerMoved(at)]);
                run(vec![ev(true)]);
                if let Some(ThemeMode::Fixed(id)) = run(vec![ev(false)]) {
                    picked.push(id);
                }
            }
        }
        assert!(!picked.is_empty(), "algum cartão respondeu ao clique");
        assert!(picked.iter().all(|id| *id != ThemeId::DARK));
    }
}
