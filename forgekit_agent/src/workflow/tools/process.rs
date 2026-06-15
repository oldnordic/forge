use super::{TaskResult, ToolCompensation, ToolError, ToolResult};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ProcessGuard {
    pid: u32,
    pub tool_name: String,
    pub(super) terminated: Arc<AtomicBool>,
}

impl ProcessGuard {
    pub fn new(pid: u32, tool_name: impl Into<String>) -> Self {
        Self {
            pid,
            tool_name: tool_name.into(),
            terminated: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn terminate(&self) -> Result<(), ToolError> {
        if self.terminated.load(Ordering::SeqCst) {
            return Ok(());
        }

        #[cfg(unix)]
        {
            use std::process::Command;
            let result = Command::new("kill")
                .arg("-TERM")
                .arg(self.pid.to_string())
                .output();

            match result {
                Ok(output) => {
                    if output.status.success() {
                        self.terminated.store(true, Ordering::SeqCst);
                        Ok(())
                    } else {
                        Err(ToolError::TerminationFailed(format!(
                            "kill command failed for process {}",
                            self.pid
                        )))
                    }
                }
                Err(e) => Err(ToolError::TerminationFailed(format!(
                    "Failed to execute kill command: {}",
                    e
                ))),
            }
        }

        #[cfg(not(unix))]
        {
            Err(ToolError::TerminationFailed(
                "Process termination not supported on this platform".to_string(),
            ))
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::SeqCst)
    }
}

impl fmt::Display for ProcessGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProcessGuard(pid={}, tool={}, terminated={})",
            self.pid,
            self.tool_name,
            self.is_terminated()
        )
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if !self.is_terminated() {
            if let Err(e) = self.terminate() {
                eprintln!("ProcessGuard drop error: {}", e);
            }
        }
    }
}

impl From<ProcessGuard> for ToolCompensation {
    fn from(guard: ProcessGuard) -> Self {
        ToolCompensation::new(
            format!("Terminate process: {} ({})", guard.tool_name, guard.pid),
            move |_context| {
                if guard.terminate().is_ok() {
                    Ok(TaskResult::Success)
                } else {
                    Ok(TaskResult::Skipped)
                }
            },
        )
    }
}

#[derive(Clone, Debug)]
pub struct ToolInvocationResult {
    pub result: ToolResult,
    pub guard: Option<ProcessGuard>,
}

impl ToolInvocationResult {
    pub fn new(result: ToolResult, guard: Option<ProcessGuard>) -> Self {
        Self { result, guard }
    }

    pub fn completed(result: ToolResult) -> Self {
        Self {
            result,
            guard: None,
        }
    }
}
