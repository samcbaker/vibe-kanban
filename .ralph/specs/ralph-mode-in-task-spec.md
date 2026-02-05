# Ralph Mode in Task Specification

## Overview

Integrate the Ralph loop execution system into the task workflow, allowing users to execute tasks using Ralph's two-phase approach: **Plan** (creates `IMPLEMENTATION_PLAN.md`) and **Build** (implements based on the plan). The feature provides visual feedback during execution and requires user approval of the implementation plan before building.

## Related BDD Scenarios

No existing BDD scenarios found for this feature.

## Architecture Overview

### Ralph Execution Flow
```
Task Selected for Ralph → Create Workspace (Attempt) → Copy .ralph to worktree
                                    ↓
                              Plan Mode (ExecutionProcess)
                                    ↓
                              User Review
                                    ↓
                    ┌───────────────┼───────────────┐
                    ↓               ↓               ↓
                 Approve      Run Plan Again      Cancel
                    ↓               ↓               ↓
              Build Mode    Plan Mode (new)   Reset Status
             (ExecutionProcess)
                    ↓
             Complete/Failed
```

### Key Architecture Decisions

1. **Ralph as a CodingAgent Executor** - Implement Ralph as a new executor type following the `StandardCodingAgentExecutor` trait pattern
2. **Reuse existing infrastructure** - Use existing Workspace, Session, and ExecutionProcess models
3. **Plan and Build as separate ExecutionProcesses** - Each phase is tracked as its own process with `run_reason`
4. **Task description IS the spec** - The task's description field contains the spec content that Ralph uses for planning
5. **Spec file at `.ralph-vibe-kanban/spec`** - Task description is written to this file for Ralph to read
6. **Copy `.ralph` folder during worktree setup** - Integrate with existing worktree creation flow

### Key Requirements
1. Copy `.ralph` folder to task worktree root (as `.ralph-vibe-kanban` to avoid conflicts)
2. Write task description (the spec) to `.ralph-vibe-kanban/spec` file
3. Execute `.ralph-vibe-kanban/loop.sh plan` for planning phase
4. User reviews and approves `IMPLEMENTATION_PLAN.md`
5. Execute `.ralph-vibe-kanban/loop.sh` for build phase
6. Visual feedback throughout the process via existing streaming infrastructure

### Task Description = Spec File
When a user creates a task for Ralph execution, the **task description field contains the spec content**. This spec is what Ralph uses to understand what to implement. The description is written to `.ralph-vibe-kanban/spec` before executing the Ralph loop.

---

## Specs

### Spec 1: Add Ralph Executor Type

**Goal:** Create a new Ralph executor implementing the existing `StandardCodingAgentExecutor` trait

**Files to modify:**
- `crates/executors/src/executors/ralph.rs` (new)
- `crates/executors/src/executors/mod.rs`
- `crates/executors/src/lib.rs`

