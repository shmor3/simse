//! Agent executor — dispatches execution to ACP, MCP, or Library providers.
//!
//! This module ports `src/ai/agent/agent-executor.ts` and `types.ts`.
//! The executor is fundamentally a dispatcher: based on the [`ProviderRef`],
//! it routes to the appropriate backend via trait objects.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::error::SimseError;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Token usage statistics from a generation call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Metrics about tool calls made during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolMetrics {
    pub calls_made: u32,
    pub calls_failed: u32,
}

/// The raw result produced by executing a single agent call.
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub output: String,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    pub tool_metrics: Option<ToolMetrics>,
}

/// Configuration for a single agent execution step.
///
/// Contains only execution-relevant fields (no orchestration concerns).
#[derive(Debug, Clone)]
pub struct AgentStepConfig {
    pub name: String,
    pub agent_id: Option<String>,
    pub server_name: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub system_prompt: Option<String>,
}

/// Identifies which provider backend should handle a step.
#[derive(Debug, Clone)]
pub enum ProviderRef {
    /// Route to an ACP agent server.
    Acp { server_name: String },
    /// Route to an MCP tool on a specific server.
    Mcp { server_name: String, tool_name: String },
    /// Route to the library (vector store) for search.
    Library,
}

// ---------------------------------------------------------------------------
// Provider traits
// ---------------------------------------------------------------------------

/// Trait for ACP (Agent Client Protocol) backends.
#[async_trait]
pub trait AcpProvider: Send + Sync {
    async fn generate(
        &self,
        prompt: &str,
        config: &AgentStepConfig,
        cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError>;
}

/// Trait for MCP (Model Context Protocol) tool-call backends.
#[async_trait]
pub trait McpProvider: Send + Sync {
    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        input: &str,
        cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError>;
}

/// Trait for library (vector store) query backends.
#[async_trait]
pub trait LibraryProvider: Send + Sync {
    async fn query(
        &self,
        prompt: &str,
        cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError>;
}

// ---------------------------------------------------------------------------
// AgentExecutor
// ---------------------------------------------------------------------------

/// Dispatches agent step execution to the appropriate provider backend.
///
/// The executor holds boxed trait objects for each provider kind and routes
/// calls based on the [`ProviderRef`] variant.
pub struct AgentExecutor {
    acp: Box<dyn AcpProvider>,
    mcp: Box<dyn McpProvider>,
    library: Box<dyn LibraryProvider>,
}

impl AgentExecutor {
    /// Creates a new executor with the given provider implementations.
    pub fn new(
        acp: Box<dyn AcpProvider>,
        mcp: Box<dyn McpProvider>,
        library: Box<dyn LibraryProvider>,
    ) -> Self {
        Self { acp, mcp, library }
    }

    /// Executes a step by dispatching to the provider identified by `provider`.
    pub async fn execute(
        &self,
        step: &AgentStepConfig,
        provider: &ProviderRef,
        prompt: &str,
        cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        tracing::debug!(step = %step.name, provider = ?provider, "executing agent step");

        match provider {
            ProviderRef::Acp { .. } => {
                self.acp.generate(prompt, step, cancel).await
            }
            ProviderRef::Mcp {
                server_name,
                tool_name,
            } => {
                self.mcp
                    .call_tool(server_name, tool_name, prompt, cancel)
                    .await
            }
            ProviderRef::Library => {
                self.library.query(prompt, cancel).await
            }
        }
    }
}
