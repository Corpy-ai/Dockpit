use crossterm::event::KeyCode;
use crate::app::message::{Message, Effect, DockerOp, LogEntry};
use crate::app::state::{AppState, ViewMode, NavigationMode, MenuMode, Notification, TransitionState};

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
            // Handle numeric input mode first
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
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    state.numeric_input.start();
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
                KeyCode::Right | KeyCode::Char('l') => {
                    if state.view_mode == ViewMode::Logs || state.view_mode == ViewMode::LogsExpanded {
                        state.navigation_mode = NavigationMode::Logs;
                    } else {
                        state.navigation_mode = NavigationMode::Stats;
                    }
                }

                // View mode switches
                KeyCode::Char('L') => {
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
                KeyCode::Char('s') | KeyCode::Char('S') => {
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

                // Scroll controls
                KeyCode::PageUp => {
                    let amount = state.viewport_height.saturating_sub(2).max(1);
                    (state, effects) = handle_scroll_up(state, amount);
                    state.needs_redraw = true;
                }
                KeyCode::PageDown => {
                    let amount = state.viewport_height.saturating_sub(2).max(1);
                    state.logs.scroll_position = (state.logs.scroll_position + amount)
                        .min(state.logs.max_scroll(state.viewport_height));
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
                    state.logs.scroll_to_bottom(state.viewport_height);
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
            // Refresh containers list every 2 seconds (less aggressive = less visual noise)
            if state.last_refresh.elapsed().as_secs() >= 2 {
                effects.push(Effect::LoadContainers);
            }
            // Schedule next tick
            effects.push(Effect::ScheduleTick(std::time::Duration::from_millis(250)));
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
            let was_at_bottom = state.logs.is_at_bottom(state.viewport_height);
            state.logs.push(entry);

            if was_at_bottom {
                state.logs.scroll_to_bottom(state.viewport_height);
            }

            // v3.3.0: Complete transition if we were loading
            if matches!(state.transition_state, TransitionState::Loading(_)) {
                state.transition_state = TransitionState::Ready;
                state.force_full_redraw = true;
            }
            state.needs_redraw = true;
        }

        Message::LogsBatchReceived(logs) => {
            let was_at_bottom = state.logs.is_at_bottom(state.viewport_height);
            for raw in logs {
                let entry = LogEntry::from_raw(&raw);
                state.logs.push(entry);
            }
            if was_at_bottom {
                state.logs.scroll_to_bottom(state.viewport_height);
            }
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

        // === Navigation Events ===
        Message::SelectContainer(index) => {
            if index < state.containers.len() {
                // v3.3.0: Start transition with loading screen
                state.transition_state = TransitionState::Loading("Switching container...".to_string());
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

        Message::ScrollUp(amount) => {
            (state, effects) = handle_scroll_up(state, amount);
            state.needs_redraw = true;
        }

        Message::ScrollDown(amount) => {
            state.logs.scroll_position = (state.logs.scroll_position + amount)
                .min(state.logs.max_scroll(state.viewport_height));
            state.needs_redraw = true;
        }

        Message::ScrollToTop => {
            state.logs.scroll_position = 0;
            state.needs_redraw = true;
        }

        Message::ScrollToBottom => {
            state.logs.scroll_to_bottom(state.viewport_height);
            state.needs_redraw = true;
        }

        Message::LoadMoreLogs => {
            if !state.logs.is_loading_more && state.logs.has_more_history {
                state.logs.is_loading_more = true;
                state.needs_redraw = true;  // Show loading indicator
                if let Some(container) = state.selected_container() {
                    effects.push(Effect::LoadHistoricalLogs {
                        container_id: container.id.clone(),
                        before_timestamp: state.logs.oldest_timestamp,
                        batch_size: HISTORICAL_BATCH_SIZE,
                    });
                }
            }
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

        // === Menu Events ===
        Message::CloseMenu => {
            state.menu_mode = MenuMode::None;
            state.needs_redraw = true;
        }

        // === System Events ===
        Message::Quit => {
            state.should_quit = true;
            effects.push(Effect::Quit);
        }

        Message::Resize(_, height) => {
            state.viewport_height = height.saturating_sub(6) as usize; // Account for header/footer
            // Adjust scroll position if it exceeds new max scroll
            let max_scroll = state.logs.max_scroll(state.viewport_height);
            if state.logs.scroll_position > max_scroll {
                state.logs.scroll_position = max_scroll;
            }
            // v3.3.0: Force full redraw on resize
            state.force_full_redraw = true;
            state.needs_redraw = true;
        }

        // Handle remaining messages
        _ => {}
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
                .min(state.logs.max_scroll(state.viewport_height));
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
                .visible_entries(state.viewport_height)
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('4') => {
            // From current position
            let content: String = state.logs.entries
                .iter()
                .skip(state.logs.scroll_position)
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
            state.menu_mode = MenuMode::None;
        }
        KeyCode::Char('5') | KeyCode::Char('6') => {
            // All logs
            let content: String = state.logs.entries
                .iter()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            effects.push(Effect::CopyToClipboard(content));
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
