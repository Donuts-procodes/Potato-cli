use colored::Colorize;

/// Generates a colored unified diff between two strings.
/// Returns a formatted string suitable for terminal display.
pub fn unified_diff(path: &str, old_content: &str, new_content: &str) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    let changeset = diff_lines(&old_lines, &new_lines);

    if changeset.is_empty() {
        return format!("  {} (no changes)", path.dimmed());
    }

    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", path).red().to_string());
    output.push_str(&format!("+++ b/{}\n", path).green().to_string());

    // Group changes into hunks
    let hunks = build_hunks(&changeset, 3);
    for hunk in hunks {
        output.push_str(&format!(
            "{}\n",
            format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            )
            .cyan()
        ));
        for line in &hunk.lines {
            match line {
                DiffLine::Context(s) => output.push_str(&format!(" {}\n", s)),
                DiffLine::Added(s) => output.push_str(&format!("{}\n", format!("+{}", s).green())),
                DiffLine::Removed(s) => output.push_str(&format!("{}\n", format!("-{}", s).red())),
            }
        }
    }

    output
}

/// Returns a short summary of changes (e.g., "+15 -3 lines").
pub fn diff_stat(old_content: &str, new_content: &str) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let changeset = diff_lines(&old_lines, &new_lines);

    let added = changeset.iter().filter(|c| matches!(c, Change::Added(_))).count();
    let removed = changeset.iter().filter(|c| matches!(c, Change::Removed(_))).count();

    format!(
        "{}{}",
        if added > 0 { format!("+{}", added).green().to_string() } else { String::new() },
        if removed > 0 { format!(" -{}", removed).red().to_string() } else { String::new() },
    )
}

// --- Internal diff algorithm (simple LCS-based) ---

#[derive(Debug)]
enum Change<'a> {
    Equal(&'a str),
    Added(&'a str),
    Removed(&'a str),
}

#[derive(Debug)]
enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<DiffLine>,
}

fn diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Change<'a>> {
    // Myers-like simple diff: build edit script via LCS
    let n = old.len();
    let m = new.len();

    // DP table for LCS length
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to produce changes
    let mut changes = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            changes.push(Change::Equal(old[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            changes.push(Change::Added(new[j - 1]));
            j -= 1;
        } else {
            changes.push(Change::Removed(old[i - 1]));
            i -= 1;
        }
    }
    changes.reverse();
    changes
}

fn build_hunks(changes: &[Change<'_>], context_lines: usize) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let mut old_line: usize = 1;
    let mut new_line: usize = 1;

    // Find ranges of non-equal changes
    let mut i = 0;
    while i < changes.len() {
        // Skip equal lines
        if matches!(changes[i], Change::Equal(_)) {
            old_line += 1;
            new_line += 1;
            i += 1;
            continue;
        }

        // Found a change — build a hunk with context
        let hunk_start = i.saturating_sub(context_lines);
        let hunk_old_start = if hunk_start < i { old_line.saturating_sub(i - hunk_start) } else { old_line };
        let hunk_new_start = if hunk_start < i { new_line.saturating_sub(i - hunk_start) } else { new_line };

        let mut lines = Vec::new();
        let mut old_count = 0usize;
        let mut new_count = 0usize;

        // Emit changes until we hit context_lines consecutive equals or end
        let mut consecutive_equals = 0;
        let mut j = i;
        while j < changes.len() {
            match &changes[j] {
                Change::Equal(s) => {
                    consecutive_equals += 1;
                    if consecutive_equals > context_lines * 2 {
                        break;
                    }
                    lines.push(DiffLine::Context(s.to_string()));
                    old_count += 1;
                    new_count += 1;
                }
                Change::Added(s) => {
                    consecutive_equals = 0;
                    lines.push(DiffLine::Added(s.to_string()));
                    new_count += 1;
                }
                Change::Removed(s) => {
                    consecutive_equals = 0;
                    lines.push(DiffLine::Removed(s.to_string()));
                    old_count += 1;
                }
            }
            j += 1;
        }

        // Trim trailing context
        while !lines.is_empty() && matches!(lines.last(), Some(DiffLine::Context(_))) && consecutive_equals > context_lines {
            lines.pop();
            old_count = old_count.saturating_sub(1);
            new_count = new_count.saturating_sub(1);
            consecutive_equals -= 1;
        }

        if !lines.is_empty() {
            hunks.push(Hunk {
                old_start: hunk_old_start,
                old_count,
                new_start: hunk_new_start,
                new_count,
                lines,
            });
        }

        // Advance past this hunk
        for change in &changes[i..j] {
            match change {
                Change::Equal(_) => { old_line += 1; new_line += 1; }
                Change::Added(_) => { new_line += 1; }
                Change::Removed(_) => { old_line += 1; }
            }
        }
        i = j;
    }

    hunks
}
