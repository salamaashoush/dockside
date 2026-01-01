use gpui::{div, prelude::*, px, App, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    menu::{DropdownMenu, PopupMenuItem},
    theme::ActiveTheme,
    v_flex,
    Icon, IconName, IndexPath, Sizable,
};

use crate::assets::AppIcon;
use crate::colima::ColimaVm;
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Machine list events emitted to parent
pub enum MachineListEvent {
    Selected(ColimaVm),
    NewMachine,
}

/// Delegate for the machine list
pub struct MachineListDelegate {
    docker_state: Entity<DockerState>,
    selected_index: Option<IndexPath>,
}

impl MachineListDelegate {
    fn machines<'a>(&self, cx: &'a App) -> &'a Vec<ColimaVm> {
        &self.docker_state.read(cx).colima_vms
    }
}

impl ListDelegate for MachineListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, cx: &App) -> usize {
        self.machines(cx).len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let machines = self.machines(cx);
        let machine = machines.get(ix.row)?;
        let colors = &cx.theme().colors;

        let is_selected = self.selected_index == Some(ix);
        let is_running = machine.status.is_running();
        let machine_name = machine.name.clone();

        let icon_bg = if is_running {
            colors.primary
        } else {
            colors.muted_foreground
        };

        let subtitle = format!("latest, {}", machine.arch);
        let status_color = if is_running {
            colors.success
        } else {
            colors.muted_foreground
        };

        let item_content = h_flex()
            .w_full()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .size(px(36.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(icon_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(AppIcon::Machine).text_color(colors.background)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .gap(px(2.))
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(Label::new(machine.name.clone())),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(subtitle),
                    ),
            )
            // Status dot
            .child(
                div()
                    .flex_shrink_0()
                    .size(px(8.))
                    .rounded_full()
                    .bg(status_color),
            );

        // Three-dot menu button
        let name = machine_name.clone();
        let running = is_running;
        let row = ix.row;

        let item = ListItem::new(ix)
            .py(px(6.))
            .rounded(px(6.))
            .selected(is_selected)
            .child(item_content)
            .suffix(move |_, _| {
                let n = name.clone();
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(("menu", row))
                            .icon(IconName::Ellipsis)
                            .ghost()
                            .xsmall()
                            .dropdown_menu(move |menu, _window, _cx| {
                                let mut menu = menu;
                                let n = n.clone();

                                if running {
                                    menu = menu
                                        .item(
                                            PopupMenuItem::new("Use Docker Context")
                                                .icon(IconName::Check)
                                                .on_click({
                                                    let n = n.clone();
                                                    move |_, _, cx| {
                                                        services::set_docker_context(n.clone(), cx);
                                                    }
                                                }),
                                        )
                                        .separator()
                                        .item(
                                            PopupMenuItem::new("Stop")
                                                .icon(Icon::new(AppIcon::Stop))
                                                .on_click({
                                                    let n = n.clone();
                                                    move |_, _, cx| {
                                                        services::stop_machine(n.clone(), cx);
                                                    }
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new("Restart")
                                                .icon(Icon::new(AppIcon::Restart))
                                                .on_click({
                                                    let n = n.clone();
                                                    move |_, _, cx| {
                                                        services::restart_machine(n.clone(), cx);
                                                    }
                                                }),
                                        )
                                        .separator()
                                        .item(PopupMenuItem::new("Terminal").icon(Icon::new(AppIcon::Terminal)))
                                        .item(PopupMenuItem::new("Files").icon(Icon::new(AppIcon::Files)));
                                } else {
                                    menu = menu.item(
                                        PopupMenuItem::new("Start")
                                            .icon(Icon::new(AppIcon::Play))
                                            .on_click({
                                                let n = n.clone();
                                                move |_, _, cx| {
                                                    services::start_machine(n.clone(), cx);
                                                }
                                            }),
                                    );
                                }

                                menu = menu.separator().item(
                                    PopupMenuItem::new("Delete")
                                        .icon(Icon::new(AppIcon::Trash))
                                        .on_click({
                                            let n = n.clone();
                                            move |_, _, cx| {
                                                services::delete_machine(n.clone(), cx);
                                            }
                                        }),
                                );

                                menu
                            }),
                    )
            })
            .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.delegate_mut().selected_index = Some(ix);
                cx.notify();
            }));

        Some(item)
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }
}

/// Self-contained machine list component with working context menus
pub struct MachineList {
    docker_state: Entity<DockerState>,
    list_state: Entity<ListState<MachineListDelegate>>,
}

impl MachineList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        let delegate = MachineListDelegate {
            docker_state: docker_state.clone(),
            selected_index: None,
        };

        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        // Subscribe to list events
        cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| {
            match event {
                ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
                    let delegate = state.read(cx).delegate();
                    if let Some(machine) = delegate.machines(cx).get(ix.row) {
                        cx.emit(MachineListEvent::Selected(machine.clone()));
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Subscribe to docker state changes to refresh list
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            if matches!(event, StateChanged::MachinesUpdated) {
                // Notify list state to re-render
                this.list_state.update(cx, |_state, cx| {
                    cx.notify();
                });
                cx.notify();
            }
        })
        .detach();

        Self {
            docker_state,
            list_state,
        }
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        v_flex()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .py(px(48.))
            .child(
                div()
                    .size(px(64.))
                    .rounded(px(12.))
                    .bg(colors.sidebar)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(AppIcon::Machine).text_color(colors.muted_foreground)),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.secondary_foreground)
                    .child("No Machines"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Start Colima to create a Linux VM"),
            )
            .child(
                Button::new("new-machine")
                    .label("New Machine")
                    .primary()
                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                        cx.emit(MachineListEvent::NewMachine);
                    })),
            )
    }
}

impl gpui::EventEmitter<MachineListEvent> for MachineList {}

impl Render for MachineList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.docker_state.read(cx);
        let machines_empty = state.colima_vms.is_empty();

        // Toolbar
        let toolbar = h_flex()
            .h(px(52.))
            .w_full()
            .px(px(16.))
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(Label::new("Machines"))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("search")
                            .icon(Icon::new(AppIcon::Search))
                            .ghost()
                            .compact(),
                    )
                    .child(
                        Button::new("add")
                            .icon(Icon::new(AppIcon::Plus))
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(MachineListEvent::NewMachine);
                            })),
                    ),
            );

        let content: gpui::Div = if machines_empty {
            self.render_empty(cx)
        } else {
            // List needs explicit height for virtualization
            div()
                .size_full()
                .p(px(8.))
                .child(List::new(&self.list_state))
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(toolbar)
            .child(
                div()
                    .id("machine-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
    }
}
