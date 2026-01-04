use anyhow::Result;
use bollard::query_parameters::{
  PruneContainersOptions, PruneImagesOptions, PruneNetworksOptions, PruneVolumesOptions,
};
use serde::{Deserialize, Serialize};

use super::DockerClient;

/// Result of a prune operation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PruneResult {
  // Docker
  pub containers_deleted: Vec<String>,
  pub images_deleted: Vec<String>,
  pub volumes_deleted: Vec<String>,
  pub networks_deleted: Vec<String>,
  pub space_reclaimed: u64,
  // Kubernetes
  pub pods_deleted: Vec<String>,
  pub deployments_deleted: Vec<String>,
  pub services_deleted: Vec<String>,
}

impl PruneResult {
  pub fn display_space_reclaimed(&self) -> String {
    bytesize::ByteSize(self.space_reclaimed).to_string()
  }

  pub fn total_items_deleted(&self) -> usize {
    self.containers_deleted.len()
      + self.images_deleted.len()
      + self.volumes_deleted.len()
      + self.networks_deleted.len()
      + self.pods_deleted.len()
      + self.deployments_deleted.len()
      + self.services_deleted.len()
  }

  pub fn is_empty(&self) -> bool {
    self.total_items_deleted() == 0
  }
}

impl DockerClient {
  /// Prune stopped containers
  pub async fn prune_containers(&self) -> Result<PruneResult> {
    let docker = self.client()?;

    let options = PruneContainersOptions::default();
    let response = docker.prune_containers(Some(options)).await?;

    Ok(PruneResult {
      containers_deleted: response.containers_deleted.unwrap_or_default(),
      space_reclaimed: u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0),
      ..Default::default()
    })
  }

  /// Prune unused images
  pub async fn prune_images(&self, dangling_only: bool) -> Result<PruneResult> {
    use std::collections::HashMap;
    let docker = self.client()?;

    // Docker API: dangling=true removes only untagged images
    //             dangling=false removes ALL unused images (like docker image prune -a)
    //             No filter defaults to dangling=true
    let mut filters = HashMap::new();
    filters.insert(
      "dangling".to_string(),
      vec![if dangling_only { "true" } else { "false" }.to_string()],
    );

    let options = PruneImagesOptions { filters: Some(filters) };

    let response = docker.prune_images(Some(options)).await?;

    let images_deleted = response
      .images_deleted
      .unwrap_or_default()
      .into_iter()
      .filter_map(|item| item.untagged.or(item.deleted))
      .collect();

    Ok(PruneResult {
      images_deleted,
      space_reclaimed: u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0),
      ..Default::default()
    })
  }

  /// Prune unused volumes
  pub async fn prune_volumes(&self) -> Result<PruneResult> {
    let docker = self.client()?;

    let options = PruneVolumesOptions::default();
    let response = docker.prune_volumes(Some(options)).await?;

    Ok(PruneResult {
      volumes_deleted: response.volumes_deleted.unwrap_or_default(),
      space_reclaimed: u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0),
      ..Default::default()
    })
  }

  /// Prune unused networks
  pub async fn prune_networks(&self) -> Result<PruneResult> {
    let docker = self.client()?;

    let options = PruneNetworksOptions::default();
    let response = docker.prune_networks(Some(options)).await?;

    Ok(PruneResult {
      networks_deleted: response.networks_deleted.unwrap_or_default(),
      ..Default::default()
    })
  }
}
