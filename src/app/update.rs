use crossterm::event::KeyCode;
use crate::app::message::{contains_ci, Message, Effect, DockerOp, LogEntry, LogLevel};
use crate::app::state::{AppState, ViewMode, NavigationMode, MenuMode, Notification, TransitionState};

/// Cycle through level filters: All → Error → Warn → Info → Debug → Trace → All.
fn cycle_filter(current: Option<LogLevel>) -> Option<LogLevel> {
    match current {
        None => Some(LogLevel::Error),
        Some(LogLevel::Error) => Some(LogLevel::Warn),
        Some(LogLevel::Warn) => Some(LogLevel::Info),
        Some(LogLevel::Info) => Some(LogLevel::Debug),
        Some(LogLevel::Debug) => Some(LogLevel::Trace),
        Some(LogLevel::Trace) | Some(LogLevel::Unknown) => None,
    }
}

/// Number of historical logs to load per batch when scrolling up
/// Using 50 for smooth incremental loading experience
const HISTORICAL_BATCH_SIZE: usize = 50;

/// Pure function that handles state transitions.
/// Given the current state and a message, returns the new state and any side effects.
///
/// This function is the heart of the Elm Architecture:
/// - It NEVER performs I/O directly
/// - It NEVER mutates global state
/// - It returns a new state and a list of effects to execute
pub fn update(mut state: AppState, message: Message) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();

    match message {
        // === Input Events ===
        Message::KeyPressed(key) => {
            // Any keypress counts as activity (drives adaptive tick cadence).
            state.last_activity = std::time::Instant::now();

            // Handle search input mode first - captures every key, including digits
            if state.search.active {
                match key.code {
                    KeyCode::Char(c) => state.search.push(c),
                    KeyCode::Backspace => state.search.pop(),
                    KeyCode::Esc => state.search.cancel(),
                    KeyCode::Enter => {
                        let needle = state.search.query.to_ascii_uppercase();
                        if needle.is_empty() {
                            state.search.cancel();
                        } else {
                            let matches: Vec<usize> = state.logs
                                .filtered_entries()
                                .enumerate()
                                .filter(|(_, e)| contains_ci(&e.content, &needle))
                                .map(|(i, _)| i)
                                .collect();
                            if let Some(&first) = matches.first() {
                                state.search.matches = matches;
                                state.search.current = 0;
                                state.search.active = false;
                                state.logs.scroll_position = first;
                            } else {
                                let q = state.search.query.clone();
                                state.search.cancel();
                                state.notification = Some(Notification::error(
                                    format!("No matches for '{}'", q)
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                state.needs_redraw = true;
                return (state, effects);
            }

            // Handle numeric input mode
            if state.numeric_input.active {
                match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        state.numeric_input.push(c);
                    }
                    KeyCode::Enter => {
                        if let Some(num) = state.numeric_input.get_value() {
                            let index = num.saturating_sub(1);
                            if index < state.containers.len() {
                                // v3.3.0: Start transition with loading screen
                                state.transition_state = TransitionState::Loading(
                                    format!("Jumping to container #{}...", num)
                                );
                                state.force_full_redraw = true;
                                state.needs_redraw = true;
                                // v3.2.2: Increment generation FIRST to invalidate pending messages
                                state.stream_generation = state.stream_generation.wrapping_add(1);
                                state.selected_container = index;
                                // v3.2.2: Clear logs IMMEDIATELY
                                state.logs.clear();
                                state.stats = None;
                                effects.push(Effect::StopAllStreams);
                                if let Some(container) = state.containers.get(index) {
                                    state.current_container_id = Some(container.id.clone());
                                    effects.push(Effect::StartLogsStream {
                                        container_id: container.id.clone(),
                                        initial_lines: 100,
                                        generation: state.stream_generation,
                                    });
                                }
                            } else {
                                state.notification = Some(Notification::error(
                                    format!("Container #{} does not exist", num)
                                ));
                            }
                        }
                        state.numeric_input.cancel();
                    }
                    KeyCode::Esc => {
                        state.numeric_input.cancel();
                    }
                    KeyCode::Backspace => {
                        state.numeric_input.pop();
                    }
                    _ => {}
                }
                return (state, effects);
            }

            // Handle menu mode
            match state.menu_mode {
                MenuMode::DockerOps => {
                    return handle_docker_ops_menu(state, key.code);
                }
                MenuMode::Clipboard => {
                    return handle_clipboard_menu(state, key.code);
                }
                MenuMode::None => {}
            }

            // Handle normal navigation
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    state.should_quit = true;
                    effects.push(Effect::Quit);
                }

                // Numeric shortcuts for container selection
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    state.numeric_input.start();
                    state.numeric_input.push(c);
                }

                // In-log search
                KeyCode::Char('/') => {
                    state.search.start();
                    state.needs_redraw = true;
                }
                KeyCode::Char('n') => {
                    if let Some(pos) = state.search.next() {
                        state.logs.scroll_position = pos;
                        state.needs_redraw = true;
                    }
                }
                KeyCode::Char('N') => {
                    if let Some(pos) = state.search.prev() {
                        state.logs.scroll_position = pos;
                        state.needs_redraw = true;
                    }
                }

                // Cycle log-level filter
                KeyCode::Tab => {
                    state.logs.level_filter = cycle_filter(state.logs.level_filter);
                    // Filter changed → rebuild the O(1) filtered-count cache.
                    state.logs.recount_filtered();
                    state.logs.scroll_position = state.logs.scroll_position
                        .min(state.logs.max_scroll(state.logs_panel_height));
                    state.force_full_redraw = true;
                    state.needs_redraw = true;
                }

                // Navigation
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                    (state, effects) = handle_navigate_up(state);
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                    (state, effects) = handle_navigate_down(state);
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                    state.navigation_mode = NavigationMode::Containers;
                }
                KeyCode::Right => {
                    if state.view_mode == ViewMode::Logs || state.view_mode == ViewMode::LogsExpanded {
                        state.navigation_mode = NavigationMode::Logs;
                    } else {
                        state.navigation_mode = NavigationMode::Stats;
                    }
                }

                // View mode switches (l/L and s/S are symmetric; already-in-view just refocuses)
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    if matches!(state.view_mode, ViewMode::Logs | ViewMode::LogsExpanded) {
                        // Already viewing logs: just focus the logs panel (old `l` behavior).
                        state.navigation_mode = NavigationMode::Logs;
                        state.needs_redraw = true;
                    } else {
                        // v3.3.0: Start transition with loading screen
                        state.transition_state = TransitionState::Loading("Loading logs...".to_string());
                        state.force_full_redraw = true;
                        state.needs_redraw = true;
                        state.view_mode = ViewMode::Logs;
                        state.navigation_mode = NavigationMode::Logs;
                        // v3.2.2: Increment generation for view mode switch
                        state.stream_generation = state.stream_generation.wrapping_add(1);
                        state.logs.clear();
                        state.stats = None;
                        effects.push(Effect::StopAllStreams);
                        // Clone container id first to avoid borrow conflict
                        let container_id = state.selected_container().map(|c| c.id.clone());
                        if let Some(id) = container_id {
                            state.current_container_id = Some(id.clone());
                            effects.push(Effect::StartLogsStream {
                                container_id: id,
                                initial_lines: 100,
                                generation: state.stream_generation,
                            });
                        }
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if state.view_mode == ViewMode::Stats {
                        // Already viewing stats: just focus the stats panel.
                        state.navigation_mode = NavigationMode::Stats;
                        state.needs_redraw = true;
                    } else {
                        // v3.3.0: Start transition with loading screen
                        state.transition_state = TransitionState::Loading("Loading stats...".to_string());
                        state.force_full_redraw = true;
                        state.needs_redraw = true;
                        state.view_mode = ViewMode::Stats;
                        state.navigation_mode = NavigationMode::Stats;
                        state.logs.clear();
                        effects.push(Effect::StopAllStreams);
                        if let Some(container) = state.selected_container() {
                            effects.push(Effect::StartStatsStream {
                                container_id: container.id.clone(),
                            });
                        }
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    // v3.3.0: Force full redraw for clean transition
                    // No loading screen needed - just layout change
                    state.force_full_redraw = true;
                    state.needs_redraw = true;
                    state.view_mode = match state.view_mode {
                        ViewMode::LogsExpanded => ViewMode::Logs,
                        _ => ViewMode::LogsExpanded,
                    };
                }

                // Menus
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    state.menu_mode = MenuMode::Clipboard;
                    state.needs_redraw = true;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    state.menu_mode = MenuMode::DockerOps;
                    state.needs_redraw = true;
                }

                // Quick restart
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(container) = state.selected_container() {
                        effects.push(Effect::DockerOperation(DockerOp::Restart(container.id.clone())));
                    }
                }

                // Scroll controls (use logs_panel_height for accurate scroll calculations)
                KeyCode::PageUp => {
                    let amount = state.logs_panel_height.saturating_sub(2).max(1);
                    (state, effects) = handle_scroll_up(state, amount);
                    state.needs_redraw = true;
                }
                KeyCode::PageDown => {
                    let amount = state.logs_panel_height.saturating_sub(2).max(1);
                    state.logs.scroll_position = (state.logs.scroll_position + amount)
                        .min(state.logs.max_scroll(state.logs_panel_height));
                    state.needs_redraw = true;
                }
                KeyCode::Home => {
                    state.logs.scroll_position = 0;
                    state.needs_redraw = true;
                    // Trigger load more if at top
                    if state.logs.has_more_history && !state.logs.is_loading_more {
                        state.logs.is_loading_more = true;
                        if let Some(container) = state.selected_container() {
                            effects.push(Effect::LoadHistoricalLogs {
                                container_id: container.id.clone(),
                                before_timestamp: state.logs.oldest_timestamp,
                                batch_size: HISTORICAL_BATCH_SIZE,
                            });
                        }
                    }
                }
                KeyCode::End => {
                    state.logs.scroll_to_bottom(state.logs_panel_height);
                    state.needs_redraw = true;
                }

                KeyCode::Esc => {
                    state.menu_mode = MenuMode::None;
                    state.numeric_input.cancel();
                    state.needs_redraw = true;
                }

                _ => {}
            }
        }

        Message::Tick => {
            // Clear expired notifications
            state.clear_expired_notification();
            // Adaptive cadence: once the user has been idle for a few seconds, slow
            // the tick and the container refresh to cut CPU usage and wakeups.
            // The render loop still polls keyboard events instantly, so input latency
            // is unaffected.
            let idle = state.last_activity.elapsed().as_secs() >= 5;
            let refresh_interval = if idle { 5 } else { 2 };
            if state.last_refresh.elapsed().as_secs() >= refresh_interval {
                effects.push(Effect::LoadContainers);
            }
            let tick_ms = if idle { 1000 } else { 250 };
            effects.push(Effect::ScheduleTick(std::time::Duration::from_millis(tick_ms)));
        }

        // === Data Events ===
        Message::ContainersLoaded(containers) => {
            // Maintain selection if container still exists
            if state.selected_container >= containers.len() && !containers.is_empty() {
                state.selected_container = containers.len() - 1;
            }
            state.containers = containers;
            state.last_refresh = std::time::Instant::now();
            // v3.3.0: Mark for redraw
            state.needs_redraw = true;
        }

        Message::LogReceived { container_id, generation, content } => {
            // v3.2.2: Validate generation to reject stale messages from old streams
            if generation != state.stream_generation {
                // Stale message from old stream - discard silently
                return (state, effects);
            }

            // v3.2.2: Validate container_id matches current
            if state.current_container_id.as_ref() != Some(&container_id) {
                // Message for wrong container - discard silently
                return (state, effects);
            }

            let entry = LogEntry::from_raw(&content);
            let was_at_bottom = state.logs.is_at_bottom(state.logs_panel_height);
            state.logs.push(entry);

            if was_at_bottom {
                state.logs.scroll_to_bottom(state.logs_panel_height);
            }

            // v3.3.0: Complete transition if we were loading
            if matches!(state.transition_state, TransitionState::Loading(_)) {
                state.transition_state = TransitionState::Ready;
                state.force_full_redraw = true;
            }
            state.needs_redraw = true;
        }

        Message::HistoricalLogsLoaded { logs, has_more } => {
            state.logs.prepend(logs);
            state.logs.has_more_history = has_more;
            state.logs.is_loading_more = false;
            if !has_more {
                state.notification = Some(Notification::success("Reached beginning of logs"));
            }
            // v3.3.0: Mark for redraw
            state.needs_redraw = true;
        }

        Message::StatsReceived(stats) => {
            state.stats = Some(stats);
            // v3.3.0: Complete transition if we were loading
            if matches!(state.transition_state, TransitionState::Loading(_)) {
                state.transition_state = TransitionState::Ready;
                state.force_full_redraw = true;
            }
            state.needs_redraw = true;
        }

        // === Operation Results ===
        Message::OperationSuccess(msg) => {
            state.notification = Some(Notification::success(msg));
            effects.push(Effect::LoadContainers);
            state.needs_redraw = true;
        }

        Message::OperationError(msg) => {
            state.notification = Some(Notification::error(msg));
            state.needs_redraw = true;
        }

        Message::ClipboardSuccess(msg) => {
            state.notification = Some(Notification::success(msg));
            state.menu_mode = MenuMode::None;
            state.needs_redraw = true;
        }

        Message::ClipboardError(msg) => {
            state.notification = Some(Notification::error(msg));
            state.menu_mode = MenuMode::None;
            state.needs_redraw = true;
        }

        // === System Events ===
        Message::Quit => {
            state.should_quit = true;
            effects.push(Effect::Quit);
        }

        Message::Resize(height) => {
            state.viewport_height = height.saturating_sub(6) as usize; // Account for header/footer
            // logs_panel_height = content height - panel borders (2 for top/bottom border)
            state.logs_panel_height = height.saturating_sub(8) as usize;
            // Adjust scroll position if it exceeds new max scroll (use logs_panel_height for accuracy)
            let max_scroll = state.logs.max_scroll(state.logs_panel_height);
            if state.logs.scroll_position > max_scroll {
                state.logs.scroll_position = max_scroll;
            }
            // v3.3.0: Force full redraw on resize
            state.force_full_redraw = true;
            state.needs_redraw = true;
        }
    }

    (state, effects)
}

