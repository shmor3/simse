//! File content validators for the VFS.
//!
//! Ports `src/ai/vfs/validators.ts` to Rust. Provides:
//! - `VfsValidator` trait for pluggable content validation
//! - Built-in validators: JSON syntax, trailing whitespace, mixed indentation,
//!   empty file, mixed line endings, missing trailing newline
//! - `validate_snapshot()` to validate an entire VFS snapshot

use super::vfs::SnapshotData;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity of a validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationSeverity {
	Error,
	Warning,
}

/// A single validation issue found in a file.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
	/// VFS path of the file.
	pub path: String,
	/// Severity level.
	pub severity: ValidationSeverity,
	/// Machine-readable error code (e.g., "json_syntax", "trailing_whitespace").
	pub code: String,
	/// Human-readable description of the issue.
	pub message: String,
	/// 1-indexed line number where the issue occurs (if applicable).
	pub line: Option<usize>,
}

/// Aggregated validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
	/// All issues found.
	pub issues: Vec<ValidationIssue>,
	/// Number of errors.
	pub errors: usize,
	/// Number of warnings.
	pub warnings: usize,
	/// True if no errors were found (warnings are allowed).
	pub passed: bool,
}

// ---------------------------------------------------------------------------
// VfsValidator trait
// ---------------------------------------------------------------------------

/// Trait for file content validators.
///
/// Implementations inspect file content and return zero or more issues.
/// The `extensions()` method can restrict the validator to specific file types.
pub trait VfsValidator: Send + Sync {
	/// Human-readable name of the validator.
	fn name(&self) -> &str;

	/// File extensions this validator applies to (without dot). If `None`,
	/// the validator applies to all text files.
	fn extensions(&self) -> Option<&[&str]>;

	/// Validate a file's content and return any issues found.
	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue>;
}

// ---------------------------------------------------------------------------
// Built-in validators
// ---------------------------------------------------------------------------

/// Validates JSON syntax — reports parsing errors.
pub struct JsonSyntaxValidator;

impl VfsValidator for JsonSyntaxValidator {
	fn name(&self) -> &str {
		"json_syntax"
	}

	fn extensions(&self) -> Option<&[&str]> {
		Some(&["json"])
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		if text.trim().is_empty() {
			return vec![ValidationIssue {
				path: path.to_string(),
				severity: ValidationSeverity::Error,
				code: "json_syntax".to_string(),
				message: "Empty JSON file".to_string(),
				line: None,
			}];
		}

		match serde_json::from_str::<serde_json::Value>(text) {
			Ok(_) => Vec::new(),
			Err(e) => {
				let line = if e.line() > 0 { Some(e.line()) } else { None };
				vec![ValidationIssue {
					path: path.to_string(),
					severity: ValidationSeverity::Error,
					code: "json_syntax".to_string(),
					message: format!("JSON parse error: {}", e),
					line,
				}]
			}
		}
	}
}

/// Warns on trailing whitespace in lines.
pub struct TrailingWhitespaceValidator;

impl VfsValidator for TrailingWhitespaceValidator {
	fn name(&self) -> &str {
		"trailing_whitespace"
	}

	fn extensions(&self) -> Option<&[&str]> {
		None
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		let mut issues = Vec::new();
		for (i, line) in text.lines().enumerate() {
			if line != line.trim_end() {
				issues.push(ValidationIssue {
					path: path.to_string(),
					severity: ValidationSeverity::Warning,
					code: "trailing_whitespace".to_string(),
					message: "Trailing whitespace".to_string(),
					line: Some(i + 1),
				});
			}
		}
		issues
	}
}

/// Errors on mixed tabs and spaces for indentation.
pub struct MixedIndentationValidator;

impl VfsValidator for MixedIndentationValidator {
	fn name(&self) -> &str {
		"mixed_indentation"
	}

	fn extensions(&self) -> Option<&[&str]> {
		None
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		let mut has_tabs = false;
		let mut has_spaces = false;

		for line in text.lines() {
			let leading: String = line.chars().take_while(|c| *c == '\t' || *c == ' ').collect();
			if leading.is_empty() {
				continue;
			}
			if leading.contains('\t') {
				has_tabs = true;
			}
			if leading.contains(' ') {
				has_spaces = true;
			}
		}

		if has_tabs && has_spaces {
			vec![ValidationIssue {
				path: path.to_string(),
				severity: ValidationSeverity::Error,
				code: "mixed_indentation".to_string(),
				message: "File uses mixed tabs and spaces for indentation".to_string(),
				line: None,
			}]
		} else {
			Vec::new()
		}
	}
}

