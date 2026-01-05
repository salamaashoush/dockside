use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Root, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  notification::NotificationType,
  scroll::ScrollableElement,
  sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
  theme::{ActiveTheme, Theme},
  v_flex,
};

use crate::ui::components::spinning_loader;

use crate::keybindings::{
  DeleteSelected, FocusSearch, GoToActivityMonitor, GoToCompose, GoToContainers, GoToDeployments, GoToImages,
  GoToMachines, GoToNetworks, GoToPods, GoToServices, GoToSettings, GoToVolumes, InspectSelected, NewResource,
  OpenCommandPalette, OpenTerminal, Refresh, RestartSelected, ShowKeyboardShortcuts, StartSelected, StopSelected,
  ViewLogs,
};

use crate::assets::AppIcon;
use crate::services::{DispatcherEvent, dispatcher, task_manager};
use crate::state::{CurrentView, DockerState, Selection, StateChanged, docker_state};
use crate::ui::activity::ActivityMonitorView;
use crate::ui::command_palette::{CommandPalette, CommandPaletteEvent, PaletteAction};
use crate::ui::compose::ComposeView;
use crate::ui::containers::ContainersView;
use crate::ui::deployments::DeploymentsView;
use crate::ui::dialogs;
use crate::ui::global_search::{GlobalSearch, GlobalSearchEvent};
use crate::ui::images::ImagesView;
use crate::ui::machines::MachinesView;
use crate::ui::networks::NetworksView;
use crate::ui::pods::PodsView;
use crate::ui::services::ServicesView;
use crate::ui::settings::SettingsView;
use crate::ui::setup_dialog::{
  SetupDialog, diagnose_k8s_quick, is_colima_installed, is_colima_running, is_docker_installed,
};
use crate::ui::volumes::VolumesView;

/// Main application - only handles layout and view switching
pub struct DocksideApp {
  docker_state: Entity<DockerState>,
  machines_view: Entity<MachinesView>,
  containers_view: Entity<ContainersView>,
  compose_view: Entity<ComposeView>,
  volumes_view: Entity<VolumesView>,
  images_view: Entity<ImagesView>,
  networks_view: Entity<NetworksView>,
  pods_view: Entity<PodsView>,
  services_view: Entity<ServicesView>,
  deployments_view: Entity<DeploymentsView>,
  activity_view: Entity<ActivityMonitorView>,
  settings_view: Entity<SettingsView>,
  // Centralized notification handling - prevents duplicate notifications on view switch
  pending_notifications: Vec<(NotificationType, String)>,
  // Pending setup check result - triggers dialog when set
  pending_setup_check: Option<(bool, bool, bool)>, // (colima_installed, docker_installed, colima_running)
  // Focus handle for keyboard shortcuts
  focus_handle: FocusHandle,
  // Show keyboard shortcuts overlay
  show_shortcuts_overlay: bool,
  // Command palette
  command_palette: Option<Entity<CommandPalette>>,
  // Global search
  global_search: Option<Entity<GlobalSearch>>,
  // Pending action from command palette (processed in render to have access to window)
  pending_palette_action: Option<PaletteAction>,
}

impl Focusable for DocksideApp {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl DocksideApp {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    // Theme is already applied in main.rs from saved settings - no need to force dark mode here

    let focus_handle = cx.focus_handle();
    let docker_state = docker_state(cx);