/// Handle navigation up (containers list or scroll)
fn handle_navigate_up(mut state: AppState) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();

    match state.navigation_mode {
        NavigationMode::Containers => {
            if state.selected_container > 0 {
                // v3.3.0: Start transition with loading screen
                state.transition_state = TransitionState::Loading("Switching container...".to_string());
                state.force_full_redraw = true;
                state.needs_redraw = true;
                // v3.2.2: Increment generation FIRST to invalidate pending messages
                state.stream_generation = state.stream_generation.wrapping_add(1);
                state.selected_container -= 1;
                // v3.2.2: Clear logs IMMEDIATELY
                state.logs.clear();
                state.stats = None;
                effects.push(Effect::StopAllStreams);
                if let Some(container) = state.containers.get(state.selected_container) {
                    state.current_container_id = Some(container.id.clone());
                    match state.view_mode {
                        ViewMode::Logs | ViewMode::LogsExpanded => {
                            effects.push(Effect::StartLogsStream {
                                container_id: container.id.clone(),
                                initial_lines: 100,
                                generation: state.stream_generation,
                            });
                        }
                        ViewMode::Stats => {
                            effects.push(Effect::StartStatsStream {
                                container_id: container.id.clone(),
                            });
                        }
                    }
                }
            }
        }
        NavigationMode::Logs | NavigationMode::Stats => {
            (state, effects) = handle_scroll_up(state, 1);
            state.needs_redraw = true;
        }
    }

    (state, effects)
}

