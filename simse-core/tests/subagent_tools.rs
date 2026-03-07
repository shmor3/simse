//! Tests for subagent and delegation tool registration.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use simse_core::error::SimseError;
use simse_core::tools::delegation::{
	DelegationCallbacks, DelegationInfo, DelegationResult, DelegationToolsOptions,
	ServerDelegator, register_delegation_tools, reset_delegation_counter,
};
use simse_core::tools::subagent::{
	DelegateRunner, SubagentCallbacks, SubagentInfo, SubagentLoopRunner, SubagentMode,
	SubagentResult, SubagentToolsOptions, register_subagent_tools,
};
use simse_core::tools::{ToolCallRequest, ToolRegistry, ToolRegistryOptions};

// ---------------------------------------------------------------------------
// Mock SubagentLoopRunner
// ---------------------------------------------------------------------------

struct MockLoopRunner {
	result_text: String,
	result_turns: u32,
	should_fail: bool,
}

impl Default for MockLoopRunner {
	fn default() -> Self {
		Self {
			result_text: "Subagent completed the task successfully.".to_string(),
			result_turns: 3,
			should_fail: false,
		}
	}
}

#[async_trait]
impl SubagentLoopRunner for MockLoopRunner {
	async fn run_subagent(
		&self,
		_task: &str,
		_max_turns: u32,
		_system_prompt: Option<&str>,
		_depth: u32,
	) -> Result<SubagentResult, SimseError> {
		if self.should_fail {
			return Err(SimseError::other("Loop runner failed"));
		}
		Ok(SubagentResult {
			text: self.result_text.clone(),
			turns: self.result_turns,
			duration_ms: 150,
		})
	}
}

// ---------------------------------------------------------------------------
// Mock DelegateRunner
// ---------------------------------------------------------------------------

struct MockDelegateRunner {
	result_text: String,
	should_fail: bool,
}

impl Default for MockDelegateRunner {
	fn default() -> Self {
		Self {
			result_text: "Delegation response from ACP.".to_string(),
			should_fail: false,
		}
	}
}

#[async_trait]
impl DelegateRunner for MockDelegateRunner {
	async fn delegate(
		&self,
		_task: &str,
		_server_name: Option<&str>,
		_agent_id: Option<&str>,
	) -> Result<String, SimseError> {
		if self.should_fail {
			return Err(SimseError::other("Delegate runner failed"));
		}
		Ok(self.result_text.clone())
	}
}

// ---------------------------------------------------------------------------
// Mock ServerDelegator
// ---------------------------------------------------------------------------

struct MockServerDelegator {
	servers: Vec<String>,
	response: String,
	should_fail: bool,
}

impl Default for MockServerDelegator {
	fn default() -> Self {
		Self {
			servers: vec![
				"primary-server".to_string(),
				"claude-3".to_string(),
				"gpt-4".to_string(),
			],
			response: "Response from delegated server.".to_string(),
			should_fail: false,
		}
	}
}

#[async_trait]
impl ServerDelegator for MockServerDelegator {
	fn server_names(&self) -> Vec<String> {
		self.servers.clone()
	}

	async fn generate(
		&self,
		_task: &str,
		_server_name: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		if self.should_fail {
			return Err(SimseError::other("Server delegation failed"));
		}
		Ok(self.response.clone())
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_subagent_options(
	loop_runner: Arc<dyn SubagentLoopRunner>,
	delegate_runner: Arc<dyn DelegateRunner>,
) -> SubagentToolsOptions {
	SubagentToolsOptions {
		loop_runner,
		delegate_runner,
		callbacks: None,
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	}
}

fn make_call(name: &str, args: serde_json::Value) -> ToolCallRequest {
	ToolCallRequest {
		id: "test_call_1".to_string(),
		name: name.to_string(),
		arguments: args,
	}
}

// ===========================================================================
// Subagent tool tests
// ===========================================================================

#[tokio::test]
async fn subagent_spawn_registered_at_depth_0() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner::default()),
	);

	register_subagent_tools(&mut registry, &options, 0);

	assert!(registry.is_registered("subagent_spawn"));
	let def = registry.get_tool_definition("subagent_spawn").unwrap();
	assert_eq!(def.category, simse_core::tools::ToolCategory::Subagent);
	assert!(def.parameters.contains_key("task"));
	assert!(def.parameters.contains_key("description"));
	assert!(def.parameters.contains_key("maxTurns"));
	assert!(def.parameters.contains_key("systemPrompt"));
	assert!(def.parameters.get("task").unwrap().required);
	assert!(def.parameters.get("description").unwrap().required);
	assert!(!def.parameters.get("maxTurns").unwrap().required);
	assert!(!def.parameters.get("systemPrompt").unwrap().required);
}

