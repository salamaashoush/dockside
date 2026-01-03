use anyhow::{Result, anyhow};
use bollard::Docker;
use std::path::Path;

pub struct DockerClient {
  inner: Option<Docker>,
  socket_path: String,
}

impl DockerClient {
  pub fn new(socket_path: String) -> Self {
    Self {
      inner: None,
      socket_path,
    }
  }

  pub fn from_colima(profile: Option<&str>) -> Self {
    let home = dirs::home_dir().unwrap_or_default();
    let profile_name = profile.unwrap_or("default");
    let socket_path = format!("{}/.colima/{}/docker.sock", home.display(), profile_name);
    Self::new(socket_path)
  }

  pub async fn connect(&mut self) -> Result<()> {
    if !Path::new(&self.socket_path).exists() {
      return Err(anyhow!(
        "Docker socket not found at {}. Is Colima running?",
        self.socket_path
      ));
    }

    let docker = Docker::connect_with_socket(&self.socket_path, 120, bollard::API_DEFAULT_VERSION)?;

    docker
      .ping()
      .await
      .map_err(|e| anyhow!("Failed to connect to Docker: {}. Is Colima running?", e))?;

    self.inner = Some(docker);
    Ok(())
  }

  pub fn client(&self) -> Result<&Docker> {
    self.inner.as_ref().ok_or_else(|| anyhow!("Not connected to Docker"))
  }
}

impl Default for DockerClient {
  fn default() -> Self {
    Self::from_colima(None)
  }
}
