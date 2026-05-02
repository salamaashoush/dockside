//! Kubernetes resource operations (pods, services, deployments, secrets, configmaps)

pub mod cluster;
pub mod configmaps;
pub mod cronjobs;
pub mod daemonsets;
pub mod deployments;
pub mod ingresses;
pub mod jobs;
pub mod pods;
pub mod pvcs;
pub mod secrets;
pub mod services;
pub mod statefulsets;

pub use cluster::*;
pub use configmaps::*;
pub use cronjobs::*;
pub use daemonsets::*;
pub use deployments::*;
pub use ingresses::*;
pub use jobs::*;
pub use pods::*;
pub use pvcs::*;
pub use secrets::*;
pub use services::*;
pub use statefulsets::*;
