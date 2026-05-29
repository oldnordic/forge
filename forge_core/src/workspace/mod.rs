use std::path::{Path, PathBuf};

use crate::error::{ForgeError, Result};
use crate::project::ProjectInfo;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub projects: Vec<ProjectInfo>,
}

impl Workspace {
    pub fn detect(path: &Path) -> Result<Option<Workspace>> {
        let root = find_workspace_root(path)?;
        let Some(root) = root else {
            return Ok(None);
        };
        let projects = discover_projects(&root);
        Ok(Some(Workspace { root, projects }))
    }

    pub fn open(path: &Path) -> Result<Workspace> {
        let root = find_workspace_root(path)?.ok_or_else(|| {
            ForgeError::ToolError(format!("no workspace root found from {}", path.display()))
        })?;
        let projects = discover_projects(&root);
        Ok(Workspace { root, projects })
    }

    pub fn project_for_path(&self, file: &Path) -> Option<&ProjectInfo> {
        let canonical = file.canonicalize().ok()?;
        self.projects
            .iter()
            .filter(|p| {
                p.root
                    .canonicalize()
                    .ok()
                    .is_some_and(|r| canonical.starts_with(r))
            })
            .max_by_key(|p| {
                p.root
                    .canonicalize()
                    .ok()
                    .map(|r| r.components().count())
                    .unwrap_or(0)
            })
    }
}

fn find_workspace_root(start: &Path) -> Result<Option<PathBuf>> {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()?.join(start)
    };

    loop {
        if is_workspace_marker(&current) {
            return Ok(Some(current));
        }
        current = match current.parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(None),
        };
    }
}

fn is_workspace_marker(dir: &Path) -> bool {
    [
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pnpm-workspace.yaml",
        "rush.json",
        "Lerna.json",
        "bazel/WORKSPACE",
        "WORKSPACE",
        "BUCK",
    ]
    .iter()
    .any(|marker| dir.join(marker).exists())
}

fn discover_projects(root: &Path) -> Vec<ProjectInfo> {
    let mut projects = Vec::new();

    if root.join("Cargo.toml").exists() {
        if let Some(info) = detect_rust_workspace(root) {
            projects.extend(info);
        } else {
            projects.push(single_rust_project(root));
        }
    }

    if root.join("pnpm-workspace.yaml").exists() {
        projects.extend(discover_pnpm_packages(root));
    } else if root.join("package.json").exists() {
        projects.push(single_node_project(root));
    }

    if root.join("go.mod").exists() {
        projects.push(single_go_project(root));
    }

    if projects.is_empty() {
        projects.push(generic_project(root));
    }

    projects
}

fn detect_rust_workspace(root: &Path) -> Option<Vec<ProjectInfo>> {
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    if !cargo_toml.contains("[workspace]") {
        return None;
    }
    let mut members = Vec::new();

    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("members") {
            let rest = rest.trim_start_matches([' ', '=']);
            for part in rest.split(',') {
                let part =
                    part.trim_matches(|c: char| c == '"' || c == ' ' || c == '[' || c == ']');
                if !part.is_empty() {
                    let member_path = root.join(part).join("Cargo.toml");
                    if member_path.exists() {
                        members.push(single_rust_project(&root.join(part)));
                    }
                }
            }
        } else if trimmed.contains("members =") && trimmed.contains('[') {
            let start = trimmed.find('[').unwrap_or(0);
            let end = trimmed.rfind(']').unwrap_or(trimmed.len());
            let inner = &trimmed[start + 1..end];
            for part in inner.split(',') {
                let part = part.trim_matches(|c: char| c == '"' || c == ' ');
                if !part.is_empty() {
                    let member_path = root.join(part).join("Cargo.toml");
                    if member_path.exists() {
                        members.push(single_rust_project(&root.join(part)));
                    }
                }
            }
        } else {
            let part = trimmed.trim_matches(|c: char| c == '"' || c == ',' || c == ' ');
            if !part.is_empty()
                && !part.starts_with('#')
                && !part.starts_with('[')
                && !part.contains('=')
            {
                let member_path = root.join(part).join("Cargo.toml");
                if member_path.exists() {
                    members.push(single_rust_project(&root.join(part)));
                }
            }
        }
    }

    if members.is_empty() {
        return None;
    }
    Some(members)
}

