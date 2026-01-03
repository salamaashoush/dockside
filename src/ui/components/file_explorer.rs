use gpui::{App, Entity, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

/// Trait for file entry types that can be displayed in the file explorer.
/// All file entry types (ContainerFileEntry, VolumeFileEntry, VmFileEntry) implement this.
pub trait FileEntry: Clone {
  fn name(&self) -> &str;
  fn path(&self) -> &str;
  fn is_dir(&self) -> bool;
  fn is_symlink(&self) -> bool;
  fn size(&self) -> u64;
  fn permissions(&self) -> &str;
  fn display_size(&self) -> String;

  /// Optional extended fields for VM file entries
  fn owner(&self) -> Option<&str> {
    None
  }
  fn group(&self) -> Option<&str> {
    None
  }
  fn modified(&self) -> Option<&str> {
    None
  }
}

// Implement FileEntry for all file types

impl FileEntry for crate::docker::ContainerFileEntry {
  fn name(&self) -> &str {
    &self.name
  }
  fn path(&self) -> &str {
    &self.path
  }
  fn is_dir(&self) -> bool {
    self.is_dir
  }
  fn is_symlink(&self) -> bool {
    self.is_symlink
  }
  fn size(&self) -> u64 {
    self.size
  }
  fn permissions(&self) -> &str {
    &self.permissions
  }
  fn display_size(&self) -> String {
    self.display_size()
  }
}

impl FileEntry for crate::docker::VolumeFileEntry {
  fn name(&self) -> &str {
    &self.name
  }
  fn path(&self) -> &str {
    &self.path
  }
  fn is_dir(&self) -> bool {
    self.is_dir
  }
  fn is_symlink(&self) -> bool {
    self.is_symlink
  }
  fn size(&self) -> u64 {
    self.size
  }
  fn permissions(&self) -> &str {
    &self.permissions
  }
  fn display_size(&self) -> String {
    self.display_size()
  }
}

impl FileEntry for crate::colima::VmFileEntry {
  fn name(&self) -> &str {
    &self.name
  }
  fn path(&self) -> &str {
    &self.path
  }
  fn is_dir(&self) -> bool {
    self.is_dir
  }
  fn is_symlink(&self) -> bool {
    self.is_symlink
  }
  fn size(&self) -> u64 {
    self.size
  }
  fn permissions(&self) -> &str {
    &self.permissions
  }
  fn display_size(&self) -> String {
    self.display_size()
  }
  fn owner(&self) -> Option<&str> {
    Some(&self.owner)
  }
  fn group(&self) -> Option<&str> {
    Some(&self.group)
  }
  fn modified(&self) -> Option<&str> {
    Some(&self.modified)
  }
}

type NavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type FileSelectCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type CloseViewerCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type SymlinkClickCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// State for the file explorer
#[derive(Debug, Clone, Default)]
pub struct FileExplorerState {
  /// Current directory path
  pub current_path: String,
  /// Whether files are loading
  pub is_loading: bool,
  /// Selected file path for viewing
  pub selected_file: Option<String>,
  /// Content of selected file
  pub file_content: String,
  /// Whether file content is loading
  pub file_content_loading: bool,
}

impl FileExplorerState {
  pub fn new() -> Self {
    Self {
      current_path: "/".to_string(),
      ..Default::default()
    }
  }

  pub fn with_path(mut self, path: impl Into<String>) -> Self {
    self.current_path = path.into();
    self
  }
}

/// Configuration options for the file explorer
#[derive(Clone)]
pub struct FileExplorerConfig {
  /// Show owner/group columns (for VM file entries)
  pub show_owner: bool,
  /// Show modified time column
  pub show_modified: bool,
  /// Empty directory message
  pub empty_message: String,
  /// Disabled message (e.g., "Container must be running")
  pub disabled_message: Option<String>,
}

impl Default for FileExplorerConfig {
  fn default() -> Self {
    Self {
      show_owner: false,
      show_modified: false,
      empty_message: "Directory is empty".to_string(),
      disabled_message: None,
    }
  }
}

impl FileExplorerConfig {
  pub fn show_owner(mut self, show: bool) -> Self {
    self.show_owner = show;
    self
  }

  pub fn show_modified(mut self, show: bool) -> Self {
    self.show_modified = show;
    self
  }

  pub fn empty_message(mut self, msg: impl Into<String>) -> Self {
    self.empty_message = msg.into();
    self
  }

  pub fn disabled_message(mut self, msg: impl Into<String>) -> Self {
    self.disabled_message = Some(msg.into());
    self
  }
}

/// Generic file explorer component that works with any FileEntry type
pub struct FileExplorer<F: FileEntry + 'static> {
  files: Vec<F>,
  state: FileExplorerState,
  config: FileExplorerConfig,
  file_content_editor: Option<Entity<InputState>>,
  on_navigate: Option<NavigateCallback>,
  on_file_select: Option<FileSelectCallback>,
  on_close_viewer: Option<CloseViewerCallback>,
  on_symlink_click: Option<SymlinkClickCallback>,
}

