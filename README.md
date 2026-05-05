# CHAMP

**CHAMP By Thirawat27**

CHAMP is a cross-platform desktop local web development stack:

**Caddy + HTTP(S) + Adminer + MySQL + PHP**

Source repository: https://github.com/thirawat27/CHAMP

License holder: Thirawat Sinlapasomsak

It is designed to run without administrator privileges by keeping runtime binaries, generated configs, logs, database data, and projects in the current user's writable app data folder.

## Stack

| Component | Default                   |
| --------- | ------------------------- |
| Caddy     | 2.11.2                    |
| PHP       | 8.5.x package selection   |
| MySQL     | 8.4 LTS package selection |
| Adminer   | 5.4.1                     |

## Default URLs

| Service     | URL                           |
| ----------- | ----------------------------- |
| Site root   | http://localhost:8080         |
| Adminer     | http://localhost:8080/adminer |
| MySQL       | 127.0.0.1:3307                |
| PHP FastCGI | 127.0.0.1:9000                |

## Development

```bash
pnpm install
pnpm dev
pnpm build
pnpm tauri dev
```

Rust backend commands:

```bash
cd src-tauri
cargo build
cargo test
cargo clippy
```

## User Data

CHAMP writes runtime and generated files under the user data directory, for example:

```text
%LOCALAPPDATA%\CHAMP\
├── config\
├── logs\
├── mysql\data\
├── projects\
└── runtime\
```

This avoids `Access is denied` errors when the application itself is installed in a protected folder such as `Program Files`.
