pub mod app;
pub mod components;
pub mod input;
pub mod layout;
pub mod view;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::{
    error::Error,
    io,
    time::Duration,
};
use tokio::sync::mpsc;

use crate::docker::DockerManager;
use crate::app::{AppState, Message, Effect, EffectRunner, update};

/// Main entry point for the TUI application.
/// Uses message-based architecture with NO terminal.clear() for flicker-free rendering.
pub async fn run_ui(docker_manager: DockerManager) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let title = format!("Docker Manager v{}", env!("CARGO_PKG_VERSION"));
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        SetTitle(&title)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the application
    let res = run_app(&mut terminal, docker_manager).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// Main application loop using Elm Architecture (Message -> Update -> View).
///
/// Key principles:
/// 1. NO terminal.clear() - Ratatui handles diffing automatically
/// 2. All state changes go through Message -> update()
/// 3. Side effects are executed via EffectRunner
/// 4. Rendering is a pure function of state
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    docker_manager: DockerManager,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Message channel for communication
    let (tx, mut rx) = mpsc::channel::<Message>(256);

    // Initialize state
    let mut state = AppState::default();

    // Get initial viewport height
    let size = terminal.size()?;
    state.viewport_height = size.height.saturating_sub(6) as usize;

    // Initialize effect runner
    let mut effect_runner = EffectRunner::new(docker_manager, tx.clone());

    // Load initial containers
    effect_runner.run(Effect::LoadContainers);

    // Schedule first tick
    effect_runner.run(Effect::ScheduleTick(Duration::from_millis(100)));

    // Start logs stream if containers are loaded
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        // Wait a bit for containers to load
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = tx_clone.send(Message::Tick).await;
    });

    // Main event loop - optimized for minimal latency (v3.3.0)
    loop {
        // 1. RENDER - Conditional based on needs_redraw (v3.3.0)
        if state.needs_redraw || state.force_full_redraw {
            // If force_full_redraw, clear physical terminal (CRITICAL for ghost chars)
            if state.force_full_redraw {
                terminal.clear()?;  // Sends ESC[2J + ESC[H to terminal
            }

            // Always render main view - loading is shown as overlay in logs panel only
            terminal.draw(|frame| {
                view::render(frame, &state);
            })?;

            // Reset flags after render
            state.needs_redraw = false;
            state.force_full_redraw = false;
        }

        // 2. COLLECT MESSAGES - prioritize keyboard events for responsiveness
        let message = {
            // First: Check for immediate keyboard/resize events (non-blocking)
            if event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            Some(Message::Quit)
                        } else {
                            Some(Message::KeyPressed(key))
                        }
                    }
                    Event::Resize(w, h) => Some(Message::Resize(w, h)),
                    _ => None,
                }
            } else {
                // No immediate events - check channel with timeout
                tokio::select! {
                    biased;  // Prioritize in order listed

                    Some(msg) = rx.recv() => Some(msg),

                    // Frame limiter: ~30fps max when idle to reduce CPU
                    _ = tokio::time::sleep(Duration::from_millis(33)) => None,
                }
            }
        };

        // 3. UPDATE - Process message if we have one
        if let Some(msg) = message {
            let (new_state, effects) = update(state, msg);
            state = new_state;

            // 4. EXECUTE EFFECTS
            for effect in effects {
                if matches!(effect, Effect::Quit) {
                    return Ok(());
                }
                effect_runner.run(effect);
            }
        }

        // 5. CHECK QUIT
        if state.should_quit {
            break;
        }
    }

    Ok(())
}

// Legacy app module - kept for reference during migration
// TODO: Remove once all functionality is migrated to new architecture
