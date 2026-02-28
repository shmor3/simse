/// Myers diff algorithm — computes shortest edit script between two line arrays,
/// then groups changes into hunks with configurable context lines.

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineType {
    Add,
    Remove,
    Equal,
}

impl DiffLineType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Equal => "equal",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub text: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug)]
pub struct DiffOutput {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
}

/// Compute a unified diff between `old_lines` and `new_lines` using the Myers
/// shortest-edit-script algorithm.
///
/// * `context` — number of unchanged context lines to include around each change.
/// * `max_lines` — if the combined line count exceeds this limit, return `Err`.
pub fn compute_diff(
    old_lines: &[&str],
    new_lines: &[&str],
    context: usize,
    max_lines: u32,
) -> Result<DiffOutput, String> {
    // 1. Check total lines against max_lines limit
    let total = old_lines.len() + new_lines.len();
    if total as u64 > max_lines as u64 {
        return Err(format!(
            "Diff input too large: {} + {} = {} lines exceeds limit ({})",
            old_lines.len(),
            new_lines.len(),
            total,
            max_lines,
        ));
    }

    // 2. Handle trivial cases
    if old_lines.is_empty() && new_lines.is_empty() {
        return Ok(DiffOutput {
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
        });
    }

    if old_lines == new_lines {
        return Ok(DiffOutput {
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
        });
    }

    // 3. Compute raw diff lines via Myers algorithm
    let raw_lines = compute_lcs(old_lines, new_lines);

    // 4. Count additions and deletions
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for line in &raw_lines {
        match line.line_type {
            DiffLineType::Add => additions += 1,
            DiffLineType::Remove => deletions += 1,
            DiffLineType::Equal => {}
        }
    }

    // 5. Build hunks with context lines
    let hunks = build_hunks(&raw_lines, context);

    Ok(DiffOutput {
        hunks,
        additions,
        deletions,
    })
}

/// Myers shortest-edit-script algorithm. Returns a flat list of diff lines
/// (add / remove / equal) with 1-based line numbers.
fn compute_lcs(old_lines: &[&str], new_lines: &[&str]) -> Vec<DiffLine> {
    let n = old_lines.len();
    let m = new_lines.len();
    let max = n + m;

    if max == 0 {
        return Vec::new();
    }

    // Trivial: empty old → all additions
    if n == 0 {
        return new_lines
            .iter()
            .enumerate()
            .map(|(i, text)| DiffLine {
                line_type: DiffLineType::Add,
                text: text.to_string(),
                old_line: None,
                new_line: Some(i + 1),
            })
            .collect();
    }

    // Trivial: empty new → all deletions
    if m == 0 {
        return old_lines
            .iter()
            .enumerate()
            .map(|(i, text)| DiffLine {
                line_type: DiffLineType::Remove,
                text: text.to_string(),
                old_line: Some(i + 1),
                new_line: None,
            })
            .collect();
    }

    // V array indexed by diagonal k (range -max..max), offset so index 0 maps to k=-max.
    let v_size = 2 * max + 1;
    let offset = max as isize;
    let mut v: Vec<isize> = vec![-1; v_size];
    v[(offset + 1) as usize] = 0;

    // Traces: snapshots of V at each edit distance d, for backtracking.
    let mut traces: Vec<Vec<isize>> = Vec::new();

    let mut found = false;
    for d in 0..=max as isize {
        traces.push(v.clone());
        let mut k = -d;
        while k <= d {
            let idx_k = (offset + k) as usize;

            let x: isize;
            if k == -d
                || (k != d
                    && v[(offset + k - 1) as usize] < v[(offset + k + 1) as usize])
            {
                // Move down (insertion from new)
                x = v[(offset + k + 1) as usize];
            } else {
                // Move right (deletion from old)
                x = v[(offset + k - 1) as usize] + 1;
            }

            let mut cx = x;
            let mut cy = cx - k;

            // Extend along diagonal (equal lines)
            while cx < n as isize
                && cy < m as isize
                && old_lines[cx as usize] == new_lines[cy as usize]
            {
                cx += 1;
                cy += 1;
            }

            v[idx_k] = cx;

            if cx >= n as isize && cy >= m as isize {
                found = true;
                break;
            }

            k += 2;
        }
        if found {
            break;
        }
    }

    // Backtrack through traces to build the edit script
    let mut result: Vec<DiffLine> = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;

    for d in (0..traces.len()).rev() {
        let v_prev = &traces[d];
        let k = x - y;
        let d_i = d as isize;

        let prev_k: isize;
        if k == -d_i
            || (k != d_i
                && v_prev[(offset + k - 1) as usize] < v_prev[(offset + k + 1) as usize])
        {
            prev_k = k + 1;
        } else {
            prev_k = k - 1;
        }

        let prev_x = v_prev[(offset + prev_k) as usize];
        let prev_y = prev_x - prev_k;

        // Walk diagonal (equal lines)
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            result.push(DiffLine {
                line_type: DiffLineType::Equal,
                text: old_lines[x as usize].to_string(),
                old_line: Some(x as usize + 1),
                new_line: Some(y as usize + 1),
            });
        }

        if d > 0 {
            if x == prev_x {
                // Insertion
                y -= 1;
                result.push(DiffLine {
                    line_type: DiffLineType::Add,
                    text: new_lines[y as usize].to_string(),
                    old_line: None,
                    new_line: Some(y as usize + 1),
                });
            } else {
                // Deletion
                x -= 1;
                result.push(DiffLine {
                    line_type: DiffLineType::Remove,
                    text: old_lines[x as usize].to_string(),
                    old_line: Some(x as usize + 1),
                    new_line: None,
                });
            }
        }
    }

    result.reverse();
    result
}

