//! Command Palette Component
//!
//! A fuzzy search overlay that lets users quickly access any action in the app.
//! Triggered by Cmd+K.

use gpui::{
  App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, MouseButton, Render, SharedString, Styled,
  Window, anchored, div, point, prelude::*, px,
};
use gpui_component::{
  Icon, IconName,
  input::{Input, InputState},
  theme::ActiveTheme,
  v_flex,
};

use crate::state::CurrentView;

// Actions for keyboard navigation within the palette
gpui::actions!(command_palette, [Cancel, SelectUp, SelectDown, Confirm]);

const CONTEXT: &str = "CommandPalette";

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    KeyBinding::new("up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("enter", Confirm, Some(CONTEXT)),
  ]);
}

/// Command that can be executed from the palette
#[derive(Clone)]
pub struct PaletteCommand {
  pub id: &'static str,
  pub label: &'static str,
  pub shortcut: Option<&'static str>,
  pub category: &'static str,
  pub icon: IconName,
  pub action: PaletteAction,
}

/// Action to execute when a command is selected
#[derive(Clone, Copy, Debug)]
pub enum PaletteAction {
  // Navigation
  Navigate(CurrentView),

  // Refresh actions
  RefreshAll,
  RefreshContainers,
  RefreshImages,
  RefreshVolumes,
  RefreshNetworks,
  RefreshMachines,
  RefreshPods,
  RefreshDeployments,
  RefreshServices,

  // Create/Dialog actions
  ShowPullImageDialog,
  ShowCreateVolumeDialog,
  ShowCreateNetworkDialog,
  ShowCreateMachineDialog,
  ShowCreateDeploymentDialog,
  ShowCreateServiceDialog,
  ShowPruneDialog,

  // Machine actions (default profile)
  StartDefaultMachine,
  StopDefaultMachine,
  RestartDefaultMachine,

  // Kubernetes actions
  ResetKubernetes,

  // UI actions
  ShowShortcuts,
}

/// Event emitted when the command palette performs an action
#[derive(Clone, Debug)]
pub enum CommandPaletteEvent {
  Close,
  Action(PaletteAction),
}

/// The command palette view
pub struct CommandPalette {
  query: String,
  input_state: Entity<InputState>,
  focus_handle: FocusHandle,
  selected_index: usize,
  filtered_commands: Vec<PaletteCommand>,
}

impl CommandPalette {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command..."));

    // Focus the input immediately
    input_state.update(cx, |input, cx| {
      input.focus(window, cx);
    });

    let mut palette = Self {
      query: String::new(),
      input_state,
      focus_handle,
      selected_index: 0,
      filtered_commands: Vec::new(),
    };

