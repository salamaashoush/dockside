use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use crate::docker::NetworkInfo;
use crate::state::{DockerState, Selection, StateChanged, docker_state};
use crate::ui::dialogs;

use super::detail::NetworkDetail;
use super::list::{NetworkList, NetworkListEvent};

/// Self-contained Networks view - handles list, detail, and all state
pub struct NetworksView {
  docker_state: Entity<DockerState>,
  network_list: Entity<NetworkList>,
  // View-specific state (not selection - that's in global DockerState)
  active_tab: usize,
}

impl NetworksView {
  /// Get the currently selected network from global state
  fn selected_network(&self, cx: &App) -> Option<NetworkInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Network(ref id) = state.selection {
      state.networks.iter().find(|n| n.id == *id).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Create network list entity
    let network_list = cx.new(|cx| NetworkList::new(window, cx));

    // Subscribe to network list events
    cx.subscribe_in(
      &network_list,
      window,
      |this, _list, event: &NetworkListEvent, window, cx| match event {
        NetworkListEvent::Selected(network) => {
          this.on_select_network(network.as_ref(), cx);
        }
        NetworkListEvent::CreateNetwork => {
          Self::show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to state changes
    cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
      if let StateChanged::NetworksUpdated = event {
        // If selected network was deleted, clear selection
        let selected_id = {
          if let Selection::Network(ref id) = this.docker_state.read(cx).selection {
            Some(id.clone())
          } else {
            None
          }
        };

        if let Some(id) = selected_id {
          let ds = state.read(cx);
          if ds.networks.iter().any(|n| n.id == id) {
            // Network still exists, nothing to update (selection stores ID, not full data)
          } else {
            // Network was deleted
            this.docker_state.update(cx, |s, _| {
              s.set_selection(Selection::None);
            });
            this.active_tab = 0;
          }
        }
        cx.notify();
      }
    })
    .detach();

    Self {
      docker_state,
      network_list,
      active_tab: 0,
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_create_network_dialog(window, cx);
  }

  fn on_select_network(&mut self, network: &NetworkInfo, cx: &mut Context<'_, Self>) {
    // Update global selection (single source of truth)
    self.docker_state.update(cx, |state, _cx| {
      state.set_selection(Selection::Network(network.id.clone()));
    });

    // Reset view-specific state
    self.active_tab = 0;

    cx.notify();
  }

  fn on_tab_change(&mut self, tab: usize, cx: &mut Context<'_, Self>) {
    self.active_tab = tab;
    cx.notify();
  }
}

impl Render for NetworksView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let selected_network = self.selected_network(cx);
    let active_tab = self.active_tab;
    let has_selection = selected_network.is_some();

    // Build detail panel
    let detail = NetworkDetail::new()
      .network(selected_network)
      .active_tab(active_tab)
      .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
        this.on_tab_change(*tab, cx);
      }))
      .on_delete(cx.listener(|this, _id: &str, _window, cx| {
        this.docker_state.update(cx, |s, _| s.set_selection(Selection::None));
        this.active_tab = 0;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Network list - fixed width when selected, full width when not
        div()
          .when(has_selection, |el| {
            el.w(px(320.)).border_r_1().border_color(colors.border)
          })
          .when(!has_selection, gpui::Styled::flex_1)
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .child(self.network_list.clone()),
      )
      .when(has_selection, |el| {
        el.child(
          // Right: Detail panel - only shown when selection exists
          div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(detail.render(window, cx)),
        )
      })
  }
}
