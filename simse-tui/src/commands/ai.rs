//! AI commands: `/chain`, `/prompts`.

use super::CommandOutput;

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

	if chain_args.is_empty() {
		vec![CommandOutput::Info(format!(
			"Would call bridge to run chain \"{name}\" with no arguments"
		))]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to run chain \"{name}\" with args: {chain_args}"
		))]
	}
}

/// `/prompts` -- list available prompt templates.
pub fn handle_prompts(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to list available prompt templates".into(),
	)]
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /chain ───────────────────────────────────────────

	#[test]
	fn chain_empty_is_error() {
		let out = handle_chain("");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	#[test]
	fn chain_name_only() {
		let out = handle_chain("summarize");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("summarize") && msg.contains("no arguments"))
		);
	}

	#[test]
	fn chain_name_and_args() {
		let out = handle_chain("translate en es");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("translate") && msg.contains("en es"))
		);
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
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("my-cool_chain"))
		);
	}

	#[test]
	fn chain_trims_whitespace() {
		let out = handle_chain("  analyze  some text  ");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("analyze") && msg.contains("some text"))
		);
	}

	// ── /prompts ─────────────────────────────────────────

	#[test]
	fn prompts_returns_info() {
		let out = handle_prompts("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("prompt templates")));
	}
}