    palette.filtered_commands = Self::filter_commands("");
    palette
  }

  fn all_commands() -> Vec<PaletteCommand> {
    vec![
      // === NAVIGATION ===
      PaletteCommand {
        id: "nav-containers",
        label: "Go to Containers",
        shortcut: Some("Cmd+1"),
        category: "Navigation",
        icon: IconName::SquareTerminal,
        action: PaletteAction::Navigate(CurrentView::Containers),
      },
      PaletteCommand {
        id: "nav-compose",
        label: "Go to Compose",
        shortcut: Some("Cmd+2"),
        category: "Navigation",
        icon: IconName::LayoutDashboard,
        action: PaletteAction::Navigate(CurrentView::Compose),
      },
      PaletteCommand {
        id: "nav-images",
        label: "Go to Images",
        shortcut: Some("Cmd+3"),
        category: "Navigation",
        icon: IconName::GalleryVerticalEnd,
        action: PaletteAction::Navigate(CurrentView::Images),
      },
      PaletteCommand {
        id: "nav-volumes",
        label: "Go to Volumes",
        shortcut: Some("Cmd+4"),
        category: "Navigation",
        icon: IconName::Folder,
        action: PaletteAction::Navigate(CurrentView::Volumes),
      },
      PaletteCommand {
        id: "nav-networks",
        label: "Go to Networks",
        shortcut: Some("Cmd+5"),
        category: "Navigation",
        icon: IconName::Globe,
        action: PaletteAction::Navigate(CurrentView::Networks),
      },
      PaletteCommand {
        id: "nav-pods",
        label: "Go to Pods",
        shortcut: Some("Cmd+6"),
        category: "Navigation",
        icon: IconName::Globe,
        action: PaletteAction::Navigate(CurrentView::Pods),
      },
      PaletteCommand {
        id: "nav-deployments",
        label: "Go to Deployments",
        shortcut: Some("Cmd+7"),
        category: "Navigation",
        icon: IconName::Copy,
        action: PaletteAction::Navigate(CurrentView::Deployments),
      },
      PaletteCommand {
        id: "nav-services",
        label: "Go to Services",
        shortcut: Some("Cmd+8"),
        category: "Navigation",
        icon: IconName::Globe,
        action: PaletteAction::Navigate(CurrentView::Services),
      },
      PaletteCommand {
        id: "nav-machines",
        label: "Go to Machines",
        shortcut: Some("Cmd+9"),
        category: "Navigation",
        icon: IconName::Frame,
        action: PaletteAction::Navigate(CurrentView::Machines),
      },
      PaletteCommand {
        id: "nav-activity",
        label: "Go to Activity Monitor",
        shortcut: Some("Cmd+0"),
        category: "Navigation",
        icon: IconName::ChartPie,
        action: PaletteAction::Navigate(CurrentView::ActivityMonitor),
      },
      PaletteCommand {
        id: "nav-settings",
        label: "Go to Settings",
        shortcut: Some("Cmd+,"),
        category: "Navigation",
        icon: IconName::Settings,
        action: PaletteAction::Navigate(CurrentView::Settings),
      },
      // === REFRESH ACTIONS ===
      PaletteCommand {
        id: "refresh-all",
        label: "Refresh All Data",
        shortcut: Some("Cmd+R"),
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshAll,
      },
      PaletteCommand {
        id: "refresh-containers",
        label: "Refresh Containers",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshContainers,
      },
      PaletteCommand {
        id: "refresh-images",
        label: "Refresh Images",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshImages,
      },
      PaletteCommand {
        id: "refresh-volumes",
        label: "Refresh Volumes",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshVolumes,
      },
      PaletteCommand {
        id: "refresh-networks",
        label: "Refresh Networks",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshNetworks,
      },
      PaletteCommand {
        id: "refresh-machines",
        label: "Refresh Machines",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshMachines,
      },
      PaletteCommand {
        id: "refresh-pods",
        label: "Refresh Pods",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshPods,
      },
      PaletteCommand {
        id: "refresh-deployments",
        label: "Refresh Deployments",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshDeployments,
      },
      PaletteCommand {
        id: "refresh-services",
        label: "Refresh Services",
        shortcut: None,
        category: "Refresh",
        icon: IconName::Redo,
        action: PaletteAction::RefreshServices,
      },
      // === CREATE/DIALOG ACTIONS ===
      PaletteCommand {
        id: "pull-image",
        label: "Pull Image",
        shortcut: None,
        category: "Docker",
        icon: IconName::ArrowDown,
        action: PaletteAction::ShowPullImageDialog,
      },
      PaletteCommand {
        id: "create-volume",
        label: "Create Volume",
        shortcut: None,
        category: "Docker",
        icon: IconName::Plus,
        action: PaletteAction::ShowCreateVolumeDialog,
      },
      PaletteCommand {
        id: "create-network",
        label: "Create Network",
        shortcut: None,
        category: "Docker",
        icon: IconName::Plus,
        action: PaletteAction::ShowCreateNetworkDialog,
      },
      PaletteCommand {
        id: "prune",
        label: "Prune Docker Resources",
        shortcut: None,
        category: "Docker",
        icon: IconName::Delete,
        action: PaletteAction::ShowPruneDialog,
      },
      // === COLIMA MACHINE ACTIONS ===
      PaletteCommand {
        id: "create-machine",
        label: "Create Colima Machine",
        shortcut: None,
        category: "Colima",
        icon: IconName::Plus,
        action: PaletteAction::ShowCreateMachineDialog,
      },
      PaletteCommand {
        id: "start-default-machine",
        label: "Start Default Machine",
        shortcut: None,
        category: "Colima",
        icon: IconName::ChevronRight,
        action: PaletteAction::StartDefaultMachine,
      },
      PaletteCommand {
        id: "stop-default-machine",
        label: "Stop Default Machine",
        shortcut: None,
        category: "Colima",
        icon: IconName::Minus,
        action: PaletteAction::StopDefaultMachine,
      },
      PaletteCommand {
        id: "restart-default-machine",
        label: "Restart Default Machine",
        shortcut: None,
        category: "Colima",
        icon: IconName::Redo,
        action: PaletteAction::RestartDefaultMachine,
      },
      // === KUBERNETES ACTIONS ===
      PaletteCommand {
        id: "create-deployment",
        label: "Create Deployment",
        shortcut: None,
        category: "Kubernetes",
        icon: IconName::Plus,
        action: PaletteAction::ShowCreateDeploymentDialog,
      },
      PaletteCommand {
        id: "create-service",
        label: "Create Service",
        shortcut: None,
        category: "Kubernetes",
        icon: IconName::Plus,
        action: PaletteAction::ShowCreateServiceDialog,
      },
      PaletteCommand {
        id: "reset-kubernetes",
        label: "Reset Kubernetes Cluster",
        shortcut: None,
        category: "Kubernetes",
        icon: IconName::Redo,
        action: PaletteAction::ResetKubernetes,
      },
      // === UI ACTIONS ===
      PaletteCommand {
        id: "show-shortcuts",
        label: "Show Keyboard Shortcuts",
        shortcut: Some("?"),
        category: "Help",
        icon: IconName::Info,
        action: PaletteAction::ShowShortcuts,
      },
    ]
  }

  fn filter_commands(query: &str) -> Vec<PaletteCommand> {
    let query = query.to_lowercase();
    if query.is_empty() {
      return Self::all_commands();
    }

    let mut commands = Self::all_commands();
    commands.retain(|cmd| {
      let label = cmd.label.to_lowercase();
      let category = cmd.category.to_lowercase();

      // Simple fuzzy matching: check if query chars appear in order
      if label.contains(&query) || category.contains(&query) {
        return true;
      }

      // Check if all query chars appear in order in the label
      let mut query_chars = query.chars().peekable();
      for c in label.chars() {
        if query_chars.peek() == Some(&c) {
          query_chars.next();
        }
      }
      query_chars.peek().is_none()
    });

    // Sort by relevance (exact matches first, then by label)
    commands.sort_by(|a, b| {
      let a_label = a.label.to_lowercase();
      let b_label = b.label.to_lowercase();

      // Exact prefix match gets priority
      let a_starts = a_label.starts_with(&query);
      let b_starts = b_label.starts_with(&query);

      if a_starts && !b_starts {
        std::cmp::Ordering::Less
      } else if !a_starts && b_starts {
        std::cmp::Ordering::Greater
      } else {
        a_label.cmp(&b_label)
      }
    });

    commands
  }

  fn on_query_changed(&mut self, cx: &mut Context<'_, Self>) {
    self.query = self.input_state.read(cx).text().to_string();
    self.filtered_commands = Self::filter_commands(&self.query);
    self.selected_index = 0;
    cx.notify();
  }

  fn select_up(&mut self, _: &SelectUp, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    if !self.filtered_commands.is_empty() {
      self.selected_index = if self.selected_index == 0 {
        self.filtered_commands.len() - 1
      } else {
        self.selected_index - 1
      };
      cx.notify();
    }
  }

  fn select_down(&mut self, _: &SelectDown, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    if !self.filtered_commands.is_empty() {
      self.selected_index = (self.selected_index + 1) % self.filtered_commands.len();
      cx.notify();
    }
  }

  fn confirm(&mut self, _: &Confirm, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    self.execute_selected(cx);
  }

  fn cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<'_, Self>) {
    let _ = &self; // Required by action handler signature
    cx.stop_propagation();
    cx.emit(CommandPaletteEvent::Close);
  }

  fn execute_selected(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(cmd) = self.filtered_commands.get(self.selected_index) {
      cx.emit(CommandPaletteEvent::Action(cmd.action));
      cx.emit(CommandPaletteEvent::Close);
    }
  }

  fn on_item_click(&mut self, ix: usize, cx: &mut Context<'_, Self>) {
    self.selected_index = ix;
    self.execute_selected(cx);
  }
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CommandPalette {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let selected_idx = self.selected_index;
    let commands = self.filtered_commands.clone();
    let view_size = window.viewport_size();

    // Check for input changes
    let current_text = self.input_state.read(cx).text().to_string();
    if current_text != self.query {
      self.on_query_changed(cx);
    }

    // Use anchored to position at top-left, then manually center
    anchored().position(point(px(0.), px(0.))).child(
      div()
          .id("command-palette-overlay")
          .key_context(CONTEXT)
          .track_focus(&self.focus_handle)
          .w(view_size.width)
          .h(view_size.height)
          .bg(gpui::hsla(0.0, 0.0, 0.0, 0.7))
          .flex()
          .justify_center()
          .pt(px(100.))
          .occlude()
          // Action handlers
          .on_action(cx.listener(Self::cancel))
          .on_action(cx.listener(Self::select_up))
          .on_action(cx.listener(Self::select_down))
          .on_action(cx.listener(Self::confirm))
          // Close on click outside
          .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _, cx| {
            cx.stop_propagation();
            cx.emit(CommandPaletteEvent::Close);
          }))
          .child(
            v_flex()
              .w(px(500.))
              .max_h(px(400.))
              .bg(colors.background)
              .border_1()
              .border_color(colors.border)
              .rounded(px(12.))
              .overflow_hidden()
              .occlude()
              // Stop click events from closing the palette when clicking inside
              .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
              })
              // Search input
              .child(
                div()
                  .p(px(12.))
                  .border_b_1()
                  .border_color(colors.border)
                  .child(
                    Input::new(&self.input_state)
                      .cleanable(true)
                      .appearance(false),
                  ),
              )
              // Results list
              .child(
                div()
                  .flex_1()
                  .overflow_hidden()
                  .py(px(8.))
                  .when(commands.is_empty(), |el| {
                    el.child(
                      div()
                        .px(px(16.))
                        .py(px(12.))
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("No commands found"),
                    )
                  })
                  .children(commands.iter().enumerate().map(|(idx, cmd)| {
                    let is_selected = idx == selected_idx;
                    let shortcut = cmd.shortcut;
                    let label = SharedString::from(cmd.label);
                    let category = SharedString::from(cmd.category);
                    let icon = cmd.icon.clone();
                    let item_id = SharedString::from(cmd.id);
                    let group_name = format!("cmd-item-{idx}");

                    div()
                      .id(item_id)
                      .group(SharedString::from(group_name.clone()))
                      .px(px(12.))
                      .py(px(8.))
                      .mx(px(8.))
                      .rounded(px(6.))
                      .cursor_pointer()
                      .when(is_selected, |el| el.bg(colors.accent).text_color(colors.accent_foreground))
                      .when(!is_selected, |el| {
                        el.group_hover(SharedString::from(group_name), |el| {
                          el.bg(colors.accent).text_color(colors.accent_foreground)
                        })
                      })
                      .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered {
                          this.selected_index = idx;
                          cx.notify();
                        }
                      }))
                      .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                      })
                      .on_click(cx.listener(move |this, _, _, cx| {
                        this.on_item_click(idx, cx);
                      }))
                      .child(
                        div()
                          .flex()
                          .items_center()
                          .gap(px(12.))
                          .child(
                            Icon::new(icon)
                              .size(px(16.))
                              .when(!is_selected, |el| el.text_color(colors.muted_foreground)),
                          )
                          .child(
                            div()
                              .flex_1()
                              .flex()
                              .flex_col()
                              .child(
                                div()
                                  .text_sm()
                                  .child(label),
                              )
                              .child(
                                div()
                                  .text_xs()
                                  .when(!is_selected, |el| el.text_color(colors.muted_foreground))
                                  .child(category),
                              ),
                          )
                          .when_some(shortcut, |el, s| {
                            el.child(
                              div()
                                .px(px(8.))
                                .py(px(2.))
                                .bg(colors.border)
                                .rounded(px(4.))
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(s),
                            )
                          }),
                      )
                  })),
              )
              // Footer hint
              .child(
                div()
                  .px(px(16.))
                  .py(px(8.))
                  .border_t_1()
                  .border_color(colors.border)
                  .flex()
                  .items_center()
                  .gap(px(16.))
                  .child(
                    div()
                      .flex()
                      .items_center()
                      .gap(px(4.))
                      .text_xs()
                      .text_color(colors.muted_foreground)
                      .child("Up/Down")
                      .child("to navigate"),
                  )
                  .child(
                    div()
                      .flex()
                      .items_center()
                      .gap(px(4.))
                      .text_xs()
                      .text_color(colors.muted_foreground)
                      .child("Enter")
                      .child("to select"),
                  )
                  .child(
                    div()
                      .flex()
                      .items_center()
                      .gap(px(4.))
                      .text_xs()
                      .text_color(colors.muted_foreground)
                      .child("Esc")
                      .child("to close"),
                  ),
              ),
          ),
    )
  }
}
