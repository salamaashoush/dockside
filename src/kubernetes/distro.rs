//! Kubernetes distribution detection + per-distro "add a node" guidance.
//!
//! Read-only (roadmap phase 4a): we never run anything remotely — we
//! detect the distro from node signals and hand back copy-pasteable
//! commands / console deep links so the user joins a node themselves.

use crate::kubernetes::NodeInfo;

/// Detected cluster distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
  K3s,
  K3d,
  Kubeadm,
  Eks,
  Gke,
  Aks,
  Kind,
  Minikube,
  Unknown,
}

impl Distro {
  pub fn label(self) -> &'static str {
    match self {
      Distro::K3s => "k3s",
      Distro::K3d => "k3d",
      Distro::Kubeadm => "kubeadm",
      Distro::Eks => "Amazon EKS",
      Distro::Gke => "Google GKE",
      Distro::Aks => "Azure AKS",
      Distro::Kind => "kind",
      Distro::Minikube => "minikube",
      Distro::Unknown => "Kubernetes",
    }
  }

  /// True for cloud-managed control planes — node count is changed via the
  /// provider, not a join token.
  pub fn is_managed(self) -> bool {
    matches!(self, Distro::Eks | Distro::Gke | Distro::Aks)
  }
}

/// Guidance shown in the Add Node dialog.
#[derive(Debug, Clone)]
pub struct JoinGuide {
  pub distro: Distro,
  pub title: String,
  /// Ordered human steps.
  pub steps: Vec<String>,
  /// A single copy-pasteable command, when one applies.
  pub command: Option<String>,
  /// Cloud console deep link for managed clusters.
  pub console_url: Option<String>,
}

fn has_label_prefix(nodes: &[NodeInfo], prefix: &str) -> bool {
  nodes
    .iter()
    .any(|n| n.labels.iter().any(|(k, _)| k.starts_with(prefix) || k == prefix))
}

fn any_kubelet_contains(nodes: &[NodeInfo], needle: &str) -> bool {
  nodes.iter().any(|n| n.version.contains(needle))
}

fn any_node_name_contains(nodes: &[NodeInfo], needle: &str) -> bool {
  nodes.iter().any(|n| n.name.contains(needle))
}

/// Host of the API server URL (no scheme/port), for console links.
fn server_host(server: &str) -> String {
  url::Url::parse(server)
    .ok()
    .and_then(|u| u.host_str().map(ToString::to_string))
    .unwrap_or_else(|| server.to_string())
}

/// Detect the distro from node labels, kubelet version and API endpoint.
/// Cloud labels are definitive so they win; then k3s/k3d, kind, minikube,
/// and finally kubeadm as the generic self-managed fallback.
#[must_use]
pub fn detect(nodes: &[NodeInfo], server: &str) -> Distro {
  let host = server_host(server);

  if has_label_prefix(nodes, "eks.amazonaws.com/") || host.ends_with(".eks.amazonaws.com") {
    return Distro::Eks;
  }
  if has_label_prefix(nodes, "cloud.google.com/gke-") {
    return Distro::Gke;
  }
  if has_label_prefix(nodes, "kubernetes.azure.com/") {
    return Distro::Aks;
  }
  // k3d runs k3s inside Docker; node names are k3d-<cluster>-...
  if any_node_name_contains(nodes, "k3d-") {
    return Distro::K3d;
  }
  if any_kubelet_contains(nodes, "+k3s") || has_label_prefix(nodes, "k3s.io/") {
    return Distro::K3s;
  }
  if has_label_prefix(nodes, "minikube.k8s.io/") || any_node_name_contains(nodes, "minikube") {
    return Distro::Minikube;
  }
  if has_label_prefix(nodes, "kind.x-k8s.io/")
    || nodes
      .iter()
      .any(|n| n.name.ends_with("-control-plane") || n.name.ends_with("-worker"))
  {
    return Distro::Kind;
  }
  if has_label_prefix(nodes, "node-role.kubernetes.io/control-plane")
    || has_label_prefix(nodes, "node-role.kubernetes.io/master")
  {
    return Distro::Kubeadm;
  }
  Distro::Unknown
}

