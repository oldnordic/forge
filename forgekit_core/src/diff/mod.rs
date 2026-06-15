use std::path::PathBuf;

pub struct UnifiedDiff {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub hunks: Vec<Hunk>,
}

pub struct Hunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DiffLine {
    Context(String),
    Add(String),
    Remove(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct DiffStats {
    pub additions: usize,
    pub removals: usize,
    pub files_changed: usize,
}

impl UnifiedDiff {
    pub fn generate(old: &str, new: &str, old_path: &str, new_path: &str) -> Self {
        let changes = similar::TextDiff::from_lines(old, new);
        let mut hunks = Vec::new();

        for hunk in changes.unified_diff().iter_hunks() {
            let mut lines = Vec::new();
            let mut old_start = None;
            let mut old_count = 0u32;
            let mut new_start = None;
            let mut new_count = 0u32;

            for change in hunk.iter_changes() {
                if old_start.is_none() && change.old_index().is_some() {
                    old_start = Some(change.old_index().unwrap() as u32);
                }
                if new_start.is_none() && change.new_index().is_some() {
                    new_start = Some(change.new_index().unwrap() as u32);
                }
                match change.tag() {
                    similar::ChangeTag::Equal => {
                        lines.push(DiffLine::Context(change.to_string()));
                        old_count += 1;
                        new_count += 1;
                    }
                    similar::ChangeTag::Delete => {
                        lines.push(DiffLine::Remove(change.to_string()));
                        old_count += 1;
                    }
                    similar::ChangeTag::Insert => {
                        lines.push(DiffLine::Add(change.to_string()));
                        new_count += 1;
                    }
                }
            }

            if !lines.is_empty() {
                hunks.push(Hunk {
                    old_start: old_start.unwrap_or(0) + 1,
                    old_count,
                    new_start: new_start.unwrap_or(0) + 1,
                    new_count,
                    lines,
                });
            }
        }

        Self {
            old_path: PathBuf::from(old_path),
            new_path: PathBuf::from(new_path),
            hunks,
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "--- {}\n+++ {}\n",
            self.old_path.display(),
            self.new_path.display()
        ));

        for hunk in &self.hunks {
            out.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            ));
            for line in &hunk.lines {
                match line {
                    DiffLine::Context(s) => out.push_str(&format!(" {}", s)),
                    DiffLine::Add(s) => out.push_str(&format!("+{}", s)),
                    DiffLine::Remove(s) => out.push_str(&format!("-{}", s)),
                }
            }
        }

