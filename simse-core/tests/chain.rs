//! Tests for the chain execution module.
//!
//! Covers: Chain with single/multiple steps, parallel steps (concat/keyed merge),
//! ChainBuilder fluent API, error callbacks, create_chain_from_definition,
//! run_named_chain, and cancellation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use simse_core::agent::{
	AcpProvider, AgentExecutor, AgentResult, AgentStepConfig, LibraryProvider, McpProvider,
};
use simse_core::chain::chain::{
	create_chain_from_definition, run_named_chain, Chain, ChainBuilder,
};
use simse_core::chain::template::PromptTemplate;
use simse_core::chain::types::{
	ChainCallbacks, ChainStepConfig, MergeStrategy, ParallelConfig, ParallelSubStepConfig,
	Provider,
};
use simse_core::config::{AppConfig, ChainDefinition, ChainStepDefinition};
use simse_core::error::SimseError;

// ---------------------------------------------------------------------------
// Mock providers
// ---------------------------------------------------------------------------

/// An ACP provider that returns a canned response including the prompt received.
struct EchoAcp;

#[async_trait]
impl AcpProvider for EchoAcp {
	async fn generate(
		&self,
		prompt: &str,
		config: &AgentStepConfig,
		_cancel: CancellationToken,
	) -> Result<AgentResult, SimseError> {
		Ok(AgentResult {
			output: format!("[acp:{}] {}", config.name, prompt),
			model: Some("test-model".to_string()),
			usage: None,
			tool_metrics: None,
		})
	}
}

/// An ACP provider that fails on every call.
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

/// An ACP provider that counts how many times it was called.
struct CountingAcp {
	count: Arc<AtomicUsize>,
}

#[async_trait]
impl AcpProvider for CountingAcp {
	async fn generate(
		&self,
		prompt: &str,
		config: &AgentStepConfig,
		_cancel: CancellationToken,
	) -> Result<AgentResult, SimseError> {
		self.count.fetch_add(1, Ordering::SeqCst);
		Ok(AgentResult {
			output: format!("[acp:{}] {}", config.name, prompt),
			model: Some("test-model".to_string()),
			usage: None,
			tool_metrics: None,
		})
	}
}

struct NoopMcp;

#[async_trait]
impl McpProvider for NoopMcp {
	async fn call_tool(
		&self,
		_server: &str,
		_tool: &str,
		_input: &str,
		_cancel: CancellationToken,
	) -> Result<AgentResult, SimseError> {
		Ok(AgentResult {
			output: "mcp result".to_string(),
			model: Some("mcp-model".to_string()),
			usage: None,
			tool_metrics: None,
		})
	}
}

struct NoopLibrary;

