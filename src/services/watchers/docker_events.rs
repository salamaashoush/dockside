#![allow(unknown_lints, clippy::duration_suboptimal_units)]
//! Docker events watcher
//!
//! Watches Docker daemon events via the Events API and emits resource change notifications.
//! This enables real-time UI updates when containers, images, volumes, or networks change.
//!
//! Features:
//! - Exponential backoff on connection failures (1s -> 2s -> 4s -> ... -> 60s max)
//! - Automatic reconnection with health checks
//! - Graceful degradation when Docker is unavailable

use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use bollard::query_parameters::EventsOptions;
use bollard::secret::{EventMessage, EventMessageTypeEnum};
use futures::StreamExt;
use tokio::sync::RwLock;

use super::{WatcherControl, debouncer::ResourceType};
use crate::docker::DockerClient;

/// Backoff configuration for reconnection
struct Backoff {
  current: Duration,
  max: Duration,
}

impl Backoff {
  fn new() -> Self {
    Self {
      current: Duration::from_secs(1),
      max: Duration::from_secs(60), // 1 minute cap on backoff
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

/// Events emitted by the Docker watcher (fields used via Debug for logging)
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum DockerResourceEvent {
  /// A container was created, started, stopped, removed, etc.
  ContainerChanged { id: String, action: String },
  /// An image was pulled, removed, tagged, etc.
  ImageChanged { id: String, action: String },
  /// A volume was created, removed, or mounted
  VolumeChanged { name: String, action: String },
  /// A network was created, removed, connected, or disconnected
  NetworkChanged { id: String, action: String },
}

impl DockerResourceEvent {
  /// Get the resource type for this event
  pub fn resource_type(&self) -> ResourceType {
    match self {
      Self::ContainerChanged { .. } => ResourceType::Container,
      Self::ImageChanged { .. } => ResourceType::Image,
      Self::VolumeChanged { .. } => ResourceType::Volume,
      Self::NetworkChanged { .. } => ResourceType::Network,
    }
  }
}

/// Docker events watcher
pub struct DockerEventWatcher {
  client: Arc<RwLock<Option<DockerClient>>>,
}

impl DockerEventWatcher {
  pub fn new(client: Arc<RwLock<Option<DockerClient>>>) -> Self {
    Self { client }
  }

  /// Start watching Docker events
  ///
  /// This returns a stream of resource events. The watcher runs until
  /// the control handle is stopped or an error occurs.
  ///
  /// Uses exponential backoff for reconnection attempts.
  #[allow(unused_assignments)]
  pub async fn watch<F>(&self, control: WatcherControl, mut on_event: F)
  where
    F: FnMut(DockerResourceEvent) + Send,
  {
    let mut backoff = Backoff::new();
    let mut consecutive_failures = 0u32;

    while control.is_running() {
      // Try to get the Docker client
      let docker = {
        let guard = self.client.read().await;
        if let Some(d) = guard.as_ref().and_then(|c| c.client().ok()) {
          d.clone()
        } else {
          // No client available, wait with backoff
          let delay = backoff.next_delay();
          tracing::debug!("Docker client not available, retrying in {:?}", delay);
          tokio::time::sleep(delay).await;
          continue;
        }
      };

      // Verify connection with ping before subscribing
      if docker.ping().await.is_err() {
        let delay = backoff.next_delay();
        tracing::debug!("Docker ping failed, retrying in {:?}", delay);
        tokio::time::sleep(delay).await;
        continue;
      }

      // Connection successful, reset backoff
      backoff.reset();
      consecutive_failures = 0;

      // Subscribe to events
      match self.watch_events(&docker, &control, &mut on_event).await {
        Ok(()) => {
          // Stream ended normally (e.g., Docker restarted)
          tracing::debug!("Docker events stream ended, reconnecting...");
        }
        Err(e) => {
          consecutive_failures += 1;
          let delay = backoff.next_delay();
          tracing::warn!(
            "Docker events watcher error (attempt {}): {e}, retrying in {:?}",
            consecutive_failures,
            delay
          );
          tokio::time::sleep(delay).await;
        }
      }
    }
  }

  async fn watch_events<F>(&self, docker: &Docker, control: &WatcherControl, on_event: &mut F) -> anyhow::Result<()>
  where
    F: FnMut(DockerResourceEvent) + Send,
  {
    let options = EventsOptions { ..Default::default() };

    let mut stream = docker.events(Some(options));

    while control.is_running() {
      tokio::select! {
        event = stream.next() => {
          match event {
            Some(Ok(event)) => {
              if let Some(resource_event) = Self::parse_event(&event) {
                on_event(resource_event);
              }
            }
            Some(Err(e)) => {
              return Err(anyhow::anyhow!("Event stream error: {e}"));
            }
            None => {
              // Stream ended, reconnect
              return Ok(());
            }
          }
        }
        () = tokio::time::sleep(Duration::from_secs(30)) => {
          // Periodic check in case stream is stalled
        }
      }
    }

    Ok(())
  }

  fn parse_event(event: &EventMessage) -> Option<DockerResourceEvent> {
    let typ = event.typ.as_ref()?;
    let action = event.action.as_ref()?.clone();
    let actor = event.actor.as_ref()?;
    let id = actor.id.as_ref()?.clone();

    match typ {
      EventMessageTypeEnum::CONTAINER => {
        // Container events: create, start, stop, die, destroy, pause, unpause, etc.
        Some(DockerResourceEvent::ContainerChanged { id, action })
      }
      EventMessageTypeEnum::IMAGE => {
        // Image events: pull, push, tag, untag, delete
        Some(DockerResourceEvent::ImageChanged { id, action })
      }
      EventMessageTypeEnum::VOLUME => {
        // Volume events: create, destroy, mount, unmount
        let name = actor.attributes.as_ref()?.get("name")?.clone();
        Some(DockerResourceEvent::VolumeChanged { name, action })
      }
      EventMessageTypeEnum::NETWORK => {
        // Network events: create, destroy, connect, disconnect
        Some(DockerResourceEvent::NetworkChanged { id, action })
      }
      _ => None,
    }
  }
}
