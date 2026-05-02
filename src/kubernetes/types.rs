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

  pub fn is_running(self) -> bool {
    matches!(self, PodPhase::Running)
  }

  pub fn is_pending(self) -> bool {
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
    let ready = format!("{ready_count}/{total_count}");

    // Calculate total restarts
    let restarts = container_statuses.iter().map(|c| c.restart_count).sum();

    // Calculate age
    let age = creation_timestamp.map_or_else(|| "Unknown".to_string(), format_age);

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
            target_port: p.target_port.as_ref().map_or_else(
              || p.port.to_string(),
              |tp| match tp {
                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => i.to_string(),
                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s.clone(),
              },
            ),
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

    let age = creation_timestamp.map_or_else(|| "Unknown".to_string(), format_age);

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

    let age = creation_timestamp.map_or_else(|| "Unknown".to_string(), format_age);

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

// ============================================================================
// Secret + ConfigMap Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct SecretInfo {
  pub name: String,
  pub namespace: String,
  pub secret_type: String,
  pub keys: Vec<String>,
  pub age: String,
  #[allow(dead_code)]
  pub labels: HashMap<String, String>,
}

impl SecretInfo {
  pub fn from_secret(s: &k8s_openapi::api::core::v1::Secret) -> Self {
    let metadata = &s.metadata;
    let name = metadata.name.clone().unwrap_or_default();
    let namespace = metadata.namespace.clone().unwrap_or_else(|| "default".to_string());
    let labels: HashMap<String, String> = metadata.labels.clone().unwrap_or_default().into_iter().collect();
    let creation_timestamp = metadata.creation_timestamp.as_ref().map(|t| t.0);
    let secret_type = s.type_.clone().unwrap_or_else(|| "Opaque".to_string());
    let mut keys: Vec<String> = s.data.as_ref().map(|d| d.keys().cloned().collect()).unwrap_or_default();
    if let Some(string_data) = s.string_data.as_ref() {
      keys.extend(string_data.keys().cloned());
    }
    keys.sort();
    keys.dedup();
    let age = creation_timestamp.map_or_else(|| "Unknown".to_string(), format_age);
    Self {
      name,
      namespace,
      secret_type,
      keys,
      age,
      labels,
    }
  }
}

#[derive(Debug, Clone)]
pub struct ConfigMapInfo {
  pub name: String,
  pub namespace: String,
  pub keys: Vec<String>,
  pub age: String,
  #[allow(dead_code)]
  pub labels: HashMap<String, String>,
}

