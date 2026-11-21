//! Host-side service registration and command → service dispatch.

use crate::config::{ComponentConfig, ComponentKind, ExtensionConfig, ExtensionsConfig};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::os::raw::c_uchar;
use thiserror::Error;

/// C ABI vtable for a long-lived Lab host service (playback-shaped in v1).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ImpLabServiceV0 {
    /// Opaque service context.
    pub ctx: *mut c_void,
    /// Start playback / primary action with graph JSON + optional neighbor path.
    pub start: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            graph_json: *const c_uchar,
            graph_len: usize,
            neighbor_path: *const c_char,
            err_out: *mut c_char,
            err_len: usize,
        ) -> c_int,
    >,
    /// Stop / cancel.
    pub stop: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            err_out: *mut c_char,
            err_len: usize,
        ) -> c_int,
    >,
    /// Toggle start/stop.
    pub toggle: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            graph_json: *const c_uchar,
            graph_len: usize,
            neighbor_path: *const c_char,
            err_out: *mut c_char,
            err_len: usize,
        ) -> c_int,
    >,
    /// Returns 1 when active, 0 when idle, negative on error.
    pub is_playing: Option<unsafe extern "C" fn(ctx: *mut c_void) -> c_int>,
}

// Safety: service vtables are process-lifetime function pointers + opaque ctx owned by the extension.
unsafe impl Send for ImpLabServiceV0 {}
unsafe impl Sync for ImpLabServiceV0 {}

/// One registered service keyed by implementation id.
#[derive(Clone, Copy)]
pub struct RegisteredService {
    /// Implementation id from `register_service` / service component.
    pub implementation_id: &'static str,
    /// Service vtable.
    pub vtable: ImpLabServiceV0,
}

/// In-memory map of registered services (implementation id → vtable).
#[derive(Default)]
pub struct ServiceRegistry {
    services: BTreeMap<String, ImpLabServiceV0>,
}

impl ServiceRegistry {
    /// Insert or replace a service by implementation id.
    pub fn insert(&mut self, implementation_id: impl Into<String>, vtable: ImpLabServiceV0) {
        self.services.insert(implementation_id.into(), vtable);
    }

    /// Look up a registered service.
    pub fn get(&self, implementation_id: &str) -> Option<&ImpLabServiceV0> {
        self.services.get(implementation_id)
    }

    /// Number of registered services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Whether any services are registered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Consume into the underlying map.
    pub fn into_map(self) -> BTreeMap<String, ImpLabServiceV0> {
        self.services
    }
}

/// Resolved command → service action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommandAction {
    /// Service component id from command params (`serviceId`).
    pub service_component_id: String,
    /// Service implementation id used to look up the registered vtable.
    pub service_implementation_id: String,
    /// Action name: `start` | `stop` | `toggle` | `is_playing`.
    pub action: String,
}

/// Menu item contribution derived from a `menu-item` component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemContribution {
    /// Command id to emit (`implementationId`).
    pub command_id: String,
    /// Display label.
    pub label: String,
    /// Submenu title (`params.menu`).
    pub menu: String,
    /// Sort order within the submenu (`params.order`, default 0).
    pub order: i64,
}

/// Failures resolving or invoking an extension command.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandDispatchError {
    /// No enabled `command` component with this id.
    #[error("unknown or disabled command `{0}`")]
    UnknownCommand(String),
    /// Command params missing `serviceId` / `action`.
    #[error("command `{command}` missing params.{field}")]
    MissingParam {
        /// Command id.
        command: String,
        /// Param field name.
        field: &'static str,
    },
    /// No enabled `service` component for `serviceId`.
    #[error("unknown or disabled service component `{0}`")]
    UnknownServiceComponent(String),
    /// Service was not registered by any loaded cdylib.
    #[error("service `{0}` is not registered")]
    ServiceNotRegistered(String),
    /// Unsupported action name.
    #[error("unsupported service action `{0}`")]
    UnsupportedAction(String),
    /// Service returned a non-zero code (optional message).
    #[error("{0}")]
    ServiceFailed(String),
}

