use crate::visual::ThemeColors;
use crate::ResponseFlatten;
use eframe::egui;
use egui::{FontFamily, FontId, RichText, Ui, Widget};

pub fn central(theme: &ThemeColors) -> impl Widget + use<'_> {
    |ui: &mut Ui| {
        ui.centered_and_justified(|ui| ui.label(RichText::new("In development").font(FontId::new(32.0, FontFamily::Name("IBMPlexMono".into()))).color(theme.bg_text)))
            .flatten()
    }
}
