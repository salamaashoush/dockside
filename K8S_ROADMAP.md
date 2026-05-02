# Kubernetes Support Roadmap

Plan for evolving Dockside's Kubernetes capabilities from a single-context viewer into a full multi-cluster operations console. This document captures intent, scope, architecture decisions, and a phased delivery plan. Each phase ships independently and leaves the app in a usable state.

## Goals

- **First-class multi-cluster**: switch contexts cleanly, manage credentials in one place, support local clusters (k3s, kind, minikube, kubeadm) and remote clusters (managed providers, bare-metal).
- **Cluster lifecycle**: connect to existing clusters, register new ones, validate connectivity, remove/rename, all without leaving the app.
- **Node operations**: depth equal to `kubectl` for day-2 tasks — cordon/uncordon/drain (already shipped), label/taint editing, conditions, capacity, pods-on-node, top metrics, debug pod for shell/logs.
- **Add nodes**: surface join commands and tokens for self-managed distros (k3s, kubeadm); link to console for managed clusters; optional SSH-based remote provisioning.
- **Smart defaults**: detect distro, surface relevant ops, hide nonsense (e.g. drain on a single-node k3s warns).
- **Read-mostly safety**: every destructive operation goes through a confirmation dialog when `settings.confirm_destructive` is on.

## Non-Goals

- Cluster provisioning from scratch (`k3sup install`, `eksctl create cluster`). Out of scope — vendor-specific tooling, better left to dedicated CLIs.
- Workload authoring (deployment YAML generators, helm chart browser). Different problem domain.
- RBAC editor. Big surface area, defer indefinitely.
- Metrics dashboards. Grafana exists; we surface raw metrics-server values only.
- Custom resource definitions browser. Defer until base UX is solid.

## Architecture Principles

1. **One `KubeClient` per context, lazy-built**. Pool keyed by context name. Switching context activates a different client; no global mutation of `~/.kube/config`.
2. **State scoped per context**. `DockerState.k8s` becomes `HashMap<ContextName, ClusterState>` plus `active_context: Option<String>`. Refresh services key off the active context.
3. **Kubeconfig is source of truth**. We read/write user kubeconfig; we do not maintain a parallel credential store. Optional: per-app override path in `settings.kubeconfig_path`.
4. **All cluster mutations are reversible from kubectl**. If a user adds a context in Dockside, `kubectl config get-contexts` shows it. Avoids lock-in.
5. **Async-only on the network**. UI never blocks on kube API. Existing `Tokio::spawn` + `cx.spawn` pattern stands.
6. **Distro detection by signal, not heuristic on hostnames**. Use node labels (`node.kubernetes.io/instance-type`, `k3s.io/hostname`), kubelet version (`+k3s`, `kubeadm`), API server endpoint suffix (eks.amazonaws.com, gke).

## Current Gaps (as of 2026-05)

| Area | State | Gap |
|---|---|---|
| Contexts | Single, default-resolved | No switch, no list, no add/remove |
| Cluster CRUD | Namespace + delete only | No add/remove cluster, no kubeconfig edit |
| Node detail | Tabular row only | No detail pane, no pods-on-node, no taints/labels editor |
| Node ops | cordon/uncordon/drain via row menu | No taint editor, no metrics, no debug shell |
| Add nodes | None | No join command surface, no SSH provisioner |
| Auth methods | Whatever kube crate auto-resolves | No UI for token rotation, exec auth setup, oidc refresh |
| Connection feedback | Generic error banner | No "test connection" affordance, no version display |

## Phased Plan

### Phase 1 — Multi-context foundation

**Goal**: switch between kubeconfig contexts inside the app; refresh all k8s state on switch.

**Scope**:
- Parse kubeconfig at startup. Surface `Vec<ContextSummary { name, cluster, user, namespace }>`.
- New `ClusterState` struct holding all per-context data currently flat in `DockerState`. Migrate `pods`, `services`, `deployments`, etc. into it.
- `active_context: Option<String>` in `DockerState`. Fall back to kubeconfig `current-context`.
- Context switcher UI: dropdown in topbar (next to namespace selector) showing context name + cluster server. Clicking switches and triggers full refresh.
- All `KubeClient::new()` callsites take a context name. Centralize in `KubeClient::for_context(name)`.
- On switch: clear all per-resource state, set `LoadState::NotLoaded`, fan out refresh.
- Settings field `default_context: Option<String>` (already have `kubeconfig_path`).

**Risks**:
- Existing `KubeClient::new()` is called from ~20 services. Touchy refactor. Mitigate with helper that reads active context from `DockerState`.
- `cx.spawn` futures may outlive a context switch and write to old state slot. Solution: tag in-flight requests with the context they were issued under; drop results if `active_context` changed.

**Acceptance**:
- Two contexts in `~/.kube/config`. Switch in UI. Pods/Services lists swap. No stale data shown after switch. No panic if a context is unreachable.

### Phase 2 — Cluster CRUD (kubeconfig editor)

