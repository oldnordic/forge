use std::path::Path;
use std::time::Duration;

use crate::diagnostic::{Diagnostic, DiagnosticParser};
use crate::error::{ForgeError, Result};
#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub duration: Duration,
    pub stdout: String,
    pub stderr: String,
}

impl BuildOutput {
    pub fn ok(stdout: String, stderr: String, duration: Duration) -> Self {
        Self {
            success: true,
            diagnostics: Vec::new(),
            duration,
            stdout,
            stderr,
        }
    }

    pub fn fail(
        diagnostics: Vec<Diagnostic>,
        stdout: String,
        stderr: String,
        duration: Duration,
    ) -> Self {
        Self {
            success: false,
            diagnostics,
            duration,
            stdout,
            stderr,
        }
    }

    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, crate::diagnostic::DiagnosticSeverity::Error))
            .collect()
    }

    pub fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, crate::diagnostic::DiagnosticSeverity::Warning))
            .collect()
    }
}

pub struct BuildModule {
    system: Box<dyn BuildSystem>,
}

impl BuildModule {
    pub fn new(system: Box<dyn BuildSystem>) -> Self {
        Self { system }
    }

    pub fn detect(project_root: &Path) -> Option<Self> {
        detect_build_system(project_root).map(Self::new)
    }

    pub fn system_name(&self) -> &str {
        self.system.name()
    }

    pub async fn check(&self, project_root: &Path) -> Result<BuildOutput> {
        self.system.check(project_root).await
    }

    pub async fn build(&self, project_root: &Path) -> Result<BuildOutput> {
        self.system.build(project_root).await
    }

    pub async fn test(&self, project_root: &Path) -> Result<BuildOutput> {
        self.system.test(project_root).await
    }

    pub async fn clean(&self, project_root: &Path) -> Result<BuildOutput> {
        self.system.clean(project_root).await
    }
}

pub fn detect_build_system(project_root: &Path) -> Option<Box<dyn BuildSystem>> {
    let systems: Vec<Box<dyn BuildSystem>> = vec![
        Box::new(CargoBuildSystem),
        Box::new(GoBuildSystem),
        Box::new(NpmBuildSystem),
        Box::new(MakeBuildSystem),
    ];

    systems.into_iter().find(|sys| sys.detect(project_root))
}

#[async_trait::async_trait]
pub trait BuildSystem: Send + Sync {
    fn name(&self) -> &str;
    fn detect(&self, project_root: &Path) -> bool;
    async fn check(&self, project_root: &Path) -> Result<BuildOutput>;
    async fn build(&self, project_root: &Path) -> Result<BuildOutput>;
    async fn test(&self, project_root: &Path) -> Result<BuildOutput>;
    async fn clean(&self, project_root: &Path) -> Result<BuildOutput>;
}

async fn run_command(
    program: &str,
    args: &[&str],
    working_dir: &Path,
    parser: &dyn DiagnosticParser,
) -> Result<BuildOutput> {
    let start = std::time::Instant::now();
    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| {
            ForgeError::ToolError(format!(
                "Failed to run {} {}: {}",
                program,
                args.join(" "),
                e
            ))
        })?;

    let duration = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let diagnostics = parser.parse(&stdout, &stderr);

    Ok(BuildOutput {
        success: output.status.success(),
        diagnostics,
        duration,
        stdout,
        stderr,
    })
}

pub struct CargoBuildSystem;

#[async_trait::async_trait]
impl BuildSystem for CargoBuildSystem {
    fn name(&self) -> &str {
        "cargo"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("Cargo.toml").exists()
    }

    async fn check(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::CargoDiagnosticParser;
        run_command(
            "cargo",
            &["check", "--message-format=json"],
            project_root,
            &parser,
        )
        .await
    }

    async fn build(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::CargoDiagnosticParser;
        run_command("cargo", &["build"], project_root, &parser).await
    }

    async fn test(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::CargoDiagnosticParser;
        run_command("cargo", &["test"], project_root, &parser).await
    }

    async fn clean(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "cargo".to_string(),
        };
        run_command("cargo", &["clean"], project_root, &parser).await
    }
}

pub struct GoBuildSystem;

#[async_trait::async_trait]
impl BuildSystem for GoBuildSystem {
    fn name(&self) -> &str {
        "go"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("go.mod").exists()
    }