        out
    }

    pub fn apply(&self, original: &str) -> anyhow::Result<String> {
        let mut old_lines: Vec<&str> = original.lines().collect();
        let mut offset: i64 = 0;

        for hunk in &self.hunks {
            let start = (hunk.old_start as i64 - 1 + offset) as usize;
            let mut new_lines = Vec::new();

            for line in &hunk.lines {
                match line {
                    DiffLine::Context(s) => {
                        new_lines.push(s.trim_end_matches('\n').trim_end_matches('\r'));
                    }
                    DiffLine::Remove(_) => {}
                    DiffLine::Add(s) => {
                        new_lines.push(s.trim_end_matches('\n').trim_end_matches('\r'));
                    }
                }
            }

            let end = start + hunk.old_count as usize;
            if end <= old_lines.len() {
                let removed = end - start;
                old_lines.splice(start..end, new_lines.iter().copied());
                offset += new_lines.len() as i64 - removed as i64;
            }
        }

        Ok(old_lines.join("\n"))
    }

    pub fn reverse(&self) -> Self {
        let hunks = self
            .hunks
            .iter()
            .map(|h| {
                let lines = h
                    .lines
                    .iter()
                    .map(|l| match l {
                        DiffLine::Add(s) => DiffLine::Remove(s.clone()),
                        DiffLine::Remove(s) => DiffLine::Add(s.clone()),
                        DiffLine::Context(s) => DiffLine::Context(s.clone()),
                    })
                    .collect();
                Hunk {
                    old_start: h.new_start,
                    old_count: h.new_count,
                    new_start: h.old_start,
                    new_count: h.old_count,
                    lines,
                }
            })
            .collect();

        Self {
            old_path: self.new_path.clone(),
            new_path: self.old_path.clone(),
            hunks,
        }
    }

    pub fn stats(&self) -> DiffStats {
        let mut additions = 0;
        let mut removals = 0;

        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line {
                    DiffLine::Add(_) => additions += 1,
                    DiffLine::Remove(_) => removals += 1,
                    DiffLine::Context(_) => {}
                }
            }
        }

        DiffStats {
            additions,
            removals,
            files_changed: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_no_changes() {
        let diff = UnifiedDiff::generate("hello\n", "hello\n", "a.txt", "a.txt");
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn test_generate_add_line() {
        let diff = UnifiedDiff::generate("line1\n", "line1\nline2\n", "a.txt", "a.txt");
        assert!(!diff.hunks.is_empty());

        let has_add = diff
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| matches!(l, DiffLine::Add(_))));
        assert!(has_add, "should have at least one Add line");
    }

    #[test]
    fn test_generate_remove_line() {
        let diff = UnifiedDiff::generate("line1\nline2\n", "line1\n", "a.txt", "a.txt");
        assert!(!diff.hunks.is_empty());

        let has_remove = diff
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| matches!(l, DiffLine::Remove(_))));
        assert!(has_remove, "should have at least one Remove line");
    }

    #[test]
    fn test_generate_replace_line() {
        let diff = UnifiedDiff::generate("old\n", "new\n", "a.txt", "a.txt");
        assert!(!diff.hunks.is_empty());

        let stats = diff.stats();
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.removals, 1);
    }

    #[test]
    fn test_stats_counts() {
        let diff = UnifiedDiff::generate("a\nb\nc\n", "a\nx\nd\n", "old.txt", "new.txt");
        let stats = diff.stats();
        assert_eq!(stats.files_changed, 1);
        assert!(stats.additions >= 2);
        assert!(stats.removals >= 2);
    }

    #[test]
    fn test_to_string_format() {
        let diff = UnifiedDiff::generate("hello\n", "world\n", "a.txt", "b.txt");
        let s = diff.render();
        assert!(s.starts_with("--- a.txt\n+++ b.txt\n"));
        assert!(s.contains("@@"));
    }

    #[test]
    fn test_apply_roundtrip_add() {
        let old = "line1\n";
        let new = "line1\nline2\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let applied = diff.apply(old).unwrap();
        assert_eq!(applied, "line1\nline2");
    }

    #[test]
    fn test_apply_roundtrip_remove() {
        let old = "line1\nline2\n";
        let new = "line1\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let applied = diff.apply(old).unwrap();
        assert_eq!(applied, "line1");
    }

    #[test]
    fn test_apply_roundtrip_replace() {
        let old = "a\nb\nc\n";
        let new = "a\nx\nc\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let applied = diff.apply(old).unwrap();
        assert_eq!(applied, "a\nx\nc");
    }

    #[test]
    fn test_reverse_then_apply() {
        let old = "alpha\nbeta\n";
        let new = "alpha\ngamma\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let reversed = diff.reverse();
        let applied = reversed.apply(new).unwrap();
        assert_eq!(applied, "alpha\nbeta");
    }

    #[test]
    fn test_generate_empty_to_content() {
        let diff = UnifiedDiff::generate("", "new content\n", "a.txt", "a.txt");
        assert!(!diff.hunks.is_empty());
        let stats = diff.stats();
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.removals, 0);
    }

    #[test]
    fn test_generate_content_to_empty() {
        let diff = UnifiedDiff::generate("old content\n", "", "a.txt", "a.txt");
        assert!(!diff.hunks.is_empty());
        let stats = diff.stats();
        assert_eq!(stats.additions, 0);
        assert_eq!(stats.removals, 1);
    }

    #[test]
    fn test_apply_preserves_context() {
        let old = "line1\nline2\nline3\nline4\n";
        let new = "line1\nline2\nMODIFIED\nline4\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let applied = diff.apply(old).unwrap();
        assert_eq!(applied, "line1\nline2\nMODIFIED\nline4");
    }

    #[test]
    fn test_multiline_hunk() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nB\nC\nd\ne\n";
        let diff = UnifiedDiff::generate(old, new, "a.txt", "a.txt");
        let stats = diff.stats();
        assert!(stats.additions >= 2);
        assert!(stats.removals >= 2);
    }
}
