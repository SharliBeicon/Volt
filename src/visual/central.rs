use crate::visual::ThemeColors;
use crate::ResponseFlatten;
use eframe::egui;
use egui::{RichText, Ui, Widget};

pub fn central(theme: &ThemeColors) -> impl Widget + use<'_> {
    |ui: &mut Ui| ui.centered_and_justified(|ui| ui.label(RichText::new("In development").size(32.).color(theme.bg_text))).flatten()
}
