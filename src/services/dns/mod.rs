//! Local `*.dockside.test` DNS server + container route map. The reverse
//! proxy (sibling module) reads the same `RouteMap` to know where to
//! forward traffic.

mod route_builder;
mod route_map;
mod runtime;
mod server;
mod watcher;

#[cfg(test)]
pub(crate) use route_map::Route;
pub use route_map::{Backend, SharedRouteMap};
pub use runtime::{init, manager};
