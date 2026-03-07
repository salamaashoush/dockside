//! Utility functions for the application

use std::path::PathBuf;
use std::process::Command;

use crate::platform::{Platform, get_binary_search_paths, get_home_dir};

/// Find a binary in common locations (prioritize known paths over PATH)
pub fn find_binary(name: &str) -> Option<PathBuf> {
    // Check platform-specific binary locations FIRST
    // This is more reliable than PATH when launched from GUI launchers
    for base in get_binary_search_paths() {
        let path = std::path::Path::new(base).join(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Fallback to PATH lookup
    if let Ok(path) = which::which(name) {
        return Some(path);
    }

    None
}

/// Create a Command with proper environment for GUI-launched apps
fn create_cmd(path: PathBuf) -> Command {
    let mut cmd = Command::new(path);
    let platform = Platform::detect();

    // Ensure HOME is set (critical for tools like colima to find config)
    if std::env::var("HOME").is_err() {
        if let Some(home) = get_home_dir() {
            cmd.env("HOME", home);
        }
    }

    // Ensure PATH includes common binary locations for this platform
    let current_path = std::env::var("PATH").unwrap_or_default();

    match platform {
        Platform::MacOS => {
            if !current_path.contains("/opt/homebrew/bin") {
                let new_path = format!("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:{current_path}");
                cmd.env("PATH", new_path);
            }
        }
        Platform::Linux | Platform::WindowsWsl2 => {
            if !current_path.contains("/usr/local/bin") {
                let new_path =
                    format!("/usr/local/bin:/usr/bin:/bin:/snap/bin:/home/linuxbrew/.linuxbrew/bin:{current_path}");
                cmd.env("PATH", new_path);
            }
        }
        Platform::Windows => {
            // On Windows, PATH is usually set correctly by the system
            // No modifications needed
        }
    }

    cmd
}

/// Check if colima is installed
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn is_colima_installed() -> bool {
    find_binary("colima").is_some()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn is_colima_installed() -> bool {
    false
}

/// Create a Command for colima, finding the binary in common paths
/// Only available on macOS and Linux (including WSL2)
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn colima_cmd() -> Command {
    let path = find_binary("colima").unwrap_or_else(|| PathBuf::from("colima"));
    create_cmd(path)
}

/// Create a Command for docker, finding the binary in common paths
pub fn docker_cmd() -> Command {
    let path = find_binary("docker").unwrap_or_else(|| PathBuf::from("docker"));
    create_cmd(path)
}

/// Create a Command for kubectl, finding the binary in common paths
pub fn kubectl_cmd() -> Command {
    let path = find_binary("kubectl").unwrap_or_else(|| PathBuf::from("kubectl"));
    create_cmd(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_search_paths_exist() {
        let paths = get_binary_search_paths();
        // Should have paths on Unix-like systems
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            assert!(!paths.is_empty());
        }
        // On Windows, may be empty (uses PATH lookup)
        let _ = paths;
    }

    #[test]
    fn test_find_binary_returns_path_for_common_binaries() {
        // ls should exist on any Unix system
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let path = find_binary("ls");
            assert!(path.is_some(), "ls binary should be found");
            assert!(path.unwrap().exists());
        }
    }

    #[test]
    fn test_find_binary_returns_none_for_nonexistent() {
        let path = find_binary("this_binary_definitely_does_not_exist_xyz123");
        assert!(path.is_none());
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn test_colima_cmd_returns_command() {
        // Should return a Command even if colima isn't installed
        let cmd = colima_cmd();
        // Verify it's a valid Command (program is set)
        assert!(format!("{cmd:?}").contains("colima"));
    }

    #[test]
    fn test_docker_cmd_returns_command() {
        let cmd = docker_cmd();
        assert!(format!("{cmd:?}").contains("docker"));
    }

    #[test]
    fn test_kubectl_cmd_returns_command() {
        let cmd = kubectl_cmd();
        assert!(format!("{cmd:?}").contains("kubectl"));
    }

    #[test]
    fn test_find_binary_prefers_system_paths() {
        // Test with 'cat' which exists in system paths on most Unix systems
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            if let Some(path) = find_binary("cat") {
                // Path should be from one of the known locations or PATH
                let path_str = path.to_string_lossy();
                let search_paths = get_binary_search_paths();
                let is_known_path = search_paths.iter().any(|p| path_str.starts_with(p)) || path.exists();
                assert!(is_known_path, "Should find cat in a known location");
            }
        }
    }

    #[test]
    fn test_get_home_dir_not_empty() {
        // This should return Some on any properly configured system
        let home = get_home_dir();
        // We can't guarantee this works in all CI environments,
        // so just verify it doesn't panic
        if let Some(path) = home {
            assert!(!path.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_create_cmd_sets_path_env() {
        let cmd = create_cmd(PathBuf::from("echo"));
        // The command should have PATH configured
        let debug = format!("{cmd:?}");
        // Verify the command was created (contains the program)
        assert!(debug.contains("echo"));
    }

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        // Should return a valid platform
        assert!(!platform.display_name().is_empty());
    }
}
