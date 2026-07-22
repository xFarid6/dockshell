# Architecture

Same shape as [proxmox-desktop](https://github.com/xFarid6/proxmox-desktop)
("pxx-dex"), deliberately: dockshell, pgcove and hopline all reuse pxx-dex's
patterns against different backends so the connection-manager / keyring /
console / task-panel code can eventually be extracted into a shared library
(dockshell issue #14).

```
src/                      Vue 3 + TS frontend
  api.ts                  typed invoke() wrappers — the only IPC touchpoint
  App.vue                 layout: sidebar (connections) + main (containers) + task panel
  components/
    ConnectionList.vue    saved hosts, select/delete
    ConnectionForm.vue    add host (endpoint "local" or tcp://…)
    ContainerTable.vue    name/image/state/status/ports + start/stop/restart
    ContainerDetail.vue   inspect view — env/mounts/ports/labels (issue #8)
    TaskLogPanel.vue      task results; log *streaming* is issue #1
  __tests__/              Vitest (happy-dom); deferred.spec.ts = todo surface

src-tauri/                Rust backend
  src/connections.rs      profile store (JSON in app config dir) + OS keyring
  src/docker.rs           bollard client factory + list/actions mapping
  src/commands.rs         #[tauri::command] IPC surface
  tests/docker_api.rs     wiremock mock of the Docker Engine API
  tests/deferred.rs       #[ignore] stubs naming deferred-feature issues
```

## Key decisions

- **bollard** for the Docker Engine API — async, maintained, typed. The
  client is built per-command (no long-lived pooled state) which is fine at
  scaffold scale; revisit if per-call latency ever matters.
- **Connection pattern copied from pxx-dex `connections.rs`**: non-secret
  profile fields in `connections.json` under the app config dir, secrets only
  in the OS keyring (`keyring` crate, service name `dockshell`). One
  deliberate divergence: store functions take the directory as a `&Path`
  parameter instead of reading it from the `AppHandle`, so the file store is
  unit-testable without booting Tauri. Worth backporting to pxx-dex.
- **Backend tests mock the Engine API with wiremock** (same approach as
  pxx-dex's `mock_api.rs`) — CI needs no Docker daemon.
- **No vue-router / no state library yet**: one window, one view, plain refs
  in `App.vue`. Add them when a second real view exists, not before.
- **CI split**: `ci.yml` = cheap ubuntu-only checks on every push;
  `release.yml` = tag-only cross-platform bundles (Windows 2× / macOS 10×
  billing).

## Shared vs diverged (vs pxx-dex)

| Piece | Status |
|---|---|
| Connection manager + keyring | reused pattern (testability refactor) |
| Task/log panel | reused concept; streaming not built yet (issue #1) |
| Embedded console (xterm) | not here yet — issue #2 will generalize pxx-dex issue #11 work |
| API client layer | diverged: bollard instead of hand-rolled reqwest client |
