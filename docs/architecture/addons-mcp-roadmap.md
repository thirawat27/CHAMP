# Add-ons, framework-neutral features, and MCP — architecture note

Pass level: **Focus Mode** — scope crosses three modules (add-ons, agent connector,
PHP env injection) and a settings schema change is on the table.
Status of everything under "Proposed": **proposed, not implemented, needs owner approval.**

## 1. Evidence checked

| Area | Files |
|---|---|
| Add-on model | `src-tauri/src/addons/mod.rs`, `catalog.rs`, `runtime.rs` |
| Add-on settings | `src-tauri/src/config/settings.rs:62-110` (`AddonSettings`) |
| PHP env injection | `src-tauri/src/process/manager/php_config.rs:143,225` |
| Agent connector | `src-tauri/src/agent/tools.rs`, `handlers.rs` |
| Frontend add-ons | `src/types/addons.ts:42`, `src/components/AddonsModal.tsx` |
| Project domains | `src-tauri/src/projects.rs` |
| Repo rules | `AGENTS.md` |

Skipped: `addons/install.rs`, `addons/tests.rs`, `agent/http.rs`, `agent/database.rs` internals,
frontend styling. Not observed: Xdebug support (no matches in repo).

## 2. Confirmed facts

- Add-ons are a closed Rust enum `AddonKind { Redis, Mailpit, Meilisearch }` (`addons/mod.rs:29`).
  Behaviour is spread across `match kind` blocks in `mod.rs`, `catalog.rs`, `runtime.rs`, `install.rs`.
- `AddonSettings` stores one flat field per add-on and per port (`redis_enabled`, `redis_port`,
  `mailpit_smtp_port`, ...) (`settings.rs:76-83`).
- Downloads are compiled-in, pinned by version + SHA-256 (`catalog.rs`). This is a security rule in `AGENTS.md`.
- PHP-FPM already receives framework-neutral env vars `CHAMP_REDIS_HOST`, `CHAMP_MAIL_PORT`,
  `CHAMP_MEILISEARCH_HOST`, ... (`addons/mod.rs:317`, injected at `php_config.rs:225`).
- The `.env` text shown to users and agents is Laravel-only (`REDIS_CLIENT=predis`, `SCOUT_DRIVER`, ...)
  (`addons/mod.rs:341`, UI title `addonEnvTitle` = "Project .env lines (Laravel)").
- MCP tools cover only the four core services; `service_schema` enum is
  `caddy | php-fpm | mysql | postgresql` (`agent/tools.rs:18`). No add-on tool exists.
- MCP server advertises only the `tools` capability; protocol versions up to `2025-06-18`
  (`handlers.rs:17,38`). Results already carry `structuredContent` (`handlers.rs:404`), but tools declare no `outputSchema`.

## 3. Findings

### F1 — Adding an add-on means editing ~6 places (Medium, Direct)
- **Evidence:** enum + `key/display_name/enabled/set_enabled` matches (`mod.rs:38-69`), port match in
  `apply_change` (`mod.rs:408`), `browser_port` match (`mod.rs:174`), `ports_for` and `build_command`
  (`runtime.rs`), `package_for` (`catalog.rs:139`), flat settings fields, plus `src/types/addons.ts`.
- **Impact:** every new service grows the settings schema and every match; this is the main reason add-ons feel inflexible.
- **Direction (proposed):** one descriptor table — `AddonSpec { key, name, ports: &[PortSpec{role, default}],
  web_role, args(plan) -> Vec<String>, env(ports) -> EnvProfile, download(os, arch) }` — and settings as
  `addons: BTreeMap<String, AddonConfig { enabled, ports: BTreeMap<String, u16> }>` with a serde migration
  that reads the old flat fields once. Pinned catalog stays compiled-in.

### F2 — The `.env` block is built in two places (Medium, Direct)
- **Evidence:** `addons/mod.rs:341` (`dotenv`) and `src/types/addons.ts:42` (`addonDotenv`) contain the same lines.
- **Impact:** the UI and the agent can show different values after any edit; F3 makes this worse (×N frameworks).
- **Direction (proposed):** backend returns ready env profiles (in `AddonStatus` or a `get_env_profiles` command); frontend only renders.

### F3 — Env guidance is hard-wired to Laravel (Medium, Direct)
- **Evidence:** `dotenv()`, the `note` in `environment_json()` (`mod.rs:392`), translation keys 705/717/728.
- **Impact:** Symfony, WordPress, CodeIgniter, Slim and plain-PHP users get keys their framework ignores,
  although the stack itself (Caddy + PHP-FPM, `public/` detection) is already framework-neutral.
