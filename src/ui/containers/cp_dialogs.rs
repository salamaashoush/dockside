//! Prompt flows for `docker cp` upload and download from the container row
//! menu. Upload picks a host file/dir, then asks for an in-container
//! destination directory. Download asks for an in-container path, then a
//! host destination directory.

use gpui::{App, Entity, ParentElement, Styled, Window, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  input::{Input, InputState},
  v_flex,
};

use crate::services;

pub fn prompt_upload_to_container(container_id: String, _window: &mut Window, cx: &mut App) {
  let opts = gpui::PathPromptOptions {
    files: true,
    directories: true,
    multiple: false,
    prompt: Some("Choose file or folder to upload".into()),
  };
  let rx = cx.prompt_for_paths(opts);
  cx.spawn(async move |cx| {
    let Ok(Ok(Some(paths))) = rx.await else { return };
    let Some(src) = paths.into_iter().next() else { return };
    let _ = cx.update(|cx| {
      open_dest_path_dialog(container_id, src, cx);
    });
  })
  .detach();
}

fn open_dest_path_dialog(container_id: String, src: std::path::PathBuf, cx: &mut App) {
  let initial = "/tmp".to_string();
  let id_for_window = container_id.clone();
  let src_for_dialog = src.clone();
  let _ = id_for_window;
  let _ = src_for_dialog;

  let window_handle = cx
    .active_window()
    .or_else(|| cx.windows().into_iter().next())
    .expect("no active window for upload dialog");
  window_handle
    .update(cx, move |_, window, cx| {
      let input_state: Entity<InputState> = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("/path/inside/container");
        state.set_value(initial, window, cx);
        state
      });
      let input_for_save = input_state.clone();

      window.open_dialog(cx, move |dialog, _window, _cx| {
        let input_for_render = input_state.clone();
        let input_for_btn = input_for_save.clone();
        let id_for_btn = container_id.clone();
        let src_for_btn = src.clone();
        dialog
          .title("Upload to Container")
          .min_w(px(460.))
          .child(
            v_flex()
              .gap(px(8.))
              .p(px(16.))
              .child(Input::new(&input_for_render).w_full()),
          )
          .footer(move |_dialog_state, _, _window, _cx| {
            let input = input_for_btn.clone();
            let id = id_for_btn.clone();
            let src = src_for_btn.clone();
            vec![
              Button::new("upload-go")
                .label("Upload")
                .primary()
                .on_click(move |_ev, window, cx| {
                  let dest = input.read(cx).text().to_string().trim().to_string();
                  if dest.is_empty() {
                    return;
                  }
                  services::cp_to_container(id.clone(), src.clone(), dest, cx);
                  window.close_dialog(cx);
                })
                .into_any_element(),
              Button::new("upload-cancel")
                .label("Cancel")
                .ghost()
                .on_click(|_ev, window, cx| {
                  window.close_dialog(cx);
                })
                .into_any_element(),
            ]
          })
      });
    })
    .ok();
}

pub fn prompt_download_from_container(container_id: String, window: &mut Window, cx: &mut App) {
  let input_state: Entity<InputState> = cx.new(|cx| InputState::new(window, cx).placeholder("/path/inside/container"));
  let input_for_save = input_state.clone();

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let input_for_render = input_state.clone();
    let input_for_btn = input_for_save.clone();
    let id_for_btn = container_id.clone();
    dialog
      .title("Download from Container")
      .min_w(px(460.))
      .child(
        v_flex()
          .gap(px(8.))
          .p(px(16.))
          .child(Input::new(&input_for_render).w_full()),
      )
      .footer(move |_dialog_state, _, _window, _cx| {
        let input = input_for_btn.clone();
        let id = id_for_btn.clone();
        vec![
          Button::new("download-pick")
            .label("Choose Destination")
            .primary()
            .on_click(move |_ev, window, cx| {
              let src = input.read(cx).text().to_string().trim().to_string();
              if src.is_empty() {
                return;
              }
              window.close_dialog(cx);
              let opts = gpui::PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some("Choose Destination Folder".into()),
              };
              let rx = cx.prompt_for_paths(opts);
              let id = id.clone();
              cx.spawn(async move |cx| {
                let Ok(Ok(Some(paths))) = rx.await else { return };
                let Some(dest) = paths.into_iter().next() else { return };
                let _ = cx.update(|cx| {
                  services::cp_from_container(id, src, dest, cx);
                });
              })
              .detach();
            })
            .into_any_element(),
          Button::new("download-cancel")
            .label("Cancel")
            .ghost()
            .on_click(|_ev, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}
