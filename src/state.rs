use std::sync::Arc;

use tokio::sync::Semaphore;

/// Shared server state for resource limits and tunables.
#[derive(Clone)]
pub struct AppState {
    /// Limits how many WebSocket streaming connections may be open at once.
    pub ws_connection_limit: Arc<Semaphore>,
}

impl AppState {
    /// Default cap on simultaneous `/stream` connections.
    pub const DEFAULT_MAX_CONCURRENT_WS_STREAMS: usize = 4096;

    pub fn new(max_concurrent_ws_streams: usize) -> Self {
        Self {
            ws_connection_limit: Arc::new(Semaphore::new(max_concurrent_ws_streams)),
        }
    }
}
