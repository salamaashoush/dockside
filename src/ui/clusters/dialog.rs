//! Dialogs for the Clusters editor: manual add, file import, rename.

use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Sizable, h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};

use crate::kubernetes::{AuthMethod, NewCluster};
use crate::ui::components::form_field;

/// Manual "Add cluster" form (token / insecure / CA). Exec, OIDC and
/// client-cert configs are richer than a form — import those from a file.
pub struct ClusterFormDialog {
  focus_handle: FocusHandle,
  name: Option<Entity<InputState>>,
  server: Option<Entity<InputState>>,
  namespace: Option<Entity<InputState>>,
  token: Option<Entity<InputState>>,
  exec_cmd: Option<Entity<InputState>>,
  client_cert: Option<Entity<InputState>>,
  client_key: Option<Entity<InputState>>,
  ca: Option<Entity<InputState>>,
  insecure: bool,
}

impl ClusterFormDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      name: None,
      server: None,
      namespace: None,
      token: None,
      exec_cmd: None,
      client_cert: None,
      client_key: None,
      ca: None,
      insecure: false,
    }
  }

  fn ensure(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.name.is_none() {
      self.name = Some(cx.new(|cx| InputState::new(window, cx).placeholder("context name")));
      self.server = Some(cx.new(|cx| InputState::new(window, cx).placeholder("https://1.2.3.4:6443")));
      self.namespace = Some(cx.new(|cx| InputState::new(window, cx).placeholder("default (optional)")));
      self.token = Some(cx.new(|cx| InputState::new(window, cx).placeholder("bearer token (optional)")));
      self.exec_cmd = Some(cx.new(|cx| {
        InputState::new(window, cx).placeholder("exec command + args, e.g. aws eks get-token --cluster-name x")
      }));
      self.client_cert = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .placeholder("client certificate PEM (optional)")
      }));
      self.client_key = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .placeholder("client key PEM (optional)")
      }));
      self.ca = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .placeholder("-----BEGIN CERTIFICATE----- (optional)")
      }));
    }
  }

  pub fn build(&self, cx: &App) -> NewCluster {
    let read = |i: &Option<Entity<InputState>>| {
      i.as_ref()
        .map(|s| s.read(cx).text().to_string())
        .unwrap_or_default()
        .trim()
        .to_string()
    };
    let ns = read(&self.namespace);
    let ca = read(&self.ca);
    let exec = read(&self.exec_cmd);
    let cert = read(&self.client_cert);
    let key = read(&self.client_key);

    // Precedence: exec plugin > client cert > bearer token. Exec covers
    // the cloud cases (aws-iam-authenticator, gke-gcloud-auth-plugin).
    let auth = if !exec.is_empty() {
      let mut parts = exec.split_whitespace().map(ToString::to_string);
      let command = parts.next().unwrap_or_default();
      AuthMethod::Exec {
        command,
        args: parts.collect(),
        env: Vec::new(),
      }
    } else if !cert.is_empty() && !key.is_empty() {
      AuthMethod::ClientCert {
        cert_pem: cert,
        key_pem: key,
      }
    } else {
      AuthMethod::Token(read(&self.token))
    };

    NewCluster {
      context_name: read(&self.name),
      server: read(&self.server),
      ca_pem: if ca.is_empty() { None } else { Some(ca) },
      insecure: self.insecure,
      namespace: if ns.is_empty() { None } else { Some(ns) },
      auth,
    }
  }
}

