use crate::path::{normalize_path, to_local_path, VFS_SCHEME};

// ── Brace expansion ─────────────────────────────────────────────────────────

/// Expand `{a,b}` brace patterns in a glob string, handling nested braces.
pub fn expand_braces(pattern: &str) -> Vec<String> {
    let brace_start = match pattern.find('{') {
        Some(pos) => pos,
        None => return vec![pattern.to_string()],
    };

    // Find matching closing brace (handle nesting)
    let mut depth = 0i32;
    let mut brace_end: Option<usize> = None;
    for (i, ch) in pattern[brace_start..].char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                brace_end = Some(brace_start + i);
                break;
            }
        }
    }

    let brace_end = match brace_end {
        Some(pos) => pos,
        None => return vec![pattern.to_string()], // unmatched brace, treat as literal
    };

    let prefix = &pattern[..brace_start];
    let suffix = &pattern[brace_end + 1..];
    let alternatives = &pattern[brace_start + 1..brace_end];

    // Split alternatives by comma (top-level only, respecting nested braces)
    let alts = split_brace_alternatives(alternatives);

    let mut results = Vec::new();
    for alt in &alts {
        let combined = format!("{}{}{}", prefix, alt.trim(), suffix);
        // Recursively expand in case of nested braces or suffix braces
        for expanded in expand_braces(&combined) {
            results.push(expanded);
        }
    }

    results
}

/// Split a brace-interior string by commas, respecting nested braces.
fn split_brace_alternatives(s: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                results.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    results.push(s[start..].to_string());
    results
}

// ── Segment matching ────────────────────────────────────────────────────────

/// Match a single path segment against a pattern with `*` and `?` wildcards.
pub fn match_segment(segment: &str, pattern: &str) -> bool {
    let seg_bytes = segment.as_bytes();
    let pat_bytes = pattern.as_bytes();
    let mut si: usize = 0;
    let mut pi: usize = 0;
    let mut star_si: Option<usize> = None;
    let mut star_pi: Option<usize> = None;

    while si < seg_bytes.len() {
        if pi < pat_bytes.len() && pat_bytes[pi] == b'?' {
            si += 1;
            pi += 1;
        } else if pi < pat_bytes.len() && pat_bytes[pi] == b'*' {
            star_pi = Some(pi);
            star_si = Some(si);
            pi += 1;
        } else if pi < pat_bytes.len() && seg_bytes[si] == pat_bytes[pi] {
            si += 1;
            pi += 1;
        } else if let (Some(sp), Some(ss)) = (star_pi, star_si) {
            pi = sp + 1;
            let next_ss = ss + 1;
            star_si = Some(next_ss);
            si = next_ss;
        } else {
            return false;
        }
    }

    while pi < pat_bytes.len() && pat_bytes[pi] == b'*' {
        pi += 1;
    }

    pi == pat_bytes.len()
}

// ── Multi-segment matching ──────────────────────────────────────────────────

/// Match path parts against pattern parts, handling `**` globstar.
pub fn match_parts(
    path_parts: &[&str],
    mut pi: usize,
    pat_parts: &[&str],
    mut gi: usize,
) -> bool {
    while pi < path_parts.len() && gi < pat_parts.len() {
        if pat_parts[gi] == "**" {
            // ** matches zero or more path segments
            if gi == pat_parts.len() - 1 {
                return true;
            }
            for skip in pi..=path_parts.len() {
                if match_parts(path_parts, skip, pat_parts, gi + 1) {
                    return true;
                }
            }
            return false;
        }
        if !match_segment(path_parts[pi], pat_parts[gi]) {
            return false;
        }
        pi += 1;
        gi += 1;
    }

    // Consume trailing ** patterns
    while gi < pat_parts.len() && pat_parts[gi] == "**" {
        gi += 1;
    }

    pi == path_parts.len() && gi == pat_parts.len()
}

// ── Top-level glob matching ─────────────────────────────────────────────────

