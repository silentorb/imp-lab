//! Extension settings: settings, settings categories, and setting → category associations are
//! independent components. Each category is one settings page; settings without a category go on
//! a fallback page. Status items display a topic.
//!
//! Lab draws settings from these declarations without knowing what they mean; values are opaque
//! JSON stored by the host and delivered to the owning service.

use crate::config::{ComponentConfig, ComponentKind, ExtensionsConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Title of the page for settings that no category claims.
pub const FALLBACK_PAGE_TITLE: &str = "Other";

/// How a setting's value is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingValueType {
    /// One value from `options` (a string).
    Choice,
    /// Several values from `options` (an array of strings).
    MultiChoice,
    /// On / off.
    Boolean,
}

/// One selectable option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}

/// A `setting` component.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDecl {
    pub id: String,
    pub extension_id: String,
    pub label: String,
    pub description: Option<String>,
    pub value_type: SettingValueType,
    pub default: Value,
    /// Static options (choice / multi-choice).
    pub options: Option<Vec<SettingOption>>,
    /// Topic whose value is `[{value, label}]`, for options that change at runtime.
    pub options_topic: Option<String>,
    /// Least number of selected values (multi-choice).
    pub min_selected: Option<u32>,
    /// Implementation id of the service that receives this setting's value.
    pub service_id: Option<String>,
}

/// A `settings-category` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCategoryDecl {
    pub id: String,
    pub title: String,
}

/// A `settings-category-item` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCategoryItemDecl {
    pub setting_id: String,
    pub category_id: String,
    pub order: i64,
}

/// A `status-item` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusItemDecl {
    pub id: String,
    pub label: Option<String>,
    /// Topic whose value is `{text, level}` (`level`: `ok` | `warn` | `error` | `off`).
    pub topic: String,
    /// Settings category to open when the item is clicked.
    pub open_category: Option<String>,
    pub order: i64,
}

/// One settings page: a category, or the fallback page (`category_id: None`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPage {
    pub category_id: Option<String>,
    pub title: String,
    pub settings: Vec<SettingDecl>,
}

/// Everything the Settings dialog draws, plus problems found in the declarations.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLayout {
    pub pages: Vec<SettingsPage>,
    pub warnings: Vec<String>,
}

/// Enabled components of `kind` whose extension is enabled.
fn enabled_components(
    config: &ExtensionsConfig,
    kind: ComponentKind,
) -> impl Iterator<Item = (&ComponentConfig, BTreeMap<String, Value>)> {
    config.components.iter().filter_map(move |component| {
        if !component.enabled || component.kind != kind {
            return None;
        }
        let extension = config
            .extensions
            .iter()
            .find(|e| e.id == component.extension_id);
        if extension.is_some_and(|e| !e.enabled) {
            return None;
        }
        Some((component, component.merged_params(extension)))
    })
}

fn param_str(params: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Service component id → its implementation id.
fn service_implementation(config: &ExtensionsConfig, component_id: &str) -> Option<String> {
    enabled_components(config, ComponentKind::Service)
        .find(|(c, _)| c.id == component_id)
        .map(|(c, _)| c.implementation_id.clone())
}

/// The enabled settings, with problems.
pub fn setting_decls(config: &ExtensionsConfig) -> (Vec<SettingDecl>, Vec<String>) {
    let mut warnings = Vec::new();
    let settings = enabled_components(config, ComponentKind::Setting)
        .filter_map(|(c, params)| {
            let value_type = match params
                .get("valueType")
                .cloned()
                .map(serde_json::from_value::<SettingValueType>)
            {
                Some(Ok(value_type)) => value_type,
                _ => {
                    warnings.push(format!(
                        "setting `{}` has no valid params.valueType (choice | multi-choice | boolean)",
                        c.id
                    ));
                    return None;
                }
            };
            let options = match params.get("options") {
                Some(value) => match serde_json::from_value::<Vec<SettingOption>>(value.clone()) {
                    Ok(options) => Some(options),
                    Err(_) => {
                        warnings.push(format!("setting `{}` has invalid params.options", c.id));
                        None
                    }
                },
                None => None,
            };
            let service_id = param_str(&params, "serviceId").and_then(|component_id| {
                let found = service_implementation(config, &component_id);
                if found.is_none() {
                    warnings.push(format!(
                        "setting `{}` names unknown service `{component_id}`",
                        c.id
                    ));
                }
                found
            });
            let default = params.get("default").cloned().unwrap_or(match value_type {
                SettingValueType::Choice => Value::Null,
                SettingValueType::MultiChoice => Value::Array(Vec::new()),
                SettingValueType::Boolean => Value::Bool(false),
            });
            Some(SettingDecl {
                id: c.id.clone(),
                extension_id: c.extension_id.clone(),
                label: c.label.clone().unwrap_or_else(|| c.id.clone()),
                description: param_str(&params, "description"),
                value_type,
                default,
                options,
                options_topic: param_str(&params, "optionsTopic"),
                min_selected: params
                    .get("minSelected")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok()),
                service_id,
            })
        })
        .collect();
    (settings, warnings)
}

