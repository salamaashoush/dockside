use std::collections::HashMap;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};

/// Pod status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PodPhase {
  Running,
  Pending,
  Succeeded,
  Failed,
  #[default]
  Unknown,
}

impl PodPhase {
  pub fn from_str(s: &str) -> Self {
    match s {
      "Running" => PodPhase::Running,
      "Pending" => PodPhase::Pending,
      "Succeeded" => PodPhase::Succeeded,
      "Failed" => PodPhase::Failed,
      _ => PodPhase::Unknown,
    }
  }

  pub fn is_running(&self) -> bool {
    matches!(self, PodPhase::Running)
  }

  pub fn is_pending(&self) -> bool {
    matches!(self, PodPhase::Pending)
  }
}

impl std::fmt::Display for PodPhase {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PodPhase::Running => write!(f, "Running"),
      PodPhase::Pending => write!(f, "Pending"),
      PodPhase::Succeeded => write!(f, "Succeeded"),
      PodPhase::Failed => write!(f, "Failed"),
      PodPhase::Unknown => write!(f, "Unknown"),
    }
  }
}

/// Container info within a pod
#[derive(Debug, Clone)]
pub struct PodContainer {
  pub name: String,
  pub image: String,
  pub ready: bool,
  pub restart_count: i32,
}

impl PodContainer {
  pub fn from_status(status: &ContainerStatus) -> Self {
    Self {
      name: status.name.clone(),
      image: status.image.clone(),
      ready: status.ready,
      restart_count: status.restart_count,
    }
  }
}

/// Pod information
#[derive(Debug, Clone)]
pub struct PodInfo {
  pub name: String,
  pub namespace: String,
  pub phase: PodPhase,
  pub ready: String,
  pub restarts: i32,
  pub age: String,
  pub node: Option<String>,
  pub ip: Option<String>,
  pub containers: Vec<PodContainer>,
  pub labels: HashMap<String, String>,
}

impl PodInfo {
  pub fn from_pod(pod: &Pod) -> Self {
    let metadata = &pod.metadata;
    let spec = pod.spec.as_ref();
    let status = pod.status.as_ref();

    let name = metadata.name.clone().unwrap_or_default();
    let namespace = metadata.namespace.clone().unwrap_or_else(|| "default".to_string());
    let labels: HashMap<String, String> = metadata.labels.clone().unwrap_or_default().into_iter().collect();
    let creation_timestamp = metadata.creation_timestamp.as_ref().map(|t| t.0);

    let phase = status
      .and_then(|s| s.phase.as_ref())
      .map(|p| PodPhase::from_str(p))
      .unwrap_or_default();

    let node = spec.and_then(|s| s.node_name.clone());
    let ip = status.and_then(|s| s.pod_ip.clone());

    // Get container statuses
    let container_statuses: Vec<PodContainer> = status
      .and_then(|s| s.container_statuses.as_ref())
      .map(|cs| cs.iter().map(PodContainer::from_status).collect())
      .unwrap_or_default();

    // Calculate ready string (e.g., "1/2")
    let ready_count = container_statuses.iter().filter(|c| c.ready).count();
    let total_count = container_statuses.len();
    let ready = format!("{}/{}", ready_count, total_count);

    // Calculate total restarts
    let restarts = container_statuses.iter().map(|c| c.restart_count).sum();

    // Calculate age
    let age = creation_timestamp
      .map(format_age)
      .unwrap_or_else(|| "Unknown".to_string());

    Self {
      name,
      namespace,
      phase,
      ready,
      restarts,
      age,
      node,
      ip,
      containers: container_statuses,
      labels,
    }
  }
}

/// Format age as human-readable string
fn format_age(creation: DateTime<Utc>) -> String {
  let now = Utc::now();
  let duration = now.signed_duration_since(creation);

  if duration.num_days() > 0 {
    format!("{}d", duration.num_days())
  } else if duration.num_hours() > 0 {
    format!("{}h", duration.num_hours())
  } else if duration.num_minutes() > 0 {
    format!("{}m", duration.num_minutes())
  } else {
    format!("{}s", duration.num_seconds().max(0))
  }
}

/// Namespace info
#[derive(Debug, Clone)]
pub struct NamespaceInfo {
  pub name: String,
}

// ============================================================================
// Service Types
// ============================================================================

/// Service port information
#[derive(Debug, Clone)]
pub struct ServicePortInfo {
  pub name: Option<String>,
  pub protocol: String,
  pub port: i32,
  pub target_port: String,
  pub node_port: Option<i32>,
}

