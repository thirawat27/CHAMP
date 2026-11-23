import { invoke } from "@tauri-apps/api/core";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

const settings = {
  web_port: 8080,
  php_port: 9000,
  mysql_port: 3306,
  postgresql_port: 5432,
  auto_start_services: false,
  package_selection: { php: "php-8.4", mysql: "mysql-8.4", phpmyadmin: "phpmyadmin-5.2" },
};

describe("SettingsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case "get_settings":
          return settings;
        case "get_available_packages_cmd":
        case "refresh_runtime_catalog":
          return { php: [], mysql: [], postgresql: [], phpmyadmin: [] };
        case "get_installed_php_versions":
          return [];
        case "get_runtime_platform":
          return "windowsX64";
        default:
          return null;
      }
    });
  });

  it("shows one category at a time and remembers the last one", async () => {
    const { unmount } = render(<SettingsPanel onClose={vi.fn()} />);
    const general = await screen.findByTestId("settings-nav-general");
    expect(general).toHaveAttribute("aria-selected", "true");

    fireEvent.click(screen.getByTestId("settings-nav-ports"));
    expect(screen.getByTestId("settings-nav-ports")).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByTestId("settings-nav-addons")).not.toBeInTheDocument();
    unmount();

    render(<SettingsPanel onClose={vi.fn()} />);
    expect(await screen.findByTestId("settings-nav-ports")).toHaveAttribute(
      "aria-selected",
      "true"
    );
  });

  it("turns project domains off and saves it with the other settings", async () => {
    render(<SettingsPanel onClose={vi.fn()} initialSection="general" />);
    const section = await screen.findByTestId("project-domains-section");
    expect(section).toHaveTextContent("http://<folder>.localhost:8080/");
    const toggle = section.querySelector("input") as HTMLInputElement;
    expect(toggle.checked).toBe(true);

    fireEvent.click(toggle);
    expect(section).toHaveTextContent("only at http://localhost:8080/<folder>/");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "save_settings",
        expect.objectContaining({
          settings: expect.objectContaining({ project_domains: false }),
        })
      )
    );
  });

  it("opens on the requested category", async () => {
    window.localStorage.setItem("champ.settings.section", "ai");
    render(<SettingsPanel onClose={vi.fn()} initialSection="ports" />);
    expect(await screen.findByTestId("settings-nav-ports")).toHaveAttribute(
      "aria-selected",
      "true"
    );
  });
});