impl ConfigMapInfo {
  pub fn from_configmap(cm: &k8s_openapi::api::core::v1::ConfigMap) -> Self {
    let metadata = &cm.metadata;
    let name = metadata.name.clone().unwrap_or_default();
    let namespace = metadata.namespace.clone().unwrap_or_else(|| "default".to_string());
    let labels: HashMap<String, String> = metadata.labels.clone().unwrap_or_default().into_iter().collect();
    let creation_timestamp = metadata.creation_timestamp.as_ref().map(|t| t.0);
    let mut keys: Vec<String> = cm
      .data
      .as_ref()
      .map(|d| d.keys().cloned().collect())
      .unwrap_or_default();
    if let Some(binary) = cm.binary_data.as_ref() {
      keys.extend(binary.keys().cloned());
    }
    keys.sort();
    keys.dedup();
    let age = creation_timestamp.map_or_else(|| "Unknown".to_string(), format_age);
    Self {
      name,
      namespace,
      keys,
      age,
      labels,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_pod_phase_from_str() {
    assert_eq!(PodPhase::from_str("Running"), PodPhase::Running);
    assert_eq!(PodPhase::from_str("Pending"), PodPhase::Pending);
    assert_eq!(PodPhase::from_str("Succeeded"), PodPhase::Succeeded);
    assert_eq!(PodPhase::from_str("Failed"), PodPhase::Failed);
    assert_eq!(PodPhase::from_str("Unknown"), PodPhase::Unknown);
    assert_eq!(PodPhase::from_str("invalid"), PodPhase::Unknown);
  }

  #[test]
  fn test_pod_phase_is_running() {
    assert!(PodPhase::Running.is_running());
    assert!(!PodPhase::Pending.is_running());
    assert!(!PodPhase::Succeeded.is_running());
    assert!(!PodPhase::Failed.is_running());
    assert!(!PodPhase::Unknown.is_running());
  }

  #[test]
  fn test_pod_phase_is_pending() {
    assert!(PodPhase::Pending.is_pending());
    assert!(!PodPhase::Running.is_pending());
    assert!(!PodPhase::Succeeded.is_pending());
    assert!(!PodPhase::Failed.is_pending());
  }

  #[test]
  fn test_pod_phase_display() {
    assert_eq!(format!("{}", PodPhase::Running), "Running");
    assert_eq!(format!("{}", PodPhase::Pending), "Pending");
    assert_eq!(format!("{}", PodPhase::Succeeded), "Succeeded");
    assert_eq!(format!("{}", PodPhase::Failed), "Failed");
    assert_eq!(format!("{}", PodPhase::Unknown), "Unknown");
  }

  #[test]
  fn test_pod_phase_default() {
    assert_eq!(PodPhase::default(), PodPhase::Unknown);
  }

  #[test]
  fn test_pod_container() {
    let container = PodContainer {
      name: "nginx".to_string(),
      image: "nginx:latest".to_string(),
      ready: true,
      restart_count: 0,
    };
    assert_eq!(container.name, "nginx");
    assert_eq!(container.image, "nginx:latest");
    assert!(container.ready);
    assert_eq!(container.restart_count, 0);
  }

  #[test]
  fn test_pod_info() {
    let pod = PodInfo {
      name: "my-pod".to_string(),
      namespace: "default".to_string(),
      phase: PodPhase::Running,
      ready: "1/1".to_string(),
      restarts: 0,
      age: "5m".to_string(),
      node: Some("node-1".to_string()),
      ip: Some("10.0.0.1".to_string()),
      containers: vec![PodContainer {
        name: "nginx".to_string(),
        image: "nginx:latest".to_string(),
        ready: true,
        restart_count: 0,
      }],
      labels: HashMap::from([("app".to_string(), "nginx".to_string())]),
    };
    assert_eq!(pod.name, "my-pod");
    assert_eq!(pod.namespace, "default");
    assert!(pod.phase.is_running());
    assert_eq!(pod.containers.len(), 1);
    assert_eq!(pod.labels.get("app"), Some(&"nginx".to_string()));
  }

  #[test]
  fn test_namespace_info() {
    let ns = NamespaceInfo {
      name: "kube-system".to_string(),
    };
    assert_eq!(ns.name, "kube-system");
  }

  #[test]
  fn test_service_port_info() {
    let port = ServicePortInfo {
      name: Some("http".to_string()),
      protocol: "TCP".to_string(),
      port: 80,
      target_port: "8080".to_string(),
      node_port: Some(30080),
    };
    assert_eq!(port.name, Some("http".to_string()));
    assert_eq!(port.protocol, "TCP");
    assert_eq!(port.port, 80);
    assert_eq!(port.target_port, "8080");
    assert_eq!(port.node_port, Some(30080));
  }

  #[test]
  fn test_service_info_ports_display() {
    let svc = ServiceInfo {
      name: "my-service".to_string(),
      namespace: "default".to_string(),
      service_type: "NodePort".to_string(),
      cluster_ip: Some("10.96.0.1".to_string()),
      external_ips: vec![],
      ports: vec![
        ServicePortInfo {
          name: Some("http".to_string()),
          protocol: "TCP".to_string(),
          port: 80,
          target_port: "8080".to_string(),
          node_port: Some(30080),
        },
        ServicePortInfo {
          name: Some("https".to_string()),
          protocol: "TCP".to_string(),
          port: 443,
          target_port: "8443".to_string(),
          node_port: None,
        },
      ],
      selector: HashMap::from([("app".to_string(), "nginx".to_string())]),
      age: "1h".to_string(),
      labels: HashMap::new(),
    };
    assert_eq!(svc.ports_display(), "80:30080/TCP, 443:8443/TCP");
  }

  #[test]
  fn test_service_info_ports_display_no_node_port() {
    let svc = ServiceInfo {
      name: "cluster-ip-svc".to_string(),
      namespace: "default".to_string(),
      service_type: "ClusterIP".to_string(),
      cluster_ip: Some("10.96.0.2".to_string()),
      external_ips: vec![],
      ports: vec![ServicePortInfo {
        name: None,
        protocol: "TCP".to_string(),
        port: 8080,
        target_port: "http".to_string(),
        node_port: None,
      }],
      selector: HashMap::new(),
      age: "2h".to_string(),
      labels: HashMap::new(),
    };
    assert_eq!(svc.ports_display(), "8080:http/TCP");
  }

  #[test]
  fn test_deployment_info_ready_display() {
    let dep = DeploymentInfo {
      name: "my-deployment".to_string(),
      namespace: "default".to_string(),
      replicas: 3,
      ready_replicas: 2,
      updated_replicas: 3,
      available_replicas: 2,
      age: "1d".to_string(),
      labels: HashMap::new(),
      images: vec!["nginx:latest".to_string()],
    };
    assert_eq!(dep.ready_display(), "2/3");
  }

  #[test]
  fn test_deployment_info_all_ready() {
    let dep = DeploymentInfo {
      name: "ready-deployment".to_string(),
      namespace: "default".to_string(),
      replicas: 5,
      ready_replicas: 5,
      updated_replicas: 5,
      available_replicas: 5,
      age: "2d".to_string(),
      labels: HashMap::from([("app".to_string(), "web".to_string())]),
      images: vec!["app:v1".to_string(), "sidecar:v1".to_string()],
    };
    assert_eq!(dep.ready_display(), "5/5");
    assert_eq!(dep.images.len(), 2);
  }

  // Edge case tests

  #[test]
  fn test_pod_phase_case_sensitive() {
    // Unlike Docker ContainerState, Kubernetes PodPhase is case-sensitive
    // This documents the expected behavior
    assert_eq!(PodPhase::from_str("running"), PodPhase::Unknown); // lowercase
    assert_eq!(PodPhase::from_str("RUNNING"), PodPhase::Unknown); // uppercase
    assert_eq!(PodPhase::from_str("Running"), PodPhase::Running); // correct case
  }

  #[test]
  fn test_service_info_empty_ports() {
    let svc = ServiceInfo {
      name: "headless".to_string(),
      namespace: "default".to_string(),
      service_type: "ClusterIP".to_string(),
      cluster_ip: None, // Headless service
      external_ips: vec![],
      ports: vec![], // No ports defined
      selector: HashMap::new(),
      age: "1h".to_string(),
      labels: HashMap::new(),
    };
    assert_eq!(svc.ports_display(), "");
  }

  #[test]
  fn test_deployment_zero_replicas() {
    // Scaled down deployment
    let dep = DeploymentInfo {
      name: "scaled-down".to_string(),
      namespace: "default".to_string(),
      replicas: 0,
      ready_replicas: 0,
      updated_replicas: 0,
      available_replicas: 0,
      age: "1h".to_string(),
      labels: HashMap::new(),
      images: vec!["app:v1".to_string()],
    };
    assert_eq!(dep.ready_display(), "0/0");
  }

  #[test]
  fn test_pod_with_high_restart_count() {
    // CrashLoopBackOff scenario
    let container = PodContainer {
      name: "crashy".to_string(),
      image: "buggy:latest".to_string(),
      ready: false,
      restart_count: 100,
    };
    assert!(!container.ready);
    assert_eq!(container.restart_count, 100);
  }

  #[test]
  fn test_pod_multiple_containers() {
    // Sidecar pattern: multiple containers in one pod
    let pod = PodInfo {
      name: "multi-container".to_string(),
      namespace: "default".to_string(),
      phase: PodPhase::Running,
      ready: "2/3".to_string(), // One container not ready
      restarts: 5,
      age: "10m".to_string(),
      node: Some("node-1".to_string()),
      ip: Some("10.0.0.5".to_string()),
      containers: vec![
        PodContainer {
          name: "main".to_string(),
          image: "app:v1".to_string(),
          ready: true,
          restart_count: 0,
        },
        PodContainer {
          name: "sidecar".to_string(),
          image: "envoy:v1".to_string(),
          ready: true,
          restart_count: 0,
        },
        PodContainer {
          name: "init-container".to_string(),
          image: "init:v1".to_string(),
          ready: false, // Not ready
          restart_count: 5,
        },
      ],
      labels: HashMap::new(),
    };
    assert_eq!(pod.containers.len(), 3);
    assert_eq!(pod.restarts, 5);
    assert!(pod.phase.is_running());
  }
}