/// Warns on empty files (zero bytes or whitespace only).
pub struct EmptyFileValidator;

impl VfsValidator for EmptyFileValidator {
	fn name(&self) -> &str {
		"empty_file"
	}

	fn extensions(&self) -> Option<&[&str]> {
		None
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		if text.trim().is_empty() {
			vec![ValidationIssue {
				path: path.to_string(),
				severity: ValidationSeverity::Warning,
				code: "empty_file".to_string(),
				message: "File is empty or whitespace-only".to_string(),
				line: None,
			}]
		} else {
			Vec::new()
		}
	}
}

/// Warns when a file contains a mix of CRLF and LF line endings.
pub struct MixedLineEndingsValidator;

impl VfsValidator for MixedLineEndingsValidator {
	fn name(&self) -> &str {
		"mixed_line_endings"
	}

	fn extensions(&self) -> Option<&[&str]> {
		None
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		let has_crlf = text.contains("\r\n");
		// Check for lone LF (not preceded by CR)
		let has_lone_lf = text
			.as_bytes()
			.windows(2)
			.any(|w| w[0] != b'\r' && w[1] == b'\n')
			|| text.as_bytes().first() == Some(&b'\n');

		if has_crlf && has_lone_lf {
			vec![ValidationIssue {
				path: path.to_string(),
				severity: ValidationSeverity::Warning,
				code: "mixed_line_endings".to_string(),
				message: "File contains mixed CRLF and LF line endings".to_string(),
				line: None,
			}]
		} else {
			Vec::new()
		}
	}
}

/// Warns when a file does not end with a trailing newline.
pub struct MissingTrailingNewlineValidator;

impl VfsValidator for MissingTrailingNewlineValidator {
	fn name(&self) -> &str {
		"missing_trailing_newline"
	}

	fn extensions(&self) -> Option<&[&str]> {
		None
	}

	fn validate(&self, path: &str, text: &str) -> Vec<ValidationIssue> {
		if text.is_empty() {
			return Vec::new(); // Empty files are handled by EmptyFileValidator
		}
		if !text.ends_with('\n') {
			vec![ValidationIssue {
				path: path.to_string(),
				severity: ValidationSeverity::Warning,
				code: "missing_trailing_newline".to_string(),
				message: "File does not end with a trailing newline".to_string(),
				line: None,
			}]
		} else {
			Vec::new()
		}
	}
}

// ---------------------------------------------------------------------------
// Factory & snapshot validation
// ---------------------------------------------------------------------------

/// Return all built-in validators.
pub fn default_validators() -> Vec<Box<dyn VfsValidator>> {
	vec![
		Box::new(JsonSyntaxValidator),
		Box::new(TrailingWhitespaceValidator),
		Box::new(MixedIndentationValidator),
		Box::new(EmptyFileValidator),
		Box::new(MixedLineEndingsValidator),
		Box::new(MissingTrailingNewlineValidator),
	]
}

/// Validate all text files in a snapshot using the provided validators.
///
/// Binary files are skipped. For each text file, validators whose `extensions()`
/// match the file extension (or `None` for all-file validators) are run.
pub fn validate_snapshot(
	snapshot: &SnapshotData,
	validators: &[Box<dyn VfsValidator>],
) -> ValidationResult {
	let mut issues: Vec<ValidationIssue> = Vec::new();

	for file in &snapshot.files {
		// Skip binary files
		if file.content_type == "binary" {
			continue;
		}

		let text = file.text.as_deref().unwrap_or("");
		let ext = std::path::Path::new(&file.path)
			.extension()
			.and_then(|e| e.to_str())
			.map(|e| e.to_lowercase());

		for validator in validators {
			let should_run = match validator.extensions() {
				None => true,
				Some(exts) => {
					if let Some(ref file_ext) = ext {
						exts.iter().any(|e| e.eq_ignore_ascii_case(file_ext))
					} else {
						false
					}
				}
			};

			if should_run {
				let mut file_issues = validator.validate(&file.path, text);
				issues.append(&mut file_issues);
			}
		}
	}

	let errors = issues
		.iter()
		.filter(|i| i.severity == ValidationSeverity::Error)
		.count();
	let warnings = issues
		.iter()
		.filter(|i| i.severity == ValidationSeverity::Warning)
		.count();

	ValidationResult {
		issues,
		errors,
		warnings,
		passed: errors == 0,
	}
}
