// Allow indexed loops in math-heavy PCN code where multiple arrays
// share the same index — iterators would be less clear.
#![allow(clippy::needless_range_loop)]

pub mod config;
pub mod encoder;
pub mod error;
pub mod layer;
pub mod network;
pub mod persistence;
pub mod predictor;
pub mod protocol;
pub mod server;
pub mod snapshot;
pub mod trainer;
pub mod transport;
pub mod vocabulary;
