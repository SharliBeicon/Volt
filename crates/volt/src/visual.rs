use egui::{hex_color, Color32};

// Expose components
pub mod browser;
pub mod central;
pub mod navbar;
pub mod switch;

// Theming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub navbar_background: Color32,
    pub navbar_outline: Color32,
    pub central_background: Color32,
    pub browser: Color32,
    pub browser_outline: Color32,
    pub browser_selected_button_fg: Color32,
    pub browser_unselected_button_fg: Color32,
    pub browser_unselected_hover_button_fg: Color32,
    pub browser_invalid_name_bg: Color32,
    pub browser_unselected_hover_button_fg_invalid: Color32,
    pub browser_unselected_button_fg_invalid: Color32,
    pub bg_text: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            navbar_background: hex_color!("262b3b"),
            navbar_outline: hex_color!("00000080"),
            central_background: hex_color!("1e222f"),
            browser: hex_color!("242938"),
            browser_outline: hex_color!("00000080"),
            browser_selected_button_fg: hex_color!("ffcf7b"),
            browser_unselected_button_fg: hex_color!("646d88"),
            browser_unselected_hover_button_fg: hex_color!("8591b5"),
            browser_invalid_name_bg: hex_color!("ff000010"),
            browser_unselected_button_fg_invalid: hex_color!("a46d88"),
            browser_unselected_hover_button_fg_invalid: hex_color!("f591b5"),
            bg_text: hex_color!("646987"),
        }
    }
}
