use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use chrono::Local;

use crate::app::{AppState, ViewMode, NavigationMode, MenuMode, LogLevel};
use crate::app::message::LogEntry;
use crate::app::state::TransitionState;
use crate::docker::ContainerState;

/// Short uppercase label for a log level (used in the filter indicator).
fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
        LogLevel::Unknown => "UNKNOWN",
    }
}

/// The terminal color used to render each log level.
fn level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Error => Color::Red,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Info => Color::Green,
        LogLevel::Debug => Color::Blue,
        LogLevel::Trace => Color::DarkGray,
        LogLevel::Unknown => Color::White,
    }
}

/// Build a styled log line, optionally highlighting search-query occurrences.
fn build_log_line<'a>(entry: &'a LogEntry, query_upper: Option<&str>) -> Line<'a> {
    let base = Style::default().fg(level_color(entry.level));
    match query_upper {
        Some(q) if !q.is_empty() => Line::from(highlight_spans(&entry.content, q, base)),
        _ => Line::from(Span::styled(entry.content.as_str(), base)),
    }
}

/// Split `content` into spans, highlighting ASCII case-insensitive occurrences of
/// `needle_upper` (which must already be uppercase). Match positions always fall on
/// char boundaries: an uppercase-ASCII needle can only match single-byte chars, so
/// the byte slicing below never lands mid-UTF-8.
fn highlight_spans<'a>(content: &'a str, needle_upper: &str, base: Style) -> Vec<Span<'a>> {
    let hl = base
        .bg(Color::Yellow)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let bytes = content.as_bytes();
    let n = needle_upper.len();
    let mut spans = Vec::new();
    let mut seg_start = 0;
    let mut i = 0;
    while n > 0 && i + n <= bytes.len() {
        let is_match = bytes[i..i + n]
            .iter()
            .zip(needle_upper.as_bytes())
            .all(|(a, b)| a.to_ascii_uppercase() == *b);
        if is_match {
            if seg_start < i {
                spans.push(Span::styled(&content[seg_start..i], base));
            }
            spans.push(Span::styled(&content[i..i + n], hl));
            i += n;
            seg_start = i;
        } else {
            i += 1;
        }
    }
    if seg_start < content.len() {
        spans.push(Span::styled(&content[seg_start..], base));
    }
    spans
}

/// Uppercased search query, or `None` when search is inactive/empty.
fn search_query_upper(state: &AppState) -> Option<String> {
    if state.search.query.is_empty() {
        None
    } else {
        Some(state.search.query.to_ascii_uppercase())
    }
}

/// Filter indicator suffix for log panel titles (empty when no filter is active).
fn filter_suffix(state: &AppState) -> String {
    match state.logs.level_filter {
        Some(level) => format!(" [filter:{}]", level_label(level)),
        None => String::new(),
    }
}

