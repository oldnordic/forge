use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub template: String,
    #[serde(default)]
    pub few_shot_examples: Vec<FewShotExample>,
    #[serde(default)]
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FewShotExample {
    pub user: String,
    pub assistant: String,
}

impl PromptTemplate {
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        PromptTemplate {
            name: name.into(),
            template: template.into(),
            few_shot_examples: Vec::new(),
            version: String::new(),
        }
    }

    pub fn with_example(mut self, user: impl Into<String>, assistant: impl Into<String>) -> Self {
        self.few_shot_examples.push(FewShotExample {
            user: user.into(),
            assistant: assistant.into(),
        });
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();
        for (key, value) in vars {
            let placeholder = format!("{{{key}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    pub fn render_with_examples(&self, vars: &HashMap<String, String>) -> String {
        let base = self.render(vars);
        if self.few_shot_examples.is_empty() {
            return base;
        }
        let mut parts = vec![base];
        parts.push(String::from("\n\nExamples:"));
        for (i, ex) in self.few_shot_examples.iter().enumerate() {
            parts.push(format!("\nExample {}:", i + 1));
            parts.push(format!("User: {}", ex.user));
            parts.push(format!("Assistant: {}", ex.assistant));
        }
        parts.join("\n")
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let template: PromptTemplate = serde_json::from_str(&content).or_else(|_| {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string());
            let tmpl = PromptTemplate::new(name, &content);
            Ok::<PromptTemplate, serde_json::Error>(tmpl)
        })?;
        Ok(template)
    }

    pub fn load_from_dir(dir: &Path) -> Result<Vec<Self>> {
        let mut templates = Vec::new();
        if !dir.exists() {
            return Ok(templates);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .extension()
                .map(|e| e == "md" || e == "json")
                .unwrap_or(false)
            {
                if let Ok(tmpl) = Self::load_from_file(&path) {
                    templates.push(tmpl);
                }
            }
        }
        Ok(templates)
    }
}

pub struct PromptLibrary {
    templates: HashMap<String, PromptTemplate>,
}

impl PromptLibrary {
    pub fn new() -> Self {
        PromptLibrary {
            templates: HashMap::new(),
        }
    }

    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let mut library = PromptLibrary {
            templates: HashMap::new(),
        };
        let loaded = PromptTemplate::load_from_dir(&dir)?;
        for tmpl in loaded {
            library.templates.insert(tmpl.name.clone(), tmpl);
        }
        Ok(library)
    }

    pub fn register(&mut self, template: PromptTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    pub fn get(&self, name: &str) -> Option<&PromptTemplate> {
        self.templates.get(name)
    }

    pub fn render(&self, name: &str, vars: &HashMap<String, String>) -> Option<String> {
        self.templates.get(name).map(|t| t.render(vars))
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PromptLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_with_variables() {
        let tmpl = PromptTemplate::new("test", "Hello {name}, you are in {project}.");
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("project".to_string(), "forge".to_string());
        assert_eq!(tmpl.render(&vars), "Hello Alice, you are in forge.");
    }

    #[test]
    fn render_missing_vars_unchanged() {
        let tmpl = PromptTemplate::new("test", "Hello {name}, {missing}");
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Bob".to_string());
        assert_eq!(tmpl.render(&vars), "Hello Bob, {missing}");
    }

    #[test]
    fn render_no_vars() {
        let tmpl = PromptTemplate::new("test", "No variables here.");
        let vars = HashMap::new();
        assert_eq!(tmpl.render(&vars), "No variables here.");
    }

    #[test]
    fn render_with_few_shot() {
        let tmpl = PromptTemplate::new("test", "You are a coder.")
            .with_example("Read foo.rs", "Here is foo.rs: ...")
            .with_example("Fix bug", "Fixed in commit abc");

        let vars = HashMap::new();
        let rendered = tmpl.render_with_examples(&vars);
        assert!(rendered.contains("You are a coder."));
        assert!(rendered.contains("Example 1:"));
        assert!(rendered.contains("User: Read foo.rs"));
        assert!(rendered.contains("Assistant: Fixed in commit abc"));
    }

    #[test]
    fn library_register_and_get() {
        let mut lib = PromptLibrary::new();
        lib.register(PromptTemplate::new("system", "You are {role}."));
        lib.register(PromptTemplate::new("error", "Fix this: {error}"));

        let mut vars = HashMap::new();
        vars.insert("role".to_string(), "helpful".to_string());
        assert_eq!(
            lib.render("system", &vars),
            Some("You are helpful.".to_string())
        );
        assert!(lib.get("nonexistent").is_none());
    }

    #[test]
    fn library_list_names() {
        let mut lib = PromptLibrary::new();
        lib.register(PromptTemplate::new("a", "a"));
        lib.register(PromptTemplate::new("b", "b"));
        let mut names = lib.list_names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn load_from_markdown_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("system.md");
        std::fs::write(&path, "You are {role}, working on {project}.").unwrap();

        let tmpl = PromptTemplate::load_from_file(&path).unwrap();
        assert_eq!(tmpl.name, "system");
        assert!(tmpl.template.contains("{role}"));

        let mut vars = HashMap::new();
        vars.insert("role".to_string(), "coder".to_string());
        vars.insert("project".to_string(), "forge".to_string());
        assert_eq!(tmpl.render(&vars), "You are coder, working on forge.");
    }

    #[test]
    fn load_from_json_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("error.json");
        let json = serde_json::json!({
            "name": "error-fix",
            "template": "Fix error: {error} in {file}",
            "few_shot_examples": [{"user": "fix null pointer", "assistant": "added null check"}]
        });
        std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

        let tmpl = PromptTemplate::load_from_file(&path).unwrap();
        assert_eq!(tmpl.name, "error-fix");
        assert_eq!(tmpl.few_shot_examples.len(), 1);
    }

    #[test]
    fn library_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("system.md"), "You are {role}.").unwrap();
        std::fs::write(
            dir.path().join("fix.json"),
            serde_json::to_string(&serde_json::json!({
                "name": "fix",
                "template": "Fix: {error}"
            }))
            .unwrap(),
        )
        .unwrap();

        let lib = PromptLibrary::from_dir(dir.path()).unwrap();
        assert_eq!(lib.list_names().len(), 2);
        let mut vars = HashMap::new();
        vars.insert("role".to_string(), "coder".to_string());
        assert_eq!(
            lib.render("system", &vars),
            Some("You are coder.".to_string())
        );
    }

    #[test]
    fn load_from_nonexistent_dir_returns_empty() {
        let lib = PromptLibrary::from_dir("/nonexistent/path").unwrap();
        assert!(lib.list_names().is_empty());
    }
}
