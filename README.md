# dockshell

A lightweight, cross-platform native GUI for managing local and remote Docker
hosts — in the spirit of OrbStack, but not Mac-locked. For developers who want
Docker Desktop's convenience without its licensing terms or resource
footprint, and homelabbers who want a native app instead of a Portainer
browser tab.

Built with **Tauri v2 + Vue 3 + TypeScript + Rust** ([bollard](https://crates.io/crates/bollard)
for the Docker Engine API). Same architecture as
[proxmox-desktop](https://github.com/xFarid6/proxmox-desktop) — see
[ARCHITECTURE.md](ARCHITECTURE.md).

**Status: scaffold.** Working today: connection manager (profiles on disk,
secrets in the OS keyring), container list for the active host, and
start/stop/restart actions. Everything else is a filed issue on the
[project board](https://github.com/users/xFarid6/projects) — see the repo
issues for the v1 plan.

## Quickstart (dev)

Prereqs: Node ≥ 22, pnpm ≥ 11, Rust stable (on this dev machine cargo is not
on PATH in a fresh shell — run
`$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"` first in PowerShell).

```sh
pnpm install
pnpm tauri dev
```

### Pointing it at a Docker host

This machine intentionally has **no local Docker install** — dev/test against
a remote engine over TCP instead:

- Add a connection with endpoint `tcp://192.168.1.105:2375` (the homelab
  `wyse-server` box), or any Docker host with the TCP socket enabled.
- Endpoint `local` uses the platform default (named pipe on Windows,
  `/var/run/docker.sock` on Linux/macOS) if you do have Docker locally.
- Plain-TCP (2375) is unauthenticated — LAN/dev only. TLS client-cert auth
  for remote hosts is issue #7.

### Tests

```sh
pnpm test                       # frontend (Vitest)
cd src-tauri; cargo test        # backend (wiremock Docker API — no Docker needed)
cargo test -- --ignored         # + real OS-keyring roundtrip (local only)
```

## Open questions for a human

- **Keyring secret semantics**: the connection form stores an optional secret
  in the OS keyring to keep the pxx-dex pattern live from day one, but plain
  Docker TCP has no secret. It becomes the TLS client key when issue #7
  lands. If you'd rather not show the field until then, remove it from
  `ConnectionForm.vue`.
- **proxmox-desktop has no CLAUDE.md** — the prompt said to mirror it, but it
  doesn't exist there. [CLAUDE.md](CLAUDE.md) here was written fresh from
  pxx-dex's observable conventions (branch-per-issue, board columns, Windows
  toolchain quirk). Consider backporting it.
- **bollard 0.19** chosen over hand-rolled HTTP: it's the de-facto Rust
  Docker client and matches the reqwest-style async backend approach.
- **Branch protection**: not enforceable on a free-plan private repo (GitHub
  Pro feature — proxmox-desktop's works because it's public). Treat "CI
  green before merge" as policy; enable real protection if the repo goes
  public or the account upgrades.
