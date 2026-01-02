use anyhow::Result;
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, KillContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, RenameContainerOptions, RestartContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use futures::stream::StreamExt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DockerClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerState {
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
    Created,
    Removing,
    Unknown,
}

impl std::fmt::Display for ContainerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerState::Running => write!(f, "Running"),
            ContainerState::Paused => write!(f, "Paused"),
            ContainerState::Restarting => write!(f, "Restarting"),
            ContainerState::Exited => write!(f, "Exited"),
            ContainerState::Dead => write!(f, "Dead"),
            ContainerState::Created => write!(f, "Created"),
            ContainerState::Removing => write!(f, "Removing"),
            ContainerState::Unknown => write!(f, "Unknown"),
        }
    }
}

impl ContainerState {
    pub fn is_running(&self) -> bool {
        matches!(self, ContainerState::Running)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, ContainerState::Paused)
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => ContainerState::Running,
            "paused" => ContainerState::Paused,
            "restarting" => ContainerState::Restarting,
            "exited" => ContainerState::Exited,
            "dead" => ContainerState::Dead,
            "created" => ContainerState::Created,
            "removing" => ContainerState::Removing,
            _ => ContainerState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub state: ContainerState,
    pub status: String,
    pub created: Option<DateTime<Utc>>,
    pub ports: Vec<PortMapping>,
    pub labels: HashMap<String, String>,
    pub command: Option<String>,
    pub size_rw: Option<i64>,
    pub size_root_fs: Option<i64>,
}

impl ContainerInfo {
    pub fn short_id(&self) -> &str {
        if self.id.len() >= 12 {
            &self.id[..12]
        } else {
            &self.id
        }
    }

