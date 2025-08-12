use anyhow::Result;
use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::docker::{Container, ContainerState, DockerManager, Stats};
use crate::utils::clipboard::ClipboardManager;

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Logs,
    Stats,
    LogsExpanded,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationMode {
    Containers,
    Logs,
    Stats,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuMode {
    None,
    DockerOps,
    Clipboard,
}

pub struct App {
    pub quit: bool,
    pub docker_manager: DockerManager,
    pub containers: Vec<Container>,
    pub selected_container: usize,
    pub container_list_state: ListState,
    pub logs_buffer: VecDeque<String>,
    pub logs_scroll: usize,
    pub stats_buffer: Option<Stats>,
    pub view_mode: ViewMode,
    pub navigation_mode: NavigationMode,
    pub menu_mode: MenuMode,
    pub numeric_input: String,
    pub show_numeric_input: bool,
    pub clipboard_manager: ClipboardManager,
    pub error_message: Option<(String, std::time::Instant)>,
    pub success_message: Option<(String, std::time::Instant)>,
    logs_receiver: Option<mpsc::Receiver<String>>,
    stats_receiver: Option<mpsc::Receiver<Stats>>,
}

impl App {
    pub async fn new(docker_manager: DockerManager) -> Result<Self> {
        let containers = docker_manager.list_containers().await?;
        let mut container_list_state = ListState::default();
        
        if !containers.is_empty() {
            container_list_state.select(Some(0));
        }

        let mut app = Self {
            quit: false,
            docker_manager,
            containers,
            selected_container: 0,
            container_list_state,
            logs_buffer: VecDeque::with_capacity(5000),
            logs_scroll: 0,
            stats_buffer: None,
            view_mode: ViewMode::Logs,
            navigation_mode: NavigationMode::Containers,
            menu_mode: MenuMode::None,
            numeric_input: String::new(),
            show_numeric_input: false,
            clipboard_manager: ClipboardManager::new(),
            error_message: None,
            success_message: None,
            logs_receiver: None,
            stats_receiver: None,
        };

        // Start initial logs stream if there are containers
        if !app.containers.is_empty() {
            app.start_logs_stream().await?;
        }

        Ok(app)
    }

    pub async fn refresh(&mut self) -> Result<()> {
        // Update containers list
        self.containers = self.docker_manager.list_containers().await?;
        
        // Maintain selection if possible
        if self.selected_container >= self.containers.len() && !self.containers.is_empty() {
            self.selected_container = self.containers.len() - 1;
            self.container_list_state.select(Some(self.selected_container));
        }

        // Process logs if receiver exists
        if let Some(receiver) = &mut self.logs_receiver {
            while let Ok(log) = receiver.try_recv() {
                self.logs_buffer.push_back(log);
                if self.logs_buffer.len() > 5000 {
                    self.logs_buffer.pop_front();
                }
            }
        }

        // Process stats if receiver exists
        if let Some(receiver) = &mut self.stats_receiver {
            if let Ok(stats) = receiver.try_recv() {
                self.stats_buffer = Some(stats);
            }
        }

        // Clear old messages
        if let Some((_, time)) = &self.error_message {
            if time.elapsed().as_secs() > 3 {
                self.error_message = None;
            }
        }
        if let Some((_, time)) = &self.success_message {
            if time.elapsed().as_secs() > 2 {
                self.success_message = None;
            }
        }

        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle numeric input mode
        if self.show_numeric_input {
            match key.code {
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.numeric_input.push(c);
                }
                KeyCode::Enter => {
                    if let Ok(num) = self.numeric_input.parse::<usize>() {
                        self.jump_to_container(num.saturating_sub(1)).await?;
                    }
                    self.numeric_input.clear();
                    self.show_numeric_input = false;
                }
                KeyCode::Esc => {
                    self.numeric_input.clear();
                    self.show_numeric_input = false;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle menu modes
        match self.menu_mode {
            MenuMode::DockerOps => return self.handle_docker_ops_menu(key).await,
            MenuMode::Clipboard => return self.handle_clipboard_menu(key).await,
            _ => {}
        }

        // Handle normal navigation
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let num = c.to_digit(10).unwrap() as usize;
                if num > 0 && num <= 9 {
                    self.jump_to_container(num - 1).await?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => self.navigate_up().await?,
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => self.navigate_down().await?,
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => self.switch_to_containers_mode(),
            KeyCode::Right | KeyCode::Char('l') => self.switch_to_logs_mode().await?,
            KeyCode::Char('L') => {
                self.view_mode = ViewMode::Logs;
                self.switch_to_logs_mode().await?;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.view_mode = ViewMode::Stats;
                self.switch_to_stats_mode().await?;
            }
            KeyCode::Char('f') | KeyCode::Char('F') => self.toggle_expanded_logs(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.menu_mode = MenuMode::Clipboard,
            KeyCode::Char('d') | KeyCode::Char('D') => self.menu_mode = MenuMode::DockerOps,
            KeyCode::Char('r') | KeyCode::Char('R') => self.restart_container().await?,
            KeyCode::PageUp => self.scroll_up(10),
            KeyCode::PageDown => self.scroll_down(10),
            KeyCode::Home => self.scroll_to_top(),
            KeyCode::End => self.scroll_to_bottom(),
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.show_numeric_input = true;
                self.numeric_input.clear();
            }
            _ => {}
        }

        Ok(())
    }

    async fn navigate_up(&mut self) -> Result<()> {
        match self.navigation_mode {
            NavigationMode::Containers => {
                if self.selected_container > 0 {
                    self.selected_container -= 1;
                    self.container_list_state.select(Some(self.selected_container));
                    // Clear buffers before switching
                    self.logs_buffer.clear();
                    self.stats_buffer = None;
                    // Start appropriate stream
                    match self.view_mode {
                        ViewMode::Logs | ViewMode::LogsExpanded => self.start_logs_stream().await?,
                        ViewMode::Stats => self.start_stats_stream().await?,
                    }
                }
            }
            NavigationMode::Logs | NavigationMode::Stats => {
                self.scroll_up(1);
            }
        }
        Ok(())
    }

    async fn navigate_down(&mut self) -> Result<()> {
        match self.navigation_mode {
            NavigationMode::Containers => {
                if self.selected_container < self.containers.len().saturating_sub(1) {
                    self.selected_container += 1;
                    self.container_list_state.select(Some(self.selected_container));
                    // Clear buffers before switching
                    self.logs_buffer.clear();
                    self.stats_buffer = None;
                    // Start appropriate stream
                    match self.view_mode {
                        ViewMode::Logs | ViewMode::LogsExpanded => self.start_logs_stream().await?,
                        ViewMode::Stats => self.start_stats_stream().await?,
                    }
                }
            }
            NavigationMode::Logs | NavigationMode::Stats => {
                self.scroll_down(1);
            }
        }
        Ok(())
    }

    fn scroll_up(&mut self, amount: usize) {
        self.logs_scroll = self.logs_scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.logs_buffer.len().saturating_sub(10);
        self.logs_scroll = (self.logs_scroll + amount).min(max_scroll);
    }

    fn scroll_to_top(&mut self) {
        self.logs_scroll = 0;
    }

    fn scroll_to_bottom(&mut self) {
        self.logs_scroll = self.logs_buffer.len().saturating_sub(10);
    }

    async fn jump_to_container(&mut self, index: usize) -> Result<()> {
        if index < self.containers.len() {
            self.selected_container = index;
            self.container_list_state.select(Some(index));
            // Clear buffers before switching
            self.logs_buffer.clear();
            self.stats_buffer = None;
            // Start appropriate stream based on current view
            match self.view_mode {
                ViewMode::Logs | ViewMode::LogsExpanded => self.start_logs_stream().await?,
                ViewMode::Stats => self.start_stats_stream().await?,
            }
        } else {
            self.error_message = Some((
                format!("Container #{} does not exist", index + 1),
                std::time::Instant::now(),
            ));
        }
        Ok(())
    }

    fn switch_to_containers_mode(&mut self) {
        self.navigation_mode = NavigationMode::Containers;
    }

    async fn switch_to_logs_mode(&mut self) -> Result<()> {
        self.navigation_mode = NavigationMode::Logs;
        self.view_mode = ViewMode::Logs;
        self.start_logs_stream().await?;
        Ok(())
    }

    async fn switch_to_stats_mode(&mut self) -> Result<()> {
        self.navigation_mode = NavigationMode::Stats;
        self.view_mode = ViewMode::Stats;
        self.start_stats_stream().await?;
        Ok(())
    }

    fn toggle_expanded_logs(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::LogsExpanded => ViewMode::Logs,
            _ => ViewMode::LogsExpanded,
        };
    }

    async fn start_logs_stream(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            // Stop previous receiver by dropping it
            self.logs_receiver = None;
            
            // Clear previous logs
            self.logs_buffer.clear();
            self.logs_scroll = 0;
            
            // Only start if container is running
            if container.state == ContainerState::Running {
                // Create new channel for logs
                let (tx, rx) = mpsc::channel(100);
                self.logs_receiver = Some(rx);
                
                // Start streaming logs
                self.docker_manager
                    .get_container_logs(&container.id, 100, tx)
                    .await?;
            } else {
                self.logs_buffer.push_back(format!("Container '{}' is not running", container.name));
            }
        }
        Ok(())
    }

    async fn start_stats_stream(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            // Stop previous receiver by dropping it
            self.stats_receiver = None;
            self.stats_buffer = None;
            
            // Only start if container is running
            if container.state == ContainerState::Running {
                // Create new channel for stats
                let (tx, rx) = mpsc::channel(10);
                self.stats_receiver = Some(rx);
                
                // Start streaming stats
                self.docker_manager
                    .get_container_stats(&container.id, tx)
                    .await?;
            }
        }
        Ok(())
    }

    async fn restart_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.restart_container(&container.id).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' restarted successfully", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list to show new status
                    self.refresh().await?;
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to restart container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        Ok(())
    }

    async fn handle_docker_ops_menu(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('1') => self.start_container().await?,
            KeyCode::Char('2') => self.stop_container().await?,
            KeyCode::Char('3') => self.restart_container().await?,
            KeyCode::Char('4') => self.pause_container().await?,
            KeyCode::Char('5') => self.unpause_container().await?,
            KeyCode::Char('6') => self.remove_container().await?,
            KeyCode::Esc | KeyCode::Char('q') => self.menu_mode = MenuMode::None,
            _ => {}
        }
        Ok(())
    }

    async fn handle_clipboard_menu(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('1') => self.copy_logs_to_clipboard(100),
            KeyCode::Char('2') => self.copy_logs_to_clipboard(500),
            KeyCode::Char('3') => self.copy_all_logs_to_clipboard(),
            KeyCode::Esc | KeyCode::Char('q') => self.menu_mode = MenuMode::None,
            _ => {}
        }
        Ok(())
    }

    async fn start_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.start_container(&container.id).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' started", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list and start logs
                    self.refresh().await?;
                    self.start_logs_stream().await?;
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to start container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        self.menu_mode = MenuMode::None;
        Ok(())
    }

    async fn stop_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.stop_container(&container.id).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' stopped", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list
                    self.refresh().await?;
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to stop container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        self.menu_mode = MenuMode::None;
        Ok(())
    }

    async fn pause_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.pause_container(&container.id).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' paused", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list
                    self.refresh().await?;
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to pause container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        self.menu_mode = MenuMode::None;
        Ok(())
    }

    async fn unpause_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.unpause_container(&container.id).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' unpaused", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list and resume logs
                    self.refresh().await?;
                    self.start_logs_stream().await?;
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to unpause container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        self.menu_mode = MenuMode::None;
        Ok(())
    }

    async fn remove_container(&mut self) -> Result<()> {
        if let Some(container) = self.containers.get(self.selected_container) {
            match self.docker_manager.remove_container(&container.id, true).await {
                Ok(_) => {
                    self.success_message = Some((
                        format!("Container '{}' removed", container.name),
                        std::time::Instant::now(),
                    ));
                    // Refresh container list and select previous or first
                    self.refresh().await?;
                    if self.selected_container >= self.containers.len() && !self.containers.is_empty() {
                        self.selected_container = self.containers.len() - 1;
                        self.container_list_state.select(Some(self.selected_container));
                        self.start_logs_stream().await?;
                    }
                }
                Err(e) => {
                    self.error_message = Some((
                        format!("Failed to remove container: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        self.menu_mode = MenuMode::None;
        Ok(())
    }

    fn copy_logs_to_clipboard(&mut self, lines: usize) {
        let logs: Vec<String> = self.logs_buffer
            .iter()
            .rev()
            .take(lines)
            .rev()
            .cloned()
            .collect();
        
        let content = logs.join("\n");
        
        match self.clipboard_manager.copy_to_clipboard(&content) {
            Ok(_) => {
                self.success_message = Some((
                    format!("Copied {} lines to clipboard", logs.len()),
                    std::time::Instant::now(),
                ));
            }
            Err(e) => {
                self.error_message = Some((
                    format!("Failed to copy to clipboard: {}", e),
                    std::time::Instant::now(),
                ));
            }
        }
        
        self.menu_mode = MenuMode::None;
    }

    fn copy_all_logs_to_clipboard(&mut self) {
        let content: String = self.logs_buffer.iter().cloned().collect::<Vec<_>>().join("\n");
        
        match self.clipboard_manager.copy_to_clipboard(&content) {
            Ok(_) => {
                self.success_message = Some((
                    format!("Copied all {} lines to clipboard", self.logs_buffer.len()),
                    std::time::Instant::now(),
                ));
            }
            Err(e) => {
                self.error_message = Some((
                    format!("Failed to copy to clipboard: {}", e),
                    std::time::Instant::now(),
                ));
            }
        }
        
        self.menu_mode = MenuMode::None;
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let size = f.area();
        
        // Main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Content
                Constraint::Length(3),  // Footer
            ])
            .split(size);

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw content based on view mode
        match self.view_mode {
            ViewMode::LogsExpanded => self.draw_expanded_logs(f, chunks[1]),
            _ => {
                // Split content area into two columns
                let content_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(35),  // Container list
                        Constraint::Min(0),      // Logs/Stats view
                    ])
                    .split(chunks[1]);

                self.draw_container_list(f, content_chunks[0]);
                
                match self.view_mode {
                    ViewMode::Logs => self.draw_logs_panel(f, content_chunks[1]),
                    ViewMode::Stats => self.draw_stats_panel(f, content_chunks[1]),
                    _ => {}
                }
            }
        }

        // Draw footer
        self.draw_footer(f, chunks[2]);

        // Draw overlay menus if active
        self.draw_overlay_menus(f, size);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let header_text = vec![
            Line::from(vec![
                Span::styled("Docker Manager v3.0", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(
                    format!("{} containers", self.containers.len()),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" | "),
                Span::styled(
                    Local::now().format("%H:%M").to_string(),
                    Style::default().fg(Color::Gray),
                ),
            ])
        ];

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn draw_container_list(&mut self, f: &mut Frame, area: Rect) {
        let border_color = if self.navigation_mode == NavigationMode::Containers {
            Color::Cyan
        } else {
            Color::Yellow
        };

        let items: Vec<ListItem> = self.containers
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

        f.render_stateful_widget(containers_list, area, &mut self.container_list_state);
    }

    fn draw_logs_panel(&self, f: &mut Frame, area: Rect) {
        let border_color = if self.navigation_mode == NavigationMode::Logs {
            Color::Cyan
        } else {
            Color::Yellow
        };

        let logs_text: Vec<Line> = self.logs_buffer
            .iter()
            .skip(self.logs_scroll)
            .take(area.height as usize - 2)
            .map(|log| Line::from(log.as_str()))
            .collect();

        let title = if let Some(container) = self.containers.get(self.selected_container) {
            format!(" Logs: {} ", container.name)
        } else {
            " Logs ".to_string()
        };

        let logs = Paragraph::new(logs_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title))
            .wrap(Wrap { trim: false });

        f.render_widget(logs, area);

        // Draw scrollbar if needed
        if self.logs_buffer.len() > area.height as usize - 2 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(self.logs_buffer.len())
                .position(self.logs_scroll);
            
            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn draw_stats_panel(&self, f: &mut Frame, area: Rect) {
        let border_color = if self.navigation_mode == NavigationMode::Stats {
            Color::Cyan
        } else {
            Color::Yellow
        };

        let title = if let Some(container) = self.containers.get(self.selected_container) {
            format!(" Stats: {} ", container.name)
        } else {
            " Stats ".to_string()
        };

        let stats_text = if let Some(stats) = &self.stats_buffer {
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
                Line::from(vec![
                    Span::raw("Network RX: "),
                    Span::styled(
                        format!("{:.2} MB", stats.network_rx as f64 / 1_048_576.0),
                        Style::default().fg(Color::Blue),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Network TX: "),
                    Span::styled(
                        format!("{:.2} MB", stats.network_tx as f64 / 1_048_576.0),
                        Style::default().fg(Color::Blue),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Block Read: "),
                    Span::styled(
                        format!("{:.2} MB", stats.block_read as f64 / 1_048_576.0),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Block Write: "),
                    Span::styled(
                        format!("{:.2} MB", stats.block_write as f64 / 1_048_576.0),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
            ]
        } else {
            vec![Line::from("Loading stats...")]
        };

        let stats = Paragraph::new(stats_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title));

        f.render_widget(stats, area);
    }

    fn draw_expanded_logs(&mut self, f: &mut Frame, area: Rect) {
        let logs_text: Vec<Line> = self.logs_buffer
            .iter()
            .skip(self.logs_scroll)
            .take(area.height as usize - 2)
            .map(|log| Line::from(log.as_str()))
            .collect();

        let title = if let Some(container) = self.containers.get(self.selected_container) {
            format!(" Expanded Logs: {} (Press F to exit) ", container.name)
        } else {
            " Expanded Logs (Press F to exit) ".to_string()
        };

        let logs = Paragraph::new(logs_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(title))
            .wrap(Wrap { trim: false });

        f.render_widget(logs, area);

        // Draw scrollbar
        if self.logs_buffer.len() > area.height as usize - 2 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(self.logs_buffer.len())
                .position(self.logs_scroll);
            
            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let footer_text = if let Some((msg, _)) = &self.error_message {
            vec![Line::from(vec![
                Span::styled("✗ ", Style::default().fg(Color::Red)),
                Span::styled(msg, Style::default().fg(Color::Red)),
            ])]
        } else if let Some((msg, _)) = &self.success_message {
            vec![Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(msg, Style::default().fg(Color::Green)),
            ])]
        } else if self.show_numeric_input {
            vec![Line::from(vec![
                Span::raw("Jump to container: "),
                Span::styled(&self.numeric_input, Style::default().fg(Color::Cyan)),
                Span::raw("_"),
            ])]
        } else {
            vec![Line::from(vec![
                Span::raw("[Q]uit | "),
                Span::raw("[↑↓]Navigate | "),
                Span::raw("[L]ogs | "),
                Span::raw("[S]tats | "),
                Span::raw("[F]ullscreen | "),
                Span::raw("[C]lipboard | "),
                Span::raw("[D]ocker Ops | "),
                Span::raw("[R]estart | "),
                Span::raw("[1-9]Jump"),
            ])]
        };

        let footer = Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(footer, area);
    }

    fn draw_overlay_menus(&self, f: &mut Frame, area: Rect) {
        match self.menu_mode {
            MenuMode::DockerOps => self.draw_docker_ops_menu(f, area),
            MenuMode::Clipboard => self.draw_clipboard_menu(f, area),
            _ => {}
        }
    }

    fn draw_docker_ops_menu(&self, f: &mut Frame, area: Rect) {
        let menu_items = vec![
            Line::from("1. Start Container"),
            Line::from("2. Stop Container"),
            Line::from("3. Restart Container"),
            Line::from("4. Pause Container"),
            Line::from("5. Unpause Container"),
            Line::from("6. Remove Container"),
            Line::from(""),
            Line::from("ESC to cancel"),
        ];

        let menu = Paragraph::new(menu_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Docker Operations "))
            .alignment(Alignment::Left);

        let menu_area = centered_rect(40, 12, area);
        f.render_widget(menu, menu_area);
    }

    fn draw_clipboard_menu(&self, f: &mut Frame, area: Rect) {
        let menu_items = vec![
            Line::from("1. Copy last 100 lines"),
            Line::from("2. Copy last 500 lines"),
            Line::from("3. Copy all logs"),
            Line::from(""),
            Line::from("ESC to cancel"),
        ];

        let menu = Paragraph::new(menu_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Clipboard Options "))
            .alignment(Alignment::Left);

        let menu_area = centered_rect(30, 8, area);
        f.render_widget(menu, menu_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}