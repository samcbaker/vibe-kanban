use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;

use crate::{
    actions::Executable,
    approvals::ExecutorApprovalService,
    env::ExecutionEnv,
    executors::{ExecutorError, SpawnedChild},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub enum RalphLoopMode {
    /// Plan mode - generates implementation plan
    Plan,
    /// Build mode - executes the loop to implement the plan
    Build,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct RalphLoopRequest {
    /// Path to the .ralph directory (e.g., /path/to/project/.ralph)
    pub ralph_path: String,
    /// The task specification content to write to .ralph/specs/
    pub task_spec: String,
    /// The filename for the spec (without .md extension)
    pub spec_filename: String,
    /// Mode to run: Plan or Build
    pub mode: RalphLoopMode,
    /// Maximum iterations (0 for unlimited)
    #[serde(default)]
    pub max_iterations: u32,
}

impl RalphLoopRequest {
    pub fn new_plan(ralph_path: String, task_spec: String, spec_filename: String) -> Self {
        Self {
            ralph_path,
            task_spec,
            spec_filename,
            mode: RalphLoopMode::Plan,
            max_iterations: 5, // Default to 5 iterations for planning
        }
    }

    pub fn new_build(ralph_path: String, task_spec: String, spec_filename: String) -> Self {
        Self {
            ralph_path,
            task_spec,
            spec_filename,
            mode: RalphLoopMode::Build,
            max_iterations: 0, // No limit for build mode
        }
    }
}

#[async_trait]
impl Executable for RalphLoopRequest {
    async fn spawn(
        &self,
        current_dir: &Path,
        _approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let ralph_dir = Path::new(&self.ralph_path);

        // Ensure the specs directory exists
        let specs_dir = ralph_dir.join("specs");
        tokio::fs::create_dir_all(&specs_dir)
            .await
            .map_err(ExecutorError::Io)?;

        // Write the task spec to the specs directory
        let spec_path = specs_dir.join(format!("{}.md", self.spec_filename));
        tokio::fs::write(&spec_path, &self.task_spec)
            .await
            .map_err(ExecutorError::Io)?;

        // Build the command
        let loop_script = ralph_dir.join("loop.sh");

        // Check if loop.sh exists
        if !loop_script.exists() {
            return Err(ExecutorError::ExecutableNotFound {
                program: loop_script.to_string_lossy().to_string(),
            });
        }

        let mut command = Command::new(&loop_script);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(current_dir);

        // Add mode argument
        match self.mode {
            RalphLoopMode::Plan => {
                command.arg("plan");
            }
            RalphLoopMode::Build => {
                // Build mode is the default, no arg needed
            }
        }

        // Add max iterations if specified
        if self.max_iterations > 0 {
            command.arg(self.max_iterations.to_string());
        }

        // Apply environment variables
        env.apply_to_command(&mut command);

        let child = command.group_spawn()?;

        Ok(child.into())
    }
}
