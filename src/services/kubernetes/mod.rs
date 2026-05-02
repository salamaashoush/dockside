//! Kubernetes resource operations (pods, services, deployments, secrets, configmaps)

pub mod configmaps;
pub mod deployments;
pub mod pods;
pub mod secrets;
pub mod services;

pub use configmaps::*;
pub use deployments::*;
pub use pods::*;
pub use secrets::*;
pub use services::*;
