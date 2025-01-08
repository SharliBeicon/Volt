use std::collections::HashMap;
use std::io::BufReader;
use std::ops::BitOr;
use std::path::PathBuf;
use std::{fs::File, num::NonZeroU64};

use blerp::processing::effects::{clip::Clip, scale::Scale};
use eframe::egui;
use egui::{hex_color, vec2, Color32, CursorIcon, Frame, Id, Margin, Rect, Response, Sense, Stroke, Ui, UiBuilder, Vec2, Widget};
use graph::{Graph, Node, NodeData, NodeId};
use itertools::Itertools;
use rodio::{Decoder, Source};
use tap::Tap;

mod graph {
    use blerp::processing::effects::Effect;
    use egui::Vec2;
    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::num::NonZeroU64;

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    pub enum NodeId {
        Output,
        Middle(NonZeroU64),
    }

    pub struct Graph {
        pub nodes: HashMap<NodeId, Node>,
        pub pan_offset: Vec2,
        pub drag_start_offset: Option<Vec2>,
    }

    pub struct Node {
        pub position: Vec2,
        pub data: NodeData,
        pub drag_start_offset: Option<Vec2>,
    }

    pub enum NodeData {
        Output,
        Middle { effect: Box<dyn Effect>, output: Option<NodeId> },
    }
}

enum Mode {
    Playlist,
    Graph(Graph),
}

pub struct Central {
    mode: Mode,
}

impl Default for Central {
    fn default() -> Self {
        Self::new()
    }
}

impl Central {
    pub fn new() -> Self {
        Self {
            mode: Mode::Graph(Graph {
                nodes: [
                    (
                        NodeId::Middle(NonZeroU64::new(1).unwrap()),
                        Node {
                            data: NodeData::Middle {
                                effect: Box::new(Clip::new_symmetrical(0.5)),
                                output: Some(NodeId::Middle(NonZeroU64::new(2).unwrap())),
                            },
                            position: vec2(-200., -20.),
                            drag_start_offset: None,
                        },
                    ),
                    (
                        NodeId::Middle(NonZeroU64::new(2).unwrap()),
                        Node {
                            data: NodeData::Middle {
                                effect: Box::new(Scale::new(2.)),
                                output: Some(NodeId::Output),
                            },
                            position: vec2(-30., 80.),
                            drag_start_offset: None,
                        },
                    ),
                    (
                        NodeId::Output,
                        Node {
                            data: NodeData::Output,
                            position: vec2(150., 10.),
                            drag_start_offset: None,
                        },
                    ),
                ]
                .into(),
                pan_offset: Vec2::ZERO,
                drag_start_offset: None,
            }),
        }
    }

    fn add_playlist(ui: &mut Ui) -> Response {
        ui.style_mut().spacing.item_spacing = Vec2::splat(8.);
        ui.vertical(|ui| {
            (0..5)
                .map(|y| {
                    Frame::default()
                        .rounding(2.)
                        .inner_margin(Margin::same(8.))
                        .stroke(Stroke::new(1., hex_color!("00000080")))
                        .show(ui, |ui| {
                            let (response, painter) = ui.allocate_painter(vec2(ui.available_width(), 48.), Sense::hover());
                            if let Some(path) = response.dnd_hover_payload::<PathBuf>() {
                                if let Some(duration) = File::open(&*path)
                                    .ok()
                                    .and_then(|file| Decoder::new(BufReader::new(file)).ok())
                                    .and_then(|decoder| decoder.total_duration())
                                {
                                    let width = duration.as_secs_f32();
                                    painter.debug_rect(response.rect.tap_mut(|rect| rect.set_width(width)), Color32::RED, format!("{}", path.to_string_lossy()));
                                }
                            };
                            ui.label(format!("Track {y}")).union(response)
                        })
                        .response
                })
                .reduce(Response::bitor)
        })
        .response
    }
}

