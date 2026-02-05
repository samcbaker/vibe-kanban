//! Ralph Mode API routes for task implementation planning and building.
//!
//! These routes manage the Ralph AI loop execution for tasks.
//! Ralph operates in two modes:
//! - Plan mode: Analyzes the spec and creates an IMPLEMENTATION_PLAN.md
//! - Build mode: Implements the plan created during planning

use axum::{
    Extension, Router,
    extract::State,
    http::StatusCode,
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason},
    execution_process_logs::ExecutionProcessLogs,
    project_repo::ProjectRepo,
    repo::Repo,
    session::{CreateSession, Session},
    task::{RalphStatus, Task},
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use deployment::Deployment;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        coding_agent_initial::CodingAgentInitialRequest,
    },
    executors::BaseCodingAgent,
    profile::ExecutorProfileId,
};
use serde::{Deserialize, Serialize};
use services::services::{container::ContainerService, worktree_manager::WorktreeManager};
use std::path::{Path, PathBuf};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_task_middleware};

/// Response for Ralph status (for debugging/smoke testing)
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct RalphStatusResponse {
    pub ralph_status: RalphStatus,
    pub task_id: Uuid,
}

/// Response when starting a Ralph operation
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct RalphStartResponse {
    pub workspace_id: Uuid,
    pub process_id: Uuid,
}

/// Response for plan content
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct RalphPlanResponse {
    pub content: String,
}

/// Response for execution details (useful for debugging failures)
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct RalphExecutionDetailsResponse {
    /// Current Ralph status for the task
    pub ralph_status: RalphStatus,
    /// Exit code of the last Ralph process (if completed)
    pub exit_code: Option<i64>,
    /// Status of the last Ralph execution process
    pub process_status: Option<String>,
    /// Run reason (RalphPlan or RalphBuild)
    pub run_reason: Option<String>,
    /// When the process completed (if finished)
    pub completed_at: Option<String>,
    /// Last log content (stderr/stdout, truncated to last 50 lines)
    pub last_logs: Option<String>,
}

/// Helper function to setup Ralph in a worktree
/// This copies .ralph to .ralph-vibe-kanban with VK-specific prompts
async fn setup_ralph_for_workspace(
    pool: &sqlx::SqlitePool,
    workspace: &Workspace,
) -> Result<(), ApiError> {
    // Get the worktree path from the workspace
    let worktree_path = workspace.container_ref.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Workspace has no container reference (worktree not set up)".to_string())
    })?;

    // Get the first repo to find the source .ralph directory
    let workspace_repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let first_repo_id = workspace_repos.first().ok_or_else(|| {
        ApiError::BadRequest("No repositories found for workspace".to_string())
    })?.id;

    let repo = Repo::find_by_id(pool, first_repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Repository not found".to_string()))?;

    // Setup Ralph in worktree - copy .ralph to .ralph-vibe-kanban with VK-specific prompts
    let ralph_source = repo.path.join(".ralph");
    if !ralph_source.exists() {
        return Err(ApiError::BadRequest(format!(
            "Ralph source directory not found at: {}. Ensure .ralph folder exists in the repository root.",
            ralph_source.display()
        )));
    }

    WorktreeManager::setup_ralph_in_worktree(Path::new(worktree_path), &ralph_source)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to setup Ralph in worktree: {}", e)))?;

    tracing::info!(
        "Ralph setup complete in worktree: {} (source: {})",
        worktree_path,
        ralph_source.display()
    );

    Ok(())
}