**Code Example:**
```rust
// crates/executors/src/executors/ralph.rs

use std::path::Path;
use async_trait::async_trait;
use tokio::process::Command;
use crate::{ExecutionEnv, ExecutorError, SpawnedChild};
use super::StandardCodingAgentExecutor;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct RalphExecutor {
    /// Whether to run in plan mode (creates IMPLEMENTATION_PLAN.md)
    /// If false, runs in build mode (implements the plan)
    #[serde(default)]
    pub plan_mode: bool,
}

impl Default for RalphExecutor {
    fn default() -> Self {
        Self { plan_mode: true }
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for RalphExecutor {
    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,  // Task title + description
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let ralph_dir = current_dir.join(".ralph-vibe-kanban");
        let script_path = ralph_dir.join("loop.sh");

        if !script_path.exists() {
            return Err(ExecutorError::SpawnError(
                "Ralph not set up in worktree. Missing .ralph-vibe-kanban/loop.sh".into()
            ));
        }

        // Write spec content to the spec file for Ralph to read
        // The prompt contains the task description which IS the spec
        let spec_path = ralph_dir.join("spec");
        tokio::fs::write(&spec_path, prompt).await
            .map_err(|e| ExecutorError::SpawnError(format!("Failed to write spec file: {}", e)))?;

        let mut cmd = Command::new(&script_path);
        cmd.current_dir(current_dir);

        if self.plan_mode {
            cmd.arg("plan");
        }

        // Set up stdio for streaming
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::null());

        // Inject environment variables
        for (key, value) in &env.env_vars {
            cmd.env(key, value);
        }

        let child = AsyncCommandGroup::new(cmd)
            .spawn()
            .map_err(|e| ExecutorError::SpawnError(format!("Failed to spawn Ralph: {}", e)))?;

        Ok(SpawnedChild {
            child,
            exit_signal: None,
            cancel: None,
        })
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        _session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Ralph doesn't have follow-up sessions - each run is independent
        // Just spawn a new execution
        self.spawn(current_dir, prompt, env).await
    }

    async fn spawn_review(
        &self,
        _current_dir: &Path,
        _prompt: &str,
        _session_id: Option<&str>,
        _env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        Err(ExecutorError::UnsupportedAction("Ralph does not support review mode".into()))
    }

    fn normalize_logs(&self, _raw_logs: Arc<MsgStore>, _worktree_path: &Path) {
        // Ralph outputs plain text - use default text processor
    }

    fn default_mcp_config_path(&self) -> Option<PathBuf> {
        None // Ralph has its own MCP setup
    }
}
```

**Add to CodingAgent enum:**
```rust
// crates/executors/src/executors/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS, enum_dispatch)]
#[ts(export)]
pub enum CodingAgent {
    // ... existing variants ...
    Ralph(RalphExecutor),
}
```

**Acceptance Criteria:**
- [ ] `RalphExecutor` struct created with `plan_mode` field
- [ ] Implements `StandardCodingAgentExecutor` trait
- [ ] Added to `CodingAgent` enum with enum_dispatch
- [ ] Spawns `loop.sh plan` when `plan_mode: true`
- [ ] Spawns `loop.sh` (build) when `plan_mode: false`
- [ ] Writes task description (the spec) to `.ralph-vibe-kanban/spec` file
- [ ] TypeScript types regenerated via `pnpm run generate-types`

---

### Spec 2: Add Ralph Run Reason

**Goal:** Add a new ExecutionProcessRunReason for Ralph executions

**Files to modify:**
- `crates/db/src/models/execution_process.rs`

**Code Example:**
```rust
// crates/db/src/models/execution_process.rs

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, TS, PartialEq, Eq)]
#[sqlx(type_name = "execution_process_run_reason", rename_all = "lowercase")]
#[ts(export)]
pub enum ExecutionProcessRunReason {
    SetupScript,
    CleanupScript,
    CodingAgent,
    DevServer,
    RalphPlan,    // NEW: Ralph planning phase
    RalphBuild,   // NEW: Ralph build phase
}
```

**Migration SQL:**
```sql
-- Add new enum values to execution_process_run_reason
ALTER TYPE execution_process_run_reason ADD VALUE 'ralphplan';
ALTER TYPE execution_process_run_reason ADD VALUE 'ralphbuild';
```

**Acceptance Criteria:**
- [ ] `RalphPlan` and `RalphBuild` added to enum
- [ ] Migration adds new enum values
- [ ] TypeScript types regenerated

---

### Spec 3: Add Ralph Status to Task Model

**Goal:** Track the overall Ralph workflow state on the task

**Files to modify:**
- `crates/db/src/models/task.rs`
- `crates/server/src/bin/generate_types.rs`
- Database migration file

**Code Example:**
```rust
// crates/db/src/models/task.rs

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, sqlx::Type, PartialEq, Eq, Default)]
#[sqlx(type_name = "ralph_status", rename_all = "lowercase")]
#[ts(export)]
pub enum RalphStatus {
    #[default]
    None,              // Not using Ralph
    Planning,          // Running loop.sh plan
    AwaitingApproval,  // Plan created, waiting for user approval
    Building,          // Running loop.sh (build mode)
    Completed,         // Ralph execution finished successfully
    Failed,            // Ralph execution failed
}

// In TaskWithAttemptStatus query, add:
pub struct TaskWithAttemptStatus {
    // ... existing fields ...
    pub ralph_status: RalphStatus,
}
```

