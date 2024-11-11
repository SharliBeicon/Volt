use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{CentralPanel, Color32, Context, FontData, FontDefinitions, FontFamily, SidePanel, TopBottomPanel};
use egui_extras::install_image_loaders;
use std::{collections::HashSet, path::PathBuf, str::FromStr};
mod blerp;
mod test;
// TODO: Move everything into components (visual)
mod browser;
mod info;
mod visual;

use browser::{Browser, Category, OpenFolder};
use visual::ThemeColors;

fn main() -> eframe::Result {
    info::handle();

    // Panic handling
    std::panic::set_hook(Box::new(|panic_info| {
        info::panic_handler(panic_info);
    }));

    let title = "Volt";
    let native_options = NativeOptions {
        vsync: true,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::wgpu::PresentMode::Immediate,
            ..Default::default()
        },
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
        install_image_loaders(&cc.egui_ctx);
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("IBMPlexMono".to_owned(), FontData::from_static(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")));
        fonts.families.insert(FontFamily::Name("IBMPlexMono".into()), vec!["IBMPlexMono".to_owned()]);
        cc.egui_ctx.set_fonts(fonts);
        Self {
            browser: Browser {
                selected_category: Category::Files,
                open_folders: vec![OpenFolder {
                    path: PathBuf::from_str("/").unwrap(),
                    expanded_directories: HashSet::new(),
                }],
                preview: browser::Preview {
                    preview_thread: Some(std::thread::spawn(|| {})),
                },
                offset_y: 0.,
                dragging_audio: false,
                dragging_audio_text: String::new(),
            },
            themes: ThemeColors::default(),
        }
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        TopBottomPanel::top("navbar").frame(egui::Frame::default().fill(self.themes.navbar)).show(ctx, |ui| {
            visual::navbar::paint_navbar(ui);
        });
        SidePanel::left("sidebar").default_width(300.).frame(egui::Frame::default().fill(self.themes.browser)).show(ctx, |ui| {
            self.browser.paint(ctx, ui, &ui.clip_rect(), &self.themes);
        });
        CentralPanel::default()
            .frame(egui::Frame::none())
            .frame(egui::Frame::default().fill(Color32::from_hex("#1e222f").unwrap_or_default()))
            .show(ctx, |ui| {
                visual::main::paint_main(ui, &self.themes);
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
