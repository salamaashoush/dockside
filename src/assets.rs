use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};
use gpui_component::IconNamed;
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// Embed project-specific assets
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
pub struct ProjectAssets;

/// Combined asset source that loads from both gpui-component and project assets
pub struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    if path.is_empty() {
      return Ok(None);
    }

    // First try project assets
    if let Some(data) = ProjectAssets::get(path) {
      return Ok(Some(data.data));
    }

    // Fall back to gpui-component assets
    gpui_component_assets::Assets
      .load(path)
      .map_err(|_| anyhow!("could not find asset at path \"{path}\""))
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    // Combine listings from both sources
    let mut result: Vec<SharedString> = ProjectAssets::iter()
      .filter_map(|p| p.starts_with(path).then(|| p.into()))
      .collect();

    if let Ok(component_assets) = gpui_component_assets::Assets.list(path) {
      for asset in component_assets {
        if !result.contains(&asset) {
          result.push(asset);
        }
      }
    }

    Ok(result)
  }
}

/// Application-specific icons not available in gpui-component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIcon {
  // Actions
  Play,
  Stop,
  Restart,
  Trash,
  Plus,
  Search,
  // Resources
  Container,
  Image,
  Volume,
  Network,
  Pod,
  Deployment,
  Service,
  Machine,
  // UI elements
  Terminal,
  Logs,
  Files,
  Activity,
  ChevronRight,
  ChevronDown,
  // Platforms
  Kubernetes,
}

impl IconNamed for AppIcon {
  fn path(self) -> SharedString {
    match self {
      // Actions
      Self::Play => "icons/play.svg",
      Self::Stop => "icons/stop.svg",
      Self::Restart => "icons/restart.svg",
      Self::Trash => "icons/trash.svg",
      Self::Plus => "icons/plus.svg",
      Self::Search => "icons/search.svg",
      // Resources
      Self::Container => "icons/container.svg",
      Self::Image => "icons/image.svg",
      Self::Volume => "icons/volume.svg",
      Self::Network => "icons/network.svg",
      Self::Pod => "icons/pod.svg",
      Self::Deployment => "icons/deployment.svg",
      Self::Service => "icons/service.svg",
      Self::Machine => "icons/machine.svg",
      // UI elements
      Self::Terminal => "icons/terminal.svg",
      Self::Logs => "icons/logs.svg",
      Self::Files => "icons/files.svg",
      Self::Activity => "icons/activity.svg",
      Self::ChevronRight => "icons/chevron-right.svg",
      Self::ChevronDown => "icons/chevron-down.svg",
      // Platforms
      Self::Kubernetes => "icons/kubernetes.svg",
    }
    .into()
  }
}