**Migration SQL:**
```sql
CREATE TYPE ralph_status AS ENUM ('none', 'planning', 'awaitingapproval', 'building', 'completed', 'failed');

ALTER TABLE tasks
ADD COLUMN ralph_status ralph_status NOT NULL DEFAULT 'none';
```

**Acceptance Criteria:**
- [ ] `RalphStatus` enum created with all states
- [ ] `ralph_status` column added to tasks table
- [ ] `TaskWithAttemptStatus` includes `ralph_status`
- [ ] Default value is `None` for existing tasks
- [ ] TypeScript types regenerated

---

### Spec 4: Copy Ralph Folder During Worktree Setup

**Goal:** Automatically copy `.ralph` folder to worktree when creating a workspace for Ralph execution

**Files to modify:**
- `crates/services/src/services/worktree_manager.rs`
- `crates/local-deployment/src/container.rs`

**Code Example:**
```rust
// crates/services/src/services/worktree_manager.rs

/// Copy Ralph folder to worktree for Ralph execution
pub async fn setup_ralph_in_worktree(
    worktree_path: &Path,
    ralph_source_path: &Path,  // Path to .ralph in main repo
) -> Result<PathBuf, WorktreeError> {
    let target_path = worktree_path.join(".ralph-vibe-kanban");

    // Remove existing if present (for re-runs)
    if target_path.exists() {
        tokio::fs::remove_dir_all(&target_path).await
            .map_err(|e| WorktreeError::SetupError(format!("Failed to clean old Ralph folder: {}", e)))?;
    }

    // Copy directory recursively
    copy_dir_recursive(ralph_source_path, &target_path).await
        .map_err(|e| WorktreeError::SetupError(format!("Failed to copy Ralph folder: {}", e)))?;

    // Make loop.sh executable
    let script_path = target_path.join("loop.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script_path).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script_path, perms).await?;
    }

    Ok(target_path)
}

async fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type().await?.is_dir() {
            // Skip .venv directory (large, not needed)
            if entry.file_name() == ".venv" {
                continue;
            }
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}
```

**Acceptance Criteria:**
- [ ] `setup_ralph_in_worktree()` function created
- [ ] Copies `.ralph` to `.ralph-vibe-kanban` in worktree
- [ ] Skips `.venv` directory (not needed, large)
- [ ] Makes `loop.sh` executable
- [ ] Handles re-runs by cleaning existing folder first

---

### Spec 5: API Endpoints for Ralph Operations

**Goal:** Create REST API endpoints for Ralph task execution

**Files to modify:**
- `crates/server/src/routes/ralph.rs` (new)
- `crates/server/src/routes/mod.rs`

