# Machines View Improvement Plan: Native Docker Host Support + Runtime Switching

## Problem Statement

On Linux with native Docker (no Colima), the Machines view is empty/broken. Linux users should:
1. See a "Host" machine representing the native Docker daemon
2. View Docker runtime info (version, CPUs, memory, kernel, etc.)
3. Optionally enable Colima/K8s support via Settings
4. **Switch between Host and Colima VMs seamlessly** - all Docker views update accordingly

## Research Findings

### Docker API Capabilities (via bollard)

The `docker.info()` API provides:
- **System**: OS name, kernel version, architecture, hostname
- **Resources**: CPU count, total memory
- **Storage**: driver name, root directory
- **Runtime**: Docker version, API version, containerd/runc versions
- **Stats**: container count, image count, running/paused/stopped counts

The `docker.version()` API provides:
- Client/Server version details
- Git commit, build time
- Go version

### Current ColimaVm Structure

```rust
pub struct ColimaVm {
  pub name: String,
  pub status: VmStatus,           // Running, Stopped, Unknown
  pub runtime: VmRuntime,         // Docker, Containerd, Incus
  pub arch: VmArch,               // Host, Aarch64, X86_64
  pub cpus: u32,
  pub memory: u64,                // bytes
  pub disk: u64,                  // bytes
  pub kubernetes: bool,
  pub address: Option<String>,    // IP
  pub driver: Option<String>,
  pub docker_socket: Option<String>,
  pub hostname: Option<String>,
  // ... more fields
}
```

### MachineDetail Tabs

1. **Info** - Name, status, IP, socket, OS info, versions, K8s status
2. **Config** - CPUs, memory, disk, VM settings, mounts, env vars
3. **Stats** - Memory/disk usage bars
4. **Processes** - Process list with kill capability
5. **Logs** - System/Docker/Containerd logs
6. **Terminal** - SSH access
7. **Files** - File explorer

## Current Architecture

### Global Docker Client
```rust
// src/services/core.rs
static DOCKER_CLIENT: OnceLock<Arc<RwLock<Option<DockerClient>>>> = OnceLock::new();
```

- `Arc<RwLock<Option<>>>` allows replacing inner value - switching IS possible

### Files Requiring Refactor (Complete List)

| File | Current | Change To |
|------|---------|-----------|
| `src/state/docker_state.rs` | `colima_vms: Vec<ColimaVm>` | `machines: Vec<Machine>` |
| `src/state/docker_state.rs` | `Selection::Machine(String)` | `Selection::Machine(MachineId)` |
| `src/ui/machines/list.rs` | Returns `&Vec<ColimaVm>` | Returns `&Vec<Machine>` |
| `src/ui/machines/detail.rs` | `machine: Option<ColimaVm>` | `machine: Option<Machine>` |
| `src/ui/machines/view.rs` | `selected_machine() -> Option<ColimaVm>` | `-> Option<Machine>` |
| `src/ui/machines/machine_dialog.rs` | `Edit(ColimaVm)` | `EditColima(ColimaVm)`, `EditHost` |
| `src/services/colima/machines.rs` | All ColimaVm functions | Add match on Machine type |
| `src/services/init.rs` | Sets colima_vms | Sets machines (host + colima) |
| `src/services/watchers/machines.rs` | Polls Colima only | Polls host + Colima |

### Terminal/Files Tabs
- Require SSH access to Colima VM
- Host has no SSH - disable these tabs for Host machines

## Implementation Plan

### Phase 0: Runtime Switching Architecture (CRITICAL)

**New Concept: Active Machine**

The app needs to track which machine (Host or Colima VM) is currently active. When switching:
1. Disconnect current Docker client
2. Connect to new machine's Docker socket
3. Refresh all Docker data
4. Update UI to show active machine

**File: `src/state/docker_state.rs`**

```rust
/// Identifies which machine is active for Docker operations
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveMachine {
  Host,                    // Native Docker daemon
  Colima(String),          // Colima VM by name
}

pub struct DockerState {
  // ... existing fields

  /// Currently active machine for Docker operations
  pub active_machine: ActiveMachine,

  /// Host machine info (always populated if Docker connected)
  pub host_machine: Option<DockerHostInfo>,
}
```

**File: `src/services/core.rs`**

Add function to switch Docker runtime:

```rust
/// Switch to a different Docker runtime
pub async fn switch_runtime(runtime: DockerRuntime) -> Result<()> {
  let client_lock = docker_client();
  let mut client_guard = client_lock.write().await;

  // Disconnect existing
  *client_guard = None;

  // Create and connect new client
  let mut new_client = DockerClient::new(runtime);
  new_client.connect().await?;

  *client_guard = Some(new_client);
  Ok(())
}
```

**File: `src/services/docker/mod.rs`**

Add service function for runtime switch:

```rust
pub fn switch_to_machine(machine: &Machine, cx: &mut App) {
  let runtime = match machine {
    Machine::Host(host) => DockerRuntime::NativeDocker {
      socket_path: host.docker_socket.clone(),
    },
    Machine::Colima(vm) => DockerRuntime::Colima {
      profile: vm.name.clone(),
    },
  };

  let state = docker_state(cx);

  cx.spawn(async move |cx| {
    // Switch runtime
    if let Err(e) = switch_runtime(runtime).await {
      // Handle error - emit event
      return;
    }

    // Refresh all data
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.active_machine = machine.to_active();
        cx.emit(StateChanged::RuntimeSwitched);
      });

      // Trigger refresh of all Docker data
      refresh_containers(cx);
      refresh_images(cx);
      refresh_volumes(cx);
      refresh_networks(cx);
    });
  }).detach();
}
```

**New Event:**

```rust
pub enum StateChanged {
  // ... existing
  RuntimeSwitched,  // Active machine changed
}
```