/// Handle navigation down (containers list or scroll)
fn handle_navigate_down(mut state: AppState) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();

    match state.navigation_mode {
        NavigationMode::Containers => {
            if state.selected_container < state.containers.len().saturating_sub(1) {
                // v3.3.0: Start transition with loading screen
                state.transition_state = TransitionState::Loading("Switching container...".to_string());
                state.force_full_redraw = true;
                state.needs_redraw = true;
                // v3.2.2: Increment generation FIRST to invalidate pending messages
                state.stream_generation = state.stream_generation.wrapping_add(1);
                state.selected_container += 1;
                // v3.2.2: Clear logs IMMEDIATELY
                state.logs.clear();
                state.stats = None;
                effects.push(Effect::StopAllStreams);
                if let Some(container) = state.containers.get(state.selected_container) {
                    state.current_container_id = Some(container.id.clone());
                    match state.view_mode {
                        ViewMode::Logs | ViewMode::LogsExpanded => {
                            effects.push(Effect::StartLogsStream {
                                container_id: container.id.clone(),
                                initial_lines: 100,
                                generation: state.stream_generation,
                            });
                        }
                        ViewMode::Stats => {
                            effects.push(Effect::StartStatsStream {
                                container_id: container.id.clone(),
                            });
                        }
                    }
                }
            }
        }
        NavigationMode::Logs | NavigationMode::Stats => {
            state.logs.scroll_position = (state.logs.scroll_position + 1)
                .min(state.logs.max_scroll(state.logs_panel_height));
            state.needs_redraw = true;
        }
    }

    (state, effects)
}