/// Helper function to create a workspace for Ralph if one doesn't exist
/// This mirrors the logic from create_task_attempt but uses project repos
async fn get_or_create_workspace_for_ralph(
    deployment: &DeploymentImpl,
    task: &Task,
) -> Result<Workspace, ApiError> {
    let pool = &deployment.db().pool;

    // Check if workspace already exists
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;
    if let Some(workspace) = workspaces.first() {
        return Ok(workspace.clone());
    }

    // Get the project for this task
    let project = task
        .parent_project(pool)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Task has no parent project".to_string()))?;

    // Get the project's repos
    let repos = ProjectRepo::find_repos_for_project(pool, project.id).await?;
    if repos.is_empty() {
        return Err(ApiError::BadRequest(
            "Project has no repositories configured".to_string(),
        ));
    }

    // Compute agent_working_dir based on repo count
    // - Single repo: join repo name with default_working_dir (if set), or just repo name
    // - Multiple repos: use None (agent runs in workspace root)
    let agent_working_dir = if repos.len() == 1 {
        let repo = &repos[0];
        match &repo.default_working_dir {
            Some(subdir) => {
                let path = PathBuf::from(&repo.name).join(subdir);
                Some(path.to_string_lossy().to_string())
            }
            None => Some(repo.name.clone()),
        }
    } else {
        None
    };

    let workspace_id = Uuid::new_v4();
    let git_branch_name = deployment
        .container()
        .git_branch_from_workspace(&workspace_id, &task.title)
        .await;

    // Create the workspace
    let workspace = Workspace::create(
        pool,
        &CreateWorkspace {
            branch: git_branch_name,
            agent_working_dir,
        },
        workspace_id,
        task.id,
    )
    .await?;

    // Create workspace repos using each repo's default_target_branch
    let workspace_repos: Vec<CreateWorkspaceRepo> = repos
        .iter()
        .map(|repo| CreateWorkspaceRepo {
            repo_id: repo.id,
            target_branch: repo
                .default_target_branch
                .clone()
                .unwrap_or_else(|| "main".to_string()),
        })
        .collect();

    WorkspaceRepo::create_many(pool, workspace.id, &workspace_repos).await?;

    // Create the workspace container (worktrees) without starting any agent
    // Ralph will handle its own execution after this
    if let Err(err) = deployment.container().create(&workspace).await {
        tracing::error!("Failed to create workspace for Ralph: {}", err);
        return Err(ApiError::BadRequest(format!(
            "Failed to setup workspace: {}",
            err
        )));
    }

    // Reload workspace to get updated container_ref
    let workspace = Workspace::find_by_id(pool, workspace.id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace not found after creation".to_string()))?;

    tracing::info!(
        "Created workspace {} for Ralph task {}",
        workspace.id,
        task.id
    );

    Ok(workspace)
}

/// Get Ralph status (for debugging/smoke testing)
///
/// GET /tasks/:id/ralph/status
pub async fn get_status(
    Extension(task): Extension<Task>,
) -> Result<ResponseJson<ApiResponse<RalphStatusResponse>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(RalphStatusResponse {
        ralph_status: task.ralph_status,
        task_id: task.id,
    })))
}

/// Start Ralph plan mode for a task
///
/// POST /tasks/:id/ralph/start-plan
/// Valid from: None, Failed
/// Transitions to: Planning
pub async fn start_plan(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<RalphStartResponse>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    if !matches!(task.ralph_status, RalphStatus::None | RalphStatus::Failed) {
        return Err(ApiError::BadRequest(format!(
            "Cannot start Ralph from state {:?}. Valid states: None, Failed",
            task.ralph_status
        )));
    }

    // Verify task has a description (spec)
    let spec_content = task
        .description
        .as_ref()
        .filter(|d| !d.trim().is_empty())
        .ok_or_else(|| {
            ApiError::BadRequest("Task must have a description (spec) to use Ralph".to_string())
        })?;

    // Get or create a workspace for this task
    let workspace = get_or_create_workspace_for_ralph(&deployment, &task).await?;

    // Setup Ralph in worktree - copy .ralph to .ralph-vibe-kanban with VK-specific prompts
    setup_ralph_for_workspace(pool, &workspace).await?;

    // Update ralph_status to Planning BEFORE starting execution
    Task::update_ralph_status(pool, task.id, RalphStatus::Planning).await?;

    // Create a new session for this Ralph execution
    let session = Session::create(
        pool,
        &CreateSession {
            executor: Some("RALPH".to_string()),
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await?;

    // Build executor action with Ralph executor
    // Note: next_action must be None - Ralph handles its own completion
    let executor_action = ExecutorAction::new(
        ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
            prompt: spec_content.clone(),
            executor_profile_id: ExecutorProfileId {
                executor: BaseCodingAgent::Ralph,
                variant: Some("PLAN".to_string()),
            },
            working_dir: None,
        }),
        None, // CRITICAL: next_action must be None for Ralph
    );

    // Start execution with RalphPlan run_reason
    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::RalphPlan,
        )
        .await?;

    tracing::info!(
        "Started Ralph plan for task {} (workspace={}, process={})",
        task.id,
        workspace.id,
        execution_process.id
    );

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(RalphStartResponse {
            workspace_id: workspace.id,
            process_id: execution_process.id,
        })),
    ))
}

