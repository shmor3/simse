//! Re-exports the query DSL from the vector engine.
//!
//! The full DSL parser lives in `simse_vector_engine::query_dsl`. This module
//! re-exports all public items so consumers can access them through `simse_core::library`.

pub use simse_vector_engine::query_dsl::{parse_query, ParsedQuery, TextSearchParsed};