/// Collect enabled menu-item contributions, sorted by menu title then order.
pub fn menu_contributions_from_config(config: &ExtensionsConfig) -> Vec<MenuItemContribution> {
    let mut items: Vec<MenuItemContribution> = config
        .components
        .iter()
        .filter(|c| c.enabled && matches!(c.kind, ComponentKind::MenuItem))
        .filter_map(|c| {
            let ext = config.extensions.iter().find(|e| e.id == c.extension_id);
            if let Some(e) = ext {
                if !e.enabled {
                    return None;
                }
            }
            let params = c.merged_params(ext);
            let menu = params
                .get("menu")
                .and_then(|v| v.as_str())
                .unwrap_or("Extensions")
                .to_string();
            let order = params.get("order").and_then(|v| v.as_i64()).unwrap_or(0);
            let label = c
                .label
                .clone()
                .unwrap_or_else(|| c.implementation_id.clone());
            Some(MenuItemContribution {
                command_id: c.implementation_id.clone(),
                label,
                menu,
                order,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        a.menu
            .cmp(&b.menu)
            .then(a.order.cmp(&b.order))
            .then(a.command_id.cmp(&b.command_id))
    });
    items
}

/// Resolve a command id to a service action using enabled components.
pub fn resolve_command_action(
    config: &ExtensionsConfig,
    command_id: &str,
) -> Result<ResolvedCommandAction, CommandDispatchError> {
    let command = config
        .components
        .iter()
        .find(|c| {
            c.enabled
                && matches!(c.kind, ComponentKind::Command)
                && (c.id == command_id || c.implementation_id == command_id)
        })
        .ok_or_else(|| CommandDispatchError::UnknownCommand(command_id.to_string()))?;

    let ext = config
        .extensions
        .iter()
        .find(|e| e.id == command.extension_id);
    if let Some(e) = ext {
        if !e.enabled {
            return Err(CommandDispatchError::UnknownCommand(command_id.to_string()));
        }
    }

    let params = command.merged_params(ext);
    let service_component_id = param_string(&params, "serviceId").ok_or_else(|| {
        CommandDispatchError::MissingParam {
            command: command_id.to_string(),
            field: "serviceId",
        }
    })?;
    let action = param_string(&params, "action").ok_or_else(|| {
        CommandDispatchError::MissingParam {
            command: command_id.to_string(),
            field: "action",
        }
    })?;

    let service = config
        .components
        .iter()
        .find(|c| {
            c.enabled
                && matches!(c.kind, ComponentKind::Service)
                && c.id == service_component_id
        })
        .ok_or_else(|| {
            CommandDispatchError::UnknownServiceComponent(service_component_id.clone())
        })?;

    Ok(ResolvedCommandAction {
        service_component_id,
        service_implementation_id: service.implementation_id.clone(),
        action,
    })
}

fn param_string(params: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    params.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// Invoke a resolved action on a registered service.
///
/// # Safety
///
/// `vtable` and its `ctx` must remain valid for the duration of the call (process-lifetime
/// extension registration).
pub unsafe fn invoke_service_action(
    vtable: &ImpLabServiceV0,
    action: &str,
    graph_json: Option<&str>,
    neighbor_path: Option<&str>,
) -> Result<Option<bool>, CommandDispatchError> {
    let mut err_buf = vec![0u8; 1024];
    let neighbor_c = neighbor_path
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| CommandDispatchError::ServiceFailed("invalid neighbor path".into()))?;
    let neighbor_ptr = neighbor_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());
    let (graph_ptr, graph_len) = match graph_json {
        Some(s) => (s.as_ptr(), s.len()),
        None => (std::ptr::null(), 0),
    };

    match action {
        "start" => {
            let Some(start) = vtable.start else {
                return Err(CommandDispatchError::UnsupportedAction(action.into()));
            };
            let code = start(
                vtable.ctx,
                graph_ptr,
                graph_len,
                neighbor_ptr,
                err_buf.as_mut_ptr() as *mut c_char,
                err_buf.len(),
            );
            if code != 0 {
                return Err(CommandDispatchError::ServiceFailed(read_err(&err_buf, code)));
            }
            Ok(None)
        }
        "stop" => {
            let Some(stop) = vtable.stop else {
                return Err(CommandDispatchError::UnsupportedAction(action.into()));
            };
            let code = stop(
                vtable.ctx,
                err_buf.as_mut_ptr() as *mut c_char,
                err_buf.len(),
            );
            if code != 0 {
                return Err(CommandDispatchError::ServiceFailed(read_err(&err_buf, code)));
            }
            Ok(None)
        }
        "toggle" => {
            let Some(toggle) = vtable.toggle else {
                return Err(CommandDispatchError::UnsupportedAction(action.into()));
            };
            let code = toggle(
                vtable.ctx,
                graph_ptr,
                graph_len,
                neighbor_ptr,
                err_buf.as_mut_ptr() as *mut c_char,
                err_buf.len(),
            );
            if code != 0 {
                return Err(CommandDispatchError::ServiceFailed(read_err(&err_buf, code)));
            }
            Ok(None)
        }
        "is_playing" => {
            let Some(is_playing) = vtable.is_playing else {
                return Err(CommandDispatchError::UnsupportedAction(action.into()));
            };
            let code = is_playing(vtable.ctx);
            if code < 0 {
                return Err(CommandDispatchError::ServiceFailed(format!(
                    "is_playing failed ({code})"
                )));
            }
            Ok(Some(code != 0))
        }
        other => Err(CommandDispatchError::UnsupportedAction(other.into())),
    }
}

fn read_err(buf: &[u8], code: c_int) -> String {
    let nul = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    if nul == 0 {
        return format!("service failed ({code})");
    }
    match std::str::from_utf8(&buf[..nul]) {
        Ok(s) if !s.is_empty() => s.to_string(),
        _ => format!("service failed ({code})"),
    }
}

/// Write a UTF-8 error message into a C out buffer (NUL-terminated when possible).
///
/// # Safety
///
/// `err_out` must be null or point to a writable buffer of at least `err_len` bytes.
pub unsafe fn write_service_error(err_out: *mut c_char, err_len: usize, message: &str) {
    if err_out.is_null() || err_len == 0 {
        return;
    }
    let bytes = message.as_bytes();
    let copy_len = bytes.len().min(err_len.saturating_sub(1));
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), err_out as *mut u8, copy_len);
    *err_out.add(copy_len) = 0;
}

