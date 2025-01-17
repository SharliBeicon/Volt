#![warn(clippy::clone_on_ref_ptr)]

use blerp::utils::zip;
use itertools::Itertools;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use open::that_detached;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    fs::{read_dir, DirEntry, File},
    io::{self, BufReader},
    iter::Iterator,
    ops::BitOr,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    string::ToString,
    sync::mpsc::{channel, Receiver, Sender, TryRecvError},
    thread::spawn,
    time::{Duration, Instant},
};
use strum::Display;
use tap::Pipe;
use tracing::{error, trace};

use egui::{
    emath::TSTransform, include_image, vec2, Button, CollapsingHeader, Context, CursorIcon, DragAndDrop, DroppedFile, Id, Image, InnerResponse, LayerId, Margin, Order, Response, RichText, ScrollArea,
    Sense, Separator, Ui, UiBuilder, Widget,
};

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

impl<P: AsRef<Path>> From<P> for EntryKind {
    fn from(value: P) -> Self {
        if value.as_ref().is_dir() {
            Self::Directory
        } else if value
            .as_ref()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| AUDIO_EXTENSIONS.into_iter().any(|audio_extension| audio_extension.eq_ignore_ascii_case(extension)))
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
    selected_category: Category,
    other_category_hovered: bool,
    open_paths: Rc<RefCell<Vec<PathBuf>>>,
    preview: Preview,
    hovered_entry: Option<PathBuf>,
    theme: ThemeColors,
    cached_entries: HashMap<PathBuf, CachedEntry>,
    watcher: RecommendedWatcher,
    watcher_rx: Receiver<notify::Result<Event>>,
}

struct CachedEntry {
    rx: Receiver<io::Result<Vec<PathBuf>>>,
    entries: Option<Rc<[PathBuf]>>,
}

impl Browser {
    pub fn new(themes: ThemeColors) -> Self {
        let (watcher_tx, watcher_rx) = channel();
        let watcher = recommended_watcher(watcher_tx).unwrap();
        Self {
            selected_category: Category::Files,
            other_category_hovered: false,
            open_paths: Rc::new(RefCell::new(vec![PathBuf::from_str("/").unwrap()])),
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
            theme: themes,
            cached_entries: HashMap::new(),
            watcher,
            watcher_rx,
        }
    }

    pub fn button<'a>(theme: &'a ThemeColors, selected: bool, text: &'a str, hovered: bool) -> impl Widget + use<'a> {
        move |ui: &mut Ui| {
            let color = if selected {
                theme.browser_selected_button_fg
            } else if hovered {
                theme.browser_unselected_hover_button_fg
            } else {
                theme.browser_unselected_button_fg
            };
            ui.allocate_ui(vec2(0., 24.), |ui| {
                let button = ui.centered_and_justified(|ui| Button::new(RichText::new(text).size(14.).color(color)).frame(false).ui(ui)).inner;
                ui.visuals_mut().widgets.noninteractive.bg_stroke.color = color;
                ui.add(Separator::default().spacing(0.));
                button
            })
            .inner
        }
    }

