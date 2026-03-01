//! Tests for the config module.

use simse_core::config::*;

// ---------------------------------------------------------------------------
// Default config
// ---------------------------------------------------------------------------

#[test]
fn test_default_config() {
	let config = AppConfig::default();
	assert!(config.acp.servers.is_empty());
	assert!(config.mcp.client.servers.is_empty());
	assert!(!config.mcp.server.enabled);
	assert!(!config.library.enabled);
	assert!(config.chains.is_empty());
}

// ---------------------------------------------------------------------------
// define_config — minimal
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_minimal() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "/usr/bin/agent"
			}]
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(config.acp.servers.len(), 1);
}

// ---------------------------------------------------------------------------
// define_config — with ACP server fields
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_with_acp_server() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "/usr/bin/agent",
				"args": ["--mode", "fast"],
				"cwd": "/tmp",
				"defaultAgent": "my-agent",
				"timeoutMs": 5000,
				"permissionPolicy": "auto-approve"
			}]
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(config.acp.servers.len(), 1);
	let server = &config.acp.servers[0];
	assert_eq!(server.name, "test");
	assert_eq!(server.command, "/usr/bin/agent");
	assert_eq!(
		server.args.as_ref().unwrap(),
		&vec!["--mode".to_string(), "fast".to_string()]
	);
	assert_eq!(server.cwd.as_ref().unwrap(), "/tmp");
	assert_eq!(server.default_agent.as_ref().unwrap(), "my-agent");
	assert_eq!(server.timeout_ms.unwrap(), 5000);
	assert_eq!(server.permission_policy.as_ref().unwrap(), &PermissionPolicy::AutoApprove);
}

// ---------------------------------------------------------------------------
// define_config — with ACP defaults
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_acp_default_server() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "copilot",
				"command": "copilot"
			}],
			"defaultServer": "copilot",
			"defaultAgent": "default"
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(config.acp.default_server.as_ref().unwrap(), "copilot");
	assert_eq!(config.acp.default_agent.as_ref().unwrap(), "default");
}

// ---------------------------------------------------------------------------
// define_config — timeout_ms default applied
// ---------------------------------------------------------------------------

#[test]
fn test_acp_server_timeout_default() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "agent"
			}]
		}
	});
	let config = define_config(raw, None).unwrap();
	// timeoutMs defaults to 30_000 when not specified
	assert_eq!(config.acp.servers[0].timeout_ms.unwrap(), 30_000);
}

// ---------------------------------------------------------------------------
// Validation — empty ACP servers
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_empty_servers() {
	let raw = serde_json::json!({
		"acp": { "servers": [] }
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert_eq!(err.code(), "CONFIG_VALIDATION_FAILED");
}

// ---------------------------------------------------------------------------
// Validation — empty server name
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_empty_server_name() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "",
				"command": "agent"
			}]
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Validation — empty command
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_empty_command() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": ""
			}]
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Validation — timeoutMs below minimum
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_invalid_timeout() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "agent",
				"timeoutMs": 100
			}]
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Validation — timeoutMs above maximum
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_timeout_above_max() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "agent",
				"timeoutMs": 700000
			}]
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Validation — duplicate server names
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_duplicate_server_names() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [
				{ "name": "test", "command": "a" },
				{ "name": "test", "command": "b" }
			]
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Validation — defaultServer references nonexistent server
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rejects_unknown_default_server() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }],
			"defaultServer": "nonexistent"
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// MCP config
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_with_mcp() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"client": {
				"servers": [{
					"name": "tools",
					"transport": "stdio",
					"command": "/usr/bin/tool-server"
				}],
				"clientName": "simse",
				"clientVersion": "1.0.0"
			},
			"server": {
				"enabled": true,
				"name": "my-server",
				"version": "1.0.0"
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(config.mcp.client.servers.len(), 1);
	assert_eq!(config.mcp.client.servers[0].name, "tools");
	assert!(config.mcp.server.enabled);
	assert_eq!(config.mcp.server.name.as_ref().unwrap(), "my-server");
	assert_eq!(config.mcp.server.version.as_ref().unwrap(), "1.0.0");
}

