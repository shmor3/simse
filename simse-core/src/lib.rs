// simse-core: Pure Rust orchestration library for simse
// Links simse-acp, simse-mcp, simse-vector, simse-vfs as library dependencies

pub mod agent;
pub mod agentic_loop;
pub mod chain;
pub mod config;
pub mod context;
pub mod conversation;
pub mod error;
pub mod events;
pub mod hooks;
pub mod library;
pub mod logger;
pub mod prompts;
pub mod server;
pub mod tasks;
pub mod tools;
pub mod utils;
pub mod vfs;

// Re-export key types at the crate root for convenience
pub use config::AppConfig;
pub use context::CoreContext;
pub use conversation::Conversation;
pub use error::SimseError;
pub use events::EventBus;
pub use logger::Logger;
pub use tasks::TaskList;
