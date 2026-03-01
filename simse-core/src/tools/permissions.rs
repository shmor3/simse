//! Tool permission resolver — gates tool execution via an async check.
//!
//! The `ToolPermissionResolver` trait allows consumers to plug in custom
//! authorization logic (e.g. user approval, role-based access control)
//! before a tool handler is invoked.

use async_trait::async_trait;

use crate::tools::types::{ToolCallRequest, ToolDefinition};

/// Async permission check that runs before each tool execution.
///
/// Return `true` to allow the call, `false` to deny.
#[async_trait]
pub trait ToolPermissionResolver: Send + Sync {
	async fn check(
		&self,
		request: &ToolCallRequest,
		definition: Option<&ToolDefinition>,
	) -> bool;
}
