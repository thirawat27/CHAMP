import { invoke } from "@tauri-apps/api/core";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AddonsController } from "../hooks/useAddons";
import {
  AddonStatus,
  EnvProfilesReply,
  isValidAddonId,
  orderAddons,
  parseAddonPort,
  splitArgs,
} from "../types/addons";
import { ServiceState } from "../types/services";
import { AddonsModal } from "./AddonsModal";
import { AddonsCard } from "./dashboard/AddonsCard";
import { AddonsSection } from "./settings/AddonsSection";

const NAMES: Record<string, string> = {
  redis: "Redis",
  mailpit: "Mailpit",
  meilisearch: "Meilisearch",
  centrifugo: "Centrifugo",
  rustfs: "RustFS (S3)",
};
const PORTS: Record<string, number> = {
  redis: 6379,
  mailpit: 1025,
  meilisearch: 7700,
  centrifugo: 8000,
  rustfs: 9100,
};

const status = (overrides: Partial<AddonStatus> & Pick<AddonStatus, "kind">): AddonStatus => ({
  name: NAMES[overrides.kind] ?? overrides.kind,
  category: "cache",
  custom: false,
  version: "1.0.0",
  installed_version: null,
  installed: false,
  source: null,
  downloadable: true,
  install_hint: null,
  update_available: false,
  installing: false,
  enabled: false,
  state: ServiceState.Stopped,
  port: PORTS[overrides.kind] ?? 6500,
  web_port: overrides.kind === "mailpit" ? 8025 : null,
  web_url: null,
  error: null,
  log_path: null,
  ...overrides,
});

const controller = (addons: AddonStatus[], extra: Partial<AddonsController> = {}) =>
  ({
    addons,
    loaded: true,
    busyKind: null,
    progress: {},
    error: null,
    clearError: vi.fn(),
    mailCount: null,
    refresh: vi.fn().mockResolvedValue(undefined),
    install: vi.fn().mockResolvedValue(true),
    start: vi.fn().mockResolvedValue(true),
    stop: vi.fn().mockResolvedValue(true),
    configure: vi.fn().mockResolvedValue(true),
    saveCustom: vi.fn().mockResolvedValue(true),
    removeCustom: vi.fn().mockResolvedValue(true),
    ...extra,
  }) satisfies AddonsController;

const PROFILES: EnvProfilesReply = {
  project: null,
  framework: null,
  recommended: "laravel",
  profiles: [
    {
      id: "laravel",
      label: "Laravel",
      target: ".env",
      format: "dotenv",
      content: "DB_HOST=127.0.0.1\nMAIL_PORT=1025",
      notes: ["Laravel Scout needs laravel/scout."],
    },
    {
      id: "symfony",
      label: "Symfony",
      target: ".env.local",
      format: "dotenv",
      content: "MAILER_DSN=smtp://127.0.0.1:1025",
      notes: [],
    },
  ],
};

const mockBackend = () => {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    switch (command) {
      case "get_env_profiles":
        return PROFILES;
      case "get_project_info":
        return [];
      case "get_workers":
        return [];
      default:
        return null;
    }
  });
};

const allStopped = () => [
  status({ kind: "redis" }),
  status({ kind: "mailpit" }),
  status({ kind: "meilisearch" }),
  status({ kind: "centrifugo" }),
  status({ kind: "rustfs" }),
];

const renderModal = (c: AddonsController) => {
  const props = {
    controller: c,
    onClose: vi.fn(),
    openExternal: vi.fn(),
    openFolder: vi.fn(),
    logsDir: "C:/CHAMP/logs",
  };
  render(<AddonsModal {...props} />);
  return props;
};

describe("parseAddonPort", () => {
  it.each([
    ["6379", 6379],
    [" 1025 ", 1025],
    ["1024", 1024],
    ["65535", 65535],
  ])("accepts %j", (input, expected) => {
    expect(parseAddonPort(input)).toBe(expected);
  });

  it.each(["", "abc", "80", "1023", "65536", "99999", "6379.5", "-1", "123456", "1e4"])(
    "refuses %j",
    (input) => {
      expect(parseAddonPort(input)).toBeNull();
    }
  );
});

