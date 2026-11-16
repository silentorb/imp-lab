//! Iced Canvas DAG view with pan/zoom and always-on graph editing.

use std::collections::{BTreeMap, BTreeSet};

use iced::alignment;
use iced::mouse;
use iced::widget::canvas::{self, path::Builder, Frame, Geometry, Path, Stroke, Text};
use iced::widget::Canvas;
use iced::{Color, Element, Fill, Font, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::graph_edit::{expand_input_slots, CatalogPort, InputSlot};
use crate::graph_layout::{
    layout_graph, node_size_for, GraphLayout, NODE_HEADER_HEIGHT, PORT_SPACING,
};
use crate::graph_ui::{ConnectionMode, GraphUiState};
use imp_core_types::{EdgeId, Graph, NodeId};

const MIN_SCALE: f32 = 0.25;
const MAX_SCALE: f32 = 4.0;
const ZOOM_STEP: f32 = 1.1;
const NODE_RADIUS: f32 = 8.0;
const PORT_RADIUS: f32 = 5.0;
const PORT_HIT_RADIUS: f32 = 10.0;
const EDGE_HIT_DISTANCE: f32 = 8.0;
const PORT_LABEL_GAP: f32 = 6.0;
const PORT_LABEL_SIZE: f32 = 11.0;
/// Screen-space movement before a Simple node press becomes a connect drag.
const CONNECT_DRAG_THRESHOLD: f32 = 6.0;

/// Input/output port templates for a node type instance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodePortInfo {
    /// Catalog input templates (variadic inputs expand at draw/hit time).
    pub inputs: Vec<CatalogPort>,
    /// Output port ids (top→bottom).
    pub outputs: Vec<String>,
}

/// Port catalog keyed by node id.
pub type PortCatalog = BTreeMap<NodeId, NodePortInfo>;

/// Canvas edit events emitted by the graph view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEdit {
    /// Select a node (or clear when `None`).
    SelectNode {
        /// Node id, or `None` to clear.
        node_id: Option<String>,
    },
    /// Select an edge (or clear when `None`).
    SelectEdge {
        /// Edge id, or `None` to clear.
        edge_id: Option<String>,
    },
    /// Insert a node of the host's current add type.
    AddNode,
    /// Simple-mode connect between two nodes.
    ConnectNodes {
        /// Source node.
        from: String,
        /// Target node.
        to: String,
    },
    /// Precise-mode connect between ports.
    ConnectPorts {
        /// Source node.
        from_node: String,
        /// Source output port.
        from_port: String,
        /// Target node.
        to_node: String,
        /// Target input port.
        to_port: String,
        /// When true, append to a variadic input with the next `Edge.order`.
        variadic_target: bool,
    },
    /// Begin a connect gesture.
    BeginConnect {
        /// Source node.
        from_node: String,
        /// Source port when precise.
        from_port: Option<String>,
    },
    /// Cancel an in-progress connect.
    CancelConnect,
    /// Erase a node and incident edges.
    EraseNode {
        /// Node to remove.
        node_id: String,
    },
    /// Erase a single edge by id.
    EraseEdge {
        /// Edge to remove.
        edge_id: String,
    },
    /// Erase all edges between two nodes (simple-mode line).
    EraseEdgesBetween {
        /// One endpoint.
        a: String,
        /// Other endpoint.
        b: String,
    },
    /// Disconnect every edge attached to a port (right-click).
    DisconnectPort {
        /// Node owning the port.
        node_id: String,
        /// Port id.
        port_id: String,
        /// `true` for an output port; `false` for an input.
        is_output: bool,
    },
    /// Reorder variadic inbound edges to `ordered_edge_ids` (`0..n-1`).
    ReorderVariadicInputs {
        /// Node owning the variadic input.
        node_id: String,
        /// Variadic catalog port id.
        port: String,
        /// Edge ids in the desired top→bottom order.
        ordered_edge_ids: Vec<String>,
    },
}

/// Clamps canvas zoom to the supported range.
pub fn clamp_scale(scale: f32) -> f32 {
    scale.clamp(MIN_SCALE, MAX_SCALE)
}

/// Pans by a screen-space delta.
pub fn pan_by(translation: Vector, delta: Vector) -> Vector {
    translation + delta
}