#[tokio::test]
async fn subagent_delegate_registered_at_depth_0() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner::default()),
	);

	register_subagent_tools(&mut registry, &options, 0);

	assert!(registry.is_registered("subagent_delegate"));
	let def = registry.get_tool_definition("subagent_delegate").unwrap();
	assert_eq!(def.category, simse_core::tools::ToolCategory::Subagent);
	assert!(def.parameters.contains_key("task"));
	assert!(def.parameters.contains_key("description"));
	assert!(def.parameters.contains_key("serverName"));
	assert!(def.parameters.contains_key("agentId"));
}

#[tokio::test]
async fn subagent_tools_not_registered_at_max_depth() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner::default()),
	);

	register_subagent_tools(&mut registry, &options, 2);

	assert!(!registry.is_registered("subagent_spawn"));
	assert!(!registry.is_registered("subagent_delegate"));
	assert_eq!(registry.tool_count(), 0);
}

#[tokio::test]
async fn subagent_tools_not_registered_above_max_depth() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner::default()),
	);

	register_subagent_tools(&mut registry, &options, 5);

	assert!(!registry.is_registered("subagent_spawn"));
	assert!(!registry.is_registered("subagent_delegate"));
}

#[tokio::test]
async fn subagent_spawn_execution_returns_result_text() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner {
			result_text: "Task done: computed 42".to_string(),
			..MockLoopRunner::default()
		}),
		Arc::new(MockDelegateRunner::default()),
	);

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_spawn",
		serde_json::json!({
			"task": "Compute the meaning of life",
			"description": "Computing answer"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);
	assert_eq!(result.output, "Task done: computed 42");
}

#[tokio::test]
async fn subagent_delegate_execution_returns_result_text() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner {
			result_text: "Summary: Rust is a systems language.".to_string(),
			..MockDelegateRunner::default()
		}),
	);

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_delegate",
		serde_json::json!({
			"task": "Summarize Rust documentation",
			"description": "Summarizing docs"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);
	assert_eq!(result.output, "Summary: Rust is a systems language.");
}

