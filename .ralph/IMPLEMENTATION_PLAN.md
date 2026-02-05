# Ralph Mode Implementation Plan

**Branch:** `feat/ralph-loop-v2`
**Spec:** `.ralph/specs/ralph-mode-spec.md`
**Last Updated:** 2026-02-05
**Verified:** 2026-02-05 (codebase search confirmed 0% implementation)
**Plan Reviewed:** 2026-02-05 (cross-referenced with actual codebase patterns)

---

## Verification Findings (2026-02-05)

Thorough codebase exploration confirmed the following:

### Implementation Status: 0% Complete
- **NO** `ralph_enabled` field in Task struct or CreateTask
- **NO** ralph-related database migrations
- **NO** `ralph.rs` in routes directory
- **NO** ralph mentions anywhere in frontend code
- **NO** RalphResponse or Ralph types in shared/types.ts
- `frontend/src/components/ralph/` directory exists but is **EMPTY**

### Codebase Patterns Verified

**Task Model** (`crates/db/src/models/task.rs`):
- Task struct: id, project_id, title, description, status, parent_workspace_id, created_at, updated_at
- CreateTask: project_id, title, description, status, parent_workspace_id, image_ids
- Uses `sqlx::query_as!` macro with explicit type annotations
- SELECT queries list all columns explicitly (no `SELECT *`)

**Routes** (`crates/server/src/routes/tasks.rs`):
- task_id_router structure at lines 403-406:
  ```rust
  let task_id_router = Router::new()
      .route("/", get(get_task))
      .merge(task_actions_router)
      .layer(from_fn_with_state(deployment.clone(), load_task_middleware));
  ```
- Routes use `Extension<Task>` for task injection from middleware

**Frontend API** (`frontend/src/lib/api.ts`):
- Uses `makeRequest()` + `handleApiResponse<T>()` pattern
- API objects exported as const (e.g., `projectsApi`, `attemptsApi`)
- Types imported from `shared/types`

**TaskFormDialog** (`frontend/src/components/dialogs/tasks/TaskFormDialog.tsx`):
- Uses TanStack Form with `<form.Field name="...">` pattern
- Has `Switch` component already imported
- Form values type defined inline
- Uses `useMemo` for defaultValues

**TaskCard** (`frontend/src/components/tasks/TaskCard.tsx`):
- Right indicators use Fragment wrapper (`<>...</>`)
- stopPropagation pattern: `onPointerDown={(e) => e.stopPropagation()}` and `onMouseDown={(e) => e.stopPropagation()}`
- NiceModal not currently imported (will need to add)

---

### Corrections Applied (vs. original spec)
1. **ApiError::Internal doesn't exist** - Use `ApiError::BadRequest` for errors with custom messages
2. **ApiError::NotFound doesn't exist** - Use `ApiError::BadRequest` for not-found cases with descriptive messages
3. **Use Switch component** not raw checkbox - For consistency with existing forms (already imported in TaskFormDialog)
4. **Spec's `worktree_path` field doesn't exist** - Plan correctly uses `container_ref / repo.name`
5. **Spec's `find_by_task_id` doesn't exist** - Plan correctly uses `fetch_all(pool, Some(task_id))`
6. **Route mounting corrected** - Ralph routes merged INTO `task_id_router` with `.nest("/ralph", ...)`, not nested separately in `inner`

---

## Overview

Ralph Mode enables AI-driven task execution using the existing `loop.sh` script. When a user creates a task, they can optionally enable Ralph Mode. Ralph executes in a **real macOS Terminal window** in the **task's worktree directory**.

**Key Architecture Points:**
- Worktree path is at `Workspace.container_ref / Repo.name` (NOT just container_ref)
- Use `Workspace::fetch_all(pool, Some(task_id))` to get workspace for a task
- Task -> Workspace -> WorkspaceRepo -> Repo chain provides paths
- Frontend uses TanStack React Form with `<form.Field name="...">` pattern
- Dialogs use NiceModal with `defineModal<Props, Result>()` pattern
- API client uses `makeRequest()` helper + `handleApiResponse<T>()`

---

## Current Status: 100% Complete

