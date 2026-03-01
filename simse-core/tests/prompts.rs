//! Tests for the prompts module: SystemPromptBuilder, EnvironmentInfo,
//! PromptMode, ProviderPromptResolver, and discover_instructions.

use std::io::Write;

use simse_core::prompts::{
	discover_instructions, format_environment, provider_prompt, DiscoveredInstruction,
	EnvironmentInfo, PromptMode, ProviderPromptConfig, ProviderPromptResolver,
	SystemPromptBuildContext, SystemPromptBuilder,
};

// ===========================================================================
// SystemPromptBuilder
// ===========================================================================

#[test]
fn builder_builds_with_all_sections_in_order() {
	let tool_defs = "# Tools\n- tool_a: does stuff".to_string();
	let env = EnvironmentInfo {
		platform: "linux".into(),
		shell: "bash".into(),
		cwd: "/home/user/project".into(),
		date: "2026-03-01".into(),
		git_branch: Some("main".into()),
		git_status: Some("clean".into()),
	};
	let instructions = vec![DiscoveredInstruction {
		path: "/home/user/project/CLAUDE.md".into(),
		content: "Follow TDD.".into(),
	}];

	let builder = SystemPromptBuilder::new()
		.identity("You are a helpful AI.")
		.mode(PromptMode::Build)
		.tool_guidelines("Use tools wisely.")
		.environment(env)
		.instructions(instructions)
		.custom("Custom section content.")
		.tool_defs(tool_defs)
		.memory("Remember this context.");

	let result = builder.build();

	// Verify all sections are present
	assert!(result.contains("You are a helpful AI."));
	assert!(result.contains("Operating Mode: Build"));
	assert!(result.contains("Use tools wisely."));
	assert!(result.contains("Platform: linux"));
	assert!(result.contains("CLAUDE.md"));
	assert!(result.contains("Follow TDD."));
	assert!(result.contains("Custom section content."));
	assert!(result.contains("# Tools"));
	assert!(result.contains("Remember this context."));

	// Verify ordering: identity before mode before tool_guidelines before env before instructions
	let identity_pos = result.find("You are a helpful AI.").unwrap();
	let mode_pos = result.find("Operating Mode: Build").unwrap();
	let tool_guide_pos = result.find("Use tools wisely.").unwrap();
	let env_pos = result.find("Platform: linux").unwrap();
	let instr_pos = result.find("CLAUDE.md").unwrap();
	let custom_pos = result.find("Custom section content.").unwrap();
	let tool_defs_pos = result.find("# Tools").unwrap();
	let memory_pos = result.find("Remember this context.").unwrap();

	assert!(identity_pos < mode_pos, "identity before mode");
	assert!(mode_pos < tool_guide_pos, "mode before tool_guidelines");
	assert!(tool_guide_pos < env_pos, "tool_guidelines before environment");
	assert!(env_pos < instr_pos, "environment before instructions");
	assert!(instr_pos < custom_pos, "instructions before custom");
	assert!(custom_pos < tool_defs_pos, "custom before tool_defs");
	assert!(tool_defs_pos < memory_pos, "tool_defs before memory");
}

#[test]
fn builder_skips_empty_sections() {
	let builder = SystemPromptBuilder::new()
		.identity("You are an assistant.")
		.mode(PromptMode::Plan);

	let result = builder.build();

	// Should have identity and mode
	assert!(result.contains("You are an assistant."));
	assert!(result.contains("Operating Mode: Plan"));

	// Should NOT contain empty section headers for omitted sections
	assert!(!result.contains("# Environment"));
	assert!(!result.contains("# Project Instructions"));
	assert!(!result.contains("# Memory Context"));
	assert!(!result.contains("# Tool Usage Guidelines"));
}

#[test]
fn builder_uses_default_identity() {
	let builder = SystemPromptBuilder::new().mode(PromptMode::Build);
	let result = builder.build();
	assert!(result.contains("You are a software development assistant."));
}

