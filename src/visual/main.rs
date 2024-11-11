use eframe::egui;
use egui::{FontFamily, FontId, RichText, Ui};

use crate::visual::ThemeColors;

pub fn paint_main(ui: &mut Ui, theme: &ThemeColors) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new("In development").font(FontId::new(32.0, FontFamily::Name("IBMPlexMono".into()))).color(theme.bg_text));
    });
}
