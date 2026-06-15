use std::path::PathBuf;

pub const MIN_CONFIDENCE_SCORE: f64 = 2.0;
pub const MAX_INJECTED_BYTES: usize = 32_768;

#[derive(Clone, Debug)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub location: PathBuf,
    pub triggers: Vec<String>,
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SkillContent {
    pub manifest: SkillManifest,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct SkillMatch {
    pub manifest: SkillManifest,
    pub score: f64,
}

impl SkillManifest {
    pub fn matches(&self, query: &str) -> bool {
        self.match_score(query) >= MIN_CONFIDENCE_SCORE
    }

    pub fn match_score(&self, query: &str) -> f64 {
        let lower = query.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let mut score: f64 = 0.0;

        for trigger in &self.triggers {
            let trigger_lower = trigger.to_lowercase();
            let trigger_words: Vec<&str> = trigger_lower.split_whitespace().collect();

            for tw in &trigger_words {
                for qw in &words {
                    if qw == tw {
                        score += 2.0;
                    } else if tw.len() >= 3 && qw.len() >= 3 && (qw.contains(tw) || tw.contains(qw))
                    {
                        score += 1.0;
                    }
                }
            }

            if lower.contains(&trigger_lower) {
                score += 3.0;
            }
        }

        let name_lower = self.name.to_lowercase();
        if lower.contains(&name_lower) {
            score += 5.0;
        }
        let name_words: Vec<&str> = name_lower.split(&['-', '_'][..]).collect();
        for nw in &name_words {
            for qw in &words {
                if qw == nw {
                    score += 1.5;
                } else if nw.len() >= 3 && qw.len() >= 3 && (qw.contains(nw) || nw.contains(qw)) {
                    score += 0.5;
                }
            }
        }

        let desc_lower = self.description.to_lowercase();
        for qw in &words {
            if qw.len() >= 3 && desc_lower.contains(qw) {
                score += 0.3;
            }
        }

        score
    }
}

impl SkillContent {
    pub fn system_prompt_fragment(&self) -> String {
        let deps = if self.manifest.depends_on.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nPrerequisites: load these skills first: {}",
                self.manifest.depends_on.join(", ")
            )
        };
        format!(
            "## Skill: {}\n\n{}\n\nSource: {}{}",
            self.manifest.name,
            self.content,
            self.manifest.location.display(),
            deps,
        )
    }

    pub fn fragment_byte_cost(&self) -> usize {
        let name_bytes = self.manifest.name.len();
        let source_bytes = self.manifest.location.display().to_string().len();
        let deps_bytes: usize = if self.manifest.depends_on.is_empty() {
            0
        } else {
            self.manifest
                .depends_on
                .iter()
                .map(|d| d.len() + 2)
                .sum::<usize>()
                + 45
        };
        self.content.len() + name_bytes + source_bytes + deps_bytes + 40
    }

    pub fn system_prompt_fragment_bounded(&self, max_bytes: usize) -> String {
        let fragment = self.system_prompt_fragment();
        if fragment.len() <= max_bytes {
            return fragment;
        }

        let target = max_bytes.saturating_sub(80);
        let cut = find_char_boundary(&fragment, target);
        let last_newline = fragment[..cut].rfind('\n').unwrap_or(0);
        format!(
            "{}\n\n[... truncated at {} bytes, full skill: {}]\n",
            &fragment[..last_newline],
            max_bytes,
            self.manifest.location.display()
        )
    }
}