// ---------------------------------------------------------------------------
// MCP validation — enabled without name
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_server_requires_name_when_enabled() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"server": {
				"enabled": true,
				"version": "1.0.0"
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// MCP validation — bad semver
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_server_rejects_bad_semver() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"server": {
				"enabled": true,
				"name": "s",
				"version": "bad"
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// MCP client — HTTP transport
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_client_http_server() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"client": {
				"servers": [{
					"name": "remote",
					"transport": "http",
					"url": "https://example.com/mcp"
				}]
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(config.mcp.client.servers[0].transport, McpTransport::Http);
	assert_eq!(
		config.mcp.client.servers[0].url.as_ref().unwrap(),
		"https://example.com/mcp"
	);
}

// ---------------------------------------------------------------------------
// MCP validation — HTTP without URL
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_http_requires_url() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"client": {
				"servers": [{
					"name": "remote",
					"transport": "http"
				}]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Library config
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_with_library() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": true,
			"embeddingAgent": "embed-agent",
			"similarityThreshold": 0.75,
			"maxResults": 20
		}
	});
	let config = define_config(raw, None).unwrap();
	assert!(config.library.enabled);
	assert_eq!(config.library.embedding_agent.as_ref().unwrap(), "embed-agent");
	assert!((config.library.similarity_threshold.unwrap() - 0.75).abs() < f64::EPSILON);
	assert_eq!(config.library.max_results.unwrap(), 20);
}

// ---------------------------------------------------------------------------
// Library validation — enabled without required fields
// ---------------------------------------------------------------------------

#[test]
fn test_library_enabled_requires_embedding_agent() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": true,
			"similarityThreshold": 0.7,
			"maxResults": 10
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

#[test]
fn test_library_similarity_threshold_range() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": true,
			"embeddingAgent": "e",
			"similarityThreshold": 1.5,
			"maxResults": 10
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

#[test]
fn test_library_max_results_range() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": true,
			"embeddingAgent": "e",
			"similarityThreshold": 0.7,
			"maxResults": 200
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Library disabled — no required fields needed
// ---------------------------------------------------------------------------

#[test]
fn test_library_disabled_no_required_fields() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": false
		}
	});
	let config = define_config(raw, None).unwrap();
	assert!(!config.library.enabled);
}

// ---------------------------------------------------------------------------
// Chain config
// ---------------------------------------------------------------------------

#[test]
fn test_define_config_with_chains() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"summarize": {
				"description": "Summarize a doc",
				"steps": [{
					"name": "step1",
					"template": "Summarize: {text}"
				}]
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	assert!(config.chains.contains_key("summarize"));
	let chain = &config.chains["summarize"];
	assert_eq!(chain.description.as_ref().unwrap(), "Summarize a doc");
	assert_eq!(chain.steps.len(), 1);
	assert_eq!(chain.steps[0].name, "step1");
	assert_eq!(chain.steps[0].template, "Summarize: {text}");
}

// ---------------------------------------------------------------------------
// Chain validation — empty steps
// ---------------------------------------------------------------------------

#[test]
fn test_chain_rejects_empty_steps() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"empty": {
				"steps": []
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Chain validation — step name pattern
// ---------------------------------------------------------------------------

#[test]
fn test_chain_rejects_bad_step_name() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"c": {
				"steps": [{
					"name": "123bad",
					"template": "t"
				}]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Chain — parallel config
// ---------------------------------------------------------------------------

#[test]
fn test_chain_parallel_config() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"p": {
				"steps": [{
					"name": "par_step",
					"template": "t",
					"parallel": {
						"subSteps": [
							{ "name": "sub1", "template": "t1" },
							{ "name": "sub2", "template": "t2" }
						],
						"mergeStrategy": "concat",
						"failTolerant": true,
						"concatSeparator": "\n"
					}
				}]
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	let parallel = config.chains["p"].steps[0].parallel.as_ref().unwrap();
	assert_eq!(parallel.sub_steps.len(), 2);
	assert_eq!(parallel.merge_strategy.as_ref().unwrap(), &MergeStrategy::Concat);
	assert!(parallel.fail_tolerant.unwrap());
	assert_eq!(parallel.concat_separator.as_ref().unwrap(), "\n");
}

// ---------------------------------------------------------------------------
// Chain — parallel requires 2+ sub-steps
// ---------------------------------------------------------------------------

#[test]
fn test_chain_parallel_requires_two_substeps() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"p": {
				"steps": [{
					"name": "par_step",
					"template": "t",
					"parallel": {
						"subSteps": [
							{ "name": "sub1", "template": "t1" }
						]
					}
				}]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Chain — MCP step requires mcpServerName and mcpToolName
// ---------------------------------------------------------------------------

#[test]
fn test_chain_mcp_step_requires_fields() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"c": {
				"steps": [{
					"name": "mcp_step",
					"template": "t",
					"provider": "mcp"
				}]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Serde deserialization
// ---------------------------------------------------------------------------

#[test]
fn test_config_deserialization() {
	let json = r#"{"acp":{"servers":[]},"mcp":{"client":{"servers":[]},"server":{}}}"#;
	let config: AppConfig = serde_json::from_str(json).unwrap();
	assert!(config.acp.servers.is_empty());
	assert!(config.mcp.client.servers.is_empty());
}

// ---------------------------------------------------------------------------
// Serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_config_serialization_roundtrip() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "copilot",
				"command": "copilot",
				"args": ["--acp"],
				"timeoutMs": 5000
			}],
			"defaultServer": "copilot"
		}
	});
	let config = define_config(raw, None).unwrap();
	let serialized = serde_json::to_string(&config).unwrap();
	let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
	assert_eq!(deserialized.acp.servers[0].name, "copilot");
	assert_eq!(deserialized.acp.servers[0].timeout_ms.unwrap(), 5000);
}