- **Direction (proposed):** see §4.2 env profiles.

### F4 — Agents cannot see or control add-ons (Medium, Direct)
- **Evidence:** `tools.rs:18-24`, `WRITE_TOOLS` list; `get_environment` includes add-ons read-only only.
- **Impact:** an agent cannot start Redis, read Mailpit's log, or check that a sign-up email was sent —
  common agent workflows today.

### F5 — Server instructions still describe only path URLs (Low, Direct)
- **Evidence:** `SERVER_INSTRUCTIONS` (`handlers.rs:20`) mentions `http://localhost:<port>/<folder>/`;
  `get_environment` tells agents to set `APP_URL` to `<folder>.localhost` (`handlers.rs:120`).
- **Impact:** agents may configure the path URL first, which breaks root-relative assets.

### F6 — MCP protocol surface is minimal (Low, Inferred, Verify first: Yes)
- **Evidence:** only `tools` capability; newest supported version `2025-06-18`.
- **Impact:** clients cannot attach logs/env as resources or use prompts; newer spec revisions
  (released after 2025-06-18) fall back silently. Check the current MCP spec before changing versions.

## 4. Proposed changes (all need approval)

### 4.1 Add-on candidates

Fit criteria (from `AGENTS.md`): portable single binary or archive, no admin, binds `127.0.0.1`,
pinned SHA-256, works on Windows/macOS/Linux. "Verify" = check upstream builds before committing.

| Add-on | Why now | Fit | Verify |
|---|---|---|---|
| **S3-compatible storage** (Garage / SeaweedFS / RustFS) | File uploads in every framework talk S3 today | Single Go/Rust binary | Windows build, licence; MinIO community binaries are no longer a safe pick |
| **Centrifugo** (WebSockets / realtime) | Live UI, notifications; framework-neutral (Laravel Broadcasting, Symfony Mercure-style, plain PHP via HTTP API) | Single Go binary, all OSes | Yes |
| **Mercure hub** | Symfony's native realtime; also built into Caddy | Could ride on the existing Caddy | Caddy build with the module |
| **NATS** (queues / pub-sub) | Lightweight queue alternative to Redis | Single Go binary | PHP client maturity |
| **Valkey** as Redis engine option | Open-source Redis fork many hosts moved to | Engine switch, like MySQL/PostgreSQL | Windows build |
| **Typesense** as search option | Alternative to Meilisearch | Single binary | Windows build likely missing |
| **Ollama** (local LLM + embeddings) | AI features in apps without cloud keys | Large; user model downloads | Size, GPU; keep strictly optional |
| **pgvector** for PostgreSQL | Vector search for RAG/AI features | Extension, not add-on | Windows build of the extension |

Not recommended: RabbitMQ (needs Erlang), MongoDB (large, licence), Docker-only tools (Gotenberg).

### 4.2 Framework-neutral features

1. **Env profiles** (fixes F2/F3). Backend emits one profile per framework from the same port data:
   - `laravel` — current block
   - `symfony` — `DATABASE_URL=mysql://root:@127.0.0.1:3306/app`, `MAILER_DSN=smtp://127.0.0.1:1025`,
     `REDIS_URL=redis://127.0.0.1:6379`, `MEILISEARCH_URL=...`
   - `wordpress` — `define('DB_HOST', '127.0.0.1:3306');` + SMTP plugin hints
   - `dsn` (generic) — URL-style `redis://`, `smtp://`, `mysql://`, `postgresql://` (works with most libraries)
   - `champ` — the `CHAMP_*` env vars PHP already receives (`getenv('CHAMP_REDIS_PORT')`)
   UI: a profile picker instead of the "(Laravel)" title; the agent gets all profiles.
2. **Project detection.** Read `composer.json` (`laravel/framework`, `symfony/framework-bundle`,
   `codeigniter4/framework`, `slim/slim`, `yiisoft/*`), `wp-config.php`, and `require.php` to report
   framework, web root, and required PHP version. Feeds `list_projects`, the env profile default, and a
   "PHP version mismatch" hint.
3. **Xdebug toggle** (not observed today). Per-PHP-version on/off, `xdebug.mode=debug,develop`,
   port 9003; benefits every framework.