/// Zooms so the layout point under `cursor` stays fixed.
pub fn zoom_at_point(
    translation: Vector,
    scale: f32,
    cursor: Point,
    factor: f32,
) -> (Vector, f32) {
    let new_scale = clamp_scale(scale * factor);
    if (new_scale - scale).abs() < f32::EPSILON {
        return (translation, scale);
    }

    let world = Point::new(
        (cursor.x - translation.x) / scale,
        (cursor.y - translation.y) / scale,
    );
    let new_translation = Vector::new(
        cursor.x - world.x * new_scale,
        cursor.y - world.y * new_scale,
    );
    (new_translation, new_scale)
}

/// Translation that centers `layout` in `viewport` at `scale`.
pub fn center_translation(layout: &GraphLayout, viewport: Size, scale: f32) -> Vector {
    let center = layout.content_center();
    Vector::new(
        viewport.width / 2.0 - center.x * scale,
        viewport.height / 2.0 - center.y * scale,
    )
}

fn port_position(node_pos: Point, size: Size, index: usize, is_output: bool) -> Point {
    let y = node_pos.y + NODE_HEADER_HEIGHT + PORT_SPACING / 2.0 + index as f32 * PORT_SPACING;
    let x = if is_output {
        node_pos.x + size.width
    } else {
        node_pos.x
    };
    Point::new(x, y)
}

/// Canvas program that draws and edits a laid-out Imp graph.
#[derive(Debug, Clone)]
pub struct GraphView {
    layout: GraphLayout,
    document_key: u64,
    connection_mode: ConnectionMode,
    selected_node: Option<String>,
    selected_edge: Option<String>,
    ports: PortCatalog,
    /// Expanded Precise input slots per node (includes variadic dummies).
    input_slots: BTreeMap<NodeId, Vec<InputSlot>>,
    /// Full edges for precise drawing / hit-testing.
    edges: Vec<(EdgeId, NodeId, String, NodeId, String)>,
}

impl GraphView {
    /// Creates a view for `graph` keyed by `document_key` (host session identity).
    pub fn new(
        document_key: u64,
        graph: &Graph,
        ui: &GraphUiState,
        ports: PortCatalog,
    ) -> Self {
        let edges = graph
            .edges
            .iter()
            .map(|(id, edge)| {
                (
                    id.clone(),
                    edge.from.node.clone(),
                    edge.from.port.clone(),
                    edge.to.node.clone(),
                    edge.to.port.clone(),
                )
            })
            .collect();
        let mut layout_ids: BTreeSet<NodeId> = graph.nodes.keys().cloned().collect();
        for edge in graph.edges.values() {
            layout_ids.insert(edge.from.node.clone());
            layout_ids.insert(edge.to.node.clone());
        }
        let input_slots: BTreeMap<NodeId, Vec<InputSlot>> = layout_ids
            .iter()
            .map(|id| {
                let slots = ports
                    .get(id)
                    .map(|info| expand_input_slots(&info.inputs, graph, id))
                    .unwrap_or_default();
                (id.clone(), slots)
            })
            .collect();
        let precise = ui.connection_mode == ConnectionMode::Precise;
        let node_sizes = layout_ids
            .iter()
            .map(|id| {
                let inputs = input_slots.get(id).map(|slots| slots.len()).unwrap_or(0);
                let outputs = ports.get(id).map(|info| info.outputs.len()).unwrap_or(0);
                (id.clone(), node_size_for(inputs, outputs, precise))
            })
            .collect();
        Self {
            layout: layout_graph(graph, &node_sizes),
            document_key,
            connection_mode: ui.connection_mode,
            selected_node: ui.selected_node.clone(),
            selected_edge: ui.selected_edge.clone(),
            ports,
            input_slots,
            edges,
        }
    }

    fn connect_source_point(&self, from_node: &str, from_port: Option<&str>) -> Option<Point> {
        let node = self.layout.nodes.iter().find(|n| n.id == from_node)?;
        let size = self.size_of(from_node);
        if self.connection_mode == ConnectionMode::Precise {
            let port = from_port?;
            let info = self.ports.get(from_node)?;
            let index = info.outputs.iter().position(|p| p == port).unwrap_or(0);
            Some(port_position(node.position, size, index, true))
        } else {
            Some(Point::new(
                node.position.x + size.width,
                node.position.y + size.height / 2.0,
            ))
        }
    }

