//! Platform-specific path utilities

use std::path::PathBuf;

use super::Platform;

/// Get the user's home directory reliably across platforms
#[must_use]
pub fn get_home_dir() -> Option<PathBuf> {
    // Try dirs crate first (handles most cases)
    if let Some(home) = dirs::home_dir() {
        return Some(home);
    }

    // Fallback: try HOME env var
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }

    // Platform-specific fallbacks
    #[cfg(target_os = "macos")]
    {
        if let Ok(user) = std::env::var("USER") {
            let home = PathBuf::from(format!("/Users/{user}"));
            if home.exists() {
                return Some(home);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(user) = std::env::var("USER") {
            let home = PathBuf::from(format!("/home/{user}"));
            if home.exists() {
                return Some(home);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(userprofile));
        }
    }

    None
}

/// Get the configuration directory for Dockside
/// - macOS: `~/Library/Application Support/dockside/`
/// - Linux: `~/.config/dockside/` (XDG_CONFIG_HOME)
/// - Windows: `%APPDATA%/dockside/`
#[must_use]
pub fn get_config_dir() -> PathBuf {
    let platform = Platform::detect();

    match platform {
        Platform::MacOS => {
            if let Some(home) = get_home_dir() {
                return home.join("Library").join("Application Support").join("dockside");
            }
        }
        Platform::Linux | Platform::WindowsWsl2 => {
            // Respect XDG_CONFIG_HOME if set
            if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                if !xdg.is_empty() {
                    return PathBuf::from(xdg).join("dockside");
                }
            }
            if let Some(home) = get_home_dir() {
                return home.join(".config").join("dockside");
            }
        }
        Platform::Windows => {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join("dockside");
            }
            if let Some(home) = get_home_dir() {
                return home.join("AppData").join("Roaming").join("dockside");
            }
        }
    }

    // Final fallback
    PathBuf::from(".").join(".dockside")
}

/// Get the binary search paths for the current platform
/// These are paths where common tools like docker, colima, kubectl are typically installed
#[must_use]
pub fn get_binary_search_paths() -> &'static [&'static str] {
    let platform = Platform::detect();

    match platform {
        Platform::MacOS => &[
            "/opt/homebrew/bin", // Apple Silicon Homebrew
            "/usr/local/bin",    // Intel Homebrew / manual installs
            "/usr/bin",          // System binaries
        ],
        Platform::Linux | Platform::WindowsWsl2 => &[
            "/usr/local/bin",            // Manual installs
            "/usr/bin",                  // System binaries
            "/snap/bin",                 // Snap packages
            "/home/linuxbrew/.linuxbrew/bin", // Linuxbrew
        ],
        Platform::Windows => &[
            // Windows uses PATH lookup primarily
            // These are common installation paths
        ],
    }
}

/// Get additional PATH entries to prepend for GUI-launched applications
/// This ensures tools are found even when launched from desktop environment
#[must_use]
pub fn get_path_additions() -> String {
    let platform = Platform::detect();

    match platform {
        Platform::MacOS => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_string(),
        Platform::Linux | Platform::WindowsWsl2 => {
            "/usr/local/bin:/usr/bin:/bin:/snap/bin:/home/linuxbrew/.linuxbrew/bin".to_string()
        }
        Platform::Windows => {
            // On Windows, PATH is usually set correctly
            String::new()
        }
    }
}

/// Get the default Docker socket path for the current platform
#[must_use]
pub fn get_default_docker_socket() -> Option<String> {
    let platform = Platform::detect();

    match platform {
        Platform::MacOS => {
            // Check common macOS Docker socket locations
            // Docker Desktop
            let docker_desktop = "/var/run/docker.sock";
            if std::path::Path::new(docker_desktop).exists() {
                return Some(docker_desktop.to_string());
            }

            // Colima default
            if let Some(home) = get_home_dir() {
                let colima_socket = format!("{}/.colima/default/docker.sock", home.display());
                if std::path::Path::new(&colima_socket).exists() {
                    return Some(colima_socket);
                }
            }

            None
        }
        Platform::Linux | Platform::WindowsWsl2 => {
            let socket = "/var/run/docker.sock";
            if std::path::Path::new(socket).exists() {
                return Some(socket.to_string());
            }

            // Check rootless Docker socket
            #[cfg(unix)]
            {
                if let Some(uid) = get_current_uid() {
                    let rootless = format!("/run/user/{uid}/docker.sock");
                    if std::path::Path::new(&rootless).exists() {
                        return Some(rootless);
                    }
                }
            }

            None
        }
        Platform::Windows => {
            // Windows uses named pipes or TCP, not Unix sockets
            None
        }
    }
}

/// Get current user ID on Unix systems
#[cfg(unix)]
fn get_current_uid() -> Option<u32> {
    // Try reading from /proc/self/status
    if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("Uid:") {
                // Format: "Uid:\t1000\t1000\t1000\t1000"
                if let Some(uid_str) = line.split_whitespace().nth(1) {
                    if let Ok(uid) = uid_str.parse::<u32>() {
                        return Some(uid);
                    }
                }
            }
        }
    }

    // Fallback: try to get from environment
    if let Ok(uid_str) = std::env::var("UID") {
        if let Ok(uid) = uid_str.parse::<u32>() {
            return Some(uid);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_home_dir() {
        let home = get_home_dir();
        // Should return Some on any properly configured system
        // We can't guarantee this works in all CI environments
        if let Some(path) = home {
            assert!(!path.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_get_config_dir_not_empty() {
        let config_dir = get_config_dir();
        assert!(!config_dir.as_os_str().is_empty());
        // Should end with "dockside"
        assert!(config_dir.ends_with("dockside") || config_dir.ends_with(".dockside"));
    }

    #[test]
    fn test_get_binary_search_paths_not_empty_on_unix() {
        let paths = get_binary_search_paths();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            assert!(!paths.is_empty());
            // Should contain /usr/bin or /usr/local/bin
            assert!(paths.iter().any(|p| p.contains("/usr")));
        }
        // On Windows, empty is acceptable
        let _ = paths;
    }

    #[test]
    fn test_get_path_additions() {
        let additions = get_path_additions();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            assert!(!additions.is_empty());
            assert!(additions.contains("/usr"));
        }
        // On Windows, empty is acceptable
        let _ = additions;
    }

    #[test]
    fn test_get_default_docker_socket() {
        // This may or may not return Some depending on whether Docker is installed
        let socket = get_default_docker_socket();
        if let Some(path) = socket {
            assert!(!path.is_empty());
        }
    }
}
