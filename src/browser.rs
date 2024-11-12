use itertools::Itertools;
use open::that_detached;
use std::{
    fs::{read_dir, DirEntry, File},
    iter::Iterator,
    mem::{transmute_copy, ManuallyDrop, MaybeUninit},
    path::{Path, PathBuf},
    string::ToString,
    thread::JoinHandle,
};
use strum::Display;

use egui::{
    include_image, vec2, Button, CollapsingHeader, CollapsingResponse, Context, CursorIcon, DroppedFile, FontFamily, FontId, Id, Image, InnerResponse, Margin, PointerButton, RichText, ScrollArea,
    Sense, Stroke, Ui, Widget,
};

// FIXME: Temporary rodio playback, might need to use cpal or make rodio proper
use rodio::{Decoder, OutputStream, Sink};

use std::io::BufReader;

use crate::visual::ThemeColors;

// https://veykril.github.io/tlborm/decl-macros/building-blocks/counting.html#bit-twiddling
macro_rules! count_tts {
    () => { 0 };
    ($odd:tt $($a:tt $b:tt)*) => { (count_tts!($($a)*) << 1) | 1 };
    ($($a:tt $even:tt)*) => { count_tts!($($a)*) << 1 };
}

macro_rules! enum_with_array {
    {
        #[derive($($derives:ident),*)]
        pub enum $name:ident
        {
            $($variants:ident),*
            $(,)?
        }
    } => {
        #[derive($($derives),*)]
        pub enum $name {
            $($variants,)*
        }
        impl $name {
            pub const VARIANTS: [$name; count_tts!($($variants)*)] = [$($name::$variants),*];
        }
    };
}

enum_with_array! {
    #[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Category {
        Files,
        Devices,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub indent: usize,
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Directory,
    Audio,
    File,
}

impl From<&DirEntry> for EntryKind {
    fn from(value: &DirEntry) -> Self {
        if value.path().is_dir() {
            Self::Directory
        } else if value
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| AUDIO_EXTENSIONS.contains(&extension))
        {
            Self::Audio
        } else {
            Self::File
        }
    }
}

pub struct Preview {
    pub preview_thread: Option<JoinHandle<()>>,
}

impl Preview {
    pub fn play_file(&mut self, file: File) {
        // Kill the current thread if it's not sleeping
        if let Some(thread) = self.preview_thread.take() {
            if !thread.is_finished() {
                thread.thread().unpark();
            }
        }

        let file = BufReader::new(file);
        self.preview_thread = Some(std::thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let source = Decoder::new(file).unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();
            // let source = SineWave::new(440.0).take_duration(Duration::from_secs_f32(0.25)).amplify(0.20);
            sink.append(source);
            std::thread::park();
        }));
    }
}

pub struct Browser {
    pub selected_category: Category,
    pub other_category_hovered: bool,
    pub open_folders: Vec<PathBuf>,
    pub preview: Preview,
    pub hovered_entry: Option<PathBuf>,
}

