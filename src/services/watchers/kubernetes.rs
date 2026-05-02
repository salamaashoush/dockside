#![allow(unknown_lints, clippy::duration_suboptimal_units)]
//! Kubernetes resource watchers
//!
//! Watches Kubernetes resources using the Watch API for real-time updates.
//! Uses a generic watcher implementation to avoid code duplication.
//!
//! Features:
//! - Exponential backoff on connection failures
//! - Generic implementation for all K8s resource types
//! - Automatic reconnection with cluster availability checks

use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use k8s_openapi::NamespaceResourceScope;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::runtime::watcher::{self, Event as WatchEvent};
use kube::{Api, Client, Resource};

use super::WatcherControl;
use super::debouncer::ResourceType;

/// Backoff configuration for reconnection
struct Backoff {
  current: Duration,
  max: Duration,
}

impl Backoff {
  fn new() -> Self {
    Self {
      current: Duration::from_secs(1),
      max: Duration::from_secs(60),
    }
  }

  fn next_delay(&mut self) -> Duration {
    let delay = self.current;
    self.current = (self.current * 2).min(self.max);
    delay
  }

  fn reset(&mut self) {
    self.current = Duration::from_secs(1);
  }
}

/// Kubernetes resource watcher using generics to avoid duplication
pub struct KubernetesWatcher {
  client: Option<Client>,
}

impl KubernetesWatcher {
  pub async fn new() -> Self {
    let client = Client::try_default().await.ok();
    Self { client }
  }

  pub fn is_available(&self) -> bool {
    self.client.is_some()
  }

  /// Watch all Kubernetes resources and emit resource type on changes
  pub async fn watch_all<F>(&self, control: WatcherControl, mut on_change: F)
  where
    F: FnMut(ResourceType) + Send,
  {
    let Some(client) = &self.client else {
      return;
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ResourceType>(256);

    // Spawn generic watchers for each resource type
    let handles = [
      Self::spawn_watcher::<Pod>(client.clone(), control.clone(), tx.clone(), ResourceType::Pod),
      Self::spawn_watcher::<Deployment>(client.clone(), control.clone(), tx.clone(), ResourceType::Deployment),
      Self::spawn_watcher::<Service>(client.clone(), control, tx, ResourceType::Service),
    ];

    // Forward events to callback
    while let Some(resource_type) = rx.recv().await {
      on_change(resource_type);
    }

    // Clean up
    for handle in handles {
      handle.abort();
    }
  }

  /// Spawn a generic watcher for any Kubernetes resource type
  fn spawn_watcher<K>(
    client: Client,
    control: WatcherControl,
    tx: tokio::sync::mpsc::Sender<ResourceType>,
    resource_type: ResourceType,
  ) -> tokio::task::JoinHandle<()>
  where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
      + Clone
      + std::fmt::Debug
      + Send
      + Sync
      + serde::de::DeserializeOwned
      + 'static,
    K::DynamicType: Default,
  {
    tokio::spawn(async move {
      Self::watch_resource::<K>(client, control, tx, resource_type).await;
    })
  }

  /// Generic resource watcher - handles any Kubernetes resource type
  /// Uses exponential backoff for reconnection
  async fn watch_resource<K>(
    client: Client,
    control: WatcherControl,
    tx: tokio::sync::mpsc::Sender<ResourceType>,
    resource_type: ResourceType,
  ) where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
      + Clone
      + std::fmt::Debug
      + Send
      + Sync
      + serde::de::DeserializeOwned
      + 'static,
    K::DynamicType: Default,
  {
    let api: Api<K> = Api::all(client);
    let type_name = std::any::type_name::<K>().split("::").last().unwrap_or("Unknown");
    let mut backoff = Backoff::new();
    let mut consecutive_failures = 0u32;

    while control.is_running() {
      match Self::run_watcher(&api, &control, &tx, resource_type).await {
        Ok(()) => {
          // Stream ended normally, reset backoff and reconnect
          backoff.reset();
          consecutive_failures = 0;
          tracing::debug!("{type_name} watcher stream ended, reconnecting...");
        }
        Err(e) => {
          consecutive_failures += 1;
          let delay = backoff.next_delay();
          tracing::warn!("{type_name} watcher error (attempt {consecutive_failures}): {e}, retrying in {delay:?}");
          tokio::time::sleep(delay).await;
        }
      }
    }
  }

  /// Run the actual watch loop for a resource
  async fn run_watcher<K>(
    api: &Api<K>,
    control: &WatcherControl,
    tx: &tokio::sync::mpsc::Sender<ResourceType>,
    resource_type: ResourceType,
  ) -> Result<()>
  where
    K: Resource<DynamicType = ()> + Clone + std::fmt::Debug + Send + Sync + serde::de::DeserializeOwned + 'static,
    K::DynamicType: Default,
  {
    let watcher = watcher::watcher(api.clone(), watcher::Config::default());
    futures::pin_mut!(watcher);

    while control.is_running() {
      tokio::select! {
        event = watcher.next() => {
          match event {
            Some(Ok(watch_event)) => {
              if Self::is_meaningful_event(&watch_event) {
                let _ = tx.try_send(resource_type);
              }
            }
            Some(Err(e)) => {
              return Err(anyhow::anyhow!("Watch error: {e}"));
            }
            None => {
              return Ok(());
            }
          }
        }
        () = tokio::time::sleep(Duration::from_secs(60)) => {
          // Periodic keepalive
        }
      }
    }

    Ok(())
  }

  /// Check if an event is meaningful (skip init events)
  fn is_meaningful_event<K>(event: &WatchEvent<K>) -> bool
  where
    K: Resource,
  {
    matches!(event, WatchEvent::Apply(_) | WatchEvent::Delete(_))
  }
}
