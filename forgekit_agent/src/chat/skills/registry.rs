use super::loader::SkillLoader;
use super::manifest::{SkillContent, SkillManifest, SkillMatch, MIN_CONFIDENCE_SCORE};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SkillRegistry {
    manifests: Vec<SkillManifest>,
    loaded: Arc<RwLock<HashMap<String, SkillContent>>>,
    loader: SkillLoader,
}

impl SkillRegistry {
    pub fn new(loader: SkillLoader) -> Self {
        let manifests = loader.discover();
        SkillRegistry {
            manifests,
            loaded: Arc::new(RwLock::new(HashMap::new())),
            loader,
        }
    }

    pub fn empty() -> Self {
        SkillRegistry {
            manifests: Vec::new(),
            loaded: Arc::new(RwLock::new(HashMap::new())),
            loader: SkillLoader::new(None),
        }
    }

    pub fn available_skills(&self) -> &[SkillManifest] {
        &self.manifests
    }

    pub fn find_matching(&self, query: &str) -> Vec<&SkillManifest> {
        self.manifests.iter().filter(|m| m.matches(query)).collect()
    }

    pub fn rank_matching(&self, query: &str) -> Vec<SkillMatch> {
        let mut matches: Vec<SkillMatch> = self
            .manifests
            .iter()
            .filter_map(|m| {
                let score = m.match_score(query);
                if score >= MIN_CONFIDENCE_SCORE {
                    Some(SkillMatch {
                        manifest: m.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();
        matches.sort();
        matches
    }

    pub fn has_skill(&self, name: &str) -> bool {
        self.manifests.iter().any(|m| m.name == name)
    }

    pub async fn load(&self, name: &str) -> Option<SkillContent> {
        {
            let loaded = self.loaded.read().await;
            if let Some(content) = loaded.get(name) {
                return Some(content.clone());
            }
        }

        let content = self.loader.load_content(name)?;

        self.loaded
            .write()
            .await
            .insert(name.to_string(), content.clone());

        Some(content)
    }

    pub async fn load_with_deps(&self, name: &str) -> Vec<SkillContent> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.load_deps_recursive(name, &mut seen, &mut results)
            .await;
        results
    }

    fn load_deps_recursive<'a>(
        &'a self,
        name: &'a str,
        seen: &'a mut std::collections::HashSet<String>,
        results: &'a mut Vec<SkillContent>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if seen.contains(name) {
                return;
            }
            seen.insert(name.to_string());

            let content = match self.load(name).await {
                Some(c) => c,
                None => return,
            };

            for dep in &content.manifest.depends_on {
                self.load_deps_recursive(dep, seen, results).await;
            }

            results.push(content);
        })
    }

    pub async fn rank_and_load(
        &self,
        query: &str,
        max_root_skills: usize,
        max_bytes: usize,
    ) -> Vec<SkillContent> {
        let ranked = self.rank_matching(query);
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut total_bytes = 0usize;
        let mut root_count = 0usize;

        for skill_match in ranked.iter() {
            if root_count >= max_root_skills {
                break;
            }

            let root_name = &skill_match.manifest.name;
            if seen.contains(root_name.as_str()) {
                continue;
            }

            let contents = self.load_with_deps(root_name).await;
            root_count += 1;

            for content in &contents {
                let c_name = &content.manifest.name;
                if seen.contains(c_name.as_str()) {
                    continue;
                }

                let cost = content.fragment_byte_cost();
                if total_bytes + cost > max_bytes {
                    continue;
                }

                seen.insert(c_name.clone());
                total_bytes += cost;
                results.push(content.clone());
            }
        }

        results
    }

    pub async fn loaded_skills(&self) -> Vec<String> {
        self.loaded.read().await.keys().cloned().collect()
    }

    pub fn skill_names(&self) -> Vec<&str> {
        self.manifests.iter().map(|m| m.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_BYTES: usize = super::super::manifest::MAX_INJECTED_BYTES;

    #[test]
    fn test_empty_registry() {
        let reg = SkillRegistry::empty();
        assert!(reg.available_skills().is_empty());
        assert!(reg.skill_names().is_empty());
    }

    #[test]
    fn test_rank_matching() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let skill_a_dir = temp.path().join(".forge").join("skills").join("debugging");
        std::fs::create_dir_all(&skill_a_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_a_dir.join("SKILL.md"),
            "---\nname: debugging\ndescription: \"Find root cause before proposing fixes\"\n---\n# Debugging\n\nTriggers: bug, test failure, unexpected behavior",
        )
        .expect("invariant: write succeeds");

        let skill_b_dir = temp.path().join(".forge").join("skills").join("planning");
        std::fs::create_dir_all(&skill_b_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_b_dir.join("SKILL.md"),
            "---\nname: planning\ndescription: \"Plan a feature or refactor\"\n---\n# Planning\n\nTriggers: plan, feature, refactor",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let ranked = reg.rank_matching("there is a bug in the code");
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].manifest.name, "debugging");
        assert!(ranked[0].score >= MIN_CONFIDENCE_SCORE);
    }