fn find_char_boundary(s: &str, max_byte: usize) -> usize {
    if max_byte >= s.len() {
        return s.len();
    }
    let mut pos = max_byte;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

impl PartialEq for SkillMatch {
    fn eq(&self, other: &Self) -> bool {
        self.manifest.name == other.manifest.name
    }
}

impl Eq for SkillMatch {}

impl Ord for SkillMatch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for SkillMatch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_matches_trigger_above_threshold() {
        let manifest = SkillManifest {
            name: "grounded-coding".to_string(),
            description: "Grounded coding skill".to_string(),
            location: PathBuf::from("/skills/grounded-coding/SKILL.md"),
            triggers: vec![
                "code change".to_string(),
                "make code changes".to_string(),
                "debugging".to_string(),
            ],
            depends_on: vec![],
        };
        assert!(manifest.matches("I need to make code changes"));
        assert!(manifest.matches("debugging a test failure"));
        assert!(manifest.matches("use grounded-coding"));
        assert!(!manifest.matches("write a poem"));
    }

    #[test]
    fn test_manifest_below_threshold_rejected() {
        let manifest = SkillManifest {
            name: "x".to_string(),
            description: "A thing".to_string(),
            location: PathBuf::new(),
            triggers: vec!["zyxwvu".to_string()],
            depends_on: vec![],
        };
        assert!(!manifest.matches("completely unrelated query about poetry"));
    }

    #[test]
    fn test_skill_content_fragment() {
        let content = SkillContent {
            manifest: SkillManifest {
                name: "test-skill".to_string(),
                description: "A test".to_string(),
                location: PathBuf::from("/skills/test-skill/SKILL.md"),
                triggers: vec![],
                depends_on: vec!["core".to_string()],
            },
            content: "Do the thing.".to_string(),
        };
        let fragment = content.system_prompt_fragment();
        assert!(fragment.contains("test-skill"));
        assert!(fragment.contains("Do the thing."));
        assert!(fragment.contains("Prerequisites: load these skills first: core"));
    }

    #[test]
    fn test_fragment_byte_cost_approximates_actual() {
        let content = SkillContent {
            manifest: SkillManifest {
                name: "test".to_string(),
                description: String::new(),
                location: PathBuf::from("/skills/test/SKILL.md"),
                triggers: vec![],
                depends_on: vec![],
            },
            content: "Hello world".to_string(),
        };
        let actual = content.system_prompt_fragment().len();
        let estimated = content.fragment_byte_cost();
        let diff = (actual as i64 - estimated as i64).unsigned_abs();
        assert!(
            diff < 20,
            "cost estimate off by {diff}: actual={actual} estimated={estimated}"
        );
    }

    #[test]
    fn test_system_prompt_fragment_bounded_truncates() {
        let long_content = "x".repeat(1000);
        let content = SkillContent {
            manifest: SkillManifest {
                name: "big-skill".to_string(),
                description: String::new(),
                location: PathBuf::from("/big"),
                triggers: vec![],
                depends_on: vec![],
            },
            content: long_content,
        };
        let bounded = content.system_prompt_fragment_bounded(200);
        assert!(bounded.len() <= 300);
        assert!(bounded.contains("truncated at"));
    }

    #[test]
    fn test_system_prompt_fragment_bounded_utf8_safe() {
        let content = SkillContent {
            manifest: SkillManifest {
                name: "unicode".to_string(),
                description: String::new(),
                location: PathBuf::from("/u"),
                triggers: vec![],
                depends_on: vec![],
            },
            content: "日本語テスト🎉".repeat(100),
        };
        let bounded = content.system_prompt_fragment_bounded(200);
        assert!(bounded.len() <= 300, "bounded should not panic on UTF-8");
    }

    #[test]
    fn test_system_prompt_fragment_bounded_under_limit() {
        let content = SkillContent {
            manifest: SkillManifest {
                name: "small".to_string(),
                description: String::new(),
                location: PathBuf::new(),
                triggers: vec![],
                depends_on: vec![],
            },
            content: "short".to_string(),
        };
        let bounded = content.system_prompt_fragment_bounded(1024);
        assert_eq!(bounded, content.system_prompt_fragment());
    }

    #[test]
    fn test_match_score_exact_trigger() {
        let manifest = SkillManifest {
            name: "debugging".to_string(),
            description: "Find root cause before proposing fixes".to_string(),
            location: PathBuf::from("/skills/debugging/SKILL.md"),
            triggers: vec!["bug".to_string(), "test failure".to_string()],
            depends_on: vec![],
        };
        let score_bug = manifest.match_score("there is a bug in the code");
        let score_unrelated = manifest.match_score("write a poem about cats");
        assert!(score_bug > score_unrelated);
        assert!(score_bug >= MIN_CONFIDENCE_SCORE);
    }

    #[test]
    fn test_match_score_name_match() {
        let manifest = SkillManifest {
            name: "grounded-coding".to_string(),
            description: "Graph-backed coding discipline".to_string(),
            location: PathBuf::from("/skills/gc/SKILL.md"),
            triggers: vec![],
            depends_on: vec![],
        };
        let score = manifest.match_score("use grounded coding for this task");
        assert!(score > 0.0);
    }

    #[test]
    fn test_skill_match_ordering() {
        let m1 = SkillMatch {
            manifest: SkillManifest {
                name: "a".to_string(),
                description: String::new(),
                location: PathBuf::new(),
                triggers: vec![],
                depends_on: vec![],
            },
            score: 1.0,
        };
        let m2 = SkillMatch {
            manifest: SkillManifest {
                name: "b".to_string(),
                description: String::new(),
                location: PathBuf::new(),
                triggers: vec![],
                depends_on: vec![],
            },
            score: 3.0,
        };
        let mut sorted = [m1, m2];
        sorted.sort();
        assert_eq!(sorted[0].manifest.name, "b");
        assert_eq!(sorted[1].manifest.name, "a");
    }

    #[test]
    fn test_routing_fix_bug_prefers_debugging_over_tdd() {
        let debugging = SkillManifest {
            name: "debugging".to_string(),
            description: "Find root cause before proposing fixes".to_string(),
            location: PathBuf::new(),
            triggers: vec!["bug".to_string(), "unexpected behavior".to_string()],
            depends_on: vec![],
        };
        let tdd = SkillManifest {
            name: "tdd".to_string(),
            description: "Write tests first".to_string(),
            location: PathBuf::new(),
            triggers: vec![
                "implement".to_string(),
                "feature".to_string(),
                "fix".to_string(),
            ],
            depends_on: vec![],
        };
        let score_debug = debugging.match_score("fix the bug in react.rs");
        let score_tdd = tdd.match_score("fix the bug in react.rs");
        assert!(
            score_debug > score_tdd,
            "debugging ({score_debug}) should outrank tdd ({score_tdd}) for bug queries"
        );
    }

    #[test]
    fn test_routing_verify_prefers_verification() {
        let verification = SkillManifest {
            name: "verification".to_string(),
            description: "Run tests clippy audit deny gitleaks semgrep verify passed evidence"
                .to_string(),
            location: PathBuf::new(),
            triggers: vec![
                "verify".to_string(),
                "clippy".to_string(),
                "tests".to_string(),
                "audit".to_string(),
            ],
            depends_on: vec![],
        };
        let other = SkillManifest {
            name: "other".to_string(),
            description: "Some other skill".to_string(),
            location: PathBuf::new(),
            triggers: vec!["something".to_string()],
            depends_on: vec![],
        };
        let score_verify = verification.match_score("verify the tests pass and check clippy");
        let score_other = other.match_score("verify the tests pass and check clippy");
        assert!(score_verify > score_other);
        assert!(score_verify >= MIN_CONFIDENCE_SCORE);
    }

    #[test]
    fn test_find_char_boundary_ascii() {
        assert_eq!(find_char_boundary("hello world", 5), 5);
        assert_eq!(find_char_boundary("hello world", 100), 11);
    }

    #[test]
    fn test_find_char_boundary_utf8() {
        let s = "日本語テスト";
        let byte_3 = find_char_boundary(s, 3);
        assert_eq!(byte_3, 3, "first char is 3 bytes");
        let byte_4 = find_char_boundary(s, 4);
        assert_eq!(byte_4, 3, "4 lands mid-char, should back up to 3");
    }
}
