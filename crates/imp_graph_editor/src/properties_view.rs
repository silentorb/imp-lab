//! Data-driven two-column properties table (label | value).
//!
//! Thin wrapper over Iced's built-in [`table`](iced::widget::table) so hosts
//! map a collection of property key/value pairs without hand-rolling layout.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use iced::widget::{table, text, Space};
use iced::{Element, Fill, Length};
use native_theme_iced::{font_size, AccessibilityPreferences, ResolvedTheme};

/// One property row: a label key and a value widget.
///
/// Iced's `table` clones each row once per column. `Element` is not `Clone`, so
/// the value is held behind shared ownership and taken when the value column
/// builds its cell (label column runs first and only reads `key`).
pub struct Property<'a, Message> {
    key: String,
    value: Rc<RefCell<Option<Element<'a, Message>>>>,
}

impl<'a, Message> Property<'a, Message> {
    /// Builds a property row from a key and value element.
    pub fn new(
        key: impl Into<Cow<'a, str>>,
        value: impl Into<Element<'a, Message>>,
    ) -> Self {
        Self {
            key: key.into().into_owned(),
            value: Rc::new(RefCell::new(Some(value.into()))),
        }
    }
}

impl<Message> Clone for Property<'_, Message> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: Rc::clone(&self.value),
        }
    }
}

fn take_value<'a, Message: 'a>(
    property: &Property<'a, Message>,
) -> Element<'a, Message> {
    property
        .value
        .borrow_mut()
        .take()
        .unwrap_or_else(|| Space::new().into())
}

/// Renders an ordered collection of properties as a two-column table.
///
/// The first property occupies Iced's required header row (same widgets as a
/// body row — the table does not style headers differently). Remaining
/// properties are body rows. The label column shrinks to fit keys; the value
/// column fills remaining width so variable value content wraps or clips.
pub fn properties_view<'a, Message: 'a>(
    properties: impl IntoIterator<Item = Property<'a, Message>>,
    resolved: &ResolvedTheme,
    accessibility: &AccessibilityPreferences,
) -> Element<'a, Message> {
    let body = font_size(resolved, accessibility);
    let line = resolved.separator.line_width.max(1.0);
    let mut rows: Vec<Property<'a, Message>> = properties.into_iter().collect();
    if rows.is_empty() {
        return Space::new().width(Fill).into();
    }

    let first = rows.remove(0);
    let header_value = take_value(&first);
    let header_key = text(first.key).size(body);

    table(
        [
            table::column(header_key, move |property: Property<'a, Message>| {
                text(property.key).size(body)
            })
            .align_y(iced::Alignment::Center),
            table::column(header_value, |property: Property<'a, Message>| {
                take_value(&property)
            })
            .width(Fill)
            .align_y(iced::Alignment::Center),
        ],
        rows,
    )
    .width(Length::Fill)
    .padding_x(6)
    .padding_y(4)
    .separator(line)
    .into()
}

#[cfg(test)]
mod tests {
    use iced::widget::text;
    use iced_test::simulator;
    use native_theme_iced::{from_preset, AccessibilityPreferences};

    use super::{properties_view, Property};

    #[test]
    fn properties_view_renders_supplied_keys_and_values() {
        let (_, resolved) = from_preset("adwaita", false).expect("preset");
        let accessibility = AccessibilityPreferences::default();
        let view = properties_view::<()>(
            [
                Property::new("Alpha", text("one")),
                Property::new("Beta", text("two")),
            ],
            &resolved,
            &accessibility,
        );

        let mut ui = simulator(view);
        assert!(ui.find("Alpha").is_ok(), "label column should show Alpha");
        assert!(ui.find("Beta").is_ok(), "label column should show Beta");
        assert!(ui.find("one").is_ok(), "value column should show one");
        assert!(ui.find("two").is_ok(), "value column should show two");
    }

    #[test]
    fn properties_view_renders_single_property_as_header_row() {
        let (_, resolved) = from_preset("adwaita", false).expect("preset");
        let accessibility = AccessibilityPreferences::default();
        let view = properties_view::<()>(
            [Property::new("Only", text("value"))],
            &resolved,
            &accessibility,
        );

        let mut ui = simulator(view);
        assert!(ui.find("Only").is_ok());
        assert!(ui.find("value").is_ok());
    }
}
