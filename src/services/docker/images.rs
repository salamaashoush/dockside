//! Image operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{ImageInspectData, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher, docker_client};

pub fn refresh_images(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    match guard.as_ref() {
      Some(docker) => docker.list_images(false).await.unwrap_or_default(),
      None => vec![],
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let images = result.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_images(images);
        cx.emit(StateChanged::ImagesUpdated);
      });
    })
  })
  .detach();
}

pub fn delete_image(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting image...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_image(&id, true).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image deleted".to_string(),
          });
        });
        refresh_images(cx);
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

/// Build an image and stream the daemon output into a fresh `LogStream`,
/// returning it so the caller can hand the same stream to a viewer
/// entity. The build runs on the tokio runtime; errors and completion
/// notifications go through the existing task / dispatcher pipeline.
pub fn build_image(
  context_dir: String,
  dockerfile: String,
  tag: String,
  build_args: Vec<(String, String)>,
  target: Option<String>,
  platform: Option<String>,
  no_cache: bool,
  pull: bool,
  log_stream: std::sync::Arc<crate::terminal::LogStream>,
  cx: &mut App,
) {
  let task_id = start_task(cx, format!("Building {tag}..."));
  let tag_for_msg = tag.clone();
  let disp = dispatcher(cx);
  let client = docker_client();
  let log_for_task = log_stream.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    let path = std::path::PathBuf::from(&context_dir);
    let target_ref = target.as_deref();
    let platform_ref = platform.as_deref();
    docker
      .build_image_with_progress(
        &path,
        &dockerfile,
        &tag,
        build_args,
        target_ref,
        platform_ref,
        no_cache,
        pull,
        |ev| {
          // Stitch stream / status / error into one CRLF-terminated
          // chunk so the libghostty grid breaks lines correctly.
          let mut text = String::new();
          if !ev.stream.is_empty() {
            text.push_str(&ev.stream);
          }
          if !ev.status.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
              text.push('\n');
            }
            text.push_str(&ev.status);
          }
          if !ev.error.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
              text.push('\n');
            }
            text.push_str("ERROR: ");
            text.push_str(&ev.error);
          }
          if !text.is_empty() {
            let mut bytes = Vec::with_capacity(text.len() + 16);
            let mut prev = 0u8;
            for &b in text.as_bytes() {
              if b == b'\n' && prev != b'\r' {
                bytes.push(b'\r');
              }
              bytes.push(b);
              prev = b;
            }
            log_for_task.feed_bytes(bytes);
          }
        },
      )
      .await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Image {tag_for_msg} built"),
          });
        });
        refresh_images(cx);
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

pub fn tag_image(source: String, repo: String, tag: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Tagging {source} as {repo}:{tag}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.tag_image(&source, &repo, &tag).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image tagged".to_string(),
          });
        });
        refresh_images(cx);
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

pub fn push_image(image: String, tag: String, username: Option<String>, password: Option<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Pushing {image}:{tag}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let auth = match (username, password) {
    (Some(u), Some(p)) => Some((u, p)),
    _ => None,
  };

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker
      .push_image_with_progress(&image, &tag, auth, |line| {
        tracing::debug!(target: "docker.push", "{line}");
      })
      .await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image pushed".to_string(),
          });
        });
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

pub fn pull_image(image: String, platform: Option<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Pulling image {image}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  // Channel for progress events emitted from the bollard stream task.
  let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::docker::PullProgressEvent>(64);

  let image_for_call = image.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker
      .pull_image_with_progress(&image_for_call, platform.as_deref(), |ev| {
        let _ = tx.try_send(ev);
      })
      .await
  });

  // Drain the progress channel on the UI side and update the task.
  cx.spawn(async move |cx| {
    // Per-layer total/current state for an aggregate fraction.
    let mut totals: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
    while let Some(ev) = rx.recv().await {
      if let (Some(cur), Some(tot)) = (ev.current, ev.total)
        && tot > 0
      {
        totals.insert(ev.id.clone(), (cur, tot));
      }
      let (sum_cur, sum_tot): (i64, i64) = totals
        .values()
        .fold((0i64, 0i64), |(a, b), (c, t)| (a + c, b + t));
      #[allow(clippy::cast_precision_loss)]
      let frac = if sum_tot > 0 {
        (sum_cur as f32) / (sum_tot as f32)
      } else {
        0.0
      };
      let status = if ev.id.is_empty() {
        ev.status.clone()
      } else {
        format!("{}: {}", ev.id, ev.status)
      };
      let _ = cx.update(|cx| {
        crate::services::task_manager::set_task_progress(cx, task_id, frac, Some(status));
      });
    }
  })
  .detach();

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Image {image} pulled"),
          });
        });
        refresh_images(cx);
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

pub fn inspect_image(image_id: String, cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();
  let image_id_clone = image_id.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

    // Get image inspect
    let _image = docker.image_inspect(&image_id).await?;

    // Get full inspect data from bollard
    let bollard_docker = docker.client()?;
    let inspect = bollard_docker.inspect_image(&image_id).await?;

    // Parse config
    let config = inspect.config.unwrap_or_default();
    let config_cmd = config.cmd;
    let config_workdir = config.working_dir;
    let config_entrypoint = config.entrypoint;
    let config_exposed_ports: Vec<String> = config
      .exposed_ports
      .map(|p| p.keys().cloned().collect())
      .unwrap_or_default();

    // Parse environment variables
    let config_env: Vec<(String, String)> = config
      .env
      .unwrap_or_default()
      .into_iter()
      .filter_map(|e| {
        let parts: Vec<&str> = e.splitn(2, '=').collect();
        if parts.len() == 2 {
          Some((parts[0].to_string(), parts[1].to_string()))
        } else {
          None
        }
      })
      .collect();

    // Image history (per-layer breakdown).
    let history = docker.image_history(&image_id).await.unwrap_or_default();

    Ok::<_, anyhow::Error>((
      config_cmd,
      config_workdir,
      config_env,
      config_entrypoint,
      config_exposed_ports,
      history,
      image_id,
    ))
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      if let Ok(Ok((
        config_cmd,
        config_workdir,
        config_env,
        config_entrypoint,
        config_exposed_ports,
        history,
        _image_id,
      ))) = result
      {
        // Get containers using this image
        let docker_state_entity = docker_state(cx);
        let containers = docker_state_entity.read(cx).containers.clone();
        let used_by: Vec<String> = containers
          .iter()
          .filter(|c| c.image_id == image_id_clone)
          .map(|c| c.name.clone())
          .collect();

        state.update(cx, |_state, cx| {
          cx.emit(StateChanged::ImageInspectLoaded {
            image_id: image_id_clone,
            data: ImageInspectData {
              config_cmd,
              config_workdir,
              config_env,
              config_entrypoint,
              config_exposed_ports,
              used_by,
              history,
            },
          });
        });
      }
    })
  })
  .detach();
}
