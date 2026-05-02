use std::collections::HashMap;

use super::{ContainerInfo, ContainerState};

/// Docker Compose label keys
pub const COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";
pub const COMPOSE_SERVICE_LABEL: &str = "com.docker.compose.service";
pub const COMPOSE_WORKING_DIR_LABEL: &str = "com.docker.compose.project.working_dir";
pub const COMPOSE_CONFIG_FILES_LABEL: &str = "com.docker.compose.project.config_files";

/// Represents a Docker Compose project with its services
#[derive(Debug, Clone, Default)]
pub struct ComposeProject {
  pub name: String,
  pub services: Vec<ComposeService>,
  /// Project root pulled from the `working_dir` label (`docker compose`
  /// chdirs here before reading the config file).
  pub working_dir: Option<String>,
  /// One or more compose YAML paths from the `config_files` label
  /// (comma-separated upstream).
  pub config_files: Vec<String>,
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
      format!("{running}/{total} running")
    } else if running == 0 {
      format!("{running}/{total} stopped")
    } else {
      format!("{running}/{total} running")
    }
  }
}

/// Represents a service within a Docker Compose project
#[derive(Debug, Clone)]
pub struct ComposeService {
  pub name: String,
  pub container_id: String,
  pub image: String,
  pub state: ContainerState,
}

