import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { LogDoctorModal } from "./LogDoctorModal";
import { ServiceType } from "../types/services";

const diagnosis = (title: string) => ({
  service: ServiceType.Caddy,
  ai_used: false,
  matches: [
    {
      id: title,
      title: { en: title, th: title },
      cause: { en: "cause", th: "cause" },
      steps: { en: ["step"], th: ["step"] },
      confidence: 0.9,
      evidence: "evidence",
      method: "signature",
      action: null,
      url: null,
    },
  ],
  excerpt: [],
  log_files: [],
});

describe("LogDoctorModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("ignores a slow answer for a tab the user has left", async () => {
    let finishCaddy: (value: unknown) => void = () => {};
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "diagnose_service") {
        const { service } = args as { service: ServiceType };
        if (service === ServiceType.Caddy) {
          return new Promise((resolve) => {
            finishCaddy = resolve;
          });
        }
        return diagnosis("PHP diagnosis");
      }
      return {};
    });

    render(
      <LogDoctorModal
        initialService={ServiceType.Caddy}
        services={{}}
        appPaths={null}
        onClose={() => {}}
        onOpenSettings={() => {}}
        onStartPhp={() => {}}
      />
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("diagnose_service", { service: ServiceType.Caddy })
    );
    fireEvent.click(screen.getByRole("tab", { name: "PHP-FPM" }));
    expect(await screen.findByText("PHP diagnosis")).toBeInTheDocument();

    finishCaddy(diagnosis("Caddy diagnosis"));
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText("PHP diagnosis")).toBeInTheDocument();
    expect(screen.queryByText("Caddy diagnosis")).not.toBeInTheDocument();
  });
});