describe("add-on helpers", () => {
  it("validates ids like the backend", () => {
    for (const id of ["valkey", "my-s3", "a", "q2"]) expect(isValidAddonId(id)).toBe(true);
    for (const id of ["", "Valkey", "2fa", "a-", "-a", "a b", "a".repeat(33)]) {
      expect(isValidAddonId(id)).toBe(false);
    }
  });

  it("splits arguments and keeps quoted words together", () => {
    expect(splitArgs('--dir "C:/my data" --port {port}')).toEqual([
      "--dir",
      "C:/my data",
      "--port",
      "{port}",
    ]);
    expect(splitArgs("  ")).toEqual([]);
  });

  it("orders built-ins first, then custom add-ons", () => {
    const custom = status({ kind: "valkey", custom: true });
    const ordered = orderAddons([custom, status({ kind: "rustfs" }), status({ kind: "redis" })]);
    expect(ordered.map((addon) => addon.kind)).toEqual(["redis", "rustfs", "valkey"]);
  });
});

describe("AddonsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    mockBackend();
  });

  it("switches tabs and remembers the last one", () => {
    const { unmount } = render(
      <AddonsModal
        controller={controller(allStopped())}
        onClose={vi.fn()}
        openExternal={vi.fn()}
        openFolder={vi.fn()}
      />
    );
    expect(screen.getByTestId("addons-tab-services")).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByTestId("addons-tab-tasks"));
    expect(screen.getByTestId("workers-panel")).toBeInTheDocument();
    expect(screen.queryByTestId("addon-redis")).not.toBeInTheDocument();
    unmount();
    renderModal(controller(allStopped()));
    expect(screen.getByTestId("addons-tab-tasks")).toHaveAttribute("aria-selected", "true");
  });

  it("lists every add-on with install buttons when nothing is installed", () => {
    renderModal(controller(allStopped()));
    for (const kind of ["redis", "mailpit", "meilisearch", "centrifugo", "rustfs"]) {
      const row = screen.getByTestId(`addon-${kind}`);
      expect(within(row).getByText("Not installed")).toBeInTheDocument();
      expect(within(row).getByRole("button", { name: /Install/ })).toBeInTheDocument();
      expect(within(row).queryByRole("button", { name: /Start/ })).not.toBeInTheDocument();
    }
  });

  it("renders the backend's framework profiles and switches between them", async () => {
    renderModal(controller(allStopped()));
    fireEvent.click(screen.getByTestId("addons-tab-config"));
    await waitFor(() =>
      expect(screen.getByTestId("addons-dotenv")).toHaveTextContent("MAIL_PORT=1025")
    );
    expect(screen.getByText("Laravel Scout needs laravel/scout.")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("env-profile"), { target: { value: "symfony" } });
    expect(screen.getByTestId("addons-dotenv")).toHaveTextContent("MAILER_DSN=");
    expect(invoke).toHaveBeenCalledWith("get_env_profiles", { project: null });
  });

  it("registers a custom add-on only with a valid form", () => {
    const c = controller(allStopped());
    renderModal(c);
    fireEvent.click(screen.getByRole("button", { name: /Add service/ }));
    const form = screen.getByTestId("custom-addon-form");
    const save = within(form).getByRole("button", { name: /Save/ });
    expect(save).toBeDisabled();
    const inputs = within(form).getAllByRole("textbox");
    fireEvent.change(inputs[0], { target: { value: "valkey" } });
    fireEvent.change(inputs[1], { target: { value: "Valkey" } });
    fireEvent.change(inputs[2], { target: { value: "6380" } });
    fireEvent.change(inputs[3], { target: { value: "C:/tools/valkey-server.exe" } });
    fireEvent.change(inputs[4], { target: { value: "--port {port}" } });
    expect(save).toBeEnabled();
    fireEvent.click(save);
    expect(c.saveCustom).toHaveBeenCalledWith({
      id: "valkey",
      name: "Valkey",
      command: "C:/tools/valkey-server.exe",
      args: ["--port", "{port}"],
      port: 6380,
      web: false,
      enabled: false,
    });
  });

  it("labels a custom add-on user-managed and offers edit and remove", () => {
    const c = controller([
      status({
        kind: "valkey",
        name: "Valkey",
        custom: true,
        installed: true,
        source: "custom",
        downloadable: false,
        definition: {
          id: "valkey",
          name: "Valkey",
          command: "C:/tools/valkey-server.exe",
          args: [],
          port: 6500,
          web: false,
          enabled: false,
        },
      }),
    ]);
    renderModal(c);
    const row = screen.getByTestId("addon-valkey");
    expect(within(row).getAllByText(/User-managed/).length).toBeGreaterThan(0);
    expect(within(row).queryByRole("button", { name: /Install/ })).not.toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: /Remove/ }));
    expect(c.removeCustom).toHaveBeenCalledWith("valkey");
  });

  it("opens the RustFS console and shows its console port", () => {
    const c = controller([
      status({
        kind: "rustfs",
        installed: true,
        enabled: true,
        state: ServiceState.Running,
        ports: [
          { role: "main", label: "RustFS S3 API", port: 9100 },
          { role: "console", label: "RustFS console", port: 9101 },
        ],
        web_url: "http://127.0.0.1:9101/",
      }),
    ]);
    const props = renderModal(c);
    const row = screen.getByTestId("addon-rustfs");
    expect(within(row).getByText("9101")).toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: /Open console/ }));
    expect(props.openExternal).toHaveBeenCalledWith("http://127.0.0.1:9101/");
  });

  it("shows the system install command where CHAMP cannot download", () => {
    renderModal(
      controller([
        status({ kind: "redis", downloadable: false, install_hint: "brew install redis" }),
      ])
    );
    const row = screen.getByTestId("addon-redis");
    expect(within(row).getByText("brew install redis")).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: /Install/ })).not.toBeInTheDocument();
  });

  it("enables an add-on from its toggle", () => {
    const c = controller(allStopped());
    renderModal(c);
    fireEvent.click(within(screen.getByTestId("addon-mailpit")).getByRole("checkbox"));
    expect(c.configure).toHaveBeenCalledWith("mailpit", { enabled: true });
  });

  it("saves a valid port on Enter and never sends an invalid one", () => {
    const c = controller(allStopped());
    renderModal(c);
    const input = screen.getByLabelText("Port", { selector: "#redis-port" });

    fireEvent.change(input, { target: { value: "80" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.blur(input);
    expect(c.configure).not.toHaveBeenCalled();
    expect(screen.getByText("Use a port from 1024 to 65535")).toBeInTheDocument();

    fireEvent.change(input, { target: { value: "6390" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(c.configure).toHaveBeenCalledWith("redis", { port: 6390 });
  });

  it("does not save an unchanged port", () => {
    const c = controller(allStopped());
    renderModal(c);
    const input = screen.getByLabelText("Port", { selector: "#redis-port" });
    fireEvent.change(input, { target: { value: "6379" } });
    fireEvent.blur(input);
    expect(c.configure).not.toHaveBeenCalled();
  });

  it("Esc in a port field undoes the edit instead of closing the window", () => {
    const c = controller(allStopped());
    const props = renderModal(c);
    const input = screen.getByLabelText("Port", { selector: "#redis-port" }) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "7000" } });
    fireEvent.keyDown(input, { key: "Escape", code: "Escape" });
    expect(input.value).toBe("6379");
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("offers Stop and the inbox for a running Mailpit", () => {
    const c = controller(
      [
        status({
          kind: "mailpit",
          installed: true,
          installed_version: "1.34.0",
          source: "champ",
          enabled: true,
          state: ServiceState.Running,
          web_url: "http://127.0.0.1:8025/",
        }),
      ],
      { mailCount: 3 }
    );
    const props = renderModal(c);
    const row = screen.getByTestId("addon-mailpit");
    fireEvent.click(within(row).getByRole("button", { name: /Stop/ }));
    expect(c.stop).toHaveBeenCalledWith("mailpit");

    fireEvent.click(within(row).getByRole("button", { name: /Open inbox/ }));
    expect(props.openExternal).toHaveBeenCalledWith("http://127.0.0.1:8025/");
    expect(within(row).getByText("3")).toBeInTheDocument();
  });

  it("opens the Meilisearch search preview", () => {
    const c = controller([
      status({
        kind: "meilisearch",
        installed: true,
        enabled: true,
        state: ServiceState.Running,
        web_url: "http://127.0.0.1:7700/",
      }),
    ]);
    const props = renderModal(c);
    const row = screen.getByTestId("addon-meilisearch");
    fireEvent.click(within(row).getByRole("button", { name: /Open search preview/ }));
    expect(props.openExternal).toHaveBeenCalledWith("http://127.0.0.1:7700/");
  });

  it("locks every action while one command runs", () => {
    renderModal(
      controller(
        [
          status({ kind: "redis", installed: true, installed_version: "1.0.0" }),
          status({ kind: "mailpit" }),
        ],
        { busyKind: "mailpit", progress: { mailpit: 42 } }
      )
    );
    expect(
      within(screen.getByTestId("addon-redis")).getByRole("button", { name: /Start/ })
    ).toBeDisabled();
    expect(within(screen.getByTestId("addon-mailpit")).getByText(/42%/)).toBeInTheDocument();
  });

  it("offers an update when an older version is installed", () => {
    renderModal(
      controller([
        status({
          kind: "redis",
          version: "8.10.3",
          installed: true,
          installed_version: "8.8.0",
          update_available: true,
        }),
      ])
    );
    expect(screen.getByRole("button", { name: /Update to 8.10.3/ })).toBeInTheDocument();
  });

  it("shows an add-on error", () => {
    renderModal(
      controller([
        status({
          kind: "redis",
          installed: true,
          state: ServiceState.Error,
          error: "Redis cannot start: port 6379 is already in use",
        }),
      ])
    );
    expect(screen.getByText(/port 6379 is already in use/)).toBeInTheDocument();
  });
});

describe("AddonsCard", () => {
  it("stays hidden while no add-on is in use", () => {
    render(
      <AddonsCard controller={controller(allStopped())} onManage={vi.fn()} openExternal={vi.fn()} />
    );
    expect(screen.queryByTestId("addons-card")).not.toBeInTheDocument();
  });

  it("lists enabled add-ons and opens the manager", () => {
    const onManage = vi.fn();
    render(
      <AddonsCard
        controller={controller([
          status({ kind: "redis", enabled: true, installed: true, state: ServiceState.Running }),
          status({ kind: "mailpit" }),
        ])}
        onManage={onManage}
        openExternal={vi.fn()}
      />
    );
    const card = screen.getByTestId("addons-card");
    expect(within(card).getByText("Redis")).toBeInTheDocument();
    expect(within(card).queryByText("Mailpit")).not.toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Manage" }));
    expect(onManage).toHaveBeenCalled();
  });
});

describe("AddonsSection (Settings)", () => {
  it("lists every add-on and saves a toggle at once", () => {
    const c = controller(allStopped());
    render(<AddonsSection controller={c} openExternal={vi.fn()} />);
    const section = screen.getByTestId("addons-settings");
    for (const kind of ["redis", "mailpit"]) {
      expect(within(section).getByTestId(`addon-${kind}`)).toBeInTheDocument();
    }
    expect(within(section).getByText(/saved at once/)).toBeInTheDocument();

    fireEvent.click(within(screen.getByTestId("addon-mailpit")).getByRole("checkbox"));
    expect(c.configure).toHaveBeenCalledWith("mailpit", { enabled: true });
  });

  it("opens the add-on window only when offered", () => {
    const { unmount } = render(
      <AddonsSection controller={controller(allStopped())} openExternal={vi.fn()} />
    );
    expect(screen.queryByRole("button", { name: /Open Add-on services/ })).not.toBeInTheDocument();
    unmount();

    const onOpenWindow = vi.fn();
    render(
      <AddonsSection
        controller={controller(allStopped())}
        openExternal={vi.fn()}
        onOpenWindow={onOpenWindow}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /Open Add-on services/ }));
    expect(onOpenWindow).toHaveBeenCalled();
  });

  it("shows a loading state and a dismissible error", () => {
    const clearError = vi.fn();
    render(
      <AddonsSection
        controller={controller([], {
          loaded: false,
          error: "Redis is already being installed.",
          clearError,
        })}
        openExternal={vi.fn()}
      />
    );
    expect(screen.getByText("Loading...")).toBeInTheDocument();
    fireEvent.click(within(screen.getByRole("alert")).getByRole("button", { name: "Close" }));
    expect(clearError).toHaveBeenCalled();
  });
});