/// Handle scroll up with infinite scroll trigger
fn handle_scroll_up(mut state: AppState, amount: usize) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();
    let old_scroll = state.logs.scroll_position;
    state.logs.scroll_position = state.logs.scroll_position.saturating_sub(amount);

    // Trigger load more logs when reaching top
    if old_scroll > 0
        && state.logs.scroll_position == 0
        && !state.logs.is_loading_more
        && state.logs.has_more_history
    {
        state.logs.is_loading_more = true;
        if let Some(container) = state.selected_container() {
            effects.push(Effect::LoadHistoricalLogs {
                container_id: container.id.clone(),
                before_timestamp: state.logs.oldest_timestamp,
                batch_size: HISTORICAL_BATCH_SIZE,
            });
        }
    }

    // v3.3.0: Mark for redraw
    state.needs_redraw = true;
    (state, effects)
}

/// Handle Docker operations menu
fn handle_docker_ops_menu(mut state: AppState, key: KeyCode) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();

    match key {
        KeyCode::Char('1') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Start(container.id.clone())));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('2') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Stop(container.id.clone())));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('3') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Restart(container.id.clone())));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('4') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Pause(container.id.clone())));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('5') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Unpause(container.id.clone())));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('6') => {
            if let Some(container) = state.selected_container() {
                effects.push(Effect::DockerOperation(DockerOp::Remove {
                    id: container.id.clone(),
                    force: false,
                }));
            }
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.menu_mode = MenuMode::None;
        }
        _ => {}
    }

    // v3.3.0: Mark for redraw when menu closes
    state.needs_redraw = true;
    (state, effects)
}

