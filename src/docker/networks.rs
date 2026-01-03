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