    // Subscribe to state changes for re-rendering on view changes
    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::ViewChanged | StateChanged::Loading) {
        cx.notify();
      }
    })
    .detach();

    // Observe theme changes to re-render when theme is switched
    cx.observe_global::<Theme>(|_this, cx| {
      cx.notify();
    })
    .detach();

    // Subscribe to dispatcher events for centralized notification handling
    let disp = dispatcher(cx);
    cx.subscribe(&disp, |this, _disp, event: &DispatcherEvent, cx| {
      match event {
        DispatcherEvent::TaskCompleted { message, .. } => {
          this
            .pending_notifications
            .push((NotificationType::Success, message.clone()));
        }
        DispatcherEvent::TaskFailed { error, .. } => {
          this
            .pending_notifications
            .push((NotificationType::Error, error.clone()));
        }
      }
      cx.notify();
    })
    .detach();

    // Create self-contained views
    let machines_view = cx.new(|cx| MachinesView::new(window, cx));
    let containers_view = cx.new(|cx| ContainersView::new(window, cx));
    let compose_view = cx.new(|cx| ComposeView::new(window, cx));
    let volumes_view = cx.new(|cx| VolumesView::new(window, cx));
    let images_view = cx.new(|cx| ImagesView::new(window, cx));
    let networks_view = cx.new(|cx| NetworksView::new(window, cx));
    let pods_view = cx.new(|cx| PodsView::new(window, cx));
    let services_view = cx.new(|cx| ServicesView::new(window, cx));
    let deployments_view = cx.new(|cx| DeploymentsView::new(window, cx));
    let activity_view = cx.new(|cx| ActivityMonitorView::new(window, cx));
    let settings_view = cx.new(SettingsView::new);

    // Run setup checks async - only show dialog if there are issues
    cx.spawn(async move |this, cx| {
      // Run checks in background (non-blocking)
      let (colima_installed, docker_installed, colima_running, k8s_diag) = cx
        .background_executor()
        .spawn(async move {
          (
            is_colima_installed(),
            is_docker_installed(),
            is_colima_running(),
            diagnose_k8s_quick(),
          )
        })
        .await;

      // Check if setup is needed
      // K8s issues: only check context mismatch at startup (API check is slow and done in dialog)
      let has_k8s_issues = colima_installed
        && docker_installed
        && colima_running
        && k8s_diag.expected_context.is_some()
        && k8s_diag.context_mismatch;

      let needs_setup = !colima_installed || !docker_installed || !colima_running || has_k8s_issues;

      if needs_setup {
        // Show dialog on main thread
        this
          .update(cx, |this, cx| {
            this.pending_setup_check = Some((colima_installed, docker_installed, colima_running));
            cx.notify();
          })
          .ok();
      }
    })
    .detach();

    // Focus the app immediately so keyboard shortcuts work
    focus_handle.focus(window);

    Self {
      docker_state,
      machines_view,
      containers_view,
      compose_view,
      volumes_view,
      images_view,
      networks_view,
      pods_view,
      services_view,
      deployments_view,
      activity_view,
      settings_view,
      pending_notifications: Vec::new(),
      pending_setup_check: None,
      focus_handle,
      show_shortcuts_overlay: false,
      command_palette: None,
      global_search: None,
      pending_palette_action: None,
    }
  }

  fn open_global_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.global_search.is_some() {
      return; // Already open
    }

    let search = cx.new(|cx| GlobalSearch::new(window, cx));

    // Subscribe to search events - use subscribe_in to get window access for focus restoration
    cx.subscribe_in(
      &search,
      window,
      |this, _search, event: &GlobalSearchEvent, window, cx| {
        match event {
          GlobalSearchEvent::Close => {
            this.global_search = None;
            // Restore focus to the app so keyboard shortcuts work
            this.focus_handle.focus(window);
          }
          GlobalSearchEvent::Navigate { view, selection } => {
            // Navigate to the view and set selection
            crate::services::set_view(*view, cx);
            this.docker_state.update(cx, |state, cx| {
              state.set_selection(selection.clone());
              cx.emit(StateChanged::SelectionChanged);
            });
          }
        }
        cx.notify();
      },
    )
    .detach();

    self.global_search = Some(search);
    cx.notify();
  }

  fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.command_palette.is_some() {
      return; // Already open
    }

    let palette = cx.new(|cx| CommandPalette::new(window, cx));

    // Subscribe to palette events - use subscribe_in to get window access for focus restoration
    cx.subscribe_in(
      &palette,
      window,
      |this, _palette, event: &CommandPaletteEvent, window, cx| {
        match event {
          CommandPaletteEvent::Close => {
            this.command_palette = None;
            // Restore focus to the app so keyboard shortcuts work
            this.focus_handle.focus(window);
          }
          CommandPaletteEvent::Action(action) => {
            this.pending_palette_action = Some(*action);
          }
        }
        cx.notify();
      },
    )
    .detach();

    self.command_palette = Some(palette);
    cx.notify();
  }

  fn toggle_shortcuts_overlay(&mut self, cx: &mut Context<'_, Self>) {
    self.show_shortcuts_overlay = !self.show_shortcuts_overlay;
    cx.notify();
  }

  fn render_shortcuts_overlay(cx: &mut Context<'_, Self>) -> impl IntoElement + use<> {
    let shortcuts = crate::keybindings::get_all_shortcuts();
    let colors = cx.theme().colors;

    // Group shortcuts by category
    let mut categories: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
    for shortcut in &shortcuts {
      categories.entry(shortcut.category).or_default().push(shortcut);
    }

    // Helper to render a single shortcut row
    let render_shortcut = |description: &'static str,
                           keys: &'static str,
                           fg: gpui::Hsla,
                           muted: gpui::Hsla,
                           bg: gpui::Hsla,
                           border: gpui::Hsla| {
      h_flex()
        .py(px(6.))
        .gap(px(12.))
        .justify_between()
        .child(div().text_sm().text_color(muted).child(description))
        .child(h_flex().gap(px(4.)).children(keys.split('+').map(move |key| {
          div()
            .px(px(8.))
            .py(px(3.))
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded(px(6.))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(fg)
            .child(key.trim())
        })))
    };

    // Helper to render a category section
    let render_category = |name: &'static str,
                           items: &[&crate::keybindings::KeyboardShortcut],
                           fg: gpui::Hsla,
                           muted: gpui::Hsla,
                           bg: gpui::Hsla,
                           border: gpui::Hsla| {
      v_flex()
        .gap(px(4.))
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(fg)
            .pb(px(8.))
            .mb(px(4.))
            .border_b_1()
            .border_color(border)
            .child(name),
        )
        .children(
          items
            .iter()
            .map(move |shortcut| render_shortcut(shortcut.description, shortcut.keys, fg, muted, bg, border)),
        )
    };

    let fg = colors.foreground;
    let muted = colors.muted_foreground;
    let bg = colors.sidebar;
    let border = colors.border;
    let background = colors.background;

    // Get categories
    let navigation = categories.get("Navigation").cloned().unwrap_or_default();
    let general = categories.get("General").cloned().unwrap_or_default();
    let containers = categories.get("Containers").cloned().unwrap_or_default();

    div()
      .id("shortcuts-overlay")
      .absolute()
      .inset_0()
      .bg(gpui::hsla(0.0, 0.0, 0.0, 0.6))
      .flex()
      .items_center()
      .justify_center()
      .on_mouse_down(
        gpui::MouseButton::Left,
        cx.listener(|this, _ev, _window, cx| {
          this.show_shortcuts_overlay = false;
          cx.notify();
        }),
      )
      .child(
        div()
          .on_mouse_down(gpui::MouseButton::Left, |_ev, _window, cx| {
            cx.stop_propagation();
          })
          .bg(background)
          .border_1()
          .border_color(border)
          .rounded(px(12.))
          .shadow_lg()
          .w(px(750.))
          .max_h(px(550.))
          .overflow_hidden()
          .child(
            v_flex()
              .size_full()
              // Header
              .child(
                h_flex()
                  .px(px(24.))
                  .py(px(16.))
                  .border_b_1()
                  .border_color(border)
                  .justify_between()
                  .items_center()
                  .child(
                    h_flex()
                      .gap(px(12.))
                      .items_center()
                      .child(
                        div()
                          .p(px(8.))
                          .bg(bg)
                          .rounded(px(8.))
                          .child(Icon::new(IconName::Info).size(px(20.)).text_color(fg)),
                      )
                      .child(
                        div()
                          .text_lg()
                          .font_weight(gpui::FontWeight::SEMIBOLD)
                          .text_color(fg)
                          .child("Keyboard Shortcuts"),
                      ),
                  )
                  .child(
                    h_flex()
                      .gap(px(4.))
                      .items_center()
                      .child(
                        div()
                          .px(px(6.))
                          .py(px(2.))
                          .bg(bg)
                          .border_1()
                          .border_color(border)
                          .rounded(px(4.))
                          .text_xs()
                          .text_color(muted)
                          .child("Esc"),
                      )
                      .child(
                        div()
                          .text_xs()
                          .text_color(muted)
                          .child("to close"),
                      ),
                  ),
              )
              // Content - Two columns
              .child(
                h_flex()
                  .flex_1()
                  .overflow_y_scrollbar()
                  .p(px(24.))
                  .gap(px(32.))
                  // Left column - Navigation
                  .child(
                    v_flex()
                      .flex_1()
                      .gap(px(20.))
                      .child(render_category("Navigation", &navigation, fg, muted, bg, border))
                  )
                  // Right column - General & Containers
                  .child(
                    v_flex()
                      .flex_1()
                      .gap(px(20.))
                      .child(render_category("General", &general, fg, muted, bg, border))
                      .child(render_category("Containers", &containers, fg, muted, bg, border))
                  ),
              ),
          ),
      )
  }

  fn show_setup_dialog_with_title(window: &mut Window, cx: &mut Context<'_, Self>, title: &'static str) {
    let dialog_entity = cx.new(SetupDialog::new);

    window.open_dialog(cx, move |dialog, _window, cx| {
      let dialog_clone = dialog_entity.clone();
      let _colors = cx.theme().colors;

      dialog
        .title(title)
        .min_w(px(550.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_refresh = dialog_clone.clone();

          vec![
            Button::new("refresh")
              .label("Check Again")
              .ghost()
              .on_click({
                let dialog = dialog_for_refresh.clone();
                move |_ev, _window, cx| {
                  dialog.update(cx, |d, cx| {
                    d.refresh_status(cx);
                    cx.notify();
                  });
                }
              })
              .into_any_element(),
            Button::new("continue")
              .label("Continue Anyway")
              .primary()
              .on_click(move |_ev, window, cx| {
                window.close_dialog(cx);
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn show_prune_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_prune_dialog(window, cx);
  }

  fn render_sidebar(&self, cx: &mut Context<'_, Self>) -> impl IntoElement + use<> {
    let state = self.docker_state.read(cx);
    let current_view = state.current_view;

    Sidebar::left()
            .collapsible(false)
            .pt(px(52.)) // Space for traffic lights
            .child(
                SidebarGroup::new("Docker").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Containers")
                                .icon(IconName::SquareTerminal)
                                .active(current_view == CurrentView::Containers)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Containers, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Compose")
                                .icon(IconName::LayoutDashboard)
                                .active(current_view == CurrentView::Compose)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Compose, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Volumes")
                                .icon(IconName::Folder)
                                .active(current_view == CurrentView::Volumes)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Volumes, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Images")
                                .icon(IconName::GalleryVerticalEnd)
                                .active(current_view == CurrentView::Images)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Images, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Networks")
                                .icon(IconName::Globe)
                                .active(current_view == CurrentView::Networks)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Networks, cx);
                                })),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Kubernetes").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Pods")
                                .icon(Icon::new(AppIcon::Pod))
                                .active(current_view == CurrentView::Pods)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Pods, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Deployments")
                                .icon(Icon::new(AppIcon::Deployment))
                                .active(current_view == CurrentView::Deployments)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Deployments, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Services")
                                .icon(IconName::Globe)
                                .active(current_view == CurrentView::Services)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Services, cx);
                                })),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Colima").child(
                    SidebarMenu::new().child(
                        SidebarMenuItem::new("Machines")
                            .icon(IconName::Frame)
                            .active(current_view == CurrentView::Machines)
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                crate::services::set_view(CurrentView::Machines, cx);
                            })),
                    ),
                ),
            )
            .child(
                SidebarGroup::new("General").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Activity Monitor")
                                .icon(IconName::ChartPie)
                                .active(current_view == CurrentView::ActivityMonitor)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::ActivityMonitor, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Prune")
                                .icon(Icon::new(AppIcon::Trash))
                                .on_click(cx.listener(|_this, _ev, window, cx| {
                                    Self::show_prune_dialog(window, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Settings")
                                .icon(IconName::Settings)
                                .active(current_view == CurrentView::Settings)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Settings, cx);
                                })),
                        ),
                ),
            )
  }

  fn render_content(&self, cx: &mut Context<'_, Self>) -> impl IntoElement + use<> {
    let state = self.docker_state.read(cx);

    match state.current_view {
      CurrentView::Machines => div().size_full().child(self.machines_view.clone()),
      CurrentView::Containers => div().size_full().child(self.containers_view.clone()),
      CurrentView::Compose => div().size_full().child(self.compose_view.clone()),
      CurrentView::Volumes => div().size_full().child(self.volumes_view.clone()),
      CurrentView::Images => div().size_full().child(self.images_view.clone()),
      CurrentView::Networks => div().size_full().child(self.networks_view.clone()),
      CurrentView::Pods => div().size_full().child(self.pods_view.clone()),
      CurrentView::Services => div().size_full().child(self.services_view.clone()),
      CurrentView::Deployments => div().size_full().child(self.deployments_view.clone()),
      CurrentView::ActivityMonitor => div().size_full().child(self.activity_view.clone()),
      CurrentView::Settings => div().size_full().child(self.settings_view.clone()),
    }
  }

  fn render_task_bar(cx: &App) -> Option<impl IntoElement + use<>> {
    let tasks = task_manager(cx);
    let running_tasks: Vec<_> = tasks.read(cx).running_tasks().into_iter().cloned().collect();

    if running_tasks.is_empty() {
      return None;
    }

    let colors = &cx.theme().colors;

    Some(
      h_flex()
        .w_full()
        .py(px(8.))
        .px(px(16.))
        .bg(colors.sidebar)
        .border_t_1()
        .border_color(colors.border)
        .gap(px(16.))
        .items_center()
        .children(running_tasks.iter().map(|task| {
          let progress = task.stage_progress();
          let status_text = task.display_status();
          let has_stages = !task.stages.is_empty();

          h_flex()
            .flex_1()
            .gap(px(10.))
            .items_center()
            // Activity indicator (spinning)
            .child(spinning_loader(px(16.), colors.primary))
            // Task name and stage
            .child(
              v_flex()
                .gap(px(2.))
                .child(
                  div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child(task.description.clone()),
                )
                .child(
                  div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(status_text),
                ),
            )
            // Progress bar (if staged)
            .when(has_stages, |el| {
              el.child(
                div()
                  .flex_1()
                  .h(px(4.))
                  .rounded(px(2.))
                  .bg(colors.border)
                  .child(
                    div()
                      .h_full()
                      .rounded(px(2.))
                      .bg(colors.primary)
                      .w(gpui::relative(progress)),
                  ),
              )
            })
            // Stage counter (if staged)
            .when(has_stages, |el| {
              el.child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(format!(
                    "{}/{}",
                    task.current_stage + 1,
                    task.stages.len()
                  )),
              )
            })
        })),
    )
  }

  fn execute_palette_action(&mut self, action: PaletteAction, window: &mut Window, cx: &mut Context<'_, Self>) {
    match action {
      // Navigation actions
      PaletteAction::Navigate(view) => {
        crate::services::set_view(view, cx);
      }

      // Refresh actions
      PaletteAction::RefreshAll => {
        crate::services::load_initial_data(cx);
      }
      PaletteAction::RefreshContainers => {
        crate::services::refresh_containers(cx);
      }
      PaletteAction::RefreshImages => {
        crate::services::refresh_images(cx);
      }
      PaletteAction::RefreshVolumes => {
        crate::services::refresh_volumes(cx);
      }
      PaletteAction::RefreshNetworks => {
        crate::services::refresh_networks(cx);
      }
      PaletteAction::RefreshMachines => {
        crate::services::refresh_machines(cx);
      }
      PaletteAction::RefreshPods => {
        crate::services::refresh_pods(cx);
      }
      PaletteAction::RefreshDeployments => {
        crate::services::refresh_deployments(cx);
      }
      PaletteAction::RefreshServices => {
        crate::services::refresh_services(cx);
      }

      // Dialog actions - use centralized dialog helpers
      PaletteAction::ShowPullImageDialog => {
        dialogs::open_pull_image_dialog(window, cx);
      }
      PaletteAction::ShowCreateVolumeDialog => {
        dialogs::open_create_volume_dialog(window, cx);
      }
      PaletteAction::ShowCreateNetworkDialog => {
        dialogs::open_create_network_dialog(window, cx);
      }
      PaletteAction::ShowCreateMachineDialog => {
        dialogs::open_create_machine_dialog(window, cx);
      }
      PaletteAction::ShowCreateDeploymentDialog => {
        dialogs::open_create_deployment_dialog(window, cx);
      }
      PaletteAction::ShowCreateServiceDialog => {
        dialogs::open_create_service_dialog(window, cx);
      }
      PaletteAction::ShowPruneDialog => {
        dialogs::open_prune_dialog(window, cx);
      }

      // Machine actions (default profile)
      PaletteAction::StartDefaultMachine => {
        crate::services::start_colima(None, cx);
      }
      PaletteAction::StopDefaultMachine => {
        crate::services::stop_colima(None, cx);
      }
      PaletteAction::RestartDefaultMachine => {
        crate::services::restart_colima(None, cx);
      }

      // Kubernetes actions
      PaletteAction::ResetKubernetes => {
        crate::services::reset_colima_kubernetes(cx);
      }

      // UI actions
      PaletteAction::ShowShortcuts => {
        self.show_shortcuts_overlay = true;
      }
    }
    cx.notify();
  }
}

