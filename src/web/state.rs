use std::time::Instant;

use tokio::sync::Mutex;

/// Shared state of the web UI.
pub struct AppState {
    pub started: Instant,
    /// Mutating actions (sync/link/clean/exec, config writes) are serialised
    /// so two browser tabs cannot fight over the same cabinets.
    pub action_lock: Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            action_lock: Mutex::new(()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
