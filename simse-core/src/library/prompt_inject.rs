//! Re-exports the context formatter from the vector engine.
//!
//! The full implementation lives in `simse_adaptive_engine::context_format`. This module
//! re-exports all public items so consumers can access them through `simse_core::library`.

pub use simse_adaptive_engine::context_format::{
	format_age, format_context, ContextFormatOptions,
};
