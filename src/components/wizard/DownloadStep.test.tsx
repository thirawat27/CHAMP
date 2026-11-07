import { beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { usePreferencesStore } from "../../stores/preferencesStore";
import { getTranslation } from "../../i18n/translations";
import { DownloadProgress } from "../../types/services";
import { DownloadStep } from "./DownloadStep";

const batch: DownloadProgress = {
  step: "downloading",
  percent: 41,
  currentComponent: "All 4",
  componentDisplay: "All Components",
  version: "",
  totalComponents: 4,
  downloadedBytes: 160,
  totalBytes: 396,
};

function renderStep(progress: DownloadProgress) {
  return render(<DownloadStep progress={progress} error={null} onDismissError={() => {}} />);
}

describe("DownloadStep", () => {
  beforeEach(() => {
    usePreferencesStore.setState({ language: "th", t: getTranslation("th") });
  });

  it("names a parallel batch in the selected language instead of the backend's English label", () => {
    renderStep(batch);

    expect(screen.getByRole("heading").textContent).toBe("ดาวน์โหลด ทุกส่วนประกอบ");
    expect(screen.getByText("ทั้งหมด 4 รายการ")).toBeTruthy();
    expect(screen.queryByText(/All/)).toBeNull();
  });

  it("keeps the real component name for a single download", () => {
    renderStep({
      ...batch,
      currentComponent: "Caddy",
      componentDisplay: "Caddy",
      version: "2.11.9",
      totalComponents: 1,
    });

    expect(screen.getByRole("heading").textContent).toBe("ดาวน์โหลด Caddy");
    expect(screen.getByText("2.11.9")).toBeTruthy();
  });
});
