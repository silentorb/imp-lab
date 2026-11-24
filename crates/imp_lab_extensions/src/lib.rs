//! Tome-shaped Imp Lab extensions: config parse + dynamically linked host modules.

mod config;
mod host;
mod loader;
mod service;
pub mod settings;
pub mod topics;
mod ts_json;

pub use config::{
    parse_extensions_json, ComponentConfig, ComponentKind, ExtensionConfig, ExtensionsConfig,
    ExtensionsError, SUPPORTED_VERSION,
};
pub use host::{
    host_add_json, host_vtable, ExtensionHost, ExtensionRegisterFn, ImpLabHostV0,
    EXTENSION_REGISTER_SYMBOL,
};
pub use loader::{
    load_extensions_default, load_extensions_from_path, resolve_extensions_config_path,
    ExtensionContributions, ExtensionsConfigDto, ExtensionsLoadResult, ExtensionsRuntime,
    LoadedCatalogs, LoadError, MenuItemContributionDto,
};
pub use service::{
    invoke_service_action, menu_contributions_from_config, resolve_command_action,
    send_service_event, send_service_request, service_position_secs, write_service_error,
    write_service_reply, CommandDispatchError, ImpLabServiceV0, MenuItemContribution,
    RegisteredService, ResolvedCommandAction, ServiceRegistry, SERVICE_REPLY_TOO_SMALL,
};
pub use settings::{settings_layout, status_items, SettingsLayout, StatusItemDecl};
pub use topics::{topic_hub, Publisher, PublishFn, TopicHub};
pub use ts_json::imp_json_to_ts_camel;
