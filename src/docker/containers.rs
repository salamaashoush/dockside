use anyhow::{Result, anyhow};
use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
  CommitContainerOptions, CreateContainerOptions, KillContainerOptions, ListContainersOptions, LogsOptions,
  RemoveContainerOptions, RenameContainerOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
  TopOptionsBuilder,
};
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::DockerClient;

/// Translate any bare `\n` in the input into `\r\n` so a terminal
/// emulator (libghostty) advances both column and row. Bollard log
/// records typically arrive with `\n` only.
fn ensure_crlf(input: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(input.len() + 16);
  let mut prev: u8 = 0;
  for &b in input {
    if b == b'\n' && prev != b'\r' {
      out.push(b'\r');
    }
    out.push(b);
    prev = b;
  }
  out
}

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
  pub hostname: Option<String>,
  /// User-supplied labels (key, value).
  pub labels: Vec<(String, String)>,
  /// Number of CPU cores allowed (e.g. 1.5 → 1,500,000,000 `NanoCPUs`).
  pub cpus: Option<f64>,
  /// Hard memory cap in bytes.
  pub memory_bytes: Option<i64>,
  /// Memory + swap cap in bytes (Docker uses `memory_bytes` + swap; -1 = unlimited).
  pub memory_swap_bytes: Option<i64>,
  /// Max number of pids inside the container.
  pub pids_limit: Option<i64>,
  /// Healthcheck command. First element is interpreted (CMD-SHELL by
  /// default) when wrapping in a shell; bollard expects the raw argv.
  pub healthcheck_cmd: Option<Vec<String>>,
  /// Interval between healthcheck attempts (nanoseconds, 0 = default).
  pub healthcheck_interval_ns: Option<i64>,
  /// Per-attempt timeout (nanoseconds, 0 = default).
  pub healthcheck_timeout_ns: Option<i64>,
  /// Initial start period before failures count (nanoseconds, 0 = default).
  pub healthcheck_start_period_ns: Option<i64>,
  /// Number of retries before marking unhealthy.
  pub healthcheck_retries: Option<i64>,
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
  /// Names of volumes (named, not bind/tmpfs) mounted into the container.
  /// Pulled from the daemon's container list response so the Volumes
  /// view can cross-reference without an inspect-per-container.
  pub volumes_used: Vec<String>,
  /// Names of networks the container is attached to. Pulled from
  /// `NetworkSettings.Networks` on the `list_containers` response so
  /// the Networks view can cross-reference without inspecting each
  /// container.
  pub networks_used: Vec<String>,
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