#[async_trait]
impl LibraryProvider for NoopLibrary {
	async fn query(
		&self,
		_prompt: &str,
		_cancel: CancellationToken,
	) -> Result<AgentResult, SimseError> {
		Ok(AgentResult {
			output: "library result".to_string(),
			model: Some("library-model".to_string()),
			usage: None,
			tool_metrics: None,
		})
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_executor(acp: impl AcpProvider + 'static) -> AgentExecutor {
	AgentExecutor::new(
		Box::new(acp),
		Box::new(NoopMcp),
		Box::new(NoopLibrary),
	)
}

fn make_echo_executor() -> AgentExecutor {
	make_executor(EchoAcp)
}

fn make_template(s: &str) -> PromptTemplate {
	PromptTemplate::new(s).expect("valid template")
}

fn make_step(name: &str, template: &str) -> ChainStepConfig {
	ChainStepConfig {
		name: name.to_string(),
		template: make_template(template),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: None,
	}
}

// ---------------------------------------------------------------------------
// Chain with single generate step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chain_single_generate_step() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();
	chain
		.add_step(make_step("greet", "Hello {name}"))
		.unwrap();

	let mut values = HashMap::new();
	values.insert("name".to_string(), "World".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	assert_eq!(results[0].step_name, "greet");
	assert_eq!(results[0].input, "Hello World");
	assert_eq!(results[0].output, "[acp:greet] Hello World");
	assert_eq!(results[0].provider, Provider::Acp);
	assert_eq!(results[0].step_index, 0);
	assert_eq!(results[0].model, "test-model");
}

// ---------------------------------------------------------------------------
// Chain with multiple sequential steps
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chain_multiple_sequential_steps() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();
	chain
		.add_step(make_step("step1", "First: {input}"))
		.unwrap();
	chain
		.add_step(make_step("step2", "Second: {previous_output}"))
		.unwrap();
	chain
		.add_step(make_step("step3", "Third: {step1}"))
		.unwrap();

	let mut values = HashMap::new();
	values.insert("input".to_string(), "start".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 3);

	// Step 1: uses "input" variable
	assert_eq!(results[0].step_name, "step1");
	assert_eq!(results[0].input, "First: start");
	assert_eq!(results[0].output, "[acp:step1] First: start");

	// Step 2: uses "previous_output" (output of step1)
	assert_eq!(results[1].step_name, "step2");
	assert!(results[1].input.contains("[acp:step1] First: start"));

	// Step 3: uses "step1" named output
	assert_eq!(results[2].step_name, "step3");
	assert!(results[2].input.contains("[acp:step1] First: start"));
}

// ---------------------------------------------------------------------------
// Chain step output populates named values
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_step_output_populates_named_values() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();
	chain
		.add_step(make_step("extract", "Extract from {doc}"))
		.unwrap();
	chain
		.add_step(make_step("summarize", "Summarize: {extract}"))
		.unwrap();

	let mut values = HashMap::new();
	values.insert("doc".to_string(), "the document".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);
	// The second step should have access to the first step's output via {extract}
	assert!(results[1].input.contains("[acp:extract]"));
}

