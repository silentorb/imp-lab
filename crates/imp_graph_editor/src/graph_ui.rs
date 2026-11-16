//! Graph-dag connection-mode preferences and session UI state.

use serde::{Deserialize, Serialize};

/// How connections are drawn and created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConnectionMode {
    /// Node→node with automatic port pairing.
    #[default]
    Simple,
    /// Port handles; drag output→input.
    Precise,
}

impl ConnectionMode {
    /// Stable string id for user-state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Precise => "precise",
        }
    }

    /// Parses a mode id; unknown values return [`None`].
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "simple" => Some(Self::Simple),
            "precise" => Some(Self::Precise),
            _ => None,
        }
    }
}

/// Serializable prefs entry for graph-dag UI (host chooses persistence key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphUiEntry {
    /// Connection UI mode (`simple` | `precise`).
    pub connection_mode: String,
    /// Last node type id chosen in the Add node dialog.
    pub add_node_type: String,
}

impl GraphUiEntry {
    /// Builds an entry with Simple mode and the host's default Add type.
    pub fn new(default_add_type: impl Into<String>) -> Self {
        Self {
            connection_mode: ConnectionMode::Simple.as_str().to_string(),
            add_node_type: default_add_type.into(),
        }
    }
}

/// In-progress connection gesture (session-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectDraft {
    /// Source node id.
    pub from_node: String,
    /// Source output port when in precise mode.
    pub from_port: Option<String>,
}

/// In-memory graph-dag UI state used by the host.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphUiState {
    /// Connection UI mode.
    pub connection_mode: ConnectionMode,
    /// Node type for Add node.
    pub add_node_type: String,
    /// Selected node id, if any.
    pub selected_node: Option<String>,
    /// Selected edge id, if any.
    pub selected_edge: Option<String>,
    /// Draft connection start.
    pub connect_draft: Option<ConnectDraft>,
    /// Title field text when a node is selected (mirrors title until committed).
    pub title_buffer: String,
}

impl GraphUiState {
    /// Builds state with Simple mode and the host's default Add type.
    pub fn new(default_add_type: impl Into<String>) -> Self {
        let default_add_type = default_add_type.into();
        Self::from_entry(&GraphUiEntry::new(default_add_type.clone()), &default_add_type)
    }

    /// Builds state from a persisted entry (invalid fields fall back).
    ///
    /// Empty `add_node_type` in the entry uses `default_add_type`.
    pub fn from_entry(entry: &GraphUiEntry, default_add_type: &str) -> Self {
        let connection_mode = ConnectionMode::parse(&entry.connection_mode).unwrap_or_default();
        let add_node_type = if entry.add_node_type.trim().is_empty() {
            default_add_type.to_string()
        } else {
            entry.add_node_type.clone()
        };
        Self {
            connection_mode,
            add_node_type,
            selected_node: None,
            selected_edge: None,
            connect_draft: None,
            title_buffer: String::new(),
        }
    }

    /// Serializes prefs for host persistence (selection is session-only).
    pub fn to_entry(&self) -> GraphUiEntry {
        GraphUiEntry {
            connection_mode: self.connection_mode.as_str().to_string(),
            add_node_type: self.add_node_type.clone(),
        }
    }

    /// Clears selection and connect draft.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
        self.selected_edge = None;
        self.connect_draft = None;
        self.title_buffer.clear();
    }

    /// Selects a node and seeds the title buffer.
    pub fn select_node(&mut self, node_id: Option<String>, title: impl Into<String>) {
        self.selected_edge = None;
        self.connect_draft = None;
        self.title_buffer = title.into();
        self.selected_node = node_id;
    }

    /// Selects an edge.
    pub fn select_edge(&mut self, edge_id: Option<String>) {
        self.selected_node = None;
        self.connect_draft = None;
        self.title_buffer.clear();
        self.selected_edge = edge_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_trip_defaults() {
        let state = GraphUiState::new("sine");
        let entry = state.to_entry();
        assert_eq!(entry.connection_mode, "simple");
        assert_eq!(entry.add_node_type, "sine");
        let restored = GraphUiState::from_entry(&entry, "sine");
        assert_eq!(restored.connection_mode, ConnectionMode::Simple);
        assert_eq!(restored.add_node_type, "sine");
    }

    #[test]
    fn invalid_entry_fields_fall_back() {
        let entry = GraphUiEntry {
            connection_mode: String::from("weird"),
            add_node_type: String::new(),
        };
        let state = GraphUiState::from_entry(&entry, "sine");
        assert_eq!(state.connection_mode, ConnectionMode::Simple);
        assert_eq!(state.add_node_type, "sine");
    }
}