/// Get the implementation plan content
///
/// GET /tasks/:id/ralph/plan
pub async fn get_plan(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<RalphPlanResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // Plan can be viewed when awaiting approval or completed
    if !matches!(
        task.ralph_status,
        RalphStatus::AwaitingApproval | RalphStatus::Completed
    ) {
        return Err(ApiError::BadRequest(format!(
            "No plan available. Current state: {:?}. Plan is available when AwaitingApproval or Completed.",
            task.ralph_status
        )));
    }

    // Get the workspace
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;
    let workspace = workspaces.first().ok_or_else(|| {
        ApiError::BadRequest("No workspace found for task".to_string())
    })?;

    // Get the worktree path
    let worktree_path = workspace.container_ref.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Workspace has no container reference".to_string())
    })?;

    // Read the implementation plan
    let plan_path = std::path::Path::new(worktree_path).join("IMPLEMENTATION_PLAN.md");
    let content = tokio::fs::read_to_string(&plan_path)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Implementation plan not found: {}", e)))?;

    Ok(ResponseJson(ApiResponse::success(RalphPlanResponse {
        content,
    })))
}

/// Approve the plan and start build
///
/// POST /tasks/:id/ralph/approve
/// Valid from: AwaitingApproval
/// Transitions to: Building
pub async fn approve(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<RalphStartResponse>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    if task.ralph_status != RalphStatus::AwaitingApproval {
        return Err(ApiError::BadRequest(format!(
            "Cannot approve from state {:?}. Must be AwaitingApproval.",
            task.ralph_status
        )));
    }

    // Get the spec content (validated when plan started)
    let spec_content = task
        .description
        .as_ref()
        .cloned()
        .unwrap_or_default();

    // Get the workspace
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;
    let workspace = workspaces.first().ok_or_else(|| {
        ApiError::BadRequest("No workspace found for task".to_string())
    })?;

    // Update ralph_status to Building BEFORE starting execution
    Task::update_ralph_status(pool, task.id, RalphStatus::Building).await?;

    // Create a new session for build execution
    let session = Session::create(
        pool,
        &CreateSession {
            executor: Some("RALPH".to_string()),
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await?;

    // Build executor action with Ralph executor in build mode
    let executor_action = ExecutorAction::new(
        ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
            prompt: spec_content,
            executor_profile_id: ExecutorProfileId {
                executor: BaseCodingAgent::Ralph,
                variant: Some("BUILD".to_string()),
            },
            working_dir: None,
        }),
        None, // CRITICAL: next_action must be None for Ralph
    );

    // Start execution with RalphBuild run_reason
    let execution_process = deployment
        .container()
        .start_execution(
            workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::RalphBuild,
        )
        .await?;

    tracing::info!(
        "Started Ralph build for task {} (workspace={}, process={})",
        task.id,
        workspace.id,
        execution_process.id
    );

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(RalphStartResponse {
            workspace_id: workspace.id,
            process_id: execution_process.id,
        })),
    ))
}