fn single_rust_project(root: &Path) -> ProjectInfo {
    let src_dir = if root.join("src").exists() {
        root.join("src")
    } else {
        root.to_path_buf()
    };
    ProjectInfo {
        root: root.to_path_buf(),
        language: crate::types::Language::Rust,
        entry_point: src_dir.join("main.rs"),
        manifest: Some(root.join("Cargo.toml")),
        source_dir: src_dir,
    }
}

fn single_node_project(root: &Path) -> ProjectInfo {
    let src_dir = if root.join("src").exists() {
        root.join("src")
    } else if root.join("lib").exists() {
        root.join("lib")
    } else {
        root.to_path_buf()
    };
    ProjectInfo {
        root: root.to_path_buf(),
        language: crate::types::Language::TypeScript,
        entry_point: src_dir.join("index.ts"),
        manifest: Some(root.join("package.json")),
        source_dir: src_dir,
    }
}

fn single_go_project(root: &Path) -> ProjectInfo {
    ProjectInfo {
        root: root.to_path_buf(),
        language: crate::types::Language::Go,
        entry_point: root.join("main.go"),
        manifest: Some(root.join("go.mod")),
        source_dir: root.to_path_buf(),
    }
}

fn discover_pnpm_packages(root: &Path) -> Vec<ProjectInfo> {
    let mut packages = Vec::new();
    for entry in walk_dirs(root, 3) {
        if entry.join("package.json").exists() {
            packages.push(single_node_project(&entry));
        }
    }
    if packages.is_empty() {
        packages.push(single_node_project(root));
    }
    packages
}

fn generic_project(root: &Path) -> ProjectInfo {
    ProjectInfo {
        root: root.to_path_buf(),
        language: crate::types::Language::Unknown("generic".to_string()),
        entry_point: root.to_path_buf(),
        manifest: None,
        source_dir: root.to_path_buf(),
    }
}

fn walk_dirs(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth >= max_depth {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with('.') || name == "node_modules" || name == "target" {
                        continue;
                    }
                    result.push(path.clone());
                    stack.push((path, depth + 1));
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_empty_dir_is_none() {
        let temp = tempfile::tempdir().unwrap();
        let result = Workspace::detect(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_cargo_toml() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "").unwrap();
        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.projects.len(), 1);
        assert!(matches!(
            ws.projects[0].language,
            crate::types::Language::Rust
        ));
    }

    #[test]
    fn test_detect_go_mod() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("go.mod"), "module example.com/m\n").unwrap();
        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert!(ws
            .projects
            .iter()
            .any(|p| matches!(p.language, crate::types::Language::Go)));
    }

    #[test]
    fn test_detect_package_json() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"), r#"{"name": "test"}"#).unwrap();
        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert!(ws
            .projects
            .iter()
            .any(|p| matches!(p.language, crate::types::Language::TypeScript)));
    }

    #[test]
    fn test_workspace_walks_up() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "").unwrap();
        let deep = temp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();
        let ws = Workspace::detect(&deep).unwrap().unwrap();
        assert_eq!(ws.root, temp.path());
    }

    #[test]
    fn test_rust_workspace_members() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/core\", \"crates/cli\"]\n",
        )
        .unwrap();

        let core = temp.path().join("crates").join("core");
        let cli = temp.path().join("crates").join("cli");
        std::fs::create_dir_all(core.join("src")).unwrap();
        std::fs::create_dir_all(cli.join("src")).unwrap();
        std::fs::write(core.join("Cargo.toml"), "").unwrap();
        std::fs::write(cli.join("Cargo.toml"), "").unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.projects.len(), 2);
    }

    #[test]
    fn test_project_for_path() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "").unwrap();

        let sub = temp.path().join("crates").join("lib");
        std::fs::create_dir_all(sub.join("src")).unwrap();
        std::fs::write(sub.join("Cargo.toml"), "").unwrap();

        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/lib\"]\n",
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        let file = sub.join("src").join("lib.rs");
        std::fs::write(&file, "").unwrap();
        let found = ws.project_for_path(&file);
        assert!(found.is_some());
        assert!(found.unwrap().root.ends_with("lib"));
    }

    #[test]
    fn test_open_fails_without_root() {
        let temp = tempfile::tempdir().unwrap();
        let result = Workspace::open(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_no_markers_finds_generic() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("WORKSPACE");
        std::fs::write(&marker, "").unwrap();
        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.projects.len(), 1);
        assert!(matches!(
            ws.projects[0].language,
            crate::types::Language::Unknown(_)
        ));
    }
}
