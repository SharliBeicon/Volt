use itertools::Itertools;
use open::that_detached;
use std::{
    fs::{read_dir, DirEntry},
    iter::Iterator,
    mem::{transmute_copy, ManuallyDrop, MaybeUninit},
    path::{Path, PathBuf},
    string::ToString,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use strum::Display;
use tap::Pipe;

use egui::{include_image, vec2, Button, CollapsingHeader, Context, CursorIcon, DroppedFile, Id, Image, Margin, PointerButton, Response, RichText, ScrollArea, Sense, Stroke, Ui, Widget};

use crate::{visual::ThemeColors, ResponseFlatten, TryResponseFlatten};

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
    pub path: Option<PathBuf>,
    pub path_tx: Sender<PathBuf>,
    pub file_data_rx: Receiver<PreviewData>,
    pub file_data: Option<PreviewData>,
}

impl Preview {
    pub fn play_file(&mut self, path: PathBuf) {
        self.path = Some(path.clone());
        self.path_tx.send(path).unwrap();
        self.file_data = None;
    }

    pub fn data(&mut self) -> Option<PreviewData> {
        self.file_data = match self.file_data_rx.try_recv() {
            Ok(data) => Some(data),
            Err(_) => self.file_data,
        };
        if self.file_data.is_some_and(|data| data.length.is_some_and(|length| data.progress() > length)) {
            self.path = None;
            self.file_data = None;
        }
        self.file_data
    }
}

#[derive(Clone, Copy)]
pub struct PreviewData {
    pub length: Option<Duration>,
    pub started_playing: Instant,
}

impl PreviewData {
    fn progress(&self) -> Duration {
        self.started_playing.elapsed()
    }

    fn remaining(&self) -> Option<Duration> {
        self.length.map(|length| length - self.progress())
    }