/// Handle clipboard menu
fn handle_clipboard_menu(mut state: AppState, key: KeyCode) -> (AppState, Vec<Effect>) {
    let mut effects = Vec::new();

    match key {
        KeyCode::Char('1') => {
            // Last 100 lines
            let content = collect_logs(&state.logs, 100);
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('2') => {
            // Last 500 lines
            let content = collect_logs(&state.logs, 500);
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('3') => {
            // Visible lines
            let content: String = state.logs
                .visible_entries(state.logs_panel_height)
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('4') => {
            // From current position to the end. `scroll_position` lives in
            // filtered-index space (same as navigation / `visible_entries`),
            // so we must skip over the *filtered* view, not the raw buffer —
            // otherwise an active level filter copies the wrong lines.
            let content: String = state.logs
                .filtered_entries()
                .skip(state.logs.scroll_position)
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('5') => {
            // All logs
            let content: String = state.logs.entries
                .iter()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('6') => {
            // Export all logs to a timestamped file
            let content: String = state.logs.entries
                .iter()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let container_name = state.selected_container()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "container".to_string());
            effects.push(Effect::ExportLogs { content, container_name });
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('7') => {
            // Print all loaded (filtered) logs to the terminal scrollback for
            // manual selection. Works over SSH on any terminal (incl. GNOME
            // Terminal, which can't do OSC 52).
            let content: String = state.logs
                .filtered_entries()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::PrintForManualCopy(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.menu_mode = MenuMode::None;
        }
        _ => {}
    }

    // v3.3.0: Mark for redraw when menu closes
    state.needs_redraw = true;
    (state, effects)
}

/// Collect last N log lines
fn collect_logs(logs: &crate::app::state::LogsState, n: usize) -> String {
    let len = logs.entries.len();
    let start = len.saturating_sub(n);
    logs.entries
        .iter()
        .skip(start)
        .map(|e| e.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
