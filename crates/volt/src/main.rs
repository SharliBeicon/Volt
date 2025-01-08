use std::io::{BufReader, Cursor};

use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{include_image, CentralPanel, Context, FontData, FontDefinitions, FontFamily, FontId, IconData, SidePanel, TextStyle, TopBottomPanel, ViewportBuilder};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use image::{io::Reader, ImageFormat, ImageReader};
use info::handle_args;
// TODO: Move everything into components (visual)
mod info;
mod visual;

use tap::{Pipe, Tap};
use visual::{browser::Browser, central::Central, navbar::navbar, ThemeColors};

fn main() -> eframe::Result {
    setup_panic!();
    if handle_args().is_break() {
        return Ok(());
    };
    run_native(
        "Volt",
        NativeOptions {
            vsync: true,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                present_mode: eframe::wgpu::PresentMode::Immediate,
                ..Default::default()
            },
            viewport: ViewportBuilder::default().with_drag_and_drop(true).with_icon(
                ImageReader::new(BufReader::new(Cursor::new(include_bytes!("images/icons/icon.png").as_ref())))
                    .tap_mut(|reader| reader.set_format(ImageFormat::Png))
                    .decode()
                    .unwrap()
                    .pipe(|image| IconData {
                        rgba: image.to_rgb8().into_raw(),
                        height: image.height(),
                        width: image.width(),
                    }),
            ),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(VoltApp::new(cc)))),
    )
}

struct VoltApp {
    pub browser: Browser,
    pub central: Central,
    pub themes: ThemeColors,
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        const FONT_NAME: &str = "IBMPlexMono";
        install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_fonts({
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert(FONT_NAME.to_string(), FontData::from_static(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")));
            fonts.families.insert(FontFamily::Proportional, vec![FONT_NAME.to_string()]);
            fonts
        });
        cc.egui_ctx.all_styles_mut(|style| {
            const BODY_TEXT_SIZE: f32 = 12.;
            let id = FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional);
            style.override_font_id = Some(id);
            style.text_styles = [
                (TextStyle::Heading, BODY_TEXT_SIZE * 1.5),
                (TextStyle::Body, BODY_TEXT_SIZE),
                (TextStyle::Button, BODY_TEXT_SIZE),
                (TextStyle::Small, BODY_TEXT_SIZE * 0.8),
                (TextStyle::Monospace, BODY_TEXT_SIZE),
            ]
            .map(|(text_style, size)| (text_style, FontId::new(size, FontFamily::Proportional)))
            .into();
        });
        let themes = ThemeColors::default();
        Self {
            browser: Browser::new(themes),
            central: Central::new(),
            themes,
        }
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        ctx.request_repaint();
        TopBottomPanel::top("navbar").frame(egui::Frame::default().fill(self.themes.navbar_background)).show(ctx, |ui| {
            ui.add(navbar());
        });
        SidePanel::left("browser").default_width(300.).frame(egui::Frame::default().fill(self.themes.browser)).show(ctx, |ui| {
            ui.add(&mut self.browser);
        });
        CentralPanel::default().frame(egui::Frame::default().fill(self.themes.central_background)).show(ctx, |ui| {
            ui.add(&mut self.central);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Log the exit
        println!("Volt is exiting!");

        // Perform any final saves or cleanup
        // For example, you might want to save user preferences or state
        // self.save_state();

        // Close any open connections or files
        // self.close_connections();

        // You can add more cleanup code here as needed
    }
}
