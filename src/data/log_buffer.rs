use std::collections::VecDeque;
use crate::app::LogEntry;

/// Efficient ring buffer for log entries with O(1) access and prepend support.
/// Designed for virtual scrolling - only visible entries are rendered.
#[derive(Debug, Clone)]
pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    /// Create a new log buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new entry to the end (newest)
    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push_back(entry);

        // Trim from front if over capacity
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Push multiple entries to the end
    pub fn push_batch(&mut self, entries: Vec<LogEntry>) {
        for entry in entries {
            self.push(entry);
        }
    }

    /// Prepend entries at the beginning (for loading historical logs)
    /// Returns the number of entries actually prepended
    pub fn prepend(&mut self, entries: Vec<LogEntry>) -> usize {
        let count = entries.len();

        // Add entries to the front in reverse order to maintain chronological order
        for entry in entries.into_iter().rev() {
            self.entries.push_front(entry);
        }

        // Trim from back if over capacity
        while self.entries.len() > self.capacity {
            self.entries.pop_back();
        }

        count
    }

    /// Get entry at index (0 = oldest)
    pub fn get(&self, index: usize) -> Option<&LogEntry> {
        self.entries.get(index)
    }

    /// Get a range of entries for rendering
    pub fn get_range(&self, start: usize, count: usize) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().skip(start).take(count)
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the oldest entry (first in buffer)
    pub fn oldest(&self) -> Option<&LogEntry> {
        self.entries.front()
    }

    /// Get the newest entry (last in buffer)
    pub fn newest(&self) -> Option<&LogEntry> {
        self.entries.back()
    }

    /// Get oldest timestamp for pagination
    pub fn oldest_timestamp(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.entries.front().and_then(|e| e.timestamp)
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(10000) // 10k entries by default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::LogLevel;

    fn make_entry(content: &str) -> LogEntry {
        LogEntry {
            timestamp: None,
            content: content.to_string(),
            level: LogLevel::Info,
        }
    }

    #[test]
    fn test_push_and_get() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_entry("line 1"));
        buffer.push(make_entry("line 2"));

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.get(0).unwrap().content, "line 1");
        assert_eq!(buffer.get(1).unwrap().content, "line 2");
    }

    #[test]
    fn test_capacity_limit() {
        let mut buffer = LogBuffer::new(3);
        buffer.push(make_entry("line 1"));
        buffer.push(make_entry("line 2"));
        buffer.push(make_entry("line 3"));
        buffer.push(make_entry("line 4"));

        assert_eq!(buffer.len(), 3);
        // Oldest entry should be removed
        assert_eq!(buffer.get(0).unwrap().content, "line 2");
    }

    #[test]
    fn test_prepend() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_entry("line 3"));
        buffer.push(make_entry("line 4"));

        buffer.prepend(vec![
            make_entry("line 1"),
            make_entry("line 2"),
        ]);

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.get(0).unwrap().content, "line 1");
        assert_eq!(buffer.get(1).unwrap().content, "line 2");
        assert_eq!(buffer.get(2).unwrap().content, "line 3");
        assert_eq!(buffer.get(3).unwrap().content, "line 4");
    }

    #[test]
    fn test_get_range() {
        let mut buffer = LogBuffer::new(100);
        for i in 1..=10 {
            buffer.push(make_entry(&format!("line {}", i)));
        }

        let range: Vec<_> = buffer.get_range(3, 4).collect();
        assert_eq!(range.len(), 4);
        assert_eq!(range[0].content, "line 4");
        assert_eq!(range[3].content, "line 7");
    }
}