/// Pure rendering function - reads state, produces UI, no side effects
pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Main layout: header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    render_header(frame, chunks[0], state);

    // Content based on view mode
    match state.view_mode {
        ViewMode::LogsExpanded => {
            render_expanded_logs(frame, chunks[1], state);
        }
        _ => {
            // Split content: container list + logs/stats
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(35),  // Container list
                    Constraint::Min(0),      // Logs/Stats panel
                ])
                .split(chunks[1]);

            render_container_list(frame, content_chunks[0], state);

            match state.view_mode {
                ViewMode::Logs => render_logs_panel(frame, content_chunks[1], state),
                ViewMode::Stats => render_stats_panel(frame, content_chunks[1], state),
                _ => {}
            }
        }
    }

    render_footer(frame, chunks[2], state);

    // Overlay menus
    render_overlay_menus(frame, size, state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let header_text = vec![
        Line::from(vec![
            Span::styled(
                format!("Dockpit v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ),
            Span::raw(" | "),
            Span::styled(
                format!("{} containers", state.containers.len()),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" | "),
            Span::styled(
                Local::now().format("%H:%M:%S").to_string(),
                Style::default().fg(Color::Gray),
            ),
        ])
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    frame.render_widget(header, area);
}

fn render_container_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let border_color = if state.navigation_mode == NavigationMode::Containers {
        Color::Cyan
    } else {
        Color::Yellow
    };

    let items: Vec<ListItem> = state.containers
        .iter()
        .enumerate()
        .map(|(i, container)| {
            let status_symbol = match container.state {
                ContainerState::Running => "●",
                ContainerState::Paused => "⏸",
                ContainerState::Stopped => "○",
                ContainerState::Dead => "✗",
                ContainerState::Restarting => "↻",
            };

            let status_color = match container.state {
                ContainerState::Running => Color::Green,
                ContainerState::Paused => Color::Yellow,
                ContainerState::Stopped => Color::Red,
                ContainerState::Dead => Color::DarkGray,
                ContainerState::Restarting => Color::Blue,
            };

            let content = Line::from(vec![
                Span::raw(format!("{:2}. ", i + 1)),
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::raw(&container.name),
            ]);

            ListItem::new(content)
        })
        .collect();

    let containers_list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Containers "))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_container));
    frame.render_stateful_widget(containers_list, area, &mut list_state);
}

fn render_logs_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let viewport_height = (area.height as usize).saturating_sub(2);
    let border_color = if state.navigation_mode == NavigationMode::Logs {
        Color::Cyan
    } else {
        Color::Yellow
    };

    // Check if we're in loading state for this panel
    let is_loading = matches!(state.transition_state, TransitionState::Loading(_));

    // Get visible log entries (highlighting search matches when searching)
    let query_upper = search_query_upper(state);
    let logs_text: Vec<Line> = state.logs
        .visible_entries(viewport_height)
        .map(|entry| build_log_line(entry, query_upper.as_deref()))
        .collect();

    let total = state.logs.filtered_len();

    // Build title with loading + filter indicators
    let loading_suffix = if state.logs.is_loading_more || is_loading { " ⏳" } else { "" };
    let filter = filter_suffix(state);
    let title = if let Some(container) = state.selected_container() {
        format!(
            " Logs: {} [{}/{}]{}{} ",
            container.name,
            state.logs.scroll_position + 1,
            total.max(1),
            filter,
            loading_suffix
        )
    } else {
        format!(" Logs{}{} ", filter, loading_suffix)
    };

    let title_style = if state.logs.is_loading_more || is_loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let logs = Paragraph::new(logs_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title)
            .title_style(title_style))
        .wrap(Wrap { trim: false });

    frame.render_widget(logs, area);

    // Scrollbar (based on the filtered count, matching scroll_position's space)
    if total > viewport_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.logs.scroll_position);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    // Show loading overlay if in transition state
    if let TransitionState::Loading(ref message) = state.transition_state {
        render_loading_overlay(frame, area, message);
    }
}

fn render_expanded_logs(frame: &mut Frame, area: Rect, state: &AppState) {
    let viewport_height = (area.height as usize).saturating_sub(2);

    // Check if we're in loading state
    let is_loading = matches!(state.transition_state, TransitionState::Loading(_));

    let query_upper = search_query_upper(state);
    let logs_text: Vec<Line> = state.logs
        .visible_entries(viewport_height)
        .map(|entry| build_log_line(entry, query_upper.as_deref()))
        .collect();

    let total = state.logs.filtered_len();

    // Build title with loading + filter indicators
    let loading_suffix = if state.logs.is_loading_more || is_loading { " ⏳" } else { "" };
    let filter = filter_suffix(state);
    let title = if let Some(container) = state.selected_container() {
        format!(
            " Logs (Expanded): {} [{}/{}]{}{} - Press F to minimize ",
            container.name,
            state.logs.scroll_position + 1,
            total.max(1),
            filter,
            loading_suffix
        )
    } else {
        format!(" Logs (Expanded){}{} - Press F to minimize ", filter, loading_suffix)
    };

    let title_style = if state.logs.is_loading_more || is_loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let logs = Paragraph::new(logs_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title)
            .title_style(title_style))
        .wrap(Wrap { trim: false });

    frame.render_widget(logs, area);

    // Scrollbar (based on the filtered count, matching scroll_position's space)
    if total > viewport_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.logs.scroll_position);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    // Show loading overlay if in transition state
    if let TransitionState::Loading(ref message) = state.transition_state {
        render_loading_overlay(frame, area, message);
    }
}

