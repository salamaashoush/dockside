//! Docker Compose operations

use gpui::App;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;

use crate::services::{complete_task, fail_task, start_task};
use crate::terminal::LogStream;
use crate::utils::docker_cmd;

use super::super::core::{DispatcherEvent, dispatcher};
use super::containers::refresh_containers;

/// Build a `docker compose` invocation for `project_name`, prefixing
/// `-f <config>` for every known compose file and chdir-ing into the
/// project's working dir if available. Without these the daemon's
/// `docker compose -p <name>` lookup fails with "no configuration file
/// provided" because compose can't locate the YAML from a project name
/// alone.
fn compose_invocation(project_name: &str, working_dir: Option<&str>, config_files: &[String]) -> Command {
  let mut cmd = docker_cmd();
  if let Some(dir) = working_dir {
    cmd.current_dir(dir);
  }
  cmd.arg("compose");
  for f in config_files {
    cmd.args(["-f", f]);
  }
  cmd.args(["-p", project_name]);
  cmd
}

pub fn compose_up(project_name: String, working_dir: Option<String>, config_files: Vec<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Starting '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let working_dir = working_dir.clone();
    let config_files = config_files.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        let output = compose_invocation(&project, working_dir.as_deref(), &config_files)
          .args(["up", "-d"])
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Started '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn compose_down(project_name: String, working_dir: Option<String>, config_files: Vec<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Stopping '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let working_dir = working_dir.clone();
    let config_files = config_files.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        let output = compose_invocation(&project, working_dir.as_deref(), &config_files)
          .arg("down")
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Stopped '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Spawn `docker compose -p <project> [--profile <p>] watch` and stream
/// stdout / stderr bytes into `log_stream` as they arrive. The process
/// is detached on the background executor; dropping the corresponding
/// `LogStream` on the caller side is the documented stop signal (the
/// downstream channel will close and the next write returns broken
/// pipe, which we treat as termination).
pub fn compose_watch(
  project_name: String,
  working_dir: Option<String>,
  config_files: Vec<String>,
  profile: Option<String>,
  log_stream: &Arc<LogStream>,
  cx: &mut App,
) {
  let task_id = start_task(cx, format!("compose watch '{project_name}'..."));
  let project_for_msg = project_name.clone();
  let disp = dispatcher(cx);
  let log_for_task = log_stream.clone();

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let mut cmd = compose_invocation(&project_name, working_dir.as_deref(), &config_files);
        if let Some(p) = profile.as_deref() {
          cmd.args(["--profile", p]);
        }
        cmd.arg("watch");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd
          .spawn()
          .map_err(|e| format!("failed to spawn docker compose watch: {e}"))?;
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();

        // Drain stdout and stderr concurrently into the LogStream.
        // CR-prefix raw '\n' so libghostty's grid breaks lines; the
        // build pipeline does the same in services::build_image.
        let log_stdout = log_for_task.clone();
        let log_stderr = log_for_task.clone();
        let stdout_handle = std::thread::spawn(move || {
          if let Some(out) = stdout.as_mut() {
            let mut buf = [0u8; 4096];
            while let Ok(n) = out.read(&mut buf)
              && n > 0
            {
              log_stdout.feed_bytes(crlf_normalize(&buf[..n]));
            }
          }
        });
        let stderr_handle = std::thread::spawn(move || {
          if let Some(err) = stderr.as_mut() {
            let mut buf = [0u8; 4096];
            while let Ok(n) = err.read(&mut buf)
              && n > 0
            {
              log_stderr.feed_bytes(crlf_normalize(&buf[..n]));
            }
          }
        });

        let status = child.wait().map_err(|e| format!("wait failed: {e}"))?;
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        if status.success() {
          Ok(())
        } else {
          Err(format!("docker compose watch exited with status {status}"))
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("compose watch '{project_for_msg}' stopped"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed compose watch '{project_for_msg}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

fn crlf_normalize(input: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(input.len() + 16);
  let mut prev = 0u8;
  for &b in input {
    if b == b'\n' && prev != b'\r' {
      out.push(b'\r');
    }
    out.push(b);
    prev = b;
  }
  out
}

pub fn compose_restart(project_name: String, working_dir: Option<String>, config_files: Vec<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let working_dir = working_dir.clone();
    let config_files = config_files.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        let output = compose_invocation(&project, working_dir.as_deref(), &config_files)
          .arg("restart")
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Restarted '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