### Phase 1: Add Docker System Info API

**File: `src/docker/system.rs` (new)**

```rust
use bollard::system::SystemInfo;

#[derive(Debug, Clone)]
pub struct DockerHostInfo {
  pub name: String,              // Hostname
  pub os: String,                // Operating system
  pub kernel: String,            // Kernel version
  pub arch: String,              // Architecture
  pub cpus: u32,                 // CPU count
  pub memory: u64,               // Total memory bytes
  pub docker_version: String,    // Docker version
  pub api_version: String,       // API version
  pub storage_driver: String,    // overlay2, etc.
  pub docker_root: String,       // /var/lib/docker
  pub containers_total: u64,
  pub containers_running: u64,
  pub images: u64,
  pub runtime: String,           // containerd, etc.
}

impl DockerClient {
  pub async fn get_system_info(&self) -> Result<DockerHostInfo> {
    let docker = self.client()?;
    let info = docker.info().await?;
    // Map SystemInfo to DockerHostInfo
  }
}
```

**File: `src/docker/mod.rs`**
- Add `mod system;` and export `DockerHostInfo`

### Phase 2: Create Machine Enum for Host + Colima

**File: `src/colima/types.rs`**

Add a wrapper enum to represent both types:

```rust
use crate::docker::DockerHostInfo;

#[derive(Debug, Clone)]
pub enum Machine {
  Host(DockerHostInfo),
  Colima(ColimaVm),
}

impl Machine {
  /// Get unique identifier for this machine
  pub fn id(&self) -> MachineId {
    match self {
      Machine::Host(_) => MachineId::Host,
      Machine::Colima(vm) => MachineId::Colima(vm.name.clone()),
    }
  }

  pub fn name(&self) -> &str {
    match self {
      Machine::Host(h) => &h.name,
      Machine::Colima(vm) => &vm.name,
    }
  }

  pub fn is_running(&self) -> bool {
    match self {
      Machine::Host(_) => true,  // If Docker is connected, host is running
      Machine::Colima(vm) => vm.status == VmStatus::Running,
    }
  }

  pub fn is_host(&self) -> bool {
    matches!(self, Machine::Host(_))
  }

  pub fn is_colima(&self) -> bool {
    matches!(self, Machine::Colima(_))
  }

  /// Get Docker socket path for this machine
  pub fn docker_socket(&self) -> Option<String> {
    match self {
      Machine::Host(h) => Some(h.docker_socket.clone()),
      Machine::Colima(vm) => vm.docker_socket.clone(),
    }
  }

  /// Get CPUs (for display)
  pub fn cpus(&self) -> u32 {
    match self {
      Machine::Host(h) => h.cpus,
      Machine::Colima(vm) => vm.cpus,
    }
  }

  /// Get memory in bytes (for display)
  pub fn memory(&self) -> u64 {
    match self {
      Machine::Host(h) => h.memory,
      Machine::Colima(vm) => vm.memory,
    }
  }

  /// Get architecture string
  pub fn arch(&self) -> &str {
    match self {
      Machine::Host(h) => &h.arch,
      Machine::Colima(vm) => vm.arch.as_str(),
    }
  }
}

/// Machine identifier - used for selection and active tracking
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MachineId {
  Host,
  Colima(String),
}

impl MachineId {
  pub fn name(&self) -> &str {
    match self {
      MachineId::Host => "Host",
      MachineId::Colima(name) => name,
    }
  }
}
```

### Phase 3: Refactor DockerState (Full Refactor)

**File: `src/state/docker_state.rs`**

```rust
/// Machine identifier for selection - distinguishes Host from Colima
#[derive(Clone, Debug, PartialEq)]
pub enum MachineId {
  Host,
  Colima(String),  // Colima VM name
}

/// Update Selection enum
pub enum Selection {
  // ... existing variants unchanged
  Machine(MachineId),  // Changed from String to MachineId
}

pub struct DockerState {
  // REPLACE colima_vms with machines
  pub machines: Vec<Machine>,              // Was: colima_vms: Vec<ColimaVm>
  pub active_machine: Option<MachineId>,   // Currently active for Docker ops

  // Everything else unchanged
  pub containers: Vec<ContainerInfo>,
  pub images: Vec<ImageInfo>,
  pub volumes: Vec<VolumeInfo>,
  pub networks: Vec<NetworkInfo>,
  pub selection: Selection,
  pub machines_state: LoadState,
  // ... rest unchanged
}

impl DockerState {
  pub fn new() -> Self {
    Self {
      machines: Vec::new(),
      active_machine: None,
      // ... rest unchanged
    }
  }

  /// Set machines (called by refresh)
  pub fn set_machines(&mut self, machines: Vec<Machine>) {
    self.machines = machines;
    self.machines_state = LoadState::Loaded;
  }

  /// Get only Colima VMs (for Colima-specific operations)
  pub fn colima_vms(&self) -> impl Iterator<Item = &ColimaVm> {
    self.machines.iter().filter_map(|m| match m {
      Machine::Colima(vm) => Some(vm),
      _ => None,
    })
  }

  /// Get host machine if present
  pub fn host(&self) -> Option<&DockerHostInfo> {
    self.machines.iter().find_map(|m| match m {
      Machine::Host(h) => Some(h),
      _ => None,
    })
  }

  /// Find machine by ID
  pub fn get_machine(&self, id: &MachineId) -> Option<&Machine> {
    self.machines.iter().find(|m| m.id() == *id)
  }

  /// Get the active machine
  pub fn active(&self) -> Option<&Machine> {
    self.active_machine.as_ref().and_then(|id| self.get_machine(id))
  }

  /// Set active machine
  pub fn set_active(&mut self, id: MachineId) {
    self.active_machine = Some(id);
  }
}
```

### Phase 4: Update Services to Load Host Info

**File: `src/services/init.rs`**