**Code Example:**
```rust
// crates/server/src/routes/ralph.rs

use actix_web::{web, HttpResponse, post, get};

/// Start Ralph plan mode for a task
/// Creates a workspace if needed, copies Ralph, and starts planning
#[post("/tasks/{task_id}/ralph/start-plan")]
pub async fn start_ralph_plan(
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let task_id = path.into_inner();
    let task = Task::get_by_id(&state.db, task_id).await?;

    // Verify task is not already running Ralph
    if task.ralph_status != RalphStatus::None && task.ralph_status != RalphStatus::Failed {
        return Err(ApiError::BadRequest("Ralph is already running for this task".into()));
    }

    // Verify task has a spec (description)
    let spec_content = task.description.as_ref()
        .filter(|d| !d.trim().is_empty())
        .ok_or_else(|| ApiError::BadRequest(
            "Task must have a description (spec) to use Ralph".into()
        ))?;

    // Update task status
    Task::update_ralph_status(&state.db, task_id, RalphStatus::Planning).await?;

    // Get or create workspace for this task
    let workspace = get_or_create_ralph_workspace(&state, &task).await?;

    // Setup Ralph in worktree
    let worktree_path = get_worktree_path(&workspace);
    let ralph_source = state.config.ralph_source_path(); // Path to .ralph in main repo
    setup_ralph_in_worktree(&worktree_path, &ralph_source).await?;

    // Start execution process with RalphPlan reason
    // spec_content was validated above - it's the task description (the spec)
    let executor = CodingAgent::Ralph(RalphExecutor { plan_mode: true });

    let process = state.container
        .start_execution(
            &workspace,
            executor,
            spec_content,  // Pass the spec content to be written to .ralph-vibe-kanban/spec
            ExecutionProcessRunReason::RalphPlan,
        )
        .await?;

    // Spawn task to update status when planning completes
    spawn_ralph_plan_completion_handler(state.clone(), task_id, process.id);

    Ok(HttpResponse::Ok().json(json!({
        "workspace_id": workspace.id,
        "process_id": process.id,
    })))
}

/// Get the implementation plan content
#[get("/tasks/{task_id}/ralph/plan")]
pub async fn get_ralph_plan(
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let task_id = path.into_inner();
    let task = Task::get_by_id(&state.db, task_id).await?;

    if task.ralph_status != RalphStatus::AwaitingApproval {
        return Err(ApiError::BadRequest("No plan available for review".into()));
    }

    let workspace = Workspace::get_latest_for_task(&state.db, task_id).await?;
    let worktree_path = get_worktree_path(&workspace);
    let plan_path = worktree_path.join("IMPLEMENTATION_PLAN.md");

    let content = tokio::fs::read_to_string(&plan_path).await
        .map_err(|_| ApiError::NotFound("Implementation plan not found".into()))?;

    Ok(HttpResponse::Ok().json(json!({ "content": content })))
}

/// Approve the plan and start build
#[post("/tasks/{task_id}/ralph/approve")]
pub async fn approve_ralph_plan(
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let task_id = path.into_inner();
    let task = Task::get_by_id(&state.db, task_id).await?;

    if task.ralph_status != RalphStatus::AwaitingApproval {
        return Err(ApiError::BadRequest("Task is not awaiting plan approval".into()));
    }

    // Update status to Building
    Task::update_ralph_status(&state.db, task_id, RalphStatus::Building).await?;

    let workspace = Workspace::get_latest_for_task(&state.db, task_id).await?;

    // Start execution process with RalphBuild reason
    // Spec was validated when planning started - safe to unwrap
    let executor = CodingAgent::Ralph(RalphExecutor { plan_mode: false });
    let spec_content = task.description.as_deref().unwrap_or("");

    let process = state.container
        .start_execution(
            &workspace,
            executor,
            spec_content,
            ExecutionProcessRunReason::RalphBuild,
        )
        .await?;

    // Spawn task to update status when build completes
    spawn_ralph_build_completion_handler(state.clone(), task_id, process.id);

    Ok(HttpResponse::Ok().json(json!({
        "process_id": process.id,
    })))
}

/// Re-run plan mode
#[post("/tasks/{task_id}/ralph/replan")]
pub async fn rerun_ralph_plan(
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let task_id = path.into_inner();
    let task = Task::get_by_id(&state.db, task_id).await?;

    if task.ralph_status != RalphStatus::AwaitingApproval {
        return Err(ApiError::BadRequest("Can only replan when awaiting approval".into()));
    }

    // Update status back to Planning
    Task::update_ralph_status(&state.db, task_id, RalphStatus::Planning).await?;

    let workspace = Workspace::get_latest_for_task(&state.db, task_id).await?;

    // Start new plan execution
    // Spec was validated when planning first started - safe to unwrap
    let executor = CodingAgent::Ralph(RalphExecutor { plan_mode: true });
    let spec_content = task.description.as_deref().unwrap_or("");

    let process = state.container
        .start_execution(
            &workspace,
            executor,
            spec_content,
            ExecutionProcessRunReason::RalphPlan,
        )
        .await?;

    spawn_ralph_plan_completion_handler(state.clone(), task_id, process.id);

    Ok(HttpResponse::Ok().json(json!({
        "process_id": process.id,
    })))
}

/// Cancel Ralph execution
#[post("/tasks/{task_id}/ralph/cancel")]
pub async fn cancel_ralph(
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let task_id = path.into_inner();

    // Kill any running Ralph processes for this task
    let workspace = Workspace::get_latest_for_task(&state.db, task_id).await?;
    state.container.kill_running_processes(&workspace).await?;

    // Reset status
    Task::update_ralph_status(&state.db, task_id, RalphStatus::None).await?;

    Ok(HttpResponse::Ok().finish())
}

// Helper: Monitor plan completion and update task status
fn spawn_ralph_plan_completion_handler(
    state: web::Data<AppState>,
    task_id: Uuid,
    process_id: Uuid,
) {
    tokio::spawn(async move {
        // Wait for process to complete
        let result = state.container.wait_for_process(process_id).await;

        let new_status = match result {
            Ok(exit_code) if exit_code == 0 => RalphStatus::AwaitingApproval,
            _ => RalphStatus::Failed,
        };

        let _ = Task::update_ralph_status(&state.db, task_id, new_status).await;
    });
}

// Helper: Monitor build completion and update task status
fn spawn_ralph_build_completion_handler(
    state: web::Data<AppState>,
    task_id: Uuid,
    process_id: Uuid,
) {
    tokio::spawn(async move {
        let result = state.container.wait_for_process(process_id).await;

        let new_status = match result {
            Ok(exit_code) if exit_code == 0 => RalphStatus::Completed,
            _ => RalphStatus::Failed,
        };

        let _ = Task::update_ralph_status(&state.db, task_id, new_status).await;
    });
}
```

