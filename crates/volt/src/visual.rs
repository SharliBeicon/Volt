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
    pub navbar_background_2: Color32,
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
    pub playlist_bar: Color32,
    pub playlist_beat: Color32,
    pub bg_text: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            navbar_background: hex_color!("2c2b47"),
            navbar_background_2: hex_color!("221f31"),
            navbar_outline: hex_color!("453f67"),
            central_background: hex_color!("1c1b2b"),
            browser: hex_color!("1d1b2b"),
            browser_outline: hex_color!("28243e"),
            browser_selected_button_fg: hex_color!("ffcf7b"),
            browser_unselected_button_fg: hex_color!("646d88"),
            browser_unselected_hover_button_fg: hex_color!("8591b5"),
            browser_invalid_name_bg: hex_color!("ff000010"),
            browser_unselected_button_fg_invalid: hex_color!("a46d88"),
            browser_unselected_hover_button_fg_invalid: hex_color!("f591b5"),
            playlist_bar: hex_color!("5e5a75"),
            playlist_beat: hex_color!("2e2b3f"),
            bg_text: hex_color!("646987"),
        }
    }
}

// Gradient func
pub fn build_gradient(height: usize, a: Color32, b: Color32) -> egui::ColorImage {
    let gradient_image = {
        let mut img = vec![0u8; height * 4];
        for y in 0..height {
            let t = y as f32 / (height - 1) as f32;
            let color = a.to_array();
            let color2 = b.to_array();
            let pixel = [
                (color[0] as f32 * (1.0 - t) + color2[0] as f32 * t) as u8,
                (color[1] as f32 * (1.0 - t) + color2[1] as f32 * t) as u8,
                (color[2] as f32 * (1.0 - t) + color2[2] as f32 * t) as u8,
                255,
            ];
            img[y * 4..(y + 1) * 4].copy_from_slice(&pixel);
        }
        egui::ColorImage::from_rgba_unmultiplied([1, height], &img)
    };
    gradient_image
}