#[test]
fn builder_uses_default_mode_instructions_for_build() {
	let builder = SystemPromptBuilder::new().mode(PromptMode::Build);
	let result = builder.build();
	assert!(result.contains("gather-action-verify"));
}

#[test]
fn builder_uses_default_mode_instructions_for_plan() {
	let builder = SystemPromptBuilder::new().mode(PromptMode::Plan);
	let result = builder.build();
	assert!(result.contains("planning mode"));
}

#[test]
fn builder_uses_default_mode_instructions_for_explore() {
	let builder = SystemPromptBuilder::new().mode(PromptMode::Explore);
	let result = builder.build();
	assert!(result.contains("exploration mode"));
}

#[test]
fn builder_default_tool_guidelines() {
	let builder = SystemPromptBuilder::new().mode(PromptMode::Build).use_default_tool_guidelines();
	let result = builder.build();
	assert!(result.contains("Tool Usage Guidelines"));
	assert!(result.contains("execute them in parallel"));
}

#[test]
fn builder_sections_separated_by_double_newlines() {
	let builder = SystemPromptBuilder::new()
		.identity("Identity.")
		.mode(PromptMode::Build)
		.memory("Memory.");

	let result = builder.build();
	// Sections should be separated by double newlines
	assert!(result.contains("\n\n"));
}

#[test]
fn builder_multiple_custom_sections() {
	let builder = SystemPromptBuilder::new()
		.custom("Section A")
		.custom("Section B")
		.custom("Section C");

	let result = builder.build();
	assert!(result.contains("Section A"));
	assert!(result.contains("Section B"));
	assert!(result.contains("Section C"));

	let a_pos = result.find("Section A").unwrap();
	let b_pos = result.find("Section B").unwrap();
	let c_pos = result.find("Section C").unwrap();
	assert!(a_pos < b_pos);
	assert!(b_pos < c_pos);
}

#[test]
fn builder_multiple_instructions() {
	let instructions = vec![
		DiscoveredInstruction {
			path: "CLAUDE.md".into(),
			content: "Instruction 1".into(),
		},
		DiscoveredInstruction {
			path: "AGENTS.md".into(),
			content: "Instruction 2".into(),
		},
	];

	let builder = SystemPromptBuilder::new().instructions(instructions);
	let result = builder.build();

	assert!(result.contains("## CLAUDE.md"));
	assert!(result.contains("Instruction 1"));
	assert!(result.contains("## AGENTS.md"));
	assert!(result.contains("Instruction 2"));
}

#[test]
fn builder_from_context() {
	let ctx = SystemPromptBuildContext {
		mode: Some(PromptMode::Explore),
		environment: Some(EnvironmentInfo {
			platform: "darwin".into(),
			shell: "zsh".into(),
			cwd: "/tmp".into(),
			date: "2026-03-01".into(),
			git_branch: None,
			git_status: None,
		}),
		instructions: Some(vec![DiscoveredInstruction {
			path: "README.md".into(),
			content: "Read me".into(),
		}]),
		memory_context: Some("Past context".into()),
	};

	let builder = SystemPromptBuilder::from_context(&ctx);
	let result = builder.build();

	assert!(result.contains("exploration mode"));
	assert!(result.contains("Platform: darwin"));
	assert!(result.contains("Read me"));
	assert!(result.contains("Past context"));
}

// ===========================================================================
// EnvironmentInfo
// ===========================================================================

#[test]
fn environment_info_formatting_full() {
	let env = EnvironmentInfo {
		platform: "linux".into(),
		shell: "bash".into(),
		cwd: "/home/user/project".into(),
		date: "2026-03-01".into(),
		git_branch: Some("feature/new".into()),
		git_status: Some("M src/main.rs".into()),
	};

	let formatted = format_environment(&env);

	assert!(formatted.contains("# Environment"));
	assert!(formatted.contains("- Platform: linux"));
	assert!(formatted.contains("- Shell: bash"));
	assert!(formatted.contains("- Working directory: /home/user/project"));
	assert!(formatted.contains("- Date: 2026-03-01"));
	assert!(formatted.contains("- Git branch: feature/new"));
	assert!(formatted.contains("- Git status:"));
	assert!(formatted.contains("M src/main.rs"));
}