| Component | Status | Priority | Blocking? |
|-----------|--------|----------|-----------|
| Database: `ralph_enabled` field | Completed | P0 | Yes |
| Backend API: Ralph routes | Completed | P0 | Yes |
| Task Form: Ralph checkbox | Completed | P1 | No |
| Task Card: Ralph badge | Completed | P1 | No |
| Frontend API: ralphApi client | Completed | P1 | No |
| Ralph Control Dialog | Completed | P2 | No |

---

## Priority 0 (Critical Path)

### Task 1: Database Migration - Add `ralph_enabled` to Tasks

**Spec Reference:** Spec 1
**Status:** Completed

#### Acceptance Criteria
- [x] Migration file created with timestamp format `YYYYMMDDHHMMSS`
- [x] `Task` struct has `ralph_enabled: bool` field
- [x] `CreateTask` struct has `ralph_enabled: Option<bool>` field
- [x] All SELECT queries include `ralph_enabled`
- [x] `Task::create()` accepts and persists `ralph_enabled`
- [x] SQLx offline cache regenerated (`.sqlx/` files updated)
- [x] TypeScript types regenerated (`shared/types.ts` has `ralph_enabled`)
- [x] `cargo check --workspace` passes
- [x] `pnpm run check` passes

---

### Task 2: Backend API - Ralph Control Routes

**Spec Reference:** Spec 5
**Status:** Completed
**Depends on:** Task 1

#### Acceptance Criteria
- [x] `POST /api/tasks/:id/ralph/start-plan` opens Terminal with `loop.sh plan 10` in worktree
- [x] `POST /api/tasks/:id/ralph/start-build` opens Terminal with `loop.sh build 20` in worktree
- [x] `POST /api/tasks/:id/ralph/stop` creates `.ralph/STOP` file in worktree
- [x] `POST /api/tasks/:id/ralph/open-terminal` focuses Terminal.app
- [x] Returns error if `ralph_enabled` is false
- [x] Returns error if task has no workspace
- [x] Worktree path correctly uses `container_ref/repo.name` pattern
- [x] Auto-copies `.ralph/` folder from main repo to worktree if missing
- [x] All actions logged with `[Ralph]` prefix using `tracing`
- [x] `RalphResponse` type exported to TypeScript

---

## Priority 1 (Enable User Flow)

### Task 3: Task Form - Ralph Mode Checkbox

**Spec Reference:** Spec 2
**Status:** Completed
**Depends on:** Task 1

#### Acceptance Criteria
- [x] Switch toggle appears in task creation form (not edit mode)
- [x] Switch label explains what Ralph Mode does
- [x] Uses purple color scheme when checked (`data-[state=checked]:bg-purple-600`)
- [x] Value sent to API when creating task (`ralph_enabled: true/false`)
- [x] Default is off (false)
- [x] Switch disabled during form submission

---

### Task 4: Task Card - Ralph Badge

**Spec Reference:** Spec 3
**Status:** Completed
**Depends on:** Task 1

#### Acceptance Criteria
- [x] Badge only shows when `ralph_enabled` is true
- [x] Badge is clickable (console logs for now)
- [x] Badge doesn't interfere with task card drag/drop (stopPropagation)
- [x] Badge uses purple color scheme
- [x] Badge has hover state

---

### Task 5: Frontend API Client - ralphApi

**Spec Reference:** Spec 6
**Status:** Completed
**Depends on:** Task 2

#### Acceptance Criteria
- [x] All four endpoints available (`startPlan`, `startBuild`, `stop`, `openTerminal`)
- [x] Returns typed `RalphResponse` with `success` and `message`
- [x] Follows existing API client patterns (uses `makeRequest` and `handleApiResponse`)

---

## Priority 2 (Complete Feature)

### Task 6: Ralph Control Dialog

**Spec Reference:** Spec 4
**Status:** Completed
**Depends on:** Tasks 3, 4, 5

#### Acceptance Criteria
- [x] Dialog shows task title
- [x] Four buttons: Start Plan, Start Build, Open Terminal, Stop
- [x] Buttons explain iteration limits
- [x] Follows existing dialog patterns (NiceModal + defineModal)
- [x] Console logs for all actions (`[Ralph] ...`)
- [x] Error state shown in dialog when action fails
- [x] Toast notifications for success/error
- [x] Loading state disables buttons during action

