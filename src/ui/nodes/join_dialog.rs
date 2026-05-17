//! Add Node dialog.
//!
//! Two paths: a read-only distro-aware copy-paste guide (phase 4a) and,
//! below it, SSH remote provisioning (phase 4b) — manage a host
//! inventory and have Dockside SSH in to install + join a node. Remote
//! execution only happens on an explicit "Provision" click.

use gpui::{App, ClipboardItem, Context, Entity, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  IconName, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use crate::kubernetes::{JoinGuide, join_guide};
use crate::services;
use crate::state::{HostEntry, docker_state};

pub struct AddNodeDialog {
  focus_handle: FocusHandle,
  guide: JoinGuide,
  context: String,
  h_name: Option<Entity<InputState>>,
  h_addr: Option<Entity<InputState>>,
  h_user: Option<Entity<InputState>>,
  h_port: Option<Entity<InputState>>,
  h_key: Option<Entity<InputState>>,
  sel_target: Option<String>,
  sel_cp: Option<String>,
  role: String,
}

impl AddNodeDialog {
  fn new(guide: JoinGuide, context: String, cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      guide,
      context,
      h_name: None,
      h_addr: None,
      h_user: None,
      h_port: None,
      h_key: None,
      sel_target: None,
      sel_cp: None,
      role: "worker".to_string(),
    }
  }

  fn ensure(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.h_name.is_none() {
      self.h_name = Some(cx.new(|cx| InputState::new(window, cx).placeholder("name")));
      self.h_addr = Some(cx.new(|cx| InputState::new(window, cx).placeholder("host or IP")));
      self.h_user = Some(cx.new(|cx| InputState::new(window, cx).placeholder("user (default root)")));
      self.h_port = Some(cx.new(|cx| InputState::new(window, cx).placeholder("22")));
      self.h_key = Some(cx.new(|cx| InputState::new(window, cx).placeholder("~/.ssh/id_ed25519 (optional)")));
    }
  }
}

