//! Tabbed wrapper for k8s workload resources: Pods, Deployments,
//! `StatefulSets`, `DaemonSets`, Jobs, `CronJobs`.

use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Selectable, h_flex,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::ui::components::render_namespace_selector;
use crate::ui::cronjobs::CronJobsView;
use crate::ui::daemonsets::DaemonSetsView;
use crate::ui::deployments::DeploymentsView;
use crate::ui::jobs::JobsView;
use crate::ui::pods::PodsView;
use crate::ui::statefulsets::StatefulSetsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkloadsTab {
  #[default]
  Pods,
  Deployments,
  StatefulSets,
  DaemonSets,
  Jobs,
  CronJobs,
}

pub struct WorkloadsView {
  active_tab: WorkloadsTab,
  pods: Entity<PodsView>,
  deployments: Entity<DeploymentsView>,
  statefulsets: Entity<StatefulSetsView>,
  daemonsets: Entity<DaemonSetsView>,
  jobs: Entity<JobsView>,
  cronjobs: Entity<CronJobsView>,
}

impl WorkloadsView {
  pub fn new(
    pods: Entity<PodsView>,
    deployments: Entity<DeploymentsView>,
    statefulsets: Entity<StatefulSetsView>,
    daemonsets: Entity<DaemonSetsView>,
    jobs: Entity<JobsView>,
    cronjobs: Entity<CronJobsView>,
  ) -> Self {
    Self {
      active_tab: WorkloadsTab::Pods,
      pods,
      deployments,
      statefulsets,
      daemonsets,
      jobs,
      cronjobs,
    }
  }
}

impl Render for WorkloadsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let active = self.active_tab;

    let make_tab = |label: &'static str, tab: WorkloadsTab, cx: &mut Context<'_, Self>| {
      Tab::new()
        .label(label)
        .selected(active == tab)
        .on_click(cx.listener(move |this, _ev, _w, cx| {
          this.active_tab = tab;
          cx.notify();
        }))
    };

    let tab_bar = TabBar::new("workloads-tabs")
      .child(make_tab("Pods", WorkloadsTab::Pods, cx))
      .child(make_tab("Deployments", WorkloadsTab::Deployments, cx))
      .child(make_tab("StatefulSets", WorkloadsTab::StatefulSets, cx))
      .child(make_tab("DaemonSets", WorkloadsTab::DaemonSets, cx))
      .child(make_tab("Jobs", WorkloadsTab::Jobs, cx))
      .child(make_tab("CronJobs", WorkloadsTab::CronJobs, cx));

    let body = match active {
      WorkloadsTab::Pods => div().size_full().child(self.pods.clone()),
      WorkloadsTab::Deployments => div().size_full().child(self.deployments.clone()),
      WorkloadsTab::StatefulSets => div().size_full().child(self.statefulsets.clone()),
      WorkloadsTab::DaemonSets => div().size_full().child(self.daemonsets.clone()),
      WorkloadsTab::Jobs => div().size_full().child(self.jobs.clone()),
      WorkloadsTab::CronJobs => div().size_full().child(self.cronjobs.clone()),
    };

    v_flex()
      .size_full()
      .child(
        h_flex()
          .w_full()
          .items_center()
          .flex_shrink_0()
          .border_b_1()
          .border_color(colors.border)
          .child(div().flex_1().min_w_0().overflow_hidden().child(tab_bar))
          .child(
            h_flex()
              .px(px(12.))
              .flex_shrink_0()
              .child(render_namespace_selector(cx)),
          ),
      )
      .child(div().flex_1().min_h_0().child(body))
  }
}