**Goal**: add and remove cluster contexts from inside the app, write kubeconfig back atomically.

**Scope**:
- New top-level view `Clusters` (sidebar entry, above the existing Cluster overview which moves into per-cluster scope).
- Cluster list: name, server, user, current-context indicator, last-seen status.
- Add context flow:
  - **Import from file**: file picker, parse, merge into kubeconfig (preserve existing entries, dedupe by name).
  - **Manual form**: server URL, CA cert (paste or file), auth method radio:
    - Token (bearer)
    - Client certificate (cert+key, paste or file)
    - Exec plugin (command, args, env) for cloud auth (`aws-iam-authenticator`, `gke-gcloud-auth-plugin`)
    - OIDC (issuer, client-id, refresh token)
  - **From cloud**: future, separate flow per provider.
- Test connection button: hit `/version`, show server version + connectivity status.
- Edit context (rename, change default namespace, change user reference).
- Remove context: confirm dialog, atomic write of kubeconfig (write to temp file, rename).

**Risks**:
- Atomic kubeconfig write across platforms. Use `tempfile::NamedTempFile::persist`. Backup `kubeconfig.bak` on first edit.
- Auth plugin paths differ per machine. Show a hint "exec command must be on PATH at runtime."
- Multi-config merge: `KUBECONFIG` env var supports `:`-separated list. We edit only the first writable file unless user picks.

**Acceptance**:
- Add a kind cluster via "Import from file." Switch to it. Refresh all state. Remove it. `kubectl config get-contexts` reflects all changes.

### Phase 3 — Node detail + ops

**Goal**: per-node detail pane matching the depth of `kubectl describe node` plus inline editing for labels and taints.

**Scope**:
- `Selection::Node(String)`, `NodeDetailTab::{Info, Pods, Conditions, Taints, Labels, Yaml, Events}`.
- `src/ui/nodes/{list,detail,view}.rs`. Replace `ClusterView`'s Nodes tab body with `NodesView` (list + detail split). Cluster overview becomes shell with namespace selector + tabs (Nodes, Events, Namespaces).
- Info tab: status, roles, version, OS/arch, internal IP, allocatable, capacity, pod count, age, kernel, container runtime.
- Pods tab: filtered `state.pods` where `pod.node == node.name`. Reuses pod row UI.
- Conditions tab: table of (type, status, last_transition, reason, message) for Ready/MemoryPressure/DiskPressure/PIDPressure/NetworkUnavailable.
- Taints tab: list rows of `key=value:effect`, edit/add/remove inline. Patch via `spec.taints` merge.
- Labels tab: key=value rows, inline edit, patch via `metadata.labels` strategic merge. Reserved labels (`node.kubernetes.io/*`, `kubernetes.io/*`) shown read-only with a hint.
- YAML tab: standard get/apply pattern, same as other resources.
- Events tab: filter `state.events` by `object_kind == "Node" && object_name == node.name`.
- Optional: top metrics from `metrics.k8s.io/v1beta1/nodes/<name>` if metrics-server present. Probe `/apis/metrics.k8s.io` once, cache result, hide tab if 404.
- Pin/unpin via `FavoriteRef::Node(String)`.

**Risks**:
- `metrics.k8s.io` may not exist. Probe and degrade gracefully.
- Reserved labels: trying to write them returns 422. Pre-validate.
- Pods-on-node list grows large. Use the same virtualization as existing pod list.

**Acceptance**:
- k3s single-node cluster. Click node. All seven tabs populated. Add a label `team=infra` and a taint `key1=val1:NoSchedule`. Verify with `kubectl get node -o yaml`.

### Phase 4 — Add nodes (distro-aware, optional SSH)

**Goal**: surface the right join workflow for the detected distro. Phase 4 has two slices; ship 4a first.

#### Phase 4a — Read-only join surface

**Scope**:
- Distro detector reading first node's labels + kubelet version + API server endpoint:
  - k3s: kubelet version contains `+k3s`, label `k3s.io/node-name`.
  - kubeadm: label `node-role.kubernetes.io/control-plane`, kubelet version no distro suffix.
  - EKS: server endpoint matches `*.eks.amazonaws.com`, label `eks.amazonaws.com/nodegroup`.
  - GKE: server endpoint matches `container.googleapis.com` or label `cloud.google.com/gke-nodepool`.
  - AKS: label `kubernetes.azure.com/cluster`.
  - kind/minikube/k3d: detect by node names + special labels.
- New "Add Node" button on Cluster overview Nodes tab.
- Per-distro panels:
  - **k3s** (local control-plane access): read `/var/lib/rancher/k3s/server/node-token` if reachable on the loopback API server host. Show `K3S_URL=https://<server>:6443 K3S_TOKEN=<token> sh -c 'curl -sfL https://get.k3s.io | sh -'`. Copy button. Note: requires running on the master itself or SSH (4b).
  - **kubeadm**: instruct user to run `kubeadm token create --print-join-command` on a control-plane node. Copy template. Optional: if API access to a control-plane node can shell out, do it for them (requires SSH or `kubectl debug node`).
  - **Managed (EKS/GKE/AKS)**: show "Adding nodes is managed by your cloud provider" + deep-link to console URL constructed from the API server endpoint.
  - **kind/k3d**: show `kind create cluster` / `k3d node create` command template.
