use gpui::{App, Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Root, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  notification::NotificationType,
  sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
  theme::{ActiveTheme, Theme, ThemeMode},
};

use crate::assets::AppIcon;
use crate::services::{DispatcherEvent, dispatcher, task_manager};
use crate::state::{CurrentView, DockerState, StateChanged, docker_state};
use crate::ui::activity::ActivityMonitorView;
use crate::ui::compose::ComposeView;
use crate::ui::containers::ContainersView;
use crate::ui::deployments::DeploymentsView;
use crate::ui::images::ImagesView;
use crate::ui::machines::MachinesView;
use crate::ui::networks::NetworksView;
use crate::ui::pods::PodsView;
use crate::ui::prune_dialog::PruneDialog;
use crate::ui::services::ServicesView;
use crate::ui::settings::SettingsView;
use crate::ui::setup_dialog::{SetupDialog, is_colima_installed, is_docker_installed};
use crate::ui::volumes::VolumesView;

/// Main application - only handles layout and view switching
pub struct DockerApp {
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
}

impl DockerApp {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    Theme::change(ThemeMode::Dark, Some(window), cx);

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

    // Check if Docker/Colima are installed and show setup dialog if not
    let colima_installed = is_colima_installed();
    let docker_installed = is_docker_installed();

    if !colima_installed || !docker_installed {
      // Defer showing the dialog until after the window is ready
      cx.defer_in(window, |_this, window, cx| {
        Self::show_setup_dialog(window, cx);
      });
    }

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
    }
  }

  fn show_setup_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(SetupDialog::new);

    window.open_dialog(cx, move |dialog, _window, cx| {
      let dialog_clone = dialog_entity.clone();
      let _colors = cx.theme().colors;

      dialog
        .title("Setup Required")
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
    let dialog_entity = cx.new(PruneDialog::new);

    window.open_dialog(cx, move |dialog, _window, _cx| {
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("Prune Docker Resources")
        .min_w(px(450.))
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
                    crate::services::prune_docker(&options, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
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
                                .icon(IconName::LayoutDashboard)
                                .active(current_view == CurrentView::Pods)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Pods, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Deployments")
                                .icon(IconName::GalleryVerticalEnd)
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
                SidebarGroup::new("Linux").child(
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
        .py(px(6.))
        .px(px(16.))
        .bg(colors.background)
        .border_b_1()
        .border_color(colors.border)
        .gap(px(16.))
        .items_center()
        .children(running_tasks.iter().map(|task| {
          h_flex()
            .gap(px(8.))
            .items_center()
            .child(div().text_sm().text_color(colors.link).child(task.description.clone()))
        })),
    )
  }
}

impl Render for DockerApp {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Push any pending notifications (centralized handling)
    for (notification_type, message) in self.pending_notifications.drain(..) {
      window.push_notification((notification_type, SharedString::from(message)), cx);
    }

    // Required layers for gpui-component
    let notification_layer = Root::render_notification_layer(window, cx);
    let dialog_layer = Root::render_dialog_layer(window, cx);

    let sidebar = self.render_sidebar(cx);
    let content = self.render_content(cx);
    let task_bar = Self::render_task_bar(cx);

    // Get colors after mutable borrows are done
    let colors = &cx.theme().colors;
    let background = colors.background;
    let border = colors.border;

    h_flex()
            .size_full()
            .bg(background)
            .overflow_hidden()
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
            // gpui-component layers
            .children(notification_layer)
            .children(dialog_layer)
  }
}
