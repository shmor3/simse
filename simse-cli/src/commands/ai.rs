//! AI commands: `/chain`, `/prompts`.

use super::{BridgeAction, CommandContext, CommandOutput};

/// `/chain <name> [args...]` -- run a prompt chain.
pub fn handle_chain(args: &str) -> Vec<CommandOutput> {
	let args = args.trim();
	if args.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /chain <name> [args...]".into(),
		)];
	}

	let mut parts = args.splitn(2, ' ');
	let name = parts.next().unwrap_or("");
	let chain_args = parts.next().unwrap_or("").trim();

	// Validate chain name: must be non-empty, alphanumeric + hyphens/underscores.
	if !name
		.chars()
		.all(|c| c.is_alphanumeric() || c == '-' || c == '_')
	{
		return vec![CommandOutput::Error(format!(
			"Invalid chain name: \"{name}\". Use alphanumeric characters, hyphens, or underscores."
		))];
	}

	vec![
		CommandOutput::Info(format!("Running chain: {name}")),
		CommandOutput::BridgeRequest(BridgeAction::RunChain {
			name: name.to_string(),
			args: chain_args.to_string(),
		}),
	]
}

/// `/prompts` -- list available prompt templates.
pub fn handle_prompts(_args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	if ctx.prompts.is_empty() {
		return vec![CommandOutput::Info(
			"No prompt templates configured. Add prompts to .simse/prompts.json.".into(),
		)];
	}

	let rows: Vec<Vec<String>> = ctx
		.prompts
		.iter()
		.map(|p| {
			vec![
				p.name.clone(),
				p.step_count.to_string(),
				p.description.clone().unwrap_or_default(),
			]
		})
		.collect();

	vec![CommandOutput::Table {
		headers: vec!["Name".into(), "Steps".into(), "Description".into()],
		rows,
	}]
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::commands::{CommandContext, PromptInfo};

	fn empty_ctx() -> CommandContext {
		CommandContext::default()
	}

	fn prompts_ctx() -> CommandContext {
		CommandContext {
			prompts: vec![
				PromptInfo {
					name: "summarize".into(),
					description: Some("Summarize a document".into()),
					step_count: 3,
				},
				PromptInfo {
					name: "translate".into(),
					description: None,
					step_count: 2,
				},
			],
			..Default::default()
		}
	}

	// ── /chain ───────────────────────────────────────────

	#[test]
	fn chain_empty_is_error() {
		let out = handle_chain("");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	#[test]
	fn chain_name_only() {
		let out = handle_chain("summarize");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Running chain: summarize"));
		match &out[1] {
			CommandOutput::BridgeRequest(BridgeAction::RunChain { name, args }) => {
				assert_eq!(name, "summarize");
				assert_eq!(args, "");
			}
			other => panic!("expected BridgeRequest(RunChain), got {:?}", other),
		}
	}

	#[test]
	fn chain_name_and_args() {
		let out = handle_chain("translate en es");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Running chain: translate"));
		match &out[1] {
			CommandOutput::BridgeRequest(BridgeAction::RunChain { name, args }) => {
				assert_eq!(name, "translate");
				assert_eq!(args, "en es");
			}
			other => panic!("expected BridgeRequest(RunChain), got {:?}", other),
		}
	}

	#[test]
	fn chain_invalid_name() {
		let out = handle_chain("bad!name");
		assert!(
			matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Invalid chain name"))
		);
	}

	#[test]
	fn chain_name_with_hyphens_and_underscores() {
		let out = handle_chain("my-cool_chain");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		match &out[1] {
			CommandOutput::BridgeRequest(BridgeAction::RunChain { name, .. }) => {
				assert_eq!(name, "my-cool_chain");
			}
			other => panic!("expected BridgeRequest(RunChain), got {:?}", other),
		}
	}

	#[test]
	fn chain_trims_whitespace() {
		let out = handle_chain("  analyze  some text  ");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		match &out[1] {
			CommandOutput::BridgeRequest(BridgeAction::RunChain { name, args }) => {
				assert_eq!(name, "analyze");
				assert_eq!(args, "some text");
			}
			other => panic!("expected BridgeRequest(RunChain), got {:?}", other),
		}
	}

	// ── /prompts ─────────────────────────────────────────

	#[test]
	fn prompts_empty_returns_info() {
		let out = handle_prompts("", &empty_ctx());
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg == "No prompt templates configured. Add prompts to .simse/prompts.json.")
		);
	}

	#[test]
	fn prompts_returns_table() {
		let ctx = prompts_ctx();
		let out = handle_prompts("", &ctx);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Name", "Steps", "Description"]);
				assert_eq!(rows.len(), 2);
				assert_eq!(rows[0][0], "summarize");
				assert_eq!(rows[0][1], "3");
				assert_eq!(rows[0][2], "Summarize a document");
				assert_eq!(rows[1][0], "translate");
				assert_eq!(rows[1][1], "2");
				assert_eq!(rows[1][2], "");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}
}
