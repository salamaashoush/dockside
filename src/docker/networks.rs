use anyhow::Result;
use bollard::query_parameters::ListNetworksOptions;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DockerClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
  pub id: String,
  pub name: String,
  pub driver: String,
  pub scope: String,
  pub internal: bool,
  pub enable_ipv6: bool,
  pub created: Option<DateTime<Utc>>,
  pub labels: HashMap<String, String>,
  pub options: HashMap<String, String>,
  pub ipam: Option<IpamInfo>,
  pub containers: HashMap<String, NetworkContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpamInfo {
  pub driver: Option<String>,
  pub config: Vec<IpamConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpamConfig {
  pub subnet: Option<String>,
  pub gateway: Option<String>,
  pub ip_range: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkContainer {
  pub name: Option<String>,
  pub endpoint_id: Option<String>,
  pub mac_address: Option<String>,
  pub ipv4_address: Option<String>,
  pub ipv6_address: Option<String>,
}

impl NetworkInfo {
  pub fn short_id(&self) -> &str {
    if self.id.len() >= 12 { &self.id[..12] } else { &self.id }
  }

  pub fn container_count(&self) -> usize {
    self.containers.len()
  }

  pub fn is_system_network(&self) -> bool {
    matches!(self.name.as_str(), "bridge" | "host" | "none")
  }
}

impl DockerClient {
  pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
    let docker = self.client()?;

    let options = ListNetworksOptions { ..Default::default() };

    let networks = docker.list_networks(Some(options)).await?;

    let mut result = Vec::new();
    for network in networks {
      let created = network
        .created
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

      let ipam = network.ipam.map(|ipam| IpamInfo {
        driver: ipam.driver,
        config: ipam
          .config
          .unwrap_or_default()
          .into_iter()
          .map(|c| IpamConfig {
            subnet: c.subnet,
            gateway: c.gateway,
            ip_range: c.ip_range,
          })
          .collect(),
      });

      let containers = network
        .containers
        .unwrap_or_default()
        .into_iter()
        .map(|(id, container)| {
          (
            id,
            NetworkContainer {
              name: container.name,
              endpoint_id: container.endpoint_id,
              mac_address: container.mac_address,
              ipv4_address: container.ipv4_address,
              ipv6_address: container.ipv6_address,
            },
          )
        })
        .collect();

      result.push(NetworkInfo {
        id: network.id.unwrap_or_default(),
        name: network.name.unwrap_or_default(),
        driver: network.driver.unwrap_or_default(),
        scope: network.scope.unwrap_or_default(),
        internal: network.internal.unwrap_or(false),
        enable_ipv6: network.enable_ipv6.unwrap_or(false),
        created,
        labels: network.labels.unwrap_or_default(),
        options: network.options.unwrap_or_default(),
        ipam,
        containers,
      });
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
  }

  pub async fn remove_network(&self, id: &str) -> Result<()> {
    let docker = self.client()?;
    docker.remove_network(id).await?;
    Ok(())
  }

  pub async fn connect_container_to_network(&self, network_id: &str, container_id: &str) -> Result<()> {
    let docker = self.client()?;
    let req = bollard::models::NetworkConnectRequest {
      container: Some(container_id.to_string()),
      endpoint_config: None,
    };
    docker.connect_network(network_id, req).await?;
    Ok(())
  }

  pub async fn disconnect_container_from_network(
    &self,
    network_id: &str,
    container_id: &str,
    force: bool,
  ) -> Result<()> {
    let docker = self.client()?;
    let req = bollard::models::NetworkDisconnectRequest {
      container: Some(container_id.to_string()),
      force: Some(force),
    };
    docker.disconnect_network(network_id, req).await?;
    Ok(())
  }

  pub async fn create_network(&self, name: &str, enable_ipv6: bool, subnet: Option<&str>) -> Result<NetworkInfo> {
    let docker = self.client()?;

    let ipam = subnet.map(|subnet| bollard::models::Ipam {
      driver: Some("default".to_string()),
      config: Some(vec![bollard::models::IpamConfig {
        subnet: Some(subnet.to_string()),
        ..Default::default()
      }]),
      options: None,
    });

    let config = bollard::models::NetworkCreateRequest {
      name: name.to_string(),
      driver: Some("bridge".to_string()),
      enable_ipv6: Some(enable_ipv6),
      ipam,
      ..Default::default()
    };

    let response = docker.create_network(config).await?;
    let id = response.id;

    Ok(NetworkInfo {
      id,
      name: name.to_string(),
      driver: "bridge".to_string(),
      scope: "local".to_string(),
      internal: false,
      enable_ipv6,
      created: Some(Utc::now()),
      labels: HashMap::new(),
      options: HashMap::new(),
      ipam: subnet.map(|s| IpamInfo {
        driver: Some("default".to_string()),
        config: vec![IpamConfig {
          subnet: Some(s.to_string()),
          gateway: None,
          ip_range: None,
        }],
      }),
      containers: HashMap::new(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_network_info_short_id() {
    let network = NetworkInfo {
      id: "abc123def456789012345".to_string(),
      name: "my-network".to_string(),
      driver: "bridge".to_string(),
      scope: "local".to_string(),
      internal: false,
      enable_ipv6: false,
      created: None,
      labels: HashMap::new(),
      options: HashMap::new(),
      ipam: None,
      containers: HashMap::new(),
    };
    assert_eq!(network.short_id(), "abc123def456");

    let short = NetworkInfo {
      id: "abc".to_string(),
      ..network.clone()
    };
    assert_eq!(short.short_id(), "abc");
  }

  #[test]
  fn test_network_info_container_count() {
    let empty = NetworkInfo {
      id: "net1".to_string(),
      name: "empty-network".to_string(),
      driver: "bridge".to_string(),
      scope: "local".to_string(),
      internal: false,
      enable_ipv6: false,
      created: None,
      labels: HashMap::new(),
      options: HashMap::new(),
      ipam: None,
      containers: HashMap::new(),
    };
    assert_eq!(empty.container_count(), 0);

    let with_containers = NetworkInfo {
      containers: HashMap::from([
        (
          "c1".to_string(),
          NetworkContainer {
            name: Some("container1".to_string()),
            endpoint_id: None,
            mac_address: None,
            ipv4_address: Some("172.17.0.2".to_string()),
            ipv6_address: None,
          },
        ),
        (
          "c2".to_string(),
          NetworkContainer {
            name: Some("container2".to_string()),
            endpoint_id: None,
            mac_address: None,
            ipv4_address: Some("172.17.0.3".to_string()),
            ipv6_address: None,
          },
        ),
      ]),
      ..empty.clone()
    };
    assert_eq!(with_containers.container_count(), 2);
  }

  #[test]
  fn test_network_info_is_system_network() {
    let base = NetworkInfo {
      id: "net1".to_string(),
      name: "custom".to_string(),
      driver: "bridge".to_string(),
      scope: "local".to_string(),
      internal: false,
      enable_ipv6: false,
      created: None,
      labels: HashMap::new(),
      options: HashMap::new(),
      ipam: None,
      containers: HashMap::new(),
    };

    // Custom network
    assert!(!base.is_system_network());

    // Bridge network
    let bridge = NetworkInfo {
      name: "bridge".to_string(),
      ..base.clone()
    };
    assert!(bridge.is_system_network());

    // Host network
    let host = NetworkInfo {
      name: "host".to_string(),
      ..base.clone()
    };
    assert!(host.is_system_network());

    // None network
    let none = NetworkInfo {
      name: "none".to_string(),
      ..base.clone()
    };
    assert!(none.is_system_network());
  }

  #[test]
  fn test_ipam_info() {
    let ipam = IpamInfo {
      driver: Some("default".to_string()),
      config: vec![IpamConfig {
        subnet: Some("172.18.0.0/16".to_string()),
        gateway: Some("172.18.0.1".to_string()),
        ip_range: None,
      }],
    };
    assert_eq!(ipam.driver, Some("default".to_string()));
    assert_eq!(ipam.config.len(), 1);
    assert_eq!(ipam.config[0].subnet, Some("172.18.0.0/16".to_string()));
    assert_eq!(ipam.config[0].gateway, Some("172.18.0.1".to_string()));
  }

  #[test]
  fn test_network_container() {
    let container = NetworkContainer {
      name: Some("nginx".to_string()),
      endpoint_id: Some("endpoint123".to_string()),
      mac_address: Some("02:42:ac:11:00:02".to_string()),
      ipv4_address: Some("172.17.0.2/16".to_string()),
      ipv6_address: Some("fd00::2/64".to_string()),
    };
    assert_eq!(container.name, Some("nginx".to_string()));
    assert_eq!(container.ipv4_address, Some("172.17.0.2/16".to_string()));
    assert_eq!(container.ipv6_address, Some("fd00::2/64".to_string()));
  }
}