/// Match a VFS file path against a glob pattern.
/// Both the file path and pattern should start with `vfs://`.
pub fn match_glob(file_path: &str, pattern: &str) -> bool {
    let expanded = expand_braces(pattern);

    let local = match to_local_path(file_path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let path_parts: Vec<&str> = local.split('/').filter(|s| !s.is_empty()).collect();

    for exp in &expanded {
        // Ensure the expanded pattern has the vfs:// scheme
        let pat_to_normalize = if exp.starts_with(VFS_SCHEME) {
            exp.clone()
        } else {
            format!("{}{}", VFS_SCHEME, exp)
        };

        let normalized_pattern = match normalize_path(&pat_to_normalize) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let pat_local = match to_local_path(&normalized_pattern) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let pattern_parts: Vec<&str> = pat_local.split('/').filter(|s| !s.is_empty()).collect();

        if match_parts(&path_parts, 0, &pattern_parts, 0) {
            return true;
        }
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── expand_braces ───────────────────────────────────────────────────

    #[test]
    fn brace_expansion_simple() {
        let mut result = expand_braces("vfs:///{a,b}.txt");
        result.sort();
        assert_eq!(result, vec!["vfs:///a.txt", "vfs:///b.txt"]);
    }

    #[test]
    fn brace_expansion_three_alternatives() {
        let mut result = expand_braces("{x,y,z}");
        result.sort();
        assert_eq!(result, vec!["x", "y", "z"]);
    }

    #[test]
    fn brace_expansion_no_braces() {
        let result = expand_braces("foo/bar");
        assert_eq!(result, vec!["foo/bar"]);
    }

    #[test]
    fn brace_expansion_unmatched() {
        let result = expand_braces("foo{bar");
        assert_eq!(result, vec!["foo{bar"]);
    }

    #[test]
    fn brace_expansion_nested() {
        let result = expand_braces("{a,{b,c}}");
        // Should produce a, b, c
        assert!(result.contains(&"a".to_string()));
        assert!(result.contains(&"b".to_string()));
        assert!(result.contains(&"c".to_string()));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn brace_expansion_with_prefix_suffix() {
        let mut result = expand_braces("pre-{a,b}-suf");
        result.sort();
        assert_eq!(result, vec!["pre-a-suf", "pre-b-suf"]);
    }

    // ── match_segment ───────────────────────────────────────────────────

    #[test]
    fn segment_exact_match() {
        assert!(match_segment("hello", "hello"));
    }

    #[test]
    fn segment_no_match() {
        assert!(!match_segment("hello", "world"));
    }

    #[test]
    fn segment_star_wildcard() {
        assert!(match_segment("hello.txt", "*.txt"));
        assert!(match_segment("hello.txt", "hello.*"));
        assert!(match_segment("anything", "*"));
    }

    #[test]
    fn segment_question_wildcard() {
        assert!(match_segment("a", "?"));
        assert!(match_segment("ab", "?b"));
        assert!(!match_segment("ab", "?"));
    }

    #[test]
    fn segment_complex_pattern() {
        assert!(match_segment("test.spec.ts", "*.spec.*"));
        assert!(match_segment("test.spec.ts", "test.*.*"));
    }

    #[test]
    fn segment_empty_pattern() {
        assert!(match_segment("", ""));
        assert!(!match_segment("a", ""));
        assert!(match_segment("", "*"));
    }

    // ── match_glob ──────────────────────────────────────────────────────

    #[test]
    fn glob_exact_path() {
        assert!(match_glob("vfs:///foo/bar.txt", "vfs:///foo/bar.txt"));
    }

    #[test]
    fn glob_star_in_filename() {
        assert!(match_glob("vfs:///foo/bar.txt", "vfs:///foo/*.txt"));
        assert!(!match_glob("vfs:///foo/bar.js", "vfs:///foo/*.txt"));
    }

    #[test]
    fn glob_globstar() {
        assert!(match_glob("vfs:///a/b/c/d.txt", "vfs:///**/*.txt"));
        assert!(match_glob("vfs:///d.txt", "vfs:///**/*.txt"));
    }

    #[test]
    fn glob_globstar_at_end() {
        assert!(match_glob("vfs:///a/b/c", "vfs:///a/**"));
        assert!(match_glob("vfs:///a", "vfs:///a/**"));
    }

    #[test]
    fn glob_globstar_middle() {
        assert!(match_glob("vfs:///src/foo/bar/test.ts", "vfs:///src/**/test.ts"));
        assert!(match_glob("vfs:///src/test.ts", "vfs:///src/**/test.ts"));
    }

    #[test]
    fn glob_no_match() {
        assert!(!match_glob("vfs:///foo/bar.txt", "vfs:///baz/*.txt"));
    }

    #[test]
    fn glob_with_braces() {
        assert!(match_glob(
            "vfs:///src/main.ts",
            "vfs:///src/*.{ts,js}"
        ));
        assert!(match_glob(
            "vfs:///src/main.js",
            "vfs:///src/*.{ts,js}"
        ));
        assert!(!match_glob(
            "vfs:///src/main.rs",
            "vfs:///src/*.{ts,js}"
        ));
    }

    #[test]
    fn glob_question_mark() {
        assert!(match_glob("vfs:///foo/a.txt", "vfs:///foo/?.txt"));
        assert!(!match_glob("vfs:///foo/ab.txt", "vfs:///foo/?.txt"));
    }

    #[test]
    fn glob_root_match() {
        assert!(match_glob("vfs:///file.txt", "vfs:///*.txt"));
    }

    #[test]
    fn glob_double_star_zero_segments() {
        // ** can match zero segments
        assert!(match_glob("vfs:///src/file.ts", "vfs:///src/**/file.ts"));
    }
}
