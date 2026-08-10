use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::mock::{CompiledMockRegistry, RequestLog, ScenarioStore};

/// Shared server state for resource limits and tunables.
#[derive(Clone)]
pub struct AppState {
    /// Limits how many WebSocket streaming connections may be open at once.
    pub ws_connection_limit: Arc<Semaphore>,
    pub mock_registry: Option<Arc<CompiledMockRegistry>>,
    pub scenarios: Arc<ScenarioStore>,
    pub request_log: Arc<RequestLog>,
}

impl AppState {
    /// Default cap on simultaneous `/stream` connections.
    pub const DEFAULT_MAX_CONCURRENT_WS_STREAMS: usize = 4096;

    pub fn new(max_concurrent_ws_streams: usize) -> Self {
        Self {
            ws_connection_limit: Arc::new(Semaphore::new(max_concurrent_ws_streams)),
            mock_registry: None,
            scenarios: Arc::new(ScenarioStore::default()),
            request_log: Arc::new(RequestLog::default()),
        }
    }

    pub fn with_mock_registry(mut self, registry: CompiledMockRegistry) -> Self {
        self.mock_registry = Some(Arc::new(registry));
        self
    }
}