impl Browser {
    pub fn paint_button<'a>(selected: bool, text: &'a str, theme: &'a ThemeColors, hovered: bool) -> impl Widget + use<'a> {
        move |ui: &mut Ui| {
            let color = if selected {
                theme.browser_selected_button_fg
            } else if hovered {
                theme.browser_unselected_hover_button_fg
            } else {
                theme.browser_unselected_button_fg
            };
            let InnerResponse { inner, response } = ui.allocate_ui(vec2(0., 24.), |ui| {
                let InnerResponse { inner, response } = ui.centered_and_justified(|ui| {
                    Button::new(RichText::new(text).font(FontId::new(14.0, FontFamily::Name("IBMPlexMono".into()))).color(color))
                        .frame(false)
                        .ui(ui)
                });
                {
                    let (response, painter) = ui.allocate_painter(vec2(0., 0.5), Sense::hover());
                    painter.hline(response.rect.x_range(), response.rect.bottom(), Stroke::new(0.5, color));
                }
                response.union(inner)
            });
            response.union(inner)
        }
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub fn paint(&mut self, ctx: &Context, ui: &mut Ui, theme: &ThemeColors) {
        let (was_pressed, press_position) = ctx
            .input(|input_state| Some((input_state.pointer.button_released(PointerButton::Primary), Some(input_state.pointer.latest_pos()?))))
            .unwrap_or_default();
        ScrollArea::vertical().show_viewport(ui, |ui, _| {
            egui::Frame::none().inner_margin(Margin::same(8.)).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.;
                        ui.columns_const(|uis| {
                            // https://internals.rust-lang.org/t/should-there-by-an-array-zip-method/21611/5
                            fn zip<T, U, const N: usize>(ts: [T; N], us: [U; N]) -> [(T, U); N] {
                                let mut ts = ts.map(ManuallyDrop::new);
                                let mut us = us.map(ManuallyDrop::new);
                                let mut zip = [const { MaybeUninit::<(T, U)>::uninit() }; N];
                                for i in 0..N {
                                    // SAFETY: ts[i] taken once, untouched afterwards
                                    let t = unsafe { ManuallyDrop::take(&mut ts[i]) };
                                    // SAFETY: us[i] taken once, untouched afterwards
                                    let u = unsafe { ManuallyDrop::take(&mut us[i]) };
                                    zip[i].write((t, u));
                                }
                                // SAFETY: zip has been fully initialized
                                unsafe { transmute_copy(&zip) }
                            }
                            for (category, ui) in zip(Category::VARIANTS, uis.each_mut()) {
                                let selected = self.selected_category == category;
                                let string = category.to_string();
                                let button = Self::paint_button(selected, &string, theme, self.other_category_hovered);
                                let response = ui.add(button);
                                let rect = response.rect;
                                if !selected {
                                    self.other_category_hovered = response.hovered();
                                    response.on_hover_cursor(CursorIcon::PointingHand);
                                }
                                if press_position.is_some_and(|press_position| was_pressed && rect.contains(press_position)) {
                                    self.selected_category = category;
                                }
                            }
                        });
                    });
                    match self.selected_category {
                        Category::Files => {
                            self.paint_files(ctx, ui, theme);
                        }
                        Category::Devices => {
                            // TODO: Show some devices here!
                        }
                    }
                });
            });
        });
    }

    fn paint_files(&mut self, ctx: &Context, ui: &mut Ui, theme: &ThemeColors) {
        self.handle_folder_drop(ctx);
        egui::Frame::default().inner_margin(Margin::same(8.)).show(ui, |ui| {
            ui.vertical(|ui| {
                let Self {
                    open_folders, preview, hovered_entry, ..
                } = self;
                for folder in open_folders {
                    let name = folder
                        .file_name()
                        .map_or(folder.to_str(), |name| name.to_str())
                        .map(ToString::to_string)
                        .ok_or_else(|| String::from_utf8_lossy(folder.file_name().unwrap().as_encoded_bytes()).to_string());
                    let unwrapped = match &name {
                        Ok(name) | Err(name) => name,
                    }
                    .as_str();
                    CollapsingHeader::new(unwrapped).id_salt(&folder).show(ui, |ui| {
                        let mut some_hovered = false;
                        Self::directory_inner(folder, ui, preview, theme, hovered_entry, &mut some_hovered);
                        if !some_hovered {
                            *hovered_entry = None;
                        }
                    });
                }
            });
        });
    }

    #[allow(clippy::too_many_lines)]
    fn directory_inner(folder: &Path, ui: &mut Ui, preview: &mut Preview, theme: &ThemeColors, hovered_entry: &mut Option<PathBuf>, some_hovered: &mut bool) {
        for entry in read_dir(folder).unwrap().sorted_by(|a, b| {
            a.as_ref()
                .ok()
                .map(EntryKind::from)
                .cmp(&b.as_ref().ok().map(EntryKind::from))
                .then_with(|| a.as_ref().map(DirEntry::path).ok().cmp(&b.as_ref().map(DirEntry::path).ok()))
        }) {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = EntryKind::from(&entry);
            let name = path
                .file_name()
                .map_or(path.to_str(), |name| name.to_str())
                .map(ToString::to_string)
                .ok_or_else(|| String::from_utf8_lossy(path.file_name().unwrap().as_encoded_bytes()).to_string());
            let button = |hovered_entry: &mut Option<PathBuf>| {
                Button::new(
                    RichText::new(match &name {
                        Ok(name) | Err(name) => name,
                    })
                    .font(FontId::new(14., FontFamily::Name("IBMPlexMono".into())))
                    .color(match (Some(&path) == hovered_entry.as_ref(), name.is_err()) {
                        (true, true) => theme.browser_unselected_hover_button_fg_invalid,
                        (true, false) => theme.browser_unselected_hover_button_fg,
                        (false, true) => theme.browser_unselected_button_fg_invalid,
                        (false, false) => theme.browser_unselected_button_fg,
                    }),
                )
                .frame(false)
            };
            let response = match kind {
                EntryKind::Audio => {
                    let mut add_contents = |ui: &mut Ui| {
                        ui.horizontal(|ui| {
                            ui.add(Image::new(include_image!("images/icons/audio.png")));
                            ui.add(button(hovered_entry))
                        })
                    };
                    if ui.input(|input| input.modifiers.command) {
                        let InnerResponse {
                            inner: InnerResponse { inner, response: a },
                            response: b,
                        } = ui.dnd_drag_source(Id::new((&path, 0)), (), add_contents);
                        inner.union(a).union(b)
                    } else {
                        let InnerResponse { inner, response } = add_contents(ui);
                        inner.union(response)
                    }
                }
                EntryKind::File => {
                    let InnerResponse { inner, response } = ui.horizontal(|ui| {
                        ui.add(Image::new(include_image!("images/icons/file.png")));
                        ui.add(button(hovered_entry))
                    });
                    inner.union(response)
                }
                EntryKind::Directory => {
                    let CollapsingResponse { header_response, body_response, .. } = CollapsingHeader::new(match &name {
                        Ok(name) | Err(name) => name,
                    })
                    .id_salt(&path)
                    .show(ui, |ui| {
                        Self::directory_inner(&path, ui, preview, theme, hovered_entry, some_hovered);
                    });
                    match body_response {
                        Some(body_response) => header_response.union(body_response),
                        None => header_response,
                    }
                }
            };
            if response.clicked() {
                match kind {
                    EntryKind::Audio => {
                        // TODO: Proper preview implementation with cpal. This is temporary (or at least make it work well with a proper preview widget)
                        // Also, don't spawn a new thread - instead, dedicate a thread for preview
                        let file = File::open(path.as_path()).unwrap();
                        preview.play_file(file);
                    }
                    EntryKind::File => {
                        that_detached(path.clone()).unwrap();
                    }
                    EntryKind::Directory => {}
                }
            }
            if response.hovered() {
                *some_hovered = true;
                *hovered_entry = Some(path);
            }
        }
    }

    fn handle_folder_drop(&mut self, ctx: &Context) {
        // Handle folder drop
        // TODO: Enable drag and drop on Windows
        // https://docs.rs/egui/latest/egui/struct.RawInput.html#structfield.dropped_files
        let files: Vec<_> = ctx
            .input(|input| input.raw.dropped_files.iter().map(move |DroppedFile { path, .. }| path.clone().ok_or(())).try_collect())
            .unwrap_or_default();
        for path in files {
            self.open_folders.push(path);
        }
    }
}

const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "wave", "mp3", "ogg", "flac", "opus"];
