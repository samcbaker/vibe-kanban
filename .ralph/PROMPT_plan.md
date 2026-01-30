0a. Study `.ralph/specs/*` with up to 250 parallel Sonnet subagents to learn the application specifications.
0b. Study `.ralph/IMPLEMENTATION_PLAN.md` (if present) to understand the plan so far.
0c. Study `packages/` with up to 250 parallel Sonnet subagents to understand shared utilities & components.
0d. For reference, consult `CLAUDE.md` for development commands and architecture overview.

**Project Structure:**
- `app/lib/` - Main Flutter application code
- `microapps/` - Independent microapp modules (calculator, credit_services, sales)
- `packages/` - Shared packages (adapters/, apis/, features/, shared/)
- `plugins/` - Custom Flutter plugins

1. Study `.ralph/IMPLEMENTATION_PLAN.md` (if present; it may be incorrect) and use up to 500 Sonnet subagents to study existing source code and compare it against `.ralph/specs/*`. Use an Opus subagent to analyze findings, prioritize tasks, and create/update `.ralph/IMPLEMENTATION_PLAN.md` as a bullet point list sorted in priority of items yet to be implemented. Ultrathink. Study `.ralph/IMPLEMENTATION_PLAN.md` to determine starting point for research and keep it up to date with items considered complete/incomplete using subagents.

IMPORTANT: Plan only. Do NOT implement anything. Do NOT assume functionality is missing; confirm with code search first. Follow Clean Architecture layers (presentation, domain, infrastructure, data) and Microapp patterns. Prefer consolidated, idiomatic implementations in `packages/` over ad-hoc copies.

ULTIMATE GOAL: Implement features per specifications following the existing architecture patterns. Consider missing elements and plan accordingly. If an element is missing, search first to confirm it doesn't exist, then if needed author the specification at `.ralph/specs/FILENAME.md`. If you create a new element then document the plan to implement it in `.ralph/IMPLEMENTATION_PLAN.md` using a subagent.