impl Focusable for ClusterFormDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ClusterFormDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure(window, cx);
    let insecure = self.insecure;
    let form = v_flex()
      .w_full()
      .p(px(16.))
      .gap(px(14.))
      .child(form_field(
        "Context name",
        Input::new(self.name.as_ref().unwrap()).w_full().small(),
        None,
        cx,
      ))
      .child(form_field(
        "API server",
        Input::new(self.server.as_ref().unwrap()).w_full().small(),
        None,
        cx,
      ))
      .child(form_field(
        "Namespace",
        Input::new(self.namespace.as_ref().unwrap()).w_full().small(),
        Some("Default namespace for this context (optional)."),
        cx,
      ))
      .child(form_field(
        "Token",
        Input::new(self.token.as_ref().unwrap()).w_full().small(),
        Some("Bearer token. Leave blank to use exec or client cert."),
        cx,
      ))
      .child(form_field(
        "Exec plugin",
        Input::new(self.exec_cmd.as_ref().unwrap()).w_full().small(),
        Some("e.g. aws eks get-token --cluster-name my-cluster"),
        cx,
      ))
      .child(form_field(
        "Client cert (PEM)",
        div()
          .h(px(70.))
          .child(Input::new(self.client_cert.as_ref().unwrap()).w_full()),
        None,
        cx,
      ))
      .child(form_field(
        "Client key (PEM)",
        div()
          .h(px(70.))
          .child(Input::new(self.client_key.as_ref().unwrap()).w_full()),
        None,
        cx,
      ))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(
            Switch::new("insecure")
              .checked(insecure)
              .on_click(cx.listener(|this, checked: &bool, _w, cx| {
                this.insecure = *checked;
                cx.notify();
              })),
          )
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().colors.foreground)
              .child("Skip TLS verify"),
          ),
      )
      .child(form_field(
        "CA certificate (PEM)",
        div().h(px(90.)).child(Input::new(self.ca.as_ref().unwrap()).w_full()),
        None,
        cx,
      ));

    v_flex()
      .w_full()
      .max_h(px(520.))
      .child(div().w_full().flex_1().overflow_y_scrollbar().child(form))
  }
}

/// Single-field path prompt for "Import kubeconfig file".
pub struct ImportDialog {
  focus_handle: FocusHandle,
  path: Option<Entity<InputState>>,
}

impl ImportDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      path: None,
    }
  }

  pub fn path(&self, cx: &App) -> String {
    self
      .path
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default()
      .trim()
      .to_string()
  }
}

impl Focusable for ImportDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ImportDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    if self.path.is_none() {
      self.path =
        Some(cx.new(|cx| InputState::new(window, cx).placeholder("/path/to/kubeconfig or ~/Downloads/cluster.yaml")));
    }
    let colors = &cx.theme().colors;
    v_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .gap(px(8.))
      .child(
        Label::new("Kubeconfig file path")
          .text_color(colors.muted_foreground),
      )
      .child(Input::new(self.path.as_ref().unwrap()).w_full())
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Clusters, users and contexts are merged into your primary kubeconfig (same-named entries overwritten). A .dockside.bak backup is written first."),
      )
  }
}

/// Rename + re-namespace prompt for an existing context.
pub struct RenameDialog {
  focus_handle: FocusHandle,
  original: String,
  name: Option<Entity<InputState>>,
  namespace: Option<Entity<InputState>>,
}

impl RenameDialog {
  pub fn new(original: String, cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      original,
      name: None,
      namespace: None,
    }
  }

  pub fn original(&self) -> &str {
    &self.original
  }

  pub fn values(&self, cx: &App) -> (String, Option<String>) {
    let name = self
      .name
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default()
      .trim()
      .to_string();
    let ns = self
      .namespace
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default()
      .trim()
      .to_string();
    let name = if name.is_empty() { self.original.clone() } else { name };
    (name, if ns.is_empty() { None } else { Some(ns) })
  }
}

impl Focusable for RenameDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for RenameDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    if self.name.is_none() {
      let ph = format!("new name (current: {})", self.original);
      self.name = Some(cx.new(|cx| InputState::new(window, cx).placeholder(ph)));
      self.namespace = Some(cx.new(|cx| InputState::new(window, cx).placeholder("default namespace (optional)")));
    }
    let colors = &cx.theme().colors;
    v_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .gap(px(10.))
      .child(Label::new("Context name").text_color(colors.muted_foreground))
      .child(Input::new(self.name.as_ref().unwrap()).w_full())
      .child(Label::new("Default namespace").text_color(colors.muted_foreground))
      .child(Input::new(self.namespace.as_ref().unwrap()).w_full())
  }
}