// ---------------------------------------------------------------------------
// Lenient mode — warnings instead of errors
// ---------------------------------------------------------------------------

#[test]
fn test_lenient_mode_warns_on_invalid() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"library": {
			"enabled": true,
			"embeddingAgent": "e",
			"similarityThreshold": 2.0,
			"maxResults": 10
		}
	});
	let warnings = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
	let w = warnings.clone();
	let config = define_config(
		raw,
		Some(DefineConfigOptions {
			lenient: true,
			on_warn: Some(Box::new(move |issues| {
				*w.lock().unwrap() = issues;
			})),
		}),
	);
	// In lenient mode, invalid fields are reset to defaults
	assert!(config.is_ok());
	let w = warnings.lock().unwrap();
	assert!(!w.is_empty(), "should have warned about invalid threshold");
}

// ---------------------------------------------------------------------------
// Lenient mode — still rejects empty servers
// ---------------------------------------------------------------------------

#[test]
fn test_lenient_mode_still_rejects_empty_servers() {
	let raw = serde_json::json!({
		"acp": { "servers": [] }
	});
	let result = define_config(
		raw,
		Some(DefineConfigOptions {
			lenient: true,
			on_warn: None,
		}),
	);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Env variables on server entries
// ---------------------------------------------------------------------------

#[test]
fn test_acp_server_env_vars() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{
				"name": "test",
				"command": "agent",
				"env": {
					"API_KEY": "secret",
					"DEBUG": "true"
				}
			}]
		}
	});
	let config = define_config(raw, None).unwrap();
	let env = config.acp.servers[0].env.as_ref().unwrap();
	assert_eq!(env.get("API_KEY").unwrap(), "secret");
	assert_eq!(env.get("DEBUG").unwrap(), "true");
}

// ---------------------------------------------------------------------------
// Chain step — agent inheritance
// ---------------------------------------------------------------------------

#[test]
fn test_chain_step_inherits_chain_agent() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"c": {
				"agentId": "chain-agent",
				"serverName": "chain-server",
				"steps": [{
					"name": "s1",
					"template": "t"
				}]
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	let step = &config.chains["c"].steps[0];
	// Steps inherit chain-level agentId/serverName when not overridden
	assert_eq!(step.agent_id.as_ref().unwrap(), "chain-agent");
	assert_eq!(step.server_name.as_ref().unwrap(), "chain-server");
}

#[test]
fn test_chain_step_overrides_chain_agent() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"c": {
				"agentId": "chain-agent",
				"steps": [{
					"name": "s1",
					"template": "t",
					"agentId": "step-agent"
				}]
			}
		}
	});
	let config = define_config(raw, None).unwrap();
	assert_eq!(
		config.chains["c"].steps[0].agent_id.as_ref().unwrap(),
		"step-agent"
	);
}

// ---------------------------------------------------------------------------
// Duplicate MCP client server names
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_client_rejects_duplicate_server_names() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"mcp": {
			"client": {
				"servers": [
					{ "name": "dup", "transport": "stdio", "command": "a" },
					{ "name": "dup", "transport": "stdio", "command": "b" }
				]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Duplicate chain step names
// ---------------------------------------------------------------------------

#[test]
fn test_chain_rejects_duplicate_step_names() {
	let raw = serde_json::json!({
		"acp": {
			"servers": [{ "name": "a", "command": "x" }]
		},
		"chains": {
			"c": {
				"steps": [
					{ "name": "s", "template": "t1" },
					{ "name": "s", "template": "t2" }
				]
			}
		}
	});
	let result = define_config(raw, None);
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Default config — all sections present
// ---------------------------------------------------------------------------

#[test]
fn test_default_config_has_all_sections() {
	let config = AppConfig::default();
	assert!(config.acp.servers.is_empty());
	assert!(config.mcp.client.servers.is_empty());
	assert!(!config.library.enabled);
	assert!(config.tools.max_output_chars.is_none());
	assert!(config.loop_config.max_turns.is_none());
	assert!(config.prompts.system_prompt.is_none());
}
