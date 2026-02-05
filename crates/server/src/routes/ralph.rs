use axum::{
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use db::models::{
    repo::Repo,
    task::Task,
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use std::path::{Path as StdPath, PathBuf};
use std::process::Command;
use tracing::{error, info, warn};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{error::ApiError, DeploymentImpl};

/// Response for all Ralph API endpoints
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RalphResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/tasks/:task_id/ralph/start-plan
pub async fn start_plan(
    State(deployment): State<DeploymentImpl>,
    Path(task_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<RalphResponse>>, ApiError> {
    info!("[Ralph] Starting plan mode for task {}", task_id);
    start_ralph_mode(&deployment, task_id, "plan", 10).await
}

/// POST /api/tasks/:task_id/ralph/start-build
pub async fn start_build(
    State(deployment): State<DeploymentImpl>,
    Path(task_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<RalphResponse>>, ApiError> {
    info!("[Ralph] Starting build mode for task {}", task_id);
    start_ralph_mode(&deployment, task_id, "build", 20).await
}

/// POST /api/tasks/:task_id/ralph/stop
pub async fn stop(
    State(deployment): State<DeploymentImpl>,
    Path(task_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<RalphResponse>>, ApiError> {
    info!("[Ralph] Stopping Ralph for task {}", task_id);

    let pool = &deployment.db().pool;
    let (worktree_path, _) = get_paths(pool, task_id).await?;

    let stop_file = worktree_path.join(".ralph/STOP");
    std::fs::write(&stop_file, "").map_err(|e| {
        error!("[Ralph] Failed to create STOP file at {:?}: {}", stop_file, e);
        ApiError::Io(e)
    })?;

    info!("[Ralph] Created STOP file at {:?}", stop_file);
    Ok(ResponseJson(ApiResponse::success(RalphResponse {
        success: true,
        message: "Stop signal sent to worktree".into(),
    })))
}

/// POST /api/tasks/:task_id/ralph/open-terminal
pub async fn open_terminal(
    State(_deployment): State<DeploymentImpl>,
    Path(task_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<RalphResponse>>, ApiError> {
    info!("[Ralph] Opening Terminal for task {}", task_id);

    let output = Command::new("osascript")
        .args(["-e", "tell application \"Terminal\" to activate"])
        .output()
        .map_err(|e| {
            error!("[Ralph] Failed to run osascript: {}", e);
            ApiError::Io(e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("[Ralph] osascript failed: {}", stderr);
        return Err(ApiError::BadRequest(format!(
            "Terminal activation failed: {}",
            stderr
        )));
    }

    Ok(ResponseJson(ApiResponse::success(RalphResponse {
        success: true,
        message: "Terminal opened".into(),
    })))
}

// Helper: Start Ralph in specified mode
async fn start_ralph_mode(
    deployment: &DeploymentImpl,
    task_id: Uuid,
    mode: &str,
    iterations: u32,
) -> Result<ResponseJson<ApiResponse<RalphResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // 1. Validate task exists and has ralph_enabled
    let task = Task::find_by_id(pool, task_id)
        .await
        .map_err(|e| {
            error!("[Ralph] Database error finding task {}: {}", task_id, e);
            ApiError::Database(e)
        })?
        .ok_or_else(|| {
            warn!("[Ralph] Task {} not found", task_id);
            ApiError::BadRequest(format!("Task {} not found", task_id))
        })?;

    if !task.ralph_enabled {
        warn!("[Ralph] Ralph not enabled for task {}", task_id);
        return Err(ApiError::BadRequest(
            "Ralph not enabled for this task".into(),
        ));
    }

    // 2. Get worktree and main repo paths
    let (worktree_path, main_repo_path) = get_paths(pool, task_id).await?;

    info!("[Ralph] Using worktree at {:?}", worktree_path);
    info!("[Ralph] Main repo at {:?}", main_repo_path);

    // 3. Ensure .ralph folder exists in worktree (copy from main repo if missing)
    ensure_ralph_folder(&worktree_path, &main_repo_path)?;

    // 4. Write task spec to worktree so Ralph can use it for planning
    write_task_spec(&worktree_path, &task)?;

    // 5. Validate .ralph/loop.sh exists
    let script_path = worktree_path.join(".ralph/loop.sh");
    if !script_path.exists() {
        error!("[Ralph] Script not found at {:?}", script_path);
        return Err(ApiError::BadRequest(format!(
            "Ralph script not found at {} - ensure .ralph/loop.sh exists in the main repo",
            script_path.display()
        )));
    }

    // 6. Open Terminal.app with the command
    open_terminal_with_command(&worktree_path, mode, iterations)?;

    info!(
        "[Ralph] Successfully started {} mode for task {} in worktree {:?}",
        mode, task_id, worktree_path
    );

    Ok(ResponseJson(ApiResponse::success(RalphResponse {
        success: true,
        message: format!("{} mode started in worktree", mode),
    })))
}

/// Get worktree path and main repo path for a task
/// IMPORTANT: Worktree path = container_ref / repo.name (NOT just container_ref)
async fn get_paths(
    pool: &sqlx::SqlitePool,
    task_id: Uuid,
) -> Result<(PathBuf, PathBuf), ApiError> {
    // 1. Find workspace for this task using fetch_all with task_id filter
    let workspaces = Workspace::fetch_all(pool, Some(task_id))
        .await
        .map_err(|e| {
            error!("[Ralph] Failed to fetch workspaces: {}", e);
            ApiError::Workspace(e)
        })?;

    let workspace = workspaces.first().ok_or_else(|| {
        warn!("[Ralph] No workspace found for task {}", task_id);
        ApiError::BadRequest("No workspace found for task - create a workspace first".into())
    })?;

    // 2. Get workspace container_ref (root directory for this workspace)
    let container_ref = workspace.container_ref.as_ref().ok_or_else(|| {
        warn!("[Ralph] Workspace {} has no container_ref", workspace.id);
        ApiError::BadRequest("Workspace has no container path".into())
    })?;

    // 3. Get workspace repos to find the repo name
    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id)
        .await
        .map_err(|e| {
            error!("[Ralph] Failed to find workspace repos: {}", e);
            ApiError::Database(e)
        })?;

    let workspace_repo = workspace_repos.first().ok_or_else(|| {
        warn!("[Ralph] No repo associated with workspace {}", workspace.id);
        ApiError::BadRequest("No repo associated with workspace".into())
    })?;

    // 4. Get the repo to get its name and path
    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await
        .map_err(|e| {
            error!("[Ralph] Failed to find repo: {}", e);
            ApiError::Database(e)
        })?
        .ok_or_else(|| {
            warn!("[Ralph] Repo {} not found", workspace_repo.repo_id);
            ApiError::BadRequest("Repo not found".into())
        })?;

    // 5. Actual worktree path = container_ref / repo.name
    let worktree = PathBuf::from(container_ref).join(&repo.name);
    if !worktree.exists() {
        error!("[Ralph] Worktree path does not exist: {:?}", worktree);
        return Err(ApiError::BadRequest(format!(
            "Worktree path does not exist: {}",
            worktree.display()
        )));
    }

    // 6. Main repo path from Repo.path
    Ok((worktree, repo.path.clone()))
}

/// Write the task spec (title + description) to .ralph/specs/task-spec.md in the worktree
fn write_task_spec(worktree_path: &StdPath, task: &Task) -> Result<(), ApiError> {
    let specs_dir = worktree_path.join(".ralph/specs");
    std::fs::create_dir_all(&specs_dir).map_err(|e| {
        error!("[Ralph] Failed to create .ralph/specs dir at {:?}: {}", specs_dir, e);
        ApiError::Io(e)
    })?;

    let spec_path = specs_dir.join("task-spec.md");

    let mut content = format!("# {}\n", task.title);
    if let Some(ref desc) = task.description {
        content.push_str(&format!("\n{}\n", desc));
    }

    std::fs::write(&spec_path, &content).map_err(|e| {
        error!("[Ralph] Failed to write task-spec.md at {:?}: {}", spec_path, e);
        ApiError::Io(e)
    })?;

    info!("[Ralph] Wrote task spec to {:?}", spec_path);
    Ok(())
}

// Helper: Ensure .ralph folder exists in worktree, copy from main repo if missing
fn ensure_ralph_folder(worktree_path: &StdPath, main_repo_path: &StdPath) -> Result<(), ApiError> {
    let worktree_ralph = worktree_path.join(".ralph");
    let main_ralph = main_repo_path.join(".ralph");

    // If .ralph already exists in worktree, we're good
    if worktree_ralph.exists() {
        info!("[Ralph] .ralph folder already exists in worktree");
        return Ok(());
    }

    // Check if .ralph exists in main repo
    if !main_ralph.exists() {
        error!("[Ralph] No .ralph folder in main repo at {:?}", main_ralph);
        return Err(ApiError::BadRequest(format!(
            "No .ralph folder found in main repo at {}. \
             Please create .ralph/loop.sh, PROMPT_plan.md, and PROMPT_build.md first.",
            main_ralph.display()
        )));
    }

    info!("[Ralph] Copying .ralph folder from main repo to worktree");
    info!("[Ralph]   From: {:?}", main_ralph);
    info!("[Ralph]   To: {:?}", worktree_ralph);

    // Copy entire .ralph directory recursively
    copy_dir_recursive(&main_ralph, &worktree_ralph).map_err(|e| {
        error!("[Ralph] Failed to copy .ralph folder: {}", e);
        ApiError::Io(e)
    })?;

    info!("[Ralph] Successfully copied .ralph folder to worktree");
    Ok(())
}

// Helper: Recursively copy a directory
fn copy_dir_recursive(src: &StdPath, dst: &StdPath) -> std::io::Result<()> {
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

// Helper: Open Terminal.app with command
fn open_terminal_with_command(
    worktree_path: &StdPath,
    mode: &str,
    iterations: u32,
) -> Result<(), ApiError> {
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
        cmd.replace('\"', "\\\"")
    );

    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| {
            error!("[Ralph] Failed to execute osascript: {}", e);
            ApiError::Io(e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("[Ralph] AppleScript failed: {}", stderr);
        return Err(ApiError::BadRequest(format!("AppleScript error: {}", stderr)));
    }

    info!("[Ralph] Terminal opened successfully");
    Ok(())
}

/// Start Ralph plan mode for a task (used internally by create_task_and_start_ralph)
pub async fn start_ralph_for_task(
    deployment: &DeploymentImpl,
    task_id: Uuid,
) -> Result<(), crate::error::ApiError> {
    info!("[Ralph] Auto-starting plan mode for task {}", task_id);

    let pool = &deployment.db().pool;

    // Validate task exists and has ralph_enabled
    let task = Task::find_by_id(pool, task_id)
        .await
        .map_err(|e| {
            error!("[Ralph] Database error finding task {}: {}", task_id, e);
            crate::error::ApiError::Database(e)
        })?
        .ok_or_else(|| {
            warn!("[Ralph] Task {} not found", task_id);
            crate::error::ApiError::BadRequest(format!("Task {} not found", task_id))
        })?;

    if !task.ralph_enabled {
        warn!("[Ralph] Ralph not enabled for task {}", task_id);
        return Err(crate::error::ApiError::BadRequest(
            "Ralph not enabled for this task".into(),
        ));
    }

    // Get worktree and main repo paths
    let (worktree_path, main_repo_path) = get_paths(pool, task_id).await?;

    info!("[Ralph] Using worktree at {:?}", worktree_path);
    info!("[Ralph] Main repo at {:?}", main_repo_path);

    // Ensure .ralph folder exists in worktree (copy from main repo if missing)
    ensure_ralph_folder(&worktree_path, &main_repo_path)?;

    // Write task spec to worktree so Ralph can use it for planning
    write_task_spec(&worktree_path, &task)?;

    // Validate .ralph/loop.sh exists
    let script_path = worktree_path.join(".ralph/loop.sh");
    if !script_path.exists() {
        error!("[Ralph] Script not found at {:?}", script_path);
        return Err(crate::error::ApiError::BadRequest(format!(
            "Ralph script not found at {} - ensure .ralph/loop.sh exists in the main repo",
            script_path.display()
        )));
    }

    // Open Terminal.app with the plan command (10 iterations)
    open_terminal_with_command(&worktree_path, "plan", 10)?;

    info!(
        "[Ralph] Successfully started plan mode for task {} in worktree {:?}",
        task_id, worktree_path
    );

    Ok(())
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    // Note: These routes are nested under /{task_id}/ralph in tasks.rs
    // So the full path becomes /api/tasks/{task_id}/ralph/start-plan etc.
    Router::new()
        .route("/start-plan", post(start_plan))
        .route("/start-build", post(start_build))
        .route("/stop", post(stop))
        .route("/open-terminal", post(open_terminal))
}
