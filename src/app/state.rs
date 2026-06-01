use std::collections::VecDeque;
use crate::docker::{Container, Stats};
use crate::app::message::{LogEntry, LogLevel};

/// View mode - which panel is displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Logs,
    Stats,
    LogsExpanded,
}

/// Navigation mode - which panel has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    Containers,
    Logs,
    Stats,
}

/// Menu mode - which overlay menu is open
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMode {
    None,
    DockerOps,
    Clipboard,
}

/// Transition state for loading screens (v3.3.0)
/// Used to show feedback during view/container switches
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TransitionState {
    Loading(String),
    #[default]
    Ready,
}

/// Notification type for temporary messages
#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
    pub created_at: std::time::Instant,
}

impl Notification {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: false,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: true,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        let duration = if self.is_error { 5 } else { 2 };
        self.created_at.elapsed().as_secs() >= duration
    }
}

/// Immutable application state
/// All state changes happen through the update function
#[derive(Debug, Clone)]
pub struct AppState {
    // === Container State ===
    pub containers: Vec<Container>,
    pub selected_container: usize,

    // === View State ===
    pub view_mode: ViewMode,
    pub navigation_mode: NavigationMode,
    pub menu_mode: MenuMode,

    // === Logs State ===
    pub logs: LogsState,

    // === Stats State ===
    pub stats: Option<Stats>,

    // === UI State ===
    pub numeric_input: NumericInputState,
    pub search: SearchState,
    pub notification: Option<Notification>,
    pub viewport_height: usize,
    /// Real height of logs panel (for accurate scroll calculations)
    pub logs_panel_height: usize,

    // === System State ===
    pub should_quit: bool,
    pub last_refresh: std::time::Instant,
    /// Last time the user pressed a key — drives adaptive tick cadence.
    pub last_activity: std::time::Instant,

    // === Stream Management (v3.2.2) ===
    /// Generation counter - incremented on each container switch
    /// Used to invalidate stale log messages from previous streams
    pub stream_generation: u64,

    /// ID of the container we're currently streaming logs from
    /// Used for validation of incoming log messages
    pub current_container_id: Option<String>,

    // === Rendering Control (v3.3.0) ===
    /// Whether the UI needs to be redrawn
    pub needs_redraw: bool,

    /// Force a full screen clear before redraw (for ghost characters)
    pub force_full_redraw: bool,

    /// Current transition state (loading screens)
    pub transition_state: TransitionState,
}

/// State for the logs viewer
#[derive(Debug, Clone)]
pub struct LogsState {
    /// The log entries buffer (newest at the end)
    pub entries: VecDeque<LogEntry>,

    /// Current scroll position (0 = top of buffer)
    pub scroll_position: usize,

    /// Whether we're loading more historical logs
    pub is_loading_more: bool,

    /// Whether there are more historical logs available
    pub has_more_history: bool,

    /// The oldest timestamp we've loaded (for pagination)
    pub oldest_timestamp: Option<chrono::DateTime<chrono::Utc>>,

    /// Total logs loaded (for display)
    pub total_loaded: usize,

    /// Maximum buffer capacity
    pub capacity: usize,

    /// Active log-level filter. `None` shows every line.
    ///
    /// NOTE: mutating this field directly invalidates `filtered_count`. After
    /// changing it you MUST call [`LogsState::recount_filtered`] (the only
    /// real caller is the `Tab` handler in `update.rs`).
    pub level_filter: Option<LogLevel>,

    /// Cached number of entries passing `level_filter`, kept in sync
    /// incrementally by `push`/`prepend`/`clear`/trim so `filtered_len()` is
    /// O(1) instead of O(n) on the hot log path. Rebuilt by `recount_filtered`.
    filtered_count: usize,
}

impl Default for LogsState {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(10000),
            scroll_position: 0,
            is_loading_more: false,
            has_more_history: true,
            oldest_timestamp: None,
            total_loaded: 0,
            capacity: 10000,
            level_filter: None,
            filtered_count: 0,
        }
    }
}