    fn slots_for(&self, node_id: &str) -> &[InputSlot] {
        self.input_slots
            .get(node_id)
            .map(|slots| slots.as_slice())
            .unwrap_or(&[])
    }

    fn camera_for(&self, state: &ViewState, bounds: Size) -> (Vector, f32) {
        if state.document_key == Some(self.document_key) {
            (state.translation, state.scale)
        } else {
            (center_translation(&self.layout, bounds, 1.0), 1.0)
        }
    }

    fn sync_document(&self, state: &mut ViewState, bounds: Size) {
        if state.document_key == Some(self.document_key) {
            return;
        }
        state.document_key = Some(self.document_key);
        state.scale = 1.0;
        state.translation = center_translation(&self.layout, bounds, 1.0);
        state.drag_last = None;
        state.panning = false;
        state.reorder_drag = None;
        state.connect_drag = None;
        state.pending_node_press = None;
    }

    fn screen_to_world(&self, state: &ViewState, bounds: Size, screen: Point) -> Point {
        let (translation, scale) = self.camera_for(state, bounds);
        Point::new(
            (screen.x - translation.x) / scale,
            (screen.y - translation.y) / scale,
        )
    }

    fn size_of(&self, node_id: &str) -> Size {
        self.layout
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .map(|node| node.size)
            .unwrap_or_else(|| {
                let inputs = self.slots_for(node_id).len();
                let outputs = self
                    .ports
                    .get(node_id)
                    .map(|info| info.outputs.len())
                    .unwrap_or(0);
                node_size_for(
                    inputs,
                    outputs,
                    self.connection_mode == ConnectionMode::Precise,
                )
            })
    }

    fn hit_node(&self, world: Point) -> Option<NodeId> {
        for node in self.layout.nodes.iter().rev() {
            let size = node.size;
            let rect = Rectangle::new(node.position, size);
            if rect.contains(world) {
                return Some(node.id.clone());
            }
        }
        None
    }

    fn hit_input_slot(&self, world: Point) -> Option<(NodeId, usize, InputSlot)> {
        if self.connection_mode != ConnectionMode::Precise {
            return None;
        }
        for node in &self.layout.nodes {
            let slots = self.slots_for(&node.id);
            let size = node.size;
            for (index, slot) in slots.iter().enumerate() {
                let pos = port_position(node.position, size, index, false);
                if (pos.x - world.x).hypot(pos.y - world.y) <= PORT_HIT_RADIUS {
                    return Some((node.id.clone(), index, slot.clone()));
                }
            }
        }
        None
    }

    fn hit_output_port(&self, world: Point) -> Option<(NodeId, String)> {
        if self.connection_mode != ConnectionMode::Precise {
            return None;
        }
        for node in &self.layout.nodes {
            let Some(info) = self.ports.get(&node.id) else {
                continue;
            };
            let size = node.size;
            for (index, port) in info.outputs.iter().enumerate() {
                let pos = port_position(node.position, size, index, true);
                if (pos.x - world.x).hypot(pos.y - world.y) <= PORT_HIT_RADIUS {
                    return Some((node.id.clone(), port.clone()));
                }
            }
        }
        None
    }

    fn edge_endpoints(
        &self,
        edge_id: Option<&str>,
        from: &str,
        from_port: &str,
        to: &str,
        to_port: &str,
    ) -> Option<(Point, Point)> {
        let from_node = self.layout.nodes.iter().find(|n| n.id == from)?;
        let to_node = self.layout.nodes.iter().find(|n| n.id == to)?;
        let from_size = self.size_of(from);
        let to_size = self.size_of(to);

        let start = if self.connection_mode == ConnectionMode::Precise {
            let info = self.ports.get(from)?;
            let index = info.outputs.iter().position(|p| p == from_port).unwrap_or(0);
            port_position(from_node.position, from_size, index, true)
        } else {
            Point::new(
                from_node.position.x + from_size.width,
                from_node.position.y + from_size.height / 2.0,
            )
        };

        let end = if self.connection_mode == ConnectionMode::Precise {
            let slots = self.slots_for(to);
            let index = slots
                .iter()
                .position(|slot| match slot {
                    InputSlot::Variadic {
                        port_id,
                        edge_id: slot_edge,
                        ..
                    } => {
                        port_id == to_port
                            && edge_id.is_some_and(|id| id == slot_edge.as_str())
                    }
                    InputSlot::Singleton { port_id } => port_id == to_port,
                    InputSlot::VariadicDummy { .. } => false,
                })
                .or_else(|| {
                    slots
                        .iter()
                        .position(|slot| slot.port_id() == to_port)
                })
                .unwrap_or(0);
            port_position(to_node.position, to_size, index, false)
        } else {
            Point::new(
                to_node.position.x,
                to_node.position.y + to_size.height / 2.0,
            )
        };

        Some((start, end))
    }

