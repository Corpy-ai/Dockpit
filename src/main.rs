mod app;
mod docker;
mod ui;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use docker::DockerManager;
use env_logger::Builder;
use log::{error, info};
use std::io::Write;

#[derive(Parser)]
#[command(name = "docker-manager")]
#[command(author = "uniCommerce Team")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Fast and efficient Docker container manager with perfect visual interface - Zero glitches guaranteed", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all containers
    List {
        /// Show all containers (default shows only running)
        #[arg(short, long)]
        all: bool,
    },
    /// Start a container
    Start {
        /// Container name or ID
        container: String,
    },
    /// Stop a container
    Stop {
        /// Container name or ID
        container: String,
    },
    /// Restart a container
    Restart {
        /// Container name or ID
        container: String,
    },
    /// Show container logs
    Logs {
        /// Container name or ID
        container: String,
        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        lines: usize,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// Show container statistics
    Stats {
        /// Container name or ID (optional, shows all if not specified)
        container: Option<String>,
    },
    /// Execute command in container
    Exec {
        /// Container name or ID
        container: String,
        /// Command to execute
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    let log_level = if cli.debug { "debug" } else { "info" };
    Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {} - {}",
                record.level(),
                chrono::Local::now().format("%H:%M:%S"),
                record.args()
            )
        })
        .init();

    info!("Docker Manager v{} starting...", env!("CARGO_PKG_VERSION"));

    // Create Docker manager
    let docker_manager = match DockerManager::new().await {
        Ok(dm) => dm,
        Err(e) => {
            error!("Failed to connect to Docker: {}", e);
            eprintln!("Error: Failed to connect to Docker daemon");
            eprintln!("Make sure Docker is running and you have permission to access it");
            eprintln!("You might need to add your user to the docker group:");
            eprintln!("  sudo usermod -aG docker $USER");
            eprintln!("Then log out and back in for the changes to take effect");
            std::process::exit(1);
        }
    };

    // Handle CLI commands or start TUI
    match cli.command {
        Some(Commands::List { all }) => {
            handle_list_command(docker_manager, all).await?;
        }
        Some(Commands::Start { container }) => {
            handle_start_command(docker_manager, &container).await?;
        }
        Some(Commands::Stop { container }) => {
            handle_stop_command(docker_manager, &container).await?;
        }
        Some(Commands::Restart { container }) => {
            handle_restart_command(docker_manager, &container).await?;
        }
        Some(Commands::Logs { container, lines, follow }) => {
            handle_logs_command(docker_manager, &container, lines, follow).await?;
        }
        Some(Commands::Stats { container }) => {
            handle_stats_command(docker_manager, container.as_deref()).await?;
        }
        Some(Commands::Exec { container, command }) => {
            handle_exec_command(docker_manager, &container, command).await?;
        }
        None => {
            // Start interactive TUI mode
            info!("Starting interactive TUI mode");
            if let Err(e) = ui::run_ui(docker_manager).await {
                error!("TUI error: {}", e);
                eprintln!("Error running TUI: {}", e);
            }
        }
    }

    Ok(())
}

async fn handle_list_command(docker: DockerManager, all: bool) -> Result<()> {
    let containers = docker.list_containers(all).await?;
    
    println!("{:<3} {:<30} {:<20} {:<30} {:<10}", 
             "#", "NAME", "IMAGE", "STATUS", "STATE");
    println!("{}", "-".repeat(100));
    
    for (i, container) in containers.iter().enumerate() {
        let state_symbol = match container.state {
            docker::ContainerState::Running => "●",
            docker::ContainerState::Paused => "⏸",
            docker::ContainerState::Stopped => "○",
            docker::ContainerState::Dead => "✗",
            docker::ContainerState::Restarting => "↻",
        };
        
        println!("{:<3} {:<30} {:<20} {:<30} {:<10}",
                 i + 1,
                 container.name,
                 container.image,
                 container.status,
                 state_symbol);
    }
    
    Ok(())
}

async fn handle_start_command(docker: DockerManager, container: &str) -> Result<()> {
    println!("Starting container '{}'...", container);
    docker.start_container(container).await?;
    println!("Container '{}' started successfully", container);
    Ok(())
}

async fn handle_stop_command(docker: DockerManager, container: &str) -> Result<()> {
    println!("Stopping container '{}'...", container);
    docker.stop_container(container).await?;
    println!("Container '{}' stopped successfully", container);
    Ok(())
}

async fn handle_restart_command(docker: DockerManager, container: &str) -> Result<()> {
    println!("Restarting container '{}'...", container);
    docker.restart_container(container).await?;
    println!("Container '{}' restarted successfully", container);
    Ok(())
}

async fn handle_logs_command(docker: DockerManager, container: &str, lines: usize, follow: bool) -> Result<()> {
    use tokio::sync::mpsc;
    
    let (tx, mut rx) = mpsc::channel(100);
    
    docker.get_container_logs(container, lines, tx).await?;
    
    if follow {
        println!("Following logs for '{}' (Press Ctrl+C to stop)...", container);
        while let Some(log) = rx.recv().await {
            print!("{}", log);
        }
    } else {
        println!("Showing last {} lines for '{}':", lines, container);
        let mut count = 0;
        while let Some(log) = rx.recv().await {
            print!("{}", log);
            count += 1;
            if count >= lines {
                break;
            }
        }
    }
    
    Ok(())
}

async fn handle_stats_command(docker: DockerManager, container: Option<&str>) -> Result<()> {
    use tokio::sync::mpsc;
    
    if let Some(container_name) = container {
        let (tx, mut rx) = mpsc::channel(10);
        
        docker.get_container_stats(container_name, tx).await?;
        
        println!("Statistics for '{}':", container_name);
        println!("{:<15} {:<15} {:<20} {:<15} {:<15}", 
                 "CPU %", "MEM USAGE", "MEM %", "NET I/O", "BLOCK I/O");
        
        // CLI mode shows a single snapshot, then exits.
        if let Some(stats) = rx.recv().await {
            println!("{:<15.2} {:<15} {:<20.2} {:<15} {:<15}",
                     stats.cpu_percent,
                     format!("{:.2} MB", stats.memory_usage as f64 / 1_048_576.0),
                     stats.memory_percent,
                     format!("{:.2}/{:.2} MB",
                             stats.network_rx as f64 / 1_048_576.0,
                             stats.network_tx as f64 / 1_048_576.0),
                     format!("{:.2}/{:.2} MB",
                             stats.block_read as f64 / 1_048_576.0,
                             stats.block_write as f64 / 1_048_576.0));
        }
    } else {
        println!("Please specify a container name to show statistics");
    }
    
    Ok(())
}

async fn handle_exec_command(docker: DockerManager, container: &str, command: Vec<String>) -> Result<()> {
    if command.is_empty() {
        println!("No command specified");
        return Ok(());
    }
    
    println!("Executing command in '{}'...", container);
    let cmd: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
    let output = docker.exec_in_container(container, cmd).await?;
    print!("{}", output);
    
    Ok(())
}
