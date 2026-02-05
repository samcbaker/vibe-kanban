# Ralph Mode Specification (v1 - Simple)

## Overview

Ralph Mode enables AI-driven task execution using the existing `loop.sh` script. When a user creates a task, they can optionally enable Ralph Mode. Ralph executes in a **real macOS Terminal window** so users can watch the execution live.

**Important:** Ralph executes in the **task's worktree directory**, not the main repo. Each task in vibe-kanban has its own git worktree/branch, and Ralph runs there. This allows Ralph to be used across different projects.

**Key Simplifications for v1:**
- No new kanban lanes (task stays in its current column)
- Ralph status shown as a badge on the task card
- Opens real Terminal.app for visibility
- Fixed iteration limits: Plan=10, Build=20
- Only start/stop (no pause/resume)
- Runs in task's worktree (not main repo)

**Key Behavior:**
- **Ralph-enabled tasks skip normal Claude execution** - only Ralph runs, not the standard coding agent
- **Task auto-moves to "In Progress"** when Ralph starts running
- **Optional "Start immediately"** toggle on task creation to launch Ralph Plan right away

---

## User Flow

### Flow A: Create Task and Start Ralph Later
1. **Create Task** → User checks "Enable Ralph Mode" checkbox (leave "Start immediately" unchecked)
2. **Create Workspace** → User creates a workspace for the task (this creates the git worktree)
3. **Task Card** → Shows Ralph badge (task stays in "Todo")
4. **Click Badge** → Opens dialog with "Start Plan", "Start Build", "Stop", "Open Terminal" buttons
5. **Start Ralph** → Task moves to **"In Progress"**, Terminal opens with `loop.sh`
6. **Monitor** → User watches in Terminal OR clicks "Open Terminal" to focus the window
7. **Stop** → Creates `.ralph/STOP` file, task stays in "In Progress" (user moves manually when done)

### Flow B: Create Task and Start Ralph Immediately
1. **Create Task** → User checks both "Enable Ralph Mode" AND "Start immediately" checkboxes
2. **Auto-creates Workspace** → Workspace created automatically
3. **Auto-starts Ralph Plan** → Terminal opens with `loop.sh plan 10`
4. **Task moves to "In Progress"** → Automatically
5. **Monitor/Stop** → Same as Flow A

**Important behaviors:**
- **Ralph-enabled tasks do NOT run normal Claude coding agent** - clicking "Run" on a Ralph task opens the Ralph dialog instead
- **Task status auto-changes to "In Progress"** when Ralph starts (plan or build)
- **Normal Claude "Run" button is hidden** for Ralph-enabled tasks

**Note:** Ralph requires a workspace with a worktree. The **main repo** must have `.ralph/` folder with `loop.sh`, `PROMPT_plan.md`, and `PROMPT_build.md`. These files are automatically copied to the worktree on first run.

---

## Specs

### Spec 1: Database - Add `ralph_enabled` to Tasks

**Goal:** Track whether Ralph Mode is enabled for a task.

**Files to modify:**
- `crates/db/migrations/YYYYMMDDHHMMSS_add_ralph_enabled.sql` (new)
- `crates/db/src/models/task.rs`

**Code Example:**
```sql
-- Migration: Add ralph_enabled column
ALTER TABLE tasks ADD COLUMN ralph_enabled BOOLEAN NOT NULL DEFAULT FALSE;
```

```rust
// crates/db/src/models/task.rs

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Task {
    // ... existing fields
    pub ralph_enabled: bool, // NEW - whether Ralph Mode is enabled
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateTask {
    // ... existing fields
    pub ralph_enabled: Option<bool>, // NEW
}
```

**Acceptance Criteria:**
- [ ] `ralph_enabled` column added (default FALSE)
- [ ] Task struct updated with new field
- [ ] CreateTask accepts optional `ralph_enabled`
- [ ] SQLx offline cache updated
- [ ] TypeScript types regenerated

