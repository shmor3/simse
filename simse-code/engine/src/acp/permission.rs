/// Permission policy for automatic resolution of agent permission requests.
///
/// This is a simse-specific concept — not part of the ACP protocol itself.
/// The ACP protocol defines permission *requests* and *responses*; this enum
/// controls how simse automatically resolves those requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionPolicy {
	/// Automatically approve: prefer allow_always > allow_once > first option.
	AutoApprove,
	/// Forward to user for interactive resolution.
	Prompt,
	/// Automatically deny: prefer reject_once > reject_always > cancelled.
	Deny,
}

impl Default for PermissionPolicy {
	fn default() -> Self {
		Self::Prompt
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_is_prompt() {
		assert_eq!(PermissionPolicy::default(), PermissionPolicy::Prompt);
	}
}