**Acceptance Criteria:**
- [ ] `POST /tasks/{id}/ralph/start-plan` creates workspace, copies Ralph, starts planning
- [ ] `POST /tasks/{id}/ralph/start-plan` validates task has non-empty description (spec)
- [ ] `GET /tasks/{id}/ralph/plan` returns `IMPLEMENTATION_PLAN.md` content
- [ ] `POST /tasks/{id}/ralph/approve` starts build phase
- [ ] `POST /tasks/{id}/ralph/replan` re-runs planning
- [ ] `POST /tasks/{id}/ralph/cancel` kills processes and resets status
- [ ] Completion handlers update `ralph_status` automatically
- [ ] Proper state transition validation
- [ ] Process output is streamed via existing WebSocket infrastructure

---

### Spec 6: Frontend API Client for Ralph

**Goal:** Add TypeScript API functions for Ralph operations

**Files to modify:**
- `frontend/src/api/tasks.ts`

**Code Example:**
```typescript
// frontend/src/api/tasks.ts

export interface RalphStartResponse {
  workspace_id: string;
  process_id: string;
}

export interface RalphPlanResponse {
  content: string;
}

export const tasksApi = {
  // ... existing methods ...

  /** Start Ralph plan mode for a task */
  startRalphPlan: async (taskId: string): Promise<RalphStartResponse> => {
    return api.post(`/tasks/${taskId}/ralph/start-plan`);
  },

  /** Get the implementation plan content */
  getRalphPlan: async (taskId: string): Promise<string> => {
    const response = await api.get<RalphPlanResponse>(`/tasks/${taskId}/ralph/plan`);
    return response.content;
  },

  /** Approve plan and start build */
  approveRalphPlan: async (taskId: string): Promise<{ process_id: string }> => {
    return api.post(`/tasks/${taskId}/ralph/approve`);
  },

  /** Re-run plan mode */
  rerunRalphPlan: async (taskId: string): Promise<RalphStartResponse> => {
    return api.post(`/tasks/${taskId}/ralph/replan`);
  },

  /** Cancel Ralph execution */
  cancelRalph: async (taskId: string): Promise<void> => {
    await api.post(`/tasks/${taskId}/ralph/cancel`);
  },
};
```

**Acceptance Criteria:**
- [ ] All API functions properly typed
- [ ] Return types match backend responses
- [ ] Error handling follows existing patterns

---

### Spec 7: Ralph Status Indicator on Task Card

**Goal:** Show visual feedback of Ralph execution state on task cards

**Files to modify:**
- `frontend/src/components/tasks/TaskCard.tsx`
- `frontend/src/utils/ralphStatus.ts` (new)