- No remote execution in 4a. Copy-paste only.

**Acceptance**:
- k3s test cluster: open Add Node, see correct join command with token if running locally; otherwise see "join from a node with SSH access" placeholder.
- EKS cluster: see deep link to AWS console node group page.

#### Phase 4b — SSH-based remote join (stretch)

**Scope**:
- `russh` or `ssh2` crate added.
- Per-cluster "Hosts" inventory in settings: `Vec<HostEntry { name, address, user, key_path or password_ref }>`.
- "Add Node" wizard: pick host, pick role (control-plane/worker), confirm. Dockside SSHes in, runs the appropriate install/join command, streams output to a task-bar progress entry.
- For k3s: SSH into existing master to fetch token (avoids requiring local file access), then SSH into target host to run installer.
- For kubeadm: SSH into control-plane to mint a token, then SSH into target to `kubeadm join`.
- Audit log of all remote commands run, kept in `~/.config/dockside/audit.log`.
- Confirmation dialogs for every remote write.

**Risks**:
- SSH key management. Use system agent (`SSH_AUTH_SOCK`). Optional password mode reads from OS keyring (`keyring` crate).
- Long-running installs (k3s installer is ~20s, kubeadm join ~60s). Need streaming progress.
- Idempotency: detect already-joined nodes via `kubectl get nodes` before starting.
- Failure recovery: partial install leaves node in broken state. Surface remediation steps.

**Acceptance**:
- Two-VM lab: Dockside on laptop, Ubuntu masters/workers reachable via SSH. Add a worker via the wizard. `kubectl get nodes` shows it `Ready` within 90s.

### Phase 5 — Smart defaults and quality-of-life

**Goal**: polish layer that makes the multi-cluster experience pleasant.

**Scope**:
- Last-used context restored on app launch.
- Context-aware sidebar: if active context lacks an Ingress controller, hide the Networking>Ingresses badge count or show "no controller detected".
- API discovery cache per context. Hide tabs for absent APIs (e.g. metrics-server, gateway-api).
- Connection status indicator in topbar (green/yellow/red dot) reflecting last successful API call age.
- Cluster version + warning if Dockside kube crate version is significantly older.
- `kubectl` parity hints: each detail toolbar has a "copy as kubectl" button that prints the equivalent command.
- Per-context UI preferences (favorite namespaces, pinned resources) persisted separately.
- Telemetry-free crash reporter for kube errors with redacted server URLs (opt-in only).

## Data Model Changes (high level)

```rust
// settings.rs additions
struct AppSettings {
    // existing fields...
    default_context: Option<String>,
    cluster_hosts: HashMap<String, Vec<HostEntry>>, // phase 4b
}

// docker_state.rs additions
struct DockerState {
    // existing fields move into per-context map
    contexts: HashMap<String, ClusterState>,
    active_context: Option<String>,
}

struct ClusterState {
    pods: Vec<PodInfo>,
    // ... all current k8s collections ...
    api_discovery: ApiDiscovery,
    server_version: Option<String>,
    last_error: Option<String>,
}

struct ApiDiscovery {
    has_metrics_server: bool,
    has_gateway_api: bool,
    crds: Vec<CrdSummary>,
    last_refreshed: DateTime<Utc>,
}

// New variants
enum Selection {
    // existing...
    Node(String),
    Cluster(String), // phase 2
}

enum FavoriteRef {
    // existing...
    Node { context: String, name: String },
}
```

## Open Questions

1. **Context switcher placement**: topbar (cross-resource) vs sidebar k8s-group header (scoped feel). Topbar feels more correct since context affects every k8s view.
2. **kubeconfig write authority**: do we always edit `~/.kube/config`, or honor `KUBECONFIG` env var with multi-file fallback? Multi-file is correct but more code.
3. **Phase 4b auth**: SSH agent only, or password fallback? Password fallback needs OS keyring integration.
4. **Top metrics**: bundle metrics-server install hint, or pure read-only?
5. **Dockside daemon**: do we ever need a long-running Dockside agent on a node (for log tail beyond pod lifetime, kubelet metrics scrape)? Probably not for v1.

## Delivery Order Recommendation

1. Phase 1 (multi-context) — biggest unlock per LOC.
2. Phase 3 (Node detail) — addresses immediate user gap.
3. Phase 2 (Cluster CRUD) — needed before "add cluster" workflows feel complete.
4. Phase 4a (read-only join) — fills "add nodes" without SSH dependency.
5. Phase 5 (smart defaults) — interleaved as we go.
6. Phase 4b (SSH provisioner) — only if real demand exists.

Total: ~5 sessions of focused work for phases 1-3, plus 1-2 each for phase 2/4a/5.
