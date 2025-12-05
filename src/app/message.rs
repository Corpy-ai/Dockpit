use crossterm::event::KeyEvent;
use crate::docker::{Container, Stats};

/// All possible messages/events in the application.
/// This enables unidirectional data flow: Event -> Message -> Update -> View
#[derive(Debug, Clone)]
pub enum Message {
    // === Input Events ===
    KeyPressed(KeyEvent),
    Tick,

    // === Data Events ===
    ContainersLoaded(Vec<Container>),
    /// Log received from container stream - includes container_id and generation for validation
    LogReceived {
        container_id: String,
        generation: u64,
        content: String,
    },
    LogsBatchReceived(Vec<String>),
    HistoricalLogsLoaded {
        logs: Vec<LogEntry>,
        has_more: bool,
    },
    StatsReceived(Stats),

    // === Navigation Events ===
    SelectContainer(usize),
    SelectPreviousContainer,
    SelectNextContainer,
    JumpToContainer(usize),

    // === Scroll Events ===
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToTop,
    ScrollToBottom,
    ScrollPageUp,
    ScrollPageDown,
    LoadMoreLogs,  // Triggered when reaching top of logs

    // === View Mode Events ===
    SwitchToLogsView,
    SwitchToStatsView,
    ToggleExpandedLogs,
    SwitchToContainersNav,
    SwitchToLogsNav,

    // === Menu Events ===
    OpenDockerOpsMenu,
    OpenClipboardMenu,
    CloseMenu,

    // === Docker Operations ===
    StartContainer(String),
    StopContainer(String),
    RestartContainer(String),
    PauseContainer(String),
    UnpauseContainer(String),
    RemoveContainer { id: String, force: bool },

    // === Operation Results ===
    OperationSuccess(String),
    OperationError(String),

    // === Clipboard Operations ===
    CopyLogsLast(usize),      // Copy last N lines
    CopyLogsVisible,          // Copy visible lines
    CopyLogsAll,              // Copy all logs
    CopyLogsFromPosition,     // Copy from current scroll position
    ClipboardSuccess(String),
    ClipboardError(String),

    // === Numeric Input ===
    StartNumericInput,
    NumericInputChar(char),
    NumericInputSubmit,
    NumericInputCancel,
    NumericInputBackspace,

    // === System Events ===
    Quit,
    Resize(u16, u16),

    // === Stream Management ===
    StartLogsStream(String),   // container_id
    StartStatsStream(String),  // container_id
    StopAllStreams,
}

/// A parsed log entry with timestamp for efficient pagination
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub content: String,
    pub level: LogLevel,
}

impl LogEntry {
    pub fn from_raw(raw: &str) -> Self {
        let (timestamp, content) = parse_timestamp(raw);
        let level = detect_log_level(&content);

        Self {
            timestamp,
            content: content.to_string(),
            level,
        }
    }
}

/// Log level for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
    Unknown,
}

/// Parse Docker log timestamp (format: 2024-01-15T10:30:45.123456789Z)
fn parse_timestamp(raw: &str) -> (Option<chrono::DateTime<chrono::Utc>>, &str) {
    // Docker timestamps are at the start, followed by space
    if raw.len() > 30 && raw.chars().nth(4) == Some('-') {
        if let Some(space_idx) = raw.find(' ') {
            let ts_str = &raw[..space_idx];
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                return (Some(ts.with_timezone(&chrono::Utc)), &raw[space_idx + 1..]);
            }
        }
    }
    (None, raw)
}

/// Detect log level from content
fn detect_log_level(content: &str) -> LogLevel {
    let upper = content.to_uppercase();
    if upper.contains("ERROR") || upper.contains("ERR]") || upper.contains("[E]") {
        LogLevel::Error
    } else if upper.contains("WARN") || upper.contains("WRN]") || upper.contains("[W]") {
        LogLevel::Warn
    } else if upper.contains("INFO") || upper.contains("INF]") || upper.contains("[I]") {
        LogLevel::Info
    } else if upper.contains("DEBUG") || upper.contains("DBG]") || upper.contains("[D]") {
        LogLevel::Debug
    } else if upper.contains("TRACE") || upper.contains("TRC]") || upper.contains("[T]") {
        LogLevel::Trace
    } else {
        LogLevel::Unknown
    }
}

/// Side effects that result from state updates
#[derive(Debug, Clone)]
pub enum Effect {
    /// Load containers list from Docker
    LoadContainers,

    /// Start streaming logs for a container - includes generation counter for race condition prevention
    StartLogsStream { container_id: String, initial_lines: usize, generation: u64 },

    /// Load historical logs (for infinite scroll)
    LoadHistoricalLogs { container_id: String, before_timestamp: Option<chrono::DateTime<chrono::Utc>>, batch_size: usize },

    /// Start streaming stats for a container
    StartStatsStream { container_id: String },

    /// Stop all active streams
    StopAllStreams,

    /// Execute Docker operation
    DockerOperation(DockerOp),

    /// Copy to clipboard
    CopyToClipboard(String),

    /// Schedule a tick (for periodic refresh)
    ScheduleTick(std::time::Duration),

    /// Force a full redraw (use sparingly!)
    ForceRedraw,

    /// Quit the application
    Quit,
}

/// Docker operations
#[derive(Debug, Clone)]
pub enum DockerOp {
    Start(String),
    Stop(String),
    Restart(String),
    Pause(String),
    Unpause(String),
    Remove { id: String, force: bool },
}