/// Helper used by host `register_service` to read a C string implementation id.
pub(crate) unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(str::to_string)
}

/// Re-export component types used when wiring params in docs/tests.
pub type _ComponentConfig = ComponentConfig;
pub type _ExtensionConfig = ExtensionConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_extensions_json;

    fn sample_config() -> ExtensionsConfig {
        parse_extensions_json(
            r#"{
              "version": 1,
              "extensions": [
                {"id":"aura-playback","enabled":true,"libraryPath":"extensions/aura_imp_lab_playback"}
              ],
              "components": [
                {
                  "id":"aura-playback",
                  "extensionId":"aura-playback",
                  "kind":"service",
                  "implementationId":"playback",
                  "enabled":true
                },
                {
                  "id":"play.start",
                  "extensionId":"aura-playback",
                  "kind":"command",
                  "implementationId":"play.start",
                  "enabled":true,
                  "params":{"serviceId":"aura-playback","action":"start"}
                },
                {
                  "id":"play.stop",
                  "extensionId":"aura-playback",
                  "kind":"command",
                  "implementationId":"play.stop",
                  "enabled":true,
                  "params":{"serviceId":"aura-playback","action":"stop"}
                },
                {
                  "id":"menu.play.start",
                  "extensionId":"aura-playback",
                  "kind":"menu-item",
                  "implementationId":"play.start",
                  "label":"Start",
                  "enabled":true,
                  "params":{"menu":"Play","order":10}
                },
                {
                  "id":"menu.play.stop",
                  "extensionId":"aura-playback",
                  "kind":"menu-item",
                  "implementationId":"play.stop",
                  "label":"Stop",
                  "enabled":true,
                  "params":{"menu":"Play","order":20}
                }
              ]
            }"#,
        )
        .expect("parse")
    }

    #[test]
    fn resolves_play_start_to_playback_service() {
        let config = sample_config();
        let resolved = resolve_command_action(&config, "play.start").expect("resolve");
        assert_eq!(resolved.service_component_id, "aura-playback");
        assert_eq!(resolved.service_implementation_id, "playback");
        assert_eq!(resolved.action, "start");
    }

    #[test]
    fn menu_items_grouped_and_ordered() {
        let items = menu_contributions_from_config(&sample_config());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].menu, "Play");
        assert_eq!(items[0].command_id, "play.start");
        assert_eq!(items[0].order, 10);
        assert_eq!(items[1].command_id, "play.stop");
        assert_eq!(items[1].order, 20);
    }

    #[test]
    fn missing_service_param_errors() {
        let config = parse_extensions_json(
            r#"{
              "version":1,
              "extensions":[{"id":"x","enabled":true}],
              "components":[{
                "id":"play.start","extensionId":"x","kind":"command",
                "implementationId":"play.start","enabled":true,"params":{"action":"start"}
              }]
            }"#,
        )
        .unwrap();
        let err = resolve_command_action(&config, "play.start").unwrap_err();
        assert!(matches!(
            err,
            CommandDispatchError::MissingParam { field: "serviceId", .. }
        ));
    }
}
