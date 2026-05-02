//! Reverse proxy that listens on `127.0.0.1:80` (and `:443` once TLS lands)
//! and forwards `<name>.<suffix>` to the matching container's backend.

mod runtime;
mod server;

pub use runtime::{init, manager};