impl<F: FileEntry + 'static> FileExplorer<F> {
  pub fn new() -> Self {
    Self {
      files: Vec::new(),
      state: FileExplorerState::new(),
      config: FileExplorerConfig::default(),
      file_content_editor: None,
      on_navigate: None,
      on_file_select: None,
      on_close_viewer: None,
      on_symlink_click: None,
    }
  }

  pub fn files(mut self, files: Vec<F>) -> Self {
    self.files = files;
    self
  }

  pub fn state(mut self, state: FileExplorerState) -> Self {
    self.state = state;
    self
  }

  pub fn config(mut self, config: FileExplorerConfig) -> Self {
    self.config = config;
    self
  }

  pub fn file_content_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.file_content_editor = editor;
    self
  }

  pub fn on_navigate<C>(mut self, callback: C) -> Self
  where
    C: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_navigate = Some(Rc::new(callback));
    self
  }

  pub fn on_file_select<C>(mut self, callback: C) -> Self
  where
    C: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_file_select = Some(Rc::new(callback));
    self
  }

  pub fn on_close_viewer<C>(mut self, callback: C) -> Self
  where
    C: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_close_viewer = Some(Rc::new(callback));
    self
  }

  /// Set callback for when a symlink is clicked (to resolve and follow it)
  pub fn on_symlink_click<C>(mut self, callback: C) -> Self
  where
    C: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_symlink_click = Some(Rc::new(callback));
    self
  }

  /// Render the file explorer
  pub fn render(self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Check if disabled
    if let Some(ref disabled_msg) = self.config.disabled_message {
      return self.render_disabled(disabled_msg, cx).into_any_element();
    }

    // Check if viewing file content
    if let Some(ref file_path) = self.state.selected_file {
      return self.render_file_viewer(file_path, cx).into_any_element();
    }

    // Render directory browser
    let current_path = &self.state.current_path;
    let is_loading = self.state.is_loading;

    let on_navigate = self.on_navigate.clone();
    let on_navigate_up = self.on_navigate.clone();
    let on_file_select = self.on_file_select.clone();
    let on_symlink_click = self.on_symlink_click.clone();

    // Calculate parent path
    let parent_path = calculate_parent_path(current_path);

    let mut file_list = v_flex().gap(px(2.));

    for file in self.files.iter() {
      let file_path_nav = file.path().to_string();
      let file_path_select = file.path().to_string();
      let file_path_symlink = file.path().to_string();
      let is_dir = file.is_dir();
      let is_symlink = file.is_symlink();
      let nav_cb = on_navigate.clone();
      let select_cb = on_file_select.clone();
      let symlink_cb = on_symlink_click.clone();

      file_list = file_list.child(
        self
          .render_file_entry(file, cx)
          .cursor_pointer()
          // Symlinks get special handling - resolve and follow
          .when(is_symlink && symlink_cb.is_some(), |el| {
            el.when_some(symlink_cb, move |el, cb| {
              let path = file_path_symlink.clone();
              el.on_mouse_down(gpui::MouseButton::Left, move |_ev, window, cx| {
                cb(&path, window, cx);
              })
            })
          })
          // Directories navigate into them
          .when(is_dir && !is_symlink, |el| {
            el.when_some(nav_cb, move |el, cb| {
              let path = file_path_nav.clone();
              el.on_mouse_down(gpui::MouseButton::Left, move |_ev, window, cx| {
                cb(&path, window, cx);
              })
            })
          })
          // Regular files open file viewer
          .when(!is_dir && !is_symlink, |el| {
            el.when_some(select_cb, move |el, cb| {
              let path = file_path_select.clone();
              el.on_mouse_down(gpui::MouseButton::Left, move |_ev, window, cx| {
                cb(&path, window, cx);
              })
            })
          }),
      );
    }

    v_flex()
      .size_full()
      .min_h_0()
      .p(px(16.))
      .gap(px(8.))
      .child(
        h_flex()
          .flex_shrink_0()
          .items_center()
          .gap(px(8.))
          .child(Button::new("up").icon(IconName::ArrowUp).ghost().compact().when_some(
            on_navigate_up,
            move |btn, cb| {
              let path = parent_path.clone();
              btn.on_click(move |_ev, window, cx| {
                cb(&path, window, cx);
              })
            },
          ))
          .child(
            div()
              .flex_1()
              .px(px(12.))
              .py(px(8.))
              .bg(colors.background)
              .rounded(px(6.))
              .text_sm()
              .text_color(colors.secondary_foreground)
              .child(current_path.clone()),
          ),
      )
      .child(
        div()
          .id("file-explorer-list")
          .flex_1()
          .min_h_0()
          .w_full()
          .bg(colors.background)
          .rounded(px(8.))
          .p(px(8.))
          .overflow_y_scrollbar()
          .when(is_loading, |el| {
            el.child(
              div()
                .p(px(16.))
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Loading..."),
            )
          })
          .when(!is_loading && self.files.is_empty(), |el| {
            el.child(
              div()
                .p(px(16.))
                .text_sm()
                .text_color(colors.muted_foreground)
                .child(self.config.empty_message.clone()),
            )
          })
          .when(!is_loading && !self.files.is_empty(), |el| el.child(file_list)),
      )
      .into_any_element()
  }

  fn render_disabled(&self, message: &str, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .items_center()
      .justify_center()
      .gap(px(16.))
      .child(
        Icon::new(IconName::Folder)
          .size(px(48.))
          .text_color(colors.muted_foreground),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child(message.to_string()),
      )
  }

  fn render_file_viewer(&self, file_path: &str, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let is_loading = self.state.file_content_loading;
    let on_close = self.on_close_viewer.clone();

    // Extract file name from path
    let file_name = file_path
      .rsplit('/')
      .next()
      .unwrap_or(file_path)
      .to_string();

    div()
      .size_full()
      .flex()
      .flex_col()
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .items_center()
          .gap(px(8.))
          .flex_shrink_0()
          .child(
            Button::new("back")
              .icon(IconName::ArrowLeft)
              .ghost()
              .compact()
              .when_some(on_close, |btn, cb| {
                btn.on_click(move |_ev, window, cx| {
                  cb(&(), window, cx);
                })
              }),
          )
          .child(Icon::new(IconName::File).text_color(colors.secondary_foreground))
          .child(
            div()
              .flex_1()
              .text_sm()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.foreground)
              .overflow_hidden()
              .text_ellipsis()
              .child(file_name),
          )
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .overflow_hidden()
              .text_ellipsis()
              .child(file_path.to_string()),
          ),
      )
      .when(is_loading, |el| {
        el.child(
          div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Loading file..."),
            ),
        )
      })
      .when(!is_loading, |el| {
        if let Some(ref editor) = self.file_content_editor {
          el.child(
            div()
              .flex_1()
              .min_h_0()
              .child(Input::new(editor).size_full().appearance(false)),
          )
        } else {
          // Fallback if no editor
          let content = self.state.file_content.clone();

          el.child(
            div()
              .flex_1()
              .min_h_0()
              .overflow_y_scrollbar()
              .p(px(12.))
              .font_family("monospace")
              .text_xs()
              .text_color(colors.foreground)
              .child(content),
          )
        }
      })
  }

  fn render_file_entry(&self, file: &F, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let icon = if file.is_dir() {
      IconName::Folder
    } else if file.is_symlink() {
      IconName::ExternalLink
    } else {
      IconName::File
    };

    let icon_color = if file.is_dir() {
      colors.warning
    } else if file.is_symlink() {
      colors.info
    } else {
      colors.secondary_foreground
    };

    let mut row = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(8.))
      .rounded(px(4.))
      .items_center()
      .gap(px(10.))
      .hover(|s| s.bg(colors.sidebar))
      .child(Icon::new(icon).text_color(icon_color))
      .child(
        div()
          .flex_1()
          .text_sm()
          .text_color(colors.foreground)
          .child(file.name().to_string()),
      );

    // Size column
    row = row.child(
      div()
        .text_xs()
        .text_color(colors.muted_foreground)
        .w(px(70.))
        .child(file.display_size()),
    );

    // Owner column (optional)
    if self.config.show_owner {
      if let Some(owner) = file.owner() {
        row = row.child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .w(px(60.))
            .overflow_hidden()
            .text_ellipsis()
            .child(owner.to_string()),
        );
      }
    }

    // Permissions column
    row = row.child(
      div()
        .text_xs()
        .text_color(colors.muted_foreground)
        .w(px(80.))
        .child(file.permissions().to_string()),
    );

    // Modified column (optional)
    if self.config.show_modified {
      if let Some(modified) = file.modified() {
        row = row.child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .w(px(120.))
            .overflow_hidden()
            .text_ellipsis()
            .child(modified.to_string()),
        );
      }
    }

    row
  }
}

