use anyhow::{Context, Result};
use chrono::Utc;
use k8s_openapi::api::apps::v1::{Deployment, ReplicaSet};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{
  Api, Client, Config,
  api::{DeleteParams, ListParams, LogParams, Patch, PatchParams, PostParams},
  config::{KubeConfigOptions, Kubeconfig},
};
use serde_json::json;
use std::fmt::Write as _;
use std::time::Duration;

use k8s_openapi::api::core::v1::Service;

use super::types::{DeploymentInfo, NamespaceInfo, PodInfo, ServiceInfo};

/// Kubernetes client wrapper
pub struct KubeClient {
  client: Client,
}

impl KubeClient {
  /// Create a new `KubeClient` from default kubeconfig
  /// Includes VPN-aware fallback: if the server URL uses a VM IP that's unreachable,
  /// automatically tries localhost with the same port
  pub async fn new() -> Result<Self> {
    // First, try to load the config to inspect the server URL.
    // Honor the user's `kubeconfig_path` setting if set; otherwise
    // fall back to the standard `Config::infer` discovery.
    let custom_path = crate::state::AppSettings::load().kubeconfig_path;
    let config = if custom_path.is_empty() {
      Config::infer().await.context("Failed to load kubeconfig")?
    } else {
      let kubeconfig = Kubeconfig::read_from(&custom_path)
        .with_context(|| format!("Failed to read kubeconfig at {custom_path}"))?;
      Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())
        .await
        .context("Failed to build kube config from custom path")?
    };

    // Check if the server URL uses a non-localhost IP (likely VM IP)
    let server_url = config.cluster_url.to_string();
    let is_vm_ip = is_non_localhost_ip(&server_url);

    // Try connecting with a short timeout first
    let client_result = Self::try_connect_with_timeout(config.clone(), Duration::from_secs(5)).await;

