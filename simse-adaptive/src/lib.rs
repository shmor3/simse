#![allow(clippy::needless_range_loop)]

pub mod error;
pub mod types;
pub mod transport;
pub mod protocol;
pub mod cosine;
pub mod text_search;
pub mod inverted_index;
pub mod cataloging;
pub mod deduplication;
pub mod recommendation;
pub mod learning;
pub mod topic_catalog;
pub mod query_dsl;
pub mod text_cache;
pub mod persistence;
pub mod context_format;
pub mod graph;
pub mod store;
pub mod server;

// PCN modules
pub mod pcn_config;
pub mod encoder;
pub mod vocabulary;
pub mod network;
pub mod layer;
pub mod predictor;
pub mod trainer;
pub mod snapshot;
