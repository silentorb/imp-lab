//! Host ABI for dynamically linked Lab extensions (lockstep with Lab).

use crate::service::{cstr_to_string, ImpLabServiceV0, ServiceRegistry};
use crate::topics::{host_publish, topic_hub, PublishFn, Publisher, TopicHub};
use serde_json::Value;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::os::raw::c_uchar;

/// Opaque host context passed to extensions.
pub struct ExtensionHost {
    pub(crate) node_libraries: Vec<Value>,
    pub(crate) imp_files: Vec<Value>,
    pub(crate) services: ServiceRegistry,
    pub(crate) pack_root: std::path::PathBuf,
}

impl ExtensionHost {
    /// Create an empty host for one extension load.
    pub fn new(pack_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            node_libraries: Vec::new(),
            imp_files: Vec::new(),
            services: ServiceRegistry::default(),
            pack_root: pack_root.into(),
        }
    }

    /// Imp `NodeLibrary` JSON objects registered by the extension (TS camelCase).
    pub fn node_libraries(&self) -> &[Value] {
        &self.node_libraries
    }

    /// Imp file JSON objects (types, node definitions; see imp-spec `imp-files.md`).
    pub fn imp_files(&self) -> &[Value] {
        &self.imp_files
    }

    /// Services registered during this library's `imp_lab_extension_register`.
    pub fn services(&self) -> &ServiceRegistry {
        &self.services
    }

    /// Pack root used to resolve relative asset paths for the extension.
    pub fn pack_root(&self) -> &std::path::Path {
        &self.pack_root
    }
}

/// C ABI table passed to `imp_lab_extension_register`.
#[repr(C)]
pub struct ImpLabHostV1 {
    /// Host context pointer (`*mut ExtensionHost`).
    pub ctx: *mut c_void,
    /// Register a NodeLibrary JSON document (UTF-8).
    pub add_node_library_json: Option<
        unsafe extern "C" fn(ctx: *mut c_void, json: *const c_uchar, len: usize) -> c_int,
    >,
    /// Register an Imp file JSON document (UTF-8): its `types` and `definitions`.
    pub add_imp_file_json: Option<
        unsafe extern "C" fn(ctx: *mut c_void, json: *const c_uchar, len: usize) -> c_int,
    >,
    /// Resolve a pack-relative path into `out` (UTF-8, NUL-terminated). Returns 0 on success.
    pub resolve_pack_path: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            relative: *const c_char,
            out: *mut c_char,
            out_len: usize,
        ) -> c_int,
    >,
    /// Register a long-lived service vtable keyed by implementation id.
    pub register_service: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            implementation_id: *const c_char,
            service: *const ImpLabServiceV0,
        ) -> c_int,
    >,
    /// Context for `publish`. Unlike `ctx`, it lives for the whole process, so an extension may
    /// keep it (with `publish`) after registration.
    pub publisher_ctx: *mut c_void,
    /// Publish the latest value of a topic (see [`crate::topics`]). Callable from any thread
    /// after registration; never waits on the UI. Optional.
    pub publish: Option<PublishFn>,
}

impl ImpLabHostV1 {
    /// The topic publisher this host offers, for an extension to keep.
    pub fn publisher(&self) -> Option<Publisher> {
        Publisher::new(self.publish, self.publisher_ctx)
    }
}

/// Exported symbol name every Lab extension cdylib must provide.
pub const EXTENSION_REGISTER_SYMBOL: &[u8] = b"imp_lab_extension_register\0";

/// Extension entrypoint signature.
pub type ExtensionRegisterFn = unsafe extern "C" fn(host: *const ImpLabHostV1) -> c_int;

unsafe extern "C" fn add_node_library_json(
    ctx: *mut c_void,
    json: *const c_uchar,
    len: usize,
) -> c_int {
    push_json(ctx, json, len, |host, value| host.node_libraries.push(value))
}

unsafe extern "C" fn add_imp_file_json(
    ctx: *mut c_void,
    json: *const c_uchar,
    len: usize,
) -> c_int {
    push_json(ctx, json, len, |host, value| host.imp_files.push(value))
}

unsafe extern "C" fn resolve_pack_path(
    ctx: *mut c_void,
    relative: *const c_char,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    if ctx.is_null() || relative.is_null() || out.is_null() || out_len == 0 {
        return -1;
    }
    let host = &*(ctx as *const ExtensionHost);
    let rel = match CStr::from_ptr(relative).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let path = host.pack_root.join(rel);
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() + 1 > out_len {
        return -1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, bytes.len());
    *out.add(bytes.len()) = 0;
    0
}

unsafe extern "C" fn register_service(
    ctx: *mut c_void,
    implementation_id: *const c_char,
    service: *const ImpLabServiceV0,
) -> c_int {
    if ctx.is_null() || service.is_null() {
        return -1;
    }
    let Some(id) = cstr_to_string(implementation_id) else {
        return -1;
    };
    if id.is_empty() {
        return -1;
    }
    let host = &mut *(ctx as *mut ExtensionHost);
    host.services.insert(id, *service);
    0
}

unsafe fn push_json(
    ctx: *mut c_void,
    json: *const c_uchar,
    len: usize,
    push: impl FnOnce(&mut ExtensionHost, Value),
) -> c_int {
    if ctx.is_null() || json.is_null() {
        return -1;
    }
    let slice = std::slice::from_raw_parts(json, len);
    let text = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return -1,
    };
    let host = &mut *(ctx as *mut ExtensionHost);
    push(host, crate::ts_json::imp_json_to_ts_camel(value));
    0
}

/// Build the C host table for `host`.
pub fn host_vtable(host: &mut ExtensionHost) -> ImpLabHostV1 {
    ImpLabHostV1 {
        ctx: host as *mut ExtensionHost as *mut c_void,
        add_node_library_json: Some(add_node_library_json),
        add_imp_file_json: Some(add_imp_file_json),
        resolve_pack_path: Some(resolve_pack_path),
        register_service: Some(register_service),
        publisher_ctx: topic_hub() as *const TopicHub as *mut c_void,
        publish: Some(host_publish),
    }
}

/// Helper for extension crates: push UTF-8 JSON through a host callback.
///
/// # Safety
///
/// `callback` (when `Some`) and `ctx` must be valid for the duration of the call and
/// match the Lab host that provided them via [`ImpLabHostV1`].
pub unsafe fn host_add_json(
    callback: Option<unsafe extern "C" fn(*mut c_void, *const c_uchar, usize) -> c_int>,
    ctx: *mut c_void,
    json: &str,
) -> c_int {
    let Some(cb) = callback else {
        return -1;
    };
    cb(ctx, json.as_ptr(), json.len())
}
