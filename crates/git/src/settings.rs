//! App settings the backend reads (`sshKeys`, `signCommits`). They live in the
//! UI's store; `provider` reads it in the Tauri app, and the WSL agent, which
//! has no store, gets the app's values pushed with each request (`apply`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

type Provider = Box<dyn Fn(&str) -> Option<Value> + Send + Sync>;

static PROVIDER: OnceLock<Provider> = OnceLock::new();
static PUSHED: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);

pub fn set_provider(provider: impl Fn(&str) -> Option<Value> + Send + Sync + 'static) {
    let _ = PROVIDER.set(Box::new(provider));
}

pub fn get(key: &str) -> Option<Value> {
    if let Some(v) = PUSHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().and_then(|m| m.get(key)) {
        return Some(v.clone());
    }
    PROVIDER.get().and_then(|p| p(key))
}

/// Keys the agent needs from the app, snapshotted onto each request.
pub const SYNCED_KEYS: [&str; 2] = ["sshKeys", "signCommits"];

pub fn snapshot() -> HashMap<String, Value> {
    SYNCED_KEYS.iter().filter_map(|k| get(k).map(|v| (k.to_string(), v))).collect()
}

pub fn apply(values: HashMap<String, Value>) {
    *PUSHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(values);
}