fn render_stats_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let border_color = if state.navigation_mode == NavigationMode::Stats {
        Color::Cyan
    } else {
        Color::Yellow
    };

    // Check if we're in loading state
    let is_loading = matches!(state.transition_state, TransitionState::Loading(_));
    let loading_suffix = if is_loading { " ⏳" } else { "" };

    let title = if let Some(container) = state.selected_container() {
        format!(" Stats: {}{} ", container.name, loading_suffix)
    } else {
        format!(" Stats{} ", loading_suffix)
    };

    let title_style = if is_loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let stats_text = if let Some(stats) = &state.stats {
        vec![
            Line::from(vec![
                Span::raw("CPU: "),
                Span::styled(
                    format!("{:.2}%", stats.cpu_percent),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw("Memory: "),
                Span::styled(
                    format!("{:.2} MB / {:.2} MB ({:.2}%)",
                        stats.memory_usage as f64 / 1_048_576.0,
                        stats.memory_limit as f64 / 1_048_576.0,
                        stats.memory_percent
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Network RX: "),
                Span::styled(
                    format_bytes(stats.network_rx),
                    Style::default().fg(Color::Blue),
                ),
            ]),
            Line::from(vec![
                Span::raw("Network TX: "),
                Span::styled(
                    format_bytes(stats.network_tx),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Disk Read: "),
                Span::styled(
                    format_bytes(stats.block_read),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("Disk Write: "),
                Span::styled(
                    format_bytes(stats.block_write),
                    Style::default().fg(Color::Red),
                ),
            ]),
        ]
    } else if state.is_selected_running() {
        vec![Line::from(Span::styled(
            "Loading stats...",
            Style::default().fg(Color::Gray),
        ))]
    } else {
        vec![Line::from(Span::styled(
            "Container is not running",
            Style::default().fg(Color::Gray),
        ))]
    };

    let stats = Paragraph::new(stats_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title)
            .title_style(title_style));

    frame.render_widget(stats, area);

    // Show loading overlay if in transition state
    if let TransitionState::Loading(ref message) = state.transition_state {
        render_loading_overlay(frame, area, message);
    }
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let help_text = match state.menu_mode {
        MenuMode::DockerOps => "1:Start 2:Stop 3:Restart 4:Pause 5:Unpause 6:Remove | ESC:Close",
        MenuMode::Clipboard => "1:Last100 2:Last500 3:Visible 4:FromPos 5:All 6:Export 7:Print | ESC:Close",
        MenuMode::None => match state.navigation_mode {
            NavigationMode::Containers => "↑↓:Select | L:Logs S:Stats | /:Search Tab:Filter | D:Docker C:Copy R:Restart | #:Jump | Q:Quit",
            NavigationMode::Logs | NavigationMode::Stats => "↑↓:Scroll | /:Search n/N:Match Tab:Filter | Home/End ←:Back F:Expand | Q:Quit",
        },
    };

    // Show notification if present
    let footer_content = if let Some(ref notif) = state.notification {
        let color = if notif.is_error { Color::Red } else { Color::Green };
        vec![
            Line::from(Span::styled(&notif.message, Style::default().fg(color).add_modifier(Modifier::BOLD))),
        ]
    } else if state.search.active {
        vec![
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::styled(&state.search.query, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::RAPID_BLINK)),
                Span::styled("  (Enter to search, Esc to cancel)", Style::default().fg(Color::DarkGray)),
            ])
        ]
    } else if state.numeric_input.active {
        vec![
            Line::from(vec![
                Span::styled("Jump to container: ", Style::default().fg(Color::Yellow)),
                Span::styled(&state.numeric_input.value, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::RAPID_BLINK)),
            ])
        ]
    } else {
        vec![Line::from(Span::styled(help_text, Style::default().fg(Color::Gray)))]
    };

    let footer = Paragraph::new(footer_content)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

