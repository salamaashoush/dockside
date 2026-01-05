//! Global keyboard shortcuts and keybinding management
//!
//! This module provides keyboard shortcuts for:
//! - Navigation between views (Cmd+1-9)
//! - Common actions (refresh, new, etc.)
//! - Command palette (Cmd+K)
//! - Help overlay (?)

use gpui::{App, KeyBinding, actions};

// ==================== Global Navigation Actions ====================
actions!(
  dockside,
  [
    // Navigation shortcuts
    GoToContainers,
    GoToCompose,
    GoToVolumes,
    GoToImages,
    GoToNetworks,
    GoToPods,
    GoToDeployments,
    GoToServices,
    GoToMachines,
    GoToActivityMonitor,
    GoToSettings,
    // Common actions
    Refresh,
    NewResource,
    OpenCommandPalette,
    ShowKeyboardShortcuts,
    // Resource actions (work on selected resource)
    StartSelected,
    StopSelected,
    RestartSelected,
    DeleteSelected,
    ViewLogs,
    OpenTerminal,
    InspectSelected,
    // Search
    FocusSearch,
  ]
);

/// Register all keybindings for the application
pub fn register_keybindings(cx: &mut App) {
  // Navigation shortcuts (Cmd+1 through Cmd+0)
  cx.bind_keys([
    // Primary views (Cmd+1-5)
    KeyBinding::new("cmd-1", GoToContainers, None),
    KeyBinding::new("cmd-2", GoToCompose, None),
    KeyBinding::new("cmd-3", GoToImages, None),
    KeyBinding::new("cmd-4", GoToVolumes, None),
    KeyBinding::new("cmd-5", GoToNetworks, None),
    // Kubernetes views (Cmd+6-8)
    KeyBinding::new("cmd-6", GoToPods, None),
    KeyBinding::new("cmd-7", GoToDeployments, None),
    KeyBinding::new("cmd-8", GoToServices, None),
    // Other views (Cmd+9, Cmd+0)
    KeyBinding::new("cmd-9", GoToMachines, None),
    KeyBinding::new("cmd-0", GoToActivityMonitor, None),
    // Settings (Cmd+,)
    KeyBinding::new("cmd-,", GoToSettings, None),
    // Common actions
    KeyBinding::new("cmd-r", Refresh, None),
    KeyBinding::new("cmd-n", NewResource, None),
    KeyBinding::new("cmd-k", OpenCommandPalette, None),
    KeyBinding::new("shift-/", ShowKeyboardShortcuts, None), // ? key
    // Resource actions (work on selected resource)
    KeyBinding::new("cmd-enter", StartSelected, None),
    KeyBinding::new("cmd-.", StopSelected, None),
    KeyBinding::new("cmd-shift-r", RestartSelected, None),
    KeyBinding::new("cmd-backspace", DeleteSelected, None),
    KeyBinding::new("cmd-l", ViewLogs, None),
    KeyBinding::new("cmd-t", OpenTerminal, None),
    KeyBinding::new("cmd-i", InspectSelected, None),
    // Search
    KeyBinding::new("cmd-f", FocusSearch, None),
    KeyBinding::new("/", FocusSearch, None),
  ]);
}

/// Keyboard shortcuts data for display in the help overlay
pub struct KeyboardShortcut {
  pub keys: &'static str,
  pub description: &'static str,
  pub category: &'static str,
}

/// Get all keyboard shortcuts for display
pub fn get_all_shortcuts() -> Vec<KeyboardShortcut> {
  vec![
    // Navigation
    KeyboardShortcut {
      keys: "Cmd+1",
      description: "Go to Containers",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+2",
      description: "Go to Compose",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+3",
      description: "Go to Images",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+4",
      description: "Go to Volumes",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+5",
      description: "Go to Networks",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+6",
      description: "Go to Pods",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+7",
      description: "Go to Deployments",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+8",
      description: "Go to Services",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+9",
      description: "Go to Machines",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+0",
      description: "Go to Activity Monitor",
      category: "Navigation",
    },
    KeyboardShortcut {
      keys: "Cmd+,",
      description: "Go to Settings",
      category: "Navigation",
    },
    // General
    KeyboardShortcut {
      keys: "Cmd+K",
      description: "Open Command Palette",
      category: "General",
    },
    KeyboardShortcut {
      keys: "Cmd+R",
      description: "Refresh current view",
      category: "General",
    },
    KeyboardShortcut {
      keys: "Cmd+N",
      description: "Create new resource",
      category: "General",
    },
    KeyboardShortcut {
      keys: "Cmd+F or /",
      description: "Focus search",
      category: "General",
    },
    KeyboardShortcut {
      keys: "?",
      description: "Show keyboard shortcuts",
      category: "General",
    },
    // Resource Actions
    KeyboardShortcut {
      keys: "Cmd+Enter",
      description: "Start selected (container/machine)",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+.",
      description: "Stop selected (container/machine)",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+Shift+R",
      description: "Restart selected resource",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+Backspace",
      description: "Delete selected resource",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+L",
      description: "View logs (container/pod)",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+T",
      description: "Open terminal (container/pod)",
      category: "Actions",
    },
    KeyboardShortcut {
      keys: "Cmd+I",
      description: "Inspect (container/image)",
      category: "Actions",
    },
  ]
}
