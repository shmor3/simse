pub mod agent;
pub mod agentic_loop;
pub mod chain;
pub mod config;
pub mod context;
pub mod conversation;
pub mod error;
pub mod events;
pub mod hooks;
pub mod logger;
pub mod prompts;
pub mod server;
pub mod tasks;
pub mod tools;
pub mod utils;

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "adaptive")]
pub mod adaptive;

#[cfg(feature = "sandbox")]
pub mod sandbox;

#[cfg(feature = "remote")]
pub mod remote;

#[cfg(feature = "adaptive")]
pub mod library;

// Re-export key types at the crate root for convenience
pub use config::AppConfig;
pub use context::CoreContext;
pub use conversation::Conversation;
pub use error::SimseError;
pub use events::EventBus;
pub use logger::Logger;
pub use tasks::TaskList;