    #[test]
    fn test_rank_matching_below_threshold() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("obscure");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: obscure\ndescription: \"Does zyxwvu things\"\n---\n# Obscure\n\nTriggers: zyxwvu",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let ranked = reg.rank_matching("write a poem about cats");
        assert!(ranked.is_empty(), "should reject below-threshold noise");
    }

    #[tokio::test]
    async fn test_load_and_cache() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("cache-test");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Cache Test\n\nContent here.\nTriggers: caching, test, content",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let content = reg.load("cache-test").await.expect("invariant: loads");
        assert!(content.content.contains("Content here."));

        let loaded = reg.loaded_skills().await;
        assert!(loaded.contains(&"cache-test".to_string()));
    }

    #[tokio::test]
    async fn test_load_with_deps_recursive() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let core_dir = temp.path().join(".forge").join("skills").join("core");
        std::fs::create_dir_all(&core_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            core_dir.join("SKILL.md"),
            "---\nname: core\ndescription: \"Core skill\"\n---\n# Core\n\nCore content.",
        )
        .expect("invariant: write succeeds");

        let tools_dir = temp.path().join(".forge").join("skills").join("tools");
        std::fs::create_dir_all(&tools_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            tools_dir.join("SKILL.md"),
            "---\nname: tools\ndescription: \"Tools skill\"\ndepends_on: [\"core\"]\n---\n# Tools\n\nTools content.",
        )
        .expect("invariant: write succeeds");

        let tdd_dir = temp.path().join(".forge").join("skills").join("tdd");
        std::fs::create_dir_all(&tdd_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            tdd_dir.join("SKILL.md"),
            "---\nname: tdd\ndescription: \"TDD skill\"\ndepends_on: [\"tools\"]\n---\n# TDD\n\nTriggers: implement, test",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let contents = reg.load_with_deps("tdd").await;
        assert_eq!(contents.len(), 3, "should load core -> tools -> tdd");
        let names: Vec<&str> = contents.iter().map(|c| c.manifest.name.as_str()).collect();
        assert_eq!(names[0], "core");
        assert_eq!(names[1], "tools");
        assert_eq!(names[2], "tdd");
    }

    #[tokio::test]
    async fn test_load_with_deps_no_cycle() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let a_dir = temp.path().join(".forge").join("skills").join("a");
        std::fs::create_dir_all(&a_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            a_dir.join("SKILL.md"),
            "---\nname: a\ndescription: \"A\"\ndepends_on: [\"b\"]\n---\n# A",
        )
        .expect("invariant: write succeeds");

        let b_dir = temp.path().join(".forge").join("skills").join("b");
        std::fs::create_dir_all(&b_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            b_dir.join("SKILL.md"),
            "---\nname: b\ndescription: \"B\"\ndepends_on: [\"a\"]\n---\n# B",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let contents = reg.load_with_deps("a").await;
        assert_eq!(contents.len(), 2, "should handle cycles via seen set");
    }

    #[tokio::test]
    async fn test_rank_and_load_deps_dont_consume_root_slots() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let core_dir = temp.path().join(".forge").join("skills").join("core");
        std::fs::create_dir_all(&core_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            core_dir.join("SKILL.md"),
            "---\nname: core\ndescription: \"Core skill\"\n---\n# Core\n\nCore.",
        )
        .expect("invariant: write succeeds");

        let tdd_dir = temp.path().join(".forge").join("skills").join("tdd");
        std::fs::create_dir_all(&tdd_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            tdd_dir.join("SKILL.md"),
            "---\nname: tdd\ndescription: \"TDD skill\"\ndepends_on: [\"core\"]\n---\n# TDD\n\nTriggers: implement, test, tdd",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let contents = reg
            .rank_and_load("implement a tdd test", 1, MAX_BYTES)
            .await;
        let names: Vec<&str> = contents.iter().map(|c| c.manifest.name.as_str()).collect();
        assert!(
            names.contains(&"tdd"),
            "root skill should be present even with 1 dep: {:?}",
            names
        );
        assert!(
            names.contains(&"core"),
            "dep should be present alongside root: {:?}",
            names
        );
    }

    #[tokio::test]
    async fn test_rank_and_load_respects_byte_budget() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let big_dir = temp.path().join(".forge").join("skills").join("big-skill");
        std::fs::create_dir_all(&big_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            big_dir.join("SKILL.md"),
            format!(
                "---\nname: big-skill\ndescription: \"Big skill\"\n---\n# Big\n\n{}",
                "x".repeat(500)
            ),
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let contents = reg.rank_and_load("big skill query", 5, 100).await;
        let total: usize = contents.iter().map(|c| c.fragment_byte_cost()).sum();
        assert!(
            total <= 100,
            "total bytes should respect budget, got {total}"
        );
    }

    #[tokio::test]
    async fn test_rank_and_load() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");

        let skill_dir = temp
            .path()
            .join(".forge")
            .join("skills")
            .join("my-code-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Code Skill\n\nTriggers: code change, debugging",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let contents = reg
            .rank_and_load("I need to make code changes", 3, MAX_BYTES)
            .await;
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].manifest.name, "my-code-skill");
    }

    #[tokio::test]
    async fn test_find_matching() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp
            .path()
            .join(".forge")
            .join("skills")
            .join("my-code-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Code Skill\n\nTriggers: code change, debugging",
        )
        .expect("invariant: write succeeds");

        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let reg = SkillRegistry::new(loader);

        let matches = reg.find_matching("I need to make code changes");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "my-code-skill");

        let no_matches = reg.find_matching("write a poem");
        assert!(no_matches.is_empty());
    }
}
