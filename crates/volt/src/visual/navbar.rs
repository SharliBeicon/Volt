use eframe::egui;
use egui::{include_image, Image, TextureOptions, Ui, Vec2, Widget};

use super::ThemeColors;

pub fn navbar(themes: &ThemeColors) -> impl Widget + use<'_> {
    |ui: &mut Ui| {
        let navbar_texture_image = super::build_gradient(40, themes.navbar_background_gradient_top, themes.navbar_background_gradient_bottom);
        let navbar_texture = ui.ctx().load_texture("navbar_texture", navbar_texture_image, TextureOptions::default());

        ui.painter().image(
            navbar_texture.id(),
            ui.available_rect_before_wrap(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.horizontal(|ui| {
            egui::Frame::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Image::new(include_image!("../images/icons/navbar-icon.svg")).fit_to_exact_size(Vec2::splat(40.)))
                    // .union(
                    //     ui.vertical(|ui| ui.label(RichText::new("Volt").size(20.).color(Color32::WHITE)).union(ui.label("Version INDEV")))
                    //         .response,
                    // )
                })
            })
        })
        .response
    }
}