---

### Spec 2: Task Form - Ralph Toggle with Start Immediately Option

**Goal:** Add checkboxes for enabling Ralph Mode and optionally starting immediately.

**Files to modify:**
- `frontend/src/components/dialogs/tasks/TaskFormDialog.tsx`

**Code Example:**
```typescript
// In TaskFormDialog.tsx

const [ralphEnabled, setRalphEnabled] = useState(false);
const [startImmediately, setStartImmediately] = useState(false);

// In the form JSX:
<div className="space-y-2">
  <div className="flex items-center gap-2">
    <input
      type="checkbox"
      id="ralph-enabled"
      checked={ralphEnabled}
      onChange={(e) => {
        setRalphEnabled(e.target.checked);
        if (!e.target.checked) setStartImmediately(false);
      }}
    />
    <label htmlFor="ralph-enabled" className="text-sm">
      Enable Ralph Mode (AI-driven execution)
    </label>
  </div>

  {ralphEnabled && (
    <div className="flex items-center gap-2 ml-6">
      <input
        type="checkbox"
        id="start-immediately"
        checked={startImmediately}
        onChange={(e) => setStartImmediately(e.target.checked)}
      />
      <label htmlFor="start-immediately" className="text-sm text-gray-600">
        Start Ralph Plan immediately (opens Terminal)
      </label>
    </div>
  )}
</div>

// When submitting:
const createData: CreateTask = {
  // ... existing fields
  ralph_enabled: ralphEnabled,
  start_ralph_immediately: startImmediately,
};
```

**Acceptance Criteria:**
- [ ] "Enable Ralph Mode" checkbox appears in task creation form
- [ ] "Start immediately" checkbox only shows when Ralph is enabled
- [ ] Unchecking Ralph disables "Start immediately"
- [ ] Both values sent to API when creating task
- [ ] Default is both unchecked (false)

---

### Spec 3: Backend - Block Normal Execution for Ralph Tasks

**Goal:** Prevent normal Claude coding agent from running on Ralph-enabled tasks.

**Files to modify:**
- `crates/server/src/routes/sessions.rs` (or wherever coding agent execution is triggered)

**Code Example:**
```rust
// When attempting to start a coding agent session:

pub async fn start_coding_agent(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    Json(request): Json<StartCodingAgentRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    let task = Task::find_by_id(&state.pool, task_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Block normal execution for Ralph-enabled tasks
    if task.ralph_enabled {
        warn!("[Session] Blocked normal execution for Ralph-enabled task {}", task_id);
        return Err(ApiError::BadRequest(
            "This task uses Ralph Mode. Use the Ralph controls to run it.".into()
        ));
    }

    // ... continue with normal coding agent logic
}
```

**Acceptance Criteria:**
- [ ] Normal coding agent execution returns error for Ralph-enabled tasks
- [ ] Error message explains to use Ralph controls instead
- [ ] Logged with warning level

---

### Spec 4: Backend - Auto-Update Task Status When Ralph Starts

**Goal:** Automatically move task to "In Progress" when Ralph starts.

**Files to modify:**
- `crates/server/src/routes/ralph.rs`

**Code Example:**
```rust
// In start_plan and start_build functions, after validation:

// Update task status to InProgress
Task::update_status(&state.pool, task_id, TaskStatus::InProgress)
    .await
    .map_err(|e| {
        error!("[Ralph] Failed to update task status: {}", e);
        ApiError::Internal(format!("Failed to update task status: {}", e))
    })?;

info!("[Ralph] Task {} moved to InProgress", task_id);
```

**Acceptance Criteria:**
- [ ] Task status changes to "InProgress" when Ralph Plan starts
- [ ] Task status changes to "InProgress" when Ralph Build starts
- [ ] Status update logged
- [ ] Frontend receives update via WebSocket (existing task update mechanism)

---