    fn hit_edge(&self, world: Point) -> Option<(EdgeId, NodeId, NodeId)> {
        let mut best: Option<(f32, EdgeId, NodeId, NodeId)> = None;
        if self.connection_mode == ConnectionMode::Simple {
            for (from, to) in &self.layout.edges {
                let Some((start, end)) = self.edge_endpoints(None, from, "", to, "") else {
                    continue;
                };
                let dist = point_segment_distance(world, start, end);
                if dist <= EDGE_HIT_DISTANCE
                    && best.as_ref().is_none_or(|(best_d, ..)| dist < *best_d)
                {
                    // Use first matching real edge id for the pair.
                    let edge_id = self
                        .edges
                        .iter()
                        .find(|(_, f, _, t, _)| f == from && t == to)
                        .map(|(id, ..)| id.clone())
                        .unwrap_or_else(|| format!("{from}->{to}"));
                    best = Some((dist, edge_id, from.clone(), to.clone()));
                }
            }
        } else {
            for (edge_id, from, from_port, to, to_port) in &self.edges {
                let Some((start, end)) =
                    self.edge_endpoints(Some(edge_id), from, from_port, to, to_port)
                else {
                    continue;
                };
                let dist = point_segment_distance(world, start, end);
                if dist <= EDGE_HIT_DISTANCE
                    && best.as_ref().is_none_or(|(best_d, ..)| dist < *best_d)
                {
                    best = Some((dist, edge_id.clone(), from.clone(), to.clone()));
                }
            }
        }
        best.map(|(_, id, a, b)| (id, a, b))
    }

    fn handle_select_click(&self, world: Point) -> Option<GraphEdit> {
        if let Some(node) = self.hit_node(world) {
            return Some(GraphEdit::SelectNode {
                node_id: Some(node),
            });
        }
        if let Some((edge_id, _, _)) = self.hit_edge(world) {
            return Some(GraphEdit::SelectEdge {
                edge_id: Some(edge_id),
            });
        }
        Some(GraphEdit::SelectNode { node_id: None })
    }

    fn handle_port_disconnect(&self, world: Point) -> Option<GraphEdit> {
        if self.connection_mode != ConnectionMode::Precise {
            return None;
        }
        if let Some((node, port)) = self.hit_output_port(world) {
            return Some(GraphEdit::DisconnectPort {
                node_id: node,
                port_id: port,
                is_output: true,
            });
        }
        if let Some((node, _, slot)) = self.hit_input_slot(world) {
            if matches!(slot, InputSlot::VariadicDummy { .. }) {
                return None;
            }
            return Some(GraphEdit::DisconnectPort {
                node_id: node,
                port_id: slot.port_id().to_string(),
                is_output: false,
            });
        }
        None
    }

    fn connect_drop_edit(
        &self,
        from_node: &str,
        from_port: Option<&str>,
        world: Point,
    ) -> Option<GraphEdit> {
        if self.connection_mode == ConnectionMode::Precise {
            let from_port = from_port?;
            let (to_node, _, slot) = self.hit_input_slot(world)?;
            if !slot.accepts_connect() {
                return None;
            }
            let variadic_target = matches!(slot, InputSlot::VariadicDummy { .. });
            return Some(GraphEdit::ConnectPorts {
                from_node: from_node.to_string(),
                from_port: from_port.to_string(),
                to_node,
                to_port: slot.port_id().to_string(),
                variadic_target,
            });
        }
        let to = self.hit_node(world)?;
        if to == from_node {
            return None;
        }
        Some(GraphEdit::ConnectNodes {
            from: from_node.to_string(),
            to,
        })
    }

