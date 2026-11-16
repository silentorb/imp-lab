//! Pure Imp graph mutation helpers for the editor.

use std::collections::{BTreeMap, HashSet};

use imp_core_types::{Edge, Graph, Node, NodeId, PortReference};
use thiserror::Error;

/// Port template used for auto-pairing (host supplies opaque signal keys).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPort {
    /// Port id.
    pub id: String,
    /// Host-defined key; equal keys are treated as compatible signal types.
    pub signal_key: String,
    /// When true, the input accepts multiple ordered inbound edges.
    pub variadic: bool,
}

/// Host-injected lookup for node-type ports (keeps Imp type crates out of the trait API).
pub trait NodeTypeCatalog {
    /// Input ports for `node_type_id`, if the type is known.
    fn inputs(&self, node_type_id: &str) -> Option<Vec<CatalogPort>>;
    /// Output ports for `node_type_id`, if the type is known.
    fn outputs(&self, node_type_id: &str) -> Option<Vec<CatalogPort>>;
}

/// Errors from graph edit helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphEditError {
    /// Node id already exists.
    #[error("node `{0}` already exists")]
    NodeExists(String),
    /// Node id was not found.
    #[error("node `{0}` not found")]
    NodeMissing(String),
    /// Edge id was not found.
    #[error("edge `{0}` not found")]
    EdgeMissing(String),
    /// Rename target collides with an existing id.
    #[error("cannot rename to `{0}`; id already exists")]
    RenameCollision(String),
    /// Empty id is invalid.
    #[error("node id must not be empty")]
    EmptyId,
    /// No compatible free port pair for a simple connection.
    #[error("no compatible free ports between `{from}` and `{to}`")]
    NoPortPair {
        /// Source node.
        from: String,
        /// Target node.
        to: String,
    },
    /// Node type is unknown in the registry (precise connect / auto-pair).
    #[error("unknown node type `{0}`")]
    UnknownType(String),
    /// Reorder listed edges that are not a complete variadic fan-in.
    #[error("invalid variadic reorder for `{node}` port `{port}`")]
    InvalidVariadicReorder {
        /// Target node.
        node: String,
        /// Target port.
        port: String,
    },
}

