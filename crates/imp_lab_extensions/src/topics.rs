//! Topics: the extension → host channel.
//!
//! An extension publishes the latest value of a named topic (opaque JSON) whenever it changes.
//! The host keeps the latest value per topic and forwards each change to a sink (the desktop app
//! emits it to the webview). Publishing replaces the value; there is no history or queue.
//!
//! Topic ids are free strings; by convention they start with the publishing extension's id
//! (`aura.midi.status`).

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::os::raw::c_uchar;
use std::sync::{Arc, Mutex, RwLock};

/// C ABI publish callback: `topic` and `json` are UTF-8 (not NUL-terminated). Returns 0 on
/// success, non-zero for invalid UTF-8 or JSON.
pub type PublishFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    topic: *const c_uchar,
    topic_len: usize,
    json: *const c_uchar,
    len: usize,
) -> c_int;

type Sink = Arc<dyn Fn(&str, &Value) + Send + Sync>;

/// Latest value per topic, plus where changes go.
pub struct TopicHub {
    latest: Mutex<BTreeMap<String, Value>>,
    sink: RwLock<Option<Sink>>,
}

impl TopicHub {
    pub const fn new() -> Self {
        Self {
            latest: Mutex::new(BTreeMap::new()),
            sink: RwLock::new(None),
        }
    }

    /// Stores `value` as `topic`'s latest value, then hands it to the sink (outside the lock).
    pub fn publish(&self, topic: &str, value: Value) {
        if let Ok(mut latest) = self.latest.lock() {
            latest.insert(topic.to_string(), value.clone());
        }
        let sink = self.sink.read().ok().and_then(|sink| sink.clone());
        if let Some(sink) = sink {
            sink(topic, &value);
        }
    }

    /// Latest value of `topic`, if anything was published.
    pub fn get(&self, topic: &str) -> Option<Value> {
        self.latest.lock().ok()?.get(topic).cloned()
    }

    /// Every topic's latest value.
    pub fn snapshot(&self) -> BTreeMap<String, Value> {
        self.latest
            .lock()
            .map(|latest| latest.clone())
            .unwrap_or_default()
    }

    /// Sets where changes go (replacing any earlier sink). It's called on the publishing
    /// thread, so it must not block.
    pub fn set_sink(&self, sink: impl Fn(&str, &Value) + Send + Sync + 'static) {
        if let Ok(mut slot) = self.sink.write() {
            *slot = Some(Arc::new(sink));
        }
    }
}

impl Default for TopicHub {
    fn default() -> Self {
        Self::new()
    }
}

static HUB: TopicHub = TopicHub::new();

/// The process-wide hub extensions publish into.
pub fn topic_hub() -> &'static TopicHub {
    &HUB
}

/// The host's [`PublishFn`] over a `*const TopicHub` context.
pub(crate) unsafe extern "C" fn host_publish(
    ctx: *mut c_void,
    topic: *const c_uchar,
    topic_len: usize,
    json: *const c_uchar,
    len: usize,
) -> c_int {
    if ctx.is_null() || topic.is_null() || json.is_null() {
        return -1;
    }
    let Ok(topic) = std::str::from_utf8(std::slice::from_raw_parts(topic, topic_len)) else {
        return -1;
    };
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(json, len)) else {
        return -1;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return -1;
    };
    let hub = &*(ctx as *const TopicHub);
    hub.publish(topic, value);
    0
}

/// Extension-side handle for publishing topics, kept from registration. Callable from any
/// thread for the life of the process.
#[derive(Clone, Copy)]
pub struct Publisher {
    publish: PublishFn,
    ctx: *mut c_void,
}

// Safety: the host's publish callback is thread-safe and its context lives for the process.
unsafe impl Send for Publisher {}
unsafe impl Sync for Publisher {}

impl Publisher {
    /// The publisher a host offers, when it offers one.
    pub fn new(publish: Option<PublishFn>, ctx: *mut c_void) -> Option<Self> {
        let publish = publish?;
        (!ctx.is_null()).then_some(Self { publish, ctx })
    }

    /// Publishes raw JSON text. Returns whether the host accepted it.
    pub fn publish_json(&self, topic: &str, json: &str) -> bool {
        // Safety: `new` took the host's callback and its process-lifetime context.
        unsafe {
            (self.publish)(
                self.ctx,
                topic.as_ptr(),
                topic.len(),
                json.as_ptr(),
                json.len(),
            ) == 0
        }
    }

    /// Publishes `value` as JSON. Returns whether the host accepted it.
    pub fn publish<T: Serialize + ?Sized>(&self, topic: &str, value: &T) -> bool {
        serde_json::to_string(value).is_ok_and(|json| self.publish_json(topic, &json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn publisher(hub: &'static TopicHub) -> Publisher {
        Publisher::new(Some(host_publish), hub as *const TopicHub as *mut c_void)
            .expect("publisher")
    }

    #[test]
    fn latest_value_wins_and_reaches_the_sink() {
        let hub: &'static TopicHub = Box::leak(Box::new(TopicHub::new()));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink_seen = Arc::clone(&seen);
        hub.set_sink(move |topic, value| {
            sink_seen
                .lock()
                .unwrap()
                .push((topic.to_string(), value.clone()))
        });
        let publisher = publisher(hub);
        assert!(publisher.publish("x.status", &json!({"text": "a"})));
        assert!(publisher.publish("x.status", &json!({"text": "b"})));
        assert_eq!(hub.get("x.status"), Some(json!({"text": "b"})));
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert_eq!(hub.snapshot().len(), 1);
    }

    #[test]
    fn publishing_from_many_threads() {
        let hub: &'static TopicHub = Box::leak(Box::new(TopicHub::new()));
        let publisher = publisher(hub);
        let threads: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    for n in 0..100 {
                        assert!(publisher.publish(&format!("t.{i}"), &n));
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let snapshot = hub.snapshot();
        assert_eq!(snapshot.len(), 8);
        assert!(snapshot.values().all(|v| *v == json!(99)));
    }

    #[test]
    fn invalid_text_is_rejected() {
        let hub: &'static TopicHub = Box::leak(Box::new(TopicHub::new()));
        let publisher = publisher(hub);
        assert!(!publisher.publish_json("t", "{not json"));
        let bad = [0xFF_u8];
        let code = unsafe {
            host_publish(
                hub as *const TopicHub as *mut c_void,
                bad.as_ptr(),
                bad.len(),
                b"1".as_ptr(),
                1,
            )
        };
        assert_ne!(code, 0);
        assert!(hub.snapshot().is_empty());
        assert!(Publisher::new(None, std::ptr::null_mut()).is_none());
    }
}