### Spec 5: Backend - Handle Start Immediately on Task Creation

**Goal:** When task is created with `start_ralph_immediately=true`, auto-create workspace and start Ralph Plan.

**Files to modify:**
- `crates/server/src/routes/tasks.rs`

**Code Example:**
```rust
// In create_task handler:

pub async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    // Create the task
    let task = Task::create(&state.pool, &request.task_data, task_id).await?;

    // If Ralph + Start Immediately, auto-setup and start
    if request.ralph_enabled && request.start_ralph_immediately {
        info!("[Ralph] Auto-starting Ralph for new task {}", task.id);

        // 1. Auto-create workspace (uses default settings)
        let workspace = Workspace::create_for_task(&state.pool, task.id, &request.project_id)
            .await
            .map_err(|e| {
                error!("[Ralph] Failed to auto-create workspace: {}", e);
                ApiError::Internal(format!("Failed to create workspace: {}", e))
            })?;

        // 2. Start Ralph Plan (this also updates status to InProgress)
        start_ralph_plan_internal(&state, task.id).await.map_err(|e| {
            error!("[Ralph] Failed to auto-start Ralph: {}", e);
            // Don't fail task creation, just log the error
            // Task is created, user can manually start Ralph
        }).ok();

        info!("[Ralph] Auto-started Ralph Plan for task {}", task.id);
    }

    Ok(Json(TaskResponse { task }))
}
```

**Acceptance Criteria:**
- [ ] `start_ralph_immediately` field added to CreateTask request
- [ ] Workspace auto-created when flag is true
- [ ] Ralph Plan auto-starts after workspace creation
- [ ] Task status set to InProgress
- [ ] Terminal opens with Ralph Plan
- [ ] If auto-start fails, task is still created (user can retry manually)

---

### Spec 6: Task Card - Ralph Badge & Hide Normal Run Button

**Goal:** Show Ralph badge on task cards and hide/replace normal "Run" button for Ralph tasks.

**Files to modify:**
- Task card component (likely in `frontend/src/components/tasks/`)

**Code Example:**
```typescript
// Ralph badge component
function RalphBadge({ task, onClick }: { task: Task; onClick: () => void }) {
  if (!task.ralph_enabled) return null;

  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className="flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-purple-100 text-purple-700 hover:bg-purple-200"
    >
      <span>🤖</span>
      <span>Ralph</span>
    </button>
  );
}

// In task card - conditionally show Run button or Ralph badge:
{task.ralph_enabled ? (
  <RalphBadge task={task} onClick={() => openRalphDialog(task)} />
) : (
  <RunButton task={task} onClick={() => startCodingAgent(task)} />
)}
```