#[test]
fn environment_info_formatting_minimal() {
	let env = EnvironmentInfo {
		platform: "win32".into(),
		shell: "powershell".into(),
		cwd: "C:\\Users\\test".into(),
		date: "2026-01-15".into(),
		git_branch: None,
		git_status: None,
	};

	let formatted = format_environment(&env);

	assert!(formatted.contains("- Platform: win32"));
	assert!(formatted.contains("- Shell: powershell"));
	assert!(!formatted.contains("Git branch"));
	assert!(!formatted.contains("Git status"));
}

#[test]
fn environment_info_git_status_clean() {
	let env = EnvironmentInfo {
		platform: "linux".into(),
		shell: "bash".into(),
		cwd: "/tmp".into(),
		date: "2026-03-01".into(),
		git_branch: Some("main".into()),
		git_status: Some("clean".into()),
	};

	let formatted = format_environment(&env);
	assert!(formatted.contains("- Git status: clean"));
}

#[test]
fn environment_info_git_status_dirty() {
	let env = EnvironmentInfo {
		platform: "linux".into(),
		shell: "bash".into(),
		cwd: "/tmp".into(),
		date: "2026-03-01".into(),
		git_branch: Some("main".into()),
		git_status: Some("M file.rs\nA new.rs".into()),
	};

	let formatted = format_environment(&env);
	// Dirty status should be shown as a block
	assert!(formatted.contains("- Git status:\nM file.rs\nA new.rs"));
}

// ===========================================================================
// PromptMode
// ===========================================================================

#[test]
fn mode_descriptions_are_nonempty() {
	assert!(!PromptMode::Build.description().is_empty());
	assert!(!PromptMode::Plan.description().is_empty());
	assert!(!PromptMode::Explore.description().is_empty());
}

#[test]
fn mode_descriptions_are_distinct() {
	let build = PromptMode::Build.description();
	let plan = PromptMode::Plan.description();
	let explore = PromptMode::Explore.description();

	assert_ne!(build, plan);
	assert_ne!(build, explore);
	assert_ne!(plan, explore);
}

#[test]
fn mode_from_str() {
	assert_eq!("build".parse::<PromptMode>().unwrap(), PromptMode::Build);
	assert_eq!("plan".parse::<PromptMode>().unwrap(), PromptMode::Plan);
	assert_eq!(
		"explore".parse::<PromptMode>().unwrap(),
		PromptMode::Explore
	);
	assert!("invalid".parse::<PromptMode>().is_err());
}

#[test]
fn mode_display() {
	assert_eq!(format!("{}", PromptMode::Build), "build");
	assert_eq!(format!("{}", PromptMode::Plan), "plan");
	assert_eq!(format!("{}", PromptMode::Explore), "explore");
}

// ===========================================================================
// ProviderPromptResolver
// ===========================================================================

#[test]
fn provider_prompt_resolver_exact_match() {
	let config = ProviderPromptConfig {
		prompts: vec![("claude-3-opus".into(), "You are Claude Opus.".into())],
		default_prompt: Some("Default prompt.".into()),
	};
	let resolver = ProviderPromptResolver::new(config);
	assert_eq!(resolver.resolve("claude-3-opus"), "You are Claude Opus.");
}

#[test]
fn provider_prompt_resolver_glob_match() {
	let config = ProviderPromptConfig {
		prompts: vec![("claude-*".into(), "You are Claude.".into())],
		default_prompt: Some("Default.".into()),
	};
	let resolver = ProviderPromptResolver::new(config);
	assert_eq!(resolver.resolve("claude-3-opus"), "You are Claude.");
	assert_eq!(resolver.resolve("claude-3-sonnet"), "You are Claude.");
}