/// Service information
#[derive(Debug, Clone)]
pub struct ServiceInfo {
  pub name: String,
  pub namespace: String,
  pub service_type: String,
  pub cluster_ip: Option<String>,
  pub external_ips: Vec<String>,
  pub ports: Vec<ServicePortInfo>,
  pub selector: HashMap<String, String>,
  pub age: String,
  pub labels: HashMap<String, String>,
}

impl ServiceInfo {
  pub fn from_service(svc: &k8s_openapi::api::core::v1::Service) -> Self {
    let metadata = &svc.metadata;
    let spec = svc.spec.as_ref();

    let name = metadata.name.clone().unwrap_or_default();
    let namespace = metadata.namespace.clone().unwrap_or_else(|| "default".to_string());
    let labels: HashMap<String, String> = metadata.labels.clone().unwrap_or_default().into_iter().collect();
    let creation_timestamp = metadata.creation_timestamp.as_ref().map(|t| t.0);

    let service_type = spec
      .and_then(|s| s.type_.clone())
      .unwrap_or_else(|| "ClusterIP".to_string());

    let cluster_ip = spec.and_then(|s| s.cluster_ip.clone());

    let external_ips = spec.and_then(|s| s.external_ips.clone()).unwrap_or_default();

    let ports: Vec<ServicePortInfo> = spec
      .and_then(|s| s.ports.as_ref())
      .map(|ports| {
        ports
          .iter()
          .map(|p| ServicePortInfo {
            name: p.name.clone(),
            protocol: p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
            port: p.port,
            target_port: p
              .target_port
              .as_ref()
              .map(|tp| match tp {
                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => i.to_string(),
                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s.clone(),
              })
              .unwrap_or_else(|| p.port.to_string()),
            node_port: p.node_port,
          })
          .collect()
      })
      .unwrap_or_default();

    let selector: HashMap<String, String> = spec
      .and_then(|s| s.selector.clone())
      .unwrap_or_default()
      .into_iter()
      .collect();

    let age = creation_timestamp
      .map(format_age)
      .unwrap_or_else(|| "Unknown".to_string());

    Self {
      name,
      namespace,
      service_type,
      cluster_ip,
      external_ips,
      ports,
      selector,
      age,
      labels,
    }
  }

  /// Format ports for display (e.g., "80:8080/TCP, 443:8443/TCP")
  pub fn ports_display(&self) -> String {
    self
      .ports
      .iter()
      .map(|p| {
        if let Some(np) = p.node_port {
          format!("{}:{}/{}", p.port, np, p.protocol)
        } else {
          format!("{}:{}/{}", p.port, p.target_port, p.protocol)
        }
      })
      .collect::<Vec<_>>()
      .join(", ")
  }
}

// ============================================================================
// Deployment Types
// ============================================================================

/// Deployment information
#[derive(Debug, Clone)]
pub struct DeploymentInfo {
  pub name: String,
  pub namespace: String,
  pub replicas: i32,
  pub ready_replicas: i32,
  pub updated_replicas: i32,
  pub available_replicas: i32,
  pub age: String,
  pub labels: HashMap<String, String>,
  pub images: Vec<String>,
}

impl DeploymentInfo {
  pub fn from_deployment(dep: &k8s_openapi::api::apps::v1::Deployment) -> Self {
    let metadata = &dep.metadata;
    let spec = dep.spec.as_ref();
    let status = dep.status.as_ref();

    let name = metadata.name.clone().unwrap_or_default();
    let namespace = metadata.namespace.clone().unwrap_or_else(|| "default".to_string());
    let labels: HashMap<String, String> = metadata.labels.clone().unwrap_or_default().into_iter().collect();
    let creation_timestamp = metadata.creation_timestamp.as_ref().map(|t| t.0);

    let replicas = spec.and_then(|s| s.replicas).unwrap_or(0);
    let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);
    let updated_replicas = status.and_then(|s| s.updated_replicas).unwrap_or(0);
    let available_replicas = status.and_then(|s| s.available_replicas).unwrap_or(0);

    // Extract container images from pod template
    let images: Vec<String> = spec
      .and_then(|s| s.template.spec.as_ref())
      .map(|pod_spec| pod_spec.containers.iter().filter_map(|c| c.image.clone()).collect())
      .unwrap_or_default();

    let age = creation_timestamp
      .map(format_age)
      .unwrap_or_else(|| "Unknown".to_string());

    Self {
      name,
      namespace,
      replicas,
      ready_replicas,
      updated_replicas,
      available_replicas,
      age,
      labels,
      images,
    }
  }

  /// Get ready status string (e.g., "2/3")
  pub fn ready_display(&self) -> String {
    format!("{}/{}", self.ready_replicas, self.replicas)
  }
}
