use std::path::PathBuf;

use crate::error::{ForgeError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub source: DependencySource,
    pub dev: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySource {
    Registry(String),
    Git { url: String, rev: Option<String> },
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct DependencyManifest {
    pub path: PathBuf,
    pub dependencies: Vec<Dependency>,
    pub dev_dependencies: Vec<Dependency>,
}

pub struct DependencyModule {
    project_root: PathBuf,
}

impl DependencyModule {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn list(&self) -> Result<Vec<DependencyManifest>> {
        let mut manifests = Vec::new();

        if let Some(m) = self.cargo_manifest()? {
            manifests.push(m);
        }
        if let Some(m) = self.npm_manifest()? {
            manifests.push(m);
        }
        if let Some(m) = self.go_manifest()? {
            manifests.push(m);
        }

        Ok(manifests)
    }

    pub fn add(&self, name: &str, version: Option<&str>, dev: bool) -> Result<()> {
        if self.project_root.join("Cargo.toml").exists() {
            return self.add_cargo_dep(name, version, dev);
        }
        if self.project_root.join("package.json").exists() {
            return self.add_npm_dep(name, version, dev);
        }
        if self.project_root.join("go.mod").exists() {
            return self.add_go_dep(name, version);
        }
        Err(ForgeError::ToolError(
            "No recognized manifest found".to_string(),
        ))
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        if self.project_root.join("Cargo.toml").exists() {
            return self.remove_cargo_dep(name);
        }
        if self.project_root.join("package.json").exists() {
            return self.remove_npm_dep(name);
        }
        Err(ForgeError::ToolError(
            "No recognized manifest found".to_string(),
        ))
    }

    fn cargo_manifest(&self) -> Result<Option<DependencyManifest>> {
        let path = self.project_root.join("Cargo.toml");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let doc = content
            .parse::<toml::Value>()
            .map_err(|e| ForgeError::ToolError(format!("Failed to parse Cargo.toml: {}", e)))?;

        let mut deps = Vec::new();
        let mut dev_deps = Vec::new();

        if let Some(table) = doc.get("dependencies").and_then(|v| v.as_table()) {
            for (name, value) in table {
                deps.push(parse_cargo_dep(name, value, false));
            }
        }
        if let Some(table) = doc.get("dev-dependencies").and_then(|v| v.as_table()) {
            for (name, value) in table {
                dev_deps.push(parse_cargo_dep(name, value, true));
            }
        }

        Ok(Some(DependencyManifest {
            path,
            dependencies: deps,
            dev_dependencies: dev_deps,
        }))
    }

    fn npm_manifest(&self) -> Result<Option<DependencyManifest>> {
        let path = self.project_root.join("package.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let doc: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ForgeError::ToolError(format!("Failed to parse package.json: {}", e)))?;

        let mut deps = Vec::new();
        let mut dev_deps = Vec::new();

        if let Some(obj) = doc.get("dependencies").and_then(|v| v.as_object()) {
            for (name, value) in obj {
                let version = value.as_str().map(|s| s.to_string());
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    source: DependencySource::Registry("npm".to_string()),
                    dev: false,
                });
            }
        }
        if let Some(obj) = doc.get("devDependencies").and_then(|v| v.as_object()) {
            for (name, value) in obj {
                let version = value.as_str().map(|s| s.to_string());
                dev_deps.push(Dependency {
                    name: name.clone(),
                    version,
                    source: DependencySource::Registry("npm".to_string()),
                    dev: true,
                });
            }
        }

        Ok(Some(DependencyManifest {
            path,
            dependencies: deps,
            dev_dependencies: dev_deps,
        }))
    }

    fn go_manifest(&self) -> Result<Option<DependencyManifest>> {
        let path = self.project_root.join("go.mod");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut deps = Vec::new();
        let mut in_require_block = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("require (") {
                in_require_block = true;
                continue;
            }
            if in_require_block && trimmed == ")" {
                in_require_block = false;
                continue;
            }

            if in_require_block {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 && !parts[0].starts_with("//") {
                    deps.push(Dependency {
                        name: parts[0].to_string(),
                        version: Some(parts[1].to_string()),
                        source: DependencySource::Registry("go".to_string()),
                        dev: false,
                    });
                }
            } else if trimmed.starts_with("require ") && !trimmed.contains('(') {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 3 {
                    deps.push(Dependency {
                        name: parts[1].to_string(),
                        version: Some(parts[2].to_string()),
                        source: DependencySource::Registry("go".to_string()),
                        dev: false,
                    });
                }
            }
        }

        Ok(Some(DependencyManifest {
            path,
            dependencies: deps,
            dev_dependencies: Vec::new(),
        }))
    }

    fn add_cargo_dep(&self, name: &str, version: Option<&str>, dev: bool) -> Result<()> {
        let path = self.project_root.join("Cargo.toml");
        let content = std::fs::read_to_string(&path)?;
        let section = if dev {
            "dev-dependencies"
        } else {
            "dependencies"
        };
        let ver = version.unwrap_or("*");
        let dep_line = format!("{} = \"{}\"\n", name, ver);

        let new_content = if let Some(pos) = content.find(&format!("[{}]", section)) {
            let mut s = String::new();
            s.push_str(&content[..pos + format!("[{}]", section).len()]);
            s.push('\n');
            s.push_str(&dep_line);
            s.push_str(&content[pos + format!("[{}]", section).len()..]);
            s
        } else {
            let mut s = content;
            if !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(&format!("\n[{}]\n", section));
            s.push_str(&dep_line);
            s
        };

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn add_npm_dep(&self, name: &str, version: Option<&str>, dev: bool) -> Result<()> {
        let path = self.project_root.join("package.json");
        let content = std::fs::read_to_string(&path)?;
        let ver = version.unwrap_or("*");
        let key = if dev {
            "devDependencies"
        } else {
            "dependencies"
        };

        let mut doc: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ForgeError::ToolError(format!("Failed to parse package.json: {}", e)))?;

        let obj = doc.as_object_mut().ok_or_else(|| {
            ForgeError::ToolError("package.json root is not an object".to_string())
        })?;
        if !obj.contains_key(key) {
            obj.insert(
                key.to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        if let Some(deps) = obj.get_mut(key).and_then(|v| v.as_object_mut()) {
            deps.insert(name.to_string(), serde_json::Value::String(ver.to_string()));
        }

        let output = serde_json::to_string_pretty(&doc)
            .map_err(|e| ForgeError::ToolError(format!("Failed to serialize: {}", e)))?;
        std::fs::write(&path, output)?;
        Ok(())
    }

    fn add_go_dep(&self, _name: &str, _version: Option<&str>) -> Result<()> {
        Err(ForgeError::ToolError(
            "Go dependencies should be managed via `go get`. Use BuildModule::build() instead."
                .to_string(),
        ))
    }

    fn remove_cargo_dep(&self, name: &str) -> Result<()> {
        let path = self.project_root.join("Cargo.toml");
        let content = std::fs::read_to_string(&path)?;
        let mut output = String::new();
        let pattern = format!("{} ", name);

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&pattern) || trimmed == name {
                continue;
            }
            output.push_str(line);
            output.push('\n');
        }

        std::fs::write(&path, output)?;
        Ok(())
    }

    fn remove_npm_dep(&self, name: &str) -> Result<()> {
        let path = self.project_root.join("package.json");
        let content = std::fs::read_to_string(&path)?;
        let mut doc: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ForgeError::ToolError(format!("Failed to parse: {}", e)))?;

        for key in &["dependencies", "devDependencies"] {
            if let Some(deps) = doc.get_mut(*key).and_then(|v| v.as_object_mut()) {
                deps.remove(name);
            }
        }

        let output = serde_json::to_string_pretty(&doc)
            .map_err(|e| ForgeError::ToolError(format!("Failed to serialize: {}", e)))?;
        std::fs::write(&path, output)?;
        Ok(())
    }
}

