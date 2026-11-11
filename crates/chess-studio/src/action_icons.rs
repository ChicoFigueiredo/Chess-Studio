//! Nome do ícone da Ação (`chess_input::actions::meta(..).icon`, kebab-case) -> glifo Phosphor.
//! Fica à parte para o Ribbon, a Paleta e a barra de acesso rápido usarem a mesma tabela.

use egui_phosphor::regular as icon;

/// Glifo Phosphor do ícone; vazio se o nome não consta da tabela (o teste cobre todas as Ações).
pub fn glyph(name: &str) -> &'static str {
    match name {
        "arrow-clockwise" => icon::ARROW_CLOCKWISE,
        "arrow-counter-clockwise" => icon::ARROW_COUNTER_CLOCKWISE,
        "arrow-square-out" => icon::ARROW_SQUARE_OUT,
        "arrows-clockwise" => icon::ARROWS_CLOCKWISE,
        "arrows-down-up" => icon::ARROWS_DOWN_UP,
        "arrows-left-right" => icon::ARROWS_LEFT_RIGHT,
        "barbell" => icon::BARBELL,
        "books" => icon::BOOKS,
        "cloud-arrow-down" => icon::CLOUD_ARROW_DOWN,
        "compass" => icon::COMPASS,
        "camera" => icon::CAMERA,
        "camera-rotate" => icon::CAMERA_ROTATE,
        "caret-line-up" => icon::CARET_LINE_UP,
        "chart-bar" => icon::CHART_BAR,
        "chart-line" => icon::CHART_LINE,
        "chart-polar" => icon::CHART_POLAR,
        "checkerboard" => icon::CHECKERBOARD,
        "clock" => icon::CLOCK,
        "clock-counter-clockwise" => icon::CLOCK_COUNTER_CLOCKWISE,
        "command" => icon::COMMAND,
        "corners-out" => icon::CORNERS_OUT,
        "cpu" => icon::CPU,
        "crosshair" => icon::CROSSHAIR,
        "cube" => icon::CUBE,
        "cube-focus" => icon::CUBE_FOCUS,
        "cursor" => icon::CURSOR,
        "cursor-click" => icon::CURSOR_CLICK,
        "export" => icon::EXPORT,
        "eye" => icon::EYE,
        "eye-slash" => icon::EYE_SLASH,
        "file-plus" => icon::FILE_PLUS,
        "flag" => icon::FLAG,
        "frame-corners" => icon::FRAME_CORNERS,
        "gavel" => icon::GAVEL,
        "gear" => icon::GEAR,
        "grid-four" => icon::GRID_FOUR,
        "hand-grabbing" => icon::HAND_GRABBING,
        "hand-pointing" => icon::HAND_POINTING,
        "handshake" => icon::HANDSHAKE,
        "keyboard" => icon::KEYBOARD,
        "plugs-connected" => icon::PLUGS_CONNECTED,
        "layout" => icon::LAYOUT,
        "lifebuoy" => icon::LIFEBUOY,
        "list-numbers" => icon::LIST_NUMBERS,
        "magnifying-glass-minus" => icon::MAGNIFYING_GLASS_MINUS,
        "magnifying-glass-plus" => icon::MAGNIFYING_GLASS_PLUS,
        "notepad" => icon::NOTEPAD,
        "pause" => icon::PAUSE,
        "picture-in-picture" => icon::PICTURE_IN_PICTURE,
        "scales" => icon::SCALES,
        "square" => icon::SQUARE,
        "squares-four" => icon::SQUARES_FOUR,
        "stack" => icon::STACK,
        "student" => icon::STUDENT,
        "users" => icon::USERS,
        "table" => icon::TABLE,
        "text-aa" => icon::TEXT_AA,
        "timer" => icon::TIMER,
        "tree-structure" => icon::TREE_STRUCTURE,
        "x-circle" => icon::X_CIRCLE,
        _ => "",
    }
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
            assert!(
                !glyph(name).is_empty(),
                "{action:?}: ícone `{name}` sem glifo"
            );
        }
    }
}
