//! simse-bridge — JSON-RPC bridge to the SimSE TypeScript core.
//!
//! Spawns a Bun/Node subprocess running the TS core as a JSON-RPC server.
//! Communication via stdin/stdout NDJSON.

pub mod client;
pub mod protocol;
pub mod config;
pub mod storage;