/// Extras pulled from a full container inspect — surfaced on the Info tab.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerExtras {
  pub restart_count: Option<i64>,
  pub exit_code: Option<i64>,
  pub started_at: Option<String>,
  pub finished_at: Option<String>,
  pub health: Option<ContainerHealth>,
  pub mounts: Vec<MountInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerHealth {
  pub status: String,
  pub failing_streak: Option<i64>,
  pub log: Vec<HealthLogEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthLogEntry {
  pub start: Option<String>,
  pub end: Option<String>,
  pub exit_code: Option<i64>,
  pub output: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MountInfo {
  /// "bind" / "volume" / "tmpfs" etc.
  pub kind: String,
  pub source: String,
  pub destination: String,
  pub mode: String,
  pub rw: bool,
  /// Volume name (only set for `volume` mounts).
  pub name: Option<String>,
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

      let volumes_used: Vec<String> = container
        .mounts
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
          // Only named volumes are useful as cross-references on the
          // Volumes view. Bind / tmpfs mounts go into the source path
          // directly and aren't tied to a Volume object.
          let kind = m.typ.map(|t| format!("{t:?}")).unwrap_or_default().to_lowercase();
          if kind == "volume" { m.name } else { None }
        })
        .collect();

      let networks_used: Vec<String> = container
        .network_settings
        .clone()
        .and_then(|ns| ns.networks)
        .map(|m| m.into_keys().collect())
        .unwrap_or_default();

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
        volumes_used,
        networks_used,
      });
    }

    // Sort: running first (then paused, restarting), then stopped/exited/dead.
    // Within each bucket, most recently created first, name-tiebreak.
    result.sort_by(|a, b| {
      let bucket = |s: ContainerState| match s {
        ContainerState::Running => 0,
        ContainerState::Paused | ContainerState::Restarting => 1,
        ContainerState::Created => 2,
        ContainerState::Exited | ContainerState::Removing => 3,
        ContainerState::Dead | ContainerState::Unknown => 4,
      };
      bucket(a.state)
        .cmp(&bucket(b.state))
        .then_with(|| b.created.cmp(&a.created))
        .then_with(|| a.name.cmp(&b.name))
    });
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

    // Resource limits.
    if let Some(cpus) = cfg.cpus {
      // 1 CPU = 1_000_000_000 NanoCPUs.
      #[allow(clippy::cast_possible_truncation)]
      let nano = (cpus * 1_000_000_000.0) as i64;
      if nano > 0 {
        host_config.nano_cpus = Some(nano);
      }
    }
    if let Some(mem) = cfg.memory_bytes
      && mem > 0
    {
      host_config.memory = Some(mem);
    }
    if let Some(memswap) = cfg.memory_swap_bytes {
      host_config.memory_swap = Some(memswap);
    }
    if let Some(pids) = cfg.pids_limit
      && pids > 0
    {
      host_config.pids_limit = Some(pids);
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

    // Labels.
    let labels: Option<HashMap<String, String>> = if cfg.labels.is_empty() {
      None
    } else {
      Some(cfg.labels.iter().cloned().collect())
    };

    // Healthcheck.
    let healthcheck = cfg.healthcheck_cmd.as_ref().map(|argv| {
      // Wrap in CMD-SHELL only when the user gave a single string;
      // otherwise pass argv through as-is.
      let test = if argv.len() == 1 {
        vec!["CMD-SHELL".to_string(), argv[0].clone()]
      } else {
        let mut v = vec!["CMD".to_string()];
        v.extend(argv.iter().cloned());
        v
      };
      bollard::models::HealthConfig {
        test: Some(test),
        interval: cfg.healthcheck_interval_ns,
        timeout: cfg.healthcheck_timeout_ns,
        retries: cfg.healthcheck_retries,
        start_period: cfg.healthcheck_start_period_ns,
        ..Default::default()
      }
    });

    // Build container config
    let config = ContainerCreateBody {
      image: Some(cfg.image.clone()),
      cmd: cfg.command,
      entrypoint: cfg.entrypoint,
      working_dir: cfg.working_dir,
      env,
      exposed_ports,
      host_config: Some(host_config),
      hostname: cfg.hostname,
      labels,
      healthcheck,
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

  /// Fetch container logs as a snapshot (no follow).
  pub async fn container_logs(&self, id: &str, tail: Option<usize>, timestamps: bool) -> Result<String> {
    let docker = self.client()?;

    let options = LogsOptions {
      stdout: true,
      stderr: true,
      timestamps,
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

  /// Stream container logs as raw bytes until the receiver is dropped or
  /// the container exits. Bytes are post-de-mux (bollard strips Docker's
  /// 8-byte stream-frame header), so they are directly feedable into a
  /// terminal emulator. Newlines are translated to CRLF so libghostty
  /// renders one line per record.
  pub async fn stream_container_logs(
    &self,
    id: &str,
    tail: Option<usize>,
    timestamps: bool,
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
  ) -> Result<()> {
    let docker = self.client()?;

    let options = LogsOptions {
      stdout: true,
      stderr: true,
      timestamps,
      follow: true,
      tail: tail.map_or_else(|| "100".to_string(), |t| t.to_string()),
      ..Default::default()
    };

    let mut stream = docker.logs(id, Some(options));
    let mut sent = 0usize;
    while let Some(result) = stream.next().await {
      match result {
        Ok(output) => {
          let bytes = output.into_bytes();
          let raw: Vec<u8> = ensure_crlf(&bytes);
          tracing::trace!(target: "dockside.logs", bytes = raw.len(), "stream chunk");
          if tx.send(raw).await.is_err() {
            tracing::info!(target: "dockside.logs", chunks = sent, "send failed, receiver dropped");
            return Ok(());
          }
          sent += 1;
        }
        Err(e) => {
          tracing::warn!(target: "dockside.logs", err = %e, "bollard error");
          return Err(anyhow::anyhow!("Log stream failed: {e}"));
        }
      }
    }
    tracing::info!(target: "dockside.logs", chunks = sent, "bollard stream ended");
    Ok(())
  }

  /// Inspect a container and return JSON
  pub async fn inspect_container(&self, id: &str) -> Result<String> {
    use bollard::query_parameters::InspectContainerOptions;
    let docker = self.client()?;
    let info = docker.inspect_container(id, None::<InspectContainerOptions>).await?;
    let json = serde_json::to_string_pretty(&info)?;
    Ok(json)
  }

  /// Pull out the structured "extras" we need for the Info tab — health
  /// status + recent log lines, restart count, exit code, mounts list.
  pub async fn container_extras(&self, id: &str) -> Result<ContainerExtras> {
    use bollard::query_parameters::InspectContainerOptions;
    let docker = self.client()?;
    let info = docker.inspect_container(id, None::<InspectContainerOptions>).await?;

    let state = info.state.unwrap_or_default();
    let exit_code = state.exit_code;
    let restart_count = info.restart_count;
    let started_at = state.started_at;
    let finished_at = state.finished_at;

    let health = state.health.map(|h| {
      let status = h.status.map_or_else(String::new, |s| s.to_string());
      let failing_streak = h.failing_streak;
      let log: Vec<HealthLogEntry> = h
        .log
        .unwrap_or_default()
        .into_iter()
        .map(|entry| HealthLogEntry {
          start: entry.start,
          end: entry.end,
          exit_code: entry.exit_code,
          output: entry.output.unwrap_or_default(),
        })
        .collect();
      ContainerHealth {
        status,
        failing_streak,
        log,
      }
    });

    let mounts = info
      .mounts
      .unwrap_or_default()
      .into_iter()
      .map(|m| MountInfo {
        kind: m.typ.map_or_else(String::new, |t| t.to_string()),
        source: m.source.unwrap_or_default(),
        destination: m.destination.unwrap_or_default(),
        mode: m.mode.unwrap_or_default(),
        rw: m.rw.unwrap_or(true),
        name: m.name,
      })
      .collect();

    Ok(ContainerExtras {
      restart_count,
      exit_code,
      started_at,
      finished_at,
      health,
      mounts,
    })
  }

  /// Execute a command in a container and return combined stdout+stderr.
  /// Treats any nonzero exit code as success (caller may want stderr text).
  /// For richer error handling use `exec_command_full`.
  pub async fn exec_command(&self, id: &str, cmd: Vec<&str>) -> Result<String> {
    let result = self.exec_command_full(id, cmd).await?;
    let mut combined = result.stdout;
    if !result.stderr.is_empty() {
      if !combined.is_empty() && !combined.ends_with('\n') {
        combined.push('\n');
      }
      combined.push_str(&result.stderr);
    }
    Ok(combined)
  }

  /// Execute a command in a container, capturing stdout, stderr, and exit code separately.
  pub async fn exec_command_full(&self, id: &str, cmd: Vec<&str>) -> Result<ExecResult> {
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
      .await
      .map_err(|e| translate_exec_error("create exec", id, &e))?;

    let output = docker
      .start_exec(&exec.id, None)
      .await
      .map_err(|e| translate_exec_error("start exec", id, &e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let StartExecResults::Attached { mut output, .. } = output {
      while let Some(Ok(msg)) = output.next().await {
        match msg {
          LogOutput::StdOut { message } | LogOutput::Console { message } => {
            stdout.push_str(&String::from_utf8_lossy(&message));
          }
          LogOutput::StdErr { message } => {
            stderr.push_str(&String::from_utf8_lossy(&message));
          }
          LogOutput::StdIn { .. } => {}
        }
      }
    }

    // Inspect to get exit code (Docker exec exit codes are reported async).
    let exit_code = match docker.inspect_exec(&exec.id).await {
      Ok(info) => info.exit_code,
      Err(e) => {
        tracing::debug!("inspect_exec failed for {id}: {e}");
        None
      }
    };

    Ok(ExecResult {
      stdout,
      stderr,
      exit_code,
    })
  }

  /// List files in a container directory.
  ///
  /// Tries `ls -la` first, then falls back to a `sh`-based stat loop for
  /// busybox-style images. Returns a structured error if no listing strategy
  /// works (typical for distroless / scratch images).
  pub async fn list_container_files(&self, id: &str, path: &str) -> Result<Vec<ContainerFileEntry>> {
    // Strategy 1: GNU/BusyBox `ls -la` — covers most images including alpine,
    // ubuntu, debian, busybox, postgres, nginx, etc.
    let ls_result = self.exec_command_full(id, vec!["ls", "-la", "--", path]).await?;
    if ls_result.is_success() && !ls_result.stdout.is_empty() {
      return Ok(parse_ls_la_output(&ls_result.stdout, path));
    }

    // Strategy 2: BusyBox `ls` without GNU flags (no `--` end-of-options).
    let ls_simple = self.exec_command_full(id, vec!["ls", "-la", path]).await?;
    if ls_simple.is_success() && !ls_simple.stdout.is_empty() {
      return Ok(parse_ls_la_output(&ls_simple.stdout, path));
    }

    // No `ls` available — surface the clearest error we have.
    let stderr = if ls_result.stderr.trim().is_empty() {
      ls_simple.stderr
    } else {
      ls_result.stderr
    };
    Err(translate_filesystem_error("list directory", path, &stderr))
  }

  /// Read file content from a container.
  ///
  /// Caps reads at 1 MiB to keep the UI responsive and detects binary content
  /// (returns a friendly error rather than dumping garbage).
  pub async fn read_container_file(&self, id: &str, path: &str) -> Result<String> {
    const MAX_READ_BYTES: u64 = 1024 * 1024;

    let result = self.exec_command_full(id, vec!["cat", "--", path]).await?;
    if !result.is_success() {
      // Retry without `--` for busybox-style cat.
      let fallback = self.exec_command_full(id, vec!["cat", path]).await?;
      if !fallback.is_success() {
        let stderr = if fallback.stderr.trim().is_empty() {
          result.stderr
        } else {
          fallback.stderr
        };
        return Err(translate_filesystem_error("read file", path, &stderr));
      }
      return finalize_file_read(fallback.stdout, MAX_READ_BYTES);
    }
    finalize_file_read(result.stdout, MAX_READ_BYTES)
  }

  /// Resolve a symlink to its absolute target path.
  pub async fn resolve_symlink(&self, id: &str, path: &str) -> Result<String> {
    // GNU `readlink -f` is most reliable; busybox supports it too.
    let result = self.exec_command_full(id, vec!["readlink", "-f", "--", path]).await?;
    if result.is_success() && !result.stdout.trim().is_empty() {
      return Ok(result.stdout.trim().to_string());
    }
    // Some minimal images lack `readlink`. Fall back to `ls -l` parsing.
    let fallback = self.exec_command_full(id, vec!["ls", "-l", "--", path]).await?;
    if fallback.is_success()
      && let Some(target) = fallback.stdout.split(" -> ").nth(1)
    {
      return Ok(target.trim().to_string());
    }
    Err(anyhow!("Could not resolve symlink for {path}"))
  }

  /// Check if a path is a directory.
  pub async fn is_directory(&self, id: &str, path: &str) -> Result<bool> {
    // Use `sh -c test -d` since most images have at least sh.
    let result = self
      .exec_command_full(
        id,
        vec!["sh", "-c", &format!("test -d {} && echo dir", shell_quote(path))],
      )
      .await?;
    Ok(result.stdout.contains("dir"))
  }

  /// Get running processes in a container using Docker's top API
  ///
  /// This uses Docker's native `top` endpoint which works for all containers,
  /// even minimal ones that don't have `ps` installed.
  /// Falls back to `docker exec ps aux` if the API fails.
  ///
  /// Returns output formatted like `ps aux` for compatibility with parsing.
  pub async fn get_container_processes(&self, id: &str) -> Result<String> {
    let docker = self.client()?;

    // First, try Docker's native top API - this works for all containers
    // regardless of what binaries are installed
    let top_opts = TopOptionsBuilder::new().ps_args("aux").build();
    match docker.top_processes(id, Some(top_opts)).await {
      Ok(top_result) => {
        // Format the result like `ps aux` output
        let mut output = String::new();

        // Add header from titles
        if let Some(titles) = &top_result.titles {
          output.push_str(&titles.join("\t"));
          output.push('\n');
        }

        // Add process rows
        if let Some(processes) = &top_result.processes {
          for proc in processes {
            output.push_str(&proc.join("\t"));
            output.push('\n');
          }
        }

        if output.is_empty() || output.trim().lines().count() <= 1 {
          // Only header or empty - return header so UI shows "No processes found"
          return Ok("USER\tPID\t%CPU\t%MEM\tVSZ\tRSS\tTTY\tSTAT\tSTART\tTIME\tCOMMAND\n".to_string());
        }

        return Ok(output);
      }
      Err(e) => {
        // Log the error but try fallback methods
        tracing::debug!("Docker top API failed for container {id}: {e}, trying fallback methods");
      }
    }

    // Fallback: try exec with different ps variants
    // Some containers might have busybox ps, some might have procps
    let ps_commands = [
      vec!["ps", "aux"],                                                        // Standard ps (procps)
      vec!["ps", "-ef"],                                                        // POSIX compatible
      vec!["ps", "-eo", "user,pid,pcpu,pmem,vsz,rss,tty,stat,start,time,comm"], // Custom format
      vec!["/bin/ps", "aux"],                                                   // Explicit path
    ];

    for cmd in &ps_commands {
      match self.exec_command(id, cmd.clone()).await {
        Ok(output) if !output.is_empty() && !output.contains("not found") => {
          return Ok(output);
        }
        Ok(_) | Err(_) => {}
      }
    }

    // If all methods fail, return an error with guidance
    Err(anyhow::anyhow!(
      "Could not get process list. The container may not have 'ps' installed and Docker top API failed."
    ))
  }
}

// ============================================================================
// Exec helpers
// ============================================================================

/// Result of running a command inside a container.
#[derive(Debug, Clone, Default)]
pub struct ExecResult {
  pub stdout: String,
  pub stderr: String,
  /// `None` if Docker hasn't reported one yet; otherwise the child's exit status.
  pub exit_code: Option<i64>,
}

impl ExecResult {
  /// True if Docker reported a zero exit code.
  pub fn is_success(&self) -> bool {
    matches!(self.exit_code, Some(0))
  }
}

/// Translate a `bollard` create/start exec error into a friendly message.
fn translate_exec_error(stage: &str, id: &str, err: &bollard::errors::Error) -> anyhow::Error {
  let s = err.to_string();
  let lower = s.to_lowercase();
  if lower.contains("is not running") || lower.contains("is restarting") {
    anyhow!("Container {id} is not running")
  } else if lower.contains("no such container") || lower.contains("not found") {
    anyhow!("Container {id} no longer exists")
  } else if lower.contains("permission denied") {
    anyhow!("Permission denied while attaching to container {id}")
  } else {
    anyhow!("Docker {stage} failed: {s}")
  }
}

/// Translate a stderr message from a filesystem-related exec into a useful error.
fn translate_filesystem_error(action: &str, path: &str, stderr: &str) -> anyhow::Error {
  let lower = stderr.to_lowercase();
  if lower.contains("no such file") || lower.contains("not found") {
    anyhow!("{action}: {path} does not exist in this container")
  } else if lower.contains("permission denied") {
    anyhow!("{action}: permission denied for {path}")
  } else if lower.contains("is a directory") {
    anyhow!("{action}: {path} is a directory")
  } else if lower.contains("not a directory") {
    anyhow!("{action}: {path} is not a directory")
  } else if lower.contains("oci runtime exec failed") || lower.contains("executable file not found") {
    anyhow!("{action}: this container does not include the required tools (try a container with a shell + coreutils)")
  } else if stderr.trim().is_empty() {
    anyhow!("{action}: failed for {path}")
  } else {
    anyhow!("{action}: {}", stderr.trim())
  }
}

/// Parse `ls -la` output into `ContainerFileEntry` rows.
fn parse_ls_la_output(output: &str, path: &str) -> Vec<ContainerFileEntry> {
  let mut entries = Vec::new();

  for line in output.lines().skip_while(|l| l.starts_with("total ")) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
      continue;
    }
    let permissions = parts[0].to_string();
    let size: u64 = parts[4].parse().unwrap_or(0);
    let raw_name = parts[8..].join(" ");
    let is_dir = permissions.starts_with('d');
    let is_symlink = permissions.starts_with('l');
    let name = if is_symlink {
      raw_name.split(" -> ").next().unwrap_or(&raw_name).to_string()
    } else {
      raw_name
    };
    if name == "." || name == ".." {
      continue;
    }
    let full_path = if path == "/" {
      format!("/{name}")
    } else {
      format!("{}/{}", path.trim_end_matches('/'), name)
    };
    entries.push(ContainerFileEntry {
      name,
      path: full_path,
      is_dir,
      is_symlink,
      size,
      permissions,
    });
  }

  entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
    (true, false) => std::cmp::Ordering::Less,
    (false, true) => std::cmp::Ordering::Greater,
    _ => a.name.cmp(&b.name),
  });
  entries
}

/// Cap file content at `max_bytes` and reject obvious binary blobs so the UI
/// gets a clear message instead of trying to render garbage.
fn finalize_file_read(content: String, max_bytes: u64) -> Result<String> {
  if content.is_empty() {
    return Ok(content);
  }
  let bytes = content.as_bytes();
  // Heuristic: if the first 8 KiB contain a NUL byte, treat as binary.
  let scan = &bytes[..bytes.len().min(8 * 1024)];
  if scan.contains(&0) {
    return Err(anyhow!(
      "File appears to be binary; download or hex-view it instead of opening as text"
    ));
  }
  if (bytes.len() as u64) > max_bytes {
    let cap = usize::try_from(max_bytes).unwrap_or(usize::MAX).min(bytes.len());
    let truncated = String::from_utf8_lossy(&bytes[..cap]).to_string();
    return Ok(format!(
      "{truncated}\n\n--- truncated at {} ---\n",
      bytesize::ByteSize(max_bytes),
    ));
  }
  Ok(content)
}

/// Single-quote a path for `sh -c` use.
fn shell_quote(s: &str) -> String {
  let escaped = s.replace('\'', r"'\''");
  format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_container_state_from_str() {
    assert_eq!(ContainerState::from_str("running"), ContainerState::Running);
    assert_eq!(ContainerState::from_str("Running"), ContainerState::Running);
    assert_eq!(ContainerState::from_str("RUNNING"), ContainerState::Running);
    assert_eq!(ContainerState::from_str("paused"), ContainerState::Paused);
    assert_eq!(ContainerState::from_str("restarting"), ContainerState::Restarting);
    assert_eq!(ContainerState::from_str("exited"), ContainerState::Exited);
    assert_eq!(ContainerState::from_str("dead"), ContainerState::Dead);
    assert_eq!(ContainerState::from_str("created"), ContainerState::Created);
    assert_eq!(ContainerState::from_str("removing"), ContainerState::Removing);
    assert_eq!(ContainerState::from_str("unknown"), ContainerState::Unknown);
    assert_eq!(ContainerState::from_str("invalid"), ContainerState::Unknown);
  }

  #[test]
  fn test_container_state_is_running() {
    assert!(ContainerState::Running.is_running());
    assert!(!ContainerState::Paused.is_running());
    assert!(!ContainerState::Exited.is_running());
    assert!(!ContainerState::Unknown.is_running());
  }

  #[test]
  fn test_container_state_is_paused() {
    assert!(ContainerState::Paused.is_paused());
    assert!(!ContainerState::Running.is_paused());
    assert!(!ContainerState::Exited.is_paused());
  }

  #[test]
  fn test_container_state_display() {
    assert_eq!(format!("{}", ContainerState::Running), "Running");
    assert_eq!(format!("{}", ContainerState::Paused), "Paused");
    assert_eq!(format!("{}", ContainerState::Restarting), "Restarting");
    assert_eq!(format!("{}", ContainerState::Exited), "Exited");
    assert_eq!(format!("{}", ContainerState::Dead), "Dead");
    assert_eq!(format!("{}", ContainerState::Created), "Created");
    assert_eq!(format!("{}", ContainerState::Removing), "Removing");
    assert_eq!(format!("{}", ContainerState::Unknown), "Unknown");
  }

  #[test]
  fn test_container_info_short_id() {
    let container = ContainerInfo {
      id: "abc123def456789".to_string(),
      name: "test".to_string(),
      image: "nginx".to_string(),
      image_id: "sha256:abc".to_string(),
      state: ContainerState::Running,
      status: "Up 5 minutes".to_string(),
      created: None,
      ports: vec![],
      labels: HashMap::new(),
      command: None,
      size_rw: None,
      size_root_fs: None,
      volumes_used: vec![],
      networks_used: vec![],
    };
    assert_eq!(container.short_id(), "abc123def456");

    // Test with short id
    let short_container = ContainerInfo {
      id: "abc".to_string(),
      ..container.clone()
    };
    assert_eq!(short_container.short_id(), "abc");
  }

  #[test]
  fn test_container_info_display_ports() {
    let container = ContainerInfo {
      id: "abc123".to_string(),
      name: "test".to_string(),
      image: "nginx".to_string(),
      image_id: "sha256:abc".to_string(),
      state: ContainerState::Running,
      status: "Up 5 minutes".to_string(),
      created: None,
      ports: vec![
        PortMapping {
          private_port: 80,
          public_port: Some(8080),
          protocol: "tcp".to_string(),
          ip: Some("0.0.0.0".to_string()),
        },
        PortMapping {
          private_port: 443,
          public_port: Some(8443),
          protocol: "tcp".to_string(),
          ip: None,
        },
        PortMapping {
          private_port: 53,
          public_port: None, // No public port
          protocol: "udp".to_string(),
          ip: None,
        },
      ],
      labels: HashMap::new(),
      command: None,
      size_rw: None,
      size_root_fs: None,
      volumes_used: vec![],
      networks_used: vec![],
    };
    assert_eq!(container.display_ports(), "8080:80, 8443:443");

    // Test with no ports
    let no_ports = ContainerInfo {
      ports: vec![],
      ..container.clone()
    };
    assert_eq!(no_ports.display_ports(), "");
  }

  #[test]
  fn test_container_file_entry_display_size() {
    // Directory shows "-"
    let dir = ContainerFileEntry {
      name: "test".to_string(),
      path: "/test".to_string(),
      is_dir: true,
      is_symlink: false,
      size: 4096,
      permissions: "drwxr-xr-x".to_string(),
    };
    assert_eq!(dir.display_size(), "-");

    // File shows formatted size (bytesize uses binary units)
    let file = ContainerFileEntry {
      name: "test.txt".to_string(),
      path: "/test.txt".to_string(),
      is_dir: false,
      is_symlink: false,
      size: 1024,
      permissions: "-rw-r--r--".to_string(),
    };
    assert_eq!(file.display_size(), "1.0 KiB");

    // Large file
    let large_file = ContainerFileEntry {
      size: 1024 * 1024 * 5, // 5 MiB
      ..file.clone()
    };
    assert_eq!(large_file.display_size(), "5.0 MiB");
  }

  #[test]
  fn test_port_mapping() {
    let port = PortMapping {
      private_port: 80,
      public_port: Some(8080),
      protocol: "tcp".to_string(),
      ip: Some("0.0.0.0".to_string()),
    };
    assert_eq!(port.private_port, 80);
    assert_eq!(port.public_port, Some(8080));
    assert_eq!(port.protocol, "tcp");
    assert_eq!(port.ip, Some("0.0.0.0".to_string()));
  }

  #[test]
  fn test_container_flags_default() {
    let flags = ContainerFlags::default();
    assert!(!flags.auto_remove);
    assert!(!flags.privileged);
    assert!(!flags.read_only);
    assert!(!flags.init);
  }

  #[test]
  fn test_container_create_config_default() {
    let config = ContainerCreateConfig::default();
    assert!(config.image.is_empty());
    assert!(config.name.is_none());
    assert!(config.platform.is_none());
    assert!(config.command.is_none());
    assert!(config.entrypoint.is_none());
    assert!(config.working_dir.is_none());
    assert!(config.restart_policy.is_none());
    assert!(config.env_vars.is_empty());
    assert!(config.ports.is_empty());
    assert!(config.volumes.is_empty());
    assert!(config.network.is_none());
  }

  #[test]
  fn test_build_exposed_ports_map() {
    let ports: HashSet<String> = ["80/tcp".to_string(), "443/tcp".to_string()].into_iter().collect();
    let result = build_exposed_ports_map(ports);
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("80/tcp"));
    assert!(result.contains_key("443/tcp"));
  }

  // Edge case tests for real-world scenarios

  #[test]
  fn test_container_state_case_insensitive() {
    // Docker API can return states in different cases
    assert_eq!(ContainerState::from_str("RUNNING"), ContainerState::Running);
    assert_eq!(ContainerState::from_str("Running"), ContainerState::Running);
    assert_eq!(ContainerState::from_str("RuNnInG"), ContainerState::Running);
  }

  #[test]
  fn test_container_short_id_exact_12_chars() {
    let container = ContainerInfo {
      id: "123456789012".to_string(), // exactly 12 chars
      name: "test".to_string(),
      image: "nginx".to_string(),
      image_id: "sha256:abc".to_string(),
      state: ContainerState::Running,
      status: "Up".to_string(),
      created: None,
      ports: vec![],
      labels: HashMap::new(),
      command: None,
      size_rw: None,
      size_root_fs: None,
      volumes_used: vec![],
      networks_used: vec![],
    };
    assert_eq!(container.short_id(), "123456789012");
  }

  #[test]
  fn test_container_display_ports_internal_only() {
    // Container with internal port but no public port (common for linked containers)
    let container = ContainerInfo {
      id: "abc123".to_string(),
      name: "internal-only".to_string(),
      image: "redis".to_string(),
      image_id: "sha256:abc".to_string(),
      state: ContainerState::Running,
      status: "Up".to_string(),
      created: None,
      ports: vec![PortMapping {
        private_port: 6379,
        public_port: None,
        protocol: "tcp".to_string(),
        ip: None,
      }],
      labels: HashMap::new(),
      command: None,
      size_rw: None,
      size_root_fs: None,
      volumes_used: vec![],
      networks_used: vec![],
    };
    // Should return empty string when no public ports
    assert_eq!(container.display_ports(), "");
  }

  #[test]
  fn test_container_file_entry_zero_size() {
    // Empty files should display as 0 B
    let empty_file = ContainerFileEntry {
      name: "empty.txt".to_string(),
      path: "/empty.txt".to_string(),
      is_dir: false,
      is_symlink: false,
      size: 0,
      permissions: "-rw-r--r--".to_string(),
    };
    assert_eq!(empty_file.display_size(), "0 B");
  }

  #[test]
  fn test_container_file_entry_symlink_shows_size() {
    // Symlinks are files, should show size not "-"
    let symlink = ContainerFileEntry {
      name: "link".to_string(),
      path: "/link".to_string(),
      is_dir: false,
      is_symlink: true,
      size: 42,
      permissions: "lrwxrwxrwx".to_string(),
    };
    assert_eq!(symlink.display_size(), "42 B");
  }
}
