// ---------------------------------------------------------------------------
// Permission resolution — maps a PermissionPolicy + PermissionRequestParams
// to a PermissionResult for automatic handling, or None for user prompt.
// ---------------------------------------------------------------------------

use crate::protocol::{PermissionOutcome, PermissionPolicy, PermissionRequestParams, PermissionResult};

/// Resolve a permission request based on the current policy.
///
/// Returns `Some(PermissionResult)` for policies that can be resolved
/// automatically (`AutoApprove`, `Deny`), or `None` for `Prompt` — the
/// caller must forward the request to an external callback.
pub fn resolve_permission(
	policy: PermissionPolicy,
	params: &PermissionRequestParams,
) -> Option<PermissionResult> {
	match policy {
		PermissionPolicy::AutoApprove => Some(resolve_auto_approve(params)),
		PermissionPolicy::Deny => Some(resolve_deny(params)),
		PermissionPolicy::Prompt => None,
	}
}

// ---------------------------------------------------------------------------
// AutoApprove: prefer allow_always > allow_once > first option
// ---------------------------------------------------------------------------

fn resolve_auto_approve(params: &PermissionRequestParams) -> PermissionResult {
	let options = &params.options;

	// Prefer allow_always
	if let Some(opt) = options.iter().find(|o| o.kind == "allow_always") {
		return selected(&opt.option_id);
	}

	// Fallback to allow_once
	if let Some(opt) = options.iter().find(|o| o.kind == "allow_once") {
		return selected(&opt.option_id);
	}

	// Fallback to first option (whatever it is)
	if let Some(opt) = options.first() {
		return selected(&opt.option_id);
	}

	// No options at all — cancelled
	cancelled()
}

// ---------------------------------------------------------------------------
// Deny: prefer reject_once > reject_always > cancelled
// ---------------------------------------------------------------------------

fn resolve_deny(params: &PermissionRequestParams) -> PermissionResult {
	let options = &params.options;

	// Prefer reject_once
	if let Some(opt) = options.iter().find(|o| o.kind == "reject_once") {
		return selected(&opt.option_id);
	}

	// Fallback to reject_always
	if let Some(opt) = options.iter().find(|o| o.kind == "reject_always") {
		return selected(&opt.option_id);
	}

	// No reject options — cancelled
	cancelled()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn selected(option_id: &str) -> PermissionResult {
	PermissionResult {
		outcome: PermissionOutcome {
			outcome: "selected".into(),
			option_id: Some(option_id.to_string()),
		},
	}
}

fn cancelled() -> PermissionResult {
	PermissionResult {
		outcome: PermissionOutcome {
			outcome: "cancelled".into(),
			option_id: None,
		},
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::protocol::PermissionOption;

	/// Build a PermissionRequestParams with the given options.
	fn make_params(options: Vec<PermissionOption>) -> PermissionRequestParams {
		PermissionRequestParams {
			title: Some("Run bash command".into()),
			description: Some("rm -rf /tmp/test".into()),
			tool_call: None,
			options,
		}
	}

	fn opt(option_id: &str, kind: &str) -> PermissionOption {
		PermissionOption {
			option_id: option_id.into(),
			kind: kind.into(),
			name: None,
			title: None,
			description: None,
		}
	}

	// -----------------------------------------------------------------------
	// AutoApprove
	// -----------------------------------------------------------------------

	#[test]
	fn auto_approve_selects_allow_always_when_available() {
		let params = make_params(vec![
			opt("reject", "reject_once"),
			opt("once", "allow_once"),
			opt("always", "allow_always"),
		]);
		let result = resolve_permission(PermissionPolicy::AutoApprove, &params).unwrap();
		assert_eq!(result.outcome.outcome, "selected");
		assert_eq!(result.outcome.option_id.as_deref(), Some("always"));
	}

	#[test]
	fn auto_approve_falls_back_to_allow_once() {
		let params = make_params(vec![
			opt("reject", "reject_once"),
			opt("once", "allow_once"),
		]);
		let result = resolve_permission(PermissionPolicy::AutoApprove, &params).unwrap();
		assert_eq!(result.outcome.outcome, "selected");
		assert_eq!(result.outcome.option_id.as_deref(), Some("once"));
	}

	#[test]
	fn auto_approve_falls_back_to_first_option() {
		let params = make_params(vec![
			opt("reject", "reject_once"),
			opt("reject_all", "reject_always"),
		]);
		let result = resolve_permission(PermissionPolicy::AutoApprove, &params).unwrap();
		assert_eq!(result.outcome.outcome, "selected");
		assert_eq!(result.outcome.option_id.as_deref(), Some("reject"));
	}

	#[test]
	fn auto_approve_cancelled_when_no_options() {
		let params = make_params(vec![]);
		let result = resolve_permission(PermissionPolicy::AutoApprove, &params).unwrap();
		assert_eq!(result.outcome.outcome, "cancelled");
		assert!(result.outcome.option_id.is_none());
	}

	// -----------------------------------------------------------------------
	// Deny
	// -----------------------------------------------------------------------

	#[test]
	fn deny_selects_reject_once() {
		let params = make_params(vec![
			opt("once", "allow_once"),
			opt("reject", "reject_once"),
			opt("reject_all", "reject_always"),
		]);
		let result = resolve_permission(PermissionPolicy::Deny, &params).unwrap();
		assert_eq!(result.outcome.outcome, "selected");
		assert_eq!(result.outcome.option_id.as_deref(), Some("reject"));
	}

	#[test]
	fn deny_falls_back_to_reject_always() {
		let params = make_params(vec![
			opt("once", "allow_once"),
			opt("reject_all", "reject_always"),
		]);
		let result = resolve_permission(PermissionPolicy::Deny, &params).unwrap();
		assert_eq!(result.outcome.outcome, "selected");
		assert_eq!(result.outcome.option_id.as_deref(), Some("reject_all"));
	}

	#[test]
	fn deny_returns_cancelled_when_no_reject_options() {
		let params = make_params(vec![
			opt("once", "allow_once"),
			opt("always", "allow_always"),
		]);
		let result = resolve_permission(PermissionPolicy::Deny, &params).unwrap();
		assert_eq!(result.outcome.outcome, "cancelled");
		assert!(result.outcome.option_id.is_none());
	}

	// -----------------------------------------------------------------------
	// Prompt
	// -----------------------------------------------------------------------

	#[test]
	fn prompt_returns_none() {
		let params = make_params(vec![
			opt("once", "allow_once"),
			opt("always", "allow_always"),
		]);
		let result = resolve_permission(PermissionPolicy::Prompt, &params);
		assert!(result.is_none());
	}
}
