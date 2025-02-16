#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use std::{
    io::{BufReader, Cursor},
    rc::Rc, time::Duration,
};

use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{hex_color, vec2, CentralPanel, Context, FontData, FontDefinitions, FontFamily, FontId, IconData, RichText, Shadow, SidePanel, TextStyle, TopBottomPanel, ViewportBuilder};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use image::{ImageFormat, ImageReader};
use info::handle_args;
// TODO: Move everything into components (visual)
mod info;
mod visual;
mod timings;

use tap::{Pipe, Tap};
use visual::{browser::Browser, central::Central, navbar::navbar, notification::NotificationDrawer, ThemeColors};

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
    pub notification_drawer: NotificationDrawer,
    pub theme: Rc<ThemeColors>,
    pub showing_command_palette: bool,
    pub command_palette_text: String,
    pub command_palette_cursor_pos: u32,
    pub command_palette_cursor_pos_end: u32,
    pub command_palette_begin: Duration,
    pub timings_toggle: bool
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        const FONT_NAME: &str = "IBMPlexMono";
        install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_fonts({
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert(FONT_NAME.to_string(), FontData::from_static(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")).into());
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
        let theme = Rc::new(ThemeColors::default());
        Self {
            browser: Browser::new(Rc::clone(&theme)),
            central: Central::new(),
            notification_drawer: NotificationDrawer::new(),
            theme,
            showing_command_palette: false,
            command_palette_text: String::new(),
            command_palette_begin: Duration::default(),
            command_palette_cursor_pos: 0,
            command_palette_cursor_pos_end: 0,
            timings_toggle: false
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

impl App for VoltApp {
    #[allow(clippy::too_many_lines, reason = "shut")]
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let time_render_start = timings::now_ns();
        // TODO: Move this (the command palette) to its own file. This is here primarily for testing purposes.

        // Keyboard shortcut handler
        if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P))) {
            if !self.showing_command_palette {
                self.command_palette_begin = Duration::from_secs_f64(now());
            }
            self.showing_command_palette = !self.showing_command_palette;
        }

        // Handle queries
        if ctx.input_mut(|i| i.key_pressed(egui::Key::Enter)) {
            self.showing_command_palette = false;
            // TODO: Replace this with a search query implementation rather than direct matching (after moving to palette.rs).
            match self.command_palette_text.as_str() {
                "timings" => {
                    self.timings_toggle = !self.timings_toggle;
                }
                "info" => {
                    info::dump();
                    self.notification_drawer.make("Dumped system info into console!".into(), Some(Duration::from_secs(5)));
                }
                _ => {}
            }
        }

        // Reset the command palette input
        if !self.showing_command_palette && !self.command_palette_text.is_empty() {
            self.command_palette_cursor_pos = 0;
            self.command_palette_cursor_pos_end = 0;
            self.command_palette_text.clear();
        }

        // Render the command palette and handle logic
        if self.showing_command_palette {
            // Escaping the command palette
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.showing_command_palette = false;
                ctx.request_repaint();
            } else {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("command_palette")));
                let screen_rect = ctx.screen_rect();
                let palette_size = egui::vec2(300.0, 30.0);
                let mut center_top = screen_rect.center_top();
                center_top.y += 40.;
                let palette_rect = egui::Rect::from_center_size(center_top, palette_size);

                painter.add(Shadow {
                    spread: 0.0,
                    blur: 14.0,
                    offset: vec2(0., 4.),
                    color: egui::Color32::from_black_alpha(200),
                }.as_shape(palette_rect, 8.0));

                painter.rect_filled(palette_rect, 8.0, self.theme.command_palette);
                painter.rect_stroke(palette_rect, 8.0, (1.0, self.theme.command_palette_border));

                let palette_text_fontid = FontId::new(12., FontFamily::Monospace);
                #[allow(clippy::cast_precision_loss, reason = "shut")]
                #[allow(clippy::cast_possible_truncation, reason = "shut")]
                if let Some(text) = ctx.input_mut(|i| {
                    i.events.iter().find_map(|event| match event {
                        egui::Event::Text(text) => Some(text.clone()),
                        _ => None,
                    })
                }) {
                    if self.command_palette_cursor_pos == self.command_palette_cursor_pos_end {
                        self.command_palette_text.insert_str(self.command_palette_cursor_pos as usize, &text);
                        self.command_palette_cursor_pos += 1;
                    } else {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end) as usize;
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end) as usize;
                        self.command_palette_text.replace_range(start..end, &text);
                        self.command_palette_cursor_pos = (start as u32) + 1;
                    }
                    self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::Backspace)) && !self.command_palette_text.is_empty() {
                    if self.command_palette_cursor_pos != self.command_palette_cursor_pos_end {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end) as usize;
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end) as usize;
                        self.command_palette_text.replace_range(start..end, "");
                        self.command_palette_cursor_pos = start as u32;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else if self.command_palette_cursor_pos > 0 {
                        self.command_palette_text.remove(self.command_palette_cursor_pos as usize - 1);
                        self.command_palette_cursor_pos -= 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    if ctx.input_mut(|i| i.modifiers.shift) {
                        if self.command_palette_cursor_pos > 0 {
                            self.command_palette_cursor_pos -= 1;
                        }
                    } else if self.command_palette_cursor_pos > 0 {
                        self.command_palette_cursor_pos -= 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    if ctx.input_mut(|i| i.modifiers.shift) {
                        if (self.command_palette_cursor_pos as usize) < self.command_palette_text.len() {
                            self.command_palette_cursor_pos += 1;
                        }
                    } else if (self.command_palette_cursor_pos as usize) < self.command_palette_text.len() {
                        self.command_palette_cursor_pos += 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft)) {
                    let text_before = &self.command_palette_text[..(self.command_palette_cursor_pos as usize)];
                    self.command_palette_cursor_pos = text_before.rfind(|c: char| !c.is_alphanumeric())
                        .map(|i| i as u32 + 1)
                        .unwrap_or(0);
                    if !ctx.input_mut(|i| i.modifiers.shift) {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight)) {
                    let text_after = &self.command_palette_text[(self.command_palette_cursor_pos as usize)..];
                    if let Some(i) = text_after.find(|c: char| !c.is_alphanumeric()) {
                        self.command_palette_cursor_pos = (self.command_palette_cursor_pos as usize + i) as u32;
                    } else {
                        self.command_palette_cursor_pos = self.command_palette_text.len() as u32;
                    }
                    if !ctx.input_mut(|i| i.modifiers.shift) {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Backspace)) {
                    let text_before = &self.command_palette_text[..(self.command_palette_cursor_pos as usize)];
                    let prev_word_end = text_before.rfind(|c: char| !c.is_alphanumeric())
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    self.command_palette_text.drain(prev_word_end..self.command_palette_cursor_pos as usize);
                    self.command_palette_cursor_pos = prev_word_end as u32;
                    self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::Delete)) {
                    if self.command_palette_cursor_pos != self.command_palette_cursor_pos_end {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end) as usize;
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end) as usize;
                        self.command_palette_text.replace_range(start..end, "");
                        self.command_palette_cursor_pos = start as u32;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else if (self.command_palette_cursor_pos as usize) < self.command_palette_text.len() {
                        self.command_palette_text.remove(self.command_palette_cursor_pos as usize);
                    }
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Delete)) && (self.command_palette_cursor_pos as usize) < self.command_palette_text.len() {
                    let text_after = &self.command_palette_text[(self.command_palette_cursor_pos as usize)..];
                    let next_word_start = text_after.find(|c: char| !c.is_alphanumeric())
                        .map(|i| (self.command_palette_cursor_pos as usize) + i)
                        .unwrap_or(self.command_palette_text.len());
                    self.command_palette_text.drain(self.command_palette_cursor_pos as usize..next_word_start);
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.shift && i.key_pressed(egui::Key::Delete)) {
                    self.command_palette_text.clear();
                    self.command_palette_cursor_pos = 0;
                    self.command_palette_cursor_pos_end = 0;
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
                    self.command_palette_cursor_pos = self.command_palette_text.len() as u32;
                    self.command_palette_cursor_pos_end = 0;
                    self.command_palette_begin = Duration::from_secs_f64(now());
                }

                let cptext_x_offset = 10.;
                let cursor_width = 2.;

                if self.command_palette_text.is_empty() {
                    painter.text(
                        {
                            let mut lc = palette_rect.left_center();
                            lc.x += cptext_x_offset;
                            lc
                        },
                        egui::Align2::LEFT_CENTER,
                        "Type a command...",
                        palette_text_fontid.clone(),
                        self.theme.command_palette_placeholder_text,
                    );
                    // Draw cursor
                    let cursor_pos = painter.text(
                        {
                            let mut lc = palette_rect.left_center();
                            lc.x += cptext_x_offset;
                            lc
                        },
                        egui::Align2::LEFT_CENTER,
                        &self.command_palette_text[..self.command_palette_cursor_pos as usize],
                        palette_text_fontid,
                        self.theme.command_palette_text,
                    ).right();
                    // Only show cursor every 500ms
                    if (now() - self.command_palette_begin.as_secs_f64()).fract() < 0.5 {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(cursor_pos, palette_rect.center().y - 8.),
                                egui::pos2(cursor_pos + cursor_width, palette_rect.center().y + 8.),
                            ),
                            0.0,
                            egui::Color32::from_rgb(0x5c, 0x5c, 0xff),
                        );
                    }
                } else {
                    let (start_pos, end_pos) = if self.command_palette_cursor_pos < self.command_palette_cursor_pos_end {
                        (self.command_palette_cursor_pos, self.command_palette_cursor_pos_end)
                    } else {
                        (self.command_palette_cursor_pos_end, self.command_palette_cursor_pos)
                    };

                    // Draw text before selection
                    let selection_start = painter.text(
                        {
                            let mut lc = palette_rect.left_center();
                            lc.x += cptext_x_offset;
                            lc
                        },
                        egui::Align2::LEFT_CENTER,
                        &self.command_palette_text[..start_pos as usize],
                        palette_text_fontid.clone(),
                        self.theme.command_palette_text,
                    ).right();

                    // Draw selection
                    let selection_end = painter.text(
                        egui::pos2(selection_start, palette_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &self.command_palette_text[start_pos as usize..end_pos as usize],
                        palette_text_fontid.clone(),
                        hex_color!("8c8cff"),
                    ).right();

                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(selection_start, palette_rect.center().y - 8.),
                            egui::pos2(selection_end, palette_rect.center().y + 8.),
                        ),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(0x5c, 0x5c, 0xff, 0x20),
                    );

                    // Draw text after selection
                    painter.text(
                        egui::pos2(selection_end, palette_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &self.command_palette_text[end_pos as usize..],
                        palette_text_fontid,
                        self.theme.command_palette_text,
                    );

                    // Only show cursor every 500ms
                    if (now() - self.command_palette_begin.as_secs_f64()).fract() < 0.5 {
                        let cursor_pos = if self.command_palette_cursor_pos <= self.command_palette_cursor_pos_end {
                            selection_start
                        } else {
                            selection_end
                        };

                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(cursor_pos, palette_rect.center().y - 8.),
                                egui::pos2(cursor_pos + cursor_width, palette_rect.center().y + 8.),
                            ),
                            0.0,
                            egui::Color32::from_rgb(0x5c, 0x5c, 0xff),
                        );
                    }
                }

                ctx.request_repaint_after_secs(0.1);
            }
        }

        TopBottomPanel::top("navbar").frame(egui::Frame::default()).show(ctx, |ui| {
            ui.add(navbar(&self.theme));
        });
        TopBottomPanel::bottom("status").frame(egui::Frame::default()).show(ctx, |ui| {
            ui.label("bagel");
        });
        SidePanel::left("browser").default_width(300.).frame(egui::Frame::default().fill(self.theme.browser)).show(ctx, |ui| {
            ui.add(&mut self.browser);
        });
        CentralPanel::default().frame(egui::Frame::default().fill(self.theme.central_background)).show(ctx, |ui| {
            ui.add(&mut self.central);
        });

        egui::Area::new("notifications_area".into())
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::new(ctx.screen_rect().max.x, ctx.screen_rect().max.y))
            .show(ctx, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin::same(0.0),
                    outer_margin: egui::Margin::same(0.0),
                    rounding: egui::Rounding::same(0.0),
                    shadow: Shadow::NONE,
                    fill: egui::Color32::TRANSPARENT,
                    stroke: egui::Stroke::NONE,
                }
                .show(ui, |ui| {
                    ui.add(&mut self.notification_drawer);
                });
            });
        let time_render_end = timings::now_ns();
        let time_render_elapsed = time_render_end - time_render_start;
        timings::set_render_time(time_render_elapsed);

        if self.timings_toggle {
            timings::show_timings(ctx, "Timings", 4);
        }
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