/// Group raw diff lines into hunks with the given number of context lines
/// around each change.
fn build_hunks(lines: &[DiffLine], context_lines: usize) -> Vec<DiffHunk> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Collect indices of changed lines
    let change_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.line_type != DiffLineType::Equal)
        .map(|(i, _)| i)
        .collect();

    if change_indices.is_empty() {
        return Vec::new();
    }

    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut hunk_first_change = change_indices[0];

    for ci in 1..change_indices.len() {
        let prev = change_indices[ci - 1];
        let curr = change_indices[ci];

        // If gap between changes exceeds 2*context, finalize the current hunk
        // and start a new one.
        if curr - prev > context_lines * 2 + 1 {
            let hunk_last_change = prev;
            hunks.push(build_single_hunk(
                lines,
                hunk_first_change,
                hunk_last_change,
                context_lines,
            ));
            hunk_first_change = curr;
        }
    }

    // Final hunk
    hunks.push(build_single_hunk(
        lines,
        hunk_first_change,
        *change_indices.last().unwrap(),
        context_lines,
    ));

    hunks
}

/// Build a single hunk spanning from `first_change` to `last_change` (indices
/// into the raw diff lines), padded with up to `context_lines` of equal context
/// on each side.
fn build_single_hunk(
    lines: &[DiffLine],
    first_change: usize,
    last_change: usize,
    context_lines: usize,
) -> DiffHunk {
    let start = if first_change >= context_lines {
        first_change - context_lines
    } else {
        0
    };
    let end = std::cmp::min(lines.len() - 1, last_change + context_lines);

    let mut hunk_lines: Vec<DiffLine> = Vec::new();
    let mut old_start: usize = 0;
    let mut new_start: usize = 0;
    let mut old_count: usize = 0;
    let mut new_count: usize = 0;
    let mut found_first = false;

    for i in start..=end {
        let line = &lines[i];
        hunk_lines.push(line.clone());

        if !found_first {
            match line.line_type {
                DiffLineType::Equal => {
                    old_start = line.old_line.unwrap_or(1);
                    new_start = line.new_line.unwrap_or(1);
                }
                DiffLineType::Remove => {
                    old_start = line.old_line.unwrap_or(1);
                    new_start = old_start;
                }
                DiffLineType::Add => {
                    new_start = line.new_line.unwrap_or(1);
                    old_start = new_start;
                }
            }
            found_first = true;
        }

        match line.line_type {
            DiffLineType::Equal => {
                old_count += 1;
                new_count += 1;
            }
            DiffLineType::Remove => {
                old_count += 1;
            }
            DiffLineType::Add => {
                new_count += 1;
            }
        }
    }

    DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: hunk_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_no_hunks() {
        let result = compute_diff(&[], &[], 3, 10000).unwrap();
        assert!(result.hunks.is_empty());
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn identical_inputs_no_hunks() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "b", "c"];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        assert!(result.hunks.is_empty());
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn pure_additions() {
        let old: Vec<&str> = vec![];
        let new = vec!["x", "y", "z"];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        assert_eq!(result.additions, 3);
        assert_eq!(result.deletions, 0);
        assert_eq!(result.hunks.len(), 1);
        let hunk = &result.hunks[0];
        assert_eq!(hunk.new_count, 3);
        assert_eq!(hunk.old_count, 0);
        for line in &hunk.lines {
            assert_eq!(line.line_type, DiffLineType::Add);
        }
    }

    #[test]
    fn pure_deletions() {
        let old = vec!["a", "b", "c"];
        let new: Vec<&str> = vec![];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 3);
        assert_eq!(result.hunks.len(), 1);
        let hunk = &result.hunks[0];
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_count, 0);
        for line in &hunk.lines {
            assert_eq!(line.line_type, DiffLineType::Remove);
        }
    }

    #[test]
    fn simple_change_replace_one_line() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "B", "c"];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        assert_eq!(result.additions, 1);
        assert_eq!(result.deletions, 1);
        assert_eq!(result.hunks.len(), 1);

        // The hunk should contain: equal "a", remove "b", add "B", equal "c"
        let types: Vec<&str> = result.hunks[0]
            .lines
            .iter()
            .map(|l| l.line_type.as_str())
            .collect();
        assert!(types.contains(&"remove"));
        assert!(types.contains(&"add"));
    }

    #[test]
    fn mixed_changes() {
        let old = vec!["a", "b", "c", "d", "e"];
        let new = vec!["a", "X", "c", "d", "e", "f"];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        // "b" removed, "X" added, "f" added
        assert_eq!(result.additions, 2);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn context_lines_included() {
        // Create a file with enough lines that context is meaningful.
        // Lines: 0..9, change line 5 only. With context=2, the hunk should
        // include lines 3..7 (indices) = 5 lines around the change.
        let old: Vec<&str> = vec![
            "line0", "line1", "line2", "line3", "line4", "line5", "line6",
            "line7", "line8", "line9",
        ];
        let new: Vec<&str> = vec![
            "line0", "line1", "line2", "line3", "line4", "CHANGED", "line6",
            "line7", "line8", "line9",
        ];
        let result = compute_diff(&old, &new, 2, 10000).unwrap();
        assert_eq!(result.hunks.len(), 1);
        let hunk = &result.hunks[0];

        // With context=2, we expect 2 context lines before the change,
        // the remove+add pair, and 2 context lines after.
        // That's 2 + 1 + 1 + 2 = 6 lines in the hunk.
        assert_eq!(hunk.lines.len(), 6);

        // First two and last two should be "equal"
        assert_eq!(hunk.lines[0].line_type, DiffLineType::Equal);
        assert_eq!(hunk.lines[1].line_type, DiffLineType::Equal);
        assert_eq!(hunk.lines[4].line_type, DiffLineType::Equal);
        assert_eq!(hunk.lines[5].line_type, DiffLineType::Equal);
    }

    #[test]
    fn max_lines_limit_exceeded() {
        let old = vec!["a"; 100];
        let new = vec!["b"; 100];
        let result = compute_diff(&old, &new, 3, 50);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("exceeds limit"));
    }

    #[test]
    fn separate_hunks_with_small_context() {
        // Two changes far apart should produce two separate hunks with small context
        let old: Vec<&str> = vec![
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l",
        ];
        let new: Vec<&str> = vec![
            "A", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "L",
        ];
        let result = compute_diff(&old, &new, 1, 10000).unwrap();
        // With context=1, change at index 0 produces hunk [0..1],
        // change at index 11 produces hunk [10..11].
        // Gap between changes (indices 1..10) = 10 lines > 2*1+1 = 3,
        // so they should be separate hunks.
        assert_eq!(result.hunks.len(), 2);
    }

    #[test]
    fn diff_line_type_as_str() {
        assert_eq!(DiffLineType::Add.as_str(), "add");
        assert_eq!(DiffLineType::Remove.as_str(), "remove");
        assert_eq!(DiffLineType::Equal.as_str(), "equal");
    }

    #[test]
    fn line_numbers_are_one_based() {
        let old = vec!["a", "b"];
        let new = vec!["a", "B"];
        let result = compute_diff(&old, &new, 3, 10000).unwrap();
        let hunk = &result.hunks[0];
        // Find the "equal" line for "a" — should have old_line=1, new_line=1
        let equal_a = hunk
            .lines
            .iter()
            .find(|l| l.line_type == DiffLineType::Equal && l.text == "a")
            .unwrap();
        assert_eq!(equal_a.old_line, Some(1));
        assert_eq!(equal_a.new_line, Some(1));

        // The removed "b" should have old_line=2
        let remove_b = hunk
            .lines
            .iter()
            .find(|l| l.line_type == DiffLineType::Remove)
            .unwrap();
        assert_eq!(remove_b.old_line, Some(2));
        assert_eq!(remove_b.new_line, None);

        // The added "B" should have new_line=2
        let add_b = hunk
            .lines
            .iter()
            .find(|l| l.line_type == DiffLineType::Add)
            .unwrap();
        assert_eq!(add_b.old_line, None);
        assert_eq!(add_b.new_line, Some(2));
    }

    #[test]
    fn hunk_old_new_start_counts() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "X", "c"];
        let result = compute_diff(&old, &new, 1, 10000).unwrap();
        assert_eq!(result.hunks.len(), 1);
        let hunk = &result.hunks[0];
        // Context=1, change is at old line 2.
        // Hunk should start at old line 1, spanning lines: a(eq), b(del), X(add), c(eq)
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.new_start, 1);
        // old_count = equal "a" + remove "b" + equal "c" = 3
        assert_eq!(hunk.old_count, 3);
        // new_count = equal "a" + add "X" + equal "c" = 3
        assert_eq!(hunk.new_count, 3);
    }
}