On startup, fetch Docker host info:

```rust
// Load Docker host info
let host_info_task = Tokio::spawn(cx, async move {
  let guard = client.read().await;
  match guard.as_ref() {
    Some(docker) => docker.get_system_info().await.ok(),
    None => None,
  }
});
```

### Phase 5: Refactor MachineList (Full Refactor)

**File: `src/ui/machines/list.rs`**

```rust
pub struct MachineListDelegate {
  docker_state: Entity<DockerState>,
  search_query: String,
}

impl MachineListDelegate {
  // CHANGED: Returns &Vec<Machine> instead of &Vec<ColimaVm>
  fn machines<'a>(&self, cx: &'a App) -> &'a Vec<Machine> {
    &self.docker_state.read(cx).machines
  }

  fn filtered_machines(&self, cx: &App) -> Vec<Machine> {
    let machines = self.machines(cx);
    if self.search_query.is_empty() {
      return machines.clone();
    }

    let query = self.search_query.to_lowercase();
    machines
      .iter()
      .filter(|m| {
        m.name().to_lowercase().contains(&query)
          || m.arch().to_lowercase().contains(&query)
      })
      .cloned()
      .collect()
  }
}

impl ListDelegate for MachineListDelegate {
  type Item = ListItem;

  fn render_item(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<'_, ListState<Self>>) -> Option<Self::Item> {
    let machines = self.filtered_machines(cx);
    let machine = machines.get(ix.row)?;
    let state = self.docker_state.read(cx);
    let colors = &cx.theme().colors;

    // Check if this machine is selected (for UI highlight)
    let is_selected = matches!(&state.selection, Selection::Machine(id) if *id == machine.id());

    // Check if this machine is the active runtime
    let is_active = state.active_machine.as_ref() == Some(&machine.id());

    let is_running = machine.is_running();

    // Icon based on machine type
    let icon = if machine.is_host() {
      AppIcon::Docker
    } else {
      AppIcon::Machine
    };

    let icon_bg = if is_running { colors.primary } else { colors.muted_foreground };
    let status_color = if is_running { colors.success } else { colors.muted_foreground };

    let subtitle = format!("{}, {}",
      if machine.is_host() { "Native" } else { "Colima" },
      machine.arch()
    );

    let item_content = h_flex()
      .w_full()
      .items_center()
      .gap(px(10.))
      // Active indicator (left border)
      .when(is_active, |el| el.border_l_2().border_color(colors.link))
      // Icon
      .child(
        div()
          .size(px(36.))
          .rounded(px(8.))
          .bg(icon_bg)
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(icon).text_color(colors.background)),
      )
      // Name and subtitle
      .child(
        v_flex()
          .flex_1()
          .child(Label::new(machine.name()))
          .child(div().text_xs().text_color(colors.muted_foreground).child(subtitle)),
      )
      // Active badge
      .when(is_active, |el| {
        el.child(
          div()
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(colors.link)
            .text_xs()
            .text_color(colors.background)
            .child("Active")
        )
      })
      // Status dot
      .child(div().size(px(8.)).rounded_full().bg(status_color));

    // Context menu - different for Host vs Colima
    let menu = self.build_context_menu(machine, is_active, cx);

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .selected(is_selected)
      .child(item_content)
      .suffix(move |_, _| {
        Button::new(("menu", ix.row))
          .icon(IconName::Ellipsis)
          .ghost()
          .xsmall()
          .dropdown_menu(menu)
      });

    Some(item)
  }
}

impl MachineListDelegate {
  fn build_context_menu(&self, machine: &Machine, is_active: bool, cx: &App) -> DropdownMenu {
    match machine {
      Machine::Host(host) => self.build_host_menu(host, is_active),
      Machine::Colima(vm) => self.build_colima_menu(vm, is_active),
    }
  }

  fn build_host_menu(&self, host: &DockerHostInfo, is_active: bool) -> DropdownMenu {
    DropdownMenu::new()
      .when(!is_active, |menu| {
        menu.item(
          PopupMenuItem::new("Set as Active")
            .icon(IconName::CircleCheck)
            .on_click(move |_, _, cx| {
              services::switch_to_machine(MachineId::Host, cx);
            })
        )
        .separator()
      })
      .item(
        PopupMenuItem::new("Restart Docker")
          .icon(AppIcon::Restart)
          .on_click(move |_, _, cx| {
            services::restart_docker_daemon(cx);
          })
      )
      .item(
        PopupMenuItem::new("System Prune")
          .icon(AppIcon::Trash)
          .on_click(move |_, _, cx| {
            services::docker_system_prune(cx);
          })
      )
      .separator()
      .item(
        PopupMenuItem::new("Configure")
          .icon(AppIcon::Settings)
          .on_click(move |_, _, cx| {
            services::open_host_config(cx);
          })
      )
  }

  fn build_colima_menu(&self, vm: &ColimaVm, is_active: bool) -> DropdownMenu {
    // Existing Colima menu logic - Start/Stop/Restart/Terminal/Files/K8s/Edit/Delete
    // Add "Set as Active" at top if not active
    // ... (keep existing implementation)
  }
}
```

### Phase 6: Update MachineDetail for Host

**File: `src/ui/machines/detail.rs`**

Add host-specific rendering:

**Info Tab for Host:**
- Docker version, API version
- OS, Kernel, Architecture
- CPU count, Total memory
- Storage driver, Docker root
- Container/Image counts

**Config Tab for Host:**
- Show Docker daemon config (read from daemon.json if accessible)
- Display current settings in read-only view
- Button to open "Configure" dialog

**Stats Tab for Host:**
- Memory usage (from Docker info or system)
- Disk usage of Docker root

**Logs Tab for Host:**
- Linux: `journalctl -u docker`
- macOS: `/var/log/docker.log` or via `log show --predicate 'subsystem == "com.docker"'`
- Windows/WSL2: Logs from Docker daemon in WSL2 distro

