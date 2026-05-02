//! Volume operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher, docker_client};

pub fn create_volume(name: String, driver: String, labels: Vec<(String, String)>, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating volume {name}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.create_volume_with_opts(&name, &driver, labels).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(_)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Volume created".to_string(),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn backup_volume(name: String, dest_dir: std::path::PathBuf, cx: &mut App) {
  let task_id = start_task(cx, format!("Backing up volume {name}..."));
  let disp = dispatcher(cx);
  let client = docker_client();
  let label = name.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.backup_volume(&name, &dest_dir).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(path)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Volume '{label}' backed up to {}", path.display()),
          });
        });
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Backup '{label}' failed: {e}"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn restore_volume(name: String, archive_path: std::path::PathBuf, cx: &mut App) {
  let task_id = start_task(cx, format!("Restoring volume {name}..."));
  let disp = dispatcher(cx);
  let client = docker_client();
  let label = name.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.restore_volume(&name, &archive_path).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Volume '{label}' restored"),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Restore '{label}' failed: {e}"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn clone_volume(src: String, dst: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Cloning volume {src} to {dst}..."));
  let disp = dispatcher(cx);
  let client = docker_client();
  let dst_label = dst.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.clone_volume(&src, &dst).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(_)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Volume cloned to '{dst_label}'"),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn delete_volume(name: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting volume...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_volume(&name, true).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Volume deleted".to_string(),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn refresh_volumes(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    match guard.as_ref() {
      Some(docker) => docker.list_volumes().await.unwrap_or_default(),
      None => vec![],
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let volumes = result.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_volumes(volumes);
        cx.emit(StateChanged::VolumesUpdated);
      });
    })
  })
  .detach();
}

pub fn list_volume_files(volume_name: String, path: String, cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();
  let volume_name_clone = volume_name.clone();
  let path_clone = path.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.list_volume_files(&volume_name, &path).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |_state, cx| match result {
        Ok(Ok(files)) => {
          cx.emit(StateChanged::VolumeFilesLoaded {
            volume_name: volume_name_clone,
            path: path_clone,
            files,
          });
        }
        Ok(Err(_)) | Err(_) => {
          cx.emit(StateChanged::VolumeFilesError {
            volume_name: volume_name_clone,
          });
        }
      });
    })
  })
  .detach();
}