**Code Example:**
```tsx
// frontend/src/utils/ralphStatus.ts

import { Bot, FileCheck, Hammer, CheckCircle2, XCircle } from 'lucide-react';
import { RalphStatus } from '@shared/types';

export const ralphStatusConfig: Record<RalphStatus, {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  color: string;
  animate?: string;
}> = {
  none: { icon: () => null, label: '', color: '' },
  planning: {
    icon: Bot,
    label: 'Planning...',
    color: 'text-purple-500',
    animate: 'animate-pulse'
  },
  awaitingapproval: {
    icon: FileCheck,
    label: 'Review Plan',
    color: 'text-orange-500'
  },
  building: {
    icon: Hammer,
    label: 'Building...',
    color: 'text-blue-500',
    animate: 'animate-bounce'
  },
  completed: {
    icon: CheckCircle2,
    label: 'Complete',
    color: 'text-green-500'
  },
  failed: {
    icon: XCircle,
    label: 'Failed',
    color: 'text-destructive'
  },
};
```

```tsx
// frontend/src/components/tasks/TaskCard.tsx - add to status indicators

import { ralphStatusConfig } from '@/utils/ralphStatus';

// In the TaskCard component, add Ralph status indicator:
{task.ralph_status !== 'none' && (() => {
  const config = ralphStatusConfig[task.ralph_status];
  const Icon = config.icon;
  return (
    <div
      className="flex items-center gap-1 cursor-pointer"
      title={config.label}
      onClick={(e) => {
        e.stopPropagation();
        if (task.ralph_status === 'awaitingapproval') {
          RalphPlanDialog.show({ taskId: task.id, taskTitle: task.title });
        }
      }}
    >
      <Icon className={cn('h-4 w-4', config.color, config.animate)} />
      <span className="text-xs text-muted-foreground">
        {config.label}
      </span>
    </div>
  );
})()}
```

**Acceptance Criteria:**
- [ ] Ralph status icon displays on task card when not `none`
- [ ] Planning shows animated Bot icon (purple)
- [ ] Awaiting approval shows FileCheck icon (orange) - clickable to open dialog
- [ ] Building shows animated Hammer icon (blue)
- [ ] Completed shows CheckCircle icon (green)
- [ ] Failed shows XCircle icon (red)
- [ ] Clicking "awaiting approval" opens the plan review dialog

---

### Spec 8: Ralph Actions in Task Dropdown Menu

**Goal:** Add Ralph-related actions to the task actions dropdown

**Files to modify:**
- `frontend/src/components/ui/actions-dropdown.tsx`

**Code Example:**
```tsx
// In ActionsDropdown component

import { Bot, FileText, RefreshCw, XCircle } from 'lucide-react';
import { RalphPlanDialog } from '@/components/dialogs/tasks/RalphPlanDialog';

// Add Ralph section to dropdown menu items:
<DropdownMenuSeparator />
<DropdownMenuLabel className="text-xs text-muted-foreground">Ralph</DropdownMenuLabel>

{task.ralph_status === 'none' && (
  <DropdownMenuItem onClick={() => handleStartRalph()}>
    <Bot className="mr-2 h-4 w-4" />
    Start Ralph
  </DropdownMenuItem>
)}

{task.ralph_status === 'awaitingapproval' && (
  <>
    <DropdownMenuItem onClick={() => RalphPlanDialog.show({ taskId: task.id, taskTitle: task.title })}>
      <FileText className="mr-2 h-4 w-4" />
      View Plan
    </DropdownMenuItem>
    <DropdownMenuItem onClick={() => handleRerunPlan()}>
      <RefreshCw className="mr-2 h-4 w-4" />
      Run Plan Again
    </DropdownMenuItem>
  </>
)}

{task.ralph_status !== 'none' && task.ralph_status !== 'completed' && (
  <DropdownMenuItem onClick={() => handleCancelRalph()} className="text-destructive">
    <XCircle className="mr-2 h-4 w-4" />
    Cancel Ralph
  </DropdownMenuItem>
)}

// Handler functions:
const handleStartRalph = async () => {
  try {
    const { workspace_id, process_id } = await tasksApi.startRalphPlan(task.id);
    // Optionally navigate to process view
  } catch (error) {
    toast.error('Failed to start Ralph');
  }
};

const handleRerunPlan = async () => {
  try {
    await tasksApi.rerunRalphPlan(task.id);
  } catch (error) {
    toast.error('Failed to rerun plan');
  }
};

const handleCancelRalph = async () => {
  try {
    await tasksApi.cancelRalph(task.id);
  } catch (error) {
    toast.error('Failed to cancel Ralph');
  }
};
```