    fn reorder_edit_for_drop(
        &self,
        node_id: &str,
        port_id: &str,
        dragged_edge_id: &str,
        world_y: f32,
    ) -> Option<GraphEdit> {
        let node = self.layout.nodes.iter().find(|n| n.id == node_id)?;
        let slots = self.slots_for(node_id);
        let mut sibling_edge_ids: Vec<String> = slots
            .iter()
            .filter_map(|slot| match slot {
                InputSlot::Variadic {
                    port_id: slot_port,
                    edge_id,
                    ..
                } if slot_port == port_id => Some(edge_id.clone()),
                _ => None,
            })
            .collect();
        if sibling_edge_ids.len() < 2 {
            return None;
        }
        let from_index = sibling_edge_ids
            .iter()
            .position(|id| id == dragged_edge_id)?;
        let filled_slot_indices: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                InputSlot::Variadic {
                    port_id: slot_port, ..
                } if slot_port == port_id => Some(index),
                _ => None,
            })
            .collect();
        let target_index = filled_slot_indices
            .iter()
            .enumerate()
            .min_by_key(|(_, slot_index)| {
                let pos = port_position(node.position, node.size, **slot_index, false);
                ((pos.y - world_y).abs() * 1000.0) as i32
            })
            .map(|(sibling_index, _)| sibling_index)
            .unwrap_or(from_index);
        if target_index == from_index {
            return None;
        }
        let edge_id = sibling_edge_ids.remove(from_index);
        sibling_edge_ids.insert(target_index, edge_id);
        Some(GraphEdit::ReorderVariadicInputs {
            node_id: node_id.to_string(),
            port: port_id.to_string(),
            ordered_edge_ids: sibling_edge_ids,
        })
    }
}

fn point_segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len_sq = abx * abx + aby * aby;
    if len_sq < f32::EPSILON {
        return (p.x - a.x).hypot(p.y - a.y);
    }
    let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj = Point::new(a.x + t * abx, a.y + t * aby);
    (p.x - proj.x).hypot(p.y - proj.y)
}

#[derive(Debug, Clone)]
struct VariadicReorderDrag {
    node_id: String,
    port_id: String,
    edge_id: String,
}

/// In-progress connect drag (rubber-band), local to the canvas widget.
#[derive(Debug, Clone)]
struct ConnectDrag {
    from_node: String,
    from_port: Option<String>,
    cursor_world: Point,
}

/// Simple-mode node press that may become a connect drag after a threshold.
#[derive(Debug, Clone)]
struct PendingNodePress {
    node_id: String,
    start_screen: Point,
}

/// Pan/zoom camera stored in the canvas widget tree.
#[derive(Debug, Clone)]
pub struct ViewState {
    translation: Vector,
    scale: f32,
    drag_last: Option<Point>,
    panning: bool,
    reorder_drag: Option<VariadicReorderDrag>,
    connect_drag: Option<ConnectDrag>,
    pending_node_press: Option<PendingNodePress>,
    document_key: Option<u64>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            translation: Vector::ZERO,
            scale: 1.0,
            drag_last: None,
            panning: false,
            reorder_drag: None,
            connect_drag: None,
            pending_node_press: None,
            document_key: None,
        }
    }
}

/// Builds a fill-sized canvas for the active graph document.
pub fn graph_canvas<'a, Message>(
    document_key: u64,
    graph: &'a Graph,
    ui: &'a GraphUiState,
    ports: PortCatalog,
) -> Element<'a, Message>
where
    Message: 'a + Clone + From<GraphEdit>,
{
    Canvas::new(GraphView::new(document_key, graph, ui, ports))
        .width(Fill)
        .height(Fill)
        .into()
}