impl Focusable for AddNodeDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl AddNodeDialog {
  fn render_guide(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let g = &self.guide;

    let mut root = v_flex().w_full().gap(px(12.)).child(
      h_flex()
        .gap(px(8.))
        .items_center()
        .child(
          div()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(colors.primary.opacity(0.15))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.primary)
            .child(g.distro.label()),
        )
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(g.title.clone()),
        ),
    );

    if g.distro.is_managed() {
      root = root.child(
        div()
          .w_full()
          .p(px(8.))
          .rounded(px(6.))
          .bg(colors.warning.opacity(0.12))
          .text_xs()
          .text_color(colors.warning)
          .child("Managed cluster — node count is controlled by your cloud provider, not a join token."),
      );
    }

    root = root.child(
      v_flex().w_full().gap(px(6.)).children(
        g.steps
          .iter()
          .enumerate()
          .map(|(i, s)| {
            h_flex()
              .w_full()
              .gap(px(8.))
              .items_start()
              .child(
                div()
                  .flex_shrink_0()
                  .size(px(18.))
                  .rounded_full()
                  .bg(colors.sidebar)
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .flex()
                  .items_center()
                  .justify_center()
                  .child(format!("{}", i + 1)),
              )
              .child(div().flex_1().text_sm().text_color(colors.foreground).child(s.clone()))
          })
          .collect::<Vec<_>>(),
      ),
    );

    if let Some(cmd) = g.command.clone() {
      let cmd_for_copy = cmd.clone();
      root = root.child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .p(px(10.))
              .rounded(px(6.))
              .bg(colors.sidebar)
              .font_family("monospace")
              .text_xs()
              .text_color(colors.foreground)
              .child(cmd.clone()),
          )
          .child(
            Button::new("copy-join-cmd")
              .icon(IconName::Copy)
              .ghost()
              .small()
              .on_click(move |_e, _w, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(cmd_for_copy.clone()));
              }),
          ),
      );
    }

    if let Some(url) = g.console_url.clone() {
      root = root.child(
        Button::new("open-console")
          .label("Open cloud console")
          .icon(IconName::Globe)
          .outline()
          .small()
          .on_click(move |_e, _w, cx| {
            cx.open_url(&url);
          }),
      );
    }
    root
  }

  fn render_ssh(&mut self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let this = cx.entity();
    let ctx = self.context.clone();
    let hosts = services::hosts_for(&ctx, cx);
    let names: Vec<String> = hosts.iter().map(|h| h.name.clone()).collect();
    let role = self.role.clone();
    let sel_target = self.sel_target.clone();
    let sel_cp = self.sel_cp.clone();

    let mut sec = v_flex()
      .w_full()
      .gap(px(8.))
      .child(
        div()
          .text_sm()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.foreground)
          .child("Remote provisioning (SSH)"),
      )
      .child(
        div()
          .w_full()
          .p(px(8.))
          .rounded(px(6.))
          .bg(colors.warning.opacity(0.12))
          .text_xs()
          .text_color(colors.warning)
          .child("Runs the k3s/kubeadm installer as root on the target over SSH. Auth uses your ssh agent / ~/.ssh/config. Every command is logged to audit.log."),
      );

    // Registered hosts.
    for (i, h) in hosts.iter().enumerate() {
      let ctx_rm = ctx.clone();
      let name_rm = h.name.clone();
      sec = sec.child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .py(px(4.))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .text_color(colors.foreground)
              .child(format!("{}  ·  {}@{}:{}", h.name, h.user, h.address, h.port)),
          )
          .child(
            Button::new(("rm-host", i))
              .icon(IconName::Delete)
              .ghost()
              .xsmall()
              .on_click(move |_e, _w, cx| {
                services::remove_host(&ctx_rm, &name_rm, cx);
              }),
          ),
      );
    }

    // Add-host form.
    let ctx_add = ctx.clone();
    let (hn, ha, hu, hp, hk) = (
      self.h_name.clone(),
      self.h_addr.clone(),
      self.h_user.clone(),
      self.h_port.clone(),
      self.h_key.clone(),
    );
    sec = sec
      .child(
        h_flex()
          .w_full()
          .gap(px(6.))
          .when_some(self.h_name.clone(), |el, i| {
            el.child(div().flex_1().child(Input::new(&i).small()))
          })
          .when_some(self.h_addr.clone(), |el, i| {
            el.child(div().flex_1().child(Input::new(&i).small()))
          })
          .when_some(self.h_user.clone(), |el, i| {
            el.child(div().w(px(110.)).child(Input::new(&i).small()))
          })
          .when_some(self.h_port.clone(), |el, i| {
            el.child(div().w(px(60.)).child(Input::new(&i).small()))
          }),
      )
      .child(
        h_flex()
          .w_full()
          .gap(px(6.))
          .items_center()
          .when_some(self.h_key.clone(), |el, i| {
            el.child(div().flex_1().child(Input::new(&i).small()))
          })
          .child(
            Button::new("add-host")
              .label("Add host")
              .outline()
              .small()
              .on_click(move |_e, _w, cx| {
                let read = |i: &Option<Entity<InputState>>| {
                  i.as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default()
                    .trim()
                    .to_string()
                };
                let name = read(&hn);
                let address = read(&ha);
                if name.is_empty() || address.is_empty() {
                  return;
                }
                let user = {
                  let u = read(&hu);
                  if u.is_empty() { "root".to_string() } else { u }
                };
                let port = read(&hp).parse::<u16>().unwrap_or(22);
                services::add_host(
                  ctx_add.clone(),
                  HostEntry {
                    name,
                    address,
                    user,
                    port,
                    identity_file: read(&hk),
                  },
                  cx,
                );
              }),
          ),
      );

    // Provision picker.
    let target_label = sel_target.clone().unwrap_or_else(|| "target host…".to_string());
    let cp_label = sel_cp.clone().unwrap_or_else(|| "control-plane host…".to_string());
    let names_t = names.clone();
    let names_c = names.clone();

    sec = sec.child(
      h_flex()
        .w_full()
        .gap(px(8.))
        .items_center()
        .child(
          Button::new("pick-target")
            .label(format!("Target: {target_label}"))
            .outline()
            .small()
            .dropdown_caret(true)
            .dropdown_menu({
              let this = this.clone();
              move |menu, _w, _cx| {
                let mut menu = menu;
                for n in &names_t {
                  let this = this.clone();
                  let n = n.clone();
                  menu = menu.item(PopupMenuItem::new(n.clone()).on_click(move |_, _, cx| {
                    this.update(cx, |d, cx| {
                      d.sel_target = Some(n.clone());
                      cx.notify();
                    });
                  }));
                }
                menu
              }
            }),
        )
        .child(
          Button::new("pick-cp")
            .label(format!("Source: {cp_label}"))
            .outline()
            .small()
            .dropdown_caret(true)
            .dropdown_menu({
              let this = this.clone();
              move |menu, _w, _cx| {
                let mut menu = menu;
                for n in &names_c {
                  let this = this.clone();
                  let n = n.clone();
                  menu = menu.item(PopupMenuItem::new(n.clone()).on_click(move |_, _, cx| {
                    this.update(cx, |d, cx| {
                      d.sel_cp = Some(n.clone());
                      cx.notify();
                    });
                  }));
                }
                menu
              }
            }),
        )
        .child(
          Button::new("role-toggle")
            .label(format!("Role: {role}"))
            .outline()
            .small()
            .on_click(cx.listener(|this, _, _, cx| {
              this.role = if this.role == "worker" {
                "control-plane".to_string()
              } else {
                "worker".to_string()
              };
              cx.notify();
            })),
        ),
    );

    let provision_hosts = hosts.clone();
    sec.child(
      Button::new("provision")
        .label("Provision over SSH")
        .primary()
        .small()
        .on_click(cx.listener(move |this, _ev, window, cx| {
          let (Some(tn), Some(cn)) = (this.sel_target.clone(), this.sel_cp.clone()) else {
            return;
          };
          let target = provision_hosts.iter().find(|h| h.name == tn).cloned();
          let cp = provision_hosts.iter().find(|h| h.name == cn).cloned();
          if let (Some(target), Some(cp)) = (target, cp) {
            services::provision_node(target, cp, this.role.clone(), cx);
            window.close_dialog(cx);
          }
        })),
    )
  }
}