impl<F: FileEntry + 'static> Default for FileExplorer<F> {
  fn default() -> Self {
    Self::new()
  }
}

/// Calculate parent path from current path
fn calculate_parent_path(current_path: &str) -> String {
  if current_path == "/" {
    "/".to_string()
  } else {
    let parts: Vec<&str> = current_path.trim_end_matches('/').split('/').collect();
    if parts.len() <= 2 {
      "/".to_string()
    } else {
      parts[..parts.len() - 1].join("/")
    }
  }
}

/// Detect programming language from file path for syntax highlighting
pub fn detect_language_from_path(path: &str) -> &'static str {
  let extension = path.rsplit('.').next().unwrap_or("").to_lowercase();
  match extension.as_str() {
    // Rust
    "rs" => "rust",
    // JavaScript/TypeScript
    "js" | "mjs" | "cjs" => "javascript",
    "ts" | "mts" | "cts" => "typescript",
    "jsx" => "jsx",
    "tsx" => "tsx",
    // Web
    "html" | "htm" => "html",
    "css" => "css",
    "scss" | "sass" => "scss",
    "less" => "less",
    // Data formats
    "json" => "json",
    "yaml" | "yml" => "yaml",
    "toml" => "toml",
    "xml" => "xml",
    "csv" => "csv",
    // Config files
    "ini" | "cfg" | "conf" => "ini",
    "env" => "dotenv",
    // Shell
    "sh" | "bash" | "zsh" => "shell",
    "fish" => "fish",
    "ps1" | "psm1" => "powershell",
    // Systems
    "c" | "h" => "c",
    "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
    "go" => "go",
    "py" | "pyw" => "python",
    "rb" => "ruby",
    "php" => "php",
    "java" => "java",
    "kt" | "kts" => "kotlin",
    "swift" => "swift",
    "m" | "mm" => "objective-c",
    "cs" => "csharp",
    "fs" | "fsi" | "fsx" => "fsharp",
    "scala" => "scala",
    "lua" => "lua",
    "pl" | "pm" => "perl",
    "r" => "r",
    "jl" => "julia",
    "ex" | "exs" => "elixir",
    "erl" | "hrl" => "erlang",
    "hs" | "lhs" => "haskell",
    "clj" | "cljs" | "cljc" => "clojure",
    "lisp" | "lsp" | "cl" => "lisp",
    "scm" | "ss" => "scheme",
    // Documentation
    "md" | "markdown" => "markdown",
    "rst" => "restructuredtext",
    "tex" | "latex" => "latex",
    // Database
    "sql" => "sql",
    // Docker/DevOps
    "dockerfile" => "dockerfile",
    "tf" | "tfvars" => "terraform",
    "nix" => "nix",
    // Logs
    "log" => "log",
    // Default
    _ => {
      // Check for common config files by name
      let filename = path.rsplit('/').next().unwrap_or(path).to_lowercase();
      match filename.as_str() {
        "dockerfile" => "dockerfile",
        "makefile" | "gnumakefile" => "makefile",
        "cmakelists.txt" => "cmake",
        ".gitignore" | ".dockerignore" => "gitignore",
        ".bashrc" | ".zshrc" | ".profile" => "shell",
        "cargo.toml" | "cargo.lock" => "toml",
        "package.json" | "tsconfig.json" => "json",
        _ => "plaintext",
      }
    }
  }
}