    match client_result {
      Ok(client) => Ok(Self { client }),
      Err(e) if is_vm_ip => {
        // Connection failed and we're using a VM IP - try localhost fallback
        tracing::warn!(
          "Failed to connect to K8s at {} (possibly VPN blocking VM IP), trying localhost fallback: {}",
          server_url,
          e
        );

        if let Some(localhost_config) = try_localhost_fallback(&server_url, config) {
          let client =
            Client::try_from(localhost_config).context("Failed to create K8s client with localhost fallback")?;

          // Test the connection
          Self::test_connection(&client).await?;

          tracing::info!("Successfully connected to K8s via localhost fallback");
          Ok(Self { client })
        } else {
          Err(e).context("Failed to connect to Kubernetes. VM IP unreachable (VPN may be blocking local network)")
        }
      }
      Err(e) => Err(e).context("Failed to create Kubernetes client"),
    }
  }

  /// Try to connect with a timeout
  async fn try_connect_with_timeout(config: Config, timeout: Duration) -> Result<Client> {
    let client = Client::try_from(config)?;

    // Test the connection with timeout
    tokio::time::timeout(timeout, Self::test_connection(&client))
      .await
      .map_err(|_| anyhow::anyhow!("Connection timeout"))?
      .map(|()| client)
  }

  /// Test if we can actually reach the K8s API
  async fn test_connection(client: &Client) -> Result<()> {
    // Try to get API versions - lightweight call to verify connectivity
    let _ = client
      .apiserver_version()
      .await
      .context("Cannot reach K8s API server")?;
    Ok(())
  }

  /// List all namespaces
  pub async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>> {
    let api: Api<Namespace> = Api::all(self.client.clone());
    let namespaces = api
      .list(&ListParams::default())
      .await
      .context("Failed to list namespaces")?;

    let ns_list = namespaces
      .items
      .iter()
      .map(|ns| {
        let name = ns.metadata.name.clone().unwrap_or_default();
        NamespaceInfo { name }
      })
      .collect();

    Ok(ns_list)
  }

  /// List pods in a namespace (or all namespaces if None)
  pub async fn list_pods(&self, namespace: Option<&str>) -> Result<Vec<PodInfo>> {
    let pods = if let Some(ns) = namespace {
      let api: Api<Pod> = Api::namespaced(self.client.clone(), ns);
      api
        .list(&ListParams::default())
        .await
        .context(format!("Failed to list pods in namespace {ns}"))?
    } else {
      let api: Api<Pod> = Api::all(self.client.clone());
      api
        .list(&ListParams::default())
        .await
        .context("Failed to list pods in all namespaces")?
    };

    let pod_list = pods.items.iter().map(PodInfo::from_pod).collect();
    Ok(pod_list)
  }

  /// Delete a pod
  pub async fn delete_pod(&self, name: &str, namespace: &str) -> Result<()> {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    api
      .delete(name, &DeleteParams::default())
      .await
      .context(format!("Failed to delete pod {name} in namespace {namespace}"))?;
    Ok(())
  }

  /// Stream pod logs as raw bytes into the given channel until the
  /// receiver drops or the pod stops producing output.
  pub async fn stream_pod_logs(
    &self,
    name: &str,
    namespace: &str,
    container: Option<&str>,
    tail_lines: Option<i64>,
    timestamps: bool,
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
  ) -> Result<()> {
    use futures::AsyncBufReadExt;
    use futures::TryStreamExt;

    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);

    let mut params = LogParams::default();
    if let Some(c) = container {
      params.container = Some(c.to_string());
    }
    if let Some(lines) = tail_lines {
      params.tail_lines = Some(lines);
    }
    params.follow = true;
    params.timestamps = timestamps;

    let reader = api
      .log_stream(name, &params)
      .await
      .context(format!("Failed to open log stream for pod {name}"))?;
    let mut lines = reader.lines();

    while let Some(line) = lines.try_next().await? {
      // Re-add CRLF since the line stream stripped the trailing \n.
      let mut bytes = line.into_bytes();
      bytes.push(b'\r');
      bytes.push(b'\n');
      if tx.send(bytes).await.is_err() {
        return Ok(());
      }
    }
    Ok(())
  }

  /// Force delete a pod (grace period 0)
  pub async fn force_delete_pod(&self, name: &str, namespace: &str) -> Result<()> {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let dp = DeleteParams {
      grace_period_seconds: Some(0),
      ..DeleteParams::default()
    };
    api
      .delete(name, &dp)
      .await
      .context(format!("Failed to force delete pod {name} in namespace {namespace}"))?;
    Ok(())
  }

  /// Restart a pod - uses rollout restart for Deployments, delete for other controllers
  /// Returns error for standalone pods
  pub async fn restart_pod(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let pod = api.get(name).await.context(format!("Failed to get pod {name}"))?;

    // Check owner references
    let owner_refs = pod.metadata.owner_references.unwrap_or_default();
    if owner_refs.is_empty() {
      return Err(anyhow::anyhow!(
        "Cannot restart standalone pod '{name}'. It has no controller to recreate it."
      ));
    }

    let owner = &owner_refs[0];

    // If owned by ReplicaSet, check if RS is owned by Deployment
    if owner.kind == "ReplicaSet" {
      let rs_api: Api<ReplicaSet> = Api::namespaced(self.client.clone(), namespace);
      if let Ok(rs) = rs_api.get(&owner.name).await {
        let rs_owners = rs.metadata.owner_references.unwrap_or_default();
        if let Some(rs_owner) = rs_owners.first()
          && rs_owner.kind == "Deployment"
        {
          // Trigger rollout restart by patching the deployment
          return self.rollout_restart_deployment(&rs_owner.name, namespace).await;
        }
      }
    }

    // For Deployment direct ownership (shouldn't happen but handle it)
    if owner.kind == "Deployment" {
      return self.rollout_restart_deployment(&owner.name, namespace).await;
    }

    // For StatefulSet, DaemonSet, Job, etc - just delete the pod
    api
      .delete(name, &DeleteParams::default())
      .await
      .context(format!("Failed to delete pod {name}"))?;

    Ok(format!("Pod {} deleted, {} will recreate it", name, owner.kind))
  }

  /// Trigger a rollout restart on a deployment
  async fn rollout_restart_deployment(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);

    // Patch the deployment with a restart annotation (same as kubectl rollout restart)
    let patch = json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": Utc::now().to_rfc3339()
                    }
                }
            }
        }
    });

    api
      .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
      .await
      .context(format!("Failed to restart deployment {name}"))?;

    Ok(format!("Deployment {name} rollout restart triggered"))
  }

  /// Get pod describe output (formatted pod details)
  pub async fn describe_pod(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let pod = api.get(name).await.context(format!("Failed to get pod {name}"))?;

    // Format pod details similar to kubectl describe
    let mut output = String::new();

    // Basic info
    let _ = writeln!(output, "Name:         {}", pod.metadata.name.as_deref().unwrap_or(""));
    let _ = writeln!(
      output,
      "Namespace:    {}",
      pod.metadata.namespace.as_deref().unwrap_or("")
    );

    if let Some(labels) = &pod.metadata.labels {
      output.push_str("Labels:       ");
      for (i, (k, v)) in labels.iter().enumerate() {
        if i > 0 {
          output.push_str("              ");
        }
        let _ = writeln!(output, "{k}={v}");
      }
    }

    if let Some(annotations) = &pod.metadata.annotations {
      output.push_str("Annotations:  ");
      for (i, (k, v)) in annotations.iter().enumerate() {
        if i > 0 {
          output.push_str("              ");
        }
        let _ = writeln!(output, "{k}={v}");
      }
    }

    if let Some(spec) = &pod.spec {
      let _ = writeln!(
        output,
        "Node:         {}",
        spec.node_name.as_deref().unwrap_or("<none>")
      );
      let _ = writeln!(
        output,
        "Service Account: {}",
        spec.service_account_name.as_deref().unwrap_or("default")
      );
    }

    if let Some(status) = &pod.status {
      let _ = writeln!(output, "Status:       {}", status.phase.as_deref().unwrap_or("Unknown"));
      let _ = writeln!(output, "IP:           {}", status.pod_ip.as_deref().unwrap_or("<none>"));

      if let Some(conditions) = &status.conditions {
        output.push_str("\nConditions:\n");
        output.push_str("  Type              Status\n");
        for cond in conditions {
          let _ = writeln!(output, "  {:<17} {}", cond.type_, cond.status);
        }
      }

      if let Some(container_statuses) = &status.container_statuses {
        output.push_str("\nContainers:\n");
        for cs in container_statuses {
          let _ = writeln!(output, "  {}:", cs.name);
          let _ = writeln!(output, "    Ready:          {}", cs.ready);
          let _ = writeln!(output, "    Restart Count:  {}", cs.restart_count);
          if let Some(state) = &cs.state {
            if let Some(running) = &state.running {
              output.push_str("    State:          Running\n");
              if let Some(started) = &running.started_at {
                let _ = writeln!(output, "      Started:      {}", started.0);
              }
            } else if let Some(waiting) = &state.waiting {
              output.push_str("    State:          Waiting\n");
              let _ = writeln!(
                output,
                "      Reason:       {}",
                waiting.reason.as_deref().unwrap_or("")
              );
            } else if let Some(terminated) = &state.terminated {
              output.push_str("    State:          Terminated\n");
              let _ = writeln!(
                output,
                "      Reason:       {}",
                terminated.reason.as_deref().unwrap_or("")
              );
              let _ = writeln!(output, "      Exit Code:    {}", terminated.exit_code);
            }
          }
        }
      }
    }

    if let Some(spec) = &pod.spec
      && let Some(containers) = spec.containers.first()
    {
      let _ = writeln!(output, "\nImage:        {}", containers.image.as_deref().unwrap_or(""));
    }

    Ok(output)
  }

  /// Get pod YAML
  pub async fn get_pod_yaml(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let pod = api.get(name).await.context(format!("Failed to get pod {name}"))?;

    serde_yaml::to_string(&pod).context("Failed to serialize pod to YAML")
  }

  // ========================================================================
  // Service Methods
  // ========================================================================

  /// List services in a namespace (or all namespaces if None)
  pub async fn list_services(&self, namespace: Option<&str>) -> Result<Vec<ServiceInfo>> {
    let services = if let Some(ns) = namespace {
      let api: Api<Service> = Api::namespaced(self.client.clone(), ns);
      api
        .list(&ListParams::default())
        .await
        .context(format!("Failed to list services in namespace {ns}"))?
    } else {
      let api: Api<Service> = Api::all(self.client.clone());
      api
        .list(&ListParams::default())
        .await
        .context("Failed to list services in all namespaces")?
    };

    let service_list = services.items.iter().map(ServiceInfo::from_service).collect();
    Ok(service_list)
  }

  /// Delete a service
  pub async fn delete_service(&self, name: &str, namespace: &str) -> Result<()> {
    let api: Api<Service> = Api::namespaced(self.client.clone(), namespace);
    api
      .delete(name, &DeleteParams::default())
      .await
      .context(format!("Failed to delete service {name} in namespace {namespace}"))?;
    Ok(())
  }

  /// Get service YAML
  pub async fn get_service_yaml(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Service> = Api::namespaced(self.client.clone(), namespace);
    let svc = api.get(name).await.context(format!("Failed to get service {name}"))?;

    serde_yaml::to_string(&svc).context("Failed to serialize service to YAML")
  }

  // ========================================================================
  // Deployment Methods
  // ========================================================================

  /// List deployments in a namespace (or all namespaces if None)
  pub async fn list_deployments(&self, namespace: Option<&str>) -> Result<Vec<DeploymentInfo>> {
    let deployments = if let Some(ns) = namespace {
      let api: Api<Deployment> = Api::namespaced(self.client.clone(), ns);
      api
        .list(&ListParams::default())
        .await
        .context(format!("Failed to list deployments in namespace {ns}"))?
    } else {
      let api: Api<Deployment> = Api::all(self.client.clone());
      api
        .list(&ListParams::default())
        .await
        .context("Failed to list deployments in all namespaces")?
    };

    let deployment_list = deployments.items.iter().map(DeploymentInfo::from_deployment).collect();
    Ok(deployment_list)
  }

  /// Delete a deployment
  pub async fn delete_deployment(&self, name: &str, namespace: &str) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
    api
      .delete(name, &DeleteParams::default())
      .await
      .context(format!("Failed to delete deployment {name} in namespace {namespace}"))?;
    Ok(())
  }

  /// Scale a deployment
  pub async fn scale_deployment(&self, name: &str, namespace: &str, replicas: i32) -> Result<String> {
    let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);

    let patch = json!({
        "spec": {
            "replicas": replicas
        }
    });

    api
      .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
      .await
      .context(format!("Failed to scale deployment {name}"))?;

    Ok(format!("Deployment {name} scaled to {replicas} replicas"))
  }

  /// Restart a deployment (rollout restart)
  pub async fn restart_deployment(&self, name: &str, namespace: &str) -> Result<String> {
    self.rollout_restart_deployment(name, namespace).await
  }

  /// Get deployment YAML
  pub async fn get_deployment_yaml(&self, name: &str, namespace: &str) -> Result<String> {
    let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
    let dep = api
      .get(name)
      .await
      .context(format!("Failed to get deployment {name}"))?;

    serde_yaml::to_string(&dep).context("Failed to serialize deployment to YAML")
  }

  /// Create a deployment
  pub async fn create_deployment(&self, options: CreateDeploymentOptions) -> Result<String> {
    use k8s_openapi::api::apps::v1::DeploymentSpec;
    use k8s_openapi::api::core::v1::{
      Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, ResourceRequirements,
    };
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
    use std::collections::BTreeMap;

    let namespace = if options.namespace.is_empty() {
      "default".to_string()
    } else {
      options.namespace.clone()
    };

    // Build labels
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_string(), options.name.clone());
    for (k, v) in &options.labels {
      if !k.is_empty() {
        labels.insert(k.clone(), v.clone());
      }
    }

    // Build container ports
    let container_ports: Vec<ContainerPort> = options
      .ports
      .iter()
      .filter(|p| p.container_port > 0)
      .map(|p| ContainerPort {
        container_port: p.container_port,
        name: if p.name.is_empty() { None } else { Some(p.name.clone()) },
        protocol: Some(p.protocol.clone()),
        ..Default::default()
      })
      .collect();

    // Build environment variables
    let env_vars: Vec<EnvVar> = options
      .env_vars
      .iter()
      .filter(|(k, _)| !k.is_empty())
      .map(|(k, v)| EnvVar {
        name: k.clone(),
        value: Some(v.clone()),
        ..Default::default()
      })
      .collect();

    // Build resource requirements
    let mut limits: BTreeMap<String, Quantity> = BTreeMap::new();
    let mut requests: BTreeMap<String, Quantity> = BTreeMap::new();

    if !options.cpu_limit.is_empty() {
      limits.insert("cpu".to_string(), Quantity(options.cpu_limit.clone()));
    }
    if !options.memory_limit.is_empty() {
      limits.insert("memory".to_string(), Quantity(options.memory_limit.clone()));
    }
    if !options.cpu_request.is_empty() {
      requests.insert("cpu".to_string(), Quantity(options.cpu_request.clone()));
    }
    if !options.memory_request.is_empty() {
      requests.insert("memory".to_string(), Quantity(options.memory_request.clone()));
    }

    let resources = if limits.is_empty() && requests.is_empty() {
      None
    } else {
      Some(ResourceRequirements {
        limits: if limits.is_empty() { None } else { Some(limits) },
        requests: if requests.is_empty() { None } else { Some(requests) },
        ..Default::default()
      })
    };

    // Build the container
    let container = Container {
      name: options.name.clone(),
      image: Some(options.image.clone()),
      image_pull_policy: if options.image_pull_policy.is_empty() {
        None
      } else {
        Some(options.image_pull_policy.clone())
      },
      ports: if container_ports.is_empty() {
        None
      } else {
        Some(container_ports)
      },
      env: if env_vars.is_empty() { None } else { Some(env_vars) },
      resources,
      command: if options.command.is_empty() {
        None
      } else {
        Some(options.command.clone())
      },
      args: if options.args.is_empty() {
        None
      } else {
        Some(options.args.clone())
      },
      ..Default::default()
    };

    // Build the deployment
    let deployment = Deployment {
      metadata: ObjectMeta {
        name: Some(options.name.clone()),
        namespace: Some(namespace.clone()),
        labels: Some(labels.clone()),
        ..Default::default()
      },
      spec: Some(DeploymentSpec {
        replicas: Some(options.replicas),
        selector: LabelSelector {
          match_labels: Some(labels.clone()),
          ..Default::default()
        },
        template: PodTemplateSpec {
          metadata: Some(ObjectMeta {
            labels: Some(labels),
            ..Default::default()
          }),
          spec: Some(PodSpec {
            containers: vec![container],
            ..Default::default()
          }),
        },
        ..Default::default()
      }),
      ..Default::default()
    };

    let api: Api<Deployment> = Api::namespaced(self.client.clone(), &namespace);
    api
      .create(&PostParams::default(), &deployment)
      .await
      .context(format!("Failed to create deployment {}", options.name))?;

    Ok(format!(
      "Deployment {} created in namespace {}",
      options.name, namespace
    ))
  }

  /// Create a service
  pub async fn create_service(&self, options: CreateServiceOptions) -> Result<String> {
    use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
    use std::collections::BTreeMap;

    let namespace = if options.namespace.is_empty() {
      "default".to_string()
    } else {
      options.namespace.clone()
    };

    // Build labels
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_string(), options.name.clone());

    // Build selector
    let mut selector: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in &options.selector {
      if !k.is_empty() {
        selector.insert(k.clone(), v.clone());
      }
    }
    // If no selector provided, default to app=name
    if selector.is_empty() {
      selector.insert("app".to_string(), options.name.clone());
    }

    // Build ports
    let ports: Vec<ServicePort> = options
      .ports
      .iter()
      .filter(|p| p.port > 0)
      .map(|p| ServicePort {
        name: if p.name.is_empty() { None } else { Some(p.name.clone()) },
        port: p.port,
        target_port: if p.target_port > 0 {
          Some(IntOrString::Int(p.target_port))
        } else {
          Some(IntOrString::Int(p.port))
        },
        node_port: if p.node_port > 0 { Some(p.node_port) } else { None },
        protocol: Some(p.protocol.clone()),
        ..Default::default()
      })
      .collect();

    if ports.is_empty() {
      return Err(anyhow::anyhow!("At least one port is required"));
    }

    // Build the service
    let service = Service {
      metadata: ObjectMeta {
        name: Some(options.name.clone()),
        namespace: Some(namespace.clone()),
        labels: Some(labels),
        ..Default::default()
      },
      spec: Some(ServiceSpec {
        type_: Some(options.service_type.clone()),
        selector: Some(selector),
        ports: Some(ports),
        ..Default::default()
      }),
      ..Default::default()
    };

    let api: Api<Service> = Api::namespaced(self.client.clone(), &namespace);
    api
      .create(&PostParams::default(), &service)
      .await
      .context(format!("Failed to create service {}", options.name))?;

    Ok(format!("Service {} created in namespace {}", options.name, namespace))
  }
}

