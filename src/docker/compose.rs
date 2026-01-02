use std::collections::HashMap;

use super::{ContainerInfo, ContainerState};

/// Docker Compose label keys
pub const COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";
pub const COMPOSE_SERVICE_LABEL: &str = "com.docker.compose.service";
pub const COMPOSE_WORKING_DIR_LABEL: &str = "com.docker.compose.project.working_dir";

/// Represents a Docker Compose project with its services
#[derive(Debug, Clone)]
pub struct ComposeProject {
    pub name: String,
    pub working_dir: Option<String>,
    pub services: Vec<ComposeService>,
}

impl ComposeProject {
    /// Total number of containers in this project
    pub fn container_count(&self) -> usize {
        self.services.len()
    }

    /// Number of running containers in this project
    pub fn running_count(&self) -> usize {
        self.services.iter().filter(|s| s.state.is_running()).count()
    }

    /// Returns true if all containers are running
    pub fn is_all_running(&self) -> bool {
        !self.services.is_empty() && self.running_count() == self.container_count()
    }

    /// Returns true if no containers are running
    pub fn is_all_stopped(&self) -> bool {
        self.running_count() == 0
    }

    /// Status display string (e.g., "3/3 running" or "0/2 stopped")
    pub fn status_display(&self) -> String {
        let running = self.running_count();
        let total = self.container_count();
        if running == total && total > 0 {
            format!("{}/{} running", running, total)
        } else if running == 0 {
            format!("{}/{} stopped", running, total)
        } else {
            format!("{}/{} running", running, total)
        }
    }
}

/// Represents a service within a Docker Compose project
#[derive(Debug, Clone)]
pub struct ComposeService {
    pub name: String,
    pub container_id: String,
    pub container_name: String,
    pub image: String,
    pub state: ContainerState,
    pub status: String,
}

impl ComposeService {
    /// Create from ContainerInfo with compose labels
    pub fn from_container(container: &ContainerInfo, service_name: &str) -> Self {
        Self {
            name: service_name.to_string(),
            container_id: container.id.clone(),
            container_name: container.name.clone(),
            image: container.image.clone(),
            state: container.state,
            status: container.status.clone(),
        }
    }
}

/// Extract Docker Compose projects from a list of containers
/// Returns projects sorted by name
pub fn extract_compose_projects(containers: &[ContainerInfo]) -> Vec<ComposeProject> {
    let mut projects: HashMap<String, ComposeProject> = HashMap::new();

    for container in containers {
        // Check if this container is part of a compose project
        if let Some(project_name) = container.labels.get(COMPOSE_PROJECT_LABEL) {
            let service_name = container
                .labels
                .get(COMPOSE_SERVICE_LABEL)
                .cloned()
                .unwrap_or_else(|| container.name.clone());

            let working_dir = container.labels.get(COMPOSE_WORKING_DIR_LABEL).cloned();

            let service = ComposeService::from_container(container, &service_name);

            projects
                .entry(project_name.clone())
                .or_insert_with(|| ComposeProject {
                    name: project_name.clone(),
                    working_dir: working_dir.clone(),
                    services: Vec::new(),
                })
                .services
                .push(service);
        }
    }

    // Sort projects by name
    let mut result: Vec<ComposeProject> = projects.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));

    // Sort services within each project by name
    for project in &mut result {
        project.services.sort_by(|a, b| a.name.cmp(&b.name));
    }

    result
}

/// Check if a container is part of a Docker Compose project
pub fn is_compose_container(container: &ContainerInfo) -> bool {
    container.labels.contains_key(COMPOSE_PROJECT_LABEL)
}

/// Get the compose project name for a container, if any
pub fn get_compose_project(container: &ContainerInfo) -> Option<&str> {
    container.labels.get(COMPOSE_PROJECT_LABEL).map(|s| s.as_str())
}

/// Get the compose service name for a container, if any
pub fn get_compose_service(container: &ContainerInfo) -> Option<&str> {
    container.labels.get(COMPOSE_SERVICE_LABEL).map(|s| s.as_str())
}
