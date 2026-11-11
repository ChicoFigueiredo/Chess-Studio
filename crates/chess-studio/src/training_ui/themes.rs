//! Nomes dos temas dos problemas do Lichess na interface (#238): o arquivo traz
//! identificadores fixos (`mateIn2`, `fork`, `pin`…) e a pessoa vê o nome em português
//! (`puzzle-theme-<id>` nos textos). Identificador desconhecido aparece como veio.

use crate::i18n::I18n;
use chess_training::canonical_theme;

/// Nome em português do tema; o identificador cru se o app não o conhece.
pub(super) fn theme_name(i18n: &I18n, id: &str) -> String {
    canonical_theme(id)
        .map(|known| format!("puzzle-theme-{known}"))
        .filter(|key| i18n.has(key))
        .map_or_else(|| id.to_owned(), |key| i18n.t(&key))
}

/// Até `max` nomes separados por vírgula; o resto vira "e mais N".
pub(super) fn theme_names(i18n: &I18n, ids: &[String], max: usize) -> String {
    let shown: Vec<String> = ids
        .iter()
        .take(max)
        .map(|id| theme_name(i18n, id))
        .collect();
    let text = shown.join(", ");
    if ids.len() > max {
        i18n.t_with(
            "training-exercise-themes-more",
            &[("themes", &text), ("more", &(ids.len() - max).to_string())],
        )
    } else {
        text
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_training::{HIDDEN_THEMES, OFFICIAL_THEMES, ThemeGroup, known_themes};

    #[test]
    fn every_official_theme_has_a_portuguese_name() {
        let i18n = I18n::pt_br();
        for id in known_themes() {
            let key = format!("puzzle-theme-{id}");
            assert!(i18n.has(&key), "falta o nome em português do tema `{id}`");
            let name = theme_name(&i18n, id);
            assert!(
                name != id && name != key && !name.trim().is_empty(),
                "`{id}` sem nome de verdade: {name}"
            );
        }
        assert_eq!(OFFICIAL_THEMES.len(), 73);
        assert_eq!(HIDDEN_THEMES.len(), 1);
    }

    #[test]
    fn every_group_has_a_name_and_themes_are_unique() {
        let i18n = I18n::pt_br();
        for group in ThemeGroup::ALL {
            assert!(
                i18n.has(&format!("puzzle-theme-group-{}", group.key())),
                "{group:?}"
            );
            assert!(chess_training::themes_in(group).next().is_some());
        }
        let mut ids: Vec<&str> = known_themes().collect();
        let all = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all, "tema repetido na lista oficial");
        // Nomes diferentes para temas diferentes (não confunde a pessoa).
        let mut names: Vec<String> = known_themes().map(|id| theme_name(&i18n, id)).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all, "dois temas com o mesmo nome");
    }

    #[test]
    fn names_are_looked_up_without_caring_about_case_and_unknown_ids_pass_through() {
        let i18n = I18n::pt_br();
        assert_eq!(theme_name(&i18n, "mateIn2"), "Mate em 2");
        assert_eq!(theme_name(&i18n, "MATEIN2"), "Mate em 2");
        assert_eq!(theme_name(&i18n, "pin"), "Cravada");
        assert_eq!(theme_name(&i18n, "temaNovoDoLichess"), "temaNovoDoLichess");
        let many: Vec<String> = ["fork", "pin", "skewer", "mateIn1"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            theme_names(&i18n, &many, 3),
            "Garfo (ataque duplo), Cravada, Espeto (cravada invertida) e mais 1"
        );
    }
}
