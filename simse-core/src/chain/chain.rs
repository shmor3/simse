//! Chain execution -- createChain, createChainFromDefinition, runNamedChain.
//!
//! Ports `src/ai/chain/chain.ts` (~828 lines).
//!
//! A chain is a sequence of steps that execute against AI providers (ACP, MCP,
//! or library). Each step's output is made available to subsequent steps via
//! `currentValues`. Steps can run sequentially or in parallel fan-out/fan-in.

use std::collections::HashMap;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use crate::agent::{AgentExecutor, AgentStepConfig, ProviderRef};
use crate::chain::template::PromptTemplate;
use crate::chain::types::{
	ChainCallbacks, ChainErrorInfo, ChainStepConfig, MergeStrategy, ParallelConfig,
	ParallelSubResult, Provider, StepErrorInfo, StepResult, StepStartInfo,
};
use crate::config::{AppConfig, ChainDefinition, MergeStrategy as ConfigMergeStrategy, StepProvider};
use crate::error::{ChainErrorCode, SimseError};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a chain-level `Provider` to the agent-level `ProviderRef`.
fn to_provider_ref(provider: &Provider, step: &ChainStepConfig) -> ProviderRef {
	match provider {
		Provider::Acp => ProviderRef::Acp {
			server_name: step
				.server_name
				.clone()
				.unwrap_or_else(|| "default".to_string()),
		},
		Provider::Mcp => ProviderRef::Mcp {
			server_name: step
				.mcp_server_name
				.clone()
				.unwrap_or_default(),
			tool_name: step
				.mcp_tool_name
				.clone()
				.unwrap_or_default(),
		},
		Provider::Memory => ProviderRef::Library,
	}
}

/// Build an `AgentStepConfig` from a `ChainStepConfig`.
fn to_agent_step(step: &ChainStepConfig) -> AgentStepConfig {
	AgentStepConfig {
		name: step.name.clone(),
		agent_id: step.agent_id.clone(),
		server_name: step.server_name.clone(),
		timeout_ms: None,
		max_tokens: None,
		temperature: None,
		system_prompt: step.system_prompt.clone(),
	}
}

/// Fire a callback, swallowing any errors (matches TS behavior of try/catch).
async fn fire_step_start(callbacks: &Option<ChainCallbacks>, info: StepStartInfo) {
	if let Some(cb) = callbacks
		&& let Some(handler) = &cb.on_step_start {
			handler(info).await;
		}
}

async fn fire_step_complete(callbacks: &Option<ChainCallbacks>, result: StepResult) {
	if let Some(cb) = callbacks
		&& let Some(handler) = &cb.on_step_complete {
			handler(result).await;
		}
}

async fn fire_step_error(callbacks: &Option<ChainCallbacks>, info: StepErrorInfo) {
	if let Some(cb) = callbacks
		&& let Some(handler) = &cb.on_step_error {
			handler(info).await;
		}
}

async fn fire_chain_complete(callbacks: &Option<ChainCallbacks>, results: Vec<StepResult>) {
	if let Some(cb) = callbacks
		&& let Some(handler) = &cb.on_chain_complete {
			handler(results).await;
		}
}

async fn fire_chain_error(callbacks: &Option<ChainCallbacks>, info: ChainErrorInfo) {
	if let Some(cb) = callbacks
		&& let Some(handler) = &cb.on_chain_error {
			handler(info).await;
		}
}

// ---------------------------------------------------------------------------
// Chain
// ---------------------------------------------------------------------------

/// A chain of steps that execute sequentially against AI providers.
///
/// Use [`ChainBuilder`] for fluent construction or [`create_chain_from_definition`]
/// to build from a declarative config.
#[derive(Debug)]
pub struct Chain {
	steps: Vec<ChainStepConfig>,
	callbacks: Option<ChainCallbacks>,
	chain_name: Option<String>,
}

impl Chain {
	/// Create a new empty chain.
	pub fn new() -> Self {
		Self {
			steps: Vec::new(),
			callbacks: None,
			chain_name: None,
		}
	}

	/// Set the chain name (used in error messages and logging).
	pub fn set_name(&mut self, name: impl Into<String>) {
		self.chain_name = Some(name.into());
	}

