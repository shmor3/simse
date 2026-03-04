//! simse-bridge — JSON-RPC bridge to the SimSE TypeScript core.
//!
//! Spawns a Bun/Node subprocess running the TS core as a JSON-RPC server.
//! Communication via stdin/stdout NDJSON.

pub mod acp_client;
pub mod acp_types;
pub mod agentic_loop;
pub mod client;
pub mod config;
pub mod json_io;
pub mod protocol;
pub mod session_store;
pub mod storage;
pub mod tool_registry;
