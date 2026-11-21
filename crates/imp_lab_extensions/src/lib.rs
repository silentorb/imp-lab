//! Tome-shaped Imp Lab extensions: config parse + dynamically linked host modules.

mod config;
mod host;
mod loader;
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
    ExtensionsConfigDto, ExtensionsLoadResult, LoadedCatalogs, LoadError,
};
pub use ts_json::imp_json_to_ts_camel;