    fn add_files(&mut self, ui: &mut Ui) -> Response {
        self.handle_file_or_folder_drop(ui.ctx());
        egui::Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| {
                ui.vertical(|ui| Rc::clone(&self.open_paths).borrow().iter().map(|path| self.add_entry(path, ui)).reduce(Response::bitor))
            })
            .response
    }

    fn add_entry(&mut self, path: &Path, ui: &mut Ui) -> Response {
        let widget_pos_y = ui.next_widget_position().y;
        let clip_max = ui.clip_rect().max.y;
        let clip_min = ui.clip_rect().min.y;

        if widget_pos_y >= clip_max {
            ui.add_space(24.0);
            return ui.allocate_response(vec2(0.0, 24.0), Sense::hover());
        }
        // fix this later
        // if widget_pos_y + 24. + 100. <= clip_min {
        //     println!("{:?}", path);
        // }
        let kind = EntryKind::from(path);
        let name = path.file_name().map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy());
        let button = |hovered_entry: &Option<PathBuf>| {
            Button::new(RichText::new(&*name).color(match (Some(path) == hovered_entry.as_deref(), matches!(&name, &Cow::Owned(_))) {
                (true, true) => self.theme.browser_unselected_hover_button_fg_invalid,
                (true, false) => self.theme.browser_unselected_hover_button_fg,
                (false, true) => self.theme.browser_unselected_button_fg_invalid,
                (false, false) => self.theme.browser_unselected_button_fg,
            }))
            .frame(false)
        };
        let response = match kind {
            EntryKind::Audio => {
                let mut add_contents = |ui: &mut Ui| {
                    ui.horizontal(|ui| {
                        ui.add(Image::new(include_image!("../images/icons/audio.png")))
                            .union(ui.add(button(&self.hovered_entry)))
                            .pipe(|response| {
                                let data = self.preview.data();
                                if let Some(data @ PreviewData { length: Some(length), .. }) = self.preview.path.as_ref().filter(|preview_path| preview_path == &path).zip(data).map(|(_, data)| data) {
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
                let mut response = if ui.ctx().is_being_dragged(Id::new(path)) {
                    DragAndDrop::set_payload(ui.ctx(), path.to_path_buf());
                    let layer_id = LayerId::new(Order::Tooltip, Id::new(path));
                    let response = ui.scope_builder(UiBuilder::new().layer_id(layer_id), add_contents).response;
                    if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                        let delta = pointer_pos - response.rect.center();
                        ui.ctx().transform_layer_shapes(layer_id, TSTransform::from_translation(delta));
                    }
                    response
                } else {
                    let response = ui.scope(&mut add_contents).response;
                    let dnd_response = ui.interact(response.rect, Id::new(path), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Grab);
                    dnd_response | response
                };
                response.layer_id = ui.layer_id();
                response
            }
            EntryKind::File => Self::add_file(ui, button(&self.hovered_entry)),
            EntryKind::Directory => {
                let response = CollapsingHeader::new(name).id_salt(path).show(ui, |ui| {
                    self.add_directory_contents(path, ui);
                });
                response.header_response
            }
        };
        let response = response.on_hover_cursor(CursorIcon::PointingHand);
        if response.clicked() {
            match kind {
                EntryKind::Audio => {
                    // TODO: Proper preview implementation with cpal. This is temporary (or at least make it work well with a proper preview widget)
                    // Also, don't spawn a new thread - instead, dedicate a thread for preview
                    self.preview.play_file(path.to_path_buf());
                }
                EntryKind::File => {
                    that_detached(path).unwrap();
                }
                EntryKind::Directory => {}
            }
        }
        if response.hovered() {
            self.hovered_entry = Some(path.to_path_buf());
        }
        response
    }

    fn add_directory_contents(&mut self, path: &Path, ui: &mut Ui) -> Option<Response> {
        match self.watcher_rx.try_recv() {
            Ok(event) => {
                let event = event.unwrap();
                match event.kind {
                    EventKind::Access(_) => {}
                    _ => {
                        for path in event.paths.iter().map(|path| if path.is_dir() { path } else { path.parent().unwrap() }) {
                            self.cached_entries.remove(path);
                        }
                    }
                }
            }
            Err(TryRecvError::Disconnected) => {
                panic!()
            }
            Err(TryRecvError::Empty) => {}
        }
        let CachedEntry { rx, entries } = self.cached_entries.entry(path.to_path_buf()).or_insert_with(|| {
            trace!("entry cache miss for {:?}", path);
            if let Err(error) = self.watcher.watch(path, RecursiveMode::NonRecursive) {
                error!("Unexpected error while trying to watch directory: {:?}", error);
            }
            let (tx, rx) = channel();
            let read_dir = read_dir(path);
            spawn(move || {
                tx.send(read_dir.and_then(|entries| {
                    entries
                        .map(|entry| {
                            {
                                match entry {
                                    Ok(ref x) => Ok(x),
                                    Err(x) => Err(x),
                                }
                            }
                            .map(DirEntry::path)
                        })
                        .sorted_unstable_by(|a, b| {
                            let a = a.as_ref().ok();
                            let b = b.as_ref().ok();
                            Ord::cmp(&(a.map(EntryKind::from), a), &(b.map(EntryKind::from), b))
                        })
                        .try_collect()
                }))
                .unwrap();
            });
            CachedEntry { rx, entries: None }
        });
        if let Some(entries) = entries {
            return Rc::clone(entries).iter().map(|path| self.add_entry(path, ui)).reduce(Response::bitor);
        }
        match rx.try_recv() {
            Ok(Ok(recv_entries)) => {
                *entries = Some(recv_entries.into());
                Some(ui.label("Loading files..."))
            }
            Ok(Err(error)) => {
                error!("Unexpected error while adding directory contents to browser: {:?}", error);
                // TODO better error handling (probably blocked by proper notification system)
                Some(ui.label("Error loading files. Check the standard error stream."))
            }
            Err(TryRecvError::Disconnected) => {
                // TODO handle this error better (the thread panicked)
                panic!("Directory contents were not loaded before the channel disconnected");
            }
            Err(TryRecvError::Empty) => Some(ui.label("Loading files...")),
        }
    }

    fn handle_file_or_folder_drop(&self, ctx: &Context) {
        let files: Vec<_> = ctx
            .input(|input| input.raw.dropped_files.iter().map(move |DroppedFile { path, .. }| path.clone().ok_or(())).try_collect())
            .unwrap_or_default();
        for path in files {
            self.open_paths.borrow_mut().push(path);
        }
    }

    fn add_file(ui: &mut Ui, button: Button<'_>) -> Response {
        let InnerResponse { inner, response } = ui.horizontal(|ui| ui.add(Image::new(include_image!("../images/icons/file.png"))).union(ui.add(button)));
        inner | response
    }
}

impl Widget for &mut Browser {
    fn ui(self, ui: &mut Ui) -> Response {
        ScrollArea::both()
            .drag_to_scroll(false)
            .auto_shrink(false)
            .show_viewport(ui, |ui, _| {
                egui::Frame::default().inner_margin(Margin::same(8.)).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.;
                            ui.columns_const(|uis| {
                                zip(Category::VARIANTS, uis.each_mut())
                                    .map(|(category, ui)| {
                                        let selected = self.selected_category == category;
                                        let string = category.to_string();
                                        let mut response = ui.add(Browser::button(&self.theme, selected, &string, self.other_category_hovered));
                                        if !selected {
                                            response = response.on_hover_cursor(CursorIcon::PointingHand);
                                            self.other_category_hovered = response.hovered();
                                        }
                                        if response.clicked() {
                                            self.selected_category = category;
                                        }
                                        response
                                    })
                                    .into_iter()
                                    .reduce(Response::bitor)
                                    .unwrap()
                            })
                        })
                        .response
                        .union(match self.selected_category {
                            Category::Files => self.add_files(ui),
                            Category::Devices => {
                                // TODO: Show some devices here!
                                ui.label("Devices")
                            }
                        })
                    })
                })
            })
            .inner
            .response
    }
}

const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "wave", "mp3", "ogg", "flac", "opus"];