/// Re-run plan mode
///
/// POST /tasks/:id/ralph/replan
/// Valid from: AwaitingApproval
/// Transitions to: Planning
pub async fn replan(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<RalphStartResponse>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    if task.ralph_status != RalphStatus::AwaitingApproval {
        return Err(ApiError::BadRequest(format!(
            "Cannot replan from state {:?}. Must be AwaitingApproval.",
            task.ralph_status
        )));
    }

    // Get the spec content
    let spec_content = task
        .description
        .as_ref()
        .filter(|d| !d.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            ApiError::BadRequest("Task must have a description (spec) to use Ralph".to_string())
        })?;

    // Get the workspace
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;
    let workspace = workspaces.first().ok_or_else(|| {
        ApiError::BadRequest("No workspace found for task".to_string())
    })?;

    // Update ralph_status to Planning
    Task::update_ralph_status(pool, task.id, RalphStatus::Planning).await?;

    // Create a new session for re-planning
    let session = Session::create(
        pool,
        &CreateSession {
            executor: Some("RALPH".to_string()),
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await?;

    // Build executor action
    let executor_action = ExecutorAction::new(
        ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
            prompt: spec_content,
            executor_profile_id: ExecutorProfileId {
                executor: BaseCodingAgent::Ralph,
                variant: Some("PLAN".to_string()),
            },
            working_dir: None,
        }),
        None,
    );

    // Start execution with RalphPlan run_reason
    let execution_process = deployment
        .container()
        .start_execution(
            workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::RalphPlan,
        )
        .await?;

    tracing::info!(
        "Started Ralph re-plan for task {} (workspace={}, process={})",
        task.id,
        workspace.id,
        execution_process.id
    );

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(RalphStartResponse {
            workspace_id: workspace.id,
            process_id: execution_process.id,
        })),
    ))
}

/// Restart Ralph from Failed state
///
/// POST /tasks/:id/ralph/restart
/// Valid from: Failed
/// Transitions to: Planning
pub async fn restart(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<RalphStartResponse>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    if task.ralph_status != RalphStatus::Failed {
        return Err(ApiError::BadRequest(format!(
            "Cannot restart from state {:?}. Must be Failed.",
            task.ralph_status
        )));
    }

    // Get the spec content
    let spec_content = task
        .description
        .as_ref()
        .filter(|d| !d.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            ApiError::BadRequest("Task must have a description (spec) to use Ralph".to_string())
        })?;

    // Get the workspace
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;
    let workspace = workspaces.first().ok_or_else(|| {
        ApiError::BadRequest("No workspace found for task".to_string())
    })?;

    // Re-setup Ralph in worktree (may have been corrupted during failed execution)
    setup_ralph_for_workspace(pool, workspace).await?;

    // Update ralph_status to Planning
    Task::update_ralph_status(pool, task.id, RalphStatus::Planning).await?;

    // Create a new session
    let session = Session::create(
        pool,
        &CreateSession {
            executor: Some("RALPH".to_string()),
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await?;

    // Build executor action
    let executor_action = ExecutorAction::new(
        ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
            prompt: spec_content,
            executor_profile_id: ExecutorProfileId {
                executor: BaseCodingAgent::Ralph,
                variant: Some("PLAN".to_string()),
            },
            working_dir: None,
        }),
        None,
    );

    // Start execution with RalphPlan run_reason
    let execution_process = deployment
        .container()
        .start_execution(
            workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::RalphPlan,
        )
        .await?;

    tracing::info!(
        "Restarted Ralph for task {} (workspace={}, process={})",
        task.id,
        workspace.id,
        execution_process.id
    );

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(RalphStartResponse {
            workspace_id: workspace.id,
            process_id: execution_process.id,
        })),
    ))
}

/// Cancel Ralph execution
///
/// POST /tasks/:id/ralph/cancel
/// Valid from: Planning, AwaitingApproval, Building, Failed
/// Transitions to: None
pub async fn cancel(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<()>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    let valid_from = vec![
        RalphStatus::Planning,
        RalphStatus::AwaitingApproval,
        RalphStatus::Building,
        RalphStatus::Failed,
    ];
    if !valid_from.contains(&task.ralph_status) {
        return Err(ApiError::BadRequest(format!(
            "Cannot cancel from state {:?}. Valid states: {:?}",
            task.ralph_status, valid_from
        )));
    }

    // Update ralph_status to None
    Task::update_ralph_status(pool, task.id, RalphStatus::None).await?;

    tracing::info!("Cancelled Ralph for task {}", task.id);

    Ok((StatusCode::OK, ResponseJson(ApiResponse::success(()))))
}