	/// Append a step to the chain. Returns `&mut Self` for fluent chaining.
	///
	/// # Errors
	///
	/// Returns `SimseError::Chain` if:
	/// - An MCP step is missing `mcp_server_name` or `mcp_tool_name`
	/// - A parallel step has fewer than 2 sub-steps
	/// - A parallel sub-step with MCP provider is missing required fields
	pub fn add_step(&mut self, step: ChainStepConfig) -> Result<&mut Self, SimseError> {
		// Validate MCP steps eagerly
		if step.provider.as_ref() == Some(&Provider::Mcp)
			&& (step.mcp_server_name.is_none() || step.mcp_tool_name.is_none())
		{
			return Err(SimseError::chain(
				ChainErrorCode::InvalidStep,
				format!(
					"MCP step \"{}\" requires both mcp_server_name and mcp_tool_name",
					step.name
				),
			));
		}

		// Validate parallel steps
		if let Some(ref parallel) = step.parallel {
			if parallel.sub_steps.len() < 2 {
				return Err(SimseError::chain(
					ChainErrorCode::InvalidStep,
					format!(
						"Parallel step \"{}\" must have at least 2 sub-steps",
						step.name
					),
				));
			}
			for sub in &parallel.sub_steps {
				if sub.provider.as_ref() == Some(&Provider::Mcp)
					&& (sub.mcp_server_name.is_none() || sub.mcp_tool_name.is_none())
				{
					return Err(SimseError::chain(
						ChainErrorCode::InvalidStep,
						format!(
							"MCP sub-step \"{}.{}\" requires both mcp_server_name and mcp_tool_name",
							step.name, sub.name
						),
					));
				}
			}
		}

		self.steps.push(step);
		Ok(self)
	}

	/// Set chain-level callbacks.
	pub fn set_callbacks(&mut self, callbacks: ChainCallbacks) {
		self.callbacks = Some(callbacks);
	}

	/// Clear all steps from the chain.
	pub fn clear(&mut self) {
		self.steps.clear();
	}

	/// Return the number of steps in the chain.
	pub fn len(&self) -> usize {
		self.steps.len()
	}

	/// Returns `true` if the chain has no steps.
	pub fn is_empty(&self) -> bool {
		self.steps.is_empty()
	}

	/// Return the step configs (read-only).
	pub fn step_configs(&self) -> &[ChainStepConfig] {
		&self.steps
	}

	/// Execute all steps sequentially.
	///
	/// # Errors
	///
	/// Returns `SimseError::Chain` with `ChainErrorCode::Empty` if the chain
	/// has no steps. Returns `SimseError::Chain` with `ChainErrorCode::StepFailed`
	/// if any step fails. Fires appropriate callbacks.
	pub async fn run(
		&self,
		initial_values: HashMap<String, String>,
		executor: &AgentExecutor,
		cancel: CancellationToken,
	) -> Result<Vec<StepResult>, SimseError> {
		if self.steps.is_empty() {
			return Err(SimseError::chain(
				ChainErrorCode::Empty,
				"Cannot run an empty chain -- add steps first",
			));
		}

		let run_steps = self.steps.clone();
		let total_steps = run_steps.len();
		let mut results: Vec<StepResult> = Vec::new();
		let mut current_values = initial_values;

		let run_result = self
			.run_steps(
				&run_steps,
				total_steps,
				&mut results,
				&mut current_values,
				executor,
				&cancel,
			)
			.await;

		match run_result {
			Ok(()) => {
				fire_chain_complete(&self.callbacks, results.clone()).await;
				Ok(results)
			}
			Err(err) => {
				fire_chain_error(
					&self.callbacks,
					ChainErrorInfo {
						error: SimseError::other(err.to_string()),
						completed_steps: results.clone(),
					},
				)
				.await;

				// Re-throw chain errors directly; wrap anything else
				match &err {
					SimseError::Chain { .. } => Err(err),
					_ => Err(SimseError::chain(
						ChainErrorCode::ExecutionFailed,
						format!("Chain execution failed: {err}"),
					)),
				}
			}
		}
	}

