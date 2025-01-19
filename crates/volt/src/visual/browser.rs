use blerp::utils::zip;
use dashmap::DashMap;
use itertools::Itertools;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use open::that_detached;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    f32::consts::PI,
    fs::{read_dir, DirEntry, File},
    io::{self, BufReader},
    iter::Iterator,
    ops::BitOr,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    string::ToString,
    sync::{mpsc::{channel, Receiver, Sender, TryRecvError}, RwLock},
    thread::spawn,
    time::{Duration, Instant},
};
use strum::Display;
use tap::Pipe;
use tracing::{error, trace};

use egui::{
    emath::{self, TSTransform}, include_image, pos2, remap, scroll_area::ScrollBarVisibility, vec2, Button, CollapsingHeader, Color32, Context, CursorIcon, DragAndDrop, DroppedFile, Id, Image, InnerResponse, LayerId, Margin, Order, Rect, Response, RichText, ScrollArea, Sense, Separator, Shape, Stroke, Ui, UiBuilder, Vec2, Vec2b, Widget
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

static ENTRY_KIND_CACHE: Lazy<DashMap<PathBuf, EntryKind>> =
    Lazy::new(|| DashMap::with_capacity(1000));

impl<P: AsRef<Path>> From<P> for EntryKind {
    fn from(value: P) -> Self {
        let path = value.as_ref();

        if let Some(entry) = ENTRY_KIND_CACHE.get(path) {
            return *entry;
        }

        let kind = if path.is_dir() {
            Self::Directory
        } else {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some(ext) => {
                    if AUDIO_EXTENSIONS.binary_search(&ext.to_ascii_lowercase().as_str()).is_ok() {
                        Self::Audio
                    } else {
                        Self::File
                    }
                }
                None => Self::File
            }
        };

        ENTRY_KIND_CACHE.insert(path.to_path_buf(), kind);
        kind
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
    theme: Rc<ThemeColors>,
    cached_entries: HashMap<PathBuf, CachedEntry>,
    watcher: RecommendedWatcher,
    watcher_rx: Receiver<notify::Result<Event>>,
    optimized_out_dirs: i32,
    optimized_out_files: i32,
}

struct CachedEntry {
    rx: Receiver<io::Result<Vec<PathBuf>>>,
    entries: Option<io::Result<Rc<[PathBuf]>>>,
}

impl Browser {
    pub fn new(theme: Rc<ThemeColors>) -> Self {
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
                        if last_path.is_none_or(|last_path| last_path != path) || empty {
                            file_data_tx
                                .send(PreviewData {
                                    length: source.total_duration(),
                                    started_playing: Instant::now(),
                                })
                                .unwrap();
                            sink.append(source);
                        }
                        last_path = Some(path);
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
            theme,
            cached_entries: HashMap::new(),
            watcher,
            watcher_rx,
            optimized_out_dirs: 0,
            optimized_out_files: 0,
        }
    }

    // Animations
    fn loading(ui: &mut Ui) -> Response {
        #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
        let rotated = Image::new(include_image!("../images/icons/loading.png")).rotate(ui.input(|i| i.time * 6.0) as f32, vec2(0.5, 0.5));
        ui.ctx().request_repaint();
        ui.add_sized(vec2(16., 16.), rotated)
    }

    // Widgets
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

    pub fn collapsing_header_icon(ui: &Ui, theme: &Rc<ThemeColors>, path: &Path, hovered_entry: Option<&Path>, openness: f32, response: &Response) {
        let visuals = ui.style().interact(response);
        let rect = Rect::from_center_size(response.rect.center(), response.rect.size() * 0.75).expand(visuals.expansion);
        let mut points = vec![rect.left_top(), rect.right_top(), rect.center_bottom()];
        let rotation = emath::Rot2::from_angle(remap(openness, 0. ..=1., -PI / 2. ..=0.));
        for p in &mut points {
            *p = rect.center() + rotation * (*p - rect.center());
        }

        ui.painter().add(Shape::convex_polygon(
            points,
            if Some(path) == hovered_entry {
                theme.browser_folder_hover_text
            } else {
                theme.browser_folder_text
            },
            Stroke::NONE,
        ));
    }

    fn add_files(&mut self, ui: &mut Ui) -> Response {
        self.handle_file_or_folder_drop(ui.ctx());
        egui::Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let borrowed_rc = Rc::clone(&self.open_paths);
                    let borrowed = borrowed_rc.borrow();
                    for path in borrowed.iter() {
                        self.add_entry(path, ui);
                    }
                })
            })
            .response
    }

    fn add_entry(&mut self, path: &Path, ui: &mut Ui) -> Response {
        let widget_pos_y = ui.next_widget_position().y;
        let clip_max = ui.clip_rect().max.y + 48.;
        let clip_min = ui.clip_rect().min.y - 48.;

        if widget_pos_y >= clip_max {
            return ui.allocate_response(vec2(0.0, 24.0), Sense::hover());
        }
        let kind = EntryKind::from(&path);
        if widget_pos_y + 24. <= clip_min {
            if kind != EntryKind::Directory {
                self.optimized_out_files += 1;
                return ui.allocate_response(vec2(0.0, 24.0), Sense::hover())
            }
            let response: Option<Response> = {
                let mut contains = false;
                if let Some(_) = self
                    .cached_entries
                    .get(path)
                    .and_then(|entry| match &entry.entries {
                        Some(Ok(dir_entries)) => Some(dir_entries),
                        _ => None
                    }) {
                        contains = true;
                    }
                if contains {
                    None
                } else {
                    self.optimized_out_dirs += 1;
                    Some(ui.allocate_response(vec2(0.0, 24.0), Sense::hover()))
                }
            };
            if let Some(response) = response {
                return response;
            }
        }

        let name = path.file_name().map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy());

        let button = |hovered_entry: &Option<PathBuf>, theme: &Rc<ThemeColors>| {
            Button::new(RichText::new(&*name).color(match (Some(path) == hovered_entry.as_deref(), matches!(&name, &Cow::Owned(_))) {
                (true, true) => theme.browser_unselected_hover_button_fg_invalid,
                (true, false) => theme.browser_unselected_hover_button_fg,
                (false, true) => theme.browser_unselected_button_fg_invalid,
                (false, false) => theme.browser_unselected_button_fg,
            }))
            .frame(false)
        };

        let response = match kind {
            EntryKind::Audio => self.add_audio_entry(path, ui, button),
            EntryKind::File => Self::add_file(ui, button(&self.hovered_entry, &self.theme)),
            EntryKind::Directory => {
                let response = CollapsingHeader::new(RichText::new(name.clone()).color(if Some(path) == self.hovered_entry.as_deref() {
                    self.theme.browser_folder_hover_text
                } else {
                    self.theme.browser_folder_text
                }))
                .id_salt(path)
                .icon({
                    let theme = Rc::clone(&self.theme);
                    let hovered_entry = self.hovered_entry.clone();
                    let path = path.to_path_buf();
                    move |ui, openness, response| {
                        Self::collapsing_header_icon(ui, &theme, &path, hovered_entry.as_deref(), openness, response);
                    }
                })
                .show(ui, |ui| {
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
        } else {
            let mut pbuf = PathBuf::new();
            pbuf.push(Path::new(""));
            if self.hovered_entry.as_ref().unwrap_or(&pbuf).to_str().unwrap_or_default() == path.to_path_buf().to_str().unwrap_or_default() {
                self.hovered_entry = None;
            }
        }
        response
    }

    fn add_audio_entry(&mut self, path: &Path, ui: &mut Ui, button: impl Fn(&Option<PathBuf>, &Rc<ThemeColors>) -> Button<'static>) -> Response {
        let mut add_contents = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.add(Image::new(include_image!("../images/icons/audio.png")))
                    .union(ui.add(button(&self.hovered_entry, &self.theme)))
                    .pipe(|response| {
                        let data = self.preview.data();
                        if let Some(data @ PreviewData { length: Some(length), .. }) = self.preview.path.as_ref().filter(|preview_path| *preview_path == path).zip(data).map(|(_, data)| data) {
                            ui.ctx().request_repaint();
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
        let mut response = if ui.ctx().is_being_dragged(Id::new(path.to_owned())) {
            DragAndDrop::set_payload(ui.ctx(), path.to_path_buf());
            let layer_id = LayerId::new(Order::Tooltip, Id::new(path.to_owned()));
            let response = ui.scope_builder(UiBuilder::new().layer_id(layer_id), add_contents).response;
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let delta = pointer_pos - response.rect.center();
                ui.ctx().transform_layer_shapes(layer_id, TSTransform::from_translation(delta));
            }
            response
        } else {
            let response = ui.scope(&mut add_contents).response;
            let dnd_response = ui.interact(response.rect, Id::new(path.to_owned()), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Grab);
            dnd_response | response
        };
        response.layer_id = ui.layer_id();
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
        match entries {
            Some(result) => match result {
                Ok(entries) => Rc::clone(entries).iter().map(|path| self.add_entry(path, ui)).reduce(Response::bitor),
                Err(error) => Some(ui.label(format!("Failed to load contents: {error}"))),
            },
            None => match rx.try_recv() {
                Ok(Ok(recv_entries)) => {
                    *entries = Some(Ok(recv_entries.into()));
                    Some(Self::loading(ui))
                }
                Ok(Err(error)) => {
                    *entries = Some(Err(error));
                    None
                }
                Err(TryRecvError::Disconnected) => None,
                Err(TryRecvError::Empty) => Some(Self::loading(ui)),
            },
        }
    }

    fn handle_file_or_folder_drop(&self, ctx: &Context) {
        ctx.input(|input| {
            for path in input.raw.dropped_files.iter().filter_map(|DroppedFile { path, .. }| path.as_deref()) {
                self.open_paths.borrow_mut().push(path.to_path_buf());
            }
        });
    }

    fn add_file(ui: &mut Ui, button: Button<'_>) -> Response {
        let InnerResponse { inner, response } = ui.horizontal(|ui| ui.add(Image::new(include_image!("../images/icons/file.png"))).union(ui.add(button)));
        inner | response
    }
}

impl Widget for &mut Browser {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add_space(6.);
        let resp = ui.vertical(|ui| {
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
            });
            ui.add_space(4.);
            ScrollArea::both()
                .drag_to_scroll(false)
                .auto_shrink(false)
                .show_viewport(ui, |ui, _| {
                    egui::Frame::default().show(ui, |ui| {
                        ui.vertical(|ui| {
                            match self.selected_category {
                                Category::Files => self.add_files(ui),
                                Category::Devices => {
                                    // TODO: Show some devices here!
                                    ui.label("Devices")
                                }
                            }
                        })
                    })
                })
                .inner
                .response
        })
        .inner;
        ui.painter().text(
            pos2(14., ui.max_rect().bottom() - 30.),
            egui::Align2::LEFT_CENTER,
            format!("Optimized out: {} dirs, {} files", self.optimized_out_dirs, self.optimized_out_files),
            egui::FontId::proportional(12.),
            self.theme.browser_unselected_button_fg_invalid,
        );
        self.optimized_out_files = 0;
        self.optimized_out_dirs = 0;
        resp
    }
}

const AUDIO_EXTENSIONS: [&str; 6] = ["flac", "mp3", "ogg", "opus", "wav", "wave"];
