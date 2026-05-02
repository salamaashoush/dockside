//! Centralized dialog helpers
//!
//! This module provides simple helper functions to open dialogs with all buttons
//! and actions pre-configured. Call these functions from anywhere (views, command
//! palette, menu bar) to open a fully functional dialog.

use gpui::{App, AppContext, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};

use crate::docker::LintReport;
use crate::services;
use crate::ui::containers::CreateContainerDialog;
use crate::ui::deployments::create_dialog::CreateDeploymentDialog;
use crate::ui::images::LintReportDialog;
use crate::ui::images::build_dialog::BuildImageDialog;
use crate::ui::images::pull_dialog::PullImageDialog;
use crate::ui::images::push_dialog::PushImageDialog;
use crate::ui::images::registry_dialog::RegistryBrowserDialog;
use crate::ui::images::tag_dialog::TagImageDialog;
use crate::ui::machines::MachineDialog;
use crate::ui::networks::create_dialog::CreateNetworkDialog;
use crate::ui::prune_dialog::PruneDialog;
use crate::ui::services::create_dialog::CreateServiceDialog;
use crate::ui::volumes::create_dialog::CreateVolumeDialog;

/// Opens the Pull Image dialog with Pull button configured
pub fn open_pull_image_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(PullImageDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Pull Image")
      .min_w(px(500.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_pull = dialog_clone.clone();
        vec![
          Button::new("pull")
            .label("Pull")
            .primary()
            .on_click({
              let dialog = dialog_for_pull.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx);
                if !options.image.is_empty() {
                  services::pull_image(options.image, options.platform.as_docker_arg().map(String::from), cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Registry Browser dialog (Docker Hub search via the
/// daemon's `/images/search`) with a Close button.
pub fn open_registry_browser_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(RegistryBrowserDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    dialog
      .title("Browse Docker Hub")
      .min_w(px(700.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        vec![
          Button::new("close")
            .label("Close")
            .ghost()
            .on_click(|_ev, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}

/// Prompt the user for a path then stream `docker save <image_ref>`
/// bytes to it. The dialog is OS-native (gpui platform layer); the
/// task runs in the background and reports completion via the global
/// dispatcher / task pipeline.
pub fn prompt_save_image_tarball(image_ref: String, _window: &mut Window, cx: &mut App) {
  let suggested = format!(
    "{}.tar",
    image_ref.replace([':', '/'], "-")
  );
  let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
  let rx = cx.prompt_for_new_path(&cwd, Some(&suggested));
  cx.spawn(async move |cx| {
    if let Ok(Ok(Some(path))) = rx.await {
      let _ = cx.update(|cx| {
        services::save_image(image_ref, path, cx);
      });
    }
  })
  .detach();
}

/// Prompt the user for a tarball on disk and POST it to `/images/load`.
pub fn prompt_load_image_tarball(_window: &mut Window, cx: &mut App) {
  let opts = gpui::PathPromptOptions {
    files: true,
    directories: false,
    multiple: false,
    prompt: Some("Open".into()),
  };
  let rx = cx.prompt_for_paths(opts);
  cx.spawn(async move |cx| {
    if let Ok(Ok(Some(paths))) = rx.await
      && let Some(path) = paths.into_iter().next()
    {
      let _ = cx.update(|cx| {
        services::load_image(path, cx);
      });
    }
  })
  .detach();
}

/// Opens the Build Image dialog with Lint + Build buttons configured.
/// On Build, the form dialog closes and a streaming output dialog
/// opens with a libghostty-backed terminal viewer that follows the
/// build log live (selection / copy / colors all work the same as
/// container logs). Lint runs hadolint against the chosen Dockerfile
/// in-place and surfaces the result inline; "View report" opens a
/// detail dialog rendered by `LintReportDialog`.
pub fn open_build_image_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(BuildImageDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Build Image")
      .min_w(px(560.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_build = dialog_clone.clone();
        let dialog_for_lint = dialog_clone.clone();
        vec![
          Button::new("build")
            .label("Build")
            .primary()
            .on_click({
              let dialog = dialog_for_build.clone();
              move |_ev, window, cx| {
                let opts = dialog.read(cx).get_options(cx);
                if opts.context_dir.is_empty() || opts.tag.is_empty() {
                  return;
                }
                let log_stream = match crate::terminal::LogStream::new(120, 40) {
                  Ok(s) => std::sync::Arc::new(s),
                  Err(_) => return,
                };
                let tag = opts.tag.clone();
                services::build_image(
                  opts.context_dir,
                  opts.dockerfile,
                  opts.tag,
                  opts.build_args,
                  opts.target,
                  opts.platform.as_docker_arg().map(String::from),
                  opts.no_cache,
                  opts.pull,
                  log_stream.clone(),
                  cx,
                );
                window.close_dialog(cx);
                open_build_output_dialog(tag, log_stream, window, cx);
              }
            })
            .into_any_element(),
          Button::new("lint")
            .label("Lint")
            .ghost()
            .on_click({
              let dialog = dialog_for_lint.clone();
              move |_ev, _window, cx| {
                BuildImageDialog::run_lint(&dialog, cx);
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens a non-modal viewer dialog for an existing `LintReport`. Caller
/// supplies the source path (used as the dialog subtitle).
pub fn open_lint_report_dialog(dockerfile: String, report: LintReport, window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(|cx| LintReportDialog::new(dockerfile, report, cx));

  window.open_dialog(cx, move |dialog, _window, _cx| {
    dialog
      .title("Hadolint Report")
      .min_w(px(820.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        vec![
          Button::new("close")
            .label("Close")
            .ghost()
            .on_click(|_ev, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}

/// Open a profile prompt for `docker compose watch <project>`. On
/// Start the prompt closes and a `compose_watch` background task
/// kicks off, streaming output into a fresh `LogStream` rendered via
/// `open_build_output_dialog` (the same libghostty-backed terminal
/// viewer used for build logs).
pub fn open_compose_watch_dialog(
  project: String,
  working_dir: Option<String>,
  config_files: Vec<String>,
  window: &mut Window,
  cx: &mut App,
) {
  use gpui_component::input::{Input, InputState};
  let profile_input = cx.new(|cx| InputState::new(window, cx).placeholder("Profile (optional)"));

  window.open_dialog(cx, move |dialog, _window, cx| {
    let _colors = cx.theme().colors;
    let project = project.clone();
    let working_dir = working_dir.clone();
    let config_files = config_files.clone();
    let profile_input_clone = profile_input.clone();
    dialog
      .title(format!("Watch '{project}'"))
      .min_w(px(420.))
      .child(
        v_flex()
          .gap(px(12.))
          .p(px(8.))
          .child(
            div()
              .text_sm()
              .child("Run `docker compose watch` against the project — Ctrl-C / closing the output dialog stops the stream."),
          )
          .child(Input::new(&profile_input).w_full()),
      )
      .footer(move |_dialog_state, _, _window, _cx| {
        let project = project.clone();
        let working_dir = working_dir.clone();
        let config_files = config_files.clone();
        let profile = profile_input_clone.clone();
        vec![
          Button::new("start-watch")
            .label("Start")
            .primary()
            .on_click(move |_ev, window, cx| {
              let profile_text = profile.read(cx).text().to_string();
              let profile_opt = if profile_text.trim().is_empty() {
                None
              } else {
                Some(profile_text.trim().to_string())
              };
              let log_stream = match crate::terminal::LogStream::new(120, 40) {
                Ok(s) => std::sync::Arc::new(s),
                Err(_) => return,
              };
              services::compose_watch(
                project.clone(),
                working_dir.clone(),
                config_files.clone(),
                profile_opt,
                &log_stream,
                cx,
              );
              window.close_dialog(cx);
              open_compose_watch_output_dialog(project.clone(), log_stream, window, cx);
            })
            .into_any_element(),
        ]
      })
  });
}

fn open_compose_watch_output_dialog(
  project: String,
  log_stream: std::sync::Arc<crate::terminal::LogStream>,
  window: &mut Window,
  cx: &mut App,
) {
  let view = cx.new(|cx| crate::terminal::TerminalView::for_log_stream(log_stream, cx));
  window.open_dialog(cx, move |dialog, _window, _cx| {
    dialog
      .title(format!("Watching {project}"))
      .min_w(px(900.))
      .min_h(px(520.))
      .child(
        v_flex()
          .w_full()
          .h(px(480.))
          .child(div().size_full().child(view.clone())),
      )
      .footer(move |_dialog_state, _, _window, _cx| {
        vec![
          Button::new("close")
            .label("Close")
            .ghost()
            .on_click(|_ev, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}

/// Show streaming build output in a dialog. Holds a `TerminalView`
/// driven by the supplied `LogStream`.
pub fn open_build_output_dialog(
  tag: String,
  log_stream: std::sync::Arc<crate::terminal::LogStream>,
  window: &mut Window,
  cx: &mut App,
) {
  let view = cx.new(|cx| crate::terminal::TerminalView::for_log_stream(log_stream, cx));
  window.open_dialog(cx, move |dialog, _window, _cx| {
    dialog
      .title(format!("Building {tag}"))
      .min_w(px(900.))
      .min_h(px(520.))
      .child(
        v_flex()
          .w_full()
          .h(px(480.))
          .child(div().size_full().child(view.clone())),
      )
      .footer(move |_dialog_state, _, _window, _cx| {
        vec![
          Button::new("close")
            .label("Close")
            .ghost()
            .on_click(|_ev, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Tag Image dialog with a Tag button configured.
pub fn open_tag_image_dialog(source: String, window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(TagImageDialog::new(source));

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Tag Image")
      .min_w(px(500.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_tag = dialog_clone.clone();
        vec![
          Button::new("tag")
            .label("Tag")
            .primary()
            .on_click({
              let dialog = dialog_for_tag.clone();
              move |_ev, window, cx| {
                let opts = dialog.read(cx).get_options(cx);
                if !opts.repo.is_empty() {
                  services::tag_image(opts.source, opts.repo, opts.tag, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Push Image dialog with a Push button configured.
pub fn open_push_image_dialog(image: String, tag: String, window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(PushImageDialog::new(image, tag));

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Push Image")
      .min_w(px(520.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_push = dialog_clone.clone();
        vec![
          Button::new("push")
            .label("Push")
            .primary()
            .on_click({
              let dialog = dialog_for_push.clone();
              move |_ev, window, cx| {
                let opts = dialog.read(cx).get_options(cx);
                if !opts.image.is_empty() && !opts.tag.is_empty() {
                  services::push_image(opts.image, opts.tag, opts.username, opts.password, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Create Container dialog with Create and Run buttons configured
pub fn open_create_container_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(CreateContainerDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Create Container")
      .min_w(px(600.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_create = dialog_clone.clone();
        let dialog_for_run = dialog_clone.clone();
        vec![
          Button::new("create")
            .label("Create")
            .ghost()
            .on_click({
              let dialog = dialog_for_create.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx, false);
                if !options.image.is_empty() {
                  services::create_container(options, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
          Button::new("run")
            .label("Run")
            .primary()
            .on_click({
              let dialog = dialog_for_run.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx, true);
                if !options.image.is_empty() {
                  services::create_container(options, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Create Volume dialog with Create button configured
pub fn open_create_volume_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(CreateVolumeDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Create Volume")
      .min_w(px(500.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_create = dialog_clone.clone();
        vec![
          Button::new("create")
            .label("Create")
            .primary()
            .on_click({
              let dialog = dialog_for_create.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx);
                if !options.name.is_empty() {
                  services::create_volume(
                    options.name,
                    options.driver.as_docker_arg().to_string(),
                    options.labels,
                    cx,
                  );
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Create Network dialog with Create button configured
pub fn open_create_network_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(CreateNetworkDialog::new);

  window.open_dialog(cx, move |dialog, _window, cx| {
    let colors = cx.theme().colors;

    dialog
      .title("Create Network")
      .min_w(px(500.))
      .child(
        v_flex()
          .gap(px(8.))
          .child(div().text_sm().text_color(colors.muted_foreground).child(
            "Networks are groups of containers in the same subnet (IP range) that can communicate with each other.",
          ))
          .child(dialog_entity.clone()),
      )
      .footer({
        let dialog_clone = dialog_entity.clone();
        move |_dialog_state, _, _window, _cx| {
          let dialog_for_create = dialog_clone.clone();
          vec![
            Button::new("create")
              .label("Create")
              .primary()
              .on_click({
                let dialog = dialog_for_create.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx);
                  if !options.name.is_empty() {
                    services::create_network(options.name, options.enable_ipv6, options.subnet, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        }
      })
  });
}

/// Opens the Create Machine (Colima) dialog with Create button configured
pub fn open_create_machine_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(MachineDialog::new_create);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Create Colima Machine")
      .min_w(px(550.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_create = dialog_clone.clone();
        vec![
          Button::new("create")
            .label("Create")
            .primary()
            .on_click({
              let dialog = dialog_for_create.clone();
              move |_ev, window, cx| {
                let profile = dialog.read(cx).get_profile_name(cx);
                let config = dialog.read(cx).get_config(cx);
                services::create_machine(profile, config, cx);
                window.close_dialog(cx);
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Create Deployment (Kubernetes) dialog with Create button configured
pub fn open_create_deployment_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(CreateDeploymentDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Create Deployment")
      .min_w(px(550.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_create = dialog_clone.clone();
        vec![
          Button::new("create")
            .label("Create")
            .primary()
            .on_click({
              let dialog = dialog_for_create.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx);
                if !options.name.is_empty() && !options.image.is_empty() {
                  services::create_deployment(options, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Create Service (Kubernetes) dialog with Create button configured
pub fn open_create_service_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(CreateServiceDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Create Service")
      .min_w(px(550.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_create = dialog_clone.clone();
        vec![
          Button::new("create")
            .label("Create")
            .primary()
            .on_click({
              let dialog = dialog_for_create.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options(cx);
                if !options.name.is_empty() && !options.ports.is_empty() {
                  services::create_service(options, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the Prune Docker Resources dialog with Prune button configured
pub fn open_prune_dialog(window: &mut Window, cx: &mut App) {
  let dialog_entity = cx.new(PruneDialog::new);

  window.open_dialog(cx, move |dialog, _window, _cx| {
    let dialog_clone = dialog_entity.clone();

    dialog
      .title("Prune Docker Resources")
      .min_w(px(500.))
      .child(dialog_entity.clone())
      .footer(move |_dialog_state, _, _window, _cx| {
        let dialog_for_prune = dialog_clone.clone();
        vec![
          Button::new("prune")
            .label("Prune")
            .primary()
            .on_click({
              let dialog = dialog_for_prune.clone();
              move |_ev, window, cx| {
                let options = dialog.read(cx).get_options();
                if !options.is_empty() {
                  services::prune_docker(&options, cx);
                  window.close_dialog(cx);
                }
              }
            })
            .into_any_element(),
        ]
      })
  });
}

/// Opens the About Dockside dialog
pub fn open_about_dialog(window: &mut Window, cx: &mut App) {
  use gpui::{ImageSource, Resource, SharedString, img};

  window.open_dialog(cx, move |dialog, _window, cx| {
    let colors = cx.theme().colors;
    let version = env!("CARGO_PKG_VERSION");

    dialog.title("About").min_w(px(420.)).child(
      v_flex()
        .w_full()
        .gap(px(20.))
        .p(px(24.))
        // Logo and title
        .child(
          h_flex()
            .gap(px(16.))
            .items_center()
            .child(
              img(ImageSource::Resource(Resource::Embedded(SharedString::from("icon.png"))))
                .size(px(64.))
            )
            .child(
              v_flex()
                .gap(px(4.))
                .child(
                  div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.foreground)
                    .child("Dockside"),
                )
                .child(
                  div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(format!("Version {version}")),
                ),
            ),
        )
        // Description
        .child(
          div()
            .text_sm()
            .text_color(colors.foreground)
            .child("A fast, native desktop application for managing Docker containers, images, volumes, networks, and Kubernetes resources. Built for macOS with GPUI."),
        )
        // Required tools section
        .child(
          v_flex()
            .gap(px(8.))
            .child(
              div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.muted_foreground)
                .child("REQUIRED TOOLS"),
            )
            .child(
              v_flex()
                .gap(px(4.))
                .text_sm()
                .text_color(colors.foreground)
                .child(
                  h_flex()
                    .gap(px(8.))
                    .child(div().text_color(colors.primary).child("Docker"))
                    .child(div().text_color(colors.muted_foreground).child("Container runtime")),
                )
                .child(
                  h_flex()
                    .gap(px(8.))
                    .child(div().text_color(colors.primary).child("Colima"))
                    .child(div().text_color(colors.muted_foreground).child("Virtual machines on macOS")),
                )
                .child(
                  h_flex()
                    .gap(px(8.))
                    .child(div().text_color(colors.primary).child("kubectl"))
                    .child(div().text_color(colors.muted_foreground).child("Kubernetes CLI (optional)")),
                ),
            ),
        )
        // Footer
        .child(
          div()
            .pt(px(8.))
            .border_t_1()
            .border_color(colors.border)
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("Built with GPUI framework"),
            ),
        ),
    )
  });
}