impl ComposeService {
  /// Create from `ContainerInfo` with compose labels
  pub fn from_container(container: &ContainerInfo, service_name: &str) -> Self {
    Self {
      name: service_name.to_string(),
      container_id: container.id.clone(),
      image: container.image.clone(),
      state: container.state,
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

      let service = ComposeService::from_container(container, &service_name);

      let working_dir = container.labels.get(COMPOSE_WORKING_DIR_LABEL).cloned();
      let config_files: Vec<String> = container
        .labels
        .get(COMPOSE_CONFIG_FILES_LABEL)
        .map(|raw| raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

      let entry = projects
        .entry(project_name.clone())
        .or_insert_with(|| ComposeProject {
          name: project_name.clone(),
          services: Vec::new(),
          working_dir: None,
          config_files: Vec::new(),
        });
      entry.services.push(service);
      if entry.working_dir.is_none() && working_dir.is_some() {
        entry.working_dir = working_dir;
      }
      if entry.config_files.is_empty() && !config_files.is_empty() {
        entry.config_files = config_files;
      }
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

#[cfg(test)]
mod tests {
  use super::*;

  fn make_container(name: &str, image: &str, state: ContainerState, labels: HashMap<String, String>) -> ContainerInfo {
    ContainerInfo {
      id: format!("{name}-id-123456789012"),
      name: name.to_string(),
      image: image.to_string(),
      image_id: "sha256:abc".to_string(),
      state,
      status: format!("{state}"),
      created: None,
      ports: vec![],
      labels,
      command: None,
      size_rw: None,
      size_root_fs: None,
      volumes_used: vec![],
      networks_used: vec![],
    }
  }

  fn make_compose_labels(project: &str, service: &str) -> HashMap<String, String> {
    HashMap::from([
      (COMPOSE_PROJECT_LABEL.to_string(), project.to_string()),
      (COMPOSE_SERVICE_LABEL.to_string(), service.to_string()),
    ])
  }

  // ComposeProject tests

  #[test]
  fn test_compose_project_container_count() {
    let project = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Running,
        },
      ],
      ..Default::default()

    };
    assert_eq!(project.container_count(), 2);
  }

  #[test]
  fn test_compose_project_running_count() {
    let project = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
        ComposeService {
          name: "cache".to_string(),
          container_id: "ghi".to_string(),
          image: "redis".to_string(),
          state: ContainerState::Running,
        },
      ],
      ..Default::default()

    };
    assert_eq!(project.running_count(), 2);
  }

  #[test]
  fn test_compose_project_is_all_running() {
    // All running
    let all_running = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Running,
        },
      ],
      ..Default::default()

    };
    assert!(all_running.is_all_running());

    // One not running
    let partial = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };
    assert!(!partial.is_all_running());

    // Empty project
    let empty = ComposeProject {
      name: "empty".to_string(),
      services: vec![],
      ..Default::default()
    };
    assert!(!empty.is_all_running());
  }

  #[test]
  fn test_compose_project_is_all_stopped() {
    // All stopped
    let all_stopped = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Exited,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };
    assert!(all_stopped.is_all_stopped());

    // One running
    let partial = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };
    assert!(!partial.is_all_stopped());

    // Empty project is considered all stopped
    let empty = ComposeProject {
      name: "empty".to_string(),
      services: vec![],
      ..Default::default()
    };
    assert!(empty.is_all_stopped());
  }

  #[test]
  fn test_compose_project_status_display() {
    // All running
    let all_running = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Running,
        },
      ],
      ..Default::default()

    };
    assert_eq!(all_running.status_display(), "2/2 running");

    // All stopped
    let all_stopped = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Exited,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };
    assert_eq!(all_stopped.status_display(), "0/2 stopped");

    // Partial
    let partial = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "web".to_string(),
          container_id: "abc".to_string(),
          image: "nginx".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "db".to_string(),
          container_id: "def".to_string(),
          image: "postgres".to_string(),
          state: ContainerState::Exited,
        },
        ComposeService {
          name: "cache".to_string(),
          container_id: "ghi".to_string(),
          image: "redis".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };
    assert_eq!(partial.status_display(), "1/3 running");

    // Empty project
    let empty = ComposeProject {
      name: "empty".to_string(),
      services: vec![],
      ..Default::default()
    };
    assert_eq!(empty.status_display(), "0/0 stopped");
  }

  // ComposeService tests

  #[test]
  fn test_compose_service_from_container() {
    let container = make_container("my-app-web-1", "nginx:latest", ContainerState::Running, HashMap::new());
    let service = ComposeService::from_container(&container, "web");

    assert_eq!(service.name, "web");
    assert_eq!(service.container_id, container.id);
    assert_eq!(service.image, "nginx:latest");
    assert!(service.state.is_running());
  }

  // extract_compose_projects tests

  #[test]
  fn test_extract_compose_projects_empty() {
    let containers: Vec<ContainerInfo> = vec![];
    let projects = extract_compose_projects(&containers);
    assert!(projects.is_empty());
  }

  #[test]
  fn test_extract_compose_projects_no_compose_labels() {
    let containers = vec![
      make_container("standalone", "nginx", ContainerState::Running, HashMap::new()),
      make_container("another", "redis", ContainerState::Exited, HashMap::new()),
    ];
    let projects = extract_compose_projects(&containers);
    assert!(projects.is_empty());
  }

  #[test]
  fn test_extract_compose_projects_single_project() {
    let containers = vec![
      make_container(
        "myapp-web-1",
        "nginx",
        ContainerState::Running,
        make_compose_labels("myapp", "web"),
      ),
      make_container(
        "myapp-db-1",
        "postgres",
        ContainerState::Running,
        make_compose_labels("myapp", "db"),
      ),
    ];

    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "myapp");
    assert_eq!(projects[0].services.len(), 2);
  }

  #[test]
  fn test_extract_compose_projects_multiple_projects() {
    let containers = vec![
      make_container(
        "app1-web-1",
        "nginx",
        ContainerState::Running,
        make_compose_labels("app1", "web"),
      ),
      make_container(
        "app2-api-1",
        "node",
        ContainerState::Running,
        make_compose_labels("app2", "api"),
      ),
      make_container(
        "app1-db-1",
        "postgres",
        ContainerState::Exited,
        make_compose_labels("app1", "db"),
      ),
    ];

    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 2);

    // Projects should be sorted by name
    assert_eq!(projects[0].name, "app1");
    assert_eq!(projects[1].name, "app2");

    // app1 should have 2 services
    assert_eq!(projects[0].services.len(), 2);
    // app2 should have 1 service
    assert_eq!(projects[1].services.len(), 1);
  }

  #[test]
  fn test_extract_compose_projects_sorted_by_name() {
    let containers = vec![
      make_container(
        "z-app-web",
        "nginx",
        ContainerState::Running,
        make_compose_labels("z-app", "web"),
      ),
      make_container(
        "a-app-web",
        "nginx",
        ContainerState::Running,
        make_compose_labels("a-app", "web"),
      ),
      make_container(
        "m-app-web",
        "nginx",
        ContainerState::Running,
        make_compose_labels("m-app", "web"),
      ),
    ];

    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].name, "a-app");
    assert_eq!(projects[1].name, "m-app");
    assert_eq!(projects[2].name, "z-app");
  }

  #[test]
  fn test_extract_compose_projects_services_sorted() {
    let containers = vec![
      make_container(
        "myapp-cache-1",
        "redis",
        ContainerState::Running,
        make_compose_labels("myapp", "cache"),
      ),
      make_container(
        "myapp-web-1",
        "nginx",
        ContainerState::Running,
        make_compose_labels("myapp", "web"),
      ),
      make_container(
        "myapp-api-1",
        "node",
        ContainerState::Running,
        make_compose_labels("myapp", "api"),
      ),
    ];

    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 1);

    // Services should be sorted by name
    let service_names: Vec<&str> = projects[0].services.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(service_names, vec!["api", "cache", "web"]);
  }

  #[test]
  fn test_extract_compose_projects_missing_service_label() {
    // When service label is missing, use container name
    let labels = HashMap::from([(COMPOSE_PROJECT_LABEL.to_string(), "myapp".to_string())]);
    let containers = vec![make_container(
      "myapp-special-1",
      "nginx",
      ContainerState::Running,
      labels,
    )];

    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].services[0].name, "myapp-special-1"); // Falls back to container name
  }

  #[test]
  fn test_extract_compose_projects_mixed_compose_and_standalone() {
    let containers = vec![
      make_container(
        "myapp-web-1",
        "nginx",
        ContainerState::Running,
        make_compose_labels("myapp", "web"),
      ),
      make_container("standalone-nginx", "nginx", ContainerState::Running, HashMap::new()), // No compose labels
      make_container(
        "myapp-db-1",
        "postgres",
        ContainerState::Running,
        make_compose_labels("myapp", "db"),
      ),
    ];

    let projects = extract_compose_projects(&containers);
    // Only one project should be extracted (standalone container ignored)
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].services.len(), 2);
  }

  #[test]
  fn test_compose_project_various_states() {
    // Test with all different container states
    let project = ComposeProject {
      name: "test".to_string(),
      services: vec![
        ComposeService {
          name: "running".to_string(),
          container_id: "a".to_string(),
          image: "img".to_string(),
          state: ContainerState::Running,
        },
        ComposeService {
          name: "paused".to_string(),
          container_id: "b".to_string(),
          image: "img".to_string(),
          state: ContainerState::Paused,
        },
        ComposeService {
          name: "restarting".to_string(),
          container_id: "c".to_string(),
          image: "img".to_string(),
          state: ContainerState::Restarting,
        },
        ComposeService {
          name: "exited".to_string(),
          container_id: "d".to_string(),
          image: "img".to_string(),
          state: ContainerState::Exited,
        },
      ],
      ..Default::default()

    };

    // Only Running counts as running
    assert_eq!(project.running_count(), 1);
    assert_eq!(project.container_count(), 4);
    assert!(!project.is_all_running());
    assert!(!project.is_all_stopped());
    assert_eq!(project.status_display(), "1/4 running");
  }

  #[test]
  fn test_extract_pulls_working_dir_and_config_files() {
    let labels = HashMap::from([
      (COMPOSE_PROJECT_LABEL.to_string(), "myapp".to_string()),
      (COMPOSE_SERVICE_LABEL.to_string(), "web".to_string()),
      (COMPOSE_WORKING_DIR_LABEL.to_string(), "/srv/myapp".to_string()),
      (
        COMPOSE_CONFIG_FILES_LABEL.to_string(),
        "/srv/myapp/docker-compose.yml,/srv/myapp/override.yml".to_string(),
      ),
    ]);
    let containers = vec![make_container("myapp-web-1", "nginx", ContainerState::Running, labels)];
    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].working_dir.as_deref(), Some("/srv/myapp"));
    assert_eq!(
      projects[0].config_files,
      vec![
        "/srv/myapp/docker-compose.yml".to_string(),
        "/srv/myapp/override.yml".to_string(),
      ]
    );
  }

  #[test]
  fn test_extract_first_container_wins_for_metadata() {
    // Two services in same project; metadata should be set from the
    // first labelled container we walk and stay stable.
    let labels_a = HashMap::from([
      (COMPOSE_PROJECT_LABEL.to_string(), "p".to_string()),
      (COMPOSE_SERVICE_LABEL.to_string(), "a".to_string()),
      (COMPOSE_WORKING_DIR_LABEL.to_string(), "/dir-a".to_string()),
    ]);
    let labels_b = HashMap::from([
      (COMPOSE_PROJECT_LABEL.to_string(), "p".to_string()),
      (COMPOSE_SERVICE_LABEL.to_string(), "b".to_string()),
      (COMPOSE_WORKING_DIR_LABEL.to_string(), "/dir-b".to_string()),
    ]);
    let containers = vec![
      make_container("p-a-1", "img", ContainerState::Running, labels_a),
      make_container("p-b-1", "img", ContainerState::Running, labels_b),
    ];
    let projects = extract_compose_projects(&containers);
    assert_eq!(projects.len(), 1);
    // Either dir is acceptable for HashMap-iteration-order reasons,
    // but it must be one of them and must not be empty.
    let wd = projects[0].working_dir.as_deref().unwrap();
    assert!(wd == "/dir-a" || wd == "/dir-b");
  }

  #[test]
  fn test_extract_skips_empty_config_files_token() {
    let labels = HashMap::from([
      (COMPOSE_PROJECT_LABEL.to_string(), "p".to_string()),
      (COMPOSE_SERVICE_LABEL.to_string(), "a".to_string()),
      (
        COMPOSE_CONFIG_FILES_LABEL.to_string(),
        ",  ,/srv/c.yml,".to_string(),
      ),
    ]);
    let containers = vec![make_container("p-a-1", "img", ContainerState::Running, labels)];
    let projects = extract_compose_projects(&containers);
    assert_eq!(projects[0].config_files, vec!["/srv/c.yml".to_string()]);
  }
}
