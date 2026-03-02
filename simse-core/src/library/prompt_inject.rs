//! Re-exports the prompt injection / memory context formatter from the vector engine.
//!
//! The full implementation lives in `simse_vector_engine::prompt_injection`. This module
//! re-exports all public items so consumers can access them through `simse_core::library`.

pub use simse_vector_engine::prompt_injection::{
	format_age, format_memory_context, PromptInjectionOptions,
};
