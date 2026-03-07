//! Re-exports the query DSL from the vector engine.
//!
//! The full DSL parser lives in `crate::adaptive::query_dsl`. This module
//! re-exports all public items so consumers can access them through `simse_core::library`.

pub use crate::adaptive::query_dsl::{parse_query, ParsedQuery, TextSearchParsed};