**Acceptance Criteria:**
- [ ] Badge only shows when `ralph_enabled` is true
- [ ] **Normal "Run" button is hidden** for Ralph-enabled tasks
- [ ] Badge is clickable and opens Ralph control dialog
- [ ] Badge doesn't interfere with task card drag/drop
- [ ] Badge uses purple color scheme (Ralph's brand)

---

### Spec 7: Ralph Control Dialog (Simple)

**Goal:** Dialog with buttons to start/stop Ralph and open Terminal.

**Files to modify:**
- `frontend/src/components/dialogs/tasks/RalphControlDialog.tsx` (new)

**Code Example:**
```typescript
import { defineModal } from '@/components/dialogs/modal-utils';
import { toast } from 'sonner'; // or your toast library
import { ralphApi } from '@/lib/api';

interface RalphControlDialogProps {
  task: Task;
}

export const RalphControlDialog = defineModal<RalphControlDialogProps, void>(
  ({ task, resolve }) => {
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleAction = async (
      action: () => Promise<{ data: RalphResponse }>,
      successMessage: string
    ) => {
      setLoading(true);
      setError(null);

      try {
        console.log(`[Ralph] Executing action for task ${task.id}`);
        const { data } = await action();

        if (data.success) {
          console.log(`[Ralph] Success: ${data.message}`);
          toast.success(successMessage);
          resolve();
        } else {
          console.error(`[Ralph] Failed: ${data.message}`);
          setError(data.message);
          toast.error(data.message);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        console.error(`[Ralph] Error:`, err);
        setError(message);
        toast.error(`Ralph error: ${message}`);
      } finally {
        setLoading(false);
      }
    };

    const startPlan = () =>
      handleAction(
        () => ralphApi.startPlan(task.id),
        'Plan mode started - check Terminal'
      );

    const startBuild = () =>
      handleAction(
        () => ralphApi.startBuild(task.id),
        'Build mode started - check Terminal'
      );

    const stop = () =>
      handleAction(
        () => ralphApi.stop(task.id),
        'Stop signal sent'
      );

    const openTerminal = async () => {
      try {
        console.log('[Ralph] Opening Terminal');
        await ralphApi.openTerminal(task.id);
      } catch (err) {
        console.error('[Ralph] Failed to open Terminal:', err);
        toast.error('Failed to open Terminal');
      }
    };

    return (
      <Dialog>
        <DialogHeader>
          <DialogTitle>Ralph Mode - {task.title}</DialogTitle>
        </DialogHeader>
        <DialogContent>
          <p className="text-sm text-gray-600 mb-4">
            Ralph will open in a Terminal window. Plan mode analyzes and creates
            an implementation plan. Build mode executes the plan.
          </p>

          {error && (
            <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded text-red-700 text-sm">
              <strong>Error:</strong> {error}
            </div>
          )}

          <div className="flex flex-col gap-2">
            <Button onClick={startPlan} disabled={loading}>
              {loading ? '...' : '🎯 Start Plan (max 10 iterations)'}
            </Button>
            <Button onClick={startBuild} disabled={loading}>
              {loading ? '...' : '🔨 Start Build (max 20 iterations)'}
            </Button>
            <Button variant="secondary" onClick={openTerminal} disabled={loading}>
              📺 Open Terminal
            </Button>
            <Button variant="destructive" onClick={stop} disabled={loading}>
              ⏹️ Stop Ralph
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    );
  }
);
```

**Acceptance Criteria:**
- [ ] Dialog shows task title
- [ ] Four buttons: Start Plan, Start Build, Open Terminal, Stop
- [ ] Buttons explain iteration limits
- [ ] Follows existing dialog patterns
- [ ] Console logs for all actions (`[Ralph] ...`)
- [ ] Error state shown in dialog when action fails
- [ ] Toast notifications for success/error
- [ ] Loading state disables buttons during action

---

### Spec 8: Backend API - Ralph Control

**Goal:** API endpoints to start/stop Ralph and open Terminal.

**Files to modify:**
- `crates/server/src/routes/ralph.rs` (new)
- `crates/server/src/routes/mod.rs`
- `crates/server/src/lib.rs`

**Code Example:**
```rust
// crates/server/src/routes/ralph.rs

use axum::{extract::{Path, State}, routing::post, Json, Router};
use std::process::Command;
use tracing::{info, error, warn};

/// POST /api/tasks/:task_id/ralph/start-plan
/// Opens Terminal.app running loop.sh in the task's WORKTREE directory
pub async fn start_plan(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<RalphResponse>, ApiError> {
    info!("[Ralph] Starting plan mode for task {}", task_id);

    let task = Task::find_by_id(&state.pool, task_id)
        .await
        .map_err(|e| {
            error!("[Ralph] Database error finding task {}: {}", task_id, e);
            ApiError::Internal(format!("Database error: {}", e))
        })?
        .ok_or_else(|| {
            warn!("[Ralph] Task {} not found", task_id);
            ApiError::NotFound
        })?;

    if !task.ralph_enabled {
        warn!("[Ralph] Ralph not enabled for task {}", task_id);
        return Err(ApiError::BadRequest("Ralph not enabled for this task".into()));
    }

    // Get the WORKTREE path for this task (not the main repo!)
    // Each task has a workspace with its own git worktree
    let worktree_path = get_worktree_path(&state.pool, task_id).await.map_err(|e| {
        error!("[Ralph] Failed to get worktree path for task {}: {}", task_id, e);
        ApiError::Internal(format!("Failed to get worktree path: {}", e))
    })?;

    // Get main repo path (source for .ralph folder)
    let main_repo_path = get_main_repo_path(&state.pool, task_id).await.map_err(|e| {
        error!("[Ralph] Failed to get main repo path: {}", e);
        ApiError::Internal(format!("Failed to get main repo path: {}", e))
    })?;

    info!("[Ralph] Using worktree at {:?}", worktree_path);
    info!("[Ralph] Main repo at {:?}", main_repo_path);

    // Ensure .ralph folder exists in worktree (copy from main repo if missing)
    ensure_ralph_folder(&worktree_path, &main_repo_path).map_err(|e| {
        error!("[Ralph] Failed to ensure .ralph folder: {}", e);
        ApiError::BadRequest(e)
    })?;

    // Validate .ralph/loop.sh exists (should exist after ensure_ralph_folder)
    let script_path = worktree_path.join(".ralph/loop.sh");
    if !script_path.exists() {
        error!("[Ralph] Script not found at {:?}", script_path);
        return Err(ApiError::BadRequest(format!(
            "Ralph script not found at {} - ensure .ralph/loop.sh exists in the main repo",
            script_path.display()
        )));
    }

    // Open Terminal.app with the command in the WORKTREE
    open_terminal_with_command(&worktree_path, "plan", 10).map_err(|e| {
        error!("[Ralph] Failed to open Terminal for task {}: {}", task_id, e);
        ApiError::Internal(format!("Failed to open Terminal: {}", e))
    })?;

    info!("[Ralph] Successfully started plan mode for task {} in worktree {:?}", task_id, worktree_path);
    Ok(Json(RalphResponse { success: true, message: "Plan mode started in worktree".into() }))
}

/// POST /api/tasks/:task_id/ralph/start-build
pub async fn start_build(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<RalphResponse>, ApiError> {
    info!("[Ralph] Starting build mode for task {}", task_id);

    let task = Task::find_by_id(&state.pool, task_id)
        .await
        .map_err(|e| {
            error!("[Ralph] Database error: {}", e);
            ApiError::Internal(format!("Database error: {}", e))
        })?
        .ok_or(ApiError::NotFound)?;

    if !task.ralph_enabled {
        warn!("[Ralph] Ralph not enabled for task {}", task_id);
        return Err(ApiError::BadRequest("Ralph not enabled for this task".into()));
    }

    // Get the WORKTREE path for this task
    let worktree_path = get_worktree_path(&state.pool, task_id).await.map_err(|e| {
        error!("[Ralph] Failed to get worktree path: {}", e);
        ApiError::Internal(format!("Failed to get worktree path: {}", e))
    })?;

    // Get main repo path (source for .ralph folder)
    let main_repo_path = get_main_repo_path(&state.pool, task_id).await.map_err(|e| {
        error!("[Ralph] Failed to get main repo path: {}", e);
        ApiError::Internal(format!("Failed to get main repo path: {}", e))
    })?;

    info!("[Ralph] Using worktree at {:?}", worktree_path);

    // Ensure .ralph folder exists in worktree (copy from main repo if missing)
    ensure_ralph_folder(&worktree_path, &main_repo_path).map_err(|e| {
        error!("[Ralph] Failed to ensure .ralph folder: {}", e);
        ApiError::BadRequest(e)
    })?;

    // Validate script exists in worktree
    let script_path = worktree_path.join(".ralph/loop.sh");
    if !script_path.exists() {
        error!("[Ralph] Script not found at {:?}", script_path);
        return Err(ApiError::BadRequest("Ralph script not found in worktree".into()));
    }

    open_terminal_with_command(&worktree_path, "build", 20).map_err(|e| {
        error!("[Ralph] Failed to open Terminal: {}", e);
        ApiError::Internal(format!("Failed to open Terminal: {}", e))
    })?;

    info!("[Ralph] Successfully started build mode for task {} in worktree {:?}", task_id, worktree_path);
    Ok(Json(RalphResponse { success: true, message: "Build mode started in worktree".into() }))
}

/// POST /api/tasks/:task_id/ralph/stop
/// Creates .ralph/STOP file in the task's worktree to signal the loop to exit
pub async fn stop(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<RalphResponse>, ApiError> {
    info!("[Ralph] Stopping Ralph for task {}", task_id);

    // Get the WORKTREE path for this task
    let worktree_path = get_worktree_path(&state.pool, task_id).await.map_err(|e| {
        error!("[Ralph] Failed to get worktree path: {}", e);
        ApiError::Internal(format!("Failed to get worktree path: {}", e))
    })?;

    let stop_file = worktree_path.join(".ralph/STOP");
    std::fs::write(&stop_file, "").map_err(|e| {
        error!("[Ralph] Failed to create STOP file at {:?}: {}", stop_file, e);
        ApiError::Internal(format!("Failed to create STOP file: {}", e))
    })?;

    info!("[Ralph] Created STOP file at {:?}", stop_file);
    Ok(Json(RalphResponse { success: true, message: "Stop signal sent to worktree".into() }))
}

/// POST /api/tasks/:task_id/ralph/open-terminal
/// Focuses existing Terminal window or opens new one
pub async fn open_terminal(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<RalphResponse>, ApiError> {
    info!("[Ralph] Opening Terminal for task {}", task_id);

    let output = Command::new("osascript")
        .args(["-e", "tell application \"Terminal\" to activate"])
        .output()
        .map_err(|e| {
            error!("[Ralph] Failed to run osascript: {}", e);
            ApiError::Internal(format!("Failed to open Terminal: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("[Ralph] osascript failed: {}", stderr);
        return Err(ApiError::Internal(format!("Terminal activation failed: {}", stderr)));
    }

    Ok(Json(RalphResponse { success: true, message: "Terminal opened".into() }))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct RalphResponse {
    pub success: bool,
    pub message: String,
}

/// Get the worktree path for a task
/// Task -> Workspace -> WorkspaceRepo -> Repo (with worktree path)
async fn get_worktree_path(pool: &SqlitePool, task_id: Uuid) -> Result<PathBuf, String> {
    // 1. Find the workspace for this task
    let workspace = Workspace::find_by_task_id(pool, task_id)
        .await
        .map_err(|e| format!("Database error finding workspace: {}", e))?
        .ok_or_else(|| "No workspace found for task - create a workspace first".to_string())?;

    // 2. Get the workspace repos (contains worktree info)
    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id)
        .await
        .map_err(|e| format!("Database error finding workspace repos: {}", e))?;

    let workspace_repo = workspace_repos
        .first()
        .ok_or_else(|| "No repo associated with workspace".to_string())?;

    // 3. The worktree_path is stored on the workspace_repo
    let worktree_path = workspace_repo
        .worktree_path
        .as_ref()
        .ok_or_else(|| "Worktree path not set for workspace repo".to_string())?;

    let path = PathBuf::from(worktree_path);

    if !path.exists() {
        return Err(format!("Worktree path does not exist: {}", path.display()));
    }

    Ok(path)
}

/// Get the main repo path (source for .ralph folder)
async fn get_main_repo_path(pool: &SqlitePool, task_id: Uuid) -> Result<PathBuf, String> {
    let workspace = Workspace::find_by_task_id(pool, task_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| "No workspace found".to_string())?;

    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let workspace_repo = workspace_repos
        .first()
        .ok_or_else(|| "No repo associated".to_string())?;

    // Get the main repo path (not worktree)
    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| "Repo not found".to_string())?;

    Ok(PathBuf::from(&repo.path))
}

/// Ensure .ralph folder exists in worktree, copy from main repo if missing
fn ensure_ralph_folder(worktree_path: &Path, main_repo_path: &Path) -> Result<(), String> {
    let worktree_ralph = worktree_path.join(".ralph");
    let main_ralph = main_repo_path.join(".ralph");

    // If .ralph already exists in worktree, we're good
    if worktree_ralph.exists() {
        info!("[Ralph] .ralph folder already exists in worktree");
        return Ok(());
    }

    // Check if .ralph exists in main repo
    if !main_ralph.exists() {
        return Err(format!(
            "No .ralph folder found in main repo at {}. \
             Please create .ralph/loop.sh, PROMPT_plan.md, and PROMPT_build.md first.",
            main_ralph.display()
        ));
    }

    info!("[Ralph] Copying .ralph folder from main repo to worktree");
    info!("[Ralph]   From: {:?}", main_ralph);
    info!("[Ralph]   To: {:?}", worktree_ralph);

    // Copy entire .ralph directory recursively
    copy_dir_recursive(&main_ralph, &worktree_ralph).map_err(|e| {
        format!("Failed to copy .ralph folder: {}", e)
    })?;

    info!("[Ralph] Successfully copied .ralph folder to worktree");
    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn open_terminal_with_command(worktree_path: &Path, mode: &str, iterations: u32) -> Result<(), String> {
    let script_path = worktree_path.join(".ralph/loop.sh");
    let cmd = format!(
        "cd {} && bash {} {} {}",
        worktree_path.display(),
        script_path.display(),
        mode,
        iterations
    );

    info!("[Ralph] Executing in worktree: {}", worktree_path.display());
    info!("[Ralph] Command: {}", cmd);

    // Use AppleScript to open Terminal.app with command
    let applescript = format!(
        r#"tell application "Terminal"
            activate
            do script "{}"
        end tell"#,
        cmd.replace("\"", "\\\"")
    );

    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| format!("Failed to execute osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("[Ralph] AppleScript failed: {}", stderr);
        return Err(format!("AppleScript error: {}", stderr));
    }

    info!("[Ralph] Terminal opened successfully");
    Ok(())
}

pub fn ralph_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks/:task_id/ralph/start-plan", post(start_plan))
        .route("/tasks/:task_id/ralph/start-build", post(start_build))
        .route("/tasks/:task_id/ralph/stop", post(stop))
        .route("/tasks/:task_id/ralph/open-terminal", post(open_terminal))
}
```

**Acceptance Criteria:**
- [ ] `POST /api/tasks/:id/ralph/start-plan` opens Terminal with `loop.sh plan 10` **in task's worktree**
- [ ] `POST /api/tasks/:id/ralph/start-build` opens Terminal with `loop.sh build 20` **in task's worktree**
- [ ] `POST /api/tasks/:id/ralph/stop` creates `.ralph/STOP` file **in task's worktree**
- [ ] `POST /api/tasks/:id/ralph/open-terminal` focuses Terminal.app
- [ ] Returns error if `ralph_enabled` is false
- [ ] Returns error if task has no workspace/worktree
- [ ] **Auto-copies `.ralph` folder from main repo to worktree if missing**
- [ ] All actions logged with `[Ralph]` prefix using `tracing`
- [ ] Validates `.ralph/loop.sh` exists after copy
- [ ] Returns `RalphResponse` with success boolean and message
- [ ] Errors include descriptive messages for debugging

---

### Spec 9: Frontend API Client

**Goal:** Add Ralph API methods to frontend.

**Files to modify:**
- `frontend/src/lib/api.ts`

**Code Example:**
```typescript
import { RalphResponse } from '../../shared/types';

export const ralphApi = {
  startPlan: (taskId: string) =>
    apiClient.post<RalphResponse>(`/tasks/${taskId}/ralph/start-plan`),

  startBuild: (taskId: string) =>
    apiClient.post<RalphResponse>(`/tasks/${taskId}/ralph/start-build`),

  stop: (taskId: string) =>
    apiClient.post<RalphResponse>(`/tasks/${taskId}/ralph/stop`),

  openTerminal: (taskId: string) =>
    apiClient.post<RalphResponse>(`/tasks/${taskId}/ralph/open-terminal`),
};
```

**Acceptance Criteria:**
- [ ] All four endpoints available
- [ ] Returns typed `RalphResponse` with success/message
- [ ] Follows existing API client patterns

---

## Implementation Order

1. **Spec 1** - Database migration (no dependencies)
2. **Spec 2** - Task form with Ralph toggles (depends on Spec 1)
3. **Spec 3** - Block normal execution for Ralph tasks (depends on Spec 1)
4. **Spec 4** - Auto-update task status when Ralph starts (depends on Spec 1)
5. **Spec 8** - Backend API routes (depends on Specs 1, 4)
6. **Spec 5** - Handle start immediately on task creation (depends on Specs 1, 8)
7. **Spec 9** - Frontend API client (depends on Spec 8)
8. **Spec 6** - Task card badge & hide Run button (depends on Spec 1)
9. **Spec 7** - Ralph control dialog (depends on Specs 6, 8, 9)

---

## Overall Acceptance Criteria

### Core Functionality
- [ ] User can enable Ralph Mode when creating a task
- [ ] User can optionally "Start immediately" to auto-launch Ralph Plan
- [ ] Task card shows Ralph badge when enabled
- [ ] **Normal "Run" button is hidden** for Ralph-enabled tasks
- [ ] Clicking badge opens control dialog
- [ ] "Start Plan" opens Terminal with `loop.sh plan 10` **in task's worktree**
- [ ] "Start Build" opens Terminal with `loop.sh build 20` **in task's worktree**
- [ ] "Stop" creates STOP file **in worktree** and loop exits
- [ ] "Open Terminal" focuses Terminal.app window
- [ ] Works with any repo that has `.ralph/loop.sh` (not just vibe-kanban)
- [ ] **Auto-copies `.ralph/` folder from main repo to worktree if missing**

### Task Status Behavior
- [ ] **Ralph-enabled tasks cannot run normal Claude coding agent** (returns error)
- [ ] **Task auto-moves to "In Progress"** when Ralph starts (plan or build)
- [ ] No new kanban lanes needed (just badge indicator)

### Start Immediately Flow
- [ ] When "Start immediately" is checked, workspace is auto-created
- [ ] Ralph Plan auto-starts after workspace creation
- [ ] Terminal opens automatically
- [ ] Task status set to "In Progress"
- [ ] If auto-start fails, task is still created (user can retry)

### Error Monitoring
- [ ] Backend logs all Ralph actions with `[Ralph]` prefix (visible in server console)
- [ ] Frontend logs all actions to browser console with `[Ralph]` prefix
- [ ] Attempting normal run on Ralph task shows: "Use Ralph controls instead"
- [ ] Missing workspace shows clear error: "Create a workspace first"
- [ ] Missing `.ralph/` in main repo shows: "Please create .ralph/ folder with loop.sh first"
- [ ] Copy operation logged: "Copying .ralph folder from main repo to worktree"
- [ ] AppleScript/Terminal failures show user-friendly toast
- [ ] Failed actions display error in dialog UI

---

## Future Enhancements (v2)

- Track Ralph execution state in DB (Planning/Building/Stopped)
- Show Ralph output in web UI (WebSocket streaming)
- Pause/Resume functionality
- Configurable iteration limits
- Support for Linux/Windows (currently macOS only with Terminal.app)