/// Options for creating a deployment
#[derive(Debug, Clone, Default)]
pub struct CreateDeploymentOptions {
  pub name: String,
  pub namespace: String,
  pub image: String,
  pub replicas: i32,
  pub ports: Vec<ContainerPortConfig>,
  pub env_vars: Vec<(String, String)>,
  pub labels: Vec<(String, String)>,
  pub cpu_limit: String,
  pub memory_limit: String,
  pub cpu_request: String,
  pub memory_request: String,
  pub image_pull_policy: String,
  pub command: Vec<String>,
  pub args: Vec<String>,
}

/// Container port configuration
#[derive(Debug, Clone, Default)]
pub struct ContainerPortConfig {
  pub name: String,
  pub container_port: i32,
  pub protocol: String,
}

/// Options for creating a service
#[derive(Debug, Clone, Default)]
pub struct CreateServiceOptions {
  pub name: String,
  pub namespace: String,
  pub service_type: String, // ClusterIP, NodePort, LoadBalancer
  pub ports: Vec<ServicePortConfig>,
  pub selector: Vec<(String, String)>,
}

/// Service port configuration
#[derive(Debug, Clone, Default)]
pub struct ServicePortConfig {
  pub name: String,
  pub port: i32,
  pub target_port: i32,
  pub node_port: i32,
  pub protocol: String,
}

