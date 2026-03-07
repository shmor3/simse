//! Skill discovery and formatting.

use serde::{Deserialize, Serialize};

/// A skill definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
	pub name: String,
	pub description: String,
	pub trigger: String,
}