impl LogsState {
    /// Whether an entry passes the given filter (`None` = everything passes).
    fn passes(filter: Option<LogLevel>, entry: &LogEntry) -> bool {
        match filter {
            Some(level) => entry.level == level,
            None => true,
        }
    }

    /// Clear all logs and reset state
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_position = 0;
        self.is_loading_more = false;
        self.has_more_history = true;
        self.oldest_timestamp = None;
        self.total_loaded = 0;
        self.filtered_count = 0;
    }

    /// Push a new log entry to the end
    pub fn push(&mut self, entry: LogEntry) {
        // Update oldest timestamp if this is the first entry
        if self.entries.is_empty() {
            self.oldest_timestamp = entry.timestamp;
        }

        let filter = self.level_filter;
        if Self::passes(filter, &entry) {
            self.filtered_count += 1;
        }

        self.entries.push_back(entry);
        self.total_loaded += 1;

        // Trim from front if over capacity
        while self.entries.len() > self.capacity {
            if let Some(removed) = self.entries.pop_front() {
                if Self::passes(filter, &removed) {
                    self.filtered_count = self.filtered_count.saturating_sub(1);
                }
            }
        }
    }

    /// Prepend historical entries (for infinite scroll)
    /// v3.2.2: Fixed timestamp tracking - now correctly extracts oldest BEFORE reverse iteration
    pub fn prepend(&mut self, entries: Vec<LogEntry>) {
        if entries.is_empty() {
            return;
        }

        let count = entries.len();

        // Count how many of the new entries are actually visible under the
        // active filter — the scroll shift below must be in filtered-index space.
        let visible_added = match self.level_filter {
            None => count,
            Some(level) => entries.iter().filter(|e| e.level == level).count(),
        };

        // CRITICAL FIX (v3.2.2): Get oldest timestamp BEFORE reversing
        // Entries arrive in chronological order (oldest first from Docker API)
        // Previously the bug was: iterating in reverse caused NEWEST to be processed first
        let batch_oldest = entries.first().and_then(|e| e.timestamp);

        // Update oldest_timestamp with the batch's actual oldest entry
        if let Some(ts) = batch_oldest {
            match self.oldest_timestamp {
                Some(existing) if ts < existing => self.oldest_timestamp = Some(ts),
                None => self.oldest_timestamp = Some(ts),
                _ => {} // Batch oldest is not older than what we have
            }
        }

        // Newly inserted visible entries grow the filtered count.
        self.filtered_count += visible_added;

        // Insert in reverse order so oldest ends up at front
        for entry in entries.into_iter().rev() {
            self.entries.push_front(entry);
        }

        self.total_loaded += count;

        // Trim from back if over capacity
        let filter = self.level_filter;
        while self.entries.len() > self.capacity {
            if let Some(removed) = self.entries.pop_back() {
                if Self::passes(filter, &removed) {
                    self.filtered_count = self.filtered_count.saturating_sub(1);
                }
            }
        }

        // ALWAYS adjust scroll position to maintain current view
        // When historical logs are prepended, shift scroll by the visible count
        // to keep viewing the same logs (not the newly loaded historical ones)
        self.scroll_position = self.scroll_position.saturating_add(visible_added);
    }

    /// Iterator over entries that pass the active level filter.
    pub fn filtered_entries(&self) -> impl Iterator<Item = &LogEntry> {
        let filter = self.level_filter;
        self.entries.iter().filter(move |e| match filter {
            Some(level) => e.level == level,
            None => true,
        })
    }

    /// Number of entries visible under the active level filter.
    ///
    /// O(1): returns the incrementally-maintained cache. Stays correct as long
    /// as `level_filter` is only changed via [`LogsState::recount_filtered`].
    pub fn filtered_len(&self) -> usize {
        self.filtered_count
    }

    /// Rebuild the cached filtered count from scratch (O(n)).
    ///
    /// Must be called after `level_filter` changes; everywhere else the count
    /// is maintained incrementally by `push`/`prepend`/`clear`/trim.
    pub fn recount_filtered(&mut self) {
        self.filtered_count = match self.level_filter {
            None => self.entries.len(),
            Some(level) => self.entries.iter().filter(|e| e.level == level).count(),
        };
    }

    /// Get the maximum scroll position (in filtered-index space)
    pub fn max_scroll(&self, viewport_height: usize) -> usize {
        self.filtered_len().saturating_sub(viewport_height)
    }

    /// Check if scroll is at the bottom
    pub fn is_at_bottom(&self, viewport_height: usize) -> bool {
        self.scroll_position >= self.max_scroll(viewport_height)
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self, viewport_height: usize) {
        self.scroll_position = self.max_scroll(viewport_height);
    }

    /// Get visible entries for rendering (respects the active level filter).
    /// Uses bounds-safe scroll position to prevent empty results.
    pub fn visible_entries(&self, viewport_height: usize) -> impl Iterator<Item = &LogEntry> {
        // Ensure scroll_position doesn't exceed valid range
        let safe_position = self.scroll_position.min(self.filtered_len().saturating_sub(1));
        self.filtered_entries()
            .skip(safe_position)
            .take(viewport_height)
    }
}

