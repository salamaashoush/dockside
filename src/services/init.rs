//! Initial data loading with platform-aware Docker runtime detection

use gpui::App;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::colima::ColimaClient;
use crate::docker::DockerClient;
use crate::platform::{DockerRuntime, Platform};
use crate::services::Tokio;
use crate::state::{StateChanged, docker_state, settings_state};

use super::core::docker_client;

pub fn load_initial_data(cx: &mut App) {
    let state = docker_state(cx);
    let client_handle = docker_client();
    let platform = Platform::detect();

    // Get saved settings for Docker socket and Colima profile
    let settings = settings_state(cx).read(cx).settings.clone();
    let custom_socket = settings.docker_socket.clone();
    let colima_profile = settings.default_colima_profile.clone();

    // First, get colima VMs (if supported) and determine the Docker runtime
    let colima_task = cx.background_executor().spawn(async move {
        // Get Colima VMs only on platforms that support it
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let vms = if platform.supports_colima() {
            ColimaClient::list().unwrap_or_default()
        } else {
            Vec::new()
        };

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let vms = Vec::new();

        // Determine the Docker runtime to use
        let runtime = if !custom_socket.is_empty() {
            // User specified a custom socket/connection string
            if custom_socket.starts_with("http://") || custom_socket.starts_with("tcp://") {
                DockerRuntime::Custom {
                    connection_string: custom_socket,
                }
            } else {
                DockerRuntime::Custom {
                    connection_string: custom_socket,
                }
            }
        } else {
            // Auto-detect runtime based on platform
            match platform {
                Platform::MacOS => {
                    // Prefer Colima with configured profile on macOS
                    DockerRuntime::Colima {
                        profile: colima_profile,
                    }
                }
                Platform::Linux | Platform::WindowsWsl2 => {
                    // Try native Docker first, then Colima
                    let native = DockerRuntime::native_default();
                    if native.is_available() {
                        native
                    } else {
                        // Fall back to Colima if native isn't available
                        DockerRuntime::Colima {
                            profile: colima_profile,
                        }
                    }
                }
                Platform::Windows => {
                    // On Windows, use WSL2 Docker with default settings
                    // or try to detect available runtimes
                    let available = DockerRuntime::detect_available();
                    available.into_iter().next().unwrap_or_else(|| DockerRuntime::Wsl2Docker {
                        distro: "Ubuntu".to_string(),
                        port: DockerRuntime::DEFAULT_DOCKER_TCP_PORT,
                    })
                }
            }
        };

        (vms, runtime)
    });

    // Then spawn tokio task for Docker operations
    let tokio_task = Tokio::spawn(cx, async move {
        // Wait for colima info and runtime detection
        let (vms, runtime) = colima_task.await;

        // Initialize the shared Docker client with the detected runtime
        let mut new_client = DockerClient::new(runtime);
        let docker_connected = new_client.connect().await.is_ok();

        // Store in the global if connected
        if docker_connected {
            let mut guard = client_handle.write().await;
            *guard = Some(new_client);
            drop(guard);

            // Now use the shared client for all queries
            let guard = client_handle.read().await;
            let docker = guard.as_ref().unwrap();

            let containers = docker.list_containers(true).await.unwrap_or_default();
            let images = docker.list_images(false).await.unwrap_or_default();
            let volumes = docker.list_volumes().await.unwrap_or_default();
            let networks = docker.list_networks().await.unwrap_or_default();

            (vms, containers, images, volumes, networks)
        } else {
            (vms, vec![], vec![], vec![], vec![])
        }
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        let (vms, containers, images, volumes, networks) = result.unwrap_or_default();

        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_machines(vms);
                state.set_containers(containers);
                state.set_images(images);
                state.set_volumes(volumes);
                state.set_networks(networks);
                state.is_loading = false;
                cx.emit(StateChanged::MachinesUpdated);
                cx.emit(StateChanged::ContainersUpdated);
                cx.emit(StateChanged::ImagesUpdated);
                cx.emit(StateChanged::VolumesUpdated);
                cx.emit(StateChanged::NetworksUpdated);
                cx.emit(StateChanged::Loading);
            });
        })
    })
    .detach();
}
