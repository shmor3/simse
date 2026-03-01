use crate::error::VfsError;

// ── Constants ───────────────────────────────────────────────────────────────

pub const VFS_SCHEME: &str = "vfs://";
pub const VFS_ROOT: &str = "vfs:///";

// ── Limits ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VfsLimits {
    pub max_file_size: u64,
    pub max_total_size: u64,
    pub max_path_depth: usize,
    pub max_name_length: usize,
    pub max_node_count: usize,
    pub max_path_length: usize,
    pub max_diff_lines: usize,
}

impl Default for VfsLimits {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,      // 10 MB
            max_total_size: 100 * 1024 * 1024,     // 100 MB
            max_path_depth: 32,
            max_name_length: 255,
            max_node_count: 10_000,
            max_path_length: 1024,
            max_diff_lines: 10_000,
        }
    }
}

// ── Path functions ──────────────────────────────────────────────────────────

/// Strip the `vfs://` scheme prefix and return the local part.
/// Returns an error if the path does not start with `vfs://`.
pub fn to_local_path(vfs_path: &str) -> Result<&str, VfsError> {
    if let Some(rest) = vfs_path.strip_prefix(VFS_SCHEME) {
        if rest.is_empty() {
            Ok("/")
        } else {
            Ok(rest)
        }
    } else {
        Err(VfsError::InvalidPath(format!(
            "Path must start with {}: {}",
            VFS_SCHEME, vfs_path
        )))
    }
}

/// Normalize a VFS path: strip scheme, replace backslash, resolve `.` and `..`,
/// rebuild with the `vfs://` prefix.
pub fn normalize_path(input: &str) -> Result<String, VfsError> {
    if !input.starts_with(VFS_SCHEME) {
        return Err(VfsError::InvalidPath(format!(
            "Path must start with {}: {}",
            VFS_SCHEME, input
        )));
    }

    let mut p = input[VFS_SCHEME.len()..].replace('\\', "/");
    if !p.starts_with('/') {
        p = format!("/{}", p);
    }

    let segments: Vec<&str> = p.split('/').collect();
    let mut resolved: Vec<&str> = Vec::new();

    for seg in &segments {
        if seg.is_empty() || *seg == "." {
            continue;
        }
        if *seg == ".." {
            resolved.pop();
        } else {
            resolved.push(seg);
        }
    }

    if resolved.is_empty() {
        Ok(VFS_ROOT.to_string())
    } else {
        Ok(format!("{}/{}", VFS_SCHEME, resolved.join("/")))
    }
}

/// Return the parent path of a normalized VFS path.
/// Returns `None` for the root path.
pub fn parent_path(normalized_path: &str) -> Option<String> {
    if normalized_path == VFS_ROOT {
        return None;
    }
    let local_part = &normalized_path[VFS_SCHEME.len()..];
    let last_slash = local_part.rfind('/');
    match last_slash {
        Some(0) => Some(VFS_ROOT.to_string()),
        Some(pos) => Some(format!("{}{}", VFS_SCHEME, &local_part[..pos])),
        None => Some(VFS_ROOT.to_string()),
    }
}

/// Return the base name (final segment) of a normalized VFS path.
/// Returns an empty string for the root path.
pub fn base_name(normalized_path: &str) -> &str {
    if normalized_path == VFS_ROOT {
        return "";
    }
    let local_part = &normalized_path[VFS_SCHEME.len()..];
    match local_part.rfind('/') {
        Some(pos) => &local_part[pos + 1..],
        None => local_part,
    }
}

/// Return all ancestor paths of a normalized VFS path (from root to parent).
/// Does not include the path itself.
pub fn ancestor_paths(normalized_path: &str) -> Vec<String> {
    let mut result = vec![VFS_ROOT.to_string()];
    let local_part = &normalized_path[VFS_SCHEME.len()..];
    let segments: Vec<&str> = local_part.split('/').filter(|s| !s.is_empty()).collect();
    for i in 0..segments.len().saturating_sub(1) {
        result.push(format!(
            "{}/{}",
            VFS_SCHEME,
            segments[..=i].join("/")
        ));
    }
    result
}