/// State for numeric input mode
#[derive(Debug, Clone, Default)]
pub struct NumericInputState {
    pub active: bool,
    pub value: String,
}

impl NumericInputState {
    pub fn start(&mut self) {
        self.active = true;
        self.value.clear();
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.value.clear();
    }

    pub fn push(&mut self, c: char) {
        if c.is_ascii_digit() && self.value.len() < 3 {
            self.value.push(c);
        }
    }

    pub fn pop(&mut self) {
        self.value.pop();
    }

    pub fn get_value(&self) -> Option<usize> {
        self.value.parse().ok()
    }
}

/// State for in-log search mode
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether the search input bar is open and capturing keystrokes.
    pub active: bool,
    /// Current search query.
    pub query: String,
    /// Positions (in filtered-index space) of entries matching the query.
    pub matches: Vec<usize>,
    /// Index into `matches` for the currently highlighted match.
    pub current: usize,
}

impl SearchState {
    /// Open the input bar and reset previous results.
    pub fn start(&mut self) {
        self.active = true;
        self.query.clear();
        self.matches.clear();
        self.current = 0;
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }

    /// Close the bar and drop the query so highlighting stops.
    pub fn cancel(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current = 0;
    }

    /// Advance to the next match, wrapping around. Returns its filtered position.
    pub fn next(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.matches.len();
        Some(self.matches[self.current])
    }

    /// Go to the previous match, wrapping around. Returns its filtered position.
    pub fn prev(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        self.current = (self.current + self.matches.len() - 1) % self.matches.len();
        Some(self.matches[self.current])
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            containers: Vec::new(),
            selected_container: 0,
            view_mode: ViewMode::Logs,
            navigation_mode: NavigationMode::Containers,
            menu_mode: MenuMode::None,
            logs: LogsState::default(),
            stats: None,
            numeric_input: NumericInputState::default(),
            search: SearchState::default(),
            notification: None,
            viewport_height: 20,
            logs_panel_height: 20, // Will be updated by UI on first render
            should_quit: false,
            last_refresh: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            stream_generation: 0,
            current_container_id: None,
            // v3.3.0: Rendering control
            needs_redraw: true,              // Initial render required
            force_full_redraw: true,         // Force clear on first render
            transition_state: TransitionState::Ready,
        }
    }
}

impl AppState {
    /// Get the currently selected container
    pub fn selected_container(&self) -> Option<&Container> {
        self.containers.get(self.selected_container)
    }

    /// Check if the selected container is running
    pub fn is_selected_running(&self) -> bool {
        self.selected_container()
            .map(|c| c.state == crate::docker::ContainerState::Running)
            .unwrap_or(false)
    }