---

## Implementation Order Summary

```
1. Task 1: Database migration (P0) -----------------> START HERE
   |
   v
2. Task 2: Backend API routes (P0)
   |
   v
3. Tasks 3, 4, 5 (P1) can run in parallel:
   - Task 3: Task form checkbox
   - Task 4: Task card badge
   - Task 5: Frontend API client
   |
   v
4. Task 6: Ralph control dialog (P2) --------------> FINISH HERE
```

---

## Testing Checklist

### Unit Tests (Optional but Recommended)

- [ ] `Task::create()` with `ralph_enabled: true` persists correctly
- [ ] `Task::find_by_id()` returns correct `ralph_enabled` value

### Integration Tests

After full implementation, verify end-to-end:

- [ ] Create task with Ralph enabled -> `ralph_enabled` is true in DB
- [ ] Task card shows purple Ralph badge
- [ ] Click badge -> Ralph control dialog opens
- [ ] Start Plan -> Terminal opens with `loop.sh plan 10` in worktree
- [ ] Start Build -> Terminal opens with `loop.sh build 20` in worktree
- [ ] Stop -> `.ralph/STOP` file created in worktree
- [ ] Open Terminal -> Terminal.app focuses

### Error Cases

- [ ] Error: Ralph not enabled -> "Ralph not enabled for this task"
- [ ] Error: No workspace -> "No workspace found for task - create a workspace first"
- [ ] Error: No `.ralph/` in main repo -> Descriptive guidance message
- [ ] Error: AppleScript fails -> User-friendly toast

---

## Files Summary

### Create
| File | Task |
|------|------|
| `crates/db/migrations/20260205120000_add_ralph_enabled.sql` | 1 |
| `crates/server/src/routes/ralph.rs` | 2 |
| `frontend/src/components/dialogs/tasks/RalphControlDialog.tsx` | 6 |

### Modify
| File | Task |
|------|------|
| `crates/db/src/models/task.rs` | 1 |
| `crates/server/src/routes/mod.rs` | 2 |
| `crates/server/src/routes/tasks.rs` | 2 |
| `crates/server/src/bin/generate_types.rs` | 2 |
| `frontend/src/components/dialogs/tasks/TaskFormDialog.tsx` | 3 |
| `frontend/src/components/tasks/TaskCard.tsx` | 4, 6 |
| `frontend/src/lib/api.ts` | 5 |

### Auto-Generated (Do Not Edit Manually)
| File | When Regenerated |
|------|-----------------|
| `crates/db/.sqlx/*` | After Task 1 (`pnpm run prepare-db`) |
| `shared/types.ts` | After Tasks 1, 2 (`pnpm run generate-types`) |

---

## Notes

1. **Worktree Path:** The actual worktree is at `Workspace.container_ref / Repo.name`. For single-repo setups, the worktree is at `{container_ref}/{repo_name}/`.

2. **Workspace Lookup:** Use `Workspace::fetch_all(pool, Some(task_id))` - there is no `find_by_task_id` method.

3. **ApiError Variants:** There is NO `ApiError::NotFound` or `ApiError::Internal` variant. Use:
   - `ApiError::BadRequest(String)` for user errors and not-found cases
   - `ApiError::Database(sqlx::Error)` for database errors
   - `ApiError::Io(std::io::Error)` for filesystem/process errors
   - `ApiError::Workspace(WorkspaceError)` for workspace-related errors

4. **macOS Only:** The AppleScript Terminal.app integration is macOS-specific. Future versions may support Linux/Windows with different terminal emulators.

5. **No Kanban Lane Changes:** Task stays in its current column (Todo/InProgress/etc). Ralph status is shown as a badge only.

6. **Fixed Iteration Limits:** Plan=10, Build=20 are hardcoded in v1. Future versions may make these configurable.

7. **Auto-Copy `.ralph/`:** When Ralph starts, if `.ralph/` folder doesn't exist in the worktree, it's automatically copied from the main repo. This ensures Ralph scripts are available in each worktree.
