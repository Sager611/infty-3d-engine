> AGENTS.md must be <100 lines, excluding this terminal Changelog.

- REQUIREMENTS.md is the complete source of project requirements.
- Every change: maximum deletion, minimum addition; minimize final words/characters/code across all files, including AGENTS.md; preserve required behavior/tests.
- Architecture MUST be modular and functional-first at every level: dynamic, readable, maintainable, changeable, debuggable; specify module behavior, interfaces, state, and errors for every edge case; no hidden state; idempotent updates; pure cores isolated from effects.
- Every implementation MUST be general for any current or future material or geometry: drive behavior from material data and the geometry interface (`src/geometry/api.rs`); no material- or geometry-specific branches elsewhere.
- After every completed change, commit and push.
- Write tests first; preserve behavior; run strict format, compile, lint, and relevant tests; fully stage all non-ignored files before committing.
- No workers, subagents or other worktrees; work directly in this folder.
- No Docker or local Vitest; keep idle CPU and memory low.
- For visual changes, perform manual QA and report honest metrics and limits.
- Rust/wgpu/WGSL: use analytic ray intersections and closest-approach math, never ray marching; align layout, address spaces, bindings, and precision; validate shader, pipeline, resize, and device-loss paths.
- Every repo Markdown file MUST end with `## Changelog` and a `When | Who | What | Why` table describing that file.
- Run `./check`; it validates Markdown, Rust, WASM, web, TypeScript, and dependency policy.

## Changelog
| When | Who | What | Why |
|---|---|---|---|
| 2026-11-06 | Assistant | Created AGENTS.md rules | Establish concise project policy |
| 2026-11-06 | Assistant | Compacted guidance and added check instructions | Keep automation rules concise |
| 2026-11-06 18:41 UTC | Assistant | Required fully staged commits | Prevent unstaged fixes masking invalid staged code |
| 2026-11-06 19:20 UTC | Assistant | Restored deletion-first minimal-change policy | Restore explicit user intent |
| 2026-11-06 19:34 UTC | Assistant | Added commit-and-push completion rule | Keep completed changes published |
| 2026-11-07 | Assistant | Added modular functional-first architecture rule | Keep behavior explicit, dynamic, and easy to change/debug |
| 2026-11-07 | User | Prohibited workers and subagents | Require direct execution |
| 2026-11-08 | Assistant | Made REQUIREMENTS.md authoritative | Centralize project requirements |
| 2026-11-26 | User | Required material-general implementations | Stop material-specific paths |
| 2026-11-28 | User | Banned other worktrees | Keep all work in the main folder |
| 2026-11-28 | User | Required one geometry interface | Every geometry must implement every function |