/// Arranges the enabled settings into pages: one per category (by title), settings ordered by
/// each association's `order` then label, and a trailing fallback page for settings no category
/// claims (omitted when empty). A setting associated with several categories appears on each.
pub fn settings_layout(config: &ExtensionsConfig) -> SettingsLayout {
    let (settings, mut warnings) = setting_decls(config);
    let by_id: BTreeMap<&str, &SettingDecl> = settings.iter().map(|s| (s.id.as_str(), s)).collect();

    let mut categories: Vec<SettingsCategoryDecl> =
        enabled_components(config, ComponentKind::SettingsCategory)
            .map(|(c, params)| SettingsCategoryDecl {
                id: c.id.clone(),
                title: c
                    .label
                    .clone()
                    .or_else(|| param_str(&params, "title"))
                    .unwrap_or_else(|| c.id.clone()),
            })
            .collect();
    categories.sort_by(|a, b| a.title.cmp(&b.title).then(a.id.cmp(&b.id)));
    let category_ids: BTreeSet<&str> = categories.iter().map(|c| c.id.as_str()).collect();

    let mut items: Vec<SettingsCategoryItemDecl> = Vec::new();
    for (c, params) in enabled_components(config, ComponentKind::SettingsCategoryItem) {
        let (Some(setting_id), Some(category_id)) = (
            param_str(&params, "settingId"),
            param_str(&params, "categoryId"),
        ) else {
            warnings.push(format!(
                "settings-category-item `{}` needs params.settingId and params.categoryId",
                c.id
            ));
            continue;
        };
        if !by_id.contains_key(setting_id.as_str()) {
            warnings.push(format!(
                "settings-category-item `{}` names unknown setting `{setting_id}`",
                c.id
            ));
            continue;
        }
        if !category_ids.contains(category_id.as_str()) {
            warnings.push(format!(
                "settings-category-item `{}` names unknown category `{category_id}`",
                c.id
            ));
            continue;
        }
        items.push(SettingsCategoryItemDecl {
            setting_id,
            category_id,
            order: params.get("order").and_then(Value::as_i64).unwrap_or(0),
        });
    }

    let mut pages = Vec::new();
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    for category in &categories {
        let mut members: Vec<(&SettingsCategoryItemDecl, &SettingDecl)> = items
            .iter()
            .filter(|item| item.category_id == category.id)
            .filter_map(|item| Some((item, *by_id.get(item.setting_id.as_str())?)))
            .collect();
        members.sort_by(|(a, sa), (b, sb)| a.order.cmp(&b.order).then(sa.label.cmp(&sb.label)));
        let mut seen = BTreeSet::new();
        let page_settings: Vec<SettingDecl> = members
            .into_iter()
            .filter(|(_, s)| seen.insert(s.id.as_str()))
            .map(|(_, s)| {
                placed.insert(s.id.as_str());
                s.clone()
            })
            .collect();
        pages.push(SettingsPage {
            category_id: Some(category.id.clone()),
            title: category.title.clone(),
            settings: page_settings,
        });
    }

    let mut unplaced: Vec<SettingDecl> = settings
        .iter()
        .filter(|s| !placed.contains(s.id.as_str()))
        .cloned()
        .collect();
    unplaced.sort_by(|a, b| a.label.cmp(&b.label));
    if !unplaced.is_empty() {
        pages.push(SettingsPage {
            category_id: None,
            title: FALLBACK_PAGE_TITLE.to_string(),
            settings: unplaced,
        });
    }

    SettingsLayout { pages, warnings }
}

