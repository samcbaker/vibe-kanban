0a. Study `.ralph/specs/*` with up to 500 parallel Sonnet subagents to learn the application specifications.
0b. Study `.ralph/IMPLEMENTATION_PLAN.md`.
0c. For reference, consult `CLAUDE.md` for development commands and architecture overview.

**Project Structure:**
- `app/lib/` - Main Flutter application code
- `microapps/` - Independent microapp modules
- `packages/` - Shared packages (adapters/, apis/, features/, shared/)
- `plugins/` - Custom Flutter plugins

1. Your task is to implement functionality per the specifications using parallel subagents. Follow `.ralph/IMPLEMENTATION_PLAN.md` and choose the most important item to address. Before making changes, search the codebase (don't assume not implemented) using Sonnet subagents. You may use up to 500 parallel Sonnet subagents for searches/reads and only 1 Sonnet subagent for build/tests. Use Opus subagents when complex reasoning is needed (debugging, architectural decisions).
2. After implementing functionality or resolving problems, run tests for that unit of code. Use `melos test:selective_unit_test` for package tests or `melos test:diff_without_coverage` for changed files. If functionality is missing then add it per specifications. Ultrathink.
3. When you discover issues, immediately update `.ralph/IMPLEMENTATION_PLAN.md` with your findings using a subagent. When resolved, update and remove the item.
4. When there are no more pending tasks you can accomplish, create the file `.ralph/STOP` (just `touch .ralph/STOP`) to signal the loop to exit. Do not keep iterating if all work is complete or if remaining tasks are blocked/waiting on external input.

99999. Important: When authoring documentation, capture the why — tests and implementation importance.
999999. Important: Single sources of truth, no migrations/adapters. Follow Clean Architecture (presentation, domain, infrastructure, data) and Microapp patterns.
9999999. As soon as there are no build or test errors create a git tag. If there are no git tags start at 0.0.0 and increment patch by 1.
99999999. You may add extra logging if required to debug issues.
999999999. Keep `.ralph/IMPLEMENTATION_PLAN.md` current with learnings using a subagent.
9999999999. When you learn something new about how to run the application, update `.ralph/AGENTS.md` using a subagent but keep it brief.
99999999999. For any bugs you notice, resolve them or document them in `.ralph/IMPLEMENTATION_PLAN.md` using a subagent.
999999999999. Implement functionality completely. Placeholders and stubs waste efforts.
9999999999999. When `.ralph/IMPLEMENTATION_PLAN.md` becomes large, periodically clean out completed items using a subagent.
99999999999999. If you find inconsistencies in `.ralph/specs/*` then use an Opus 4.5 subagent with 'ultrathink' to update the specs.
999999999999999. IMPORTANT: Keep `.ralph/AGENTS.md` operational only — status updates belong in `.ralph/IMPLEMENTATION_PLAN.md`.