**Tabs to Disable for Host:**
- Terminal (no SSH - it's the local machine)
- Files (no VM filesystem to browse)
- Processes (could show Docker daemon processes but less relevant)

```rust
fn render_info_tab(&self, machine: &Machine, cx: &App) -> impl IntoElement {
  match machine {
    Machine::Host(host) => self.render_host_info(host, cx),
    Machine::Colima(vm) => self.render_colima_info(vm, cx),
  }
}

fn available_tabs_for_machine(machine: &Machine) -> Vec<MachineDetailTab> {
  match machine {
    Machine::Host(_) => vec![
      MachineDetailTab::Info,
      MachineDetailTab::Config,
      MachineDetailTab::Stats,
      MachineDetailTab::Logs,
    ],
    Machine::Colima(_) => MachineDetailTab::ALL.to_vec(),
  }
}
```

### Phase 6.5: Host Machine Actions & Context Menu

**Current Colima VM Actions:**
```
Running:
├─ Set as Default
├─ Stop / Restart
├─ Update Runtime
├─ Terminal / Files
├─ K8s Start/Stop/Reset (or Enable K8s)
├─ Edit
└─ Delete

Stopped:
├─ Start
├─ Edit
└─ Delete
```

**Host Machine Actions (new):**
```
├─ Set as Active          <- Switch to this runtime
├─ Restart Docker         <- systemctl restart docker (Linux) / launchctl (macOS)
├─ View Logs              <- Open Logs tab
├─ System Prune           <- docker system prune -a
├─ Configure              <- Open Host settings dialog
└─ (No Delete - can't delete host)
```

**File: `src/ui/machines/list.rs`**

Add host-specific context menu:

```rust
fn render_host_menu(host: &DockerHostInfo, cx: &mut Context) -> DropdownMenu {
  DropdownMenu::new()
    .item(
      PopupMenuItem::new("Set as Active")
        .icon(IconName::CircleCheck)
        .on_click(move |_, _, cx| {
          services::switch_to_host(cx);
        }),
    )
    .separator()
    .item(
      PopupMenuItem::new("Restart Docker")
        .icon(AppIcon::Restart)
        .on_click(move |_, _, cx| {
          services::restart_docker_daemon(cx);
        }),
    )
    .item(
      PopupMenuItem::new("View Logs")
        .icon(AppIcon::Logs)
        .on_click(move |_, _, cx| {
          services::open_host_logs(cx);
        }),
    )
    .separator()
    .item(
      PopupMenuItem::new("System Prune")
        .icon(AppIcon::Trash)
        .on_click(move |_, _, cx| {
          services::docker_system_prune(cx);
        }),
    )
    .separator()
    .item(
      PopupMenuItem::new("Configure")
        .icon(AppIcon::Settings)
        .on_click(move |_, _, cx| {
          services::open_host_config(cx);
        }),
    )
}
```

### Phase 6.6: Host Configuration Dialog

**File: `src/ui/machines/host_dialog.rs` (new)**

Create a simpler dialog for Host Docker configuration:

**Tabs (subset of Colima dialog):**

1. **Info Tab** (read-only):
   - Docker Version, API Version
   - OS, Kernel, Architecture
   - CPUs, Memory (system totals)
   - Storage Driver, Docker Root
   - Container/Image counts

2. **Docker Tab** (editable):
   - BuildKit enabled (DOCKER_BUILDKIT=1)
   - Insecure Registries (list)
   - Registry Mirrors (list)
   - Live Restore (bool)
   - Experimental Features (bool)

3. **Network Tab** (editable):
   - DNS Servers (list)
   - Default Address Pools
   - IP Forwarding (bool)
   - IPv6 (bool)

4. **Storage Tab** (mostly read-only):
   - Current Storage Driver (read-only)
   - Docker Root Path (read-only)
   - Storage Usage (with Prune button)

**Comparison: Colima vs Host Settings**

| Colima VM Tab | Host Equivalent | Notes |
|---------------|-----------------|-------|
| Basic | Info (read-only) | Can't change host CPUs/memory |
| Runtime | N/A | Docker is the runtime |
| VM | N/A | No VM layer |
| Storage | Storage (read-only) | Can't change driver on running daemon |
| Network | Network | DNS, address pools |
| Kubernetes | N/A | Use Colima for K8s |
| Env | N/A | Less common for host |
| Docker | Docker | Same settings apply |
| Provision | N/A | No provision scripts |

**daemon.json Location:**
- Linux: `/etc/docker/daemon.json`
- macOS (via Colima): `~/.colima/<profile>/docker/daemon.json` or configure via Colima template
- Windows/WSL2: `/etc/docker/daemon.json` inside WSL2 distro

**Configuration Notes by Platform:**

**Linux (Host machine):**
- Direct daemon.json editing at `/etc/docker/daemon.json`
- Restart via `systemctl restart docker`
- May require sudo for write access

**macOS (Colima VM):**
- Docker runs inside the Colima VM, NOT on the host
- Configure via Colima template (`~/.colima/_templates/default.yaml`)
- Or SSH into VM: `colima ssh -- sudo vim /etc/docker/daemon.json`
- Restart: `colima ssh -- sudo systemctl restart docker`
- For most settings, use Colima's native configuration (already in machine dialog)

**Windows/WSL2:**
- Docker runs inside WSL2 distro
- daemon.json at `/etc/docker/daemon.json` inside WSL2
- Access via `wsl -d <distro> -e cat /etc/docker/daemon.json`
- Restart: `wsl -d <distro> -e sudo systemctl restart docker`

### Phase 7: Add Settings for Colima Enable

**File: `src/state/settings.rs`**

```rust
pub struct Settings {
  // Existing...
  pub colima_enabled: bool,  // Enable Colima VM management
}
```

**File: `src/ui/settings/view.rs`**

Add toggle in Colima section:

```rust
// Colima section
fn render_colima_section(&self, ...) {
  v_flex()
    .child(section_header("Colima"))
    .child(
      setting_row("Enable Colima", "Manage Docker VMs with Colima")
        .child(Switch::new("colima_enabled").checked(settings.colima_enabled))
    )
    // Only show these when enabled:
    .when(settings.colima_enabled, |el| {
      el.child(setting_row("Default Profile", ...))
        .child(setting_row("Cache Size", ...))
        .child(setting_row("Default Template", ...))
    })
}
```

### Phase 8: Platform-Specific Defaults

**File: `src/state/settings.rs`**

```rust
impl Default for Settings {
  fn default() -> Self {
    let platform = Platform::detect();

    Self {
      // On macOS, Colima is required (no native Docker)
      // On Linux, native Docker is primary, Colima is optional
      // On Windows/WSL2, Docker in WSL2 is primary
      colima_enabled: matches!(platform, Platform::MacOS),
      // ...
    }
  }
}
```

**Platform Behavior:**

| Platform | Host Machine | Colima VMs | Default Active |
|----------|-------------|------------|----------------|
| **Linux** | Native Docker (`/var/run/docker.sock`) | Optional | Host |
| **macOS** | N/A (no native Docker) | Required | First Colima VM |
| **Windows/WSL2** | Docker in WSL2 | Optional | Host (WSL2) |

**Note:** On macOS, there is no "Host" machine because Docker cannot run natively on macOS - it always needs a Linux VM (Colima). The Colima VMs ARE the machines on macOS.

### Phase 9: Update Sidebar to Show Active Machine

**File: `src/app.rs`**

Show active machine name in sidebar header or status area:

```rust
// In sidebar, show current runtime
.child(
  h_flex()
    .child(Icon::new(AppIcon::Docker))
    .child(div().text_xs().child(format!("Runtime: {}", active_machine_name)))
)
```

Or add to the Docker section header:

```rust
SidebarGroup::new(format!("Docker ({})", active_machine_name))
```

### Phase 6.7: Host Services

**File: `src/services/host.rs` (new)**

```rust
/// Restart the Docker daemon
pub fn restart_docker_daemon(cx: &mut App) {
  let task_id = start_task(cx, "Restarting Docker daemon...".to_string());

  cx.spawn(async move |cx| {
    let result = match Platform::detect() {
      Platform::Linux => {
        // systemctl restart docker
        Command::new("systemctl")
          .args(["restart", "docker"])
          .output()
          .await
      }
      Platform::MacOS => {
        // On macOS, Docker runs inside Colima VM
        // Restart Docker daemon inside the VM via colima ssh
        Command::new("colima")
          .args(["ssh", "--", "sudo", "systemctl", "restart", "docker"])
          .output()
          .await
      }
      _ => Err(anyhow!("Unsupported platform")),
    };
    // Handle result, refresh data after restart
  }).detach();
}

/// Perform docker system prune
pub fn docker_system_prune(cx: &mut App) {
  let client = docker_client();
  cx.spawn(async move |cx| {
    let guard = client.read().await;
    if let Some(docker) = guard.as_ref() {
      // Use bollard's prune APIs
      docker.prune_containers(None).await?;
      docker.prune_images(None).await?;
      docker.prune_volumes(None).await?;
      docker.prune_networks(None).await?;
    }
    // Refresh all data after prune
  }).detach();
}

/// Read Docker daemon configuration
pub async fn read_daemon_config() -> Result<DaemonConfig> {
  let path = daemon_json_path();
  if path.exists() {
    let content = tokio::fs::read_to_string(&path).await?;
    serde_json::from_str(&content)
  } else {
    Ok(DaemonConfig::default())
  }
}

/// Write Docker daemon configuration
pub async fn write_daemon_config(config: &DaemonConfig) -> Result<()> {
  let path = daemon_json_path();
  let content = serde_json::to_string_pretty(config)?;
  tokio::fs::write(&path, content).await?;
  Ok(())
}

fn daemon_json_path(active_machine: &ActiveMachine) -> PathBuf {
  match active_machine {
    ActiveMachine::Host => {
      // Native Docker on Linux
      PathBuf::from("/etc/docker/daemon.json")
    }
    ActiveMachine::Colima(profile) => {
      // Colima VM - config is in the VM, use colima template instead
      // Or access via: colima ssh -- cat /etc/docker/daemon.json
      dirs::home_dir()
        .unwrap_or_default()
        .join(format!(".colima/{}/colima.yaml", profile))
    }
  }
}
```

**File: `src/docker/daemon_config.rs` (new)**

```rust
/// Docker daemon.json configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DaemonConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub storage_driver: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub data_root: Option<String>,

  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub insecure_registries: Vec<String>,

  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub registry_mirrors: Vec<String>,

  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub dns: Vec<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub live_restore: Option<bool>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub experimental: Option<bool>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub ipv6: Option<bool>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub ip_forward: Option<bool>,

  // Preserve unknown fields
  #[serde(flatten)]
  pub other: serde_json::Map<String, serde_json::Value>,
}
```

## Files to Create/Modify

### Core Types & API (Foundation)

| File | Action | Changes |
|------|--------|---------|
| `src/docker/system.rs` | Create | DockerHostInfo struct, get_system_info() |
| `src/docker/daemon_config.rs` | Create | DaemonConfig struct for daemon.json |
| `src/docker/mod.rs` | Modify | Add system, daemon_config modules |
| `src/colima/types.rs` | Modify | Add Machine enum, MachineId enum |

### State Management

| File | Action | Changes |
|------|--------|---------|
| `src/state/docker_state.rs` | Modify | `machines: Vec<Machine>`, `active_machine: Option<MachineId>`, `Selection::Machine(MachineId)` |
| `src/state/settings.rs` | Modify | Add colima_enabled setting |

### Services (CRITICAL - must preserve Host when updating Colima list)

| File | Action | Changes |
|------|--------|---------|
| `src/services/core.rs` | Modify | Add switch_runtime() function |
| `src/services/docker/mod.rs` | Modify | Add switch_to_machine() service |
| `src/services/host.rs` | Create | Host-specific services (restart, prune, config) |
| `src/services/init.rs` | Modify | Load host info, combine Host + Colima VMs into machines list |
| `src/services/colima/machines.rs` | Modify | **CRITICAL**: All `set_machines(vms)` must preserve Host machine - use `set_colima_vms(vms)` helper |
| `src/services/watchers/machines.rs` | Modify | Update to handle Machine enum, preserve Host |

### UI - Machines View

| File | Action | Changes |
|------|--------|---------|
| `src/ui/machines/list.rs` | Modify | Render Machine enum, host menu, active indicator |
| `src/ui/machines/detail.rs` | Modify | Host-specific tabs, available tabs per machine type |
| `src/ui/machines/view.rs` | Modify | Handle Machine enum instead of ColimaVm |
| `src/ui/machines/machine_dialog.rs` | Modify | `EditColima(ColimaVm)`, separate host handling |
| `src/ui/machines/host_dialog.rs` | Create | Host configuration dialog |

### UI - Other Views (CRITICAL - these reference colima_vms directly)

| File | Action | Changes |
|------|--------|---------|
| `src/ui/pods/list.rs` | Modify | `state.colima_vms` → `state.colima_vms()` helper method |
| `src/ui/services/list.rs` | Modify | `state.colima_vms` → `state.colima_vms()` helper method |
| `src/ui/deployments/list.rs` | Modify | `state.colima_vms` → `state.colima_vms()` helper method |
| `src/ui/global_search.rs` | Modify | Search through `machines`, use `Selection::Machine(MachineId)` |
| `src/ui/settings/view.rs` | Modify | Colima enable toggle |

### App Level (CRITICAL - keyboard shortcuts)

| File | Action | Changes |
|------|--------|---------|
| `src/app.rs` | Modify | Handle `Selection::Machine(MachineId)` - dispatch to correct operation based on Host vs Colima |

### Process View (Host processes support)

| File | Action | Changes |
|------|--------|---------|
| `src/ui/components/process_view.rs` | Modify | Add `ProcessSource::Host` variant for native Docker host processes |

## Critical Implementation Details

### 1. Preserving Host When Updating Colima List

Every Colima operation calls `set_machines(vms)` with only Colima VMs. This would wipe out Host.

**Solution**: Add helper methods to DockerState:

```rust
impl DockerState {
  /// Update only Colima VMs, preserving Host machine
  pub fn set_colima_vms(&mut self, vms: Vec<ColimaVm>) {
    // Keep existing host, replace Colima VMs
    let host = self.machines.iter()
      .find(|m| m.is_host())
      .cloned();

    self.machines = vms.into_iter()
      .map(Machine::Colima)
      .collect();

    // Re-add host at beginning
    if let Some(h) = host {
      self.machines.insert(0, h);
    }

    self.machines_state = LoadState::Loaded;
  }

  /// Set all machines (used during init)
  pub fn set_machines(&mut self, machines: Vec<Machine>) {
    self.machines = machines;
    self.machines_state = LoadState::Loaded;
  }
}
```

### 2. Keyboard Shortcuts in app.rs

Current code:
```rust
Selection::Machine(name) => {
  crate::services::start_machine(name, cx);  // Colima-specific!
}
```

**Solution**: Check machine type before dispatching:

```rust
Selection::Machine(id) => {
  match id {
    MachineId::Host => {
      // Host doesn't support start/stop via this path
      // Show notification: "Host Docker is managed by system"
    }
    MachineId::Colima(name) => {
      crate::services::start_machine(name, cx);
    }
  }
}
```

### 3. Global Search

Current code searches `colima_vms` and creates `Selection::Machine(String)`.

**Solution**:
```rust
// Search machines (both Host and Colima)
for machine in &state.machines {
  if query.is_empty() || machine.name().to_lowercase().contains(&query_lower) {
    results.push(SearchResult {
      result_type: SearchResultType::Machine,
      name: machine.name().to_string(),
      subtitle: match machine {
        Machine::Host(h) => format!("Docker {} - {} CPU, {}", h.docker_version, h.cpus, format_size(h.memory)),
        Machine::Colima(vm) => format!("{:?} - {} CPU, {}", vm.status, vm.cpus, format_size(vm.memory)),
      },
      selection: Selection::Machine(machine.id()),
    });
  }
}
```

### 4. ProcessSource for Host

Add variant for host processes:

```rust
pub enum ProcessSource {
  ColimaVm { profile: Option<String> },
  DockerContainer { container_id: String },
  Host,  // NEW: For host system processes
}

// In fetch logic:
ProcessSource::Host => {
  // Run `ps aux` on local system
  let output = std::process::Command::new("ps")
    .args(["aux"])
    .output()?;
  String::from_utf8_lossy(&output.stdout).to_string()
}
```

### 5. K8s Views Using colima_vms

The K8s views (pods, services, deployments) check for Colima VMs with K8s enabled.

**Solution**: Use the `colima_vms()` helper:

```rust
// Before:
let running_vm_without_k8s = state.colima_vms.iter()
  .find(|vm| vm.status.is_running() && !vm.kubernetes);

// After:
let running_vm_without_k8s = state.colima_vms()
  .find(|vm| vm.status.is_running() && !vm.kubernetes);
```

### 6. StateChanged Events Need MachineId

Current events use String machine_name:
```rust
StateChanged::MachineTabRequest { machine_name: String, tab: MachineDetailTab }
StateChanged::EditMachineRequest { machine_name: String }
```

**Solution**: Change to MachineId:
```rust
StateChanged::MachineTabRequest { machine_id: MachineId, tab: MachineDetailTab }
StateChanged::EditMachineRequest { machine_id: MachineId }
```

This is type-safe and handles both Host and Colima machines correctly.

### 7. Services Called from List Context Menus

Current machine list context menus call services by name:
```rust
services::start_machine(name.clone(), cx);
services::stop_machine(name.clone(), cx);
services::restart_machine(name.clone(), cx);
services::delete_machine(name.clone(), cx);
```

These are Colima-specific. For Host machine:
- **No start/stop**: Host Docker is managed by system (systemctl)
- **Restart Docker**: Different command - `systemctl restart docker`
- **No delete**: Can't delete Host

**Solution**: Services need to check machine type or use separate functions:
```rust
// Host-specific
services::restart_docker_daemon(cx);  // For Host
services::docker_system_prune(cx);    // For Host

// Colima-specific (existing)
services::start_machine(name, cx);    // For Colima
services::stop_machine(name, cx);     // For Colima
```

## Implementation Order

### Phase 0: Foundation (No Breaking Changes)
1. Create `src/docker/system.rs` - DockerHostInfo struct
2. Create `src/docker/daemon_config.rs` - DaemonConfig struct
3. Add Machine enum and MachineId enum to `src/colima/types.rs`

### Phase 1: State Management Refactor
4. Update `src/state/docker_state.rs`:
   - Change `colima_vms: Vec<ColimaVm>` → `machines: Vec<Machine>`
   - Change `Selection::Machine(String)` → `Selection::Machine(MachineId)`
   - Add `active_machine: Option<MachineId>`
   - Add helper methods: `colima_vms()`, `host()`, `set_colima_vms()`

### Phase 2: Service Layer Updates (CRITICAL)
5. Update `src/services/init.rs`:
   - Fetch Docker system info
   - Create Host machine from DockerHostInfo
   - Combine Host + Colima VMs into machines list
6. Update `src/services/colima/machines.rs`:
   - Change ALL `set_machines(vms)` → `set_colima_vms(vms)`
   - This preserves Host machine during Colima operations
7. Update `src/services/watchers/machines.rs`:
   - Handle Machine enum
   - Preserve Host during Colima polling
8. Add `src/services/core.rs` switch_runtime() function
9. Create `src/services/host.rs` for Host-specific operations
10. Update `src/services/navigation.rs`:
    - `open_machine_terminal(name)` → `open_machine_terminal(id: MachineId)`
    - `open_machine_files(name)` → `open_machine_files(id: MachineId)`
    - These should validate machine type (Terminal/Files are Colima-only)

### Phase 3: UI Machines View Refactor
10. Update `src/ui/machines/list.rs`:
    - Change `MachineListEvent::Selected(ColimaVm)` → `Selected(Machine)`
    - Render Machine enum with different icons/menus for Host vs Colima
    - Add active machine indicator

11. Update `src/ui/machines/view.rs` (EXTENSIVE CHANGES):
    - `selected_machine()` returns `Option<Machine>` instead of `Option<ColimaVm>`
    - Look up by `MachineId` instead of String name
    - `MachineTabRequest` and `EditMachineRequest` events:
      - Current: `machine_name: String` - finds in `colima_vms` by name
      - Change to: `machine_id: MachineId` - finds in `machines` by id
    - `on_select_machine(&Machine)` instead of `(&ColimaVm)`
    - `load_machine_data()`:
      - For Colima: Existing ColimaClient calls (SSH to VM for OS info, logs, files)
      - For Host: DockerClient calls + local system calls for logs
    - Terminal tab:
      - Colima: SSH terminal via `TerminalView::for_colima(profile)`
      - Host: DISABLED (no SSH to local machine, would be confusing)
    - Process tab:
      - Colima: SSH ps aux via ProcessView::for_colima(profile)
      - Host: Local ps aux via ProcessView::for_host()
    - Files tab:
      - Colima: SSH file listing via ColimaClient
      - Host: DISABLED (no VM filesystem to browse)
    - Logs tab:
      - Colima: journalctl via SSH
      - Host: journalctl -u docker locally or Docker daemon logs

12. Update `src/ui/machines/detail.rs`:
    - `machine: Option<Machine>` instead of `Option<ColimaVm>`
    - `available_tabs(machine: &Machine)`:
      ```rust
      match machine {
        Machine::Host(_) => vec![Info, Config, Stats, Logs],
        Machine::Colima(_) => MachineDetailTab::ALL.to_vec(),
      }
      ```
    - Render different content based on machine type
    - Host Info: Docker version, API version, OS, kernel, arch, CPU, memory, storage
    - Colima Info: Existing VM info display

13. Update `src/ui/machines/machine_dialog.rs`:
    - Rename to be Colima-only: `ColimaVmDialog`
    - Or keep name but only accept `ColimaVm` (validation that Host can't be edited here)

14. Create `src/ui/machines/host_dialog.rs`:
    - Tabs: Info (read-only), Docker, Network, Storage
    - Reads/writes daemon.json

### Phase 4: Other UI Updates (CRITICAL - easy to miss)
15. Update `src/ui/pods/list.rs`:
    - `state.colima_vms` → `state.colima_vms()`
16. Update `src/ui/services/list.rs`:
    - `state.colima_vms` → `state.colima_vms()`
17. Update `src/ui/deployments/list.rs`:
    - `state.colima_vms` → `state.colima_vms()`
18. Update `src/ui/global_search.rs`:
    - Search through `state.machines`
    - Use `Selection::Machine(machine.id())`
19. Update `src/app.rs`:
    - Handle `Selection::Machine(MachineId)` in keyboard shortcuts
    - Dispatch to Host vs Colima operations appropriately

### Phase 5: Process View & Settings
20. Update `src/ui/components/process_view.rs`:
    - Add `ProcessSource::Host` variant
21. Update `src/state/settings.rs`:
    - Add `colima_enabled: bool`
22. Update `src/ui/settings/view.rs`:
    - Add Colima enable toggle

### Phase 6: Final Integration
23. Platform defaults (macOS: Colima required, Linux: Host primary)
24. Sidebar active machine display
25. Testing and edge case handling

## Runtime Switching Flow

```
User clicks "Use" on a machine
         │
         ▼
┌─────────────────────────┐
│ switch_to_machine()     │
│ called with Machine     │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Get DockerRuntime from  │
│ Machine (socket path)   │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ switch_runtime()        │
│ - Disconnect old client │
│ - Connect new client    │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Update DockerState      │
│ - Set active_machine    │
│ - Emit RuntimeSwitched  │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Refresh all Docker data │
│ - Containers            │
│ - Images                │
│ - Volumes               │
│ - Networks              │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ UI updates              │
│ - Machine list shows    │
│   new active            │
│ - All views show new    │
│   runtime's data        │
│ - Sidebar shows active  │
└─────────────────────────┘
```

## UI Mockup

### Machines List (Linux with native Docker, Colima disabled)
```
┌─────────────────────────────┐
│ Machines                    │
├─────────────────────────────┤
│ 🖥️ arch-linux        ● Running │
│    Docker 27.0.3            │
└─────────────────────────────┘
```

### Machines List (Linux with Colima enabled)
```
┌─────────────────────────────┐
│ Machines                    │
├─────────────────────────────┤
│ 🖥️ arch-linux        ● Running │  <- Host
│    Docker 27.0.3            │
│ 📦 default           ● Running │  <- Colima VM
│    Docker (Colima)          │
│ 📦 k8s-dev           ○ Stopped │  <- Colima VM
│    Containerd               │
└─────────────────────────────┘
```

### Host Machine Info Tab
```
┌─────────────────────────────────────────┐
│ arch-linux                    ● Running │
├─────────────────────────────────────────┤
│ Docker Version    27.0.3                │
│ API Version       1.46                  │
│ OS                Arch Linux            │
│ Kernel            6.18.6-arch1-1        │
│ Architecture      x86_64                │
│ CPUs              16                    │
│ Memory            32 GB                 │
│ Storage Driver    overlay2              │
│ Docker Root       /var/lib/docker       │
├─────────────────────────────────────────┤
│ Containers        12 (5 running)        │
│ Images            47                    │
└─────────────────────────────────────────┘
```

### Settings Page - Colima Section
```
┌─────────────────────────────────────────┐
│ Colima                                  │
├─────────────────────────────────────────┤
│ Enable Colima VM Management    [  ON ] │
│ Create and manage Docker VMs           │
├─────────────────────────────────────────┤
│ Default Profile     [ default     ▼]   │
│ Cache Size          1.2 GB    [Prune]  │
│ Default Template    [Edit]              │
└─────────────────────────────────────────┘
```

## Testing Checklist

### Machine Display
- [ ] Linux: Host machine shows with native Docker info
- [ ] Linux: Colima disabled by default, only Host shown
- [ ] Linux: Enable Colima shows VMs alongside Host
- [ ] macOS: Only Colima VMs shown (no native Docker on macOS)
- [ ] macOS: First Colima VM is default active
- [ ] Windows/WSL2: Host shows Docker in WSL2 info
- [ ] Host Info tab displays all Docker system info
- [ ] Host Stats tab shows memory/disk usage
- [ ] Host shows only relevant tabs (Info, Config, Stats, Logs)
- [ ] Host does NOT show Terminal/Files/Processes tabs

### Runtime Switching
- [ ] Active machine has visual indicator (highlight, badge)
- [ ] Click "Use" button switches to that machine
- [ ] Context menu "Set as Active" works
- [ ] Switching shows loading state
- [ ] After switch, Containers view shows new runtime's containers
- [ ] After switch, Images view shows new runtime's images
- [ ] After switch, Volumes view shows new runtime's volumes
- [ ] After switch, Networks view shows new runtime's networks
- [ ] Sidebar shows current active machine name
- [ ] Switching to stopped Colima VM shows error or prompts to start

### Host Machine Actions
- [ ] "Set as Active" switches to Host runtime
- [ ] "Restart Docker" restarts the Docker daemon
- [ ] "View Logs" opens Logs tab with Docker daemon logs
- [ ] "System Prune" clears unused Docker resources
- [ ] "Configure" opens Host configuration dialog
- [ ] No "Start/Stop" actions (Host is always running if Docker is)
- [ ] No "Delete" action for Host

### Host Configuration Dialog
- [ ] Info tab shows read-only Docker/system info
- [ ] Docker tab allows editing insecure registries
- [ ] Docker tab allows editing registry mirrors
- [ ] Network tab allows editing DNS servers
- [ ] Save applies changes to daemon.json
- [ ] Save prompts/triggers Docker restart if needed
- [ ] Cancel discards changes
- [ ] daemon.json preserved (unknown fields kept)

### Settings
- [ ] Settings toggle persists and takes effect immediately
- [ ] Enabling Colima refreshes machine list
- [ ] Disabling Colima hides VMs, keeps Host

### Edge Cases
- [ ] Switching while operations in progress (graceful handling)
- [ ] Network error during switch (rollback or error message)
- [ ] Starting a Colima VM and switching to it
- [ ] Stopping active Colima VM (switch to Host or show warning)
- [ ] daemon.json doesn't exist - use defaults
- [ ] daemon.json not writable - show permission error
- [ ] Docker daemon restart fails - show error, don't lose config
