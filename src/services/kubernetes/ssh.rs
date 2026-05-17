//! Phase 4b — SSH-based remote node provisioning.
//!
//! Auth is delegated to the system `ssh` binary (agent / `~/.ssh/config`
//! / explicit identity file) so we never handle private keys or
//! passwords. Every remote command is appended to
//! `<config dir>/audit.log`. Provisioning is distro-aware (k3s / kubeadm)
//! and only runs after an explicit confirmation in the UI.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::{Result, anyhow};
use gpui::App;

use crate::kubernetes::{Distro, join_guide};
use crate::platform::get_config_dir;
use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{HostEntry, SettingsChanged, StateChanged, docker_state, settings_state};

use super::super::core::{DispatcherEvent, dispatcher};

// ---- host inventory (persisted per kubeconfig context) ----------------------

/// Hosts registered for `context`.
#[must_use]
pub fn hosts_for(context: &str, cx: &App) -> Vec<HostEntry> {
  settings_state(cx)
    .read(cx)
    .settings
    .cluster_hosts
    .get(context)
    .cloned()
    .unwrap_or_default()
}

pub fn add_host(context: String, host: HostEntry, cx: &mut App) {
  settings_state(cx).update(cx, |s, cx| {
    let list = s.settings.cluster_hosts.entry(context).or_default();
    if let Some(slot) = list.iter_mut().find(|h| h.name == host.name) {
      *slot = host;
    } else {
      list.push(host);
    }
    if let Err(e) = s.settings.save() {
      tracing::warn!("Failed to persist cluster_hosts: {e}");
    }
    cx.emit(SettingsChanged::SettingsUpdated);
  });
}

pub fn remove_host(context: &str, name: &str, cx: &mut App) {
  settings_state(cx).update(cx, |s, cx| {
    if let Some(list) = s.settings.cluster_hosts.get_mut(context) {
      list.retain(|h| h.name != name);
    }
    if let Err(e) = s.settings.save() {
      tracing::warn!("Failed to persist cluster_hosts: {e}");
    }
    cx.emit(SettingsChanged::SettingsUpdated);
  });
}

// ---- ssh execution + audit --------------------------------------------------

fn audit(host: &HostEntry, command: &str) {
  let line = format!(
    "{}  {}@{}:{}  ::  {command}\n",
    chrono::Utc::now().to_rfc3339(),
    host.user,
    host.address,
    host.port
  );
  let path = get_config_dir().join("audit.log");
  if let Some(dir) = path.parent() {
    let _ = std::fs::create_dir_all(dir);
  }
  if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
    let _ = f.write_all(line.as_bytes());
  }
}

fn ssh_argv(host: &HostEntry) -> Vec<String> {
  let mut a = vec![
    "-o".into(),
    "BatchMode=yes".into(),
    "-o".into(),
    "StrictHostKeyChecking=accept-new".into(),
    "-o".into(),
    "ConnectTimeout=10".into(),
    "-p".into(),
    host.port.to_string(),
  ];
  if !host.identity_file.trim().is_empty() {
    a.push("-i".into());
    a.push(host.identity_file.clone());
  }
  a.push(format!("{}@{}", host.user, host.address));
  a
}

/// Run `remote` on `host`, returning combined stdout. Audited.
async fn ssh_capture(host: &HostEntry, remote: &str) -> Result<String> {
  audit(host, remote);
  let out = tokio::process::Command::new("ssh")
    .args(ssh_argv(host))
    .arg(remote)
    .output()
    .await
    .map_err(|e| anyhow!("failed to launch ssh (is it on PATH?): {e}"))?;
  if out.status.success() {
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
  } else {
    let mut msg = String::new();
    let _ = write!(msg, "{}", String::from_utf8_lossy(&out.stderr).trim());
    if msg.is_empty() {
      msg = format!("ssh exited with status {}", out.status);
    }
    Err(anyhow!(msg))
  }
}

// ---- provisioning -----------------------------------------------------------

fn server_url(cx: &App) -> String {
  let st = docker_state(cx);
  let s = st.read(cx);
  let current = s.current_kube_context_name();
  s.kube_contexts
    .iter()
    .find(|c| Some(c.name.as_str()) == current.as_deref() || c.is_current)
    .map(|c| c.server.clone())
    .unwrap_or_default()
}

/// SSH into `control_plane` to obtain a join credential, then into
/// `target` to install + join it as `role` ("worker" | "control-plane").
/// k3s and kubeadm only; other distros are rejected with guidance.
pub fn provision_node(target: HostEntry, control_plane: HostEntry, role: String, cx: &mut App) {
  let state = docker_state(cx);
  let nodes = state.read(cx).nodes.clone();
  let server = server_url(cx);

  // Idempotency: refuse if a Ready node already matches the target.
  let already = nodes.iter().any(|n| {
    (n.name == target.name || n.internal_ip.as_deref() == Some(target.address.as_str())) && n.status == "Ready"
  });
  if already {
    dispatcher(cx).update(cx, |_, cx| {
      cx.emit(DispatcherEvent::TaskFailed {
        error: format!("'{}' already looks joined and Ready — skipping.", target.name),
      });
    });
    return;
  }

  let distro = join_guide(&nodes, &server).distro;

  // Decide the credential-fetch command up front so an unsupported
  // distro fails immediately, while we still hold `&mut App`.
  let is_k3s = matches!(distro, Distro::K3s | Distro::K3d);
  let fetch = match distro {
    Distro::K3s | Distro::K3d => "sudo cat /var/lib/rancher/k3s/server/node-token".to_string(),
    Distro::Kubeadm | Distro::Unknown => "sudo kubeadm token create --print-join-command".to_string(),
    other => {
      dispatcher(cx).update(cx, |_, cx| {
        cx.emit(DispatcherEvent::TaskFailed {
          error: format!(
            "SSH provisioning supports k3s/kubeadm only; detected {}. Use the copy-paste guide instead.",
            other.label()
          ),
        });
      });
      return;
    }
  };

  let label = target.name.clone();
  let task_id = start_task(cx, format!("Provisioning '{label}' as {role}…"));

  // Whole orchestration runs on one Tokio task (two sequential ssh
  // round-trips); the GPUI side just finalizes the task.
  let work = Tokio::spawn(cx, async move {
    let cred = ssh_capture(&control_plane, &fetch)
      .await
      .map_err(|e| format!("could not get credential from {}: {e}", control_plane.name))?
      .trim()
      .to_string();

    let install = if is_k3s {
      if role == "control-plane" {
        format!("curl -sfL https://get.k3s.io | K3S_TOKEN='{cred}' sh -s - server --server {server}")
      } else {
        format!("curl -sfL https://get.k3s.io | K3S_URL={server} K3S_TOKEN='{cred}' sh -")
      }
    } else if role == "control-plane" {
      format!("sudo {cred} --control-plane")
    } else {
      format!("sudo {cred}")
    };

    ssh_capture(&target, &install)
      .await
      .map(|_| ())
      .map_err(|e| format!("provision failed on {}: {e}", target.name))
  });

  cx.spawn(async move |cx| {
    let result = work.await.unwrap_or_else(|e| Err(format!("{e}")));
    let _ = cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        dispatcher(cx).update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("'{label}' provisioned — it should join shortly"),
          });
        });
        super::cluster::refresh_nodes(cx);
        docker_state(cx).update(cx, |_s, cx| cx.emit(StateChanged::NodesUpdated));
      }
      Err(e) => fail_task(cx, task_id, e),
    });
  })
  .detach();
}
