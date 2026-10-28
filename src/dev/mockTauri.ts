/**
 * Development-only IPC mock so the UI can be opened in a regular browser
 * (`pnpm dev`, http://localhost:1420) for layout work. Loaded only when
 * `import.meta.env.DEV` is true and the page is not running inside Tauri, so
 * it never ships in a production build.
 */

import { mockIPC } from "@tauri-apps/api/mocks";

const services = {
  caddy: { service_type: "caddy", state: "running", port: 8080 },
  "php-fpm": { service_type: "php-fpm", state: "running", port: 9000 },
  mysql: {
    service_type: "mysql",
    state: "error",
    port: 3306,
    error_message: "Can't start server: Bind on TCP/IP port: Address already in use",
  },
  postgresql: { service_type: "postgresql", state: "stopped", port: 5432 },
};

const paths = {
  base_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP",
  portable: false,
  runtime_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\runtime",
  config_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\config",
  mysql_data_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\mysql\\data",
  postgresql_data_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\postgresql\\data",
  logs_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\logs",
  projects_dir: "C:\\Users\\dev\\AppData\\Local\\CHAMP\\projects",
};

const responses: Record<string, unknown> = {
  check_runtime_installed: true,
  get_language_settings: { language: "th", sound_enabled: false },
  get_all_statuses: services,
  get_https_tunnel_status: {
    running: false,
    url: null,
    ready: false,
    local_url: "http://127.0.0.1:8080",
    error: null,
    log_path: null,
    pid: null,
  },
  get_app_paths: paths,
  get_installed_versions: {
    caddy: "2.10.2",
    php: "8.4.14",
    mysql: "8.4.7",
    phpmyadmin: "5.2.3",
    cloudflared: "2025.10.1",
  },
  get_settings: {
    web_port: 8080,
    php_port: 9000,
    mysql_port: 3306,
    postgresql_port: 5432,
    auto_start_services: false,
    auto_check_updates: true,
    package_selection: {
      php: "php-8.4",
      mysql: "mysql-8.4",
      postgresql: "postgresql-17",
      phpmyadmin: "phpmyadmin-5.2",
    },
  },
  get_system_metrics: {
    cpu_usage: 7.4,
    memory_used_bytes: 9.2 * 1024 ** 3,
    memory_total_bytes: 16 * 1024 ** 3,
    network_rx_bps: 18_400,
    network_tx_bps: 4_100,
  },
  get_log_doctor_status: {
    installed: false,
    downloading: false,
    model_name: "potion-base-8M",
    parameters: "7.6M",
    download_bytes: 30_920_426,
  },
  get_agent_connector_status: {
    enabled: true,
    running: true,
    read_only: false,
    port: 47823,
    url: "http://127.0.0.1:47823/mcp",
    token: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    error: null,
  },
  get_available_packages_cmd: { php: [], mysql: [], postgresql: [], phpmyadmin: [] },
  refresh_php_catalog: null,
  get_installed_php_versions: [
    {
      id: "php-8.5",
      version: "8.5.7",
      display_name: "PHP 8.5",
      installed: true,
      active: true,
      eol: false,
      lts: false,
      recommended: true,
      path: null,
      latest_version: "8.5.12",
      update_available: true,
    },
  ],
  get_runtime_platform: "windowsX64",
  switch_database_engine: null,
};

export function installMockTauri() {
  mockIPC((command) => {
    if (command in responses) return responses[command];
    if (command === "diagnose_service") {
      return {
        service: "mysql",
        ai_used: false,
        matches: [
          {
            id: "port-in-use",
            title: { en: "Port is already in use", th: "พอร์ตถูกโปรแกรมอื่นใช้อยู่" },
            cause: {
              en: "Another program is already listening on this service's port.",
              th: "มีโปรแกรมอื่นใช้พอร์ตของบริการนี้อยู่แล้ว",
            },
            steps: {
              en: [
                "Open Settings and change the port.",
                "Or stop the other program with `netstat -ano`.",
              ],
              th: [
                "เปิด Settings แล้วเปลี่ยนพอร์ต",
                "หรือปิดโปรแกรมที่ใช้พอร์ตด้วย `netstat -ano`",
              ],
            },
            confidence: 0.93,
            evidence: "Can't start server: Bind on TCP/IP port: Address already in use",
            method: "signature",
            action: "open_settings",
            url: null,
          },
        ],
        excerpt: ["Can't start server: Bind on TCP/IP port: Address already in use"],
        log_files: [],
      };
    }
    return null;
  });
}
