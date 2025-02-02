// TODO: Rewrite CollapsingHeader and a large part of the browser to be much more optimized. We should only ever call on entries we can actually see.

use blerp::utils::zip;
use itertools::Itertools;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use open::that_detached;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{
    borrow::Cow,
    collections::HashMap,
    f32::consts::PI,
    fs::{read_dir, DirEntry, File},
    io::{BufReader},
    iter::Iterator,
    ops::BitOr,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    string::ToString,
    sync::{
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Arc, RwLock,
    },
    task::Poll,
    thread::spawn,
    time::{Duration, Instant},
};
use strum::Display;
use tap::Pipe;
use tracing::{error, trace};

use egui::{
    emath::{self, TSTransform},
    include_image, remap, vec2, Button, Context, CursorIcon, DragAndDrop, DroppedFile, Id, Image, LayerId, Margin, Order, Rect, Response, RichText, ScrollArea, Sense,
    Separator, Shape, Stroke, Ui, UiBuilder, Widget,
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
    path: PathBuf,
    kind: EntryKind,
    depth: usize,
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Directory,
    Audio,
    File,
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
    open_paths: Vec<PathBuf>,
    expanded_paths: Vec<PathBuf>,
    preview: Preview,
    theme: Rc<ThemeColors>,
    cached_entries: HashMap<PathBuf, CachedEntry>,
    watcher: RecommendedWatcher,
    watcher_rx: Receiver<notify::Result<Event>>,
    cached_entry_kinds: Arc<RwLock<HashMap<PathBuf, EntryKind>>>,
}

struct CachedEntry {
    rx: Receiver<Option<Vec<PathBuf>>>,
    entries: Poll<Option<Rc<[PathBuf]>>>,
}