/// The enabled status items, by `order` then id. Items without `params.topic` are skipped.
pub fn status_items(config: &ExtensionsConfig) -> Vec<StatusItemDecl> {
    let mut items: Vec<StatusItemDecl> = enabled_components(config, ComponentKind::StatusItem)
        .filter_map(|(c, params)| {
            Some(StatusItemDecl {
                id: c.id.clone(),
                label: c.label.clone(),
                topic: param_str(&params, "topic")?,
                open_category: param_str(&params, "openCategory"),
                order: params.get("order").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect();
    items.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_extensions_json;

    fn config(components: &str) -> ExtensionsConfig {
        parse_extensions_json(&format!(
            r#"{{
              "version": 1,
              "extensions": [
                {{"id": "a", "enabled": true}},
                {{"id": "b", "enabled": true}},
                {{"id": "off", "enabled": false}}
              ],
              "components": [
                {{"id": "svc", "extensionId": "a", "kind": "service", "implementationId": "playback"}},
                {components}
              ]
            }}"#
        ))
        .expect("config")
    }

    const SETTINGS: &str = r#"
        {"id": "a.device", "extensionId": "a", "kind": "setting", "label": "Device",
         "params": {"valueType": "choice", "optionsTopic": "a.ports", "serviceId": "svc", "default": ""}},
        {"id": "a.channels", "extensionId": "a", "kind": "setting", "label": "Channels",
         "params": {"valueType": "multi-choice", "minSelected": 1,
                    "options": [{"value": "1", "label": "1"}, {"value": "2", "label": "2"}]}},
        {"id": "a.flag", "extensionId": "a", "kind": "setting", "label": "Flag",
         "params": {"valueType": "boolean"}},
        {"id": "a.midi", "extensionId": "a", "kind": "settings-category", "label": "MIDI"}
    "#;

    fn titles(layout: &SettingsLayout) -> Vec<(String, Vec<String>)> {
        layout
            .pages
            .iter()
            .map(|p| {
                (
                    p.title.clone(),
                    p.settings.iter().map(|s| s.id.clone()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn settings_register_independently_and_fall_back_to_other() {
        let layout = settings_layout(&config(SETTINGS));
        assert_eq!(
            titles(&layout),
            vec![
                ("MIDI".into(), vec![]),
                (
                    FALLBACK_PAGE_TITLE.into(),
                    vec!["a.channels".into(), "a.device".into(), "a.flag".into()]
                ),
            ]
        );
        let device = &layout.pages[1].settings[1];
        assert_eq!(device.value_type, SettingValueType::Choice);
        assert_eq!(device.options_topic.as_deref(), Some("a.ports"));
        assert_eq!(device.service_id.as_deref(), Some("playback"));
        assert!(layout.warnings.is_empty(), "{:?}", layout.warnings);
    }

    #[test]
    fn associations_from_any_extension_place_settings_on_pages_in_order() {
        let layout = settings_layout(&config(&format!(
            r#"{SETTINGS},
            {{"id": "b.general", "extensionId": "b", "kind": "settings-category", "label": "General"}},
            {{"id": "i1", "extensionId": "a", "kind": "settings-category-item",
              "params": {{"settingId": "a.device", "categoryId": "a.midi", "order": 10}}}},
            {{"id": "i2", "extensionId": "a", "kind": "settings-category-item",
              "params": {{"settingId": "a.channels", "categoryId": "a.midi", "order": 20}}}},
            {{"id": "i3", "extensionId": "b", "kind": "settings-category-item",
              "params": {{"settingId": "a.device", "categoryId": "b.general"}}}}"#
        )));
        assert_eq!(
            titles(&layout),
            vec![
                ("General".into(), vec!["a.device".into()]),
                ("MIDI".into(), vec!["a.device".into(), "a.channels".into()]),
                (FALLBACK_PAGE_TITLE.into(), vec!["a.flag".into()]),
            ]
        );
    }

    #[test]
    fn dangling_associations_warn_and_empty_fallback_is_omitted() {
        let layout = settings_layout(&config(
            r#"
            {"id": "a.flag", "extensionId": "a", "kind": "setting", "params": {"valueType": "boolean"}},
            {"id": "a.cat", "extensionId": "a", "kind": "settings-category", "params": {"title": "Cat"}},
            {"id": "ok", "extensionId": "a", "kind": "settings-category-item",
             "params": {"settingId": "a.flag", "categoryId": "a.cat"}},
            {"id": "bad1", "extensionId": "a", "kind": "settings-category-item",
             "params": {"settingId": "missing", "categoryId": "a.cat"}},
            {"id": "bad2", "extensionId": "a", "kind": "settings-category-item",
             "params": {"settingId": "a.flag", "categoryId": "missing"}},
            {"id": "bad3", "extensionId": "a", "kind": "setting", "params": {"valueType": "slider"}}
            "#,
        ));
        assert_eq!(titles(&layout), vec![("Cat".into(), vec!["a.flag".into()])]);
        assert_eq!(layout.warnings.len(), 3, "{:?}", layout.warnings);
    }

    #[test]
    fn disabled_extensions_contribute_nothing() {
        let layout = settings_layout(&config(
            r#"{"id": "x", "extensionId": "off", "kind": "setting", "params": {"valueType": "boolean"}}"#,
        ));
        assert!(layout.pages.is_empty());
    }

    #[test]
    fn status_items_need_a_topic_and_sort_by_order() {
        let items = status_items(&config(
            r#"
            {"id": "s2", "extensionId": "a", "kind": "status-item", "params": {"topic": "t2", "order": 2}},
            {"id": "s1", "extensionId": "a", "kind": "status-item",
             "params": {"topic": "t1", "order": 1, "openCategory": "a.midi"}},
            {"id": "s0", "extensionId": "a", "kind": "status-item", "params": {}}
            "#,
        ));
        assert_eq!(
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["s1", "s2"]
        );
        assert_eq!(items[0].open_category.as_deref(), Some("a.midi"));
    }
}