/// Build per-distro join guidance for the active cluster.
#[must_use]
pub fn join_guide(nodes: &[NodeInfo], server: &str) -> JoinGuide {
  let distro = detect(nodes, server);
  let host = server_host(server);
  let cluster_hint = nodes
    .iter()
    .find_map(|n| {
      n.labels
        .iter()
        .find(|(k, _)| k == "cloud.google.com/gke-nodepool" || k == "eks.amazonaws.com/nodegroup")
        .map(|(_, v)| v.clone())
    })
    .unwrap_or_default();

  match distro {
    Distro::K3s => JoinGuide {
      distro,
      title: "Add a worker to this k3s cluster".into(),
      steps: vec![
        format!("On a k3s server node, get the join token:\n  sudo cat /var/lib/rancher/k3s/server/node-token"),
        "On the new machine, run the command below (substitute <TOKEN>).".into(),
        "The node registers automatically and appears here within ~30s.".into(),
      ],
      command: Some(format!(
        "curl -sfL https://get.k3s.io | K3S_URL=https://{host}:6443 K3S_TOKEN=<TOKEN> sh -"
      )),
      console_url: None,
    },
    Distro::K3d => JoinGuide {
      distro,
      title: "Add a node to this k3d cluster".into(),
      steps: vec!["k3d manages nodes as Docker containers on this host.".into()],
      command: Some("k3d node create extra --cluster <CLUSTER> --role agent".into()),
      console_url: None,
    },
    Distro::Kubeadm => JoinGuide {
      distro,
      title: "Add a node with kubeadm".into(),
      steps: vec![
        "On a control-plane node, mint a fresh join command:\n  sudo kubeadm token create --print-join-command".into(),
        "Run the printed `kubeadm join …` on the new machine (as root).".into(),
        "For a control-plane node, add `--control-plane --certificate-key <key>` (see `kubeadm init phase upload-certs --upload-certs`).".into(),
      ],
      command: Some("sudo kubeadm token create --print-join-command".into()),
      console_url: None,
    },
    Distro::Minikube => JoinGuide {
      distro,
      title: "Add a node to minikube".into(),
      steps: vec!["minikube manages its own nodes locally.".into()],
      command: Some("minikube node add".into()),
      console_url: None,
    },
    Distro::Kind => JoinGuide {
      distro,
      title: "Add a node to a kind cluster".into(),
      steps: vec![
        "kind can't grow a running cluster — recreate it with extra nodes.".into(),
        "Write a kind config with the desired control-plane/worker roles, then:".into(),
      ],
      command: Some("kind create cluster --config kind-cluster.yaml".into()),
      console_url: None,
    },
    Distro::Eks => JoinGuide {
      distro,
      title: "Scale this EKS cluster".into(),
      steps: vec![
        "Nodes are managed by AWS. Scale the managed node group:".into(),
        "Or edit the node group's desired size in the EKS console.".into(),
      ],
      command: Some(format!(
        "eksctl scale nodegroup --cluster <CLUSTER> --name {} --nodes <N>",
        if cluster_hint.is_empty() { "<NODEGROUP>" } else { &cluster_hint }
      )),
      console_url: Some("https://console.aws.amazon.com/eks/home#/clusters".into()),
    },
    Distro::Gke => JoinGuide {
      distro,
      title: "Scale this GKE cluster".into(),
      steps: vec!["Nodes are managed by Google Cloud. Resize the node pool:".into()],
      command: Some(format!(
        "gcloud container clusters resize <CLUSTER> --node-pool {} --num-nodes <N>",
        if cluster_hint.is_empty() { "<NODEPOOL>" } else { &cluster_hint }
      )),
      console_url: Some("https://console.cloud.google.com/kubernetes/list".into()),
    },
    Distro::Aks => JoinGuide {
      distro,
      title: "Scale this AKS cluster".into(),
      steps: vec!["Nodes are managed by Azure. Scale the node pool:".into()],
      command: Some("az aks nodepool scale --resource-group <RG> --cluster-name <CLUSTER> --name <POOL> --node-count <N>".into()),
      console_url: Some("https://portal.azure.com/#blade/HubsExtension/BrowseResource/resourceType/Microsoft.ContainerService%2FmanagedClusters".into()),
    },
    Distro::Unknown => JoinGuide {
      distro,
      title: "Add a node".into(),
      steps: vec![
        "Distro not recognised. If self-managed, kubeadm is the usual path:".into(),
        "On a control-plane node: `sudo kubeadm token create --print-join-command`, then run it on the new machine.".into(),
        "If this is a managed cluster, scale nodes from your provider's console/CLI.".into(),
      ],
      command: None,
      console_url: None,
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn node(name: &str, version: &str, labels: &[(&str, &str)]) -> NodeInfo {
    NodeInfo {
      name: name.into(),
      status: "Ready".into(),
      roles: vec![],
      version: version.into(),
      os: "linux".into(),
      arch: "amd64".into(),
      kernel_version: String::new(),
      container_runtime: String::new(),
      internal_ip: None,
      age: "1d".into(),
      cpu_allocatable: String::new(),
      mem_allocatable: String::new(),
      cpu_capacity: String::new(),
      mem_capacity: String::new(),
      pods_allocatable: String::new(),
      pods_capacity: String::new(),
      pod_cidr: None,
      conditions: vec![],
      taints: vec![],
      labels: labels
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect(),
      unschedulable: false,
    }
  }

  #[test]
  fn detects_k3s_by_kubelet() {
    let n = node("srv", "v1.30.2+k3s1", &[]);
    assert_eq!(detect(&[n], "https://10.0.0.1:6443"), Distro::K3s);
  }

  #[test]
  fn detects_eks_by_endpoint() {
    let n = node("ip-10-0-0-1", "v1.30.0", &[]);
    assert_eq!(detect(&[n], "https://ABC.gr7.us-east-1.eks.amazonaws.com"), Distro::Eks);
  }

  #[test]
  fn detects_gke_and_kubeadm_and_unknown() {
    let gke = node("gke-x", "v1.29.0", &[("cloud.google.com/gke-nodepool", "default")]);
    assert_eq!(detect(&[gke], "https://1.2.3.4"), Distro::Gke);

    let kubeadm = node("cp", "v1.30.0", &[("node-role.kubernetes.io/control-plane", "")]);
    assert_eq!(detect(&[kubeadm], "https://1.2.3.4:6443"), Distro::Kubeadm);

    let unknown = node("n", "v1.30.0", &[]);
    assert_eq!(detect(&[unknown], "https://1.2.3.4:6443"), Distro::Unknown);
  }

  #[test]
  fn guide_has_command_for_known_distros() {
    let n = node("srv", "v1.30.2+k3s1", &[]);
    let g = join_guide(&[n], "https://10.0.0.1:6443");
    assert_eq!(g.distro, Distro::K3s);
    assert!(g.command.unwrap().contains("get.k3s.io"));
  }
}