	/// Internal: run all steps sequentially.
	async fn run_steps(
		&self,
		steps: &[ChainStepConfig],
		total_steps: usize,
		results: &mut Vec<StepResult>,
		current_values: &mut HashMap<String, String>,
		executor: &AgentExecutor,
		cancel: &CancellationToken,
	) -> Result<(), SimseError> {
		for (step_index, step) in steps.iter().enumerate() {
			if cancel.is_cancelled() {
				return Err(SimseError::chain(
					ChainErrorCode::ExecutionFailed,
					"Chain execution was cancelled",
				));
			}

			// Apply input mappings
			if let Some(ref mapping) = step.input_mapping {
				for (template_var, source_key) in mapping {
					if let Some(value) = current_values.get(source_key).cloned() {
						current_values.insert(template_var.clone(), value);
					}
				}
			}

			// Parallel step branch
			if step.parallel.is_some() {
				let parallel_result = self
					.run_parallel_step(
						step,
						step_index,
						total_steps,
						current_values,
						executor,
						cancel,
					)
					.await?;
				results.push(parallel_result);
				continue;
			}

			// Sequential step branch
			let start = Instant::now();
			let provider = step.provider.clone().unwrap_or(Provider::Acp);

			// Resolve the prompt
			let prompt = match step.template.format(current_values) {
				Ok(p) => p,
				Err(e) => {
					let step_error = SimseError::chain(
						ChainErrorCode::StepFailed,
						format!(
							"Step \"{}\" (index {}) template resolution failed: {}",
							step.name, step_index, e
						),
					);
					fire_step_error(
						&self.callbacks,
						StepErrorInfo {
							step_name: step.name.clone(),
							step_index,
							error: SimseError::chain(
								ChainErrorCode::StepFailed,
								format!("Template resolution failed: {e}"),
							),
						},
					)
					.await;
					return Err(step_error);
				}
			};

			// Fire onStepStart callback
			fire_step_start(
				&self.callbacks,
				StepStartInfo {
					step_name: step.name.clone(),
					step_index,
					total_steps,
					provider: provider.clone(),
					prompt: prompt.clone(),
				},
			)
			.await;

			// Execute the step via agent executor
			let provider_ref = to_provider_ref(&provider, step);
			let agent_step = to_agent_step(step);
			let agent_result = match executor
				.execute(&agent_step, &provider_ref, &prompt, cancel.clone())
				.await
			{
				Ok(r) => r,
				Err(e) => {
					let step_error = SimseError::chain(
						ChainErrorCode::StepFailed,
						format!(
							"Step \"{}\" (index {}) provider \"{}\" failed: {}",
							step.name, step_index, provider, e
						),
					);
					fire_step_error(
						&self.callbacks,
						StepErrorInfo {
							step_name: step.name.clone(),
							step_index,
							error: SimseError::chain(
								ChainErrorCode::StepFailed,
								format!("Provider \"{provider}\" failed: {e}"),
							),
						},
					)
					.await;
					return Err(step_error);
				}
			};

			// Apply output transform
			let output = if let Some(ref transform) = step.output_transform {
				transform(&agent_result.output)
			} else {
				agent_result.output.clone()
			};

			let duration_ms = start.elapsed().as_millis() as u64;
			let model = agent_result
				.model
				.clone()
				.unwrap_or_else(|| "unknown".to_string());

			let step_result = StepResult {
				step_name: step.name.clone(),
				provider: provider.clone(),
				model,
				input: prompt,
				output: output.clone(),
				duration_ms,
				step_index,
				usage: agent_result.usage,
				tool_metrics: agent_result.tool_metrics,
				sub_results: None,
			};

			results.push(step_result.clone());

			// Fire onStepComplete callback
			fire_step_complete(&self.callbacks, step_result).await;

			// Make the output available to subsequent steps
			current_values.insert(step.name.clone(), output.clone());
			current_values.insert("previous_output".to_string(), output);
		}

		Ok(())
	}