4. **Project background processes.** Per-project commands (queue worker, scheduler, `messenger:consume`,
   `wp cron`) started/stopped with the stack, logged like services. Must be user-defined in CHAMP UI,
   never by agents (AGENTS.md forbids agent shell execution).

### 4.3 MCP additions

Respects existing rules: localhost-only, token, no data-dropping tools, no arbitrary SQL/shell.

Read tools:
- `get_addons` — status, ports, URLs, env profiles.
- `read_logs` — extend `service` enum with add-on keys.
- `list_mail` / `read_mail` — latest Mailpit messages (subject, to, text body, extracted links) so an
  agent can verify sign-up and reset-password emails.
- `get_project` — detected framework, web root, required PHP, `.env` presence (§4.2.2).
- `list_tables` / `describe_table` — fixed `information_schema` queries only.
- `get_php_info` — loaded extensions and key ini values (memory_limit, upload size, Xdebug state).

Write tools (added to `WRITE_TOOLS`, hidden in read-only mode, emit `agent-activity`):
- `start_addon` / `stop_addon` / `enable_addon`.
- `switch_php_version` (IPC already exists).
- `toggle_xdebug` (if §4.2.3 lands).

Protocol:
- Declare `outputSchema` for tools that already return `structuredContent`.
- `resources`: `champ://logs/<service>`, `champ://environment`, `champ://projects/<name>`.
- `prompts`: "connect my project to CHAMP", "diagnose a 500 error".
- Send `notifications/tools/list_changed` when read-only mode flips (`listChanged` is `false` today).
- Update `SERVER_INSTRUCTIONS` for `<folder>.localhost` (F5).

## 5. Decisions (owner answers, 2026-11-23)

1. **Custom add-ons: yes, labelled user-managed.** The user supplies the binary and the port. CHAMP
   only starts and stops it and checks the port. It never downloads or verifies the binary, and the UI
   says so. Pinned add-ons keep the existing "never install unverified" rule.
2. **Frameworks: as broad as practical, Laravel first.** Env profiles and detection cover Laravel,
   Symfony, WordPress, CodeIgniter 4, Slim, Yii, CakePHP, plus the generic DSN and `CHAMP_*` profiles
   for everything else.
3. **Settings migration: approved for the next release.** Flat fields become a map, with a one-time
   migration and a round-trip test. Older CHAMP builds will not read the new port values.
4. **Project background processes: approved, PHP commands only.** For example `php artisan queue:work`
   or `bin/console messenger:consume`, run with CHAMP's PHP. They are user-defined and agents cannot
   create or run them.

Open questions: none identified.

## 6. Risks

- Settings migration (F1) can lose user ports if the serde fallback is wrong — needs a round-trip test.
- Large add-ons (Ollama, Meilisearch at ~350 MB) increase download and disk use; keep them opt-in.
- `read_mail` exposes mail bodies to agents; keep it behind the token and document it in the guide.
- Each new add-on needs a pinned SHA per platform; upstream gaps (like Meilisearch 1.56.2 on Apple Silicon) recur.

## Implementation status (2026-11-23)

All steps are implemented in the working tree, not yet committed. Built in a different order than
listed below: the `AddonSpec` refactor (step 3) came first so later steps would not need rework.
New built-ins: Centrifugo 6.10.3 and RustFS 1.0.4 (S3), with archive checksums verified against
GitHub release digests. Their launch flags come from upstream docs; the binaries were not run here.
Not done: `outputSchema` per tool, and `listChanged` notifications (the server sends JSON responses
only, with no stream). Xdebug is toggled only when an Xdebug file is present; CHAMP never downloads it.

## 7. Suggested order (smallest safe first)

1. F5 instructions fix + `get_addons` / add-on `read_logs` / `start_addon` / `stop_addon` (no schema change).
2. Env profiles in backend, frontend renders them (removes F2, F3).
3. `AddonSpec` descriptor + settings map migration (F1) — prerequisite for adding several services cheaply.
4. `list_mail` / `read_mail`, `get_project` with detection.
5. Xdebug toggle.
6. New add-ons one at a time, starting with S3-compatible storage and Centrifugo after build verification.

## 8. Validation

- Claim traceability: every fact in §2–3 cites a file; upstream build availability in §4.1 is marked Verify.
- Scope alignment: matches the request (more add-ons, framework-neutral features, MCP, flexibility). Nothing implemented.
- Handoff readiness: open questions and ordered next actions listed above.
