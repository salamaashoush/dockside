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

pub fn pull_image(image: String, platform: Option<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Pulling image {image}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.pull_image(&image, platform.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image pulled successfully".to_string(),
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