impl<Message> canvas::Program<Message> for GraphView
where
    Message: Clone + From<GraphEdit>,
{
    type State = ViewState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        self.sync_document(state, bounds.size());

        let cursor_in_bounds = cursor.position_in(bounds);

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if let Some(position) = cursor_in_bounds {
                    state.connect_drag = None;
                    state.pending_node_press = None;
                    state.drag_last = Some(position);
                    state.panning = true;
                    return Some(canvas::Action::capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let position = cursor_in_bounds?;
                let world = self.screen_to_world(state, bounds.size(), position);
                if let Some(edit) = self.handle_port_disconnect(world) {
                    return Some(canvas::Action::publish(Message::from(edit)).and_capture());
                }
                return Some(canvas::Action::capture());
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = cursor_in_bounds?;
                let world = self.screen_to_world(state, bounds.size(), position);
                state.connect_drag = None;
                state.pending_node_press = None;

                if self.connection_mode == ConnectionMode::Precise {
                    if let Some((node, port)) = self.hit_output_port(world) {
                        state.connect_drag = Some(ConnectDrag {
                            from_node: node,
                            from_port: Some(port),
                            cursor_world: world,
                        });
                        return Some(canvas::Action::capture());
                    }
                    if let Some((
                        node_id,
                        _,
                        InputSlot::Variadic {
                            port_id, edge_id, ..
                        },
                    )) = self.hit_input_slot(world)
                    {
                        state.reorder_drag = Some(VariadicReorderDrag {
                            node_id,
                            port_id,
                            edge_id,
                        });
                        state.drag_last = Some(position);
                        return Some(canvas::Action::capture());
                    }
                } else if let Some(node) = self.hit_node(world) {
                    state.pending_node_press = Some(PendingNodePress {
                        node_id: node,
                        start_screen: position,
                    });
                    return Some(canvas::Action::capture());
                }

                let hit = self.hit_node(world).is_some()
                    || self.hit_edge(world).is_some()
                    || self.hit_input_slot(world).is_some()
                    || self.hit_output_port(world).is_some();

                if !hit {
                    state.drag_last = Some(position);
                    state.panning = true;
                    return Some(canvas::Action::capture());
                }

                if let Some(edit) = self.handle_select_click(world) {
                    return Some(canvas::Action::publish(Message::from(edit)).and_capture());
                }
                return Some(canvas::Action::capture());
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.reorder_drag.take() {
                    let position = cursor_in_bounds.or(state.drag_last)?;
                    let world = self.screen_to_world(state, bounds.size(), position);
                    state.drag_last = None;
                    if let Some(edit) = self.reorder_edit_for_drop(
                        &drag.node_id,
                        &drag.port_id,
                        &drag.edge_id,
                        world.y,
                    ) {
                        return Some(canvas::Action::publish(Message::from(edit)).and_capture());
                    }
                    return Some(canvas::Action::capture());
                }
                if let Some(drag) = state.connect_drag.take() {
                    let position = cursor_in_bounds.or(state.drag_last);
                    let Some(position) = position else {
                        return Some(canvas::Action::capture());
                    };
                    let world = self.screen_to_world(state, bounds.size(), position);
                    state.drag_last = None;
                    if let Some(edit) = self.connect_drop_edit(
                        &drag.from_node,
                        drag.from_port.as_deref(),
                        world,
                    ) {
                        return Some(canvas::Action::publish(Message::from(edit)).and_capture());
                    }
                    return Some(canvas::Action::capture());
                }
                if let Some(pending) = state.pending_node_press.take() {
                    // Click without becoming a connect drag → select.
                    return Some(
                        canvas::Action::publish(Message::from(GraphEdit::SelectNode {
                            node_id: Some(pending.node_id),
                        }))
                        .and_capture(),
                    );
                }
                if state.drag_last.is_some() || state.panning {
                    state.drag_last = None;
                    state.panning = false;
                    return Some(canvas::Action::capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                if state.drag_last.is_some() || state.panning {
                    state.drag_last = None;
                    state.panning = false;
                    return Some(canvas::Action::capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(position) = cursor_in_bounds {
                    let world = self.screen_to_world(state, bounds.size(), position);
                    if let Some(pending) = state.pending_node_press.take() {
                        let dx = position.x - pending.start_screen.x;
                        let dy = position.y - pending.start_screen.y;
                        if dx.hypot(dy) >= CONNECT_DRAG_THRESHOLD {
                            state.connect_drag = Some(ConnectDrag {
                                from_node: pending.node_id,
                                from_port: None,
                                cursor_world: world,
                            });
                            return Some(canvas::Action::request_redraw().and_capture());
                        }
                        state.pending_node_press = Some(pending);
                    }
                    if let Some(drag) = state.connect_drag.as_mut() {
                        drag.cursor_world = world;
                        return Some(canvas::Action::request_redraw().and_capture());
                    }
                }
                if state.reorder_drag.is_some() {
                    if let Some(position) = cursor_in_bounds {
                        state.drag_last = Some(position);
                        return Some(canvas::Action::request_redraw().and_capture());
                    }
                }
                if let (true, Some(last), Some(position)) =
                    (state.panning, state.drag_last, cursor_in_bounds)
                {
                    let delta = Vector::new(position.x - last.x, position.y - last.y);
                    state.translation = pan_by(state.translation, delta);
                    state.drag_last = Some(position);
                    return Some(canvas::Action::request_redraw().and_capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let position = cursor_in_bounds?;
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 40.0,
                };
                if y.abs() < f32::EPSILON {
                    return None;
                }
                let factor = if y > 0.0 { ZOOM_STEP } else { 1.0 / ZOOM_STEP };
                let (translation, scale) =
                    zoom_at_point(state.translation, state.scale, position, factor);
                state.translation = translation;
                state.scale = scale;
                return Some(canvas::Action::request_redraw().and_capture());
            }
            _ => {}
        }

        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let (translation, scale) = self.camera_for(state, bounds.size());
        let mut frame = Frame::new(renderer, bounds.size());

        frame.translate(translation);
        frame.scale(scale);

        let palette = theme.extended_palette();
        let edge_color = palette.secondary.base.color;
        let selected_edge = palette.primary.strong.color;
        let node_fill = palette.background.weak.color;
        let node_selected = palette.primary.weak.color;
        let missing_fill = palette.danger.weak.color;
        let missing_selected = palette.danger.base.color;
        let node_border = palette.background.strong.color;
        let text_color = palette.background.base.text;
        let muted_text = palette.background.weak.text;
        let port_fill = palette.primary.base.color;
        let draft_color = palette.primary.base.color;

        let stroke_width = 1.5 / scale.max(0.25);

        if self.connection_mode == ConnectionMode::Precise {
            for (edge_id, from, from_port, to, to_port) in &self.edges {
                let Some((start, end)) =
                    self.edge_endpoints(Some(edge_id), from, from_port, to, to_port)
                else {
                    continue;
                };
                let mut path = Builder::new();
                path.move_to(start);
                path.line_to(end);
                let color = if self.selected_edge.as_deref() == Some(edge_id.as_str()) {
                    selected_edge
                } else {
                    edge_color
                };
                frame.stroke(
                    &path.build(),
                    Stroke::default().with_width(stroke_width).with_color(color),
                );
            }
        } else {
            for (from_id, to_id) in &self.layout.edges {
                let Some((start, end)) = self.edge_endpoints(None, from_id, "", to_id, "") else {
                    continue;
                };
                let mut path = Builder::new();
                path.move_to(start);
                path.line_to(end);
                let selected = self.selected_edge.as_ref().is_some_and(|edge_id| {
                    self.edges.iter().any(|(id, f, _, t, _)| {
                        id == edge_id && f == from_id && t == to_id
                    })
                });
                let color = if selected { selected_edge } else { edge_color };
                frame.stroke(
                    &path.build(),
                    Stroke::default().with_width(stroke_width).with_color(color),
                );
            }
        }

        if let Some(drag) = &state.connect_drag {
            if let Some(start) =
                self.connect_source_point(&drag.from_node, drag.from_port.as_deref())
            {
                let mut path = Builder::new();
                path.move_to(start);
                path.line_to(drag.cursor_world);
                frame.stroke(
                    &path.build(),
                    Stroke::default()
                        .with_width(stroke_width)
                        .with_color(Color {
                            a: 0.85,
                            ..draft_color
                        }),
                );
            }
        }

        for node in &self.layout.nodes {
            let size = node.size;
            let connect_source = state
                .connect_drag
                .as_ref()
                .is_some_and(|d| d.from_node == node.id);
            let selected = self.selected_node.as_deref() == Some(node.id.as_str()) || connect_source;
            let fill = if node.missing {
                if selected {
                    missing_selected
                } else {
                    missing_fill
                }
            } else if selected {
                node_selected
            } else {
                node_fill
            };
            let rect = Path::rounded_rectangle(node.position, size, NODE_RADIUS.into());
            frame.fill(&rect, fill);
            frame.stroke(
                &rect,
                Stroke::default()
                    .with_width(1.0 / scale.max(0.25))
                    .with_color(node_border),
            );

            let label = node
                .title
                .as_deref()
                .filter(|title| !title.is_empty())
                .unwrap_or(node.id.as_str());
            frame.fill_text(Text {
                content: label.to_string(),
                position: Point::new(node.position.x + 10.0, node.position.y + 8.0),
                color: text_color,
                size: Pixels(14.0),
                font: Font::DEFAULT,
                ..Text::default()
            });
            let type_label = if node.missing {
                "(missing)".to_string()
            } else {
                node.node_type.clone()
            };
            frame.fill_text(Text {
                content: type_label,
                position: Point::new(node.position.x + 10.0, node.position.y + 26.0),
                color: muted_text,
                size: Pixels(12.0),
                font: Font::DEFAULT,
                ..Text::default()
            });

            if self.connection_mode == ConnectionMode::Precise {
                let slots = self.slots_for(&node.id);
                for (index, slot) in slots.iter().enumerate() {
                    let pos = port_position(node.position, size, index, false);
                    let fill = if matches!(slot, InputSlot::VariadicDummy { .. }) {
                        Color {
                            a: 0.35,
                            ..port_fill
                        }
                    } else {
                        port_fill
                    };
                    frame.fill(&Path::circle(pos, PORT_RADIUS), fill);
                    frame.fill_text(Text {
                        content: slot.label(),
                        position: Point::new(pos.x + PORT_RADIUS + PORT_LABEL_GAP, pos.y),
                        color: muted_text,
                        size: Pixels(PORT_LABEL_SIZE),
                        font: Font::DEFAULT,
                        align_x: alignment::Horizontal::Left.into(),
                        align_y: alignment::Vertical::Center,
                        ..Text::default()
                    });
                }
                if let Some(info) = self.ports.get(&node.id) {
                    for (index, port) in info.outputs.iter().enumerate() {
                        let pos = port_position(node.position, size, index, true);
                        frame.fill(
                            &Path::circle(pos, PORT_RADIUS),
                            Color {
                                a: 0.9,
                                ..port_fill
                            },
                        );
                        frame.fill_text(Text {
                            content: port.clone(),
                            position: Point::new(pos.x - PORT_RADIUS - PORT_LABEL_GAP, pos.y),
                            color: muted_text,
                            size: Pixels(PORT_LABEL_SIZE),
                            font: Font::DEFAULT,
                            align_x: alignment::Horizontal::Right.into(),
                            align_y: alignment::Vertical::Center,
                            ..Text::default()
                        });
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            if state.connect_drag.is_some() {
                mouse::Interaction::Crosshair
            } else {
                mouse::Interaction::Pointer
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::layout_graph;
    use imp_core_types::Graph;
    use std::path::PathBuf;

    #[test]
    fn clamp_scale_limits_range() {
        assert_eq!(clamp_scale(0.01), MIN_SCALE);
        assert_eq!(clamp_scale(10.0), MAX_SCALE);
        assert_eq!(clamp_scale(1.5), 1.5);
    }

    #[test]
    fn pan_by_adds_delta() {
        let next = pan_by(Vector::new(10.0, 20.0), Vector::new(-3.0, 4.0));
        assert_eq!(next, Vector::new(7.0, 24.0));
    }

    #[test]
    fn zoom_at_point_keeps_world_point_stable() {
        let translation = Vector::new(50.0, 40.0);
        let scale = 1.0;
        let cursor = Point::new(100.0, 80.0);
        let (next_t, next_s) = zoom_at_point(translation, scale, cursor, 2.0);
        assert_eq!(next_s, 2.0);

        let world_before = Point::new(
            (cursor.x - translation.x) / scale,
            (cursor.y - translation.y) / scale,
        );
        let world_after = Point::new(
            (cursor.x - next_t.x) / next_s,
            (cursor.y - next_t.y) / next_s,
        );
        assert!((world_before.x - world_after.x).abs() < 1e-4);
        assert!((world_before.y - world_after.y).abs() < 1e-4);
    }

    #[test]
    fn center_translation_puts_content_center_at_viewport_center() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sine.json");
        let text = std::fs::read_to_string(path).expect("read sine fixture");
        let graph: Graph = serde_json::from_str(&text).expect("parse sine fixture");
        let layout = layout_graph(&graph, &BTreeMap::new());
        let viewport = Size::new(800.0, 600.0);
        let translation = center_translation(&layout, viewport, 1.0);
        let center = layout.content_center();
        let screen = Point::new(
            center.x * 1.0 + translation.x,
            center.y * 1.0 + translation.y,
        );
        assert!((screen.x - 400.0).abs() < 1e-3);
        assert!((screen.y - 300.0).abs() < 1e-3);
    }
}