**Acceptance Criteria:**
- [ ] "Start Ralph" action visible when ralph_status is `none`
- [ ] "View Plan" action visible when ralph_status is `awaitingapproval`
- [ ] "Run Plan Again" action visible when ralph_status is `awaitingapproval`
- [ ] "Cancel Ralph" action visible for all active Ralph states
- [ ] Actions call appropriate API functions
- [ ] Error handling with toast notifications

---

### Spec 9: Ralph Plan Approval Dialog

**Goal:** Create a dialog for reviewing and approving the implementation plan

**Files to modify:**
- `frontend/src/components/dialogs/tasks/RalphPlanDialog.tsx` (new)

**Code Example:**
```tsx
// frontend/src/components/dialogs/tasks/RalphPlanDialog.tsx

import { useState, useEffect } from 'react';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Loader2, RefreshCw, Play, XCircle } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { tasksApi } from '@/api/tasks';
import { toast } from 'sonner';

interface RalphPlanDialogProps {
  taskId: string;
  taskTitle: string;
}

type RalphPlanResult = 'approved' | 'replan' | 'canceled';

const RalphPlanDialogImpl = NiceModal.create<RalphPlanDialogProps>(({ taskId, taskTitle }) => {
  const modal = useModal();
  const [planContent, setPlanContent] = useState<string>('');
  const [isLoading, setIsLoading] = useState(true);
  const [isApproving, setIsApproving] = useState(false);
  const [isReplanning, setIsReplanning] = useState(false);
  const [isCanceling, setIsCanceling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadPlan();
  }, [taskId]);

  const loadPlan = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const content = await tasksApi.getRalphPlan(taskId);
      setPlanContent(content);
    } catch (err) {
      setError('Failed to load implementation plan');
    } finally {
      setIsLoading(false);
    }
  };

  const handleApprove = async () => {
    setIsApproving(true);
    try {
      await tasksApi.approveRalphPlan(taskId);
      toast.success('Build started');
      modal.resolve('approved');
      modal.hide();
    } catch (error) {
      toast.error('Failed to start build');
    } finally {
      setIsApproving(false);
    }
  };

  const handleReplan = async () => {
    setIsReplanning(true);
    try {
      await tasksApi.rerunRalphPlan(taskId);
      toast.success('Re-planning started');
      modal.resolve('replan');
      modal.hide();
    } catch (error) {
      toast.error('Failed to rerun plan');
    } finally {
      setIsReplanning(false);
    }
  };

  const handleCancel = async () => {
    setIsCanceling(true);
    try {
      await tasksApi.cancelRalph(taskId);
      toast.success('Ralph canceled');
      modal.resolve('canceled');
      modal.hide();
    } catch (error) {
      toast.error('Failed to cancel Ralph');
    } finally {
      setIsCanceling(false);
    }
  };

  const isAnyLoading = isApproving || isReplanning || isCanceling;

  return (
    <Dialog open={modal.visible} onOpenChange={() => !isAnyLoading && modal.hide()}>
      <DialogContent className="max-w-4xl h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>Implementation Plan</DialogTitle>
          <DialogDescription>{taskTitle}</DialogDescription>
        </DialogHeader>

        <ScrollArea className="flex-1 pr-4 border rounded-md p-4">
          {isLoading ? (
            <div className="flex items-center justify-center h-full">
              <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center h-full gap-2 text-destructive">
              <XCircle className="h-8 w-8" />
              <p>{error}</p>
              <Button variant="outline" size="sm" onClick={loadPlan}>
                Retry
              </Button>
            </div>
          ) : (
            <div className="prose prose-sm dark:prose-invert max-w-none">
              <ReactMarkdown>{planContent}</ReactMarkdown>
            </div>
          )}
        </ScrollArea>

        <DialogFooter className="gap-2 sm:gap-2">
          <Button
            variant="destructive"
            onClick={handleCancel}
            disabled={isAnyLoading}
          >
            {isCanceling && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            <XCircle className="mr-2 h-4 w-4" />
            Cancel Ralph
          </Button>

          <div className="flex-1" />

          <Button
            variant="outline"
            onClick={handleReplan}
            disabled={isAnyLoading}
          >
            {isReplanning && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            <RefreshCw className="mr-2 h-4 w-4" />
            Run Plan Again
          </Button>

          <Button
            onClick={handleApprove}
            disabled={isAnyLoading || !!error}
          >
            {isApproving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            <Play className="mr-2 h-4 w-4" />
            Approve & Build
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

export const RalphPlanDialog = defineModal<RalphPlanDialogProps, RalphPlanResult>(RalphPlanDialogImpl);
```

