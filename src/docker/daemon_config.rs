//! Docker daemon.json configuration file support

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Docker daemon.json configuration
///
/// This struct represents the daemon.json configuration file used by Docker.
/// It preserves unknown fields via serde flatten so we don't lose any
/// configuration that isn't explicitly modeled.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct DaemonConfig {
    /// Storage driver (e.g., "overlay2", "btrfs")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_driver: Option<String>,

    /// Data root directory (alternative Docker root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_root: Option<String>,

    /// Registries that can be accessed over HTTP (insecure)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub insecure_registries: Vec<String>,

    /// Registry mirrors for pulling images
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry_mirrors: Vec<String>,

    /// DNS servers for containers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,

    /// DNS search domains for containers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns_search: Vec<String>,

    /// Keep containers running during daemon restart
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_restore: Option<bool>,

    /// Enable experimental features
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<bool>,

    /// Enable IPv6 networking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<bool>,

    /// Enable IP forwarding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_forward: Option<bool>,

    /// Enable iptables rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iptables: Option<bool>,

    /// Enable IP masquerading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_masq: Option<bool>,

    /// Enable userland proxy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userland_proxy: Option<bool>,

    /// Default ulimits for containers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ulimits: Option<serde_json::Value>,

    /// Log driver configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_driver: Option<String>,

    /// Log driver options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_opts: Option<serde_json::Map<String, serde_json::Value>>,

    /// Default address pools for networks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_address_pools: Vec<AddressPool>,

    /// Fixed CIDR for bridge network
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_cidr: Option<String>,

    /// Bridge network subnet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bip: Option<String>,

    /// Maximum concurrent downloads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_downloads: Option<u32>,

    /// Maximum concurrent uploads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_uploads: Option<u32>,

    /// Preserve any unknown fields from the config file
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// Address pool configuration for Docker networks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressPool {
    /// Base CIDR for the pool
    pub base: String,
    /// Size of each subnet
    pub size: u32,
}

impl DaemonConfig {
    /// Load daemon.json from the default location for the current platform
    pub async fn load_default() -> anyhow::Result<Self> {
        let path = Self::default_path();
        Self::load_from(&path).await
    }

    /// Load daemon.json from a specific path
    pub async fn load_from(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = tokio::fs::read_to_string(path).await?;
        let config: DaemonConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save daemon.json to the default location
    pub async fn save_default(&self) -> anyhow::Result<()> {
        let path = Self::default_path();
        self.save_to(&path).await
    }

    /// Save daemon.json to a specific path
    pub async fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Get the default path for daemon.json based on the platform
    pub fn default_path() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/etc/docker/daemon.json")
        }
        #[cfg(target_os = "macos")]
        {
            // On macOS, Docker runs inside a VM (Colima, Docker Desktop, etc.)
            // The daemon.json is inside the VM, not on the host
            // For Colima, it's at ~/.colima/<profile>/docker/daemon.json
            // This returns a placeholder; actual path depends on runtime
            PathBuf::from("/etc/docker/daemon.json")
        }
        #[cfg(target_os = "windows")]
        {
            // On Windows with WSL2, daemon.json is inside the WSL2 distro
            PathBuf::from("/etc/docker/daemon.json")
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            PathBuf::from("/etc/docker/daemon.json")
        }
    }

    /// Check if any insecure registries are configured
    pub fn has_insecure_registries(&self) -> bool {
        !self.insecure_registries.is_empty()
    }

    /// Check if any registry mirrors are configured
    pub fn has_registry_mirrors(&self) -> bool {
        !self.registry_mirrors.is_empty()
    }

    /// Check if experimental features are enabled
    pub fn is_experimental(&self) -> bool {
        self.experimental.unwrap_or(false)
    }

    /// Check if live restore is enabled
    pub fn is_live_restore(&self) -> bool {
        self.live_restore.unwrap_or(false)
    }

    /// Check if IPv6 is enabled
    pub fn is_ipv6_enabled(&self) -> bool {
        self.ipv6.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DaemonConfig::default();
        assert!(config.storage_driver.is_none());
        assert!(config.insecure_registries.is_empty());
        assert!(config.registry_mirrors.is_empty());
        assert!(config.dns.is_empty());
        assert!(!config.is_experimental());
        assert!(!config.is_live_restore());
    }

    #[test]
    fn test_serialize_minimal() {
        let config = DaemonConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_serialize_with_values() {
        let config = DaemonConfig {
            storage_driver: Some("overlay2".to_string()),
            insecure_registries: vec!["localhost:5000".to_string()],
            live_restore: Some(true),
            experimental: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("storage-driver"));
        assert!(json.contains("overlay2"));
        assert!(json.contains("insecure-registries"));
        assert!(json.contains("localhost:5000"));
        assert!(json.contains("live-restore"));
        assert!(json.contains("experimental"));
    }

    #[test]
    fn test_deserialize() {
        let json = r#"{
            "storage-driver": "btrfs",
            "insecure-registries": ["registry.local:5000"],
            "registry-mirrors": ["https://mirror.example.com"],
            "dns": ["8.8.8.8", "8.8.4.4"],
            "live-restore": true,
            "experimental": false,
            "ipv6": true,
            "unknown-field": "preserved"
        }"#;

        let config: DaemonConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.storage_driver, Some("btrfs".to_string()));
        assert_eq!(config.insecure_registries, vec!["registry.local:5000"]);
        assert_eq!(config.registry_mirrors, vec!["https://mirror.example.com"]);
        assert_eq!(config.dns, vec!["8.8.8.8", "8.8.4.4"]);
        assert!(config.is_live_restore());
        assert!(!config.is_experimental());
        assert!(config.is_ipv6_enabled());
        // Unknown fields should be preserved
        assert!(config.other.contains_key("unknown-field"));
        assert_eq!(
            config.other.get("unknown-field"),
            Some(&serde_json::Value::String("preserved".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_preserves_unknown_fields() {
        let json = r#"{
            "storage-driver": "overlay2",
            "custom-option": { "nested": true },
            "another-unknown": [1, 2, 3]
        }"#;

        let config: DaemonConfig = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string(&config).unwrap();
        let reparsed: DaemonConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.storage_driver, reparsed.storage_driver);
        assert!(reparsed.other.contains_key("custom-option"));
        assert!(reparsed.other.contains_key("another-unknown"));
    }

    #[test]
    fn test_helper_methods() {
        let config = DaemonConfig {
            insecure_registries: vec!["localhost:5000".to_string()],
            registry_mirrors: vec!["https://mirror.example.com".to_string()],
            experimental: Some(true),
            live_restore: Some(true),
            ipv6: Some(true),
            ..Default::default()
        };

        assert!(config.has_insecure_registries());
        assert!(config.has_registry_mirrors());
        assert!(config.is_experimental());
        assert!(config.is_live_restore());
        assert!(config.is_ipv6_enabled());
    }

    #[test]
    fn test_address_pool() {
        let json = r#"{
            "default-address-pools": [
                {"base": "172.80.0.0/16", "size": 24},
                {"base": "172.90.0.0/16", "size": 24}
            ]
        }"#;

        let config: DaemonConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_address_pools.len(), 2);
        assert_eq!(config.default_address_pools[0].base, "172.80.0.0/16");
        assert_eq!(config.default_address_pools[0].size, 24);
    }
}