impl Render for DocksideApp {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Check if we need to show setup dialog (from async check)
    if let Some((colima_installed, docker_installed, colima_running)) = self.pending_setup_check.take() {
      // Determine title based on the issue
      let title = if !colima_installed || !docker_installed {
        "Setup Required"
      } else if !colima_running {
        "Start Colima"
      } else {
        "Kubernetes Issue Detected"
      };
      Self::show_setup_dialog_with_title(window, cx, title);
    }

    // Push any pending notifications (centralized handling)
    for (notification_type, message) in self.pending_notifications.drain(..) {
      window.push_notification((notification_type, SharedString::from(message)), cx);
    }

    // Handle pending palette action (needs window access for dialogs)
    if let Some(action) = self.pending_palette_action.take() {
      self.execute_palette_action(action, window, cx);
    }

    // Required layers for gpui-component
    let notification_layer = Root::render_notification_layer(window, cx);
    let dialog_layer = Root::render_dialog_layer(window, cx);

    let sidebar = self.render_sidebar(cx);
    let content = self.render_content(cx);
    let task_bar = Self::render_task_bar(cx);
    let show_shortcuts = self.show_shortcuts_overlay;
    let shortcuts_overlay = if show_shortcuts {
      Some(Self::render_shortcuts_overlay(cx))
    } else {
      None
    };
    let command_palette = self.command_palette.clone();
    let global_search = self.global_search.clone();

