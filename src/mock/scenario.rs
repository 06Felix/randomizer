use std::{collections::HashMap, sync::Mutex};

#[derive(Debug, Default)]
pub struct ScenarioStore {
    counters: Mutex<HashMap<String, u64>>,
}

impl ScenarioStore {
    pub fn next(&self, route_id: &str, response_count: usize) -> (usize, u64) {
        let mut counters = self
            .counters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let sequence = counters.entry(route_id.to_string()).or_insert(0);
        let index = (*sequence as usize).min(response_count.saturating_sub(1));
        let current = *sequence;
        *sequence = sequence.saturating_add(1);
        (index, current)
    }

    pub fn reset(&self) {
        self.counters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}
