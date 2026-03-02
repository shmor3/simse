//! Tests for the agent executor module.
//!
//! Follows TDD: tests written first, then implementation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use simse_core::agent::{
    AcpProvider, AgentExecutor, AgentResult, AgentStepConfig, LibraryProvider, McpProvider,
    ProviderRef, TokenUsage, ToolMetrics,
};
use simse_core::error::SimseError;

// ---------------------------------------------------------------------------
// Mock providers
// ---------------------------------------------------------------------------

struct MockAcp {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl AcpProvider for MockAcp {
    async fn generate(
        &self,
        _prompt: &str,
        _config: &AgentStepConfig,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(AgentResult {
            output: "acp response".to_string(),
            model: Some("acp:test-agent".to_string()),
            usage: Some(TokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            tool_metrics: None,
        })
    }
}

struct MockMcp {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl McpProvider for MockMcp {
    async fn call_tool(
        &self,
        _server: &str,
        _tool: &str,
        _input: &str,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(AgentResult {
            output: "mcp tool result".to_string(),
            model: Some("mcp:test-server/test-tool".to_string()),
            usage: None,
            tool_metrics: Some(ToolMetrics {
                calls_made: 1,
                calls_failed: 0,
            }),
        })
    }
}

struct MockLibrary {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl LibraryProvider for MockLibrary {
    async fn query(
        &self,
        _prompt: &str,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(AgentResult {
            output: "library search results".to_string(),
            model: Some("library:search".to_string()),
            usage: None,
            tool_metrics: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Error-returning mock providers
// ---------------------------------------------------------------------------

struct FailingAcp;

#[async_trait]
impl AcpProvider for FailingAcp {
    async fn generate(
        &self,
        _prompt: &str,
        _config: &AgentStepConfig,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        Err(SimseError::other("ACP generation failed"))
    }
}

struct FailingMcp;

#[async_trait]
impl McpProvider for FailingMcp {
    async fn call_tool(
        &self,
        _server: &str,
        _tool: &str,
        _input: &str,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        Err(SimseError::other("MCP tool call failed"))
    }
}

struct FailingLibrary;

#[async_trait]
impl LibraryProvider for FailingLibrary {
    async fn query(
        &self,
        _prompt: &str,
        _cancel: CancellationToken,
    ) -> Result<AgentResult, SimseError> {
        Err(SimseError::other("Library query failed"))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_step(name: &str) -> AgentStepConfig {
    AgentStepConfig {
        name: name.to_string(),
        agent_id: None,
        server_name: None,
        timeout_ms: None,
        max_tokens: None,
        temperature: None,
        system_prompt: None,
    }
}

// ---------------------------------------------------------------------------
// Dispatch routing tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dispatch_to_acp() {
    let acp_called = Arc::new(AtomicBool::new(false));
    let mcp_called = Arc::new(AtomicBool::new(false));
    let lib_called = Arc::new(AtomicBool::new(false));

    let executor = AgentExecutor::new(
        Box::new(MockAcp {
            called: acp_called.clone(),
        }),
        Box::new(MockMcp {
            called: mcp_called.clone(),
        }),
        Box::new(MockLibrary {
            called: lib_called.clone(),
        }),
    );

    let step = make_step("acp-step");
    let provider = ProviderRef::Acp {
        server_name: "test-server".to_string(),
    };
    let cancel = CancellationToken::new();

    let result = executor
        .execute(&step, &provider, "hello", cancel)
        .await
        .unwrap();

    assert!(acp_called.load(Ordering::SeqCst));
    assert!(!mcp_called.load(Ordering::SeqCst));
    assert!(!lib_called.load(Ordering::SeqCst));
    assert_eq!(result.output, "acp response");
    assert_eq!(result.model.as_deref(), Some("acp:test-agent"));
    assert!(result.usage.is_some());
    let usage = result.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[tokio::test]
async fn test_dispatch_to_mcp() {
    let acp_called = Arc::new(AtomicBool::new(false));
    let mcp_called = Arc::new(AtomicBool::new(false));
    let lib_called = Arc::new(AtomicBool::new(false));

    let executor = AgentExecutor::new(
        Box::new(MockAcp {
            called: acp_called.clone(),
        }),
        Box::new(MockMcp {
            called: mcp_called.clone(),
        }),
        Box::new(MockLibrary {
            called: lib_called.clone(),
        }),
    );

    let step = make_step("mcp-step");
    let provider = ProviderRef::Mcp {
        server_name: "my-server".to_string(),
        tool_name: "my-tool".to_string(),
    };
    let cancel = CancellationToken::new();

    let result = executor
        .execute(&step, &provider, "query text", cancel)
        .await
        .unwrap();

    assert!(!acp_called.load(Ordering::SeqCst));
    assert!(mcp_called.load(Ordering::SeqCst));
    assert!(!lib_called.load(Ordering::SeqCst));
    assert_eq!(result.output, "mcp tool result");
    assert!(result.tool_metrics.is_some());
    let metrics = result.tool_metrics.unwrap();
    assert_eq!(metrics.calls_made, 1);
    assert_eq!(metrics.calls_failed, 0);
}

#[tokio::test]
async fn test_dispatch_to_library() {
    let acp_called = Arc::new(AtomicBool::new(false));
    let mcp_called = Arc::new(AtomicBool::new(false));
    let lib_called = Arc::new(AtomicBool::new(false));

    let executor = AgentExecutor::new(
        Box::new(MockAcp {
            called: acp_called.clone(),
        }),
        Box::new(MockMcp {
            called: mcp_called.clone(),
        }),
        Box::new(MockLibrary {
            called: lib_called.clone(),
        }),
    );

    let step = make_step("library-step");
    let provider = ProviderRef::Library;
    let cancel = CancellationToken::new();

    let result = executor
        .execute(&step, &provider, "search query", cancel)
        .await
        .unwrap();

    assert!(!acp_called.load(Ordering::SeqCst));
    assert!(!mcp_called.load(Ordering::SeqCst));
    assert!(lib_called.load(Ordering::SeqCst));
    assert_eq!(result.output, "library search results");
    assert_eq!(result.model.as_deref(), Some("library:search"));
}

// ---------------------------------------------------------------------------
// Error propagation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_acp_error_propagates() {
    let executor = AgentExecutor::new(
        Box::new(FailingAcp),
        Box::new(MockMcp {
            called: Arc::new(AtomicBool::new(false)),
        }),
        Box::new(MockLibrary {
            called: Arc::new(AtomicBool::new(false)),
        }),
    );

    let step = make_step("fail-step");
    let provider = ProviderRef::Acp {
        server_name: "s".to_string(),
    };
    let cancel = CancellationToken::new();

    let err = executor
        .execute(&step, &provider, "prompt", cancel)
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "ACP generation failed");
}

#[tokio::test]
async fn test_mcp_error_propagates() {
    let executor = AgentExecutor::new(
        Box::new(MockAcp {
            called: Arc::new(AtomicBool::new(false)),
        }),
        Box::new(FailingMcp),
        Box::new(MockLibrary {
            called: Arc::new(AtomicBool::new(false)),
        }),
    );

    let step = make_step("fail-step");
    let provider = ProviderRef::Mcp {
        server_name: "s".to_string(),
        tool_name: "t".to_string(),
    };
    let cancel = CancellationToken::new();

    let err = executor
        .execute(&step, &provider, "prompt", cancel)
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "MCP tool call failed");
}

#[tokio::test]
async fn test_library_error_propagates() {
    let executor = AgentExecutor::new(
        Box::new(MockAcp {
            called: Arc::new(AtomicBool::new(false)),
        }),
        Box::new(MockMcp {
            called: Arc::new(AtomicBool::new(false)),
        }),
        Box::new(FailingLibrary),
    );

    let step = make_step("fail-step");
    let provider = ProviderRef::Library;
    let cancel = CancellationToken::new();

    let err = executor
        .execute(&step, &provider, "prompt", cancel)
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "Library query failed");
}

// ---------------------------------------------------------------------------
// Struct construction tests
// ---------------------------------------------------------------------------

#[test]
fn test_agent_result_defaults() {
    let result = AgentResult {
        output: "test".to_string(),
        model: None,
        usage: None,
        tool_metrics: None,
    };
    assert_eq!(result.output, "test");
    assert!(result.model.is_none());
    assert!(result.usage.is_none());
    assert!(result.tool_metrics.is_none());
}

#[test]
fn test_token_usage_fields() {
    let usage = TokenUsage {
        input_tokens: 100,
        output_tokens: 200,
        total_tokens: 300,
    };
    assert_eq!(usage.input_tokens, 100);
    assert_eq!(usage.output_tokens, 200);
    assert_eq!(usage.total_tokens, 300);
}

#[test]
fn test_tool_metrics_fields() {
    let metrics = ToolMetrics {
        calls_made: 5,
        calls_failed: 2,
    };
    assert_eq!(metrics.calls_made, 5);
    assert_eq!(metrics.calls_failed, 2);
}

#[test]
fn test_agent_step_config_defaults() {
    let step = AgentStepConfig {
        name: "my-step".to_string(),
        agent_id: None,
        server_name: None,
        timeout_ms: None,
        max_tokens: None,
        temperature: None,
        system_prompt: None,
    };
    assert_eq!(step.name, "my-step");
    assert!(step.agent_id.is_none());
}

#[test]
fn test_agent_step_config_with_all_fields() {
    let step = AgentStepConfig {
        name: "full-step".to_string(),
        agent_id: Some("agent-1".to_string()),
        server_name: Some("server-1".to_string()),
        timeout_ms: Some(5000),
        max_tokens: Some(1024),
        temperature: Some(0.7),
        system_prompt: Some("You are helpful.".to_string()),
    };
    assert_eq!(step.name, "full-step");
    assert_eq!(step.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(step.server_name.as_deref(), Some("server-1"));
    assert_eq!(step.timeout_ms, Some(5000));
    assert_eq!(step.max_tokens, Some(1024));
    assert_eq!(step.temperature, Some(0.7));
    assert_eq!(step.system_prompt.as_deref(), Some("You are helpful."));
}

#[test]
fn test_provider_ref_variants() {
    let acp = ProviderRef::Acp {
        server_name: "s".to_string(),
    };
    let mcp = ProviderRef::Mcp {
        server_name: "s".to_string(),
        tool_name: "t".to_string(),
    };
    let lib = ProviderRef::Library;

    // Verify Debug is implemented
    let _ = format!("{:?}", acp);
    let _ = format!("{:?}", mcp);
    let _ = format!("{:?}", lib);

    // Verify Clone is implemented
    let _acp2 = acp.clone();
    let _mcp2 = mcp.clone();
    let _lib2 = lib.clone();
}

// ---------------------------------------------------------------------------
// Cancellation test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cancellation_token_passed_through() {
    // We verify the token is passed by having a provider that checks it.
    struct CancelCheckAcp;

    #[async_trait]
    impl AcpProvider for CancelCheckAcp {
        async fn generate(
            &self,
            _prompt: &str,
            _config: &AgentStepConfig,
            cancel: CancellationToken,
        ) -> Result<AgentResult, SimseError> {
            // The token should not be cancelled yet
            assert!(!cancel.is_cancelled());
            Ok(AgentResult {
                output: "ok".to_string(),
                model: None,
                usage: None,
                tool_metrics: None,
            })
        }
    }

    let executor = AgentExecutor::new(
        Box::new(CancelCheckAcp),
        Box::new(MockMcp {
            called: Arc::new(AtomicBool::new(false)),
        }),
        Box::new(MockLibrary {
            called: Arc::new(AtomicBool::new(false)),
        }),
    );

    let step = make_step("cancel-test");
    let provider = ProviderRef::Acp {
        server_name: "s".to_string(),
    };
    let cancel = CancellationToken::new();

    let result = executor.execute(&step, &provider, "test", cancel).await;
    assert!(result.is_ok());
}
