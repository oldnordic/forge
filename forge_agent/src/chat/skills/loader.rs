use super::manifest::{SkillContent, SkillManifest};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct SkillLoader {
    search_paths: Vec<PathBuf>,
}

impl SkillLoader {
    pub fn new(project_dir: Option<&std::path::Path>) -> Self {
        let mut search_paths = Vec::new();

        if let Some(dir) = project_dir {
            search_paths.push(dir.join(".forge").join("skills"));
        }

        if let Ok(home) = std::env::var("HOME") {
            let user_skills = PathBuf::from(&home).join(".forge").join("skills");
            search_paths.push(user_skills);
            let claude_skills = PathBuf::from(&home).join(".claude").join("skills");
            search_paths.push(claude_skills);
            let opencode_skills = PathBuf::from(&home)
                .join(".config")
                .join("opencode")
                .join("skills");
            search_paths.push(opencode_skills);
        }

        SkillLoader { search_paths }
    }

    pub fn with_search_paths(search_paths: Vec<PathBuf>) -> Self {
        SkillLoader { search_paths }
    }

    pub fn discover(&self) -> Vec<SkillManifest> {
        let mut manifests = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(search_path) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let skill_dir = entry.path();
                            let skill_md = skill_dir.join("SKILL.md");
                            if skill_md.exists() {
                                if let Some(dir_name) = entry.file_name().to_str() {
                                    if let Some(manifest) =
                                        Self::parse_manifest(dir_name, &skill_md)
                                    {
                                        if seen_names.contains(&manifest.name) {
                                            continue;
                                        }
                                        seen_names.insert(manifest.name.clone());
                                        manifests.push(manifest);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        manifests
    }

    pub fn load_content(&self, name: &str) -> Option<SkillContent> {
        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(search_path) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let skill_md = entry.path().join("SKILL.md");
                            if skill_md.exists() {
                                if let Some(dir_name) = entry.file_name().to_str() {
                                    if let Some(manifest) =
                                        Self::parse_manifest(dir_name, &skill_md)
                                    {
                                        if manifest.name == name {
                                            let content =
                                                std::fs::read_to_string(&skill_md).ok()?;
                                            return Some(SkillContent { manifest, content });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn load_all_content(&self) -> HashMap<String, SkillContent> {
        let manifests = self.discover();
        let mut result = HashMap::new();
        for manifest in manifests {
            let name = manifest.name.clone();
            let location = manifest.location.clone();
            if let Ok(content) = std::fs::read_to_string(&location) {
                result.insert(name, SkillContent { manifest, content });
            }
        }
        result
    }

    fn parse_manifest(dir_name: &str, path: &std::path::Path) -> Option<SkillManifest> {
        let content = std::fs::read_to_string(path).ok()?;

        let (frontmatter, body) = split_frontmatter(&content);

        let (fm_name, fm_description, fm_depends) = parse_yaml_frontmatter(frontmatter);

        let final_name = fm_name.unwrap_or_else(|| dir_name.to_string());
        let description =
            fm_description.unwrap_or_else(|| extract_description(body).unwrap_or_default());
        let mut triggers = extract_triggers(body);
        let depends_on = fm_depends.unwrap_or_default();

        if triggers.is_empty() && !description.is_empty() {
            triggers = extract_implicit_triggers(&description);
        }

        Some(SkillManifest {
            name: final_name,
            description,
            location: path.to_path_buf(),
            triggers,
            depends_on,
        })
    }
}

fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }
    let after_first = &trimmed[3..];
    let after_first = after_first.trim_start_matches('\n');
    if let Some(end_idx) = after_first.find("\n---") {
        let frontmatter = &after_first[..end_idx];
        let body_start = end_idx + 4;
        let body = after_first[body_start..].trim_start();
        (Some(frontmatter), body)
    } else {
        (None, content)
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_inline_list(val: &str) -> Vec<String> {
    let val = val.trim();
    if !(val.starts_with('[') && val.ends_with(']')) {
        return Vec::new();
    }
    let inner = &val[1..val.len() - 1];
    inner
        .split(',')
        .map(unquote)
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_yaml_frontmatter(
    frontmatter: Option<&str>,
) -> (Option<String>, Option<String>, Option<Vec<String>>) {
    let fm = match frontmatter {
        Some(f) => f,
        None => return (None, None, None),
    };

    let mut name = None;
    let mut description = None;
    let mut depends_on = None;
    let mut in_dep_list = false;

    for line in fm.lines() {
        let trimmed = line.trim();

        if in_dep_list {
            if let Some(stripped) = trimmed.strip_prefix("- ") {
                depends_on
                    .get_or_insert_with(Vec::new)
                    .push(unquote(stripped));
            } else {
                in_dep_list = false;
            }
            if !trimmed.starts_with('-') && !trimmed.is_empty() {
                in_dep_list = false;
            }
            continue;
        }

        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(unquote(val));
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            description = Some(unquote(val));
        } else if let Some(val) = trimmed.strip_prefix("depends_on:") {
            let val = val.trim();
            if val.starts_with('[') {
                depends_on = Some(parse_inline_list(val));
            } else if val.starts_with('"') || val.starts_with('\'') {
                let unquoted = unquote(val);
                depends_on = Some(
                    unquoted
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            } else if val.is_empty() || val == "|" || val == ">" {
                in_dep_list = true;
            }
        }
    }

    (name, description, depends_on)
}

fn extract_description(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("//!")
            && !trimmed.starts_with("---")
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_triggers(content: &str) -> Vec<String> {
    let mut triggers = Vec::new();
    for line in content.lines() {
        let lower = line.to_lowercase();
        if lower.contains("triggers on:")
            || (lower.contains("triggers:") && !lower.contains("triggers on:"))
        {
            let after_marker = if let Some(idx) = lower.find(':') {
                &line[idx + 1..]
            } else {
                continue;
            };

            for part in after_marker.split(',') {
                let t = part.trim().trim_matches('.').trim_matches('`');
                if !t.is_empty() {
                    triggers.push(t.to_string());
                }
            }
            break;
        }
    }
    triggers
}

fn extract_implicit_triggers(description: &str) -> Vec<String> {
    let skip_words = std::collections::HashSet::from([
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
        "this", "that", "these", "those", "it", "its", "or", "and", "but", "in", "on", "at", "to",
        "for", "of", "with", "by", "from", "as", "into", "through", "during", "before", "after",
        "not", "no", "nor", "so", "if", "then", "than", "when", "where", "how", "what", "which",
        "who", "whom", "all", "each", "every", "both", "few", "more", "most", "other", "some",
        "such", "only", "own", "same", "too", "very", "just", "also", "about", "up", "out", "any",
        "their", "them", "they", "he", "she", "we", "you", "i", "me", "my", "your", "use", "used",
        "using", "should", "skill", "load",
    ]);

    let mut triggers = Vec::new();
    let lower = description.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();

    for word in cleaned.split_whitespace() {
        if word.len() >= 3 && !skip_words.contains(word) && !triggers.iter().any(|t| t == word) {
            triggers.push(word.to_string());
        }
        if triggers.len() >= 20 {
            break;
        }
    }

    triggers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter_with_fm() {
        let content = "---\nname: foo\ndescription: bar\n---\nBody here.";
        let (fm, body) = split_frontmatter(content);
        assert_eq!(fm, Some("name: foo\ndescription: bar"));
        assert!(body.starts_with("Body here."));
    }

    #[test]
    fn test_split_frontmatter_without_fm() {
        let content = "# Title\n\nBody here.";
        let (fm, body) = split_frontmatter(content);
        assert!(fm.is_none());
        assert!(body.contains("Body here."));
    }

    #[test]
    fn test_parse_yaml_frontmatter_quoted() {
        let fm = "name: grounded-coding-core\ndescription: \"Use this for coding\"\ndepends_on: [\"tools\"]";
        let (name, desc, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(name, Some("grounded-coding-core".to_string()));
        assert_eq!(desc, Some("Use this for coding".to_string()));
        assert_eq!(deps, Some(vec!["tools".to_string()]));
    }

    #[test]
    fn test_parse_yaml_frontmatter_single_quoted() {
        let fm = "name: 'my-skill'\ndescription: 'A skill'";
        let (name, desc, _) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(name, Some("my-skill".to_string()));
        assert_eq!(desc, Some("A skill".to_string()));
    }

    #[test]
    fn test_parse_yaml_frontmatter_unquoted() {
        let fm = "name: test\ndescription: This is a description";
        let (_, desc, _) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(desc, Some("This is a description".to_string()));
    }

    #[test]
    fn test_parse_yaml_frontmatter_list_style_deps() {
        let fm = "name: tdd\ndescription: \"TDD\"\ndepends_on:\n  - core\n  - planning";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(deps, Some(vec!["core".to_string(), "planning".to_string()]));
    }

    #[test]
    fn test_parse_yaml_frontmatter_list_style_quoted_deps() {
        let fm = "name: tdd\ndescription: \"TDD\"\ndepends_on:\n  - \"core\"\n  - 'planning'";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(deps, Some(vec!["core".to_string(), "planning".to_string()]));
    }

    #[test]
    fn test_parse_yaml_frontmatter_pipe_style_deps() {
        let fm = "name: tdd\ndescription: \"TDD\"\ndepends_on: |\n  - core\n  - planning";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(deps, Some(vec!["core".to_string(), "planning".to_string()]));
    }

    #[test]
    fn test_parse_yaml_frontmatter_comma_string_deps() {
        let fm = "name: skill\ndescription: \"desc\"\ndepends_on: \"deep-research, academic-paper, academic-paper-reviewer\"";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(
            deps,
            Some(vec![
                "deep-research".to_string(),
                "academic-paper".to_string(),
                "academic-paper-reviewer".to_string(),
            ])
        );
    }

    #[test]
    fn test_parse_yaml_frontmatter_comma_single_quoted_deps() {
        let fm = "name: skill\ndescription: \"desc\"\ndepends_on: 'deep-research, academic-paper'";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(
            deps,
            Some(vec![
                "deep-research".to_string(),
                "academic-paper".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_yaml_frontmatter_empty_deps() {
        let fm = "name: core\ndescription: \"Core\"";
        let (_, _, deps) = parse_yaml_frontmatter(Some(fm));
        assert_eq!(deps, None);
    }

    #[test]
    fn test_extract_description() {
        let content = "# My Skill\n\nThis is the description.\n\nMore details.";
        assert_eq!(
            extract_description(content),
            Some("This is the description.".to_string())
        );
    }

    #[test]
    fn test_extract_description_skips_comments() {
        let content = "//! Skill header\n//! More header\n\nReal description here.";
        assert_eq!(
            extract_description(content),
            Some("Real description here.".to_string())
        );
    }

    #[test]
    fn test_extract_triggers() {
        let content = "# Skill\nTriggers: code change, debugging, test failure\n\nBody.";
        let triggers = extract_triggers(content);
        assert_eq!(triggers, vec!["code change", "debugging", "test failure"]);
    }

    #[test]
    fn test_extract_triggers_line_format() {
        let content = "# Skill\nTriggers on: write paper, academic paper\n\nBody.";
        let triggers = extract_triggers(content);
        assert_eq!(triggers, vec!["write paper", "academic paper"]);
    }

    #[test]
    fn test_loader_discovers_from_temp_dir() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("test-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Test Skill\n\nA test skill for testing.\nTriggers: testing, verify",
        )
        .expect("invariant: write succeeds");

        let loader = SkillLoader::new(Some(temp.path()));
        let manifests = loader.discover();
        let found = manifests.iter().any(|m| m.name == "test-skill");
        assert!(found, "test-skill should be discovered from temp dir");
        let test_manifest = manifests
            .iter()
            .find(|m| m.name == "test-skill")
            .expect("invariant: found");
        assert_eq!(test_manifest.triggers, vec!["testing", "verify"]);
    }

    #[test]
    fn test_loader_dedup_by_manifest_name_not_dir() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let dir_a = temp.path().join(".forge").join("skills").join("dir-a");
        std::fs::create_dir_all(&dir_a).expect("invariant: dir creation succeeds");
        std::fs::write(
            dir_a.join("SKILL.md"),
            "---\nname: same-name\ndescription: \"From A\"\n---\n# A",
        )
        .expect("invariant: write succeeds");

        let dir_b = temp.path().join(".forge").join("skills").join("dir-b");
        std::fs::create_dir_all(&dir_b).expect("invariant: dir creation succeeds");
        std::fs::write(
            dir_b.join("SKILL.md"),
            "---\nname: same-name\ndescription: \"From B\"\n---\n# B",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let manifests = loader.discover();
        let count = manifests.iter().filter(|m| m.name == "same-name").count();
        assert_eq!(count, 1, "should dedup by manifest name, not dir name");
    }

    #[test]
    fn test_loader_load_by_manifest_name_not_dir() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let dir = temp.path().join(".forge").join("skills").join("my-dir");
        std::fs::create_dir_all(&dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: actual-name\ndescription: \"Loaded by name\"\n---\n# Actual",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);

        assert!(
            loader.load_content("actual-name").is_some(),
            "should load by manifest name"
        );
        assert!(
            loader.load_content("my-dir").is_none(),
            "should NOT load by directory name when frontmatter differs"
        );
    }

    #[test]
    fn test_loader_discovers_yaml_frontmatter() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("yaml-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: yaml-skill\ndescription: \"A YAML skill\"\ndepends_on: [\"core\"]\n---\n# YAML Skill\n\nBody content.",
        )
        .expect("invariant: write succeeds");

        let loader = SkillLoader::new(Some(temp.path()));
        let manifests = loader.discover();
        let m = manifests
            .iter()
            .find(|m| m.name == "yaml-skill")
            .expect("invariant: found");
        assert_eq!(m.description, "A YAML skill");
        assert_eq!(m.depends_on, vec!["core"]);
    }

    #[test]
    fn test_loader_deduplicates_across_search_paths() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let proj_skill = temp.path().join(".forge").join("skills").join("dup-skill");
        std::fs::create_dir_all(&proj_skill).expect("invariant: dir creation succeeds");
        std::fs::write(proj_skill.join("SKILL.md"), "# Dup\n\nFirst copy.\n")
            .expect("invariant: write succeeds");

        let search_path = temp.path().join(".forge").join("skills");
        let loader = SkillLoader::with_search_paths(vec![search_path.clone(), search_path]);

        let manifests = loader.discover();
        let count = manifests.iter().filter(|m| m.name == "dup-skill").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_loader_load_content() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# My Skill\n\nDoes things.\n\nDetailed instructions here.",
        )
        .expect("invariant: write succeeds");

        let loader = SkillLoader::new(Some(temp.path()));
        let content = loader.load_content("my-skill").expect("invariant: found");
        assert_eq!(content.manifest.name, "my-skill");
        assert!(content.content.contains("Detailed instructions here."));
    }

    #[test]
    fn test_loader_load_content_with_frontmatter() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("fm-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: fm-skill\ndescription: \"FM desc\"\n---\n# FM Skill\n\nContent here.",
        )
        .expect("invariant: write succeeds");

        let loader = SkillLoader::new(Some(temp.path()));
        let content = loader.load_content("fm-skill").expect("invariant: found");
        assert_eq!(content.manifest.description, "FM desc");
        assert!(content.content.contains("Content here."));
    }

    #[test]
    fn test_loader_missing_skill_returns_none() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let loader = SkillLoader::new(Some(temp.path()));
        assert!(loader.load_content("nonexistent").is_none());
    }
}
