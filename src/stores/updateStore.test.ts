import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { Update } from "@tauri-apps/plugin-updater";
import { useUpdateStore } from "./updateStore";

vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));

const makeUpdate = (download: () => Promise<void>) =>
  ({
    version: "9.9.9",
    download: vi.fn(download),
    install: vi.fn(async () => {}),
  }) as unknown as Update;

describe("update install", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUpdateStore.setState({ phase: "available", update: null, progress: null, error: null });
  });

  it("leaves services running when the download fails", async () => {
    const update = makeUpdate(() => Promise.reject(new Error("offline")));
    useUpdateStore.setState({ update });

    await useUpdateStore.getState().install();

    expect(invoke).not.toHaveBeenCalledWith("prepare_for_update");
    expect(update.install).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
    expect(useUpdateStore.getState().phase).toBe("error");
    expect(useUpdateStore.getState().error).toContain("offline");
  });

  it("stops services only after the download, then installs and relaunches", async () => {
    const order: string[] = [];
    const update = makeUpdate(async () => {
      order.push("download");
    });
    vi.mocked(update.install).mockImplementation(async () => {
      order.push("install");
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      order.push(command);
    });
    vi.mocked(relaunch).mockImplementation(async () => {
      order.push("relaunch");
    });
    useUpdateStore.setState({ update });

    await useUpdateStore.getState().install();

    expect(order).toEqual(["download", "prepare_for_update", "install", "relaunch"]);
  });
});