/// Allocates a unique node id derived from `base` (`sine`, `sine_2`, …).
pub fn unique_node_id(graph: &Graph, base: &str) -> NodeId {
    let base = if base.trim().is_empty() { "node" } else { base };
    if !graph.nodes.contains_key(base) {
        return base.to_string();
    }
    let mut index = 2u32;
    loop {
        let candidate = format!("{base}_{index}");
        if !graph.nodes.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// Allocates a unique display title derived from `base` (`sine`, `sine_2`, …).
///
/// Uniqueness is among existing present titles only; nodes without a title are ignored.
pub fn unique_node_title(graph: &Graph, base: &str) -> String {
    let base = if base.trim().is_empty() { "node" } else { base };
    let titles: std::collections::BTreeSet<&str> = graph
        .nodes
        .values()
        .filter_map(|node| node.title.as_deref())
        .collect();
    if !titles.contains(base) {
        return base.to_string();
    }
    let mut index = 2u32;
    loop {
        let candidate = format!("{base}_{index}");
        if !titles.contains(candidate.as_str()) {
            return candidate;
        }
        index += 1;
    }
}

/// Inserts a node with empty inputs and an optional display title.
pub fn add_node(
    graph: &mut Graph,
    id: impl Into<NodeId>,
    node_type: impl Into<String>,
    title: Option<String>,
) -> Result<NodeId, GraphEditError> {
    let id = id.into();
    if id.trim().is_empty() {
        return Err(GraphEditError::EmptyId);
    }
    if graph.nodes.contains_key(&id) {
        return Err(GraphEditError::NodeExists(id));
    }
    let node_type = node_type.into();
    let title = title.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    graph.nodes.insert(
        id.clone(),
        Node {
            id: id.clone(),
            title,
            node_type,
            type_args: Vec::new(),
            inputs: BTreeMap::new(),
        },
    );
    Ok(id)
}

/// Removes a node and all incident edges.
pub fn remove_node(graph: &mut Graph, node_id: &str) -> Result<(), GraphEditError> {
    if graph.nodes.remove(node_id).is_none() {
        return Err(GraphEditError::NodeMissing(node_id.to_string()));
    }
    graph.edges.retain(|_, edge| {
        edge.from.node != node_id && edge.to.node != node_id
    });
    Ok(())
}

/// Renames a node and rewrites edge endpoints that reference it.
pub fn rename_node(
    graph: &mut Graph,
    old_id: &str,
    new_id: impl Into<NodeId>,
) -> Result<NodeId, GraphEditError> {
    let new_id = new_id.into();
    if new_id.trim().is_empty() {
        return Err(GraphEditError::EmptyId);
    }
    if old_id == new_id {
        return Ok(new_id);
    }
    if graph.nodes.contains_key(&new_id) {
        return Err(GraphEditError::RenameCollision(new_id));
    }
    let Some(mut node) = graph.nodes.remove(old_id) else {
        return Err(GraphEditError::NodeMissing(old_id.to_string()));
    };
    node.id = new_id.clone();
    graph.nodes.insert(new_id.clone(), node);
    for edge in graph.edges.values_mut() {
        if edge.from.node == old_id {
            edge.from.node = new_id.clone();
        }
        if edge.to.node == old_id {
            edge.to.node = new_id.clone();
        }
    }
    Ok(new_id)
}

/// Sets or clears a node's optional display title.
///
/// Empty / whitespace-only values clear the title (`None`).
pub fn set_node_title(
    graph: &mut Graph,
    node_id: &str,
    title: Option<String>,
) -> Result<(), GraphEditError> {
    let Some(node) = graph.nodes.get_mut(node_id) else {
        return Err(GraphEditError::NodeMissing(node_id.to_string()));
    };
    node.title = title.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    Ok(())
}

/// Changes a node's type and clears instance literals / type args.
pub fn set_node_type(
    graph: &mut Graph,
    node_id: &str,
    node_type: impl Into<String>,
) -> Result<(), GraphEditError> {
    let Some(node) = graph.nodes.get_mut(node_id) else {
        return Err(GraphEditError::NodeMissing(node_id.to_string()));
    };
    node.node_type = node_type.into();
    node.type_args.clear();
    node.inputs.clear();
    Ok(())
}

/// Allocates a unique edge id.
pub fn unique_edge_id(graph: &Graph, from: &str, from_port: &str, to: &str, to_port: &str) -> String {
    let base = format!("{from}_{from_port}_to_{to}_{to_port}");
    if !graph.edges.contains_key(&base) {
        return base;
    }
    let mut index = 2u32;
    loop {
        let candidate = format!("{base}_{index}");
        if !graph.edges.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// Next `Edge.order` for a variadic target (max existing + 1, or 0).
pub fn next_variadic_order(graph: &Graph, to_node: &str, to_port: &str) -> i64 {
    graph
        .edges
        .values()
        .filter(|edge| edge.to.node == to_node && edge.to.port == to_port)
        .filter_map(|edge| edge.order)
        .max()
        .map(|order| order + 1)
        .unwrap_or(0)
}

/// Inserts a port-level edge. When `variadic_target` is true, assigns the next order.
pub fn add_edge(
    graph: &mut Graph,
    from_node: impl Into<NodeId>,
    from_port: impl Into<String>,
    to_node: impl Into<NodeId>,
    to_port: impl Into<String>,
    variadic_target: bool,
) -> Result<String, GraphEditError> {
    let from_node = from_node.into();
    let to_node = to_node.into();
    let from_port = from_port.into();
    let to_port = to_port.into();
    if !graph.nodes.contains_key(&from_node) {
        return Err(GraphEditError::NodeMissing(from_node));
    }
    if !graph.nodes.contains_key(&to_node) {
        return Err(GraphEditError::NodeMissing(to_node));
    }
    let order = if variadic_target {
        Some(next_variadic_order(graph, &to_node, &to_port))
    } else {
        None
    };
    let id = unique_edge_id(graph, &from_node, &from_port, &to_node, &to_port);
    graph.edges.insert(
        id.clone(),
        Edge {
            from: PortReference {
                node: from_node,
                port: from_port,
            },
            to: PortReference {
                node: to_node,
                port: to_port,
            },
            order,
        },
    );
    Ok(id)
}

/// Removes one edge by id.
pub fn remove_edge(graph: &mut Graph, edge_id: &str) -> Result<(), GraphEditError> {
    if graph.edges.remove(edge_id).is_none() {
        return Err(GraphEditError::EdgeMissing(edge_id.to_string()));
    }
    Ok(())
}

/// Removes every edge between two nodes (either direction).
pub fn remove_edges_between(graph: &mut Graph, a: &str, b: &str) -> usize {
    let before = graph.edges.len();
    graph.edges.retain(|_, edge| {
        let ab = edge.from.node == a && edge.to.node == b;
        let ba = edge.from.node == b && edge.to.node == a;
        !(ab || ba)
    });
    before - graph.edges.len()
}

/// Removes every edge attached to `node_id`/`port_id` as an output or input.
pub fn remove_edges_at_port(
    graph: &mut Graph,
    node_id: &str,
    port_id: &str,
    is_output: bool,
) -> usize {
    let before = graph.edges.len();
    graph.edges.retain(|_, edge| {
        if is_output {
            !(edge.from.node == node_id && edge.from.port == port_id)
        } else {
            !(edge.to.node == node_id && edge.to.port == port_id)
        }
    });
    before - graph.edges.len()
}

/// Rewrites `Edge.order` for a variadic fan-in to match `ordered_edge_ids` (`0..n-1`).
pub fn reorder_variadic_inputs(
    graph: &mut Graph,
    node_id: &str,
    port_id: &str,
    ordered_edge_ids: &[String],
) -> Result<(), GraphEditError> {
    let mut current: Vec<String> = graph
        .edges
        .iter()
        .filter(|(_, edge)| edge.to.node == node_id && edge.to.port == port_id)
        .map(|(id, _)| id.clone())
        .collect();
    current.sort();
    let mut expected = ordered_edge_ids.to_vec();
    expected.sort();
    if current != expected {
        return Err(GraphEditError::InvalidVariadicReorder {
            node: node_id.into(),
            port: port_id.into(),
        });
    }
    for (index, edge_id) in ordered_edge_ids.iter().enumerate() {
        let Some(edge) = graph.edges.get_mut(edge_id) else {
            return Err(GraphEditError::EdgeMissing(edge_id.clone()));
        };
        edge.order = Some(index as i64);
    }
    Ok(())
}

/// Picks an unused compatible output→input port pair for a simple connection.
///
/// Singleton inputs are preferred while free. Variadic inputs remain available
/// for additional wires after they already have inbound edges.
pub fn auto_pair_ports(
    catalog: &impl NodeTypeCatalog,
    graph: &Graph,
    from_node: &str,
    to_node: &str,
) -> Result<(String, String, bool), GraphEditError> {
    let from = graph
        .nodes
        .get(from_node)
        .ok_or_else(|| GraphEditError::NodeMissing(from_node.to_string()))?;
    let to = graph
        .nodes
        .get(to_node)
        .ok_or_else(|| GraphEditError::NodeMissing(to_node.to_string()))?;

    let outputs = catalog
        .outputs(&from.node_type)
        .ok_or_else(|| GraphEditError::UnknownType(from.node_type.clone()))?;
    let inputs = catalog
        .inputs(&to.node_type)
        .ok_or_else(|| GraphEditError::UnknownType(to.node_type.clone()))?;

    let used_singleton_inputs: HashSet<&str> = graph
        .edges
        .values()
        .filter(|edge| edge.to.node == to_node)
        .map(|edge| edge.to.port.as_str())
        .collect();

    let available: Vec<&CatalogPort> = inputs
        .iter()
        .filter(|port| {
            if port.variadic {
                true
            } else {
                !used_singleton_inputs.contains(port.id.as_str())
            }
        })
        .collect();

    // Prefer matching signal keys on free singleton inputs first.
    let free_singletons: Vec<&CatalogPort> = available
        .iter()
        .copied()
        .filter(|port| !port.variadic)
        .collect();
    for out in &outputs {
        for inp in &free_singletons {
            if out.signal_key == inp.signal_key {
                return Ok((out.id.clone(), inp.id.clone(), false));
            }
        }
    }
    if let (Some(out), Some(inp)) = (outputs.first(), free_singletons.first()) {
        return Ok((out.id.clone(), inp.id.clone(), false));
    }

    // Then variadic inputs (append).
    let variadic: Vec<&CatalogPort> = available
        .iter()
        .copied()
        .filter(|port| port.variadic)
        .collect();
    for out in &outputs {
        for inp in &variadic {
            if out.signal_key == inp.signal_key {
                return Ok((out.id.clone(), inp.id.clone(), true));
            }
        }
    }
    if let (Some(out), Some(inp)) = (outputs.first(), variadic.first()) {
        return Ok((out.id.clone(), inp.id.clone(), true));
    }

    Err(GraphEditError::NoPortPair {
        from: from_node.to_string(),
        to: to_node.to_string(),
    })
}

/// Expanded input slot used for Precise-mode layout and hit-testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSlot {
    /// Ordinary singleton catalog input.
    Singleton {
        /// Catalog port id.
        port_id: String,
    },
    /// One inbound edge on a variadic catalog input.
    Variadic {
        /// Catalog port id.
        port_id: String,
        /// Edge contributing this slot.
        edge_id: String,
        /// Edge order among siblings.
        order: i64,
    },
    /// Trailing empty slot for appending a new variadic connection.
    VariadicDummy {
        /// Catalog port id.
        port_id: String,
    },
}

impl InputSlot {
    /// Catalog port id for this slot.
    pub fn port_id(&self) -> &str {
        match self {
            Self::Singleton { port_id }
            | Self::Variadic { port_id, .. }
            | Self::VariadicDummy { port_id } => port_id,
        }
    }

    /// Whether Precise connect may target this slot.
    pub fn accepts_connect(&self) -> bool {
        matches!(self, Self::Singleton { .. } | Self::VariadicDummy { .. })
    }

    /// Label drawn beside the port handle.
    pub fn label(&self) -> String {
        match self {
            Self::Singleton { port_id } => port_id.clone(),
            Self::Variadic { port_id, order, .. } => format!("{port_id} {order}"),
            Self::VariadicDummy { port_id } => format!("{port_id}+"),
        }
    }
}

/// Expands catalog inputs into ordered visual slots (variadic filled + trailing dummy).
pub fn expand_input_slots(
    catalog_inputs: &[CatalogPort],
    graph: &Graph,
    node_id: &str,
) -> Vec<InputSlot> {
    let mut slots = Vec::new();
    for port in catalog_inputs {
        if !port.variadic {
            slots.push(InputSlot::Singleton {
                port_id: port.id.clone(),
            });
            continue;
        }
        let mut edges: Vec<(String, i64)> = graph
            .edges
            .iter()
            .filter(|(_, edge)| edge.to.node == node_id && edge.to.port == port.id)
            .map(|(id, edge)| (id.clone(), edge.order.unwrap_or(0)))
            .collect();
        edges.sort_by_key(|(_, order)| *order);
        for (edge_id, order) in edges {
            slots.push(InputSlot::Variadic {
                port_id: port.id.clone(),
                edge_id,
                order,
            });
        }
        slots.push(InputSlot::VariadicDummy {
            port_id: port.id.clone(),
        });
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct StubCatalog {
        inputs: BTreeMap<String, Vec<CatalogPort>>,
        outputs: BTreeMap<String, Vec<CatalogPort>>,
    }

    impl NodeTypeCatalog for StubCatalog {
        fn inputs(&self, node_type_id: &str) -> Option<Vec<CatalogPort>> {
            self.inputs.get(node_type_id).cloned()
        }

        fn outputs(&self, node_type_id: &str) -> Option<Vec<CatalogPort>> {
            self.outputs.get(node_type_id).cloned()
        }
    }

    fn port(id: &str, signal_key: &str) -> CatalogPort {
        CatalogPort {
            id: id.into(),
            signal_key: signal_key.into(),
            variadic: false,
        }
    }

    fn variadic(id: &str, signal_key: &str) -> CatalogPort {
        CatalogPort {
            id: id.into(),
            signal_key: signal_key.into(),
            variadic: true,
        }
    }

    fn stub_catalog() -> StubCatalog {
        let mut inputs = BTreeMap::new();
        let mut outputs = BTreeMap::new();
        outputs.insert("parameter".into(), vec![port("value", "Sample")]);
        inputs.insert(
            "sine".into(),
            vec![port("frequency", "Sample"), port("time", "Time")],
        );
        outputs.insert("sine".into(), vec![port("signal", "Sample")]);
        inputs.insert("add".into(), vec![variadic("inputs", "Sample")]);
        outputs.insert("add".into(), vec![port("audio", "Sample")]);
        StubCatalog { inputs, outputs }
    }

    fn empty_graph() -> Graph {
        Graph {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }

    #[test]
    fn unique_node_id_increments() {
        let mut graph = empty_graph();
        add_node(&mut graph, "sine", "sine", None).unwrap();
        assert_eq!(unique_node_id(&graph, "sine"), "sine_2");
        add_node(&mut graph, "sine_2", "sine", None).unwrap();
        assert_eq!(unique_node_id(&graph, "sine"), "sine_3");
    }

    #[test]
    fn unique_node_title_increments_among_titles() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "sine", Some("sine".into())).unwrap();
        assert_eq!(unique_node_title(&graph, "sine"), "sine_2");
        add_node(&mut graph, "b", "sine", Some("sine_2".into())).unwrap();
        assert_eq!(unique_node_title(&graph, "sine"), "sine_3");
        // Untitled nodes do not reserve the title namespace.
        add_node(&mut graph, "c", "sine", None).unwrap();
        assert_eq!(unique_node_title(&graph, "other"), "other");
    }

    #[test]
    fn set_node_title_clears_blank() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "sine", Some("osc".into())).unwrap();
        set_node_title(&mut graph, "a", Some("  ".into())).unwrap();
        assert_eq!(graph.nodes["a"].title, None);
        set_node_title(&mut graph, "a", Some("freq".into())).unwrap();
        assert_eq!(graph.nodes["a"].title.as_deref(), Some("freq"));
    }

    #[test]
    fn remove_node_drops_incident_edges() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "sine", None).unwrap();
        add_edge(&mut graph, "a", "value", "b", "frequency", false).unwrap();
        remove_node(&mut graph, "a").unwrap();
        assert!(!graph.nodes.contains_key("a"));
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn rename_rewrites_edges() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "sine", None).unwrap();
        add_edge(&mut graph, "a", "value", "b", "frequency", false).unwrap();
        rename_node(&mut graph, "a", "freq").unwrap();
        let edge = graph.edges.values().next().unwrap();
        assert_eq!(edge.from.node, "freq");
    }

    #[test]
    fn auto_pair_prefers_matching_types() {
        let catalog = stub_catalog();
        let mut graph = empty_graph();
        add_node(&mut graph, "freq", "parameter", None).unwrap();
        add_node(&mut graph, "osc", "sine", None).unwrap();
        let (from_port, to_port, variadic) =
            auto_pair_ports(&catalog, &graph, "freq", "osc").expect("pair");
        assert_eq!(from_port, "value");
        assert_eq!(to_port, "frequency");
        assert!(!variadic);
    }

    #[test]
    fn remove_edges_at_port_clears_output_or_input() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "sine", None).unwrap();
        add_node(&mut graph, "c", "sine", None).unwrap();
        add_edge(&mut graph, "a", "value", "b", "frequency", false).unwrap();
        add_edge(&mut graph, "a", "value", "c", "frequency", false).unwrap();
        assert_eq!(remove_edges_at_port(&mut graph, "a", "value", true), 2);
        assert!(graph.edges.is_empty());
        add_edge(&mut graph, "a", "value", "b", "frequency", false).unwrap();
        assert_eq!(remove_edges_at_port(&mut graph, "b", "frequency", false), 1);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn remove_edges_between_clears_pair() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "sine", None).unwrap();
        add_edge(&mut graph, "a", "value", "b", "frequency", false).unwrap();
        add_edge(&mut graph, "a", "value", "b", "time", false).unwrap();
        assert_eq!(remove_edges_between(&mut graph, "a", "b"), 2);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn add_edge_assigns_variadic_order() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "parameter", None).unwrap();
        add_node(&mut graph, "mix", "add", None).unwrap();
        let first = add_edge(&mut graph, "a", "value", "mix", "inputs", true).unwrap();
        let second = add_edge(&mut graph, "b", "value", "mix", "inputs", true).unwrap();
        assert_eq!(graph.edges.get(&first).unwrap().order, Some(0));
        assert_eq!(graph.edges.get(&second).unwrap().order, Some(1));
    }

    #[test]
    fn auto_pair_appends_to_variadic() {
        let catalog = stub_catalog();
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "parameter", None).unwrap();
        add_node(&mut graph, "mix", "add", None).unwrap();
        let (from_port, to_port, variadic) =
            auto_pair_ports(&catalog, &graph, "a", "mix").expect("first");
        assert_eq!((from_port.as_str(), to_port.as_str(), variadic), ("value", "inputs", true));
        add_edge(&mut graph, "a", from_port, "mix", to_port, true).unwrap();
        let (from_port, to_port, variadic) =
            auto_pair_ports(&catalog, &graph, "b", "mix").expect("second");
        assert_eq!((from_port.as_str(), to_port.as_str(), variadic), ("value", "inputs", true));
    }

    #[test]
    fn expand_input_slots_adds_dummy() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "mix", "add", None).unwrap();
        add_edge(&mut graph, "a", "value", "mix", "inputs", true).unwrap();
        let slots = expand_input_slots(&[variadic("inputs", "Sample")], &graph, "mix");
        assert_eq!(slots.len(), 2);
        assert!(matches!(
            &slots[0],
            InputSlot::Variadic {
                port_id,
                order: 0,
                ..
            } if port_id == "inputs"
        ));
        assert!(matches!(
            &slots[1],
            InputSlot::VariadicDummy { port_id } if port_id == "inputs"
        ));
    }

    #[test]
    fn reorder_variadic_inputs_rewrites_order() {
        let mut graph = empty_graph();
        add_node(&mut graph, "a", "parameter", None).unwrap();
        add_node(&mut graph, "b", "parameter", None).unwrap();
        add_node(&mut graph, "mix", "add", None).unwrap();
        let first = add_edge(&mut graph, "a", "value", "mix", "inputs", true).unwrap();
        let second = add_edge(&mut graph, "b", "value", "mix", "inputs", true).unwrap();
        reorder_variadic_inputs(
            &mut graph,
            "mix",
            "inputs",
            &[second.clone(), first.clone()],
        )
        .unwrap();
        assert_eq!(graph.edges.get(&second).unwrap().order, Some(0));
        assert_eq!(graph.edges.get(&first).unwrap().order, Some(1));
    }
}