impl Widget for &mut Central {
    fn ui(self, ui: &mut Ui) -> Response {
        Frame::default()
            .inner_margin(Margin::same(8.))
            .show(ui, |ui| match &mut self.mode {
                Mode::Playlist => Central::add_playlist(ui),
                Mode::Graph(Graph { nodes, pan_offset, drag_start_offset }) => {
                    let (_, rect) = ui.allocate_space(ui.available_size());
                    let painter = ui.painter_at(rect);
                    Frame::default()
                        .show(ui, |ui| {
                            let responses: HashMap<_, _> = nodes
                                .iter()
                                .map(|(id, node)| {
                                    let response = ui
                                        .allocate_new_ui(UiBuilder::new().max_rect(Rect::from_min_size(rect.center() + node.position + *pan_offset, Vec2::INFINITY)), |ui| {
                                            Frame::default()
                                                .rounding(4.)
                                                .inner_margin(4.)
                                                .stroke(Stroke::new(1., hex_color!("80808080")))
                                                .show(ui, |ui| {
                                                    ui.label("Effect");
                                                    ui.label(match &node.data {
                                                        NodeData::Output => "Output".to_string(),
                                                        NodeData::Middle { effect, output } => format!("{effect} to {output:?}"),
                                                    });
                                                })
                                                .response
                                        })
                                        .inner;
                                    (*id, response)
                                })
                                .collect();
                            let is_being_dragged = ui.ctx().is_being_dragged(Id::new("graph background"));
                            if is_being_dragged {
                                let pos = ui.ctx().pointer_interact_pos().unwrap();
                                if let Some(drag_start_offset) = drag_start_offset {
                                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                                    *pan_offset = pos - rect.center() - *drag_start_offset;
                                } else {
                                    *drag_start_offset = Some(pos - rect.center() - *pan_offset);
                                }
                            } else {
                                ui.interact(rect, Id::new("graph background"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Grab);
                                *drag_start_offset = None;
                            }
                            for (id, node) in nodes.iter_mut() {
                                let is_being_dragged = ui.ctx().is_being_dragged(Id::new(id));
                                if is_being_dragged {
                                    let pos = ui.ctx().pointer_interact_pos().unwrap();
                                    if let Some(drag_start_offset) = node.drag_start_offset {
                                        ui.ctx().set_cursor_icon(CursorIcon::Move);
                                        node.position = pos - rect.center() - drag_start_offset;
                                    } else {
                                        node.drag_start_offset = Some(pos - rect.center() - node.position);
                                    }
                                } else {
                                    ui.interact(responses.get(id).unwrap().rect, Id::new(id), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Move);
                                    node.drag_start_offset = None;
                                }
                            }
                            for (a, b) in nodes.iter().filter_map(move |(id, node)| {
                                if let NodeData::Middle { output: Some(output), .. } = &node.data {
                                    Some((responses.get(id).unwrap().rect, responses.get(output).unwrap().rect))
                                } else {
                                    None
                                }
                            }) {
                                const RESOLUTION: usize = 20;
                                let a = a.right_center();
                                let b = b.left_center();
                                let strength = 100_f32.min(a.distance(b) / 2.);

                                for (a, b) in (0..=RESOLUTION)
                                    .map(|t| {
                                        #[allow(clippy::cast_precision_loss, reason = "rounding errors are negligible because this is a visual effect")]
                                        let t = t as f32 / RESOLUTION as f32;

                                        (1. - t).powi(3) * a
                                            + (3. * (1. - t).powi(2) * t * (a + vec2(strength, 0.))).to_vec2()
                                            + (3. * (1. - t) * t.powi(2) * (b - vec2(strength, 0.))).to_vec2()
                                            + (t.powi(3) * b).to_vec2()
                                    })
                                    .tuple_windows()
                                {
                                    #[allow(clippy::tuple_array_conversions, reason = "this looks fine")]
                                    painter.line_segment([a, b], Stroke::new(2., hex_color!("#80808080")));
                                }
                            }
                        })
                        .response
                }
            })
            .response
    }
}
