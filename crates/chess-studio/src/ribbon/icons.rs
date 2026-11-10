//! Nome do ícone Phosphor (kebab-case, como a lista de Ações o guarda) -> glifo.

use egui_phosphor::regular::*;

/// Glifo do ícone, ou `None` se o nome não é conhecido.
pub fn glyph(name: &str) -> Option<&'static str> {
    Some(match name {
        "arrow-clockwise" => ARROW_CLOCKWISE,
        "arrow-counter-clockwise" => ARROW_COUNTER_CLOCKWISE,
        "arrow-left" => ARROW_LEFT,
        "arrow-square-out" => ARROW_SQUARE_OUT,
        "arrows-clockwise" => ARROWS_CLOCKWISE,
        "arrows-down-up" => ARROWS_DOWN_UP,
        "arrows-left-right" => ARROWS_LEFT_RIGHT,
        "barbell" => BARBELL,
        "books" => BOOKS,
        "camera" => CAMERA,
        "camera-rotate" => CAMERA_ROTATE,
        "caret-down" => CARET_DOWN,
        "caret-line-up" => CARET_LINE_UP,
        "caret-up" => CARET_UP,
        "chart-bar" => CHART_BAR,
        "chart-line" => CHART_LINE,
        "chart-polar" => CHART_POLAR,
        "checkerboard" => CHECKERBOARD,
        "clock" => CLOCK,
        "clock-counter-clockwise" => CLOCK_COUNTER_CLOCKWISE,
        "command" => COMMAND,
        "corners-out" => CORNERS_OUT,
        "cpu" => CPU,
        "crosshair" => CROSSHAIR,
        "cube" => CUBE,
        "cube-focus" => CUBE_FOCUS,
        "cursor" => CURSOR,
        "cursor-click" => CURSOR_CLICK,
        "dots-three" => DOTS_THREE,
        "eye" => EYE,
        "file-plus" => FILE_PLUS,
        "flag" => FLAG,
        "folder-open" => FOLDER_OPEN,
        "frame-corners" => FRAME_CORNERS,
        "gavel" => GAVEL,
        "gear" => GEAR,
        "grid-four" => GRID_FOUR,
        "hand-grabbing" => HAND_GRABBING,
        "hand-pointing" => HAND_POINTING,
        "handshake" => HANDSHAKE,
        "keyboard" => KEYBOARD,
        "layout" => LAYOUT,
        "lifebuoy" => LIFEBUOY,
        "list-numbers" => LIST_NUMBERS,
        "magnifying-glass" => MAGNIFYING_GLASS,
        "magnifying-glass-minus" => MAGNIFYING_GLASS_MINUS,
        "magnifying-glass-plus" => MAGNIFYING_GLASS_PLUS,
        "notepad" => NOTEPAD,
        "pause" => PAUSE,
        "picture-in-picture" => PICTURE_IN_PICTURE,
        "plus" => PLUS,
        "scales" => SCALES,
        "square" => SQUARE,
        "squares-four" => SQUARES_FOUR,
        "stack" => STACK,
        "student" => STUDENT,
        "table" => TABLE,
        "text-aa" => TEXT_AA,
        "timer" => TIMER,
        "tree-structure" => TREE_STRUCTURE,
        "user" => USER,
        "users" => USERS,
        "list" => LIST,
        "rows" => ROWS,
        "x" => X,
        "x-circle" => X_CIRCLE,
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
}