#[tokio::test]
async fn subagent_spawn_callbacks_fired() {

	let started = Arc::new(Mutex::new(Vec::<SubagentInfo>::new()));
	let completed = Arc::new(Mutex::new(Vec::<(String, SubagentResult)>::new()));

	let started_clone = Arc::clone(&started);
	let completed_clone = Arc::clone(&completed);

	let callbacks = Arc::new(SubagentCallbacks {
		on_start: Some(Box::new(move |info: &SubagentInfo| {
			started_clone.lock().unwrap().push(info.clone());
		})),
		on_complete: Some(Box::new(move |id: &str, result: &SubagentResult| {
			completed_clone
				.lock()
				.unwrap()
				.push((id.to_string(), result.clone()));
		})),
		on_error: None,
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner::default()),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: Some(callbacks),
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_spawn",
		serde_json::json!({
			"task": "Do something",
			"description": "Test spawn"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);

	let started_list = started.lock().unwrap();
	assert_eq!(started_list.len(), 1);
	assert_eq!(started_list[0].description, "Test spawn");
	assert_eq!(started_list[0].mode, SubagentMode::Spawn);

	let completed_list = completed.lock().unwrap();
	assert_eq!(completed_list.len(), 1);
	assert_eq!(completed_list[0].1.text, "Subagent completed the task successfully.");
}

#[tokio::test]
async fn subagent_delegate_callbacks_fired() {

	let started = Arc::new(Mutex::new(Vec::<SubagentInfo>::new()));
	let completed = Arc::new(Mutex::new(Vec::<(String, SubagentResult)>::new()));

	let started_clone = Arc::clone(&started);
	let completed_clone = Arc::clone(&completed);

	let callbacks = Arc::new(SubagentCallbacks {
		on_start: Some(Box::new(move |info: &SubagentInfo| {
			started_clone.lock().unwrap().push(info.clone());
		})),
		on_complete: Some(Box::new(move |id: &str, result: &SubagentResult| {
			completed_clone
				.lock()
				.unwrap()
				.push((id.to_string(), result.clone()));
		})),
		on_error: None,
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner::default()),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: Some(callbacks),
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_delegate",
		serde_json::json!({
			"task": "Summarize something",
			"description": "Test delegation"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);

	let started_list = started.lock().unwrap();
	assert_eq!(started_list.len(), 1);
	assert_eq!(started_list[0].description, "Test delegation");
	assert_eq!(started_list[0].mode, SubagentMode::Delegate);

	let completed_list = completed.lock().unwrap();
	assert_eq!(completed_list.len(), 1);
	assert_eq!(completed_list[0].1.turns, 1);
}

#[tokio::test]
async fn subagent_spawn_error_propagation() {

	let errors = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
	let errors_clone = Arc::clone(&errors);

	let callbacks = Arc::new(SubagentCallbacks {
		on_start: None,
		on_complete: None,
		on_error: Some(Box::new(move |id: &str, err: &SimseError| {
			errors_clone
				.lock()
				.unwrap()
				.push((id.to_string(), err.to_string()));
		})),
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner {
			should_fail: true,
			..MockLoopRunner::default()
		}),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: Some(callbacks),
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_spawn",
		serde_json::json!({
			"task": "Failing task",
			"description": "Will fail"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(result.is_error);
	assert!(result.output.contains("Loop runner failed"));

	let error_list = errors.lock().unwrap();
	assert_eq!(error_list.len(), 1);
	assert!(error_list[0].1.contains("Loop runner failed"));
}

#[tokio::test]
async fn subagent_delegate_error_propagation() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = make_subagent_options(
		Arc::new(MockLoopRunner::default()),
		Arc::new(MockDelegateRunner {
			should_fail: true,
			..MockDelegateRunner::default()
		}),
	);

	register_subagent_tools(&mut registry, &options, 0);

	let call = make_call(
		"subagent_delegate",
		serde_json::json!({
			"task": "Failing delegation",
			"description": "Will fail"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(result.is_error);
	assert!(result.output.contains("Delegate runner failed"));
}

#[tokio::test]
async fn subagent_unique_id_generation() {

	let ids = Arc::new(Mutex::new(Vec::<String>::new()));
	let ids_clone = Arc::clone(&ids);

	let callbacks = Arc::new(SubagentCallbacks {
		on_start: Some(Box::new(move |info: &SubagentInfo| {
			ids_clone.lock().unwrap().push(info.id.clone());
		})),
		on_complete: None,
		on_error: None,
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner::default()),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: Some(callbacks),
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	register_subagent_tools(&mut registry, &options, 0);

	// Call spawn twice
	for _ in 0..2 {
		let call = make_call(
			"subagent_spawn",
			serde_json::json!({
				"task": "Task",
				"description": "Desc"
			}),
		);
		registry.execute(&call).await;
	}

	// Call delegate once
	let call = make_call(
		"subagent_delegate",
		serde_json::json!({
			"task": "Task",
			"description": "Desc"
		}),
	);
	registry.execute(&call).await;

	let id_list = ids.lock().unwrap();
	assert_eq!(id_list.len(), 3);
	// All IDs should be unique
	assert_ne!(id_list[0], id_list[1]);
	assert_ne!(id_list[1], id_list[2]);
	assert_ne!(id_list[0], id_list[2]);
	// IDs should have the sub_ prefix
	for id in id_list.iter() {
		assert!(id.starts_with("sub_"));
	}
}

#[tokio::test]
async fn subagent_depth_check_at_depth_1() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner::default()),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: None,
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	// depth=1 < max_depth=2, so tools should be registered
	register_subagent_tools(&mut registry, &options, 1);

	assert!(registry.is_registered("subagent_spawn"));
	assert!(registry.is_registered("subagent_delegate"));
}

#[tokio::test]
async fn subagent_depth_check_custom_max_depth() {

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(MockLoopRunner::default()),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: None,
		default_max_turns: 10,
		max_depth: 5,
		system_prompt: None,
	};

	// depth=4 < max_depth=5, should register
	register_subagent_tools(&mut registry, &options, 4);
	assert!(registry.is_registered("subagent_spawn"));
	assert!(registry.is_registered("subagent_delegate"));

	// depth=5 == max_depth=5, should NOT register
	let mut registry2 = ToolRegistry::new(ToolRegistryOptions::default());
	register_subagent_tools(&mut registry2, &options, 5);
	assert!(!registry2.is_registered("subagent_spawn"));
	assert!(!registry2.is_registered("subagent_delegate"));
}

#[tokio::test]
async fn subagent_spawn_uses_custom_max_turns() {

	let received_turns = Arc::new(AtomicU64::new(0));
	let received_clone = Arc::clone(&received_turns);

	struct TrackingRunner {
		received_turns: Arc<AtomicU64>,
	}

	#[async_trait]
	impl SubagentLoopRunner for TrackingRunner {
		async fn run_subagent(
			&self,
			_task: &str,
			max_turns: u32,
			_system_prompt: Option<&str>,
			_depth: u32,
		) -> Result<SubagentResult, SimseError> {
			self.received_turns.store(max_turns as u64, Ordering::Relaxed);
			Ok(SubagentResult {
				text: "done".to_string(),
				turns: max_turns,
				duration_ms: 10,
			})
		}
	}

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = SubagentToolsOptions {
		loop_runner: Arc::new(TrackingRunner {
			received_turns: received_clone,
		}),
		delegate_runner: Arc::new(MockDelegateRunner::default()),
		callbacks: None,
		default_max_turns: 10,
		max_depth: 2,
		system_prompt: None,
	};

	register_subagent_tools(&mut registry, &options, 0);

	// With custom maxTurns
	let call = make_call(
		"subagent_spawn",
		serde_json::json!({
			"task": "Some task",
			"description": "Test",
			"maxTurns": 25
		}),
	);

	registry.execute(&call).await;
	assert_eq!(received_turns.load(Ordering::Relaxed), 25);
}

// ===========================================================================
// Delegation tool tests
// ===========================================================================

#[tokio::test]
async fn delegation_tools_registered_for_non_primary_servers() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator::default()),
		primary_server: Some("primary-server".to_string()),
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	// Primary server should NOT have a tool
	assert!(!registry.is_registered("delegate_primary_server"));
	// Non-primary servers should have tools
	assert!(registry.is_registered("delegate_claude_3"));
	assert!(registry.is_registered("delegate_gpt_4"));
	assert_eq!(registry.tool_count(), 2);
}

#[tokio::test]
async fn delegation_tool_execution() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator {
			response: "GPT-4 says hello.".to_string(),
			..MockServerDelegator::default()
		}),
		primary_server: Some("primary-server".to_string()),
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	let call = make_call(
		"delegate_gpt_4",
		serde_json::json!({
			"task": "Say hello"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);
	assert_eq!(result.output, "GPT-4 says hello.");
}

#[tokio::test]
async fn delegation_skips_primary_server() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator {
			servers: vec!["only-server".to_string()],
			..MockServerDelegator::default()
		}),
		primary_server: Some("only-server".to_string()),
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	// No tools registered because the only server is the primary
	assert_eq!(registry.tool_count(), 0);
}

#[tokio::test]
async fn delegation_all_servers_when_no_primary() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator::default()),
		primary_server: None,
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	// All three servers should have delegation tools
	assert!(registry.is_registered("delegate_primary_server"));
	assert!(registry.is_registered("delegate_claude_3"));
	assert!(registry.is_registered("delegate_gpt_4"));
	assert_eq!(registry.tool_count(), 3);
}

#[tokio::test]
async fn delegation_callbacks_fired() {
	reset_delegation_counter();
	let started = Arc::new(Mutex::new(Vec::<DelegationInfo>::new()));
	let completed = Arc::new(Mutex::new(Vec::<(String, DelegationResult)>::new()));

	let started_clone = Arc::clone(&started);
	let completed_clone = Arc::clone(&completed);

	let callbacks = Arc::new(DelegationCallbacks {
		on_start: Some(Box::new(move |info: &DelegationInfo| {
			started_clone.lock().unwrap().push(info.clone());
		})),
		on_complete: Some(Box::new(
			move |id: &str, result: &DelegationResult| {
				completed_clone
					.lock()
					.unwrap()
					.push((id.to_string(), result.clone()));
			},
		)),
		on_error: None,
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator::default()),
		primary_server: Some("primary-server".to_string()),
		callbacks: Some(callbacks),
	};

	register_delegation_tools(&mut registry, &options);

	let call = make_call(
		"delegate_claude_3",
		serde_json::json!({
			"task": "Generate code"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(!result.is_error);

	let started_list = started.lock().unwrap();
	assert_eq!(started_list.len(), 1);
	assert_eq!(started_list[0].server_name, "claude-3");
	assert_eq!(started_list[0].task, "Generate code");

	let completed_list = completed.lock().unwrap();
	assert_eq!(completed_list.len(), 1);
	assert_eq!(completed_list[0].1.server_name, "claude-3");
}

#[tokio::test]
async fn delegation_error_propagation() {
	reset_delegation_counter();
	let errors = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
	let errors_clone = Arc::clone(&errors);

	let callbacks = Arc::new(DelegationCallbacks {
		on_start: None,
		on_complete: None,
		on_error: Some(Box::new(move |id: &str, err: &SimseError| {
			errors_clone
				.lock()
				.unwrap()
				.push((id.to_string(), err.to_string()));
		})),
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator {
			should_fail: true,
			..MockServerDelegator::default()
		}),
		primary_server: Some("primary-server".to_string()),
		callbacks: Some(callbacks),
	};

	register_delegation_tools(&mut registry, &options);

	let call = make_call(
		"delegate_claude_3",
		serde_json::json!({
			"task": "Will fail"
		}),
	);

	let result = registry.execute(&call).await;
	assert!(result.is_error);
	assert!(result.output.contains("Server delegation failed"));

	let error_list = errors.lock().unwrap();
	assert_eq!(error_list.len(), 1);
	assert!(error_list[0].1.contains("Server delegation failed"));
}

#[tokio::test]
async fn delegation_unique_id_generation() {
	// Don't reset the counter — other tests run in parallel and race on it.
	// We only need two consecutive IDs to differ, which atomic increment guarantees.
	let ids = Arc::new(Mutex::new(Vec::<String>::new()));
	let ids_clone = Arc::clone(&ids);

	let callbacks = Arc::new(DelegationCallbacks {
		on_start: Some(Box::new(move |info: &DelegationInfo| {
			ids_clone.lock().unwrap().push(info.id.clone());
		})),
		on_complete: None,
		on_error: None,
	});

	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator::default()),
		primary_server: Some("primary-server".to_string()),
		callbacks: Some(callbacks),
	};

	register_delegation_tools(&mut registry, &options);

	// Call two different delegation tools
	let call1 = make_call(
		"delegate_claude_3",
		serde_json::json!({"task": "Task 1"}),
	);
	let call2 = make_call(
		"delegate_gpt_4",
		serde_json::json!({"task": "Task 2"}),
	);

	registry.execute(&call1).await;
	registry.execute(&call2).await;

	let id_list = ids.lock().unwrap();
	assert_eq!(id_list.len(), 2);
	assert_ne!(id_list[0], id_list[1]);
	for id in id_list.iter() {
		assert!(id.starts_with("del_"));
	}
}

#[tokio::test]
async fn delegation_name_sanitization() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator {
			servers: vec![
				"my-server.v2".to_string(),
				"server with spaces".to_string(),
			],
			..MockServerDelegator::default()
		}),
		primary_server: None,
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	// Non-alphanumeric characters replaced with _
	assert!(registry.is_registered("delegate_my_server_v2"));
	assert!(registry.is_registered("delegate_server_with_spaces"));
}

#[tokio::test]
async fn delegation_tool_has_correct_category() {
	reset_delegation_counter();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let options = DelegationToolsOptions {
		delegator: Arc::new(MockServerDelegator::default()),
		primary_server: Some("primary-server".to_string()),
		callbacks: None,
	};

	register_delegation_tools(&mut registry, &options);

	let def = registry.get_tool_definition("delegate_claude_3").unwrap();
	assert_eq!(def.category, simse_core::tools::ToolCategory::Subagent);
	assert!(def.parameters.contains_key("task"));
	assert!(def.parameters.contains_key("systemPrompt"));
	assert!(def.parameters.get("task").unwrap().required);
	assert!(!def.parameters.get("systemPrompt").unwrap().required);
}
