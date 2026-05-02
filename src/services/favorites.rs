//! Favorite item persistence and navigation.

use gpui::App;

use crate::state::{CurrentView, FavoriteRef, Selection, SettingsChanged, StateChanged, docker_state, settings_state};

/// Toggle a favorite: add it if absent, remove it if present. Persists.
pub fn toggle_favorite(item: FavoriteRef, cx: &mut App) {
  let settings_entity = settings_state(cx);
  settings_entity.update(cx, |s, cx| {
    if let Some(idx) = s.settings.favorites.iter().position(|f| f == &item) {
      s.settings.favorites.remove(idx);
    } else {
      s.settings.favorites.push(item);
    }
    if let Err(e) = s.settings.save() {
      tracing::warn!("Failed to save favorites: {e}");
    }
    cx.emit(SettingsChanged::SettingsUpdated);
  });
}

/// Read-only check whether `item` is currently a favorite.
pub fn is_favorite(item: &FavoriteRef, cx: &App) -> bool {
  settings_state(cx).read(cx).settings.favorites.contains(item)
}

/// Navigate the app to the resource referenced by a favorite. Sets both the
/// `current_view` and (for resources that have detail panes) the global
/// `Selection` so the detail view opens immediately.
pub fn open_favorite(item: &FavoriteRef, cx: &mut App) {
  let state = docker_state(cx);
  match item {
    FavoriteRef::Container { id, .. } => {
      let info = state.read(cx).containers.iter().find(|c| c.id == *id).cloned();
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Containers);
        if let Some(c) = info {
          s.set_selection(Selection::Container(c));
        }
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Image { id, .. } => {
      let info = state.read(cx).images.iter().find(|i| i.id == *id).cloned();
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Images);
        if let Some(i) = info {
          s.set_selection(Selection::Image(i));
        }
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Volume { name } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Volumes);
        s.set_selection(Selection::Volume(name.clone()));
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Network { id, .. } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Networks);
        s.set_selection(Selection::Network(id.clone()));
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Pod { name, namespace } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Pods);
        s.set_selection(Selection::Pod {
          name: name.clone(),
          namespace: namespace.clone(),
        });
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Deployment { name, namespace } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Deployments);
        s.set_selection(Selection::Deployment {
          name: name.clone(),
          namespace: namespace.clone(),
        });
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Service { name, namespace } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Services);
        s.set_selection(Selection::Service {
          name: name.clone(),
          namespace: namespace.clone(),
        });
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::StatefulSet { name, namespace } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::StatefulSets);
        s.set_selection(Selection::StatefulSet {
          name: name.clone(),
          namespace: namespace.clone(),
        });
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::DaemonSet { name, namespace } => {
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::DaemonSets);
        s.set_selection(Selection::DaemonSet {
          name: name.clone(),
          namespace: namespace.clone(),
        });
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
    FavoriteRef::Machine { id, .. } => {
      use crate::colima::MachineId;
      let parsed = if id == "host" {
        Some(MachineId::Host)
      } else {
        Some(MachineId::Colima(id.clone()))
      };
      state.update(cx, |s, cx| {
        s.set_view(CurrentView::Machines);
        if let Some(mid) = parsed {
          s.set_selection(Selection::Machine(mid));
        }
        cx.emit(StateChanged::ViewChanged);
        cx.emit(StateChanged::SelectionChanged);
      });
    }
  }
}
