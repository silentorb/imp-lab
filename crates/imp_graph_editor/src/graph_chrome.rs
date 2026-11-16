//! Lightweight toolbar / properties widget building blocks.
//!
//! Hosts compose these into a full chrome story (panes, commands, menus).

use iced::widget::{button, checkbox, column, pick_list, row, text, text_input, Space};
use iced::{Element, Fill, Length, Renderer};
use native_theme_iced::{
    at_least, button_content_min_size, button_padding, combo_box_padding, control_line_height,
    font_size, input_padding, pick_list_handle, scaled_text_size, styles, AccessibilityPreferences,
    ResolvedTheme,
};

use crate::graph_ui::ConnectionMode;

/// Messages emitted by graph chrome building blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphChromeMessage {
    /// Choose the Add-node dialog type.
    SetAddNodeType(String),
    /// Confirm Add-node dialog (insert with current type).
    AddNodeConfirm,
    /// Cancel Add-node dialog.
    AddNodeCancel,
    /// Toggle simple vs precise connection UI.
    SetConnectionMode(ConnectionMode),
    /// Title buffer edited.
    TitleInput(String),
    /// Commit the title buffer.
    TitleCommit,
    /// Change the selected node's type.
    SelectedTypeChanged(String),
}

/// Precise-ports checkbox (hosts may wrap this in a menu).
pub fn precise_ports_checkbox<'a>(
    precise: bool,
    resolved: &'a ResolvedTheme,
) -> Element<'a, GraphChromeMessage> {
    checkbox(precise)
        .label("Precise ports")
        .on_toggle(|enabled| {
            GraphChromeMessage::SetConnectionMode(if enabled {
                ConnectionMode::Precise
            } else {
                ConnectionMode::Simple
            })
        })
        .size(resolved.checkbox.indicator_width)
        .spacing(resolved.checkbox.label_gap)
        .style(styles::checkbox(resolved))
        .width(Length::Fill)
        .into()
}

/// Add-node type picker dialog body (pick list + Add / Cancel).
pub fn add_node_type_dialog<'a>(
    type_ids: Vec<String>,
    selected: String,
    resolved: &'a ResolvedTheme,
    accessibility: &'a AccessibilityPreferences,
) -> Element<'a, GraphChromeMessage> {
    let body = font_size(resolved, accessibility);
    let size = scaled_text_size(resolved.combo_box.font.size, accessibility);
    let padding = combo_box_padding(resolved);

    column![
        text("Add node").size(body),
        text("Type:").size(body),
        pick_list(type_ids, Some(selected), GraphChromeMessage::SetAddNodeType)
            .text_size(size)
            .padding(padding)
            .text_line_height(control_line_height(
                resolved,
                size,
                resolved.combo_box.min_height,
                padding,
            ))
            .handle(pick_list_handle::<Renderer>(resolved))
            .style(styles::pick_list(resolved))
            .menu_style(styles::menu(resolved))
            .width(Length::Fill),
        row![
            button(at_least(text("Cancel"), button_content_min_size(resolved)))
                .on_press(GraphChromeMessage::AddNodeCancel)
                .padding(button_padding(resolved))
                .style(styles::button(resolved)),
            Space::new().width(Fill),
            button(at_least(text("Add"), button_content_min_size(resolved)))
                .on_press(GraphChromeMessage::AddNodeConfirm)
                .padding(button_padding(resolved))
                .style(styles::button_primary(resolved)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(12)
    .padding(16)
    .into()
}

/// Title editor plus read-only id and type labels for a selected node.
pub fn node_properties_editor<'a>(
    node_id: &str,
    title_buffer: &str,
    node_type: &str,
    resolved: &'a ResolvedTheme,
    accessibility: &'a AccessibilityPreferences,
) -> Element<'a, GraphChromeMessage> {
    let body = font_size(resolved, accessibility);
    let input_size = scaled_text_size(resolved.input.font.size, accessibility);
    let input_pad = input_padding(resolved);
    let header = if title_buffer.trim().is_empty() {
        format!("Node: {node_id}")
    } else {
        format!("Node: {title_buffer}")
    };

    column![
        text(header).size(body),
        text("Title:").size(body),
        text_input("node title", title_buffer)
            .on_input(GraphChromeMessage::TitleInput)
            .on_submit(GraphChromeMessage::TitleCommit)
            .size(input_size)
            .padding(input_pad)
            .line_height(control_line_height(
                resolved,
                input_size,
                resolved.input.min_height,
                input_pad,
            ))
            .style(styles::text_input(resolved))
            .width(Length::Fill),
        text("Id:").size(body),
        text(node_id.to_string()).size(body),
        text("Type:").size(body),
        text(node_type.to_string()).size(body),
    ]
    .spacing(8)
    .into()
}
