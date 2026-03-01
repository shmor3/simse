//! @-mention parsing and autocomplete.

/// Extract the @partial from the end of an input string.
pub fn extract_at_query(input: &str) -> Option<&str> {
	let at_pos = input.rfind('@')?;
	let after = &input[at_pos + 1..];
	// Must not contain whitespace
	if after.contains(char::is_whitespace) {
		return None;
	}
	Some(after)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extract_at_query_basic() {
		assert_eq!(extract_at_query("hello @src/m"), Some("src/m"));
	}

	#[test]
	fn extract_at_query_empty_after_at() {
		assert_eq!(extract_at_query("hello @"), Some(""));
	}

	#[test]
	fn extract_at_query_no_at() {
		assert_eq!(extract_at_query("hello world"), None);
	}

	#[test]
	fn extract_at_query_with_space_after() {
		assert_eq!(extract_at_query("hello @foo bar"), None);
	}
}
