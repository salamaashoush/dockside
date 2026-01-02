use anyhow::Result;
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{
    CreateContainerOptions, LogsOptions, RemoveContainerOptions, ListVolumesOptions,
    RemoveVolumeOptions,
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DockerClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: Option<DateTime<Utc>>,
    pub labels: HashMap<String, String>,
    pub scope: String,
    pub status: Option<HashMap<String, String>>,
    pub usage_data: Option<VolumeUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUsage {
    pub size: i64,
    pub ref_count: i64,
}

impl VolumeInfo {
    pub fn display_size(&self) -> String {
        self.usage_data
            .as_ref()
            .map(|u| bytesize::ByteSize(u.size as u64).to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn is_in_use(&self) -> bool {
        self.usage_data
            .as_ref()
            .map(|u| u.ref_count > 0)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub permissions: String,
}

impl VolumeFileEntry {
    pub fn display_size(&self) -> String {
        if self.is_dir {
            "-".to_string()
        } else {
            bytesize::ByteSize(self.size).to_string()
        }
    }
}

impl DockerClient {
    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        let docker = self.client()?;

        let options = ListVolumesOptions {
            ..Default::default()
        };

        let response = docker.list_volumes(Some(options)).await?;

        let volumes = response.volumes.unwrap_or_default();
        let mut result = Vec::new();

        for volume in volumes {
            let created = volume
                .created_at
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let usage_data = volume.usage_data.map(|u| VolumeUsage {
                size: u.size,
                ref_count: u.ref_count,
            });

            result.push(VolumeInfo {
                name: volume.name,
                driver: volume.driver,
                mountpoint: volume.mountpoint,
                created,
                labels: volume.labels,
                scope: volume.scope.map(|s| format!("{:?}", s)).unwrap_or_default(),
                status: None,
                usage_data,
            });
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    pub async fn remove_volume(&self, name: &str, force: bool) -> Result<()> {
        let docker = self.client()?;
        docker
            .remove_volume(name, Some(RemoveVolumeOptions { force }))
            .await?;
        Ok(())
    }

    pub async fn create_volume(&self, name: &str) -> Result<VolumeInfo> {
        self.create_volume_with_opts(name, "local", vec![]).await
    }

    pub async fn create_volume_with_opts(
        &self,
        name: &str,
        driver: &str,
        labels: Vec<(String, String)>,
    ) -> Result<VolumeInfo> {
        let docker = self.client()?;

        let labels_map: HashMap<String, String> = labels.into_iter().collect();

        let config = bollard::volume::CreateVolumeOptions {
            name: name.to_string(),
            driver: driver.to_string(),
            labels: labels_map,
            ..Default::default()
        };

        let volume = docker.create_volume(config).await?;

        let created = volume
            .created_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(VolumeInfo {
            name: volume.name,
            driver: volume.driver,
            mountpoint: volume.mountpoint,
            created,
            labels: volume.labels,
            scope: volume.scope.map(|s| format!("{:?}", s)).unwrap_or_default(),
            status: None,
            usage_data: None,
        })
    }

    /// List files in a volume by running a temporary container with ls command
    /// Docker API doesn't expose direct volume file browsing, so we create a
    /// lightweight container that runs ls and exits immediately.
    pub async fn list_volume_files(&self, volume_name: &str, path: &str) -> Result<Vec<VolumeFileEntry>> {
        let docker = self.client()?;

        // Normalize path - volume is mounted at /data
        let normalized_path = if path.is_empty() || path == "/" {
            "/data".to_string()
        } else {
            format!("/data{}", path.trim_end_matches('/'))
        };

        // Create a temporary container that runs ls command and exits
        // Use timestamp for unique name
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let container_name = format!("docker-ui-vol-{}", timestamp);

        let host_config = bollard::models::HostConfig {
            binds: Some(vec![format!("{}:/data:ro", volume_name)]),
            ..Default::default()
        };

        let config = ContainerCreateBody {
            image: Some("alpine:latest".to_string()),
            cmd: Some(vec!["ls".to_string(), "-la".to_string(), normalized_path.clone()]),
            host_config: Some(host_config),
            tty: Some(false),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: Some(container_name.clone()),
            ..Default::default()
        };

        // Create and start container (it will run ls and exit immediately)
        docker.create_container(Some(options), config).await?;
        docker
            .start_container(&container_name, None::<bollard::query_parameters::StartContainerOptions>)
            .await?;

        // Wait for container to finish
        let mut wait_stream = docker.wait_container(
            &container_name,
            None::<bollard::query_parameters::WaitContainerOptions>,
        );
        while let Some(_) = wait_stream.next().await {}

        // Get the logs (output of ls command)
        let log_options = LogsOptions {
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut logs_stream = docker.logs(&container_name, Some(log_options));
        let mut output = String::new();
        while let Some(chunk) = logs_stream.next().await {
            if let Ok(log) = chunk {
                output.push_str(&log.to_string());
            }
        }

        // Remove the container
        let _ = docker.remove_container(
            &container_name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;

        // Parse the output
        let base_path = if path.is_empty() || path == "/" {
            "/".to_string()
        } else {
            path.trim_end_matches('/').to_string()
        };

        let mut entries = Vec::new();
        for line in output.lines().skip(1) {
            // Skip "total" line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                let permissions = parts[0].to_string();
                let size: u64 = parts[4].parse().unwrap_or(0);
                let name = parts[8..].join(" ");

                if name == "." || name == ".." {
                    continue;
                }

                let is_dir = permissions.starts_with('d');
                let is_symlink = permissions.starts_with('l');

                let file_path = if base_path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", base_path, name)
                };

                entries.push(VolumeFileEntry {
                    name,
                    path: file_path,
                    is_dir,
                    is_symlink,
                    size,
                    permissions,
                });
            }
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
