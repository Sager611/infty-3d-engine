> AGENTS.md must be <100 lines, excluding this terminal Changelog.

- Every change: maximum deletion, minimum addition; minimize final words/characters/code across all files, including AGENTS.md; preserve required behavior/tests.
- Architecture MUST be modular and functional-first at every level: dynamic, readable, maintainable, changeable, debuggable; specify module behavior, interfaces, state, and errors for every edge case; no hidden state; idempotent updates; pure cores isolated from effects.
- After every completed change, commit and push.
- Write tests first; preserve behavior; run strict format, compile, lint, and relevant tests; fully stage all non-ignored files before committing.
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