	/// Execute a parallel step: fan-out to sub-steps, then merge results.
	async fn run_parallel_step(
		&self,
		step: &ChainStepConfig,
		step_index: usize,
		total_steps: usize,
		current_values: &mut HashMap<String, String>,
		executor: &AgentExecutor,
		cancel: &CancellationToken,
	) -> Result<StepResult, SimseError> {
		let parallel = step.parallel.as_ref().ok_or_else(|| {
			SimseError::chain(
				ChainErrorCode::InvalidStep,
				format!("Parallel step \"{}\" has no parallel config", step.name),
			)
		})?;

		let start = Instant::now();

		// Fire onStepStart for the parent parallel step
		fire_step_start(
			&self.callbacks,
			StepStartInfo {
				step_name: step.name.clone(),
				step_index,
				total_steps,
				provider: Provider::Acp,
				prompt: format!("[parallel: {} sub-steps]", parallel.sub_steps.len()),
			},
		)
		.await;

		// Resolve all sub-step templates before fanning out
		let mut resolved: Vec<(usize, String)> = Vec::new();
		for (i, sub_step) in parallel.sub_steps.iter().enumerate() {
			match sub_step.template.format(current_values) {
				Ok(prompt) => resolved.push((i, prompt)),
				Err(e) => {
					let step_error = SimseError::chain(
						ChainErrorCode::StepFailed,
						format!(
							"Step \"{}.{}\" (index {}) template resolution failed for sub-step: {}",
							step.name, sub_step.name, step_index, e
						),
					);
					fire_step_error(
						&self.callbacks,
						StepErrorInfo {
							step_name: step.name.clone(),
							step_index,
							error: SimseError::chain(
								ChainErrorCode::StepFailed,
								format!("Template resolution failed for sub-step: {e}"),
							),
						},
					)
					.await;
					return Err(step_error);
				}
			}
		}

		// Execute sub-steps concurrently
		let default_provider = Provider::Acp;
		let sub_futures: Vec<_> = resolved
			.into_iter()
			.map(|(i, prompt)| {
				let sub_step = &parallel.sub_steps[i];
				let provider = sub_step.provider.clone().unwrap_or(default_provider.clone());
				let sub_name = format!("{}.{}", step.name, sub_step.name);

				// Build agent step config for sub-step
				let agent_step = AgentStepConfig {
					name: sub_name.clone(),
					agent_id: sub_step.agent_id.clone(),
					server_name: sub_step.server_name.clone(),
					timeout_ms: None,
					max_tokens: None,
					temperature: None,
					system_prompt: sub_step.system_prompt.clone(),
				};

				// Build provider ref for sub-step
				let provider_ref = match &provider {
					Provider::Acp => ProviderRef::Acp {
						server_name: sub_step
							.server_name
							.clone()
							.unwrap_or_else(|| "default".to_string()),
					},
					Provider::Mcp => ProviderRef::Mcp {
						server_name: sub_step.mcp_server_name.clone().unwrap_or_default(),
						tool_name: sub_step.mcp_tool_name.clone().unwrap_or_default(),
					},
					Provider::Memory => ProviderRef::Library,
				};

				let output_transform = sub_step.output_transform.clone();
				let callbacks = self.callbacks.clone();
				let cancel = cancel.clone();
				let sub_step_name = sub_step.name.clone();

				async move {
					let sub_start = Instant::now();

					// Fire onStepStart for the sub-step
					fire_step_start(
						&callbacks,
						StepStartInfo {
							step_name: sub_name.clone(),
							step_index,
							total_steps,
							provider: provider.clone(),
							prompt: prompt.clone(),
						},
					)
					.await;

					let agent_result = executor
						.execute(&agent_step, &provider_ref, &prompt, cancel)
						.await?;

					let raw_output = &agent_result.output;
					let output = if let Some(ref transform) = output_transform {
						transform(raw_output)
					} else {
						raw_output.clone()
					};

					let duration_ms = sub_start.elapsed().as_millis() as u64;
					let model = agent_result
						.model
						.clone()
						.unwrap_or_else(|| "unknown".to_string());

					let sub_result = ParallelSubResult {
						sub_step_name,
						provider: provider.clone(),
						model: model.clone(),
						input: prompt.clone(),
						output: output.clone(),
						duration_ms,
						usage: agent_result.usage.clone(),
						tool_metrics: agent_result.tool_metrics.clone(),
					};

					// Fire onStepComplete for the sub-step
					fire_step_complete(
						&callbacks,
						StepResult {
							step_name: sub_name,
							provider,
							model,
							input: prompt,
							output,
							duration_ms,
							step_index,
							usage: agent_result.usage,
							tool_metrics: agent_result.tool_metrics,
							sub_results: None,
						},
					)
					.await;

					Ok::<ParallelSubResult, SimseError>(sub_result)
				}
			})
			.collect();

		// Fan-out: execute all sub-steps concurrently
		let settled_sub_results: Vec<ParallelSubResult> = if parallel.fail_tolerant {
			let results = futures::future::join_all(sub_futures).await;
			let successes: Vec<ParallelSubResult> =
				results.into_iter().filter_map(|r| r.ok()).collect();

			if successes.is_empty() {
				let step_error = SimseError::chain(
					ChainErrorCode::StepFailed,
					format!(
						"Step \"{}\" (index {}): all parallel sub-steps failed",
						step.name, step_index
					),
				);
				fire_step_error(
					&self.callbacks,
					StepErrorInfo {
						step_name: step.name.clone(),
						step_index,
						error: SimseError::chain(
							ChainErrorCode::StepFailed,
							"All parallel sub-steps failed".to_string(),
						),
					},
				)
				.await;
				return Err(step_error);
			}

			successes
		} else {
			// Non-tolerant: fail on first error
			let results = futures::future::join_all(sub_futures).await;
			let mut successes = Vec::new();
			for result in results {
				match result {
					Ok(sub) => successes.push(sub),
					Err(e) => {
						let step_error = SimseError::chain(
							ChainErrorCode::StepFailed,
							format!(
								"Step \"{}\" (index {}): parallel sub-step failed: {}",
								step.name, step_index, e
							),
						);
						fire_step_error(
							&self.callbacks,
							StepErrorInfo {
								step_name: step.name.clone(),
								step_index,
								error: SimseError::chain(
									ChainErrorCode::StepFailed,
									format!("Parallel sub-step failed: {e}"),
								),
							},
						)
						.await;
						return Err(step_error);
					}
				}
			}
			successes
		};

		// Merge sub-results
		let merged_output = match &parallel.merge_strategy {
			MergeStrategy::Custom(f) => f(&settled_sub_results),
			MergeStrategy::Concat | MergeStrategy::Keyed => settled_sub_results
				.iter()
				.map(|r| r.output.as_str())
				.collect::<Vec<&str>>()
				.join(&parallel.concat_separator),
		};

		let duration_ms = start.elapsed().as_millis() as u64;

		let step_result = StepResult {
			step_name: step.name.clone(),
			provider: Provider::Acp,
			model: format!("parallel:{}", settled_sub_results.len()),
			input: format!("[parallel: {} sub-steps]", parallel.sub_steps.len()),
			output: merged_output.clone(),
			duration_ms,
			step_index,
			usage: None,
			tool_metrics: None,
			sub_results: Some(settled_sub_results.clone()),
		};

		// Fire onStepComplete for the parent step
		fire_step_complete(&self.callbacks, step_result.clone()).await;

		// Populate keyed values for 'keyed' merge strategy
		if matches!(parallel.merge_strategy, MergeStrategy::Keyed) {
			for sub in &settled_sub_results {
				current_values.insert(
					format!("{}.{}", step.name, sub.sub_step_name),
					sub.output.clone(),
				);
			}
		}

		current_values.insert(step.name.clone(), merged_output.clone());
		current_values.insert("previous_output".to_string(), merged_output);

		Ok(step_result)
	}
}

