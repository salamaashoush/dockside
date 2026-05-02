use anyhow::Result;
use bollard::query_parameters::{
  PruneBuildOptionsBuilder, PruneContainersOptions, PruneImagesOptions, PruneNetworksOptions, PruneVolumesOptions,
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
  pub build_cache_deleted: Vec<String>,
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
      + self.build_cache_deleted.len()
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

  /// Prune the `BuildKit` build cache. `all = true` removes every cache
  /// entry; `all = false` removes only unused entries (Docker's default).
  pub async fn prune_build_cache(&self, all: bool) -> Result<PruneResult> {
    let docker = self.client()?;

    let options = PruneBuildOptionsBuilder::default().all(all).build();
    let response = docker.prune_build(Some(options)).await?;

    Ok(PruneResult {
      build_cache_deleted: response.caches_deleted.unwrap_or_default(),
      space_reclaimed: u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0),
      ..Default::default()
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_prune_result_default() {
    let result = PruneResult::default();
    assert!(result.containers_deleted.is_empty());
    assert!(result.images_deleted.is_empty());
    assert!(result.volumes_deleted.is_empty());
    assert!(result.networks_deleted.is_empty());
    assert!(result.pods_deleted.is_empty());
    assert!(result.deployments_deleted.is_empty());
    assert!(result.services_deleted.is_empty());
    assert_eq!(result.space_reclaimed, 0);
  }

  #[test]
  fn test_prune_result_display_space_reclaimed() {
    let result = PruneResult {
      space_reclaimed: 1024 * 1024 * 500, // 500 MiB
      ..Default::default()
    };
    assert_eq!(result.display_space_reclaimed(), "500.0 MiB");

    let small = PruneResult {
      space_reclaimed: 1024, // 1 KiB
      ..Default::default()
    };
    assert_eq!(small.display_space_reclaimed(), "1.0 KiB");

    let zero = PruneResult::default();
    assert_eq!(zero.display_space_reclaimed(), "0 B");
  }

  #[test]
  fn test_prune_result_total_items_deleted() {
    let result = PruneResult {
      containers_deleted: vec!["c1".to_string(), "c2".to_string()],
      images_deleted: vec!["i1".to_string()],
      volumes_deleted: vec!["v1".to_string(), "v2".to_string(), "v3".to_string()],
      networks_deleted: vec!["n1".to_string()],
      build_cache_deleted: vec![],
      pods_deleted: vec!["p1".to_string()],
      deployments_deleted: vec![],
      services_deleted: vec!["s1".to_string(), "s2".to_string()],
      space_reclaimed: 0,
    };
    assert_eq!(result.total_items_deleted(), 10);
  }

  #[test]
  fn test_prune_result_is_empty() {
    let empty = PruneResult::default();
    assert!(empty.is_empty());

    let with_containers = PruneResult {
      containers_deleted: vec!["c1".to_string()],
      ..Default::default()
    };
    assert!(!with_containers.is_empty());

    let with_images = PruneResult {
      images_deleted: vec!["i1".to_string()],
      ..Default::default()
    };
    assert!(!with_images.is_empty());

    let with_k8s = PruneResult {
      pods_deleted: vec!["pod1".to_string()],
      ..Default::default()
    };
    assert!(!with_k8s.is_empty());
  }
}
