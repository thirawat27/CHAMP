import { describe, expect, it } from "vitest";
import { formatBytes, formatModelSize } from "./format";

describe("formatBytes", () => {
  it("handles zero and invalid input", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });

  it("scales units and stops at TB", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 ** 5)).toBe("5120.0 TB");
  });
});

describe("formatModelSize", () => {
  it("rounds to whole megabytes", () => {
    expect(formatModelSize(30_920_426)).toBe("29 MB");
  });
});
