pub mod app;
pub mod components;
pub mod input;
pub mod layout;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use crate::docker::DockerManager;
use app::App;

pub async fn run_ui(docker_manager: DockerManager) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout, 
        EnterAlternateScreen, 
        EnableMouseCapture,
        SetTitle("Docker Manager v3.0 - Rust")
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(docker_manager).await?;
    
    // Run the app
    let res = run_app(&mut terminal, &mut app).await;

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

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut last_refresh = Instant::now();
    let refresh_rate = Duration::from_millis(250);

    loop {
        // Draw UI
        terminal.draw(|f| app.draw(f))?;

        // Handle input with timeout for refresh
        if event::poll(refresh_rate)? {
            if let Event::Key(key) = event::read()? {
                // Handle Ctrl+C to quit
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.quit = true;
                }
                
                // Handle other keys
                app.handle_key(key).await?;
            }
        }

        // Refresh data periodically
        if last_refresh.elapsed() >= refresh_rate {
            app.refresh().await?;
            last_refresh = Instant::now();
        }

        // Check if should quit
        if app.quit {
            break;
        }
    }

    Ok(())
}