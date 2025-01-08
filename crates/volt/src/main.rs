use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{hex_color, CentralPanel, Context, FontData, FontDefinitions, FontFamily, FontId, SidePanel, TextStyle, TopBottomPanel, ViewportBuilder};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use info::handle_args;
// TODO: Move everything into components (visual)
mod info;
mod visual;

use visual::{browser::Browser, central::central, navbar::navbar, ThemeColors};

fn main() -> eframe::Result {
    setup_panic!();
    if handle_args().is_break() {
        return Ok(());
    };

    let title = "Volt";
    let native_options = NativeOptions {
        vsync: true,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::wgpu::PresentMode::Immediate,
            ..Default::default()
        },
        viewport: ViewportBuilder::default().with_drag_and_drop(true),
        ..Default::default()
    };
    run_native(title, native_options, Box::new(|cc| Ok(Box::new(VoltApp::new(cc)))))
}

struct VoltApp {
    pub browser: Browser,
    pub themes: ThemeColors,
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        const FONT_NAME: &str = "IBMPlexMono";
        install_image_loaders(&cc.egui_ctx);
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert(FONT_NAME.to_string(), FontData::from_static(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")));
        fonts.families.insert(FontFamily::Name(FONT_NAME.into()), vec![FONT_NAME.to_string()]);
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.all_styles_mut(|style| {
            let id = FontId::new(12., FontFamily::Name(FONT_NAME.into()));
            style.override_font_id = Some(id.clone());
            style.text_styles = [TextStyle::Heading, TextStyle::Body, TextStyle::Button, TextStyle::Small, TextStyle::Monospace]
                .into_iter()
                .map(|style| (style, id.clone()))
                .collect();
        });
        Self {
            browser: Browser::new(),
            themes: ThemeColors::default(),
        }
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        ctx.request_repaint();
        TopBottomPanel::top("navbar").frame(egui::Frame::default().fill(self.themes.navbar)).show(ctx, |ui| {
            ui.add(navbar());
        });
        SidePanel::left("sidebar").default_width(300.).frame(egui::Frame::default().fill(self.themes.browser)).show(ctx, |ui| {
            ui.add(self.browser.widget(ctx, &self.themes));
        });
        CentralPanel::default().frame(egui::Frame::default().fill(hex_color!("#1e222f"))).show(ctx, |ui| {
            ui.add(central(&self.themes));
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
