//! Container watcher that keeps the shared `RouteMap` in sync with the
//! Docker daemon's view of running containers. Subscribes to `/events`
//! filtered for container lifecycle changes and rebuilds the relevant
//! routes on each notification.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bollard::query_parameters::EventsOptions;
use bollard::secret::EventMessageTypeEnum;
use futures::StreamExt;
use tokio::sync::{RwLock, oneshot};

use crate::docker::DockerClient;

use super::route_builder::{RuntimeKind, route_from_container};
use super::route_map::{Route, SharedRouteMap};

/// Shared, mutable Docker client handle as held by the rest of the app.
pub type SharedDockerClient = Arc<RwLock<Option<DockerClient>>>;

/// Owned by the running watcher. Drop to stop it.
pub struct WatcherHandle {
  shutdown: Option<oneshot::Sender<()>>,
}

impl Drop for WatcherHandle {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown.take() {
      let _ = tx.send(());
    }
  }
}

/// Spawn the watcher on the current Tokio runtime. The returned handle
/// owns the task; dropping it sends a shutdown signal.
pub fn spawn(docker: SharedDockerClient, routes: SharedRouteMap) -> WatcherHandle {
  let (tx, mut rx) = oneshot::channel::<()>();
  tokio::spawn(async move {
    let mut backoff = Duration::from_secs(1);
    let mut runtime: Option<RuntimeKind> = None;

    loop {
      tokio::select! {
        biased;
        _ = &mut rx => return,
        () = async {} => {}
      }

      // Get a usable bollard handle, retrying with backoff if not connected.
      let bollard = {
        let guard = docker.read().await;
        match guard.as_ref() {
          Some(c) => c.client().ok().cloned(),
          None => None,
        }
      };
      let Some(bollard) = bollard else {
        tokio::select! {
          () = tokio::time::sleep(backoff) => {}
          _ = &mut rx => return,
        }
        backoff = (backoff * 2).min(Duration::from_secs(30));
        continue;
      };

      // First-time runtime detection.
      if runtime.is_none() {
        let info_result = {
          let guard = docker.read().await;
          if let Some(c) = guard.as_ref() {
            c.get_system_info().await.ok()
          } else {
            None
          }
        };
        runtime = Some(info_result.map_or(RuntimeKind::Unknown, |info| RuntimeKind::detect(&info.os, &info.kernel)));
      }
      let kind = runtime.unwrap_or(RuntimeKind::Unknown);

      // Initial seed.
      if let Err(e) = reseed(&docker, &routes, kind).await {
        tracing::warn!("dns watcher: initial seed failed: {e}");
      }

      let opts = EventsOptions {
        filters: Some(std::collections::HashMap::from([(
          "type".to_string(),
          vec!["container".to_string()],
        )])),
        ..Default::default()
      };
      let mut events = bollard.events(Some(opts));
      backoff = Duration::from_secs(1);

      loop {
        tokio::select! {
          _ = &mut rx => return,
          maybe = events.next() => {
            let Some(item) = maybe else { break };
            match item {
              Ok(msg) => {
                if !matches!(msg.typ, Some(EventMessageTypeEnum::CONTAINER)) {
                  continue;
                }
                let action = msg.action.unwrap_or_default();
                let id = msg.actor.and_then(|a| a.id).unwrap_or_default();
                if id.is_empty() {
                  if let Err(e) = reseed(&docker, &routes, kind).await {
                    tracing::warn!("dns watcher: reseed after event failed: {e}");
                  }
                  continue;
                }
                match action.as_str() {
                  "destroy" | "die" | "stop" | "kill" => {
                    routes.write().remove_by_id(&id);
                  }
                  _ => {
                    if let Some(route) = lookup_route_by_id(&docker, &id, kind).await {
                      routes.write().upsert(&route);
                    } else {
                      // Fallback: full reseed if we couldn't infer the route.
                      if let Err(e) = reseed(&docker, &routes, kind).await {
                        tracing::warn!("dns watcher: reseed after event failed: {e}");
                      }
                    }
                  }
                }
              }
              Err(e) => {
                tracing::debug!("dns watcher: event stream error ({e}); reconnecting");
                break;
              }
            }
          }
        }
      }

      tokio::select! {
        () = tokio::time::sleep(backoff) => {}
        _ = &mut rx => return,
      }
      backoff = (backoff * 2).min(Duration::from_secs(30));
    }
  });

  WatcherHandle { shutdown: Some(tx) }
}

/// Look up a single container by id and build a `Route` for it. Returns
/// `None` if the container is gone or has no resolvable backend.
async fn lookup_route_by_id(docker: &SharedDockerClient, id: &str, runtime: RuntimeKind) -> Option<Route> {
  let containers = {
    let guard = docker.read().await;
    let client = guard.as_ref()?;
    client.list_containers(true).await.ok()?
  };
  let target = containers.into_iter().find(|c| c.id == id)?;
  route_from_container(&target, runtime)
}

async fn reseed(docker: &SharedDockerClient, routes: &SharedRouteMap, runtime: RuntimeKind) -> Result<()> {
  let containers = {
    let guard = docker.read().await;
    let Some(client) = guard.as_ref() else {
      anyhow::bail!("docker client not connected");
    };
    client.list_containers(true).await?
  };
  let new_routes: Vec<_> = containers
    .iter()
    .filter_map(|c| route_from_container(c, runtime))
    .collect();
  routes.write().replace(&new_routes);
  Ok(())
}
