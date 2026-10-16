//! Backend → UI events, independent of who the UI is: the Tauri app emits
//! them on its window, the WSL agent ships them to the app over its pipe.

use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

type Sink = Box<dyn Fn(&str, Value) + Send + Sync>;

static SINK: OnceLock<Sink> = OnceLock::new();

pub fn set_sink(sink: impl Fn(&str, Value) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(sink));
}

/// False without a UI (tests), where prompts must not block.
pub fn has_sink() -> bool {
    SINK.get().is_some()
}

/// No-op without a sink.
pub fn emit(name: &str, payload: impl Serialize) {
    if let (Some(sink), Ok(value)) = (SINK.get(), serde_json::to_value(payload)) {
        sink(name, value);
    }
}
