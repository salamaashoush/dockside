//! Clusters editor: list every kubeconfig context with provenance and
//! manage them (add / import / rename / set-current / test / remove).

use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use super::dialog::{ClusterFormDialog, ImportDialog, RenameDialog};
use crate::assets::AppIcon;
use crate::kubernetes::KubeContextInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};
use crate::ui::components::{k8s_header_title, render_k8s_header};

pub struct ClustersView {
  docker_state: Entity<DockerState>,
}

impl ClustersView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);
    cx.subscribe(&docker_state, |_this, _s, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::KubeContextsUpdated | StateChanged::KubeContextSwitched
      ) {
        cx.notify();
      }
    })
    .detach();
    services::refresh_kube_contexts(cx);
    Self { docker_state }
  }

  fn open_add(window: &mut Window, cx: &mut App) {
    let form = cx.new(ClusterFormDialog::new);
    window.open_dialog(cx, move |dialog, _w, _cx| {
      let f = form.clone();
      dialog
        .title("Add Cluster")
        .min_w(px(520.))
        .child(form.clone())
        .footer(move |_st, _, _w, _cx| {
          let f = f.clone();
          vec![
            Button::new("add")
              .label("Add")
              .primary()
              .on_click(move |_e, window, cx| {
                let spec = f.read(cx).build(cx);
                if spec.context_name.is_empty() || spec.server.is_empty() {
                  return;
                }
                services::add_cluster(spec, cx);
                window.close_dialog(cx);
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn open_import(window: &mut Window, cx: &mut App) {
    let form = cx.new(ImportDialog::new);
    window.open_dialog(cx, move |dialog, _w, _cx| {
      let f = form.clone();
      dialog
        .title("Import Kubeconfig File")
        .min_w(px(520.))
        .child(form.clone())
        .footer(move |_st, _, _w, _cx| {
          let f = f.clone();
          vec![
            Button::new("import")
              .label("Import")
              .primary()
              .on_click(move |_e, window, cx| {
                let p = f.read(cx).path(cx);
                if p.is_empty() {
                  return;
                }
                services::import_kubeconfig_file(std::path::PathBuf::from(p), cx);
                window.close_dialog(cx);
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn open_rename(name: String, window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| RenameDialog::new(name, cx));
    window.open_dialog(cx, move |dialog, _w, _cx| {
      let f = form.clone();
      dialog
        .title("Rename / Re-namespace Context")
        .min_w(px(460.))
        .child(form.clone())
        .footer(move |_st, _, _w, _cx| {
          let f = f.clone();
          vec![
            Button::new("save")
              .label("Save")
              .primary()
              .on_click(move |_e, window, cx| {
                let original = f.read(cx).original().to_string();
                let (new_name, ns) = f.read(cx).values(cx);
                services::edit_kube_context(original, new_name, ns, cx);
                window.close_dialog(cx);
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn render_row(i: usize, ctx: &KubeContextInfo, cx: &Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let dot = if ctx.is_current {
      colors.success
    } else {
      colors.muted_foreground
    };
    let origin = std::path::Path::new(&ctx.origin)
      .file_name()
      .map_or_else(|| ctx.origin.clone(), |f| f.to_string_lossy().into_owned());
    let subtitle = format!(
      "{} · user: {} · {}",
      if ctx.server.is_empty() {
        "<no server>"
      } else {
        &ctx.server
      },
      if ctx.user.is_empty() { "-" } else { &ctx.user },
      origin
    );

    let c = ctx.clone();
    let menu = Button::new(("ctx-menu", i))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _w, _cx| {
        let mut menu = menu;
        if !c.is_current {
          let n = c.name.clone();
          menu = menu.item(
            PopupMenuItem::new("Set as current")
              .icon(IconName::Check)
              .on_click(move |_, _, cx| services::set_current_kube_context(n.clone(), cx)),
          );
        }
        let tn = c.name.clone();
        let rn = c.name.clone();
        let dn = c.name.clone();
        menu
          .item(
            PopupMenuItem::new("Test connection")
              .icon(IconName::Globe)
              .on_click(move |_, _, cx| services::test_kube_connection(tn.clone(), cx)),
          )
          .item(
            PopupMenuItem::new("Rename / namespace")
              .icon(IconName::Settings2)
              .on_click(move |_, window, cx| Self::open_rename(rn.clone(), window, cx)),
          )
          .separator()
          .item(
            PopupMenuItem::new("Remove")
              .icon(Icon::new(AppIcon::Trash))
              .on_click(move |_, _, cx| services::remove_kube_context(dn.clone(), cx)),
          )
      });

    h_flex()
      .w_full()
      .px(px(16.))
      .py(px(10.))
      .gap(px(12.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(div().size(px(8.)).rounded_full().bg(dot).flex_shrink_0())
      .child(
        v_flex()
          .flex_1()
          .min_w_0()
          .gap(px(2.))
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(colors.foreground)
                  .child(ctx.name.clone()),
              )
              .when(ctx.is_current, |el| {
                el.child(
                  div()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(4.))
                    .bg(colors.success.opacity(0.15))
                    .text_xs()
                    .text_color(colors.success)
                    .child("current"),
                )
              }),
          )
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .text_ellipsis()
              .overflow_hidden()
              .whitespace_nowrap()
              .child(subtitle),
          ),
      )
      .child(div().flex_shrink_0().child(menu))
  }
}

impl Render for ClustersView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let contexts = self.docker_state.read(cx).kube_contexts.clone();

    let actions = h_flex()
      .gap(px(8.))
      .items_center()
      .child(
        Button::new("refresh-ctx")
          .icon(Icon::new(AppIcon::Refresh))
          .ghost()
          .compact()
          .on_click(|_, _, cx| services::refresh_kube_contexts(cx)),
      )
      .child(
        Button::new("clusters-add")
          .icon(IconName::Plus)
          .ghost()
          .compact()
          .dropdown_menu(|menu, _w, _cx| {
            menu
              .item(
                PopupMenuItem::new("Add cluster…")
                  .icon(IconName::Plus)
                  .on_click(|_, window, cx| Self::open_add(window, cx)),
              )
              .item(
                PopupMenuItem::new("Import kubeconfig file…")
                  .icon(IconName::File)
                  .on_click(|_, window, cx| Self::open_import(window, cx)),
              )
          }),
      );

    let header = render_k8s_header(
      k8s_header_title("Clusters", format!("{} context(s)", contexts.len()), cx),
      false,
      actions,
      cx,
    );

    let body = if contexts.is_empty() {
      div().size_full().flex().items_center().justify_center().child(
        v_flex()
          .items_center()
          .gap(px(12.))
          .child(
            Icon::new(AppIcon::Machine)
              .size(px(40.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No kubeconfig contexts found. Add or import one."),
          ),
      )
    } else {
      div().size_full().child(
        div().size_full().id("clusters-scroll").overflow_y_scrollbar().child(
          v_flex().w_full().children(
            contexts
              .iter()
              .enumerate()
              .map(|(i, c)| Self::render_row(i, c, cx))
              .collect::<Vec<_>>(),
          ),
        ),
      )
    };

    v_flex()
      .size_full()
      .child(header)
      .child(div().flex_1().min_h_0().child(body))
  }
}
