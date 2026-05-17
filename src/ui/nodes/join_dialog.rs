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
    let ctx = self.context.clone();
    let hosts = services::hosts_for(&ctx, cx);
    let role = self.role.clone();
    let sel_target = self.sel_target.clone();
    let sel_cp = self.sel_cp.clone();
    let ready = sel_target.is_some() && sel_cp.is_some();

    let section_title = |t: &str| {
      div()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(t.to_string())
    };

    let mut sec = v_flex().w_full().gap(px(14.)).child(
      v_flex()
        .gap(px(4.))
        .child(section_title("Remote provisioning (SSH)"))
        .child(div().text_xs().text_color(colors.muted_foreground).child(
          "Dockside SSHes the source for a join token, then the target to install + join. \
                 Auth uses your ssh agent / ~/.ssh/config. Every command is logged to audit.log.",
        )),
    );

    // ---- registered hosts: pick target + source, or remove ------------------
    let mut host_block = v_flex().w_full().gap(px(4.)).child(
      div()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .child("HOSTS"),
    );

    if hosts.is_empty() {
      host_block = host_block.child(
        div()
          .w_full()
          .py(px(10.))
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No SSH hosts yet — add one below."),
      );
    } else {
      for (i, h) in hosts.iter().enumerate() {
        let is_t = sel_target.as_deref() == Some(h.name.as_str());
        let is_s = sel_cp.as_deref() == Some(h.name.as_str());
        let n_t = h.name.clone();
        let n_s = h.name.clone();
        let ctx_rm = ctx.clone();
        let name_rm = h.name.clone();
        host_block = host_block.child(
          h_flex()
            .w_full()
            .gap(px(8.))
            .items_center()
            .py(px(8.))
            .px(px(10.))
            .rounded(px(6.))
            .bg(colors.sidebar)
            .child(
              v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                  div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child(h.name.clone()),
                )
                .child(
                  div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(format!("{}@{}:{}", h.user, h.address, h.port)),
                ),
            )
            .child(
              Button::new(("pick-target", i))
                .label("Target")
                .when(is_t, Button::primary)
                .when(!is_t, Button::outline)
                .xsmall()
                .on_click(cx.listener(move |this, _, _, cx| {
                  this.sel_target = Some(n_t.clone());
                  cx.notify();
                })),
            )
            .child(
              Button::new(("pick-source", i))
                .label("Source")
                .when(is_s, Button::primary)
                .when(!is_s, Button::outline)
                .xsmall()
                .on_click(cx.listener(move |this, _, _, cx| {
                  this.sel_cp = Some(n_s.clone());
                  cx.notify();
                })),
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
    }
    sec = sec.child(host_block);

    // ---- add-host form: one labelled field per row --------------------------
    let field = |label: &str, input: &Option<Entity<InputState>>| {
      v_flex()
        .w_full()
        .gap(px(4.))
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .when_some(input.clone(), |el, i| el.child(Input::new(&i).w_full().small()))
    };

    let ctx_add = ctx.clone();
    let (hn, ha, hu, hp, hk) = (
      self.h_name.clone(),
      self.h_addr.clone(),
      self.h_user.clone(),
      self.h_port.clone(),
      self.h_key.clone(),
    );
    sec = sec.child(
      v_flex()
        .w_full()
        .gap(px(8.))
        .child(
          div()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.muted_foreground)
            .child("ADD SSH HOST"),
        )
        .child(
          h_flex()
            .w_full()
            .gap(px(10.))
            .child(field("Name", &self.h_name))
            .child(field("Host / IP", &self.h_addr)),
        )
        .child(
          h_flex()
            .w_full()
            .gap(px(10.))
            .child(field("User", &self.h_user))
            .child(field("Port", &self.h_port)),
        )
        .child(field("Identity file (optional)", &self.h_key))
        .child(
          h_flex().w_full().justify_end().child(
            Button::new("add-host")
              .label("Add host")
              .icon(IconName::Plus)
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
        ),
    );

    // ---- role + provision ---------------------------------------------------
    let role_btn = |id: &'static str, value: &'static str, label: &'static str, current: &str| {
      let active = current == value;
      Button::new(id)
        .label(label)
        .when(active, Button::primary)
        .when(!active, Button::outline)
        .xsmall()
        .on_click(cx.listener(move |this, _, _, cx| {
          this.role = value.to_string();
          cx.notify();
        }))
    };

    sec = sec.child(
      h_flex()
        .w_full()
        .gap(px(8.))
        .items_center()
        .child(div().text_xs().text_color(colors.muted_foreground).child("Role"))
        .child(role_btn("role-worker", "worker", "Worker", &role))
        .child(role_btn("role-cp", "control-plane", "Control-plane", &role)),
    );

    let provision_hosts = hosts.clone();
    sec = sec.child(
      Button::new("provision")
        .label("Provision over SSH")
        .primary()
        .when(!ready, Button::outline)
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
    );

    if !ready {
      sec = sec.child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Pick a Target and a Source host above to enable provisioning."),
      );
    }

    sec
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
