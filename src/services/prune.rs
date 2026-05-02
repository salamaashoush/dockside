//! Docker prune operations

use gpui::{App, AppContext, Entity};

use crate::docker::PruneResult;
use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::ui::PruneDialog;

use super::core::{DispatcherEvent, dispatcher, docker_client};
use super::docker::{refresh_containers, refresh_images, refresh_networks, refresh_volumes};
use super::kubernetes::{refresh_deployments, refresh_pods, refresh_services};

pub fn prune_docker(options: &crate::ui::PruneOptions, cx: &mut App) -> Entity<PruneDialog> {
  let task_id = start_task(cx, "Pruning Docker resources...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  // Create a PruneDialog entity to track results
  let prune_dialog = cx.new(|cx| {
    let mut dialog = PruneDialog::new(cx);
    dialog.set_loading(true);
    dialog
  });
  let prune_dialog_clone = prune_dialog.clone();

  let prune_containers = options.prune_containers;
  let prune_images = options.prune_images;
  let prune_volumes = options.prune_volumes;
  let prune_networks = options.prune_networks;
  let prune_build_cache = options.prune_build_cache;
  let build_cache_all = options.build_cache_all;
  let images_dangling_only = options.images_dangling_only;
  let prune_k8s_pods = options.prune_k8s_pods;
  let prune_k8s_pods_all = options.prune_k8s_pods_all;
  let prune_k8s_deployments = options.prune_k8s_deployments;
  let prune_k8s_services = options.prune_k8s_services;

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

    let mut result = PruneResult::default();

    if prune_containers && let Ok(r) = docker.prune_containers().await {
      result.containers_deleted = r.containers_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_images && let Ok(r) = docker.prune_images(images_dangling_only).await {
      result.images_deleted = r.images_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_volumes && let Ok(r) = docker.prune_volumes().await {
      result.volumes_deleted = r.volumes_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_networks && let Ok(r) = docker.prune_networks().await {
      result.networks_deleted = r.networks_deleted;
    }

    if prune_build_cache && let Ok(r) = docker.prune_build_cache(build_cache_all).await {
      result.build_cache_deleted = r.build_cache_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    // Kubernetes pruning
    let needs_k8s = prune_k8s_pods || prune_k8s_deployments || prune_k8s_services;
    if needs_k8s && let Ok(kube_client) = crate::kubernetes::KubeClient::new().await {
      // Helper to check if namespace is a system namespace
      let is_system_namespace = |ns: &str| matches!(ns, "kube-system" | "kube-public" | "kube-node-lease");

      // Helper to check if resource is a system resource
      let is_system_resource =
        |ns: &str, name: &str| is_system_namespace(ns) || (ns == "default" && name == "kubernetes");

      // Prune deployments first (this will cascade delete their pods)
      if prune_k8s_deployments && let Ok(deployments) = kube_client.list_deployments(None).await {
        for deployment in deployments {
          // Skip system namespaces
          if is_system_namespace(&deployment.namespace) {
            continue;
          }
          if kube_client
            .delete_deployment(&deployment.name, &deployment.namespace)
            .await
            .is_ok()
          {
            result
              .deployments_deleted
              .push(format!("{}/{}", deployment.namespace, deployment.name));
          }
        }
      }

      // Prune services
      if prune_k8s_services && let Ok(services) = kube_client.list_services(None).await {
        for service in services {
          // Skip system namespaces and default kubernetes service
          if is_system_resource(&service.namespace, &service.name) {
            continue;
          }
          if kube_client
            .delete_service(&service.name, &service.namespace)
            .await
            .is_ok()
          {
            result
              .services_deleted
              .push(format!("{}/{}", service.namespace, service.name));
          }
        }
      }

      // Prune pods (only orphans if deployments were pruned, or based on status)
      if prune_k8s_pods && let Ok(pods) = kube_client.list_pods(None).await {
        for pod in pods {
          // Skip system namespaces
          if is_system_namespace(&pod.namespace) {
            continue;
          }

          // If prune_k8s_pods_all is true, delete all pods
          // Otherwise, only delete completed/failed pods
          let should_delete = if prune_k8s_pods_all {
            true
          } else {
            matches!(
              pod.phase,
              crate::kubernetes::PodPhase::Succeeded | crate::kubernetes::PodPhase::Failed
            )
          };

          if should_delete && kube_client.delete_pod(&pod.name, &pod.namespace).await.is_ok() {
            result.pods_deleted.push(format!("{}/{}", pod.namespace, pod.name));
          }
        }
      }
    }

    Ok::<_, anyhow::Error>(result)
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      match result {
        Ok(Ok(prune_result)) => {
          complete_task(cx, task_id);

          let mut parts = Vec::new();
          if !prune_result.containers_deleted.is_empty() {
            parts.push(format!("{} containers", prune_result.containers_deleted.len()));
          }
          if !prune_result.images_deleted.is_empty() {
            parts.push(format!("{} images", prune_result.images_deleted.len()));
          }
          if !prune_result.volumes_deleted.is_empty() {
            parts.push(format!("{} volumes", prune_result.volumes_deleted.len()));
          }
          if !prune_result.networks_deleted.is_empty() {
            parts.push(format!("{} networks", prune_result.networks_deleted.len()));
          }
          if !prune_result.build_cache_deleted.is_empty() {
            parts.push(format!(
              "{} build cache entries",
              prune_result.build_cache_deleted.len()
            ));
          }
          if !prune_result.pods_deleted.is_empty() {
            parts.push(format!("{} pods", prune_result.pods_deleted.len()));
          }
          if !prune_result.deployments_deleted.is_empty() {
            parts.push(format!("{} deployments", prune_result.deployments_deleted.len()));
          }
          if !prune_result.services_deleted.is_empty() {
            parts.push(format!("{} services", prune_result.services_deleted.len()));
          }

          let message = if parts.is_empty() {
            "Nothing to prune".to_string()
          } else {
            format!(
              "Pruned: {}. Space reclaimed: {}",
              parts.join(", "),
              prune_result.display_space_reclaimed()
            )
          };

          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_result(prune_result);
          });

          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskCompleted { message });
          });

          // Refresh all lists
          refresh_containers(cx);
          refresh_images(cx);
          refresh_volumes(cx);
          refresh_networks(cx);
          refresh_pods(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }
        Ok(Err(e)) => {
          fail_task(cx, task_id, e.to_string());
          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_error(e.to_string());
          });
          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskFailed {
              error: format!("Failed to prune: {e}"),
            });
          });
        }
        Err(join_err) => {
          fail_task(cx, task_id, join_err.to_string());
          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_error(join_err.to_string());
          });
          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskFailed {
              error: format!("Task failed: {join_err}"),
            });
          });
        }
      }
    })
  })
  .detach();

  prune_dialog
}
