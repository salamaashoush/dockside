//! Kubernetes resource operations (pods, services, deployments, secrets, configmaps)

pub mod configmaps;
pub mod daemonsets;
pub mod deployments;
pub mod pods;
pub mod secrets;
pub mod services;
pub mod statefulsets;

pub use configmaps::*;
pub use daemonsets::*;
pub use deployments::*;
pub use pods::*;
pub use secrets::*;
pub use services::*;
pub use statefulsets::*;
