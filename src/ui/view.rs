use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use chrono::Local;

use crate::app::{AppState, ViewMode, NavigationMode, MenuMode, LogLevel};
use crate::app::state::TransitionState;
use crate::docker::ContainerState;

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
                format!("Docker Manager v{}", env!("CARGO_PKG_VERSION")),
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

    // Get visible log entries
    let logs_text: Vec<Line> = state.logs
        .visible_entries(viewport_height)
        .map(|entry| {
            let level_color = match entry.level {
                LogLevel::Error => Color::Red,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Info => Color::Green,
                LogLevel::Debug => Color::Blue,
                LogLevel::Trace => Color::DarkGray,
                LogLevel::Unknown => Color::White,
            };
            Line::from(Span::styled(&entry.content, Style::default().fg(level_color)))
        })
        .collect();

    // Build title with loading indicator if active
    let loading_suffix = if state.logs.is_loading_more || is_loading { " ⏳" } else { "" };
    let title = if let Some(container) = state.selected_container() {
        format!(
            " Logs: {} [{}/{}]{} ",
            container.name,
            state.logs.scroll_position + 1,
            state.logs.entries.len().max(1),
            loading_suffix
        )
    } else {
        format!(" Logs{} ", loading_suffix)
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

    // Scrollbar
    if state.logs.entries.len() > viewport_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(state.logs.entries.len())
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

    let logs_text: Vec<Line> = state.logs
        .visible_entries(viewport_height)
        .map(|entry| {
            let level_color = match entry.level {
                LogLevel::Error => Color::Red,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Info => Color::Green,
                LogLevel::Debug => Color::Blue,
                LogLevel::Trace => Color::DarkGray,
                LogLevel::Unknown => Color::White,
            };
            Line::from(Span::styled(&entry.content, Style::default().fg(level_color)))
        })
        .collect();

    // Build title with loading indicator if active
    let loading_suffix = if state.logs.is_loading_more || is_loading { " ⏳" } else { "" };
    let title = if let Some(container) = state.selected_container() {
        format!(
            " Logs (Expanded): {} [{}/{}]{} - Press F to minimize ",
            container.name,
            state.logs.scroll_position + 1,
            state.logs.entries.len().max(1),
            loading_suffix
        )
    } else {
        format!(" Logs (Expanded){} - Press F to minimize ", loading_suffix)
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

    // Scrollbar
    if state.logs.entries.len() > viewport_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(state.logs.entries.len())
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
        MenuMode::Clipboard => "1:Last100 2:Last500 3:Visible 4:FromPos 5:All | ESC:Close",
        MenuMode::None => match state.navigation_mode {
            NavigationMode::Containers => "↑↓:Select | L:Logs S:Stats | D:Docker C:Copy R:Restart | N/#:Jump | Q:Quit",
            NavigationMode::Logs | NavigationMode::Stats => "↑↓:Scroll | PgUp/PgDn:Page | Home/End | ←:Back F:Expand | Q:Quit",
        },
    };

    // Show notification if present
    let footer_content = if let Some(ref notif) = state.notification {
        let color = if notif.is_error { Color::Red } else { Color::Green };
        vec![
            Line::from(Span::styled(&notif.message, Style::default().fg(color).add_modifier(Modifier::BOLD))),
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
    let container_name = state.selected_container()
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    let menu_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  1. ", Style::default().fg(Color::Yellow)),
            Span::raw("Start container"),
        ]),
        Line::from(vec![
            Span::styled("  2. ", Style::default().fg(Color::Yellow)),
            Span::raw("Stop container"),
        ]),
        Line::from(vec![
            Span::styled("  3. ", Style::default().fg(Color::Yellow)),
            Span::raw("Restart container"),
        ]),
        Line::from(vec![
            Span::styled("  4. ", Style::default().fg(Color::Yellow)),
            Span::raw("Pause container"),
        ]),
        Line::from(vec![
            Span::styled("  5. ", Style::default().fg(Color::Yellow)),
            Span::raw("Unpause container"),
        ]),
        Line::from(vec![
            Span::styled("  6. ", Style::default().fg(Color::Yellow)),
            Span::styled("Remove container", Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ESC to close", Style::default().fg(Color::Gray))),
    ];

    let menu_width = 35;
    let menu_height = 12;
    let menu_area = centered_rect(menu_width, menu_height, area);

    let menu = Paragraph::new(menu_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(format!(" Docker: {} ", container_name))
            .style(Style::default().bg(Color::DarkGray)));

    frame.render_widget(menu, menu_area);
}

fn render_clipboard_menu(frame: &mut Frame, area: Rect) {
    let menu_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  1. ", Style::default().fg(Color::Yellow)),
            Span::raw("Copy last 100 lines"),
        ]),
        Line::from(vec![
            Span::styled("  2. ", Style::default().fg(Color::Yellow)),
            Span::raw("Copy last 500 lines"),
        ]),
        Line::from(vec![
            Span::styled("  3. ", Style::default().fg(Color::Yellow)),
            Span::raw("Copy visible content"),
        ]),
        Line::from(vec![
            Span::styled("  4. ", Style::default().fg(Color::Yellow)),
            Span::raw("Copy from current position"),
        ]),
        Line::from(vec![
            Span::styled("  5. ", Style::default().fg(Color::Yellow)),
            Span::raw("Copy all logs"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ESC to close", Style::default().fg(Color::Gray))),
    ];

    let menu_width = 35;
    let menu_height = 11;
    let menu_area = centered_rect(menu_width, menu_height, area);

    let menu = Paragraph::new(menu_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Clipboard ")
            .style(Style::default().bg(Color::DarkGray)));

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

/// Render loading screen during transitions (v3.3.0)
/// Shows a centered loading box with the given message
/// NOTE: This function is kept for backwards compatibility but is no longer used
/// in favor of render_loading_overlay which shows loading only in the panel area
#[allow(dead_code)]
pub fn render_loading_screen(frame: &mut Frame, message: &str) {
    use ratatui::widgets::Clear;
    let size = frame.area();

    // Clear entire screen first to prevent ghost characters
    frame.render_widget(Clear, size);

    // Calculate centered position
    let loading_width = 50u16;
    let loading_height = 7u16;
    let x = (size.width.saturating_sub(loading_width)) / 2;
    let y = (size.height.saturating_sub(loading_height)) / 2;

    let loading_area = Rect {
        x: size.x + x,
        y: size.y + y,
        width: loading_width.min(size.width),
        height: loading_height.min(size.height),
    };

    let loading_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Loading  ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(message, Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Please wait...", Style::default().fg(Color::Gray))),
    ];

    let loading_box = Paragraph::new(loading_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Loading ")
            .style(Style::default().bg(Color::DarkGray)))
        .alignment(Alignment::Center);

    frame.render_widget(loading_box, loading_area);
}