    /// Clear notification if expired
    pub fn clear_expired_notification(&mut self) {
        if let Some(ref notif) = self.notification {
            if notif.is_expired() {
                self.notification = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(s: &str) -> LogEntry {
        LogEntry::from_raw(s)
    }

    #[test]
    fn push_trims_to_capacity_dropping_oldest() {
        let mut logs = LogsState {
            capacity: 3,
            ..Default::default()
        };
        for i in 0..5 {
            logs.push(entry(&format!("INFO line {i}")));
        }
        assert_eq!(logs.entries.len(), 3);
        assert!(logs.entries.front().unwrap().content.contains("line 2"));
        assert!(logs.entries.back().unwrap().content.contains("line 4"));
    }

    #[test]
    fn prepend_keeps_chronological_order_and_shifts_scroll() {
        let mut logs = LogsState::default();
        logs.push(entry("INFO new1"));
        logs.push(entry("INFO new2"));
        logs.scroll_position = 0;

        // Batch arrives oldest-first from the Docker API.
        logs.prepend(vec![entry("INFO old1"), entry("INFO old2")]);

        let order: Vec<&str> = logs.entries.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(order, vec!["INFO old1", "INFO old2", "INFO new1", "INFO new2"]);
        // No filter: scroll shifts by the full batch count to keep the same view.
        assert_eq!(logs.scroll_position, 2);
    }

    #[test]
    fn filter_changes_visible_count_and_entries() {
        let mut logs = LogsState::default();
        logs.push(entry("ERROR a"));
        logs.push(entry("INFO b"));
        logs.push(entry("ERROR c"));

        assert_eq!(logs.filtered_len(), 3);

        logs.level_filter = Some(LogLevel::Error);
        logs.recount_filtered();
        assert_eq!(logs.filtered_len(), 2);
        let got: Vec<&str> = logs.filtered_entries().map(|e| e.content.as_str()).collect();
        assert_eq!(got, vec!["ERROR a", "ERROR c"]);

        // max_scroll and visible_entries operate in filtered space.
        assert_eq!(logs.max_scroll(1), 1);
        let visible: Vec<&str> = logs.visible_entries(1).map(|e| e.content.as_str()).collect();
        assert_eq!(visible, vec!["ERROR a"]);
    }

    #[test]
    fn prepend_scroll_shift_counts_only_filtered_entries() {
        let mut logs = LogsState::default();
        logs.push(entry("ERROR e1"));
        logs.level_filter = Some(LogLevel::Error);
        logs.recount_filtered();
        logs.scroll_position = 0;

        // One matching (ERROR) + one non-matching (INFO) prepended.
        logs.prepend(vec![entry("INFO old"), entry("ERROR old2")]);

        // Only the visible (ERROR) entry shifts the filtered view.
        assert_eq!(logs.scroll_position, 1);
    }

    /// The incrementally-maintained `filtered_count` must always match a
    /// from-scratch recount across push / prepend / capacity trim / filter
    /// changes — otherwise `filtered_len()` (and scroll bounds) would drift.
    #[test]
    fn filtered_count_cache_never_drifts() {
        let brute = |logs: &LogsState| -> usize {
            match logs.level_filter {
                None => logs.entries.len(),
                Some(level) => logs.entries.iter().filter(|e| e.level == level).count(),
            }
        };

        let mut logs = LogsState {
            capacity: 4,
            ..Default::default()
        };

        // Push past capacity with mixed levels (exercises front-trim).
        for i in 0..8 {
            let level = if i % 2 == 0 { "ERROR" } else { "INFO" };
            logs.push(entry(&format!("{level} line {i}")));
            assert_eq!(logs.filtered_len(), brute(&logs), "after push {i}");
        }

        // Apply a filter and confirm the recount matches.
        logs.level_filter = Some(LogLevel::Error);
        logs.recount_filtered();
        assert_eq!(logs.filtered_len(), brute(&logs), "after filter set");

        // Prepend a mixed batch under an active filter (exercises back-trim).
        logs.prepend(vec![
            entry("ERROR old1"),
            entry("INFO old2"),
            entry("ERROR old3"),
        ]);
        assert_eq!(logs.filtered_len(), brute(&logs), "after prepend under filter");

        // Clearing the filter recounts to the full buffer length.
        logs.level_filter = None;
        logs.recount_filtered();
        assert_eq!(logs.filtered_len(), logs.entries.len(), "after filter clear");
        assert_eq!(logs.filtered_len(), brute(&logs));

        // Clear resets the cache to zero.
        logs.clear();
        assert_eq!(logs.filtered_len(), 0);
    }
}
