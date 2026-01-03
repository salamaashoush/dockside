use anyhow::Result;
use bollard::query_parameters::StatsOptions;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{ContainerState, DockerClient};

/// Container resource statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerStats {
  pub id: String,
  pub name: String,
  pub cpu_percent: f64,
  pub memory_usage: u64,
  pub memory_limit: u64,
  pub memory_percent: f64,
  pub network_rx: u64,
  pub network_tx: u64,
  pub block_read: u64,
  pub block_write: u64,
}

impl ContainerStats {
  pub fn display_memory(&self) -> String {
    format_bytes(self.memory_usage)
  }

  pub fn display_network_rx(&self) -> String {
    format!("{}/s", format_bytes(self.network_rx))
  }

  pub fn display_block_read(&self) -> String {
    format!("{}/s", format_bytes(self.block_read))
  }
}

fn format_bytes(bytes: u64) -> String {
  const KB: u64 = 1024;
  const MB: u64 = KB * 1024;
  const GB: u64 = MB * 1024;

  if bytes >= GB {
    format!("{:.1} GB", bytes as f64 / GB as f64)
  } else if bytes >= MB {
    format!("{:.1} MB", bytes as f64 / MB as f64)
  } else if bytes >= KB {
    format!("{:.0} KB", bytes as f64 / KB as f64)
  } else {
    format!("{} B", bytes)
  }
}

/// Aggregate stats across all containers
#[derive(Debug, Clone, Default)]
pub struct AggregateStats {
  pub total_cpu_percent: f64,
  pub total_memory: u64,
  pub total_network_rx: u64,
  pub total_network_tx: u64,
  pub total_block_read: u64,
  pub total_block_write: u64,
  pub container_stats: Vec<ContainerStats>,
}

impl AggregateStats {
  pub fn display_total_memory(&self) -> String {
    format_bytes(self.total_memory)
  }

  pub fn display_total_network(&self) -> String {
    format!("{}/s", format_bytes(self.total_network_rx + self.total_network_tx))
  }

  pub fn display_total_disk(&self) -> String {
    format!("{}/s", format_bytes(self.total_block_read + self.total_block_write))
  }
}

impl DockerClient {
  /// Get stats for a single container (one-shot, not streaming)
  pub async fn get_container_stats(&self, container_id: &str) -> Result<ContainerStats> {
    let docker = self.client()?;

    let options = StatsOptions {
      stream: false,
      one_shot: true,
    };

    let mut stream = docker.stats(container_id, Some(options));

    if let Some(result) = stream.next().await {
      let stats = result?;

      // Calculate CPU percentage
      let cpu_stats = stats.cpu_stats.as_ref();
      let precpu_stats = stats.precpu_stats.as_ref();

      let cpu_delta = cpu_stats
        .and_then(|s| s.cpu_usage.as_ref())
        .and_then(|u| u.total_usage)
        .unwrap_or(0)
        .saturating_sub(
          precpu_stats
            .and_then(|s| s.cpu_usage.as_ref())
            .and_then(|u| u.total_usage)
            .unwrap_or(0),
        );
      let system_delta = cpu_stats
        .and_then(|s| s.system_cpu_usage)
        .unwrap_or(0)
        .saturating_sub(precpu_stats.and_then(|s| s.system_cpu_usage).unwrap_or(0));

      let cpu_percent = if system_delta > 0 && cpu_delta > 0 {
        let num_cpus = cpu_stats.and_then(|s| s.online_cpus).unwrap_or(1) as f64;
        (cpu_delta as f64 / system_delta as f64) * num_cpus * 100.0
      } else {
        0.0
      };

      // Memory stats
      let memory_stats = stats.memory_stats.as_ref();
      let memory_usage = memory_stats.and_then(|s| s.usage).unwrap_or(0);
      let memory_limit = memory_stats.and_then(|s| s.limit).unwrap_or(1);
      let memory_percent = if memory_limit > 0 {
        (memory_usage as f64 / memory_limit as f64) * 100.0
      } else {
        0.0
      };

      // Network stats (aggregate all interfaces)
      let (network_rx, network_tx) = if let Some(networks) = &stats.networks {
        networks.values().fold((0u64, 0u64), |(rx, tx), net| {
          (rx + net.rx_bytes.unwrap_or(0), tx + net.tx_bytes.unwrap_or(0))
        })
      } else {
        (0, 0)
      };

      // Block I/O stats
      let (block_read, block_write) = stats
        .blkio_stats
        .as_ref()
        .and_then(|s| s.io_service_bytes_recursive.as_ref())
        .map(|entries| {
          entries.iter().fold((0u64, 0u64), |(read, write), entry| {
            let op = entry.op.as_deref().unwrap_or("");
            let value = entry.value.unwrap_or(0);
            match op {
              "read" | "Read" => (read + value, write),
              "write" | "Write" => (read, write + value),
              _ => (read, write),
            }
          })
        })
        .unwrap_or((0, 0));

      // Clean up the name (remove leading /)
      let name = stats
        .name
        .as_deref()
        .map(|n| n.trim_start_matches('/'))
        .unwrap_or("")
        .to_string();

      Ok(ContainerStats {
        id: stats.id.unwrap_or_default(),
        name,
        cpu_percent,
        memory_usage,
        memory_limit,
        memory_percent,
        network_rx,
        network_tx,
        block_read,
        block_write,
      })
    } else {
      Err(anyhow::anyhow!("No stats available for container"))
    }
  }

  /// Get stats for all running containers
  pub async fn get_all_container_stats(&self) -> Result<AggregateStats> {
    let containers = self.list_containers(true).await?;

    let mut aggregate = AggregateStats::default();
    let mut stats_list = Vec::new();

    for container in containers {
      // Only get stats for running containers
      if container.state == ContainerState::Running {
        match self.get_container_stats(&container.id).await {
          Ok(mut stats) => {
            // Use the container name from list_containers if stats name is empty
            if stats.name.is_empty() {
              stats.name = container.name.clone();
            }
            aggregate.total_cpu_percent += stats.cpu_percent;
            aggregate.total_memory += stats.memory_usage;
            aggregate.total_network_rx += stats.network_rx;
            aggregate.total_network_tx += stats.network_tx;
            aggregate.total_block_read += stats.block_read;
            aggregate.total_block_write += stats.block_write;
            stats_list.push(stats);
          }
          Err(_) => {
            // Container might have stopped, skip it
            continue;
          }
        }
      }
    }

    aggregate.container_stats = stats_list;
    Ok(aggregate)
  }
}