    // Get colors after mutable borrows are done
    let colors = &cx.theme().colors;
    let background = colors.background;
    let border = colors.border;

    // Focus the app on initial load only - don't steal focus from inputs/dialogs
    // The .track_focus() on the main element will handle focus for keyboard shortcuts
    // when the user clicks on the main app area (via the on_click handler below)

    h_flex()
      .id("dockside-app")
      .key_context("DocksideApp")
      .track_focus(&self.focus_handle)
      .size_full()
      .bg(background)
      .overflow_hidden()
      // Regain focus when clicking on the main app area
      .on_click(cx.listener(|this, _, window, _cx| {
        if this.command_palette.is_none() && this.global_search.is_none() {
          this.focus_handle.focus(window);
        }
      }))
      // Navigation action handlers
      .on_action(cx.listener(|_this, _: &GoToContainers, _window, cx| {
        crate::services::set_view(CurrentView::Containers, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToCompose, _window, cx| {
        crate::services::set_view(CurrentView::Compose, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToImages, _window, cx| {
        crate::services::set_view(CurrentView::Images, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToVolumes, _window, cx| {
        crate::services::set_view(CurrentView::Volumes, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToNetworks, _window, cx| {
        crate::services::set_view(CurrentView::Networks, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToPods, _window, cx| {
        crate::services::set_view(CurrentView::Pods, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToDeployments, _window, cx| {
        crate::services::set_view(CurrentView::Deployments, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToServices, _window, cx| {
        crate::services::set_view(CurrentView::Services, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToMachines, _window, cx| {
        crate::services::set_view(CurrentView::Machines, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToActivityMonitor, _window, cx| {
        crate::services::set_view(CurrentView::ActivityMonitor, cx);
      }))
      .on_action(cx.listener(|_this, _: &GoToSettings, _window, cx| {
        crate::services::set_view(CurrentView::Settings, cx);
      }))
      // Common action handlers
      .on_action(cx.listener(|_this, _: &Refresh, _window, cx| {
        crate::services::load_initial_data(cx);
      }))
      .on_action(cx.listener(|this, _: &ShowKeyboardShortcuts, _window, cx| {
        this.toggle_shortcuts_overlay(cx);
      }))
      // Command palette
      .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
        this.open_command_palette(window, cx);
      }))
      .on_action(cx.listener(|this, _: &NewResource, window, cx| {
        let current_view = this.docker_state.read(cx).current_view;
        match current_view {
          CurrentView::Containers => {
            // For containers, guide user to pull an image first
            dialogs::open_pull_image_dialog(window, cx);
          }
          CurrentView::Images => {
            dialogs::open_pull_image_dialog(window, cx);
          }
          CurrentView::Volumes => {
            dialogs::open_create_volume_dialog(window, cx);
          }
          CurrentView::Networks => {
            dialogs::open_create_network_dialog(window, cx);
          }
          CurrentView::Machines => {
            dialogs::open_create_machine_dialog(window, cx);
          }
          CurrentView::Deployments => {
            dialogs::open_create_deployment_dialog(window, cx);
          }
          CurrentView::Services => {
            dialogs::open_create_service_dialog(window, cx);
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("No create action for this view.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
        this.open_global_search(window, cx);
      }))
      // Resource actions - use global selection
      .on_action(cx.listener(|this, _: &StartSelected, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::start_container(container.id, cx);
          }
          Selection::Machine(name) => {
            crate::services::start_machine(name, cx);
          }
          Selection::None => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a resource first.")),
              cx,
            );
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Start not available for this resource type.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &StopSelected, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::stop_container(container.id, cx);
          }
          Selection::Machine(name) => {
            crate::services::stop_machine(name, cx);
          }
          Selection::None => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a resource first.")),
              cx,
            );
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Stop not available for this resource type.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &RestartSelected, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::restart_container(container.id, cx);
          }
          Selection::Machine(name) => {
            crate::services::restart_machine(name, cx);
          }
          Selection::Pod { name, namespace } => {
            crate::services::restart_pod(name, namespace, cx);
          }
          Selection::Deployment { name, namespace } => {
            crate::services::restart_deployment(name, namespace, cx);
          }
          Selection::None => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a resource first.")),
              cx,
            );
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Restart not available for this resource type.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &DeleteSelected, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::delete_container(container.id, cx);
          }
          Selection::Image(image) => {
            crate::services::delete_image(image.id, cx);
          }
          Selection::Volume(name) => {
            crate::services::delete_volume(name, cx);
          }
          Selection::Network(id) => {
            crate::services::delete_network(id, cx);
          }
          Selection::Pod { name, namespace } => {
            crate::services::delete_pod(name, namespace, cx);
          }
          Selection::Deployment { name, namespace } => {
            crate::services::delete_deployment(name, namespace, cx);
          }
          Selection::Service { name, namespace } => {
            crate::services::delete_service(name, namespace, cx);
          }
          Selection::Machine(name) => {
            crate::services::delete_machine(name, cx);
          }
          Selection::None => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a resource first.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &ViewLogs, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::set_view(CurrentView::Containers, cx);
            crate::services::open_container_logs(container.id, cx);
          }
          Selection::Pod { name, namespace } => {
            crate::services::set_view(CurrentView::Pods, cx);
            crate::services::open_pod_logs(name, namespace, cx);
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a container or pod first.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &OpenTerminal, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::set_view(CurrentView::Containers, cx);
            crate::services::open_container_terminal(container.id, cx);
          }
          Selection::Pod { name, namespace } => {
            crate::services::set_view(CurrentView::Pods, cx);
            crate::services::open_pod_terminal(name, namespace, cx);
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a container or pod first.")),
              cx,
            );
          }
        }
      }))
      .on_action(cx.listener(|this, _: &InspectSelected, window, cx| {
        let selection = this.docker_state.read(cx).selection.clone();
        match selection {
          Selection::Container(container) => {
            crate::services::set_view(CurrentView::Containers, cx);
            crate::services::open_container_inspect(container.id, cx);
          }
          Selection::Image(image) => {
            crate::services::inspect_image(image.id, cx);
          }
          _ => {
            window.push_notification(
              (NotificationType::Info, SharedString::from("Select a container or image first.")),
              cx,
            );
          }
        }
      }))
      .child(
        div()
          .w(px(220.))
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .border_r_1()
          .border_color(border)
          .child(sidebar),
      )
      .child(
        div()
          .flex_1()
          .h_full()
          .overflow_hidden()
          .flex()
          .flex_col()
          .child(div().flex_1().overflow_hidden().child(content))
          .children(task_bar),
      )
      // Command palette
      .children(command_palette)
      // Global search
      .children(global_search)
      // Keyboard shortcuts overlay
      .children(shortcuts_overlay)
      // gpui-component layers
      .children(notification_layer)
      .children(dialog_layer)
  }
}
