use crate::sql_info::SqlQueryInfo;
use std::collections::BTreeMap;

pub struct AppState {
    pub logs_by_request_id: BTreeMap<String, LogGroup>,
    pub selected_index: usize,
    pub request_ids: Vec<String>,
    pub all_logs: Vec<LogEntry>,
}

pub struct LogGroup {
    pub title: String,
    pub entries: Vec<LogEntry>,
    pub finished: bool,
    pub sql_query_info: SqlQueryInfo,
    pub first_timestamp: chrono::DateTime<chrono::Local>,
}

impl LogGroup {
    pub fn new(log_entry: &LogEntry) -> Self {
        let mut group = Self {
            title: "...".to_string(),
            entries: Vec::with_capacity(10),
            finished: false,
            sql_query_info: SqlQueryInfo::new(),
            first_timestamp: log_entry.timestamp,
        };

        group.add_entry(log_entry.clone());
        group
    }

    pub fn add_entry(&mut self, log_entry: LogEntry) {
        let message = &log_entry.message;

        if message.contains("Started ") {
            if let Some(start_pos) = message.find("Started ") {
                self.title = message[(start_pos + 8)..].to_string();
            } else {
                self.title = message.to_string();
            }
        }

        if message.contains("Completed ") {
            self.finished = true;
        }

        if let Some(new_sql_info) = SqlQueryInfo::from_message(message) {
            self.sql_query_info.merge(&new_sql_info);
        }

        self.entries.insert(0, log_entry);
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub request_id: String,
    pub message: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            logs_by_request_id: BTreeMap::new(),
            selected_index: 0,
            request_ids: Vec::new(),
            all_logs: Vec::new(),
        }
    }

    pub fn selected_request_id(&self) -> Option<&String> {
        self.request_ids.get(self.selected_index)
    }

    pub fn selected_group(&self) -> Option<&LogGroup> {
        let request_id = self.selected_request_id()?;
        self.logs_by_request_id.get(request_id)
    }

    pub fn select_request(&mut self, index: usize) -> bool {
        if index < self.request_ids.len() {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    pub fn next_request(&mut self, n: usize) -> bool {
        if self.request_ids.is_empty() || n == 0 {
            return false;
        }
        let new_index = (self.selected_index + n).min(self.request_ids.len() - 1);
        self.select_request(new_index)
    }

    pub fn previous_request(&mut self, n: usize) -> bool {
        if self.request_ids.is_empty() || n == 0 {
            return false;
        }
        let new_index = self.selected_index.saturating_sub(n);
        self.select_request(new_index)
    }

    pub fn selected_entries_count(&self) -> usize {
        self.selected_group().map_or(0, |group| group.entries.len())
    }

    pub fn selected_sql_line_count(&self) -> usize {
        self.selected_group()
            .map_or(0, |group| group.sql_query_info.display_line_count())
    }

    pub fn add_log_entry(&mut self, log_entry: LogEntry) -> bool {
        self.all_logs.push(log_entry.clone());

        if log_entry.request_id.is_empty() {
            return false;
        }

        let request_id = log_entry.request_id.clone();
        let is_new_request = !self.logs_by_request_id.contains_key(&request_id);

        if is_new_request {
            let new_group = LogGroup::new(&log_entry);
            self.request_ids.insert(0, request_id.clone());

            if self.request_ids.len() == 1 {
                self.selected_index = 0;
            } else {
                self.selected_index += 1;
            }

            self.logs_by_request_id.insert(request_id, new_group);
        } else if let Some(group) = self.logs_by_request_id.get_mut(&request_id) {
            group.add_entry(log_entry);
        }

        is_new_request
    }

    pub fn visible_logs(&self, start_idx: usize, viewport_height: usize) -> Vec<&LogEntry> {
        let total_logs = self.all_logs.len();
        if total_logs == 0 {
            return Vec::new();
        }

        if start_idx >= total_logs {
            return Vec::new();
        }

        let visible_count = viewport_height.min(total_logs - start_idx);
        let mut result = Vec::with_capacity(visible_count);

        for i in 0..visible_count {
            let idx = start_idx + i;
            result.push(&self.all_logs[idx]);
        }

        result
    }

    pub fn logs_count(&self) -> usize {
        self.all_logs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert_eq!(state.selected_index, 0);
        assert!(state.request_ids.is_empty());
        assert!(state.logs_by_request_id.is_empty());
        assert!(state.all_logs.is_empty());
    }

    #[test]
    fn test_select_request() {
        let mut state = AppState::new();

        // Cannot select in empty state
        assert!(!state.select_request(0));

        // Add a request
        let log_entry = LogEntry {
            timestamp: Local::now(),
            request_id: "test-id".to_string(),
            message: "Started GET /test".to_string(),
        };
        state.add_log_entry(log_entry);

        // Select valid index
        assert!(state.select_request(0));
        assert_eq!(state.selected_index, 0);

        // Cannot select out of range index
        assert!(!state.select_request(1));
    }

    #[test]
    fn test_add_log_entry() {
        let mut state = AppState::new();

        // Add new request
        let log_entry = LogEntry {
            timestamp: Local::now(),
            request_id: "req-1".to_string(),
            message: "Started GET /test".to_string(),
        };

        let is_new = state.add_log_entry(log_entry);
        assert!(is_new);
        assert_eq!(state.request_ids.len(), 1);
        assert_eq!(state.all_logs.len(), 1);

        // Add entry with same request ID
        let log_entry2 = LogEntry {
            timestamp: Local::now(),
            request_id: "req-1".to_string(),
            message: "Processing by TestController".to_string(),
        };

        let is_new2 = state.add_log_entry(log_entry2);
        assert!(!is_new2);
        assert_eq!(state.request_ids.len(), 1);
        assert_eq!(state.all_logs.len(), 2);

        // Add entry with different request ID
        let log_entry3 = LogEntry {
            timestamp: Local::now(),
            request_id: "req-2".to_string(),
            message: "Started GET /another".to_string(),
        };

        let is_new3 = state.add_log_entry(log_entry3);
        assert!(is_new3);
        assert_eq!(state.request_ids.len(), 2);
        assert_eq!(state.all_logs.len(), 3);
    }
}
