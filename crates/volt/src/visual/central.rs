use std::fs::File;
use std::io::BufReader;
use std::ops::BitOr;
use std::path::PathBuf;

use eframe::egui;
use egui::{hex_color, vec2, Color32, Frame, Margin, Response, Sense, Stroke, Ui, Vec2, Widget};
use rodio::{Decoder, Source};
use tap::Tap;

pub struct Central {}

impl Default for Central {
    fn default() -> Self {
        Self::new()
    }
}

impl Central {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Widget for &mut Central {
    fn ui(self, ui: &mut Ui) -> Response {
        Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing = Vec2::splat(8.);
                ui.vertical(|ui| {
                    (0..5)
                        .map(|y| {
                            Frame::default()
                                .rounding(2.)
                                .inner_margin(Margin::same(8.))
                                .stroke(Stroke::new(1., hex_color!("00000080")))
                                .show(ui, |ui| {
                                    let (response, painter) = ui.allocate_painter(vec2(ui.available_width(), 48.), Sense::hover());
                                    if let Some(path) = response.dnd_hover_payload::<PathBuf>() {
                                        if let Some(duration) = File::open(&*path)
                                            .ok()
                                            .and_then(|file| Decoder::new(BufReader::new(file)).ok())
                                            .and_then(|decoder| decoder.total_duration())
                                        {
                                            let width = duration.as_secs_f32();
                                            painter.debug_rect(response.rect.tap_mut(|rect| rect.set_width(width)), Color32::RED, format!("{}", path.to_string_lossy()));
                                        }
                                    };
                                    ui.label(format!("Track {y}")).union(response)
                                })
                                .response
                        })
                        .reduce(Response::bitor)
                })
            })
            .response
    }
}
