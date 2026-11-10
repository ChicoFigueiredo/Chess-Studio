//! Ícones do Ribbon: os das Ações vêm de `action_icons`; aqui só os de navegação e de chrome.

use egui_phosphor::regular::*;

/// Glifo do ícone (Ação ou chrome), ou `None` se o nome não é conhecido.
pub fn glyph(name: &str) -> Option<&'static str> {
    let action = crate::action_icons::glyph(name);
    if !action.is_empty() {
        return Some(action);
    }
    Some(match name {
        "arrow-left" => ARROW_LEFT,
        "caret-down" => CARET_DOWN,
        "caret-up" => CARET_UP,
        "dots-three" => DOTS_THREE,
        "folder-open" => FOLDER_OPEN,
        "list" => LIST,
        "magnifying-glass" => MAGNIFYING_GLASS,
        "plus" => PLUS,
        "rows" => ROWS,
        "user" => USER,
        "users" => USERS,
        "x" => X,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_input::actions::meta;
    use chess_input::controls::Action;

    #[test]
    fn every_action_icon_has_a_glyph() {
        for action in Action::ALL {
            let name = meta(action).icon;
            assert!(glyph(name).is_some(), "ícone sem glifo: {name}");
        }
    }

    #[test]
    fn the_chrome_icons_resolve() {
        for name in [
            "dots-three",
            "caret-down",
            "list",
            "rows",
            "arrow-left",
            "folder-open",
        ] {
            assert!(glyph(name).is_some(), "{name}");
        }
    }
}
