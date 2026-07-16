# CLAUDE.md — dockshell

## What this is

Native Docker GUI (Tauri v2 + Vue 3 + TS + Rust). Scaffolded sibling of
proxmox-desktop, pgcove and hopline — same stack on purpose, see
ARCHITECTURE.md. License is FSL-1.1-MIT (see LICENSING.md) — don't add code
under incompatible licenses.

## Workflow

- One branch + one PR per issue. Small, focused commits.
- CI (`secrets`, `frontend`, `rust`) must be green before merge. Branch
  protection can't be enforced on a free-plan private repo — treat it as
  enforced anyway.
- Board: GitHub Project "dockshell", columns Backlog → To Do → In Progress →
  In Review → Done. Move the issue as you work it.
- Issues state the "why" in the body; keep that when editing scope.

## Windows toolchain quirks (this dev machine)

- cargo/rustc are NOT on PATH in a fresh shell. PowerShell:
  `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"` before any
  cargo/tauri command.
- Package manager is pnpm (v11+). Never npm/yarn.
- No local Docker daemon. Test against `tcp://192.168.1.105:2375`
  (wyse-server) or run the wiremock-backed `cargo test` suite.

## Testing rules

- Backend: `cargo test` in `src-tauri/` — Docker API is mocked with wiremock,
  keyring roundtrip is `#[ignore]`d (run locally with `-- --ignored`).
- Frontend: `pnpm test` (Vitest, happy-dom).
- Every new feature lands with real tests; deferred work gets a
  `test.todo(...)` / `#[ignore = "...issue #N"]` stub naming its issue.
- Lint gates: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `pnpm lint`, `pnpm typecheck`.
