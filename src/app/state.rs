use std::collections::VecDeque;
use crate::docker::{Container, Stats};
use crate::app::message::LogEntry;

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
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionState {
    Loading(String),
    Ready,
}

impl Default for TransitionState {
    fn default() -> Self {
        Self::Ready
    }
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
    pub notification: Option<Notification>,
    pub viewport_height: usize,

    // === System State ===
    pub should_quit: bool,
    pub last_refresh: std::time::Instant,

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
        }
    }
}

impl LogsState {
    /// Clear all logs and reset state
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_position = 0;
        self.is_loading_more = false;
        self.has_more_history = true;
        self.oldest_timestamp = None;
        self.total_loaded = 0;
    }

    /// Push a new log entry to the end
    pub fn push(&mut self, entry: LogEntry) {
        // Update oldest timestamp if this is the first entry
        if self.entries.is_empty() {
            self.oldest_timestamp = entry.timestamp;
        }

        self.entries.push_back(entry);
        self.total_loaded += 1;

        // Trim from front if over capacity
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Prepend historical entries (for infinite scroll)
    /// v3.2.2: Fixed timestamp tracking - now correctly extracts oldest BEFORE reverse iteration
    pub fn prepend(&mut self, entries: Vec<LogEntry>) {
        if entries.is_empty() {
            return;
        }

        let count = entries.len();

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

        // Insert in reverse order so oldest ends up at front
        for entry in entries.into_iter().rev() {
            self.entries.push_front(entry);
        }

        self.total_loaded += count;

        // Trim from back if over capacity
        while self.entries.len() > self.capacity {
            self.entries.pop_back();
        }

        // ALWAYS adjust scroll position to maintain current view
        // When historical logs are prepended, shift scroll by the count to keep
        // viewing the same logs (not the newly loaded historical ones)
        self.scroll_position = self.scroll_position.saturating_add(count);
    }

    /// Get the maximum scroll position
    pub fn max_scroll(&self, viewport_height: usize) -> usize {
        self.entries.len().saturating_sub(viewport_height)
    }

    /// Check if scroll is at the bottom
    pub fn is_at_bottom(&self, viewport_height: usize) -> bool {
        self.scroll_position >= self.max_scroll(viewport_height)
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self, viewport_height: usize) {
        self.scroll_position = self.max_scroll(viewport_height);
    }

    /// Get visible entries for rendering
    /// Uses bounds-safe scroll position to prevent empty results
    pub fn visible_entries(&self, viewport_height: usize) -> impl Iterator<Item = &LogEntry> {
        // Ensure scroll_position doesn't exceed valid range
        let safe_position = self.scroll_position.min(self.entries.len().saturating_sub(1));
        self.entries
            .iter()
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
            notification: None,
            viewport_height: 20,
            should_quit: false,
            last_refresh: std::time::Instant::now(),
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

    /// Get the ID of the currently selected container
    pub fn selected_container_id(&self) -> Option<&str> {
        self.selected_container().map(|c| c.id.as_str())
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
