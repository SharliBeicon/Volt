use eframe::egui;
use egui::{include_image, Color32, Image, Margin, RichText, Ui, Vec2, Widget};

use crate::ResponseFlatten;

pub fn navbar() -> impl Widget {
    |ui: &mut Ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            egui::Frame::default()
                .inner_margin(Margin {
                    left: 5.,
                    right: 5.,
                    top: 5.,
                    bottom: 5.,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(Image::new(include_image!("../images/icons/icon.png")).fit_to_exact_size(Vec2::splat(40.))).union(
                            ui.vertical(|ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                                    ui.label(RichText::new("Volt").size(20.).color(Color32::WHITE)).union(ui.label("Version INDEV"))
                                })
                            })
                            .flatten(),
                        )
                    })
                })
        })
        .flatten()
    }
}