impl Render for AddNodeDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure(window, cx);
    let colors = cx.theme().colors;
    let guide = self.render_guide(cx);
    let ssh = self.render_ssh(cx);
    let body = v_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .gap(px(14.))
      .child(guide)
      .child(div().w_full().h(px(1.)).bg(colors.border))
      .child(ssh);

    v_flex()
      .w_full()
      .max_h(px(560.))
      .child(div().w_full().flex_1().overflow_y_scrollbar().child(body))
  }
}

/// Open the Add Node dialog for the active cluster.
pub fn open_add_node_dialog(window: &mut Window, cx: &mut App) {
  let (nodes, server, context) = {
    let state_entity = docker_state(cx);
    let state = state_entity.read(cx);
    let current = state.current_kube_context_name();
    let ctx = state
      .kube_contexts
      .iter()
      .find(|c| Some(c.name.as_str()) == current.as_deref() || c.is_current);
    let server = ctx.map(|c| c.server.clone()).unwrap_or_default();
    let name = current.unwrap_or_default();
    (state.nodes.clone(), server, name)
  };
  let guide = join_guide(&nodes, &server);
  let entity = cx.new(|cx| AddNodeDialog::new(guide, context, cx));

  window.open_dialog(cx, move |dialog, _w, _cx| {
    dialog
      .title("Add Node")
      .min_w(px(600.))
      .child(entity.clone())
      .footer(move |_st, _, _w, _cx| {
        vec![
          Button::new("close-add-node")
            .label("Close")
            .on_click(move |_e, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}