impl Browser {
    pub fn new(theme: Rc<ThemeColors>) -> Self {
        let (watcher_tx, watcher_rx) = channel();
        let watcher = recommended_watcher(watcher_tx).unwrap();
        Self {
            selected_category: Category::Files,
            open_paths: vec![PathBuf::from_str("/").unwrap()],
            expanded_paths: Vec::new(),
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
            theme,
            cached_entries: HashMap::new(),
            watcher,
            watcher_rx,
            cached_entry_kinds: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn entry_kind_of(path: impl AsRef<Path>, cached_entry_kinds: &RwLock<HashMap<PathBuf, EntryKind>>) -> EntryKind {
        let path = path.as_ref();
        *cached_entry_kinds.write().unwrap().entry(path.to_path_buf()).or_insert_with(|| {
            if path.is_dir() {
                EntryKind::Directory
            } else {
                path.extension().and_then(|ext| ext.to_str()).map_or(EntryKind::File, |extension| {
                    const AUDIO_EXTENSIONS: [&str; 6] = ["flac", "mp3", "ogg", "opus", "wav", "wave"];
                    if AUDIO_EXTENSIONS.into_iter().any(|other| other.eq_ignore_ascii_case(extension)) {
                        EntryKind::Audio
                    } else {
                        EntryKind::File
                    }
                })
            }
        })
    }

    // Animations
    fn loading(ui: &mut Ui) -> Response {
        #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
        let rotated = Image::new(include_image!("../images/icons/loading.png")).rotate(ui.input(|i| i.time * 6.0) as f32, vec2(0.5, 0.5));
        ui.ctx().request_repaint();
        ui.add_sized(vec2(16., 16.), rotated)
    }

    // Widgets
    pub fn button<'a>(theme: &'a ThemeColors, selected: bool, text: &'a str) -> impl Widget + use<'a> {
        move |ui: &mut Ui| {
            ui.allocate_ui(vec2(0., 24.), |ui| {
                ui.visuals_mut().widgets.inactive.fg_stroke.color = theme.browser_unselected_button_fg;
                ui.visuals_mut().widgets.hovered.fg_stroke.color = theme.browser_unselected_hover_button_fg;
                let button = ui
                    .centered_and_justified(|ui| Button::new(RichText::new(text).size(14.).pipe(|text| if selected { text.color(theme.browser_selected_button_fg) } else { text })).ui(ui))
                    .inner;
                ui.visuals_mut().widgets.noninteractive.bg_stroke.color = if selected {
                    theme.browser_selected_button_fg
                } else if button.hovered() {
                    theme.browser_unselected_hover_button_fg
                } else {
                    theme.browser_unselected_button_fg
                };
                ui.add(Separator::default().spacing(0.));
                button
            })
            .inner
        }
    }

    pub fn collapsing_header_icon(ui: &Ui, theme: &Rc<ThemeColors>, openness: f32, response: &Response) {
        let visuals = ui.style().interact(response);
        let rect = Rect::from_center_size(response.rect.center(), response.rect.size() * 0.75).expand(visuals.expansion);
        let mut points = vec![rect.left_top(), rect.right_top(), rect.center_bottom()];
        let rotation = emath::Rot2::from_angle(remap(openness, 0. ..=1., -PI / 2. ..=0.));
        for p in &mut points {
            *p = rect.center() + rotation * (*p - rect.center());
        }

        ui.painter().add(Shape::convex_polygon(points, theme.browser_folder_text, Stroke::NONE));
    }

    fn add_files(&mut self, ui: &mut Ui) -> Response {
        self.handle_file_or_folder_drop(ui.ctx());
        egui::Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.visuals_mut().widgets.noninteractive.fg_stroke.color = self.theme.browser_folder_text;
                    ui.visuals_mut().widgets.hovered.fg_stroke.color = self.theme.browser_folder_hover_text;
                    for entry in self.open_paths.iter().fold(Vec::new(), |mut entries, path| {
                        Self::entries(
                            &mut entries,
                            path,
                            0,
                            &mut self.cached_entries,
                            &self.cached_entry_kinds,
                            &self.expanded_paths,
                            &self.watcher_rx,
                            &mut self.watcher,
                        );
                        entries
                    }) {
                        self.add_entry(entry, ui);
                    }
                })
            })
            .response
    }

    fn list_cached<'a>(
        path: &Path,
        cached_entries: &'a mut HashMap<PathBuf, CachedEntry>,
        watcher_rx: &Receiver<notify::Result<Event>>,
        watcher: &mut RecommendedWatcher,
        cached_entry_kinds: &Arc<RwLock<HashMap<PathBuf, EntryKind>>>,
    ) -> &'a Poll<Option<Rc<[PathBuf]>>> {
        match watcher_rx.try_recv() {
            Ok(event) => {
                let event = event.unwrap();
                match event.kind {
                    EventKind::Access(_) => {}
                    _ => {
                        for path in event.paths.iter().map(|path| if path.is_dir() { path } else { path.parent().unwrap() }) {
                            cached_entries.remove(path);
                        }
                    }
                }
            }
            Err(TryRecvError::Disconnected) => {
                panic!()
            }
            Err(TryRecvError::Empty) => {}
        }
        let CachedEntry { rx, entries } = cached_entries.entry(path.to_path_buf()).or_insert_with(|| {
            trace!("entry cache miss for {:?}", path);
            if let Err(error) = watcher.watch(path, RecursiveMode::NonRecursive) {
                error!("Unexpected error while trying to watch directory: {:?}", error);
            }
            let (tx, rx) = channel();
            let read_dir = read_dir(path);
            let cached_entry_kinds = Arc::clone(cached_entry_kinds);
            spawn(move || {
                tx.send(read_dir.ok().and_then(|entries| {
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
                            Ord::cmp(
                                &(a.map(|path| Self::entry_kind_of(path, &cached_entry_kinds)), a),
                                &(b.map(|path| Self::entry_kind_of(path, &cached_entry_kinds)), b),
                            )
                        })
                        .try_collect()
                        .ok()
                }))
                .unwrap();
            });
            CachedEntry { rx, entries: Poll::Pending }
        });
        if entries.is_pending() {
            match rx.try_recv() {
                Ok(Some(list)) => {
                    *entries = Poll::Ready(Some(list.into()));
                }
                Ok(None) => {
                    *entries = Poll::Ready(None);
                }
                Err(_) => {}
            }
        }
        entries
    }

    fn entries(
        entries: &mut Vec<Entry>,
        path: &Path,
        mut depth: usize,
        cached_entries: &mut HashMap<PathBuf, CachedEntry>,
        cached_entry_kinds: &Arc<RwLock<HashMap<PathBuf, EntryKind>>>,
        expanded_paths: &[PathBuf],
        watcher_rx: &Receiver<notify::Result<Event>>,
        watcher: &mut RecommendedWatcher,
    ) {
        if depth == 0 {
            entries.push(Entry {
                path: path.to_path_buf(),
                kind: Self::entry_kind_of(path, cached_entry_kinds),
                depth,
            });
        }
        if !expanded_paths.iter().any(|expanded| expanded == path) {
            return;
        }
        depth += 1;
        if let Poll::Ready(Some(list)) = Self::list_cached(path, cached_entries, watcher_rx, watcher, cached_entry_kinds) {
            for entry in list
                .iter()
                .cloned()
                .sorted_unstable_by(|a, b| Ord::cmp(&(Self::entry_kind_of(a, cached_entry_kinds), a), &(Self::entry_kind_of(b, cached_entry_kinds), b)))
            {
                entries.push(Entry {
                    path: entry.clone(),
                    kind: Self::entry_kind_of(&entry, cached_entry_kinds),
                    depth,
                });
                if expanded_paths.contains(&entry) {
                    Self::entries(entries, &entry, depth, cached_entries, cached_entry_kinds, expanded_paths, watcher_rx, watcher);
                }
            }
        }
    }

    fn add_entry(&mut self, entry: Entry, ui: &mut Ui) -> Response {
        const ENTRY_HEIGHT: f32 = 20.;
        let next_top = ui.next_widget_position().y;
        let next_bottom = next_top + ENTRY_HEIGHT;
        if next_top >= ui.clip_rect().bottom() || next_bottom <= ui.clip_rect().top() && entry.kind != EntryKind::Directory {
            return ui.allocate_response(vec2(0.0, ENTRY_HEIGHT), Sense::hover());
        }
        let name = entry.path.file_name().map_or_else(|| entry.path.to_string_lossy(), |name| name.to_string_lossy());
        let button = |theme: &ThemeColors| -> Button<'static> {
            Button::new(RichText::new(name.to_string()).pipe(|text| {
                if matches!(&name, &Cow::Owned(_)) {
                    text.color(theme.browser_unselected_button_fg_invalid)
                } else {
                    text
                }
            }))
        };
        let response = ui
            .allocate_ui(vec2(f32::INFINITY, ENTRY_HEIGHT), |ui| {
                ui.horizontal(|ui| {
                    const INDENT_SIZE: f32 = 16.;
                    #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
                    #[allow(clippy::cast_precision_loss, reason = "this is a visual effect")]
                    ui.add_space(INDENT_SIZE * entry.depth as f32);
                    match entry.kind {
                        EntryKind::Audio => self.add_audio_entry(&entry.path, ui, &Rc::clone(&self.theme), button),
                        EntryKind::File => Self::add_file(ui, button(&self.theme)),
                        EntryKind::Directory => ui.add(button(&self.theme)),
                    }
                })
            })
            .inner
            .inner;
        if response.clicked() {
            match entry.kind {
                EntryKind::Audio => {
                    // TODO: Proper preview implementation with cpal. This is temporary (or at least make it work well with a proper preview widget)
                    // Also, don't spawn a new thread - instead, dedicate a thread for preview
                    self.preview.play_file(entry.path.clone());
                }
                EntryKind::File => {
                    that_detached(entry.path).unwrap();
                }
                EntryKind::Directory => {
                    if let Some(index) = self.expanded_paths.iter().position(|expanded| expanded == &entry.path) {
                        self.expanded_paths.swap_remove(index);
                    } else {
                        self.expanded_paths.push(entry.path);
                    }
                }
            }
        }
        response
    }

    fn add_audio_entry(&mut self, path: &Path, ui: &mut Ui, theme: &Rc<ThemeColors>, button: impl Fn(&ThemeColors) -> Button<'static>) -> Response {
        let mut add_contents = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.add(Image::new(include_image!("../images/icons/audio.png"))).union(ui.add(button(theme))).pipe(|response| {
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

    // fn add_directory_contents(&mut self, path: &Path, ui: &mut Ui) -> Option<Response> {
    //     match self.watcher_rx.try_recv() {
    //         Ok(event) => {
    //             let event = event.unwrap();
    //             match event.kind {
    //                 EventKind::Access(_) => {}
    //                 _ => {
    //                     for path in event.paths.iter().map(|path| if path.is_dir() { path } else { path.parent().unwrap() }) {
    //                         self.cached_entries.remove(path);
    //                     }
    //                 }
    //             }
    //         }
    //         Err(TryRecvError::Disconnected) => {
    //             panic!()
    //         }
    //         Err(TryRecvError::Empty) => {}
    //     }
    //     let CachedEntry { rx, entries } = self.cached_entries.entry(path.to_path_buf()).or_insert_with(|| {
    //         trace!("entry cache miss for {:?}", path);
    //         if let Err(error) = self.watcher.watch(path, RecursiveMode::NonRecursive) {
    //             error!("Unexpected error while trying to watch directory: {:?}", error);
    //         }
    //         let (tx, rx) = channel();
    //         let read_dir = read_dir(path);
    //         let cached_entry_kinds = Arc::clone(&self.cached_entry_kinds);
    //         spawn(move || {
    //             tx.send(read_dir.and_then(|entries| {
    //                 entries
    //                     .map(|entry| {
    //                         {
    //                             match entry {
    //                                 Ok(ref x) => Ok(x),
    //                                 Err(x) => Err(x),
    //                             }
    //                         }
    //                         .map(DirEntry::path)
    //                     })
    //                     .sorted_unstable_by(|a, b| {
    //                         let a = a.as_ref().ok();
    //                         let b = b.as_ref().ok();
    //                         Ord::cmp(
    //                             &(a.map(|path| Self::entry_kind_of(path, &cached_entry_kinds)), a),
    //                             &(b.map(|path| Self::entry_kind_of(path, &cached_entry_kinds)), b),
    //                         )
    //                     })
    //                     .try_collect()
    //             }))
    //             .unwrap();
    //         });
    //         CachedEntry { rx, entries: None }
    //     });
    //     match entries {
    //         Some(result) => match result {
    //             Ok(entries) => Rc::clone(entries).iter().map(|path| self.add_entry(path, ui)).reduce(Response::bitor),
    //             Err(error) => Some(ui.label(format!("Failed to load contents: {error}"))),
    //         },
    //         None => match rx.try_recv() {
    //             Ok(Ok(recv_entries)) => {
    //                 *entries = Some(Ok(recv_entries.into()));
    //                 Some(Self::loading(ui))
    //             }
    //             Ok(Err(error)) => {
    //                 *entries = Some(Err(error));
    //                 None
    //             }
    //             Err(TryRecvError::Disconnected) => None,
    //             Err(TryRecvError::Empty) => Some(Self::loading(ui)),
    //         },
    //     }
    // }

    fn handle_file_or_folder_drop(&mut self, ctx: &Context) {
        ctx.input(|input| {
            for path in input.raw.dropped_files.iter().filter_map(|DroppedFile { path, .. }| path.as_deref()) {
                self.open_paths.push(path.to_path_buf());
            }
        });
    }

    fn add_file(ui: &mut Ui, button: Button<'_>) -> Response {
        ui.horizontal(|ui| ui.add(Image::new(include_image!("../images/icons/file.png"))).union(ui.add(button))).inner
    }
}

impl Widget for &mut Browser {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add_space(6.);
        ui.vertical(|ui| {
            ui.visuals_mut().button_frame = false;
            ui.visuals_mut().interact_cursor = Some(CursorIcon::PointingHand);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.;
                ui.columns_const(|uis| {
                    zip(Category::VARIANTS, uis.each_mut())
                        .map(|(category, ui)| {
                            let selected = self.selected_category == category;
                            let string = category.to_string();
                            let response = ui.add(Browser::button(&self.theme, selected, &string));
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
        .inner
    }
}
