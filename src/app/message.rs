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
    HistoricalLogsLoaded {
        logs: Vec<LogEntry>,
        has_more: bool,
    },
    StatsReceived(Stats),

    // === Operation Results ===
    OperationSuccess(String),
    OperationError(String),

    // === Clipboard Operations ===
    ClipboardSuccess(String),
    ClipboardError(String),

    // === System Events ===
    Quit,
    Resize(u16),
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
        let level = detect_log_level(content);

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

/// ASCII case-insensitive substring search.
///
/// Avoids allocating an uppercased copy of `haystack` (which `detect_log_level`
/// used to do on *every* log line via `to_uppercase()`). `needle` must already
/// be uppercase ASCII. Also reused by the in-log search feature.
pub fn contains_ci(haystack: &str, needle: &str) -> bool {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    if n.is_empty() {
        return true;
    }
    if h.len() < n.len() {
        return false;
    }
    h.windows(n.len())
        .any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_uppercase() == *b))
}

/// Detect log level from content
fn detect_log_level(content: &str) -> LogLevel {
    if contains_ci(content, "ERROR") || contains_ci(content, "ERR]") || contains_ci(content, "[E]") {
        LogLevel::Error
    } else if contains_ci(content, "WARN") || contains_ci(content, "WRN]") || contains_ci(content, "[W]") {
        LogLevel::Warn
    } else if contains_ci(content, "INFO") || contains_ci(content, "INF]") || contains_ci(content, "[I]") {
        LogLevel::Info
    } else if contains_ci(content, "DEBUG") || contains_ci(content, "DBG]") || contains_ci(content, "[D]") {
        LogLevel::Debug
    } else if contains_ci(content, "TRACE") || contains_ci(content, "TRC]") || contains_ci(content, "[T]") {
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

    /// Dump log content to the terminal's normal scrollback so the user can
    /// select it with the mouse (Ctrl+Shift+C). The only way to copy many
    /// lines over SSH on terminals without OSC 52 support (e.g. GNOME Terminal).
    PrintForManualCopy(String),

    /// Export log content to a timestamped file on disk
    ExportLogs { content: String, container_name: String },

    /// Schedule a tick (for periodic refresh)
    ScheduleTick(std::time::Duration),

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_ci_is_ascii_case_insensitive() {
        assert!(contains_ci("Hello ERROR world", "ERROR"));
        assert!(contains_ci("hello error world", "ERROR"));
        assert!(contains_ci("WaRnInG: x", "WARN"));
        assert!(!contains_ci("all good here", "ERROR"));
        assert!(contains_ci("anything", "")); // empty needle matches
        assert!(!contains_ci("ab", "ABC")); // needle longer than haystack
    }

    #[test]
    fn contains_ci_handles_multibyte_haystack() {
        // Non-ASCII bytes must never be mistaken for an ASCII match.
        assert!(contains_ci("café ERROR", "ERROR"));
        assert!(!contains_ci("ñoño", "NO"));
    }

    #[test]
    fn detect_log_level_classifies_by_keyword() {
        assert_eq!(detect_log_level("ERROR boom"), LogLevel::Error);
        assert_eq!(detect_log_level("a warning happened"), LogLevel::Warn);
        assert_eq!(detect_log_level("INFO started"), LogLevel::Info);
        assert_eq!(detect_log_level("debug trace here"), LogLevel::Debug);
        assert_eq!(detect_log_level("plain message"), LogLevel::Unknown);
    }
}
