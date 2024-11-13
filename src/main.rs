use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{CentralPanel, CollapsingResponse, Color32, Context, FontData, FontDefinitions, FontFamily, InnerResponse, Response, SidePanel, TopBottomPanel};
use egui_extras::install_image_loaders;
use rodio::{Decoder, OutputStream, Sink};
use stable_try_trait_v2::Try;
use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr, sync::mpsc::channel, thread::spawn};
mod blerp;
mod test;
// TODO: Move everything into components (visual)
mod browser;
mod info;
mod visual;

use browser::{Browser, Category};
use visual::{central::central, navbar::navbar, ThemeColors};

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
                other_category_hovered: false,
                open_folders: vec![PathBuf::from_str("/").unwrap()],
                preview: {
                    let (tx, rx) = channel();
                    // FIXME: Temporary rodio playback, might need to use cpal or make rodio proper
                    spawn(move || {
                        let (_stream, handle) = OutputStream::try_default().unwrap();
                        let sink = Sink::try_new(&handle).unwrap();
                        loop {
                            let path = rx.recv().unwrap();
                            let source = Decoder::new(BufReader::new(File::open(path).unwrap())).unwrap();
                            sink.stop();
                            sink.append(source);
                        }
                    });
                    browser::Preview { tx }
                },
                hovered_entry: None,
            },
            themes: ThemeColors::default(),
        }
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        TopBottomPanel::top("navbar").frame(egui::Frame::default().fill(self.themes.navbar)).show(ctx, |ui| {
            ui.add(navbar());
        });
        SidePanel::left("sidebar").default_width(300.).frame(egui::Frame::default().fill(self.themes.browser)).show(ctx, |ui| {
            ui.add(self.browser.widget(ctx, &self.themes));
        });
        CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::from_hex("#1e222f").unwrap_or_default()))
            .show(ctx, |ui| {
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

trait ResponseFlatten {
    fn flatten(self) -> Response;
}

impl<R> ResponseFlatten for CollapsingResponse<R> {
    fn flatten(self) -> Response {
        let Self { header_response, body_response, .. } = self;
        match body_response {
            Some(body_response) => header_response.union(body_response),
            None => header_response,
        }
    }
}

impl<R: ResponseFlatten> ResponseFlatten for InnerResponse<R> {
    fn flatten(self) -> Response {
        let Self { inner, response } = self;
        inner.flatten().union(response)
    }
}

impl ResponseFlatten for InnerResponse<Response> {
    fn flatten(self) -> Response {
        let Self { inner, response } = self;
        inner.union(response)
    }
}

trait TryResponseFlatten {
    type Flattened: Try;
    fn try_flatten(self) -> Self::Flattened;
}

impl<R, I> TryResponseFlatten for I
where
    I: IntoIterator<Item = R>,
    R: ResponseIteratorItem,
{
    type Flattened = Option<Response>;
    fn try_flatten(self) -> Self::Flattened {
        self.into_iter().map(ResponseIteratorItem::flatten).reduce(|a, b| a.union(b))
    }
}

trait ResponseIteratorItem {
    fn flatten(self) -> Response;
}
impl ResponseIteratorItem for Response {
    fn flatten(self) -> Response {
        self
    }
}
impl<R: ResponseFlatten> ResponseIteratorItem for R {
    fn flatten(self) -> Response {
        self.flatten()
    }
}
