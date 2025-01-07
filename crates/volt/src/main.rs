use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{hex_color, CentralPanel, Context, FontData, FontDefinitions, FontFamily, FontId, SidePanel, TextStyle, TopBottomPanel};
use egui_extras::install_image_loaders;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr, sync::mpsc::channel, thread::spawn, time::Instant};
// TODO: Move everything into components (visual)
mod browser;
mod info;
mod visual;

use browser::{Browser, Category, Preview, PreviewData};
use visual::{central::central, navbar::navbar, ThemeColors};

fn main() -> eframe::Result {
    info::handle();

    #[cfg(not(debug_assertions))]
    {
        // Panic handling
        std::panic::set_hook(Box::new(|panic_info| {
            info::panic_handler(panic_info);
        }));
    }

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
            browser: Browser {
                selected_category: Category::Files,
                other_category_hovered: false,
                open_paths: vec![PathBuf::from_str("/").unwrap()],
                preview: {
                    let (path_tx, path_rx) = channel::<PathBuf>();
                    let (file_data_tx, file_data_rx) = channel();
                    // FIXME: Temporary rodio playback, might need to use cpal or make rodio proper
                    spawn(move || {
                        let (_stream, handle) = OutputStream::try_default().unwrap();
                        let sink = Sink::try_new(&handle).unwrap();
                        let mut last_path = None;
                        loop {
                            let Ok(path) = path_rx.recv() else {
                                break;
                            };
                            let source = Decoder::new(BufReader::new(File::open(&path).unwrap())).unwrap();
                            let empty = sink.empty();
                            sink.stop();
                            if last_path != Some(path.clone()) || empty {
                                file_data_tx
                                    .send(PreviewData {
                                        length: source.total_duration(),
                                        started_playing: Instant::now(),
                                    })
                                    .unwrap();
                                sink.append(source);
                            }
                            last_path = Some(path.clone());
                        }
                    });
                    Preview {
                        path_tx,
                        file_data_rx,
                        path: None,
                        file_data: None,
                    }
                },
                hovered_entry: None,
            },
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