// ---------------------------------------------------------------------------
// Parallel steps with concat merge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_steps_concat_merge() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();

	let parallel_step = ChainStepConfig {
		name: "parallel".to_string(),
		template: make_template("unused"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: Some(ParallelConfig {
			sub_steps: vec![
				ParallelSubStepConfig {
					name: "sub_a".to_string(),
					template: make_template("A: {topic}"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
				ParallelSubStepConfig {
					name: "sub_b".to_string(),
					template: make_template("B: {topic}"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
			],
			merge_strategy: MergeStrategy::Concat,
			fail_tolerant: false,
			concat_separator: "\n---\n".to_string(),
		}),
	};

	chain.add_step(parallel_step).unwrap();

	let mut values = HashMap::new();
	values.insert("topic".to_string(), "rust".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	let result = &results[0];
	assert_eq!(result.step_name, "parallel");
	assert_eq!(result.provider, Provider::Acp);
	assert!(result.model.starts_with("parallel:"));

	// The merged output should contain both sub-step outputs joined by separator
	assert!(result.output.contains("[acp:parallel.sub_a]"));
	assert!(result.output.contains("[acp:parallel.sub_b]"));
	assert!(result.output.contains("\n---\n"));

	// Sub-results should be populated
	let subs = result.sub_results.as_ref().unwrap();
	assert_eq!(subs.len(), 2);
}

// ---------------------------------------------------------------------------
// Parallel steps with keyed merge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_steps_keyed_merge() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();

	let parallel_step = ChainStepConfig {
		name: "research".to_string(),
		template: make_template("unused"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: Some(ParallelConfig {
			sub_steps: vec![
				ParallelSubStepConfig {
					name: "history".to_string(),
					template: make_template("History of {topic}"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
				ParallelSubStepConfig {
					name: "future".to_string(),
					template: make_template("Future of {topic}"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
			],
			merge_strategy: MergeStrategy::Keyed,
			fail_tolerant: false,
			concat_separator: "\n\n".to_string(),
		}),
	};

	chain.add_step(parallel_step).unwrap();

	// Add a step that uses keyed output
	chain
		.add_step(make_step(
			"combine",
			"History: {research.history} | Future: {research.future}",
		))
		.unwrap();

	let mut values = HashMap::new();
	values.insert("topic".to_string(), "AI".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);

	// The keyed merge should have populated research.history and research.future
	// The combine step should have access to them
	let combine_result = &results[1];
	assert!(combine_result.input.contains("History:"));
	assert!(combine_result.input.contains("Future:"));
}

// ---------------------------------------------------------------------------
// ChainBuilder fluent API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chain_builder_fluent_api() {
	let executor = make_echo_executor();

	let chain = ChainBuilder::new()
		.name("test-chain")
		.step(make_step("step1", "Hello {name}"))
		.unwrap()
		.step(make_step("step2", "Goodbye {previous_output}"))
		.unwrap()
		.build();

	assert_eq!(chain.len(), 2);
	assert!(!chain.is_empty());

	let mut values = HashMap::new();
	values.insert("name".to_string(), "builder".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);
	assert_eq!(results[0].step_name, "step1");
	assert_eq!(results[1].step_name, "step2");
}

// ---------------------------------------------------------------------------
// Empty chain error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_empty_chain_errors() {
	let executor = make_echo_executor();
	let chain = Chain::new();

	let err = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap_err();

	assert!(matches!(
		err,
		SimseError::Chain {
			code: simse_core::error::ChainErrorCode::Empty,
			..
		}
	));
}

// ---------------------------------------------------------------------------
// Error in step fires error callback
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_error_in_step_fires_error_callback() {
	let executor = make_executor(FailingAcp);

	let error_fired = Arc::new(Mutex::new(false));
	let error_step_name = Arc::new(Mutex::new(String::new()));
	let chain_error_fired = Arc::new(Mutex::new(false));

	let ef = error_fired.clone();
	let esn = error_step_name.clone();
	let cef = chain_error_fired.clone();

	let callbacks = ChainCallbacks {
		on_step_start: None,
		on_step_complete: None,
		on_step_error: Some(Arc::new(move |info| {
			let ef = ef.clone();
			let esn = esn.clone();
			Box::pin(async move {
				*ef.lock().await = true;
				*esn.lock().await = info.step_name;
			})
		})),
		on_chain_complete: None,
		on_chain_error: Some(Arc::new(move |_info| {
			let cef = cef.clone();
			Box::pin(async move {
				*cef.lock().await = true;
			})
		})),
	};

	let mut chain = Chain::new();
	chain.set_callbacks(callbacks);
	chain
		.add_step(make_step("failing_step", "Will fail"))
		.unwrap();

	let err = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap_err();

	// Error should be a chain error
	assert!(matches!(err, SimseError::Chain { .. }));

	// Step error callback should have been fired
	assert!(*error_fired.lock().await);
	assert_eq!(*error_step_name.lock().await, "failing_step");

	// Chain error callback should have been fired
	assert!(*chain_error_fired.lock().await);
}

// ---------------------------------------------------------------------------
// Success callbacks fire
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_success_callbacks_fire() {
	let executor = make_echo_executor();

	let step_starts = Arc::new(Mutex::new(Vec::<String>::new()));
	let step_completes = Arc::new(Mutex::new(Vec::<String>::new()));
	let chain_complete_fired = Arc::new(Mutex::new(false));

	let ss = step_starts.clone();
	let sc = step_completes.clone();
	let ccf = chain_complete_fired.clone();

	let callbacks = ChainCallbacks {
		on_step_start: Some(Arc::new(move |info| {
			let ss = ss.clone();
			Box::pin(async move {
				ss.lock().await.push(info.step_name);
			})
		})),
		on_step_complete: Some(Arc::new(move |result| {
			let sc = sc.clone();
			Box::pin(async move {
				sc.lock().await.push(result.step_name);
			})
		})),
		on_step_error: None,
		on_chain_complete: Some(Arc::new(move |_results| {
			let ccf = ccf.clone();
			Box::pin(async move {
				*ccf.lock().await = true;
			})
		})),
		on_chain_error: None,
	};

	let mut chain = Chain::new();
	chain.set_callbacks(callbacks);
	chain.add_step(make_step("s1", "hello")).unwrap();
	chain.add_step(make_step("s2", "world")).unwrap();

	let results = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);

	let starts = step_starts.lock().await;
	assert_eq!(starts.len(), 2);
	assert_eq!(starts[0], "s1");
	assert_eq!(starts[1], "s2");

	let completes = step_completes.lock().await;
	assert_eq!(completes.len(), 2);
	assert_eq!(completes[0], "s1");
	assert_eq!(completes[1], "s2");

	assert!(*chain_complete_fired.lock().await);
}

// ---------------------------------------------------------------------------
// create_chain_from_definition constructs chain
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_chain_from_definition() {
	let executor = make_echo_executor();

	let definition = ChainDefinition {
		description: Some("Test chain".to_string()),
		agent_id: Some("test-agent".to_string()),
		server_name: None,
		initial_values: HashMap::new(),
		steps: vec![
			ChainStepDefinition {
				name: "analyze".to_string(),
				template: "Analyze {doc}".to_string(),
				provider: None,
				agent_id: None,
				server_name: None,
				agent_config: None,
				system_prompt: None,
				input_mapping: None,
				mcp_server_name: None,
				mcp_tool_name: None,
				mcp_arguments: None,
				store_to_memory: None,
				memory_metadata: None,
				parallel: None,
			},
			ChainStepDefinition {
				name: "summarize".to_string(),
				template: "Summarize: {previous_output}".to_string(),
				provider: None,
				agent_id: None,
				server_name: None,
				agent_config: None,
				system_prompt: None,
				input_mapping: None,
				mcp_server_name: None,
				mcp_tool_name: None,
				mcp_arguments: None,
				store_to_memory: None,
				memory_metadata: None,
				parallel: None,
			},
		],
	};

	let chain = create_chain_from_definition(&definition, Some("my-chain")).unwrap();
	assert_eq!(chain.len(), 2);

	let mut values = HashMap::new();
	values.insert("doc".to_string(), "test doc".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);
	assert_eq!(results[0].step_name, "analyze");
	assert_eq!(results[1].step_name, "summarize");
}

// ---------------------------------------------------------------------------
// create_chain_from_definition with empty steps errors
// ---------------------------------------------------------------------------

#[test]
fn test_create_chain_from_definition_empty_steps() {
	let definition = ChainDefinition {
		description: None,
		agent_id: None,
		server_name: None,
		initial_values: HashMap::new(),
		steps: vec![],
	};

	let err = create_chain_from_definition(&definition, None).unwrap_err();
	assert!(matches!(
		err,
		SimseError::Chain {
			code: simse_core::error::ChainErrorCode::Empty,
			..
		}
	));
}

// ---------------------------------------------------------------------------
// run_named_chain finds and runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_named_chain() {
	let executor = make_echo_executor();

	let mut chains = HashMap::new();
	chains.insert(
		"greet-chain".to_string(),
		ChainDefinition {
			description: Some("A greeting chain".to_string()),
			agent_id: None,
			server_name: None,
			initial_values: {
				let mut m = HashMap::new();
				m.insert("name".to_string(), "default-user".to_string());
				m
			},
			steps: vec![ChainStepDefinition {
				name: "greet".to_string(),
				template: "Hello {name}!".to_string(),
				provider: None,
				agent_id: None,
				server_name: None,
				agent_config: None,
				system_prompt: None,
				input_mapping: None,
				mcp_server_name: None,
				mcp_tool_name: None,
				mcp_arguments: None,
				store_to_memory: None,
				memory_metadata: None,
				parallel: None,
			}],
		},
	);

	let config = AppConfig {
		chains,
		..Default::default()
	};

	let results = run_named_chain(
		"greet-chain",
		&config,
		&executor,
		CancellationToken::new(),
		None,
	)
	.await
	.unwrap();

	assert_eq!(results.len(), 1);
	assert_eq!(results[0].input, "Hello default-user!");
}

// ---------------------------------------------------------------------------
// run_named_chain with override values
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_named_chain_with_overrides() {
	let executor = make_echo_executor();

	let mut chains = HashMap::new();
	chains.insert(
		"greet-chain".to_string(),
		ChainDefinition {
			description: None,
			agent_id: None,
			server_name: None,
			initial_values: {
				let mut m = HashMap::new();
				m.insert("name".to_string(), "default".to_string());
				m
			},
			steps: vec![ChainStepDefinition {
				name: "greet".to_string(),
				template: "Hello {name}!".to_string(),
				provider: None,
				agent_id: None,
				server_name: None,
				agent_config: None,
				system_prompt: None,
				input_mapping: None,
				mcp_server_name: None,
				mcp_tool_name: None,
				mcp_arguments: None,
				store_to_memory: None,
				memory_metadata: None,
				parallel: None,
			}],
		},
	);

	let config = AppConfig {
		chains,
		..Default::default()
	};

	let mut overrides = HashMap::new();
	overrides.insert("name".to_string(), "overridden-user".to_string());

	let results = run_named_chain(
		"greet-chain",
		&config,
		&executor,
		CancellationToken::new(),
		Some(overrides),
	)
	.await
	.unwrap();

	assert_eq!(results.len(), 1);
	assert_eq!(results[0].input, "Hello overridden-user!");
}

// ---------------------------------------------------------------------------
// run_named_chain not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_named_chain_not_found() {
	let executor = make_echo_executor();
	let config = AppConfig::default();

	let err = run_named_chain(
		"nonexistent",
		&config,
		&executor,
		CancellationToken::new(),
		None,
	)
	.await
	.unwrap_err();

	assert!(matches!(
		err,
		SimseError::Chain {
			code: simse_core::error::ChainErrorCode::NotFound,
			..
		}
	));
}

// ---------------------------------------------------------------------------
// MCP step validation
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_step_requires_server_and_tool() {
	let mut chain = Chain::new();
	let step = ChainStepConfig {
		name: "mcp-step".to_string(),
		template: make_template("query"),
		provider: Some(Provider::Mcp),
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None, // Missing!
		mcp_tool_name: None,   // Missing!
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: None,
	};

	let err = chain.add_step(step).unwrap_err();
	assert!(matches!(
		err,
		SimseError::Chain {
			code: simse_core::error::ChainErrorCode::InvalidStep,
			..
		}
	));
}

// ---------------------------------------------------------------------------
// Parallel step must have >= 2 sub-steps
// ---------------------------------------------------------------------------

#[test]
fn test_parallel_step_requires_at_least_2_substeps() {
	let mut chain = Chain::new();
	let step = ChainStepConfig {
		name: "parallel".to_string(),
		template: make_template("unused"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: Some(ParallelConfig {
			sub_steps: vec![ParallelSubStepConfig {
				name: "only_one".to_string(),
				template: make_template("solo"),
				provider: None,
				agent_id: None,
				server_name: None,
				system_prompt: None,
				output_transform: None,
				mcp_server_name: None,
				mcp_tool_name: None,
				mcp_arguments: None,
			}],
			merge_strategy: MergeStrategy::Concat,
			fail_tolerant: false,
			concat_separator: "\n\n".to_string(),
		}),
	};

	let err = chain.add_step(step).unwrap_err();
	assert!(matches!(
		err,
		SimseError::Chain {
			code: simse_core::error::ChainErrorCode::InvalidStep,
			..
		}
	));
}

// ---------------------------------------------------------------------------
// Output transform is applied
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_output_transform_applied() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();

	let step = ChainStepConfig {
		name: "transform".to_string(),
		template: make_template("hello"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: Some(Arc::new(|output: &str| output.to_uppercase())),
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: None,
	};

	chain.add_step(step).unwrap();

	let results = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	// Output should be uppercased
	assert_eq!(results[0].output, "[ACP:TRANSFORM] HELLO");
}

// ---------------------------------------------------------------------------
// Input mapping works
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_input_mapping() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();

	chain
		.add_step(make_step("step1", "Generate: {topic}"))
		.unwrap();

	let mut step2 = make_step("step2", "Refine: {content}");
	let mut mapping = HashMap::new();
	mapping.insert("content".to_string(), "step1".to_string());
	step2.input_mapping = Some(mapping);
	chain.add_step(step2).unwrap();

	let mut values = HashMap::new();
	values.insert("topic".to_string(), "rust".to_string());

	let results = chain
		.run(values, &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 2);
	// step2 should map step1's output to {content}
	assert!(results[1].input.contains("[acp:step1]"));
}

// ---------------------------------------------------------------------------
// Clear removes all steps
// ---------------------------------------------------------------------------

#[test]
fn test_chain_clear() {
	let mut chain = Chain::new();
	chain.add_step(make_step("s1", "hello")).unwrap();
	chain.add_step(make_step("s2", "world")).unwrap();
	assert_eq!(chain.len(), 2);

	chain.clear();
	assert_eq!(chain.len(), 0);
	assert!(chain.is_empty());
}

// ---------------------------------------------------------------------------
// step_configs returns current steps
// ---------------------------------------------------------------------------

#[test]
fn test_step_configs() {
	let mut chain = Chain::new();
	chain.add_step(make_step("a", "aaa")).unwrap();
	chain.add_step(make_step("b", "bbb")).unwrap();

	let configs = chain.step_configs();
	assert_eq!(configs.len(), 2);
	assert_eq!(configs[0].name, "a");
	assert_eq!(configs[1].name, "b");
}

// ---------------------------------------------------------------------------
// Template resolution failure fires error callback
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_template_resolution_failure() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();
	// Template requires {missing_var} but we won't provide it
	chain
		.add_step(make_step("bad", "Hello {missing_var}"))
		.unwrap();

	let err = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap_err();

	assert!(matches!(err, SimseError::Chain { .. }));
	let msg = err.to_string();
	assert!(msg.contains("template resolution failed") || msg.contains("Missing template"));
}

// ---------------------------------------------------------------------------
// Custom merge strategy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_custom_merge_strategy() {
	let executor = make_echo_executor();
	let mut chain = Chain::new();

	let parallel_step = ChainStepConfig {
		name: "custom_merge".to_string(),
		template: make_template("unused"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: Some(ParallelConfig {
			sub_steps: vec![
				ParallelSubStepConfig {
					name: "a".to_string(),
					template: make_template("alpha"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
				ParallelSubStepConfig {
					name: "b".to_string(),
					template: make_template("beta"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
			],
			merge_strategy: MergeStrategy::Custom(Arc::new(|results| {
				format!("MERGED({})", results.len())
			})),
			fail_tolerant: false,
			concat_separator: "\n\n".to_string(),
		}),
	};

	chain.add_step(parallel_step).unwrap();

	let results = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	assert_eq!(results[0].output, "MERGED(2)");
}

// ---------------------------------------------------------------------------
// Fail-tolerant parallel step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_fail_tolerant() {
	// Use an ACP provider where one sub-step might work differently.
	// We'll use a provider that fails for specific step names.
	struct SelectiveAcp;

	#[async_trait]
	impl AcpProvider for SelectiveAcp {
		async fn generate(
			&self,
			prompt: &str,
			config: &AgentStepConfig,
			_cancel: CancellationToken,
		) -> Result<AgentResult, SimseError> {
			if config.name.contains("fail") {
				Err(SimseError::other("selective failure"))
			} else {
				Ok(AgentResult {
					output: format!("ok:{}", prompt),
					model: Some("test".to_string()),
					usage: None,
					tool_metrics: None,
				})
			}
		}
	}

	let executor = make_executor(SelectiveAcp);
	let mut chain = Chain::new();

	let parallel_step = ChainStepConfig {
		name: "tolerant".to_string(),
		template: make_template("unused"),
		provider: None,
		agent_id: None,
		server_name: None,
		system_prompt: None,
		output_transform: None,
		input_mapping: None,
		mcp_server_name: None,
		mcp_tool_name: None,
		mcp_arguments: None,
		store_to_memory: false,
		memory_metadata: None,
		parallel: Some(ParallelConfig {
			sub_steps: vec![
				ParallelSubStepConfig {
					name: "ok_step".to_string(),
					template: make_template("good"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
				ParallelSubStepConfig {
					name: "fail_step".to_string(),
					template: make_template("bad"),
					provider: None,
					agent_id: None,
					server_name: None,
					system_prompt: None,
					output_transform: None,
					mcp_server_name: None,
					mcp_tool_name: None,
					mcp_arguments: None,
				},
			],
			merge_strategy: MergeStrategy::Concat,
			fail_tolerant: true,
			concat_separator: "\n\n".to_string(),
		}),
	};

	chain.add_step(parallel_step).unwrap();

	let results = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	// Only the successful sub-step should be in the output
	let subs = results[0].sub_results.as_ref().unwrap();
	assert_eq!(subs.len(), 1);
	assert_eq!(subs[0].sub_step_name, "ok_step");
}

// ---------------------------------------------------------------------------
// Cancellation stops chain execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cancellation_stops_chain() {
	let count = Arc::new(AtomicUsize::new(0));
	let executor = make_executor(CountingAcp {
		count: count.clone(),
	});

	let mut chain = Chain::new();
	chain.add_step(make_step("s1", "first")).unwrap();
	chain.add_step(make_step("s2", "second")).unwrap();

	let cancel = CancellationToken::new();
	cancel.cancel(); // Cancel immediately

	let err = chain
		.run(HashMap::new(), &executor, cancel)
		.await
		.unwrap_err();

	assert!(matches!(err, SimseError::Chain { .. }));
	// No steps should have executed
	assert_eq!(count.load(Ordering::SeqCst), 0);
}

// ---------------------------------------------------------------------------
// create_chain_from_definition with parallel steps
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_chain_from_definition_with_parallel() {
	use simse_core::config::{
		MergeStrategy as CfgMerge, ParallelConfigDefinition, ParallelSubStepDefinition,
	};

	let executor = make_echo_executor();

	let definition = ChainDefinition {
		description: None,
		agent_id: None,
		server_name: None,
		initial_values: HashMap::new(),
		steps: vec![ChainStepDefinition {
			name: "parallel_step".to_string(),
			template: "unused".to_string(),
			provider: None,
			agent_id: None,
			server_name: None,
			agent_config: None,
			system_prompt: None,
			input_mapping: None,
			mcp_server_name: None,
			mcp_tool_name: None,
			mcp_arguments: None,
			store_to_memory: None,
			memory_metadata: None,
			parallel: Some(ParallelConfigDefinition {
				sub_steps: vec![
					ParallelSubStepDefinition {
						name: "sub_x".to_string(),
						template: "X says hi".to_string(),
						provider: None,
						agent_id: None,
						server_name: None,
						agent_config: None,
						system_prompt: None,
						mcp_server_name: None,
						mcp_tool_name: None,
						mcp_arguments: None,
					},
					ParallelSubStepDefinition {
						name: "sub_y".to_string(),
						template: "Y says hi".to_string(),
						provider: None,
						agent_id: None,
						server_name: None,
						agent_config: None,
						system_prompt: None,
						mcp_server_name: None,
						mcp_tool_name: None,
						mcp_arguments: None,
					},
				],
				merge_strategy: Some(CfgMerge::Concat),
				fail_tolerant: None,
				concat_separator: Some(" | ".to_string()),
			}),
		}],
	};

	let chain = create_chain_from_definition(&definition, Some("par-chain")).unwrap();
	assert_eq!(chain.len(), 1);

	let results = chain
		.run(HashMap::new(), &executor, CancellationToken::new())
		.await
		.unwrap();

	assert_eq!(results.len(), 1);
	assert!(results[0].output.contains(" | "));
	assert!(results[0].sub_results.is_some());
	assert_eq!(results[0].sub_results.as_ref().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Chain default is empty
// ---------------------------------------------------------------------------

#[test]
fn test_chain_default() {
	let chain = Chain::default();
	assert!(chain.is_empty());
	assert_eq!(chain.len(), 0);
}

// ---------------------------------------------------------------------------
// ChainBuilder default
// ---------------------------------------------------------------------------

#[test]
fn test_chain_builder_default() {
	let builder = ChainBuilder::default();
	let chain = builder.build();
	assert!(chain.is_empty());
}
