//! Ralph Mode: AI loop executor for task implementation
//!
//! Ralph executes an AI loop that either:
//! - Plan mode: Analyzes the spec and creates an IMPLEMENTATION_PLAN.md
//! - Build mode: Implements the plan created during planning
//!
//! The spec is written to `.ralph-vibe-kanban/spec` and the loop script
//! `.ralph-vibe-kanban/loop.sh` is invoked to start execution.

use std::{path::Path, process::Stdio, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use ts_rs::TS;
use workspace_utils::msg_store::MsgStore;

use crate::{
    env::ExecutionEnv,
    executors::{ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    logs::{
        NormalizedEntry, NormalizedEntryType,
        plain_text_processor::PlainTextLogProcessor,
        stderr_processor::normalize_stderr_logs,
        utils::EntryIndexProvider,
    },
};

/// Ralph executor configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, TS, JsonSchema)]
pub struct RalphExecutor {
    /// When true, Ralph creates an implementation plan.
    /// When false, Ralph implements the existing plan.
    #[serde(default)]
    pub plan_mode: bool,
}

impl RalphExecutor {
    /// Create a new Ralph executor in plan mode
    pub fn plan() -> Self {
        Self { plan_mode: true }
    }

    /// Create a new Ralph executor in build mode
    pub fn build() -> Self {
        Self { plan_mode: false }
    }

    /// Path to the Ralph directory in the worktree
    fn ralph_dir(&self, worktree_path: &Path) -> std::path::PathBuf {
        worktree_path.join(".ralph-vibe-kanban")
    }

    /// Path to the loop.sh script
    fn loop_script_path(&self, worktree_path: &Path) -> std::path::PathBuf {
        self.ralph_dir(worktree_path).join("loop.sh")
    }

    /// Path to the spec file
    fn spec_path(&self, worktree_path: &Path) -> std::path::PathBuf {
        self.ralph_dir(worktree_path).join("spec")
    }

    /// Write the spec content to the spec file
    async fn write_spec(&self, worktree_path: &Path, spec: &str) -> Result<(), ExecutorError> {
        let spec_path = self.spec_path(worktree_path);
        info!("Ralph: writing spec to {:?}", spec_path);

        tokio::fs::write(&spec_path, spec)
            .await
            .map_err(|e| ExecutorError::Io(std::io::Error::other(format!(
                "Failed to write spec file: {}",
                e
            ))))?;

        Ok(())
    }

    /// Validate that Ralph is set up in the worktree
    fn validate_setup(&self, worktree_path: &Path) -> Result<(), ExecutorError> {
        let ralph_dir = self.ralph_dir(worktree_path);
        let loop_script = self.loop_script_path(worktree_path);

        if !ralph_dir.exists() {
            return Err(ExecutorError::Io(std::io::Error::other(
                "Ralph not set up in worktree. Missing .ralph-vibe-kanban directory",
            )));
        }

        if !loop_script.exists() {
            return Err(ExecutorError::Io(std::io::Error::other(
                "Ralph not set up in worktree. Missing .ralph-vibe-kanban/loop.sh",
            )));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&loop_script) {
                let permissions = metadata.permissions();
                if permissions.mode() & 0o111 == 0 {
                    return Err(ExecutorError::Io(std::io::Error::other(
                        "Ralph loop.sh is not executable",
                    )));
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for RalphExecutor {
    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        _env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let mode_str = if self.plan_mode { "plan" } else { "build" };
        info!("Ralph Executor: spawning in {} mode", mode_str);

        // 1. Validate Ralph setup
        self.validate_setup(current_dir)?;

        // 2. Validate we have a spec (prompt)
        if prompt.trim().is_empty() {
            return Err(ExecutorError::Io(std::io::Error::other(
                "Task must have a description (spec) to use Ralph",
            )));
        }

        // 3. Write spec to file
        self.write_spec(current_dir, prompt).await?;

        // 4. Build the command
        let loop_script = self.loop_script_path(current_dir);
        let mut cmd = tokio::process::Command::new(&loop_script);

        // Add "plan" argument if in plan mode
        if self.plan_mode {
            cmd.arg("plan");
        }

        cmd.current_dir(current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        debug!("Ralph: executing {:?}", cmd);

        // 5. Spawn the process
        let child = cmd.group_spawn().map_err(ExecutorError::Io)?;

        info!("Ralph Executor: process spawned successfully");
        Ok(SpawnedChild::from(child))
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        _session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Ralph doesn't support session continuation - just spawn fresh
        // This is intentional as each Ralph run is independent
        warn!("Ralph Executor: follow-up request treated as new spawn");
        self.spawn(current_dir, prompt, env).await
    }

    fn normalize_logs(&self, msg_store: Arc<MsgStore>, _worktree_path: &Path) {
        // Process stderr as error messages
        let entry_index_counter = EntryIndexProvider::start_from(&msg_store);
        normalize_stderr_logs(msg_store.clone(), entry_index_counter.clone());

        // Process stdout as assistant messages (plain text from Ralph loop)
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut stdout_lines = msg_store.stdout_lines_stream();

            let mut processor = PlainTextLogProcessor::builder()
                .normalized_entry_producer(Box::new(|content: String| NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::AssistantMessage,
                    content,
                    metadata: None,
                }))
                .transform_lines(Box::new(|lines| {
                    for line in lines.iter_mut() {
                        *line = strip_ansi_escapes::strip_str(&*line);
                    }
                }))
                .index_provider(entry_index_counter)
                .build();

            while let Some(Ok(line)) = stdout_lines.next().await {
                for patch in processor.process(line + "\n") {
                    msg_store.push_patch(patch);
                }
            }
        });
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        // Ralph doesn't use MCP configuration
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ralph_executor_plan_mode() {
        let executor = RalphExecutor::plan();
        assert!(executor.plan_mode);
    }

    #[test]
    fn test_ralph_executor_build_mode() {
        let executor = RalphExecutor::build();
        assert!(!executor.plan_mode);
    }

    #[test]
    fn test_ralph_paths() {
        let executor = RalphExecutor::default();
        let worktree = Path::new("/tmp/test-worktree");

        assert_eq!(
            executor.ralph_dir(worktree),
            Path::new("/tmp/test-worktree/.ralph-vibe-kanban")
        );
        assert_eq!(
            executor.loop_script_path(worktree),
            Path::new("/tmp/test-worktree/.ralph-vibe-kanban/loop.sh")
        );
        assert_eq!(
            executor.spec_path(worktree),
            Path::new("/tmp/test-worktree/.ralph-vibe-kanban/spec")
        );
    }

    #[test]
    fn test_default_is_build_mode() {
        let executor = RalphExecutor::default();
        // Default should be false (build mode), not plan mode
        // This is a safe default - you must explicitly set plan_mode to true
        assert!(!executor.plan_mode);
    }
}
