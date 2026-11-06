import { describe, expect, it } from "vitest";
import {
  hasPrimaryModifier,
  isEditableTarget,
  isHelpKey,
  shortcutHint,
  shortcutKeys,
} from "./shortcuts";

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("shortcut helpers", () => {
  it("requires Ctrl/Cmd without Shift or Alt", () => {
    expect(hasPrimaryModifier(key({ ctrlKey: true }))).toBe(true);
    expect(hasPrimaryModifier(key({ metaKey: true }))).toBe(true);
    expect(hasPrimaryModifier(key({ ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(hasPrimaryModifier(key({ ctrlKey: true, altKey: true }))).toBe(false);
    expect(hasPrimaryModifier(key({}))).toBe(false);
  });

  it("recognizes ? in layouts where Shift+/ produces another character", () => {
    expect(isHelpKey(key({ key: "?" }))).toBe(true);
    expect(isHelpKey(key({ key: "ฦ", code: "Slash", shiftKey: true }))).toBe(true);
    expect(isHelpKey(key({ key: "/", code: "Slash" }))).toBe(false);
  });

  it("treats form fields and editable content as typing targets", () => {
    expect(isEditableTarget(document.createElement("input"))).toBe(true);
    expect(isEditableTarget(document.createElement("textarea"))).toBe(true);
    expect(isEditableTarget(document.createElement("select"))).toBe(true);
    expect(isEditableTarget(document.createElement("button"))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });

  it("formats keys for kbd rows and tooltips", () => {
    expect(shortcutKeys("start")).toBe("Ctrl S");
    expect(shortcutHint("settings")).toBe("Ctrl+,");
  });
});
