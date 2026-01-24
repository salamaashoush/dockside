//! Docker client with cross-platform runtime support

use anyhow::{Result, anyhow};
use bollard::Docker;
use std::path::Path;

use crate::platform::DockerRuntime;

/// Docker client with support for multiple runtime configurations
pub struct DockerClient {
    inner: Option<Docker>,
    runtime: DockerRuntime,
}

impl DockerClient {
    /// Create a new Docker client with a specific runtime configuration
    pub fn new(runtime: DockerRuntime) -> Self {
        Self { inner: None, runtime }
    }

    /// Create a new Docker client with a custom socket path
    /// Kept for backward compatibility
    pub fn from_socket_path(socket_path: String) -> Self {
        Self::new(DockerRuntime::Custom {
            connection_string: socket_path,
        })
    }

    /// Create a new Docker client from a Colima profile
    /// Kept for backward compatibility
    pub fn from_colima(profile: Option<&str>) -> Self {
        Self::new(DockerRuntime::Colima {
            profile: profile.unwrap_or("default").to_string(),
        })
    }

    /// Create a Docker client with native Docker socket
    pub fn native() -> Self {
        Self::new(DockerRuntime::native_default())
    }

    /// Create a Docker client for WSL2 Docker
    #[cfg(target_os = "windows")]
    pub fn wsl2(distro: String, port: u16) -> Self {
        Self::new(DockerRuntime::Wsl2Docker { distro, port })
    }

    /// Get the current runtime configuration
    pub const fn runtime(&self) -> &DockerRuntime {
        &self.runtime
    }

    /// Connect to the Docker daemon
    pub async fn connect(&mut self) -> Result<()> {
        let docker = match &self.runtime {
            DockerRuntime::Wsl2Docker { distro: _, port } => {
                // Connect via HTTP/TCP to Docker in WSL2
                let addr = format!("http://localhost:{port}");
                Docker::connect_with_http(&addr, 120, bollard::API_DEFAULT_VERSION)
                    .map_err(|e| anyhow!("Failed to connect to Docker at {addr}: {e}"))?
            }
            DockerRuntime::Custom { connection_string } => {
                if connection_string.starts_with("http://") || connection_string.starts_with("tcp://") {
                    // HTTP/TCP connection
                    let addr = connection_string.replace("tcp://", "http://");
                    Docker::connect_with_http(&addr, 120, bollard::API_DEFAULT_VERSION)
                        .map_err(|e| anyhow!("Failed to connect to Docker at {connection_string}: {e}"))?
                } else {
                    // Unix socket connection
                    if !Path::new(connection_string).exists() {
                        return Err(anyhow!(
                            "Docker socket not found at {connection_string}. Is Docker running?"
                        ));
                    }
                    Docker::connect_with_socket(connection_string, 120, bollard::API_DEFAULT_VERSION)
                        .map_err(|e| anyhow!("Failed to connect to Docker at {connection_string}: {e}"))?
                }
            }
            // All other runtime types use Unix sockets
            runtime => {
                let socket_path = runtime.connection_string();
                if !Path::new(&socket_path).exists() {
                    return Err(anyhow!(
                        "Docker socket not found at {socket_path}. {}",
                        self.not_running_hint()
                    ));
                }
                Docker::connect_with_socket(&socket_path, 120, bollard::API_DEFAULT_VERSION)
                    .map_err(|e| anyhow!("Failed to connect to Docker at {socket_path}: {e}"))?
            }
        };

        // Verify connection with ping
        docker
            .ping()
            .await
            .map_err(|e| anyhow!("Failed to connect to Docker: {e}. {}", self.not_running_hint()))?;

        self.inner = Some(docker);
        Ok(())
    }

    /// Get a hint message for when Docker is not running
    fn not_running_hint(&self) -> &'static str {
        match &self.runtime {
            DockerRuntime::Colima { .. } => "Is Colima running?",
            DockerRuntime::Wsl2Docker { .. } => "Is Docker running in WSL2?",
            DockerRuntime::NativeDocker { .. } | DockerRuntime::RootlessDocker { .. } => "Is Docker daemon running?",
            DockerRuntime::Custom { .. } => "Is Docker running?",
        }
    }

    /// Get the inner Docker client
    pub fn client(&self) -> Result<&Docker> {
        self.inner.as_ref().ok_or_else(|| anyhow!("Not connected to Docker"))
    }

    /// Check if the client is connected
    pub const fn is_connected(&self) -> bool {
        self.inner.is_some()
    }

    /// Get a display name for the current connection
    pub fn display_name(&self) -> String {
        self.runtime.display_name()
    }

    /// Get the connection string (socket path or HTTP URL)
    pub fn connection_string(&self) -> String {
        self.runtime.connection_string()
    }
}

impl Default for DockerClient {
    fn default() -> Self {
        Self::new(DockerRuntime::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_client_from_colima() {
        let client = DockerClient::from_colima(None);
        assert!(matches!(
            client.runtime(),
            DockerRuntime::Colima { profile } if profile == "default"
        ));
    }

    #[test]
    fn test_docker_client_from_colima_profile() {
        let client = DockerClient::from_colima(Some("dev"));
        assert!(matches!(
            client.runtime(),
            DockerRuntime::Colima { profile } if profile == "dev"
        ));
    }

    #[test]
    fn test_docker_client_from_socket_path() {
        let client = DockerClient::from_socket_path("/custom/docker.sock".to_string());
        assert!(matches!(
            client.runtime(),
            DockerRuntime::Custom { connection_string } if connection_string == "/custom/docker.sock"
        ));
    }

    #[test]
    fn test_docker_client_native() {
        let client = DockerClient::native();
        assert!(matches!(client.runtime(), DockerRuntime::NativeDocker { .. }));
    }

    #[test]
    fn test_docker_client_not_connected_by_default() {
        let client = DockerClient::default();
        assert!(!client.is_connected());
        assert!(client.client().is_err());
    }

    #[test]
    fn test_docker_client_display_name() {
        let client = DockerClient::from_colima(None);
        assert_eq!(client.display_name(), "Colima");

        let client = DockerClient::from_colima(Some("dev"));
        assert_eq!(client.display_name(), "Colima (dev)");

        let client = DockerClient::native();
        assert_eq!(client.display_name(), "Docker");
    }
}
