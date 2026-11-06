import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { useEscape } from "./useEscape";

function Layer({ onClose }: { onClose: () => void }) {
  useEscape(onClose);
  return null;
}

const pressEscape = () => fireEvent.keyDown(window, { code: "Escape" });

describe("useEscape", () => {
  it("closes only the newest layer, one per key press", () => {
    const settings = vi.fn();
    const guide = vi.fn();

    const { rerender } = render(
      <>
        <Layer onClose={settings} />
        <Layer onClose={guide} />
      </>
    );

    pressEscape();
    expect(guide).toHaveBeenCalledTimes(1);
    expect(settings).not.toHaveBeenCalled();

    rerender(<Layer onClose={settings} />);
    pressEscape();
    expect(settings).toHaveBeenCalledTimes(1);
    expect(guide).toHaveBeenCalledTimes(1);
  });

  it("keeps a layer's place when its inline handler changes", () => {
    const lower = vi.fn();
    const upper = vi.fn();
    const newLower = vi.fn();
    const { rerender } = render(
      <>
        <Layer onClose={lower} />
        <Layer onClose={upper} />
      </>
    );

    rerender(
      <>
        <Layer onClose={newLower} />
        <Layer onClose={upper} />
      </>
    );
    pressEscape();

    expect(upper).toHaveBeenCalledTimes(1);
    expect(newLower).not.toHaveBeenCalled();
  });

  it("ignores other keys", () => {
    const close = vi.fn();
    render(<Layer onClose={close} />);
    fireEvent.keyDown(window, { code: "KeyA" });
    expect(close).not.toHaveBeenCalled();
  });
});
