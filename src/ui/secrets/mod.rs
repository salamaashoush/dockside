//! K8s Secret view (list + detail with reveal/copy/YAML/Events)

mod detail;
mod list;
mod view;

pub use view::SecretsView;
