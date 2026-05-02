//! Prompt flows for volume Backup, Restore, and Clone actions wired from
//! the volume row menu.

use gpui::{App, Entity, ParentElement, Styled, Window, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  input::{Input, InputState},
  v_flex,
};

use crate::services;

pub fn prompt_backup_volume(name: String, _window: &mut Window, cx: &mut App) {
  let opts = gpui::PathPromptOptions {
    files: false,
    directories: true,
    multiple: false,
    prompt: Some("Choose Backup Folder".into()),
  };
  let rx = cx.prompt_for_paths(opts);
  cx.spawn(async move |cx| {
    if let Ok(Ok(Some(paths))) = rx.await
      && let Some(dir) = paths.into_iter().next()
    {
      let _ = cx.update(|cx| services::backup_volume(name, dir, cx));
    }
  })
  .detach();
}

pub fn prompt_restore_volume(name: String, _window: &mut Window, cx: &mut App) {
  let opts = gpui::PathPromptOptions {
    files: true,
    directories: false,
    multiple: false,
    prompt: Some("Choose Backup Archive".into()),
  };
  let rx = cx.prompt_for_paths(opts);
  cx.spawn(async move |cx| {
    if let Ok(Ok(Some(paths))) = rx.await
      && let Some(path) = paths.into_iter().next()
    {
      let _ = cx.update(|cx| services::restore_volume(name, path, cx));
    }
  })
  .detach();
}

pub fn prompt_clone_volume(src: String, window: &mut Window, cx: &mut App) {
  let suggested = format!("{src}-clone");
  let input_state: Entity<InputState> = cx.new(|cx| {
    let mut state = InputState::new(window, cx).placeholder("New volume name");
    state.set_value(suggested, window, cx);
    state
  });

  let input_for_save = input_state.clone();
  window.open_dialog(cx, move |dialog, _window, _cx| {
    let input_for_render = input_state.clone();
    let input_for_btn = input_for_save.clone();
    let src_for_btn = src.clone();
    dialog
      .title(format!("Clone volume '{src}'"))
      .min_w(px(420.))
      .child(
        v_flex()
          .gap(px(8.))
          .p(px(16.))
          .child(Input::new(&input_for_render).w_full()),
      )
      .footer(move |_dialog_state, _, _window, _cx| {
        let input = input_for_btn.clone();
        let src_label = src_for_btn.clone();
        vec![
          Button::new("clone-go")
            .label("Clone")
            .primary()
            .on_click(move |_ev, window, cx| {
              let dst = input.read(cx).text().to_string().trim().to_string();
              if dst.is_empty() {
                return;
              }
              services::clone_volume(src_label.clone(), dst, cx);
              window.close_dialog(cx);
            })
            .into_any_element(),
          Button::new("clone-cancel")
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
