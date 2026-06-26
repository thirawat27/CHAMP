# CHAMP Test Guide

This repository uses Vitest for the React frontend and Cargo tests for the Rust/Tauri backend.

## Frontend

```bash
pnpm test
pnpm test:run
pnpm test:ui
pnpm test:run -- --coverage
```

Frontend tests live next to components:

```text
src/components/ServiceCard.test.tsx
src/components/Dashboard.test.tsx
src/test/setup.ts
```

Covered areas include:

- Service card rendering and service states
- Dashboard initial load
- Start, stop, and restart commands
- Error display
- Project template creation
- Stack command feedback
- HTTPS Preview URL display and ready-state controls

## Backend

```bash
cd src-tauri
cargo test
cargo test --lib
cargo test tunnel::tests
cargo test -- --nocapture
cargo clippy --all-targets -- -D warnings
```

Backend tests cover:

- Service type serialization and status maps
- Process manager state handling
- Port fallback helpers
- `.htaccess` compatibility parsing
- Project scaffold helpers
- Runtime path resolution
- HTTPS tunnel URL parsing
- UI command DTO behavior

Ignored integration tests start real runtime services and require installed runtime binaries:

```bash
cd src-tauri
cargo test --lib -- --ignored --test-threads=1
```

## Full Verification

Run this before handing off changes:

```bash
pnpm lint
pnpm build
pnpm test:run
cd src-tauri
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

On Windows, `cargo test` can fail with `Access is denied` if `src-tauri/target/debug/champ.exe` is still running. Stop that debug process and retry.

## Mocking Tauri

`src/test/setup.ts` mocks `@tauri-apps/api/core` so React components can call `invoke()` in tests. Component tests should return realistic DTOs:

```ts
const serviceMap = {
  caddy: {
    service_type: "caddy",
    state: "running",
    port: 8080,
    error_message: null,
  },
};
```

HTTPS tunnel status shape:

```ts
{
  running: true,
  url: "https://demo.trycloudflare.com",
  ready: true,
  local_url: "http://127.0.0.1:8080",
  error: null,
  log_path: "C:\\CHAMP\\logs\\https-tunnel.log",
  pid: 1234
}
```

## Adding Tests

- Test behavior, not private implementation details.
- Prefer existing Testing Library patterns.
- Keep Rust tests deterministic and avoid real network/process work unless marked ignored.
- Add regression tests when fixing user-reported UI bugs.
