# Vibe-Kanban Agent Operations

## Commands
- **Dev**: `pnpm run dev` - Run frontend + backend with hot reload
- **Type Check**: `pnpm run check` - Frontend and backend type checking
- **Rust Check**: `cargo check --workspace` - Rust workspace check
- **Generate Types**: `pnpm run generate-types` - Generate TS types from Rust
- **Prepare DB**: `pnpm run prepare-db` - Update SQLx offline cache after migrations
- **Tests**: `cargo test --workspace` - Run Rust tests

## Structure
- `crates/` - Rust workspace (server, db, executors, services, utils, deployment, local-deployment, remote)
- `frontend/` - React + Vite + Tailwind
- `shared/` - Generated TypeScript types (auto-generated, don't edit)

## Architecture
- **Backend**: Axum web framework, SQLite with SQLx, ts-rs for type generation
- **Frontend**: React + TanStack Form, NiceModal dialogs, Lexical WYSIWYG editor
- **Patterns**: `ApiError` enum for errors, `ApiResponse<T>` wrapper, `defineModal<Props, Result>()` for dialogs
