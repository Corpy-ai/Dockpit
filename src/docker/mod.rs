use anyhow::Result;
use bollard::container::{
    ListContainersOptions, LogsOptions, RemoveContainerOptions, RestartContainerOptions,
    StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::{ContainerInspectResponse, ContainerSummary, PortTypeEnum, SystemDataUsageResponse};
use bollard::Docker;
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: ContainerState,
    pub created: i64,
    pub ports: Vec<Port>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContainerState {
    Running,
    Paused,
    Stopped,
    Dead,
    Restarting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub private: u16,
    pub public: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
}

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker })
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>> {
        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;
        let mut result = Vec::new();

        for container in containers {
            result.push(self.parse_container(container)?);
        }

        // Sort containers alphabetically by name
        result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(result)
    }

    fn parse_container(&self, container: ContainerSummary) -> Result<Container> {
        let name = container
            .names
            .and_then(|names| names.first().cloned())
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();

        let state = match container.state.as_deref() {
            Some("running") => ContainerState::Running,
            Some("paused") => ContainerState::Paused,
            Some("exited") | Some("stopped") => ContainerState::Stopped,
            Some("dead") => ContainerState::Dead,
            Some("restarting") => ContainerState::Restarting,
            _ => ContainerState::Stopped,
        };

        let mut ports = Vec::new();
        if let Some(container_ports) = container.ports {
            for port in container_ports {
                ports.push(Port {
                    private: port.private_port,
                    public: port.public_port,
                    protocol: match port.typ {
                        Some(PortTypeEnum::TCP) => "tcp".to_string(),
                        Some(PortTypeEnum::UDP) => "udp".to_string(),
                        Some(PortTypeEnum::SCTP) => "sctp".to_string(),
                        _ => "tcp".to_string(),
                    },
                });
            }
        }

        Ok(Container {
            id: container.id.unwrap_or_default(),
            name,
            image: container.image.unwrap_or_default(),
            status: container.status.unwrap_or_default(),
            state,
            created: container.created.unwrap_or(0),
            ports,
            labels: container.labels.unwrap_or_default(),
        })
    }

    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await?;
        Ok(())
    }

    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let options = Some(StopContainerOptions { t: 10 });
        self.docker.stop_container(container_id, options).await?;
        Ok(())
    }

    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        let options = Some(RestartContainerOptions { t: 10 });
        self.docker.restart_container(container_id, options).await?;
        Ok(())
    }

    pub async fn pause_container(&self, container_id: &str) -> Result<()> {
        self.docker.pause_container(container_id).await?;
        Ok(())
    }

    pub async fn unpause_container(&self, container_id: &str) -> Result<()> {
        self.docker.unpause_container(container_id).await?;
        Ok(())
    }

    pub async fn remove_container(&self, container_id: &str, force: bool) -> Result<()> {
        let options = Some(RemoveContainerOptions {
            force,
            ..Default::default()
        });
        self.docker.remove_container(container_id, options).await?;
        Ok(())
    }

    pub async fn get_container_logs(
        &self,
        container_id: &str,
        lines: usize,
        tx: mpsc::Sender<String>,
    ) -> Result<()> {
        let options = Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: lines.to_string(),
            follow: true,
            timestamps: true,
            ..Default::default()
        });

        let mut stream = self.docker.logs(container_id, options);

        tokio::spawn(async move {
            while let Some(log_result) = stream.next().await {
                if let Ok(log) = log_result {
                    let log_str = log.to_string();
                    if tx.send(log_str).await.is_err() {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn get_container_stats(
        &self,
        container_id: &str,
        tx: mpsc::Sender<Stats>,
    ) -> Result<()> {
        let options = Some(StatsOptions {
            stream: true,
            ..Default::default()
        });

        let mut stream = self.docker.stats(container_id, options);

        tokio::spawn(async move {
            while let Some(stats_result) = stream.next().await {
                if let Ok(stats) = stats_result {
                    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage
                        - stats.precpu_stats.cpu_usage.total_usage;
                    let system_delta =
                        stats.cpu_stats.system_cpu_usage.unwrap_or(0)
                            - stats.precpu_stats.system_cpu_usage.unwrap_or(0);
                    
                    let cpu_percent = if system_delta > 0 && cpu_delta > 0 {
                        (cpu_delta as f64 / system_delta as f64) * 100.0
                            * stats.cpu_stats.online_cpus.unwrap_or(1) as f64
                    } else {
                        0.0
                    };

                    let memory_usage = stats.memory_stats.usage.unwrap_or(0);
                    let memory_limit = stats.memory_stats.limit.unwrap_or(1);
                    let memory_percent = (memory_usage as f64 / memory_limit as f64) * 100.0;

                    let network_rx = stats
                        .networks
                        .as_ref()
                        .map(|nets| nets.values().map(|n| n.rx_bytes).sum())
                        .unwrap_or(0);

                    let network_tx = stats
                        .networks
                        .as_ref()
                        .map(|nets| nets.values().map(|n| n.tx_bytes).sum())
                        .unwrap_or(0);

                    let block_read = stats
                        .blkio_stats
                        .io_service_bytes_recursive
                        .as_ref()
                        .map(|io| {
                            io.iter()
                                .filter(|i| i.op.to_lowercase() == "read")
                                .map(|i| i.value)
                                .sum()
                        })
                        .unwrap_or(0);

                    let block_write = stats
                        .blkio_stats
                        .io_service_bytes_recursive
                        .as_ref()
                        .map(|io| {
                            io.iter()
                                .filter(|i| i.op.to_lowercase() == "write")
                                .map(|i| i.value)
                                .sum()
                        })
                        .unwrap_or(0);

                    let parsed_stats = Stats {
                        cpu_percent,
                        memory_usage,
                        memory_limit,
                        memory_percent,
                        network_rx,
                        network_tx,
                        block_read,
                        block_write,
                    };

                    if tx.send(parsed_stats).await.is_err() {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn inspect_container(&self, container_id: &str) -> Result<ContainerInspectResponse> {
        let result = self.docker.inspect_container(container_id, None).await?;
        Ok(result)
    }

    pub async fn exec_in_container(&self, container_id: &str, cmd: Vec<&str>) -> Result<String> {
        let exec_config = CreateExecOptions {
            cmd: Some(cmd),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self.docker.create_exec(container_id, exec_config).await?;
        
        if let StartExecResults::Attached { mut output, .. } = 
            self.docker.start_exec(&exec.id, None).await? 
        {
            let mut result = String::new();
            while let Some(msg) = output.next().await {
                result.push_str(&msg?.to_string());
            }
            Ok(result)
        } else {
            Ok(String::new())
        }
    }

    pub async fn get_container_processes(&self, container_id: &str) -> Result<String> {
        let result = self.docker.top_processes::<String>(container_id, None).await?;
        
        let mut output = String::new();
        if let Some(titles) = result.titles {
            output.push_str(&titles.join("\t"));
            output.push('\n');
        }
        
        if let Some(processes) = result.processes {
            for process in processes {
                output.push_str(&process.join("\t"));
                output.push('\n');
            }
        }
        
        Ok(output)
    }

    pub async fn get_disk_usage(&self) -> Result<SystemDataUsageResponse> {
        let result = self.docker.df().await?;
        Ok(result)
    }
}