    fn percentage(&self) -> Option<f32> {
        self.length.map(|length| self.progress().as_secs_f32() / length.as_secs_f32())
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
    pub fn button<'a>(selected: bool, text: &'a str, theme: &'a ThemeColors, hovered: bool) -> impl Widget + use<'a> {
        move |ui: &mut Ui| {
            let color = if selected {
                theme.browser_selected_button_fg
            } else if hovered {
                theme.browser_unselected_hover_button_fg
            } else {
                theme.browser_unselected_button_fg
            };
            ui.allocate_ui(vec2(0., 24.), |ui| {
                let response = ui.centered_and_justified(|ui| Button::new(RichText::new(text).size(14.).color(color)).frame(false).ui(ui));
                response.flatten().union({
                    let (response, painter) = ui.allocate_painter(vec2(0., 0.5), Sense::hover());
                    painter.hline(response.rect.x_range(), response.rect.bottom(), Stroke::new(0.5, color));
                    response
                })
            })
            .flatten()
        }
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub fn widget<'a>(&'a mut self, ctx: &'a Context, theme: &'a ThemeColors) -> impl Widget + use<'a> + 'a {
        move |ui: &mut Ui| {
            let (was_pressed, press_position) = ctx
                .input(|input_state| Some((input_state.pointer.button_released(PointerButton::Primary), Some(input_state.pointer.latest_pos()?))))
                .unwrap_or_default();
            ScrollArea::vertical()
                .show_viewport(ui, |ui, _| {
                    egui::Frame::default().inner_margin(Margin::same(8.)).show(ui, |ui| {
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
                                    zip(Category::VARIANTS, uis.each_mut())
                                        .map(|(category, ui)| {
                                            let selected = self.selected_category == category;
                                            let string = category.to_string();
                                            let button = Self::button(selected, &string, theme, self.other_category_hovered);
                                            let mut response = ui.add(button);
                                            let rect = response.rect;
                                            if !selected {
                                                self.other_category_hovered = response.hovered();
                                                response = response.on_hover_cursor(CursorIcon::PointingHand);
                                            }
                                            if press_position.is_some_and(|press_position| was_pressed && rect.contains(press_position)) {
                                                self.selected_category = category;
                                            }
                                            response
                                        })
                                        .into_iter()
                                        .reduce(|a, b| a.union(b))
                                        .unwrap()
                                })
                            })
                            .flatten()
                            .union(match self.selected_category {
                                Category::Files => self.add_files(ctx, ui, theme),
                                Category::Devices => {
                                    // TODO: Show some devices here!
                                    ui.label("Devices")
                                }
                            })
                        })
                    })
                })
                .inner
                .flatten()
        }
    }

    fn add_files(&mut self, ctx: &Context, ui: &mut Ui, theme: &ThemeColors) -> Response {
        self.handle_folder_drop(ctx);
        egui::Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let Self {
                        open_folders, preview, hovered_entry, ..
                    } = self;
                    open_folders
                        .iter()
                        .map(|folder| {
                            let name = folder
                                .file_name()
                                .map_or(folder.to_str(), |name| name.to_str())
                                .map(ToString::to_string)
                                .ok_or_else(|| String::from_utf8_lossy(folder.file_name().unwrap().as_encoded_bytes()).to_string());
                            let unwrapped = match &name {
                                Ok(name) | Err(name) => name,
                            }
                            .as_str();
                            CollapsingHeader::new(unwrapped).id_salt(folder).show(ui, |ui| {
                                let mut some_hovered = false;
                                let response = Self::add_directory_contents(folder, ui, preview, theme, hovered_entry, &mut some_hovered);
                                if !some_hovered {
                                    *hovered_entry = None;
                                }
                                response
                            })
                        })
                        .try_flatten()
                })
            })
            .flatten()
    }

    fn add_directory_contents(folder: &Path, ui: &mut Ui, preview: &mut Preview, theme: &ThemeColors, hovered_entry: &mut Option<PathBuf>, some_hovered: &mut bool) -> Option<Response> {
        read_dir(folder)
            .unwrap()
            .sorted_by(|a, b| {
                a.as_ref()
                    .ok()
                    .map(EntryKind::from)
                    .cmp(&b.as_ref().ok().map(EntryKind::from))
                    .then_with(|| a.as_ref().map(DirEntry::path).ok().cmp(&b.as_ref().map(DirEntry::path).ok()))
            })
            .map(|entry| {
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
                                ui.add(Image::new(include_image!("images/icons/audio.png"))).union(ui.add(button(hovered_entry))).pipe(|response| {
                                    let data = preview.data();
                                    if let Some(data @ PreviewData { length: Some(length), .. }) = preview.path.as_ref().filter(|preview_path| preview_path == &&path).zip(data).map(|(_, data)| data) {
                                        response.union(ui.label(format!(
                                            "{:>02}:{:>02} of {:>02}:{:>02}",
                                            data.progress().as_secs() / 60,
                                            data.progress().as_secs() % 60,
                                            length.as_secs() / 60,
                                            length.as_secs() % 60
                                        )))
                                    } else {
                                        response
                                    }
                                })
                            })
                        };
                        if ui.input(|input| input.modifiers.command) {
                            ui.dnd_drag_source(Id::new((&path, 0)), (), add_contents).flatten()
                        } else {
                            add_contents(ui).flatten()
                        }
                    }
                    EntryKind::File => ui
                        .horizontal(|ui| ui.add(Image::new(include_image!("images/icons/file.png"))).union(ui.add(button(hovered_entry))))
                        .flatten(),
                    EntryKind::Directory => {
                        let response = CollapsingHeader::new(match &name {
                            Ok(name) | Err(name) => name,
                        })
                        .id_salt(&path)
                        .show(ui, |ui| {
                            Self::add_directory_contents(&path, ui, preview, theme, hovered_entry, some_hovered);
                        });
                        response.flatten()
                    }
                }
                .on_hover_cursor(CursorIcon::PointingHand);
                if response.clicked() {
                    match kind {
                        EntryKind::Audio => {
                            // TODO: Proper preview implementation with cpal. This is temporary (or at least make it work well with a proper preview widget)
                            // Also, don't spawn a new thread - instead, dedicate a thread for preview
                            preview.play_file(path.clone());
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
                response
            })
            .try_flatten()
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
