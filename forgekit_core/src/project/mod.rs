use std::path::{Path, PathBuf};

use crate::edit::EditModule;
use crate::error::{ForgeError, Result};
use crate::storage::UnifiedGraphStore;
use crate::types::Language;

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub language: Language,
    pub entry_point: PathBuf,
    pub manifest: Option<PathBuf>,
    pub source_dir: PathBuf,
}

pub struct ProjectModule {
    store: std::sync::Arc<UnifiedGraphStore>,
}

impl ProjectModule {
    pub fn new(store: std::sync::Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    pub async fn scaffold(&self, name: &str, language: Language) -> Result<ProjectInfo> {
        let project_root = self.store.codebase_path.join(name);
        if project_root.exists() {
            return Err(ForgeError::FileAlreadyExists(project_root));
        }

        let template = project_template(&language, name);
        let edit = EditModule::new(self.store.clone());

        for (rel_path, content) in &template.files {
            edit.create_file(Path::new(&format!("{}/{}", name, rel_path)), content)
                .await?;
        }

        Ok(ProjectInfo {
            root: project_root.clone(),
            language,
            entry_point: project_root.join(&template.entry_point),
            manifest: template.manifest.map(|m| project_root.join(m)),
            source_dir: project_root.join(&template.source_dir),
        })
    }

    pub fn detect(&self) -> Option<ProjectInfo> {
        let root = &self.store.codebase_path;
        detect_project(root)
    }
}

fn detect_project(root: &Path) -> Option<ProjectInfo> {
    let lang_and_manifest: Option<(Language, PathBuf)> = if root.join("Cargo.toml").exists() {
        Some((Language::Rust, root.join("Cargo.toml")))
    } else if root.join("go.mod").exists() {
        Some((Language::Go, root.join("go.mod")))
    } else if root.join("pom.xml").exists() {
        Some((Language::Java, root.join("pom.xml")))
    } else if root.join("package.json").exists() {
        let ext = if root.join("tsconfig.json").exists() {
            Language::TypeScript
        } else {
            Language::JavaScript
        };
        Some((ext, root.join("package.json")))
    } else if root.join("Makefile").exists() || root.join("makefile").exists() {
        Some((Language::C, root.join("Makefile")))
    } else if root.join("pyproject.toml").exists() || root.join("setup.py").exists() {
        Some((Language::Python, root.join("pyproject.toml")))
    } else {
        None
    };

    let (language, manifest) = lang_and_manifest?;

    let (source_dir, entry_point) = match &language {
        Language::Rust => ("src".to_string(), "src/main.rs".to_string()),
        Language::Python => ("src".to_string(), "src/main.py".to_string()),
        Language::Java => (
            "src/main/java".to_string(),
            "src/main/java/Main.java".to_string(),
        ),
        Language::C => ("src".to_string(), "src/main.c".to_string()),
        Language::TypeScript | Language::JavaScript => {
            ("src".to_string(), "src/index.ts".to_string())
        }
        Language::Go => (".".to_string(), "main.go".to_string()),
        _ => ("src".to_string(), "src/main".to_string()),
    };

    Some(ProjectInfo {
        root: root.to_path_buf(),
        language,
        entry_point: root.join(&entry_point),
        manifest: Some(manifest),
        source_dir: root.join(&source_dir),
    })
}

struct ProjectTemplate {
    files: Vec<(String, String)>,
    entry_point: String,
    manifest: Option<String>,
    source_dir: String,
}

fn project_template(lang: &Language, name: &str) -> ProjectTemplate {
    match lang {
        Language::Rust => ProjectTemplate {
            files: vec![
                (
                    "Cargo.toml".to_string(),
                    format!(
                        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                        name
                    ),
                ),
                ("src/main.rs".to_string(), "fn main() {\n    println!(\"Hello from {}!\");\n}\n".replace("{}", name)),
            ],
            entry_point: "src/main.rs".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            source_dir: "src".to_string(),
        },
        Language::Python => ProjectTemplate {
            files: vec![
                (
                    "pyproject.toml".to_string(),
                    format!(
                        "[project]\nname = \"{}\"\nversion = \"0.1.0\"\nrequires-python = \">=3.8\"\n",
                        name
                    ),
                ),
                ("src/__init__.py".to_string(), String::new()),
                (
                    "src/main.py".to_string(),
                    "def main():\n    print(\"Hello!\")\n\nif __name__ == \"__main__\":\n    main()\n".to_string(),
                ),
            ],
            entry_point: "src/main.py".to_string(),
            manifest: Some("pyproject.toml".to_string()),
            source_dir: "src".to_string(),
        },
        Language::Java => ProjectTemplate {
            files: vec![
                (
                    "pom.xml".to_string(),
                    format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project>\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>com.example</groupId>\n  <artifactId>{}</artifactId>\n  <version>0.1.0</version>\n</project>\n",
                        name
                    ),
                ),
                (
                    "src/main/java/Main.java".to_string(),
                    "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"Hello!\");\n    }\n}\n".to_string(),
                ),
            ],
            entry_point: "src/main/java/Main.java".to_string(),
            manifest: Some("pom.xml".to_string()),
            source_dir: "src/main/java".to_string(),
        },
        Language::C => ProjectTemplate {
            files: vec![
                (
                    "Makefile".to_string(),
                    format!("CC = gcc\nCFLAGS = -Wall -Wextra\n\n{}.out: src/main.o\n\t$(CC) $(CFLAGS) -o $@ $^\n\nsrc/main.o: src/main.c\n\t$(CC) $(CFLAGS) -c -o $@ $<\n\nclean:\n\trm -f *.out src/*.o\n", name),
                ),
                (
                    "src/main.c".to_string(),
                    "#include <stdio.h>\n\nint main(void) {\n    printf(\"Hello!\\n\");\n    return 0;\n}\n".to_string(),
                ),
                (
                    "include/.gitkeep".to_string(),
                    String::new(),
                ),
            ],
            entry_point: "src/main.c".to_string(),
            manifest: Some("Makefile".to_string()),
            source_dir: "src".to_string(),
        },
        Language::TypeScript => ProjectTemplate {
            files: vec![
                (
                    "package.json".to_string(),
                    format!(
                        "{{\"name\": \"{}\", \"version\": \"0.1.0\", \"main\": \"src/index.ts\", \"scripts\": {{\"build\": \"tsc\", \"test\": \"echo \\\"no tests\\\"\"}}}}\n",
                        name
                    ),
                ),
                (
                    "tsconfig.json".to_string(),
                    "{{\"compilerOptions\": {{\"target\": \"ES2020\", \"module\": \"commonjs\", \"outDir\": \"./dist\", \"strict\": true}}, \"include\": [\"src/**/*\"]}}\n".to_string(),
                ),
                (
                    "src/index.ts".to_string(),
                    "console.log(\"Hello!\");\n".to_string(),
                ),
            ],
            entry_point: "src/index.ts".to_string(),
            manifest: Some("package.json".to_string()),
            source_dir: "src".to_string(),
        },
        _ => ProjectTemplate {
            files: vec![("README.md".to_string(), format!("# {}\n", name))],
            entry_point: "README.md".to_string(),
            manifest: None,
            source_dir: ".".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    async fn make_store(dir: &Path) -> std::sync::Arc<UnifiedGraphStore> {
        std::sync::Arc::new(
            UnifiedGraphStore::open_with_path(dir, dir.join("test.db"), BackendKind::default())
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn test_scaffold_rust() {
        let temp = tempfile::tempdir().unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let info = module.scaffold("my-lib", Language::Rust).await.unwrap();
        assert!(info.root.ends_with("my-lib"));
        assert_eq!(info.language, Language::Rust);
        assert!(info.entry_point.ends_with("src/main.rs"));
        assert!(info.manifest.is_some());
        assert!(temp.path().join("my-lib/Cargo.toml").exists());
        assert!(temp.path().join("my-lib/src/main.rs").exists());
    }

    #[tokio::test]
    async fn test_scaffold_python() {
        let temp = tempfile::tempdir().unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let info = module.scaffold("my-py", Language::Python).await.unwrap();
        assert_eq!(info.language, Language::Python);
        assert!(temp.path().join("my-py/pyproject.toml").exists());
        assert!(temp.path().join("my-py/src/__init__.py").exists());
        assert!(temp.path().join("my-py/src/main.py").exists());
    }

    #[tokio::test]
    async fn test_scaffold_java() {
        let temp = tempfile::tempdir().unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let info = module.scaffold("my-java", Language::Java).await.unwrap();
        assert_eq!(info.language, Language::Java);
        assert!(temp.path().join("my-java/pom.xml").exists());
        assert!(temp.path().join("my-java/src/main/java/Main.java").exists());
    }

    #[tokio::test]
    async fn test_scaffold_c() {
        let temp = tempfile::tempdir().unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let info = module.scaffold("my-c", Language::C).await.unwrap();
        assert_eq!(info.language, Language::C);
        assert!(temp.path().join("my-c/Makefile").exists());
        assert!(temp.path().join("my-c/src/main.c").exists());
    }

    #[tokio::test]
    async fn test_scaffold_typescript() {
        let temp = tempfile::tempdir().unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let info = module
            .scaffold("my-ts", Language::TypeScript)
            .await
            .unwrap();
        assert_eq!(info.language, Language::TypeScript);
        assert!(temp.path().join("my-ts/package.json").exists());
        assert!(temp.path().join("my-ts/tsconfig.json").exists());
        assert!(temp.path().join("my-ts/src/index.ts").exists());
    }

    #[tokio::test]
    async fn test_scaffold_rejects_existing() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(temp.path().join("already-here"))
            .await
            .unwrap();
        let store = make_store(temp.path()).await;
        let module = ProjectModule::new(store);

        let result = module.scaffold("already-here", Language::Rust).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_rust_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let info = detect_project(temp.path()).unwrap();
        assert_eq!(info.language, Language::Rust);
    }

    #[test]
    fn test_detect_python_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("pyproject.toml"), "[project]\n").unwrap();
        let info = detect_project(temp.path()).unwrap();
        assert_eq!(info.language, Language::Python);
    }

    #[test]
    fn test_detect_typescript_over_js() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        std::fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        let info = detect_project(temp.path()).unwrap();
        assert_eq!(info.language, Language::TypeScript);
    }

    #[test]
    fn test_detect_nothing() {
        let temp = tempfile::tempdir().unwrap();
        assert!(detect_project(temp.path()).is_none());
    }
}