impl Default for Chain {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// ChainBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing a [`Chain`].
pub struct ChainBuilder {
	chain: Chain,
}

impl ChainBuilder {
	/// Create a new builder.
	pub fn new() -> Self {
		Self {
			chain: Chain::new(),
		}
	}

	/// Set the chain name.
	pub fn name(mut self, name: impl Into<String>) -> Self {
		self.chain.set_name(name);
		self
	}

	/// Add a step to the chain.
	///
	/// # Errors
	///
	/// Returns `SimseError::Chain` if the step config is invalid.
	pub fn step(mut self, step: ChainStepConfig) -> Result<Self, SimseError> {
		self.chain.add_step(step)?;
		Ok(self)
	}

	/// Set chain-level callbacks.
	pub fn callbacks(mut self, callbacks: ChainCallbacks) -> Self {
		self.chain.set_callbacks(callbacks);
		self
	}

	/// Build the chain.
	pub fn build(self) -> Chain {
		self.chain
	}
}

impl Default for ChainBuilder {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// create_chain_from_definition
// ---------------------------------------------------------------------------

/// Build a chain from a declarative [`ChainDefinition`] (from config).
///
/// Each step's template string is converted to a [`PromptTemplate`] instance.
///
/// # Errors
///
/// Returns `SimseError::Chain` with `ChainErrorCode::Empty` if the definition
/// has no steps. Returns `SimseError::Template` if any template string is invalid.
pub fn create_chain_from_definition(
	definition: &ChainDefinition,
	chain_name: Option<&str>,
) -> Result<Chain, SimseError> {
	if definition.steps.is_empty() {
		return Err(SimseError::chain(
			ChainErrorCode::Empty,
			"Chain definition has no steps",
		));
	}

	let mut chain = Chain::new();
	if let Some(name) = chain_name {
		chain.set_name(name);
	}

	for step_def in &definition.steps {
		let template = PromptTemplate::new(&step_def.template)?;

		let provider = step_def.provider.as_ref().map(|p| match p {
			StepProvider::Acp => Provider::Acp,
			StepProvider::Mcp => Provider::Mcp,
			StepProvider::Memory => Provider::Memory,
		});

		let parallel = if let Some(ref par_def) = step_def.parallel {
			let mut sub_steps = Vec::new();
			for sub_def in &par_def.sub_steps {
				let sub_template = PromptTemplate::new(&sub_def.template)?;
				let sub_provider = sub_def.provider.as_ref().map(|p| match p {
					StepProvider::Acp => Provider::Acp,
					StepProvider::Mcp => Provider::Mcp,
					StepProvider::Memory => Provider::Memory,
				});
				sub_steps.push(crate::chain::types::ParallelSubStepConfig {
					name: sub_def.name.clone(),
					template: sub_template,
					provider: sub_provider,
					agent_id: sub_def
						.agent_id
						.clone()
						.or_else(|| step_def.agent_id.clone())
						.or_else(|| definition.agent_id.clone()),
					server_name: sub_def
						.server_name
						.clone()
						.or_else(|| step_def.server_name.clone())
						.or_else(|| definition.server_name.clone()),
					system_prompt: sub_def.system_prompt.clone(),
					output_transform: None,
					mcp_server_name: sub_def.mcp_server_name.clone(),
					mcp_tool_name: sub_def.mcp_tool_name.clone(),
					mcp_arguments: sub_def.mcp_arguments.clone(),
				});
			}

			let merge_strategy = match par_def.merge_strategy {
				Some(ConfigMergeStrategy::Keyed) => MergeStrategy::Keyed,
				_ => MergeStrategy::Concat,
			};

			Some(ParallelConfig {
				sub_steps,
				merge_strategy,
				fail_tolerant: par_def.fail_tolerant.unwrap_or(false),
				concat_separator: par_def
					.concat_separator
					.clone()
					.unwrap_or_else(|| "\n\n".to_string()),
			})
		} else {
			None
		};

		let step_config = ChainStepConfig {
			name: step_def.name.clone(),
			template,
			provider,
			agent_id: step_def
				.agent_id
				.clone()
				.or_else(|| definition.agent_id.clone()),
			server_name: step_def
				.server_name
				.clone()
				.or_else(|| definition.server_name.clone()),
			system_prompt: step_def.system_prompt.clone(),
			output_transform: None,
			input_mapping: step_def.input_mapping.clone(),
			mcp_server_name: step_def.mcp_server_name.clone(),
			mcp_tool_name: step_def.mcp_tool_name.clone(),
			mcp_arguments: step_def.mcp_arguments.clone(),
			store_to_memory: step_def.store_to_memory.unwrap_or(false),
			memory_metadata: step_def.memory_metadata.clone(),
			parallel,
		};

		chain.add_step(step_config)?;
	}

	Ok(chain)
}

// ---------------------------------------------------------------------------
// run_named_chain
// ---------------------------------------------------------------------------

/// Build and run a chain from a named definition in the app config.
///
/// # Errors
///
/// Returns `SimseError::Chain` with `ChainErrorCode::NotFound` if the chain
/// name is not defined in the config.
pub async fn run_named_chain(
	chain_name: &str,
	config: &AppConfig,
	executor: &AgentExecutor,
	cancel: CancellationToken,
	override_values: Option<HashMap<String, String>>,
) -> Result<Vec<StepResult>, SimseError> {
	let definition = config.chains.get(chain_name).ok_or_else(|| {
		SimseError::chain(
			ChainErrorCode::NotFound,
			format!("Chain \"{chain_name}\" not found in config"),
		)
	})?;

	let chain = create_chain_from_definition(definition, Some(chain_name))?;

	let mut initial_values = definition.initial_values.clone();
	if let Some(overrides) = override_values {
		initial_values.extend(overrides);
	}

	chain.run(initial_values, executor, cancel).await
}