    async fn check(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GoDiagnosticParser;
        run_command("go", &["vet", "./..."], project_root, &parser).await
    }

    async fn build(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GoDiagnosticParser;
        run_command("go", &["build", "./..."], project_root, &parser).await
    }

    async fn test(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GoDiagnosticParser;
        run_command("go", &["test", "./..."], project_root, &parser).await
    }

    async fn clean(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "go".to_string(),
        };
        run_command("go", &["clean", "-cache"], project_root, &parser).await
    }
}

pub struct NpmBuildSystem;

#[async_trait::async_trait]
impl BuildSystem for NpmBuildSystem {
    fn name(&self) -> &str {
        "npm"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("package.json").exists()
    }

    async fn check(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "npm".to_string(),
        };
        run_command("npm", &["run", "check"], project_root, &parser).await
    }

    async fn build(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "npm".to_string(),
        };
        run_command("npm", &["run", "build"], project_root, &parser).await
    }

    async fn test(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "npm".to_string(),
        };
        run_command("npm", &["test"], project_root, &parser).await
    }

    async fn clean(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "npm".to_string(),
        };
        run_command("npm", &["run", "clean"], project_root, &parser).await
    }
}

pub struct MakeBuildSystem;

#[async_trait::async_trait]
impl BuildSystem for MakeBuildSystem {
    fn name(&self) -> &str {
        "make"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("Makefile").exists() || project_root.join("makefile").exists()
    }

    async fn check(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "make".to_string(),
        };
        run_command("make", &["check"], project_root, &parser).await
    }

    async fn build(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "make".to_string(),
        };
        run_command("make", &[], project_root, &parser).await
    }

    async fn test(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "make".to_string(),
        };
        run_command("make", &["test"], project_root, &parser).await
    }

    async fn clean(&self, project_root: &Path) -> Result<BuildOutput> {
        let parser = crate::diagnostic::GenericDiagnosticParser {
            tool_name: "make".to_string(),
        };
        run_command("make", &["clean"], project_root, &parser).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cargo() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "cargo");
    }

    #[test]
    fn test_detect_go() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("go.mod"), "module test\n").unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "go");
    }

    #[test]
    fn test_detect_npm() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "npm");
    }

    #[test]
    fn test_detect_make() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("Makefile"), "all:\n\techo hello\n").unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "make");
    }

    #[test]
    fn test_detect_makefile_lowercase() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("makefile"), "all:\n\techo hello\n").unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "make");
    }

    #[test]
    fn test_detect_nothing() {
        let temp = tempfile::tempdir().unwrap();
        assert!(detect_build_system(temp.path()).is_none());
    }

    #[test]
    fn test_detect_cargo_priority_over_make() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        std::fs::write(temp.path().join("Makefile"), "all:\n\techo hello\n").unwrap();
        let sys = detect_build_system(temp.path()).unwrap();
        assert_eq!(sys.name(), "cargo");
    }

    #[test]
    fn test_build_output_ok() {
        let out = BuildOutput::ok(
            "done".to_string(),
            String::new(),
            Duration::from_millis(100),
        );
        assert!(out.success);
        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn test_build_output_fail() {
        let out = BuildOutput::fail(
            vec![Diagnostic::error("broken")],
            String::new(),
            "error".to_string(),
            Duration::from_millis(50),
        );
        assert!(!out.success);
        assert_eq!(out.errors().len(), 1);
    }

    #[test]
    fn test_build_output_errors_warnings() {
        let diags = vec![
            Diagnostic::error("e1"),
            Diagnostic::warning("w1"),
            Diagnostic::error("e2"),
            Diagnostic::warning("w2"),
        ];
        let out = BuildOutput::fail(diags, String::new(), String::new(), Duration::ZERO);
        assert_eq!(out.errors().len(), 2);
        assert_eq!(out.warnings().len(), 2);
    }

    #[tokio::test]
    async fn test_cargo_check_on_forge() {
        let forge_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sys = CargoBuildSystem;
        if !sys.detect(&forge_root) {
            return;
        }
        let out = sys.check(&forge_root).await.unwrap();
        assert!(out.success, "forge should pass cargo check: {}", out.stderr);
    }

    #[test]
    fn test_build_module_detect() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let module = BuildModule::detect(temp.path()).unwrap();
        assert_eq!(module.system_name(), "cargo");
    }
}
