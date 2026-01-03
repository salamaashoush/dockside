use anyhow::Result;
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
  CommitContainerOptions, CreateContainerOptions, KillContainerOptions, ListContainersOptions, LogsOptions,
  RemoveContainerOptions, RenameContainerOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::DockerClient;

/// Build Docker's exposed ports map from a set of port keys.
/// Docker API requires empty objects as values for exposed ports.
/// This function encapsulates the creation pattern required by the bollard library.
#[allow(clippy::zero_sized_map_values)] // Required by bollard library's API
fn build_exposed_ports_map(port_keys: HashSet<String>) -> HashMap<String, HashMap<(), ()>> {
  port_keys.into_iter().map(|key| (key, HashMap::new())).collect()
}

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
  pub fn is_running(self) -> bool {
    matches!(self, ContainerState::Running)
  }

  pub fn is_paused(self) -> bool {
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

/// Boolean flags for container creation
#[derive(Debug, Clone, Default)]
pub struct ContainerFlags {
  pub auto_remove: bool,
  pub privileged: bool,
  pub read_only: bool,
  pub init: bool,
}

/// Configuration options for creating a new container
#[derive(Debug, Clone, Default)]
pub struct ContainerCreateConfig {
  pub image: String,
  pub name: Option<String>,
  pub platform: Option<String>,
  pub command: Option<Vec<String>>,
  pub entrypoint: Option<Vec<String>>,
  pub working_dir: Option<String>,
  pub restart_policy: Option<String>,
  pub flags: ContainerFlags,
  pub env_vars: Vec<(String, String)>,
  pub ports: Vec<(String, String, String)>, // (host_port, container_port, protocol)
  pub volumes: Vec<(String, String, bool)>, // (host_path, container_path, read_only)
  pub network: Option<String>,
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
    if self.id.len() >= 12 { &self.id[..12] } else { &self.id }
  }

  pub fn display_ports(&self) -> String {
    self
      .ports
      .iter()
      .filter_map(|p| p.public_port.map(|pub_port| format!("{}:{}", pub_port, p.private_port)))
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
        .map_or_else(|| id.clone(), |n| n.trim_start_matches('/').to_string());

      let ports = container
        .ports
        .unwrap_or_default()
        .into_iter()
        .map(|p| PortMapping {
          private_port: p.private_port,
          public_port: p.public_port,
          protocol: p.typ.map_or_else(|| "tcp".to_string(), |t| t.to_string()),
          ip: p.ip,
        })
        .collect();

      let created = container.created.and_then(|ts| DateTime::from_timestamp(ts, 0));

      result.push(ContainerInfo {
        id,
        name,
        image: container.image.unwrap_or_default(),
        image_id: container.image_id.unwrap_or_default(),
        state: ContainerState::from_str(&container.state.map(|s| format!("{s:?}")).unwrap_or_default()),
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
    docker.start_container(id, None::<StartContainerOptions>).await?;
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
        RenameContainerOptions {
          name: new_name.to_string(),
        },
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
        CommitContainerOptions {
          container: Some(id.to_string()),
          repo: Some(repo.to_string()),
          tag: Some(tag.to_string()),
          comment: comment.map(ToString::to_string),
          author: author.map(ToString::to_string),
          pause: true,
          changes: None,
        },
        bollard::models::ContainerConfig::default(),
      )
      .await?;
    Ok(result.id)
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

  /// Create a new container from an image
  pub async fn create_container(&self, cfg: ContainerCreateConfig) -> Result<String> {
    let docker = self.client()?;

    // Build host config
    let mut host_config = HostConfig {
      auto_remove: Some(cfg.flags.auto_remove),
      privileged: Some(cfg.flags.privileged),
      readonly_rootfs: Some(cfg.flags.read_only),
      init: Some(cfg.flags.init),
      ..Default::default()
    };

    if let Some(ref policy) = cfg.restart_policy
      && policy != "no"
    {
      host_config.restart_policy = Some(bollard::models::RestartPolicy {
        name: Some(match policy.as_str() {
          "always" => bollard::models::RestartPolicyNameEnum::ALWAYS,
          "on-failure" => bollard::models::RestartPolicyNameEnum::ON_FAILURE,
          "unless-stopped" => bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED,
          _ => bollard::models::RestartPolicyNameEnum::NO,
        }),
        maximum_retry_count: None,
      });
    }

    // Port bindings
    if !cfg.ports.is_empty() {
      let mut port_bindings: HashMap<String, Option<Vec<bollard::models::PortBinding>>> = HashMap::new();
      for (host_port, container_port, protocol) in &cfg.ports {
        let key = format!("{container_port}/{protocol}");
        let binding = bollard::models::PortBinding {
          host_ip: Some("0.0.0.0".to_string()),
          host_port: Some(host_port.clone()),
        };
        port_bindings.insert(key, Some(vec![binding]));
      }
      host_config.port_bindings = Some(port_bindings);
    }

    // Volume bindings
    if !cfg.volumes.is_empty() {
      let binds: Vec<String> = cfg
        .volumes
        .iter()
        .map(|(host, container, ro)| {
          if *ro {
            format!("{host}:{container}:ro")
          } else {
            format!("{host}:{container}")
          }
        })
        .collect();
      host_config.binds = Some(binds);
    }

    // Network mode
    if let Some(ref net) = cfg.network
      && !net.is_empty()
    {
      host_config.network_mode = Some(net.clone());
    }

    // Environment variables
    let env: Option<Vec<String>> = if cfg.env_vars.is_empty() {
      None
    } else {
      Some(cfg.env_vars.iter().map(|(k, v)| format!("{k}={v}")).collect())
    };

    // Exposed ports (for port mappings) - Docker API requires empty object values
    let exposed_ports = if cfg.ports.is_empty() {
      None
    } else {
      let port_keys: HashSet<String> = cfg
        .ports
        .iter()
        .map(|(_, container_port, protocol)| format!("{container_port}/{protocol}"))
        .collect();
      Some(build_exposed_ports_map(port_keys))
    };

    // Build container config
    let config = ContainerCreateBody {
      image: Some(cfg.image.clone()),
      cmd: cfg.command,
      entrypoint: cfg.entrypoint,
      working_dir: cfg.working_dir,
      env,
      exposed_ports,
      host_config: Some(host_config),
      ..Default::default()
    };

    // Create options
    let options = cfg.name.as_ref().map(|n| CreateContainerOptions {
      name: Some(n.clone()),
      platform: cfg.platform.clone().unwrap_or_default(),
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
      tail: tail.map_or_else(|| "100".to_string(), |t| t.to_string()),
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
          return Err(anyhow::anyhow!("Failed to get logs: {e}"));
        }
      }
    }

    Ok(logs)
  }

  /// Inspect a container and return JSON
  pub async fn inspect_container(&self, id: &str) -> Result<String> {
    use bollard::query_parameters::InspectContainerOptions;
    let docker = self.client()?;
    let info = docker.inspect_container(id, None::<InspectContainerOptions>).await?;
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
    let output = self.exec_command(id, vec!["ls", "-la", path]).await?;

    let mut entries = Vec::new();

    for line in output.lines().skip(1) {
      // Skip "total" line
      let parts: Vec<&str> = line.split_whitespace().collect();
      if parts.len() < 9 {
        continue;
      }

      let permissions = parts[0].to_string();
      let size: u64 = parts[4].parse().unwrap_or(0);
      let raw_name = parts[8..].join(" ");

      let is_dir = permissions.starts_with('d');
      let is_symlink = permissions.starts_with('l');

      // For symlinks, extract just the name part (before ->)
      let name = if is_symlink {
        raw_name.split(" -> ").next().unwrap_or(&raw_name).to_string()
      } else {
        raw_name
      };

      // Skip . and ..
      if name == "." || name == ".." {
        continue;
      }

      let full_path = if path == "/" {
        format!("/{name}")
      } else {
        format!("{}/{}", path.trim_end_matches('/'), name)
      };

      entries.push(ContainerFileEntry {
        name: name.clone(),
        path: full_path,
        is_dir,
        is_symlink,
        size,
        permissions,
      });
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => a.name.cmp(&b.name),
    });

    Ok(entries)
  }

  /// Read file content from a container
  pub async fn read_container_file(&self, id: &str, path: &str) -> Result<String> {
    // Use cat to read file content
    let output = self.exec_command(id, vec!["cat", path]).await?;
    Ok(output)
  }

  /// Resolve a symlink to get its target path
  pub async fn resolve_symlink(&self, id: &str, path: &str) -> Result<String> {
    // Use readlink -f to get the absolute target path
    let output = self.exec_command(id, vec!["readlink", "-f", path]).await?;
    Ok(output.trim().to_string())
  }

  /// Check if a path is a directory
  pub async fn is_directory(&self, id: &str, path: &str) -> Result<bool> {
    let output = self
      .exec_command(id, vec!["sh", "-c", &format!("test -d '{path}' && echo dir")])
      .await;
    Ok(output.map(|s| s.contains("dir")).unwrap_or(false))
  }
}
