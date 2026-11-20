import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AddonsController } from "../hooks/useAddons";
import { AddonStatus, addonDotenv, parseAddonPort } from "../types/addons";
import { ServiceState } from "../types/services";
import { AddonsModal } from "./AddonsModal";
import { AddonsCard } from "./dashboard/AddonsCard";
import { AddonsSection } from "./settings/AddonsSection";

const status = (overrides: Partial<AddonStatus> & Pick<AddonStatus, "kind">): AddonStatus => ({
  name: { redis: "Redis", memcached: "Memcached", mailpit: "Mailpit" }[overrides.kind],
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
  port: { redis: 6379, memcached: 11211, mailpit: 1025 }[overrides.kind],
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
    ...extra,
  }) satisfies AddonsController;

const allStopped = () => [
  status({ kind: "redis" }),
  status({ kind: "memcached" }),
  status({ kind: "mailpit" }),
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

describe("addonDotenv", () => {
  it("is empty when nothing is enabled", () => {
    expect(addonDotenv(allStopped())).toBe("");
  });

  it("uses memcached as the cache only without redis", () => {
    const memcachedOnly = addonDotenv([status({ kind: "memcached", enabled: true })]);
    expect(memcachedOnly).toContain("CACHE_STORE=memcached");

    const both = addonDotenv([
      status({ kind: "redis", enabled: true, port: 6390 }),
      status({ kind: "memcached", enabled: true }),
    ]);
    expect(both.match(/CACHE_STORE=/g)).toHaveLength(1);
    expect(both).toContain("CACHE_STORE=redis");
    expect(both).toContain("REDIS_PORT=6390");
    expect(both).toContain("QUEUE_CONNECTION=redis");
  });

  it("points mail at the Mailpit SMTP port", () => {
    expect(addonDotenv([status({ kind: "mailpit", enabled: true, port: 2525 })])).toContain(
      "MAIL_PORT=2525"
    );
  });
});

describe("AddonsModal", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the three add-ons with install buttons when nothing is installed", () => {
    renderModal(controller(allStopped()));
    for (const kind of ["redis", "memcached", "mailpit"]) {
      const row = screen.getByTestId(`addon-${kind}`);
      expect(within(row).getByText("Not installed")).toBeInTheDocument();
      expect(within(row).getByRole("button", { name: /Install/ })).toBeInTheDocument();
      expect(within(row).queryByRole("button", { name: /Start/ })).not.toBeInTheDocument();
    }
    expect(screen.getByText("Enable an add-on to see its .env lines")).toBeInTheDocument();
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
    fireEvent.click(within(screen.getByTestId("addon-memcached")).getByRole("checkbox"));
    expect(c.configure).toHaveBeenCalledWith("memcached", { enabled: true });
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
    expect(screen.getByTestId("addons-dotenv")).toHaveTextContent("MAIL_PORT=1025");
  });

  it("locks every action while one command runs", () => {
    renderModal(
      controller(
        [
          status({ kind: "redis", installed: true, installed_version: "1.0.0" }),
          status({ kind: "memcached" }),
        ],
        { busyKind: "memcached", progress: { memcached: 42 } }
      )
    );
    expect(
      within(screen.getByTestId("addon-redis")).getByRole("button", { name: /Start/ })
    ).toBeDisabled();
    expect(within(screen.getByTestId("addon-memcached")).getByText(/42%/)).toBeInTheDocument();
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
          status({ kind: "memcached" }),
        ])}
        onManage={onManage}
        openExternal={vi.fn()}
      />
    );
    const card = screen.getByTestId("addons-card");
    expect(within(card).getByText("Redis")).toBeInTheDocument();
    expect(within(card).queryByText("Memcached")).not.toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Manage" }));
    expect(onManage).toHaveBeenCalled();
  });
});

describe("AddonsSection (Settings)", () => {
  it("lists every add-on and saves a toggle at once", () => {
    const c = controller(allStopped());
    render(<AddonsSection controller={c} openExternal={vi.fn()} />);
    const section = screen.getByTestId("addons-settings");
    for (const kind of ["redis", "memcached", "mailpit"]) {
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