fn render_overlay_menus(frame: &mut Frame, area: Rect, state: &AppState) {
    match state.menu_mode {
        MenuMode::DockerOps => render_docker_ops_menu(frame, area, state),
        MenuMode::Clipboard => render_clipboard_menu(frame, area),
        MenuMode::None => {}
    }
}

fn render_docker_ops_menu(frame: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::widgets::Clear;

    let container_name = state.selected_container()
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Solid menu palette - high contrast so it never blends with the logs behind it.
    let bg = Color::Blue;
    let num = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let menu_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  1. ", num), Span::raw("Start container")]),
        Line::from(vec![Span::styled("  2. ", num), Span::raw("Stop container")]),
        Line::from(vec![Span::styled("  3. ", num), Span::raw("Restart container")]),
        Line::from(vec![Span::styled("  4. ", num), Span::raw("Pause container")]),
        Line::from(vec![Span::styled("  5. ", num), Span::raw("Unpause container")]),
        Line::from(vec![
            Span::styled("  6. ", num),
            Span::styled("Remove container", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ESC to close", Style::default().fg(Color::Gray))),
    ];

    let menu_width = 35;
    let menu_height = 12;
    let menu_area = centered_rect(menu_width, menu_height, area);

    // Wipe whatever is behind the menu so the background is fully opaque.
    frame.render_widget(Clear, menu_area);

    let menu = Paragraph::new(menu_text)
        .style(Style::default().bg(bg).fg(Color::White))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White).bg(bg).add_modifier(Modifier::BOLD))
            .title(format!(" Docker: {} ", container_name))
            .title_style(Style::default().fg(Color::White).bg(bg).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(bg)));

    frame.render_widget(menu, menu_area);
}

fn render_clipboard_menu(frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let bg = Color::Blue;
    let num = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let menu_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  1. ", num), Span::raw("Copy last 100 lines")]),
        Line::from(vec![Span::styled("  2. ", num), Span::raw("Copy last 500 lines")]),
        Line::from(vec![Span::styled("  3. ", num), Span::raw("Copy visible content")]),
        Line::from(vec![Span::styled("  4. ", num), Span::raw("Copy from current position")]),
        Line::from(vec![Span::styled("  5. ", num), Span::raw("Copy all logs")]),
        Line::from(vec![
            Span::styled("  6. ", num),
            Span::styled("Export all logs to file", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  7. ", num),
            Span::styled("Print to terminal (manual copy / SSH)", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ESC to close", Style::default().fg(Color::Gray))),
    ];

    let menu_width = 44;
    let menu_height = 13;
    let menu_area = centered_rect(menu_width, menu_height, area);

    // Wipe whatever is behind the menu so the background is fully opaque.
    frame.render_widget(Clear, menu_area);

    let menu = Paragraph::new(menu_text)
        .style(Style::default().bg(bg).fg(Color::White))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White).bg(bg).add_modifier(Modifier::BOLD))
            .title(" Clipboard ")
            .title_style(Style::default().fg(Color::White).bg(bg).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(bg)));

    frame.render_widget(menu, menu_area);
}

/// Helper to create a centered rect
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect {
        x: area.x + x,
        y: area.y + y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Render loading overlay within a specific panel area (v3.4.0)
/// Shows a centered loading box overlay without replacing the entire view
fn render_loading_overlay(frame: &mut Frame, area: Rect, message: &str) {
    use ratatui::widgets::Clear;

    // Calculate overlay size - smaller for panel overlay
    let overlay_width = 40u16.min(area.width.saturating_sub(4));
    let overlay_height = 5u16.min(area.height.saturating_sub(2));

    // Center within the panel area
    let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;

    let overlay_area = Rect {
        x,
        y,
        width: overlay_width,
        height: overlay_height,
    };

    // Clear the overlay area first
    frame.render_widget(Clear, overlay_area);

    let loading_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("⏳ {}", message),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let loading_box = Paragraph::new(loading_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Loading ")
            .style(Style::default().bg(Color::DarkGray)))
        .alignment(Alignment::Center);

    frame.render_widget(loading_box, overlay_area);
}
