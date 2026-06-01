pub mod view;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
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
use crate::utils::osc52;

/// Main entry point for the TUI application.
/// Uses message-based architecture with NO terminal.clear() for flicker-free rendering.
pub async fn run_ui(docker_manager: DockerManager) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let title = format!("Dockpit v{}", env!("CARGO_PKG_VERSION"));
    // NOTE: we deliberately do NOT enable mouse capture. Mouse events are never
    // consumed by the app, and capturing them only disables the terminal's own
    // click-drag text selection — which is the universal fallback for copying
    // over SSH on terminals that don't support OSC 52 (e.g. older GNOME Terminal).
    execute!(
        stdout,
        EnterAlternateScreen,
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
        LeaveAlternateScreen
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
    // logs_panel_height = content height - panel borders (2 for top/bottom border)
    state.logs_panel_height = size.height.saturating_sub(8) as usize;

    // Decide once how clipboard copy reaches the user's clipboard.
    let clipboard_backend = detect_clipboard_backend();

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

        // 2. COLLECT MESSAGES - prioritize keyboard events for responsiveness.
        // Grab the first message (blocking up to the frame limiter), then drain
        // everything already queued so a burst of logs coalesces into a SINGLE
        // render instead of one redraw per line.
        let mut batch: Vec<Message> = Vec::new();

        if event::poll(Duration::from_millis(0))? {
            // Immediate input event takes priority.
            if let Some(msg) = event_to_message(event::read()?) {
                batch.push(msg);
            }
        } else {
            // No immediate input - wait on the channel with a frame-limiter timeout.
            tokio::select! {
                biased;  // Prioritize in order listed

                Some(msg) = rx.recv() => batch.push(msg),

                // Frame limiter: ~30fps max when idle to reduce CPU
                _ = tokio::time::sleep(Duration::from_millis(33)) => {}
            }
        }

        // Drain whatever else is already pending, without blocking. Capped so we
        // still render periodically under an unbounded log flood and keep input
        // latency low. Input events are checked first each iteration.
        const MAX_DRAIN: usize = 256;
        for _ in 0..MAX_DRAIN {
            if event::poll(Duration::from_millis(0))? {
                if let Some(msg) = event_to_message(event::read()?) {
                    batch.push(msg);
                }
                continue;
            }
            match rx.try_recv() {
                Ok(msg) => batch.push(msg),
                Err(_) => break, // channel empty or closed
            }
        }

        // 3. UPDATE + 4. EXECUTE EFFECTS for each message in arrival order
        for msg in batch {
            let (new_state, effects) = update(state, msg);
            state = new_state;

            for effect in effects {
                match effect {
                    Effect::Quit => return Ok(()),
                    // Clipboard copy is special: over SSH (or a local TTY) the
                    // only thing that reaches the user's clipboard is OSC 52,
                    // which must be written to the terminal from THIS loop (the
                    // EffectRunner runs on a blocking thread with no terminal).
                    Effect::CopyToClipboard(content) if clipboard_backend == ClipboardBackend::Osc52 => {
                        let lines = content.lines().count();
                        match osc52::osc52_sequence(&content) {
                            Ok(seq) => {
                                use std::io::Write;
                                let backend = terminal.backend_mut();
                                let write_result = backend
                                    .write_all(seq.as_bytes())
                                    .and_then(|_| backend.flush());
                                match write_result {
                                    Ok(()) => {
                                        // OSC 52 has no ACK: we can't know if the
                                        // terminal honored it. Be honest and point
                                        // to option 7 for terminals that don't.
                                        let _ = tx.try_send(Message::ClipboardSuccess(
                                            format!("OSC52: sent {lines} lines — paste to verify (GNOME Terminal: use 7)"),
                                        ));
                                    }
                                    Err(e) => {
                                        let _ = tx.try_send(Message::ClipboardError(
                                            format!("Clipboard write failed: {e}"),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.try_send(Message::ClipboardError(e.to_string()));
                            }
                        }
                    }
                    // Dump logs to the terminal scrollback for manual mouse
                    // selection. Suspends the TUI; works on any terminal over SSH.
                    Effect::PrintForManualCopy(content) => {
                        if let Err(e) = print_for_manual_copy(terminal, &content) {
                            let _ = tx.try_send(Message::ClipboardError(
                                format!("Print failed: {e}"),
                            ));
                        }
                        // Repaint the TUI cleanly after returning from plain output.
                        state.force_full_redraw = true;
                        state.needs_redraw = true;
                    }
                    other => effect_runner.run(other),
                }
            }
        }

        // 5. CHECK QUIT
        if state.should_quit {
            break;
        }
    }

    Ok(())
}

/// How a clipboard-copy request is delivered to the user's clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBackend {
    /// Native clipboard on this host (arboard / xclip / wl-copy / pbcopy).
    /// Correct only when the UI runs on the same machine as the user.
    Local,
    /// OSC 52 escape sequence written to the terminal. The *local* terminal
    /// honors it, so this is the only thing that works over SSH.
    Osc52,
}

/// Pick the clipboard backend once at startup.
///
/// - Explicit `DOCKPIT_CLIPBOARD=osc52|local` always wins.
/// - Over SSH only OSC 52 reaches the user's (client) clipboard.
/// - A local graphical session uses the native clipboard (persists via a
///   clipboard manager / CLI tools).
/// - A local TTY / headless session falls back to OSC 52 (native clipboard
///   isn't reachable, but the terminal is).
fn detect_clipboard_backend() -> ClipboardBackend {
    if let Some(v) = std::env::var_os("DOCKPIT_CLIPBOARD") {
        match v.to_string_lossy().to_ascii_lowercase().as_str() {
            "osc52" | "osc" => return ClipboardBackend::Osc52,
            "local" | "native" => return ClipboardBackend::Local,
            _ => {}
        }
    }

    let over_ssh = std::env::var_os("SSH_CONNECTION").is_some()
        || std::env::var_os("SSH_TTY").is_some()
        || std::env::var_os("SSH_CLIENT").is_some();
    if over_ssh {
        return ClipboardBackend::Osc52;
    }

    let graphical = std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if graphical {
        ClipboardBackend::Local
    } else {
        ClipboardBackend::Osc52
    }
}

/// Temporarily leave the TUI and print `content` to the terminal's normal
/// scrollback so the user can select it with the mouse + Ctrl+Shift+C. This is
/// the only way to copy many lines over SSH on terminals without OSC 52 support
/// (e.g. GNOME Terminal). Blocks until the user presses Enter.
fn print_for_manual_copy(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    content: &str,
) -> io::Result<()> {
    use std::io::Write;

    let lines = content.lines().count();

    // Switch to the normal screen so the dump lands in the terminal scrollback.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    {
        let mut out = io::stdout();
        writeln!(
            out,
            "\n===== {lines} líneas — seleccioná con el mouse y copiá (Ctrl+Shift+C). Enter para volver. =====\n"
        )?;
        writeln!(out, "{content}")?;
        writeln!(
            out,
            "\n===== fin ({lines} líneas) — Enter para volver a dockpit ====="
        )?;
        out.flush()?;
    }

    // Block until the user finishes selecting/copying and presses Enter.
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;

    // Restore the TUI.
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

/// Translate a crossterm terminal event into an app `Message`.
/// Returns `None` for events we ignore (e.g. mouse movement).
fn event_to_message(event: Event) -> Option<Message> {
    match event {
        Event::Key(key) => {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                Some(Message::Quit)
            } else {
                Some(Message::KeyPressed(key))
            }
        }
        Event::Resize(_, h) => Some(Message::Resize(h)),
        _ => None,
    }
}