fn parse_cargo_dep(name: &str, value: &toml::Value, dev: bool) -> Dependency {
    match value {
        toml::Value::String(ver) => Dependency {
            name: name.to_string(),
            version: Some(ver.clone()),
            source: DependencySource::Registry("crates.io".to_string()),
            dev,
        },
        toml::Value::Table(table) => {
            let version = table
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let source = if let Some(git) = table.get("git").and_then(|v| v.as_str()) {
                DependencySource::Git {
                    url: git.to_string(),
                    rev: table
                        .get("rev")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                }
            } else if let Some(p) = table.get("path").and_then(|v| v.as_str()) {
                DependencySource::Path(PathBuf::from(p))
            } else {
                DependencySource::Registry("crates.io".to_string())
            };
            Dependency {
                name: name.to_string(),
                version,
                source,
                dev,
            }
        }
        _ => Dependency {
            name: name.to_string(),
            version: None,
            source: DependencySource::Registry("crates.io".to_string()),
            dev,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_toml() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = { version = \"1\", features = [\"full\"] }\nlocal = { path = \"../local\" }\n\n[dev-dependencies]\ntempfile = \"3\"\n",
        ).unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        let manifests = module.list().unwrap();
        assert_eq!(manifests.len(), 1);
        let m = &manifests[0];
        let deps_by_name: std::collections::HashMap<&str, &Dependency> = m
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d))
            .collect();
        assert_eq!(m.dependencies.len(), 3);
        assert_eq!(m.dev_dependencies.len(), 1);
        assert_eq!(deps_by_name["serde"].version.as_deref(), Some("1.0"));
        assert!(matches!(
            deps_by_name["tokio"].source,
            DependencySource::Registry(_)
        ));
        assert!(matches!(
            deps_by_name["local"].source,
            DependencySource::Path(_)
        ));
        assert_eq!(m.dev_dependencies[0].name, "tempfile");
        assert!(m.dev_dependencies[0].dev);
    }

    #[test]
    fn test_parse_package_json() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            "{\"dependencies\": {\"express\": \"^4.18.0\"}, \"devDependencies\": {\"jest\": \"^29.0.0\"}}",
        ).unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        let manifests = module.list().unwrap();
        assert_eq!(manifests.len(), 1);
        let m = &manifests[0];
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "express");
        assert_eq!(m.dev_dependencies[0].name, "jest");
        assert!(m.dev_dependencies[0].dev);
    }

    #[test]
    fn test_parse_go_mod() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/m\n\ngo 1.21\n\nrequire (\n\tfmt \"fmt\"\n\tstrings \"strings\"\n)\n",
        ).unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        let manifests = module.list().unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].dependencies.len(), 2);
    }

    #[test]
    fn test_add_cargo_dep() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n\n[dependencies]\n",
        )
        .unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        module.add("serde", Some("1.0"), false).unwrap();

        let content = std::fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
        assert!(content.contains("serde = \"1.0\""));
    }

    #[test]
    fn test_add_cargo_dev_dep() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n\n[dependencies]\n",
        )
        .unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        module.add("tempfile", Some("3"), true).unwrap();

        let content = std::fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
        assert!(content.contains("[dev-dependencies]"));
        assert!(content.contains("tempfile = \"3\""));
    }

    #[test]
    fn test_add_npm_dep() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"), "{\"name\": \"test\"}").unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        module.add("express", Some("^4.18"), false).unwrap();

        let content = std::fs::read_to_string(temp.path().join("package.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(doc["dependencies"]["express"], "^4.18");
    }

    #[test]
    fn test_remove_cargo_dep() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1\"\n",
        )
        .unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        module.remove("serde").unwrap();

        let content = std::fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
        assert!(!content.contains("serde"));
        assert!(content.contains("tokio"));
    }

    #[test]
    fn test_remove_npm_dep() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            "{\"dependencies\": {\"express\": \"^4.18\", \"lodash\": \"^4.0\"}}",
        )
        .unwrap();

        let module = DependencyModule::new(temp.path().to_path_buf());
        module.remove("express").unwrap();

        let content = std::fs::read_to_string(temp.path().join("package.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(doc["dependencies"]["express"].is_null());
        assert_eq!(doc["dependencies"]["lodash"], "^4.0");
    }

    #[test]
    fn test_list_no_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let module = DependencyModule::new(temp.path().to_path_buf());
        let manifests = module.list().unwrap();
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_add_no_manifest_error() {
        let temp = tempfile::tempdir().unwrap();
        let module = DependencyModule::new(temp.path().to_path_buf());
        let result = module.add("foo", Some("1.0"), false);
        assert!(result.is_err());
    }
}