/// Return the depth of a normalized VFS path.
/// Root has depth 0, `/foo` has depth 1, `/foo/bar` has depth 2, etc.
pub fn path_depth(normalized_path: &str) -> usize {
    if normalized_path == VFS_ROOT {
        return 0;
    }
    let local_part = &normalized_path[VFS_SCHEME.len()..];
    local_part.split('/').filter(|s| !s.is_empty()).count()
}

/// Check if a path segment contains forbidden characters (control chars or backslash).
fn has_forbidden_chars(segment: &str) -> bool {
    for b in segment.bytes() {
        if b <= 0x1f || b == b'\\' {
            return true;
        }
    }
    false
}

/// Validate a single path segment. Returns `Some(error_message)` if invalid.
pub fn validate_segment(segment: &str, max_name_length: usize) -> Option<String> {
    if segment.is_empty() {
        return Some("Path segment cannot be empty".to_string());
    }
    if segment.len() > max_name_length {
        return Some(format!(
            "Path segment exceeds max name length ({})",
            max_name_length
        ));
    }
    if has_forbidden_chars(segment) {
        return Some("Path segment contains forbidden characters".to_string());
    }
    None
}

/// Validate a normalized VFS path against limits. Returns `Some(error_message)` if invalid.
pub fn validate_path(normalized_path: &str, limits: &VfsLimits) -> Option<String> {
    let local_part = &normalized_path[VFS_SCHEME.len()..];
    if local_part.len() > limits.max_path_length {
        return Some(format!(
            "Path exceeds max length ({})",
            limits.max_path_length
        ));
    }
    let depth = path_depth(normalized_path);
    if depth > limits.max_path_depth {
        return Some(format!(
            "Path exceeds max depth ({})",
            limits.max_path_depth
        ));
    }
    let segments: Vec<&str> = local_part.split('/').filter(|s| !s.is_empty()).collect();
    for seg in &segments {
        if let Some(err) = validate_segment(seg, limits.max_name_length) {
            return Some(err);
        }
    }
    None
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_path ──────────────────────────────────────────────────

    #[test]
    fn normalize_basic_path() {
        assert_eq!(
            normalize_path("vfs:///foo/bar").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_root() {
        assert_eq!(normalize_path("vfs:///").unwrap(), "vfs:///");
    }

    #[test]
    fn normalize_dot_segments() {
        assert_eq!(
            normalize_path("vfs:///foo/./bar").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_dotdot_segments() {
        assert_eq!(
            normalize_path("vfs:///foo/bar/../baz").unwrap(),
            "vfs:///foo/baz"
        );
    }

    #[test]
    fn normalize_dotdot_past_root() {
        assert_eq!(
            normalize_path("vfs:///foo/../../bar").unwrap(),
            "vfs:///bar"
        );
    }

    #[test]
    fn normalize_trailing_slash() {
        assert_eq!(
            normalize_path("vfs:///foo/bar/").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_double_slashes() {
        assert_eq!(
            normalize_path("vfs:///foo//bar").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_backslashes() {
        assert_eq!(
            normalize_path("vfs:///foo\\bar").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_without_leading_slash() {
        assert_eq!(
            normalize_path("vfs://foo/bar").unwrap(),
            "vfs:///foo/bar"
        );
    }

    #[test]
    fn normalize_rejects_non_vfs_path() {
        assert!(normalize_path("/foo/bar").is_err());
    }

    // ── to_local_path ───────────────────────────────────────────────────

    #[test]
    fn to_local_path_basic() {
        assert_eq!(to_local_path("vfs:///foo/bar").unwrap(), "/foo/bar");
    }

    #[test]
    fn to_local_path_root() {
        assert_eq!(to_local_path("vfs:///").unwrap(), "/");
    }

    #[test]
    fn to_local_path_scheme_only() {
        assert_eq!(to_local_path("vfs://").unwrap(), "/");
    }

    #[test]
    fn to_local_path_rejects_non_vfs() {
        assert!(to_local_path("/foo/bar").is_err());
    }

    // ── parent_path ─────────────────────────────────────────────────────

    #[test]
    fn parent_of_root_is_none() {
        assert_eq!(parent_path(VFS_ROOT), None);
    }

    #[test]
    fn parent_of_top_level_file() {
        assert_eq!(
            parent_path("vfs:///foo"),
            Some("vfs:///".to_string())
        );
    }

    #[test]
    fn parent_of_nested_file() {
        assert_eq!(
            parent_path("vfs:///foo/bar/baz"),
            Some("vfs:///foo/bar".to_string())
        );
    }

    // ── base_name ───────────────────────────────────────────────────────

    #[test]
    fn base_name_root() {
        assert_eq!(base_name(VFS_ROOT), "");
    }

    #[test]
    fn base_name_file() {
        assert_eq!(base_name("vfs:///foo/bar.txt"), "bar.txt");
    }

    #[test]
    fn base_name_top_level() {
        assert_eq!(base_name("vfs:///foo"), "foo");
    }

    // ── path_depth ──────────────────────────────────────────────────────

    #[test]
    fn depth_root() {
        assert_eq!(path_depth(VFS_ROOT), 0);
    }

    #[test]
    fn depth_one() {
        assert_eq!(path_depth("vfs:///foo"), 1);
    }

    #[test]
    fn depth_three() {
        assert_eq!(path_depth("vfs:///a/b/c"), 3);
    }

    // ── ancestor_paths ──────────────────────────────────────────────────

    #[test]
    fn ancestors_of_root() {
        let a = ancestor_paths(VFS_ROOT);
        assert_eq!(a, vec!["vfs:///"]);
    }

    #[test]
    fn ancestors_of_top_level() {
        let a = ancestor_paths("vfs:///foo");
        assert_eq!(a, vec!["vfs:///"]);
    }

    #[test]
    fn ancestors_of_nested() {
        let a = ancestor_paths("vfs:///a/b/c");
        assert_eq!(
            a,
            vec!["vfs:///", "vfs:///a", "vfs:///a/b"]
        );
    }

    // ── validate_segment ────────────────────────────────────────────────

    #[test]
    fn validate_segment_ok() {
        assert_eq!(validate_segment("hello", 255), None);
    }

    #[test]
    fn validate_segment_empty() {
        assert!(validate_segment("", 255).is_some());
    }

    #[test]
    fn validate_segment_too_long() {
        let long = "a".repeat(256);
        assert!(validate_segment(&long, 255).is_some());
    }

    #[test]
    fn validate_segment_forbidden_chars() {
        assert!(validate_segment("foo\x01bar", 255).is_some());
        assert!(validate_segment("foo\\bar", 255).is_some());
    }

    // ── validate_path ───────────────────────────────────────────────────

    #[test]
    fn validate_path_ok() {
        let limits = VfsLimits::default();
        assert_eq!(validate_path("vfs:///foo/bar", &limits), None);
    }

    #[test]
    fn validate_path_too_deep() {
        let limits = VfsLimits {
            max_path_depth: 2,
            ..VfsLimits::default()
        };
        assert!(validate_path("vfs:///a/b/c", &limits).is_some());
    }

    #[test]
    fn validate_path_too_long() {
        let limits = VfsLimits {
            max_path_length: 10,
            ..VfsLimits::default()
        };
        assert!(validate_path("vfs:///a-very-long-path", &limits).is_some());
    }

    #[test]
    fn validate_path_bad_segment() {
        let limits = VfsLimits::default();
        assert!(validate_path("vfs:///foo\x00/bar", &limits).is_some());
    }
}