#[test]
fn provider_prompt_resolver_falls_back_to_default() {
	let config = ProviderPromptConfig {
		prompts: vec![("claude-*".into(), "Claude prompt.".into())],
		default_prompt: Some("Generic prompt.".into()),
	};
	let resolver = ProviderPromptResolver::new(config);
	assert_eq!(resolver.resolve("gpt-4"), "Generic prompt.");
}

#[test]
fn provider_prompt_resolver_empty_on_no_match_no_default() {
	let config = ProviderPromptConfig {
		prompts: vec![("claude-*".into(), "Claude only.".into())],
		default_prompt: None,
	};
	let resolver = ProviderPromptResolver::new(config);
	assert_eq!(resolver.resolve("gpt-4"), "");
}

#[test]
fn provider_prompt_resolver_first_match_wins() {
	let config = ProviderPromptConfig {
		prompts: vec![
			("claude-3-*".into(), "Claude 3 specific.".into()),
			("claude-*".into(), "Claude generic.".into()),
		],
		default_prompt: None,
	};
	let resolver = ProviderPromptResolver::new(config);
	assert_eq!(resolver.resolve("claude-3-opus"), "Claude 3 specific.");
}

#[test]
fn provider_prompt_static_function() {
	// The provider_prompt function should return something for known providers
	let anthropic = provider_prompt("anthropic");
	assert!(!anthropic.is_empty());

	let openai = provider_prompt("openai");
	assert!(!openai.is_empty());

	// Unknown providers get empty string
	let unknown = provider_prompt("totally-unknown-provider");
	assert!(unknown.is_empty());
}

// ===========================================================================
// discover_instructions
// ===========================================================================

#[tokio::test]
async fn discover_instructions_finds_files() {
	let dir = tempfile::tempdir().unwrap();

	// Create CLAUDE.md
	let claude_path = dir.path().join("CLAUDE.md");
	let mut f = std::fs::File::create(&claude_path).unwrap();
	writeln!(f, "# Claude Instructions\nFollow TDD.").unwrap();

	// Create .simse/instructions.md
	std::fs::create_dir_all(dir.path().join(".simse")).unwrap();
	let simse_path = dir.path().join(".simse/instructions.md");
	let mut f2 = std::fs::File::create(&simse_path).unwrap();
	writeln!(f2, "Simse instructions here.").unwrap();

	let results = discover_instructions(dir.path()).await;

	// Should find at least CLAUDE.md and .simse/instructions.md
	let paths: Vec<&str> = results.iter().map(|r| r.path.as_str()).collect();
	assert!(
		paths.iter().any(|p| p.contains("CLAUDE.md")),
		"Should find CLAUDE.md, got: {:?}",
		paths
	);
	assert!(
		paths.iter().any(|p| p.contains("instructions.md")),
		"Should find instructions.md, got: {:?}",
		paths
	);
}

#[tokio::test]
async fn discover_instructions_skips_missing_files() {
	let dir = tempfile::tempdir().unwrap();
	// Empty directory — should return empty vec
	let results = discover_instructions(dir.path()).await;
	assert!(results.is_empty());
}

#[tokio::test]
async fn discover_instructions_with_custom_patterns() {
	let dir = tempfile::tempdir().unwrap();

	let custom_path = dir.path().join("CUSTOM.md");
	let mut f = std::fs::File::create(&custom_path).unwrap();
	writeln!(f, "Custom content.").unwrap();

	let patterns = &["CUSTOM.md"];
	let results = discover_instructions_with_patterns(dir.path(), patterns).await;

	assert_eq!(results.len(), 1);
	assert!(results[0].content.contains("Custom content."));
}

// Helper to test custom patterns — calls the full API
async fn discover_instructions_with_patterns(
	dir: &std::path::Path,
	patterns: &[&str],
) -> Vec<DiscoveredInstruction> {
	use simse_core::prompts::discover_instructions_with;
	discover_instructions_with(dir, patterns).await
}