**Acceptance Criteria:**
- [ ] Dialog displays `IMPLEMENTATION_PLAN.md` content as rendered markdown
- [ ] Loading state while fetching plan content
- [ ] Error state with retry option
- [ ] "Approve & Build" button starts build phase
- [ ] "Run Plan Again" button re-executes planning
- [ ] "Cancel Ralph" button cancels and resets status
- [ ] All buttons disabled during async operations
- [ ] Toast notifications for success/error
- [ ] Dialog follows existing modal patterns (NiceModal)

---

### Spec 10: Real-time Ralph Status Updates

**Goal:** Ensure task cards update in real-time when Ralph status changes

**Files to modify:**
- `crates/db/src/models/task.rs` (query update)
- `crates/server/src/routes/tasks.rs` (streaming)

**Code Example:**
```rust
// Ensure the task streaming query includes ralph_status
// In the SQL query for TaskWithAttemptStatus:

SELECT
    t.*,
    t.ralph_status,  -- Include ralph_status
    -- ... existing attempt status fields
FROM tasks t
WHERE t.project_id = $1
```

**Acceptance Criteria:**
- [ ] `ralph_status` included in `TaskWithAttemptStatus` type
- [ ] WebSocket task stream broadcasts ralph_status changes
- [ ] Task cards update without page refresh
- [ ] Status transitions appear smoothly

---

## Implementation Order

1. **Spec 2** - Add Ralph Run Reason (simple enum addition)
2. **Spec 3** - Add Ralph Status to Task Model (database foundation)
3. **Spec 1** - Add Ralph Executor Type (depends on Spec 2)
4. **Spec 4** - Copy Ralph Folder During Worktree Setup
5. **Spec 5** - API Endpoints (depends on Specs 1-4)
6. **Spec 6** - Frontend API Client (depends on Spec 5)
7. **Spec 10** - Real-time Ralph Status Updates (backend streaming)
8. **Spec 7** - Ralph Status Indicator on Task Card (depends on Spec 6)
9. **Spec 8** - Ralph Actions in Task Dropdown Menu (depends on Spec 6)
10. **Spec 9** - Ralph Plan Approval Dialog (depends on Specs 6, 8)

---

## Overall Acceptance Criteria

- [ ] Ralph executor integrates with existing `CodingAgent` enum
- [ ] Plan and Build tracked as separate `ExecutionProcess` entries
- [ ] Tasks can be marked for Ralph execution from the UI
- [ ] Ralph plan mode executes and creates `IMPLEMENTATION_PLAN.md`
- [ ] User can view the implementation plan in a dialog
- [ ] User can **approve** the plan to start build mode
- [ ] User can **run plan again** to regenerate the implementation plan
- [ ] User can **cancel** Ralph execution at any point
- [ ] Ralph build mode executes after approval
- [ ] Visual feedback shows Ralph status on task cards at all stages
- [ ] Real-time updates reflect Ralph status changes via existing WebSocket
- [ ] Process output is streamed via existing infrastructure
- [ ] `.ralph` folder is copied to worktree as `.ralph-vibe-kanban`
- [ ] Task description (spec content) written to `.ralph-vibe-kanban/spec` file
- [ ] All unit tests pass
- [ ] TypeScript types are properly generated
- [ ] Code follows existing project patterns and conventions
