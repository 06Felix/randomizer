use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

const DEFAULT_CAPACITY: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct RequestLogEntry {
    pub timestamp_epoch_ms: u128,
    pub route_id: Option<String>,
    pub method: String,
    pub path: String,
    pub status: u16,
}

#[derive(Debug)]
pub struct RequestLog {
    capacity: usize,
    entries: Mutex<VecDeque<RequestLogEntry>>,
}

impl Default for RequestLog {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl RequestLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn record(&self, route_id: Option<&str>, method: &str, path: &str, status: u16) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(RequestLogEntry {
            timestamp_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            route_id: route_id.map(ToString::to_string),
            method: method.to_string(),
            path: path.to_string(),
            status,
        });
    }

    pub fn entries(&self) -> Vec<RequestLogEntry> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}