// ============================================================================
// VPN-aware connection helpers
// ============================================================================

/// Check if a URL uses a non-localhost IP address (likely a VM IP)
fn is_non_localhost_ip(url: &str) -> bool {
  // Parse the URL to extract the host
  if let Ok(parsed) = url::Url::parse(url)
    && let Some(host) = parsed.host_str()
  {
    // Check if it's an IP address that's not localhost
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
      return !ip.is_loopback();
    }
    // Also check for "localhost" hostname
    return host != "localhost";
  }
  false
}

/// Try to create a localhost fallback config
/// Extracts the port from the original URL and creates a new config pointing to localhost
fn try_localhost_fallback(original_url: &str, mut config: Config) -> Option<Config> {
  // Parse the original URL to get the port
  let parsed = url::Url::parse(original_url).ok()?;
  let port = parsed.port()?;

  // Create new localhost URL with same port and scheme
  let scheme = parsed.scheme();
  let localhost_url = format!("{scheme}://127.0.0.1:{port}");

  tracing::info!("Trying localhost fallback: {}", localhost_url);

  // Update the config with localhost URL
  config.cluster_url = localhost_url.parse().ok()?;

  // For self-signed certs, we may need to accept invalid certs when switching to localhost
  // since the cert is likely issued for the VM IP, not localhost
  config.accept_invalid_certs = true;

  Some(config)
}