/// Reset Ralph from Completed state to allow re-running
///
/// POST /tasks/:id/ralph/reset
/// Valid from: Completed
/// Transitions to: None
pub async fn reset(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<()>>), ApiError> {
    let pool = &deployment.db().pool;

    // Validate state transition
    let valid_from = vec![RalphStatus::Completed];
    if !valid_from.contains(&task.ralph_status) {
        return Err(ApiError::BadRequest(format!(
            "Cannot reset from state {:?}. Valid states: {:?}",
            task.ralph_status, valid_from
        )));
    }

    // Update ralph_status to None
    Task::update_ralph_status(pool, task.id, RalphStatus::None).await?;

    tracing::info!("Reset Ralph for task {}", task.id);

    Ok((StatusCode::OK, ResponseJson(ApiResponse::success(()))))
}

/// Get execution details for debugging Ralph failures
///
/// GET /tasks/:id/ralph/details
/// Returns information about the last Ralph execution including logs
pub async fn get_execution_details(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<RalphExecutionDetailsResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // Get the workspace for this task
    let workspaces = Workspace::fetch_all(pool, Some(task.id)).await?;

    // If no workspace, return basic status only
    let workspace = match workspaces.first() {
        Some(w) => w,
        None => {
            return Ok(ResponseJson(ApiResponse::success(RalphExecutionDetailsResponse {
                ralph_status: task.ralph_status,
                exit_code: None,
                process_status: None,
                run_reason: None,
                completed_at: None,
                last_logs: None,
            })));
        }
    };

    // Find the latest Ralph process for this workspace
    let latest_plan = ExecutionProcess::find_latest_by_workspace_and_run_reason(
        pool,
        workspace.id,
        &ExecutionProcessRunReason::RalphPlan,
    )
    .await?;

    let latest_build = ExecutionProcess::find_latest_by_workspace_and_run_reason(
        pool,
        workspace.id,
        &ExecutionProcessRunReason::RalphBuild,
    )
    .await?;

    // Pick the most recent Ralph process (by created_at)
    let latest_process = match (&latest_plan, &latest_build) {
        (Some(plan), Some(build)) => {
            if build.created_at > plan.created_at {
                Some(build)
            } else {
                Some(plan)
            }
        }
        (Some(plan), None) => Some(plan),
        (None, Some(build)) => Some(build),
        (None, None) => None,
    };

    // If no Ralph process found, return basic status
    let process = match latest_process {
        Some(p) => p,
        None => {
            return Ok(ResponseJson(ApiResponse::success(RalphExecutionDetailsResponse {
                ralph_status: task.ralph_status,
                exit_code: None,
                process_status: None,
                run_reason: None,
                completed_at: None,
                last_logs: None,
            })));
        }
    };

    // Get logs for this execution (last 50 lines)
    let log_records = ExecutionProcessLogs::find_by_execution_id(pool, process.id).await?;
    let last_logs = if log_records.is_empty() {
        None
    } else {
        // Combine all logs and take last 50 lines
        let all_logs: String = log_records
            .iter()
            .flat_map(|r| r.logs.lines())
            .collect::<Vec<_>>()
            .join("\n");

        let lines: Vec<&str> = all_logs.lines().collect();
        let last_50: String = if lines.len() > 50 {
            lines[lines.len() - 50..].join("\n")
        } else {
            all_logs
        };

        Some(last_50)
    };

    Ok(ResponseJson(ApiResponse::success(RalphExecutionDetailsResponse {
        ralph_status: task.ralph_status,
        exit_code: process.exit_code,
        process_status: Some(format!("{:?}", process.status)),
        run_reason: Some(format!("{:?}", process.run_reason)),
        completed_at: process.completed_at.map(|dt| dt.to_rfc3339()),
        last_logs,
    })))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let ralph_routes = Router::new()
        .route("/status", get(get_status))
        .route("/details", get(get_execution_details))
        .route("/start-plan", post(start_plan))
        .route("/plan", get(get_plan))
        .route("/approve", post(approve))
        .route("/replan", post(replan))
        .route("/restart", post(restart))
        .route("/cancel", post(cancel))
        .route("/reset", post(reset));

    // Nest under /tasks/:task_id/ralph with task middleware
    Router::new().nest(
        "/tasks/{task_id}/ralph",
        ralph_routes.layer(from_fn_with_state(deployment.clone(), load_task_middleware)),
    )
}
