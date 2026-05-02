//! Local CA + on-demand leaf cert minting for the reverse proxy's HTTPS
//! listener.

mod ca;
mod resolver;

pub use ca::LocalCa;
pub use resolver::DocksideCertResolver;
