use std::future::Future;
use std::sync::OnceLock;

use gpui::{App, Global, ReadGlobal, Task};
use tokio::task::JoinError;

/// Shared tokio runtime handle for use outside of App context
static TOKIO_HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// Initialize the global Tokio runtime
pub fn init(cx: &mut App) {
    let global = GlobalTokio::new();
    // Store handle for access outside App context
    let _ = TOKIO_HANDLE.set(global.runtime.handle().clone());
    cx.set_global(global);
}

struct GlobalTokio {
    runtime: tokio::runtime::Runtime,
}

impl Global for GlobalTokio {}

impl GlobalTokio {
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            // Keep footprint small since we have two executors
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime");

        Self { runtime }
    }
}

pub struct Tokio;

impl Tokio {
    /// Spawns the given future on Tokio's thread pool, and returns it via a GPUI task.
    /// The Tokio task will be cancelled if the GPUI task is dropped.
    pub fn spawn<Fut, R>(cx: &mut App, f: Fut) -> Task<Result<R, JoinError>>
    where
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let join_handle = GlobalTokio::global(cx).runtime.spawn(f);
        let abort_handle = join_handle.abort_handle();

        cx.background_executor().spawn(async move {
            let result = join_handle.await;
            // Cancel the tokio task if GPUI task is dropped before completion
            drop(abort_handle);
            result
        })
    }

    /// Get a handle to the Tokio runtime for use in async contexts.
    /// This version uses App context.
    pub fn handle(cx: &App) -> tokio::runtime::Handle {
        GlobalTokio::global(cx).runtime.handle().clone()
    }

    /// Get the tokio runtime handle without App context.
    /// Useful for use in Entity contexts (Context<T>) where App is not directly accessible.
    pub fn runtime_handle() -> tokio::runtime::Handle {
        TOKIO_HANDLE
            .get()
            .expect("Tokio runtime not initialized. Call gpui_tokio::init() first.")
            .clone()
    }
}
