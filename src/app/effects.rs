use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use futures_util::stream::StreamExt;
use bollard::container::LogsOptions;

use crate::app::{Message, Effect, DockerOp, LogEntry};
use crate::docker::DockerManager;
use crate::utils::clipboard::ClipboardManager;

/// Executes side effects and sends resulting messages back to the main loop
pub struct EffectRunner {
    docker: DockerManager,
    tx: mpsc::Sender<Message>,

    // Active stream handles for cleanup
    logs_handle: Option<JoinHandle<()>>,
    stats_handle: Option<JoinHandle<()>>,
}

impl EffectRunner {
    pub fn new(docker: DockerManager, tx: mpsc::Sender<Message>) -> Self {
        Self {
            docker,
            tx,
            logs_handle: None,
            stats_handle: None,
        }
    }

    /// Execute an effect asynchronously
    pub fn run(&mut self, effect: Effect) {
        match effect {
            Effect::LoadContainers => {
                let docker = self.docker.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    match docker.list_containers().await {
                        Ok(containers) => {
                            let _ = tx.send(Message::ContainersLoaded(containers)).await;
                        }
                        Err(e) => {
                            let _ = tx.send(Message::OperationError(e.to_string())).await;
                        }
                    }
                });
            }

            Effect::StartLogsStream { container_id, initial_lines, generation } => {
                // Stop existing stream first
                self.stop_logs_stream();

                let docker = self.docker.clone();
                let tx = self.tx.clone();
                let container_id_for_msg = container_id.clone();

                let handle = tokio::spawn(async move {
                    let options = Some(LogsOptions::<String> {
                        stdout: true,
                        stderr: true,
                        tail: initial_lines.to_string(),
                        follow: true,
                        timestamps: true,
                        ..Default::default()
                    });

                    let mut stream = docker.docker().logs(&container_id, options);

                    while let Some(log_result) = stream.next().await {
                        match log_result {
                            Ok(log) => {
                                let log_str = log.to_string();
                                // v3.2.2: Include container_id and generation for validation
                                let msg = Message::LogReceived {
                                    container_id: container_id_for_msg.clone(),
                                    generation,
                                    content: log_str,
                                };
                                if tx.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                self.logs_handle = Some(handle);
            }

            Effect::LoadHistoricalLogs { container_id, before_timestamp, batch_size } => {
                let docker = self.docker.clone();
                let tx = self.tx.clone();

                tokio::spawn(async move {
                    // Create channel for receiving historical logs
                    let (logs_tx, mut logs_rx) = mpsc::channel::<String>(batch_size + 100);

                    // Convert chrono timestamp to Unix timestamp for Docker API
                    let unix_timestamp = before_timestamp.map(|ts| ts.timestamp());

                    // Use the efficient timestamp-based method
                    match docker.get_historical_logs_by_timestamp(
                        &container_id,
                        batch_size,
                        unix_timestamp,
                        logs_tx,
                    ).await {
                        Ok(handle) => {
                            // Collect logs from the channel with proper blocking receive
                            let mut logs = Vec::with_capacity(batch_size);

                            // Use timeout to avoid blocking forever if Docker API is slow
                            let timeout = tokio::time::Duration::from_secs(10);
                            let start = std::time::Instant::now();

                            // Receive logs with blocking recv (not try_recv)
                            // This ensures we don't miss logs due to race conditions
                            loop {
                                // Check timeout
                                if start.elapsed() > timeout {
                                    break;
                                }

                                // Check if we have enough logs
                                if logs.len() >= batch_size {
                                    break;
                                }

                                // Try to receive with short timeout to allow checking conditions
                                match tokio::time::timeout(
                                    tokio::time::Duration::from_millis(100),
                                    logs_rx.recv()
                                ).await {
                                    Ok(Some(log_str)) => {
                                        logs.push(LogEntry::from_raw(&log_str));
                                    }
                                    Ok(None) => {
                                        // Channel closed - sender finished
                                        break;
                                    }
                                    Err(_) => {
                                        // Timeout - check if handle is still running
                                        if handle.is_finished() {
                                            // Task finished, drain remaining logs
                                            while let Ok(log_str) = logs_rx.try_recv() {
                                                logs.push(LogEntry::from_raw(&log_str));
                                                if logs.len() >= batch_size {
                                                    break;
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }

                            // Determine if there are more logs available
                            // Use > not >= because we want to indicate "there might be more"
                            let has_more = logs.len() >= batch_size;

                            // Gracefully stop the handle (it may have already finished)
                            handle.abort();

                            // Send the collected logs
                            let _ = tx.send(Message::HistoricalLogsLoaded { logs, has_more }).await;
                        }
                        Err(e) => {
                            let _ = tx.send(Message::OperationError(
                                format!("Failed to load historical logs: {}", e)
                            )).await;
                        }
                    }
                });
            }

            Effect::StartStatsStream { container_id } => {
                // Stop existing stream first
                self.stop_stats_stream();

                let docker = self.docker.clone();
                let tx = self.tx.clone();

                let handle = tokio::spawn(async move {
                    let (stats_tx, mut stats_rx) = mpsc::channel(10);

                    // Start the stats stream
                    if let Ok(stats_handle) = docker.get_container_stats(&container_id, stats_tx).await {
                        // Forward stats to main loop
                        while let Some(stats) = stats_rx.recv().await {
                            if tx.send(Message::StatsReceived(stats)).await.is_err() {
                                break;
                            }
                        }
                        stats_handle.abort();
                    }
                });

                self.stats_handle = Some(handle);
            }

            Effect::StopAllStreams => {
                self.stop_logs_stream();
                self.stop_stats_stream();
            }

            Effect::DockerOperation(op) => {
                let docker = self.docker.clone();
                let tx = self.tx.clone();

                tokio::spawn(async move {
                    let result = match op {
                        DockerOp::Start(id) => {
                            docker.start_container(&id).await
                                .map(|_| format!("Container started"))
                        }
                        DockerOp::Stop(id) => {
                            docker.stop_container(&id).await
                                .map(|_| format!("Container stopped"))
                        }
                        DockerOp::Restart(id) => {
                            docker.restart_container(&id).await
                                .map(|_| format!("Container restarted"))
                        }
                        DockerOp::Pause(id) => {
                            docker.pause_container(&id).await
                                .map(|_| format!("Container paused"))
                        }
                        DockerOp::Unpause(id) => {
                            docker.unpause_container(&id).await
                                .map(|_| format!("Container unpaused"))
                        }
                        DockerOp::Remove { id, force } => {
                            docker.remove_container(&id, force).await
                                .map(|_| format!("Container removed"))
                        }
                    };

                    match result {
                        Ok(msg) => {
                            let _ = tx.send(Message::OperationSuccess(msg)).await;
                        }
                        Err(e) => {
                            let _ = tx.send(Message::OperationError(e.to_string())).await;
                        }
                    }
                });
            }

            Effect::CopyToClipboard(content) => {
                let tx = self.tx.clone();
                let lines = content.lines().count();

                // Clipboard operations must be done synchronously due to arboard limitations
                let mut clipboard = ClipboardManager::new();
                match clipboard.copy_to_clipboard(&content) {
                    Ok(_) => {
                        let _ = tx.try_send(Message::ClipboardSuccess(
                            format!("Copied {} lines to clipboard", lines)
                        ));
                    }
                    Err(e) => {
                        let _ = tx.try_send(Message::ClipboardError(e.to_string()));
                    }
                }
            }

            Effect::ScheduleTick(duration) => {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    let _ = tx.send(Message::Tick).await;
                });
            }

            Effect::ForceRedraw => {
                // This effect is handled by the render loop, not here
            }

            Effect::Quit => {
                self.stop_logs_stream();
                self.stop_stats_stream();
            }
        }
    }

    fn stop_logs_stream(&mut self) {
        if let Some(handle) = self.logs_handle.take() {
            handle.abort();
        }
    }

    fn stop_stats_stream(&mut self) {
        if let Some(handle) = self.stats_handle.take() {
            handle.abort();
        }
    }
}