    pub fn display_ports(&self) -> String {
        self.ports
            .iter()
            .filter_map(|p| {
                p.public_port.map(|pub_port| {
                    format!("{}:{}", pub_port, p.private_port)
                })
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// File entry in a container filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub permissions: String,
}

impl ContainerFileEntry {
    pub fn display_size(&self) -> String {
        if self.is_dir {
            "-".to_string()
        } else {
            bytesize::ByteSize(self.size).to_string()
        }
    }
}

impl DockerClient {
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let docker = self.client()?;

        let options = ListContainersOptions {
            all,
            ..Default::default()
        };

        let containers = docker.list_containers(Some(options)).await?;

        let mut result = Vec::new();
        for container in containers {
            let id = container.id.unwrap_or_default();
            let names = container.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| id.clone());

            let ports = container
                .ports
                .unwrap_or_default()
                .into_iter()
                .map(|p| PortMapping {
                    private_port: p.private_port,
                    public_port: p.public_port,
                    protocol: p.typ.map(|t| t.to_string()).unwrap_or_else(|| "tcp".to_string()),
                    ip: p.ip,
                })
                .collect();

            let created = container
                .created
                .map(|ts| DateTime::from_timestamp(ts, 0))
                .flatten();

            result.push(ContainerInfo {
                id,
                name,
                image: container.image.unwrap_or_default(),
                image_id: container.image_id.unwrap_or_default(),
                state: ContainerState::from_str(
                    &container
                        .state
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_default(),
                ),
                status: container.status.unwrap_or_default(),
                created,
                ports,
                labels: container.labels.unwrap_or_default(),
                command: container.command,
                size_rw: container.size_rw,
                size_root_fs: container.size_root_fs,
            });
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    pub async fn start_container(&self, id: &str) -> Result<()> {
        let docker = self.client()?;
        docker
            .start_container(id, None::<StartContainerOptions>)
            .await?;
        Ok(())
    }

    pub async fn stop_container(&self, id: &str) -> Result<()> {
        let docker = self.client()?;
        docker
            .stop_container(
                id,
                Some(StopContainerOptions {
                    t: Some(10),
                    signal: None,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn restart_container(&self, id: &str) -> Result<()> {
        let docker = self.client()?;
        docker
            .restart_container(
                id,
                Some(RestartContainerOptions {
                    t: Some(10),
                    signal: None,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn remove_container(&self, id: &str, force: bool) -> Result<()> {
        let docker = self.client()?;
        docker
            .remove_container(
                id,
                Some(RemoveContainerOptions {
                    force,
                    v: false,
                    link: false,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn pause_container(&self, id: &str) -> Result<()> {
        let docker = self.client()?;
        docker.pause_container(id).await?;
        Ok(())
    }

    pub async fn unpause_container(&self, id: &str) -> Result<()> {
        let docker = self.client()?;
        docker.unpause_container(id).await?;
        Ok(())
    }

    pub async fn kill_container(&self, id: &str, signal: Option<&str>) -> Result<()> {
        let docker = self.client()?;
        docker
            .kill_container(
                id,
                Some(KillContainerOptions {
                    signal: signal.unwrap_or("SIGKILL").to_string(),
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn rename_container(&self, id: &str, new_name: &str) -> Result<()> {
        let docker = self.client()?;
        docker
            .rename_container(
                id,
                bollard::container::RenameContainerOptions { name: new_name },
            )
            .await?;
        Ok(())
    }

    pub async fn commit_container(
        &self,
        id: &str,
        repo: &str,
        tag: &str,
        comment: Option<&str>,
        author: Option<&str>,
    ) -> Result<String> {
        let docker = self.client()?;
        let result = docker
            .commit_container(
                bollard::image::CommitContainerOptions {
                    container: id,
                    repo,
                    tag,
                    comment: comment.unwrap_or(""),
                    author: author.unwrap_or(""),
                    pause: true,
                    changes: None,
                },
                bollard::container::Config::<String>::default(),
            )
            .await?;
        Ok(result.id)
    }

    pub async fn get_container_top(&self, id: &str) -> Result<Vec<Vec<String>>> {
        let docker = self.client()?;
        let result = docker
            .top_processes(id, Some(bollard::container::TopOptions { ps_args: "aux" }))
            .await?;
        Ok(result.processes.unwrap_or_default())
    }

    pub async fn export_container(&self, id: &str, output_path: &str) -> Result<()> {
        use futures::TryStreamExt;
        use tokio::io::AsyncWriteExt;

        let docker = self.client()?;
        let stream = docker.export_container(id);
        let mut file = tokio::fs::File::create(output_path).await?;

        futures::pin_mut!(stream);
        while let Some(chunk) = stream.try_next().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }

    pub async fn copy_from_container(&self, id: &str, path: &str) -> Result<Vec<u8>> {
        use futures::TryStreamExt;

        let docker = self.client()?;
        let stream = docker.download_from_container(
            id,
            Some(bollard::container::DownloadFromContainerOptions { path }),
        );

        let mut data = Vec::new();
        futures::pin_mut!(stream);
        while let Some(chunk) = stream.try_next().await? {
            data.extend_from_slice(&chunk);
        }
        Ok(data)
    }

    pub async fn copy_to_container(&self, id: &str, path: &str, data: Vec<u8>) -> Result<()> {
        use bytes::Bytes;
        let docker = self.client()?;
        docker
            .upload_to_container(
                id,
                Some(bollard::container::UploadToContainerOptions {
                    path,
                    no_overwrite_dir_non_dir: "false",
                }),
                bollard::body_full(Bytes::from(data)),
            )
            .await?;
        Ok(())
    }

    /// Create a new container from an image
    pub async fn create_container(
        &self,
        image: &str,
        name: Option<&str>,
        platform: Option<&str>,
        command: Option<Vec<String>>,
        entrypoint: Option<Vec<String>>,
        working_dir: Option<&str>,
        auto_remove: bool,
        restart_policy: Option<&str>,
        privileged: bool,
        read_only: bool,
        init: bool,
        env_vars: Vec<(String, String)>,
        ports: Vec<(String, String, String)>, // (host_port, container_port, protocol)
        volumes: Vec<(String, String, bool)>, // (host_path, container_path, read_only)
        network: Option<&str>,
    ) -> Result<String> {
        let docker = self.client()?;

        // Build host config
        let mut host_config = HostConfig::default();
        host_config.auto_remove = Some(auto_remove);
        host_config.privileged = Some(privileged);
        host_config.readonly_rootfs = Some(read_only);
        host_config.init = Some(init);

        if let Some(policy) = restart_policy {
            if policy != "no" {
                host_config.restart_policy = Some(bollard::models::RestartPolicy {
                    name: Some(match policy {
                        "always" => bollard::models::RestartPolicyNameEnum::ALWAYS,
                        "on-failure" => bollard::models::RestartPolicyNameEnum::ON_FAILURE,
                        "unless-stopped" => bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED,
                        _ => bollard::models::RestartPolicyNameEnum::NO,
                    }),
                    maximum_retry_count: None,
                });
            }
        }

        // Port bindings
        if !ports.is_empty() {
            let mut port_bindings: HashMap<String, Option<Vec<bollard::models::PortBinding>>> = HashMap::new();
            for (host_port, container_port, protocol) in &ports {
                let key = format!("{}/{}", container_port, protocol);
                let binding = bollard::models::PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(host_port.clone()),
                };
                port_bindings.insert(key, Some(vec![binding]));
            }
            host_config.port_bindings = Some(port_bindings);
        }

        // Volume bindings
        if !volumes.is_empty() {
            let binds: Vec<String> = volumes
                .iter()
                .map(|(host, container, ro)| {
                    if *ro {
                        format!("{}:{}:ro", host, container)
                    } else {
                        format!("{}:{}", host, container)
                    }
                })
                .collect();
            host_config.binds = Some(binds);
        }

        // Network mode
        if let Some(net) = network {
            if !net.is_empty() {
                host_config.network_mode = Some(net.to_string());
            }
        }

        // Environment variables
        let env: Option<Vec<String>> = if env_vars.is_empty() {
            None
        } else {
            Some(env_vars.iter().map(|(k, v)| format!("{}={}", k, v)).collect())
        };

        // Exposed ports (for port mappings)
        let exposed_ports: Option<HashMap<String, HashMap<(), ()>>> = if ports.is_empty() {
            None
        } else {
            let mut exposed = HashMap::new();
            for (_, container_port, protocol) in &ports {
                let key = format!("{}/{}", container_port, protocol);
                exposed.insert(key, HashMap::new());
            }
            Some(exposed)
        };

        // Build container config
        let config = ContainerCreateBody {
            image: Some(image.to_string()),
            cmd: command,
            entrypoint,
            working_dir: working_dir.map(String::from),
            env,
            exposed_ports,
            host_config: Some(host_config),
            ..Default::default()
        };

        // Create options
        let options = name.map(|n| CreateContainerOptions {
            name: Some(n.to_string()),
            platform: platform.map(String::from).unwrap_or_default(),
        });

        let response = docker.create_container(options, config).await?;
        Ok(response.id)
    }

    /// Get container logs
    pub async fn container_logs(&self, id: &str, tail: Option<usize>) -> Result<String> {
        let docker = self.client()?;

        let options = LogsOptions {
            stdout: true,
            stderr: true,
            tail: tail.map(|t| t.to_string()).unwrap_or_else(|| "100".to_string()),
            ..Default::default()
        };

        let mut stream = docker.logs(id, Some(options));
        let mut logs = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    logs.push_str(&output.to_string());
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to get logs: {}", e));
                }
            }
        }

        Ok(logs)
    }

    /// Inspect a container and return JSON
    pub async fn inspect_container(&self, id: &str) -> Result<String> {
        use bollard::query_parameters::InspectContainerOptions;
        let docker = self.client()?;
        let info = docker
            .inspect_container(id, None::<InspectContainerOptions>)
            .await?;
        let json = serde_json::to_string_pretty(&info)?;
        Ok(json)
    }

    /// Execute a command in a container and return output
    pub async fn exec_command(&self, id: &str, cmd: Vec<&str>) -> Result<String> {
        let docker = self.client()?;

        let exec = docker
            .create_exec(
                id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd),
                    ..Default::default()
                },
            )
            .await?;

        let output = docker.start_exec(&exec.id, None).await?;

        let mut result = String::new();
        if let StartExecResults::Attached { mut output, .. } = output {
            while let Some(Ok(msg)) = output.next().await {
                result.push_str(&msg.to_string());
            }
        }

        Ok(result)
    }

    /// List files in a container directory
    pub async fn list_container_files(&self, id: &str, path: &str) -> Result<Vec<ContainerFileEntry>> {
        // Use ls -la with specific format for parsing
        let output = self
            .exec_command(id, vec!["ls", "-la", path])
            .await?;

        let mut entries = Vec::new();

        for line in output.lines().skip(1) {
            // Skip "total" line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }

            let permissions = parts[0].to_string();
            let size: u64 = parts[4].parse().unwrap_or(0);
            let name = parts[8..].join(" ");

            // Skip . and ..
            if name == "." || name == ".." {
                continue;
            }

            let is_dir = permissions.starts_with('d');
            let is_symlink = permissions.starts_with('l');

            let full_path = if path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), name)
            };

            // For symlinks, extract just the name part (before ->)
            let display_name = if is_symlink {
                name.split(" -> ").next().unwrap_or(&name).to_string()
            } else {
                name
            };

            entries.push(ContainerFileEntry {
                name: display_name,
                path: full_path,
                is_dir,
                is_symlink,
                size,
                permissions,
            });
        }

        // Sort: directories first, then by name
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        Ok(entries)
    }
}
