import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { GuideModal } from "./GuideModal";
import { GUIDE_SECTIONS } from "../manual/content";
import { useLanguageStore } from "../stores/languageStore";

describe("GuideModal", () => {
  beforeEach(() => {
    useLanguageStore.getState().setLanguage("en");
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("lists every topic in the contents", () => {
    render(<GuideModal onClose={() => {}} />);
    const nav = screen.getByRole("navigation", { name: /contents/i });
    for (const section of GUIDE_SECTIONS) {
      expect(nav).toHaveTextContent(section.title.en);
    }
  });

  it("filters topics by search text, including body content", () => {
    render(<GuideModal onClose={() => {}} />);
    fireEvent.change(screen.getByRole("searchbox", { name: /search the guide/i }), {
      target: { value: "ngrok-free" },
    });
    expect(screen.getByRole("heading", { name: "HTTPS Preview" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Keyboard shortcuts" })).not.toBeInTheDocument();
  });

  it("explains when nothing matches", () => {
    render(<GuideModal onClose={() => {}} />);
    fireEvent.change(screen.getByRole("searchbox", { name: /search the guide/i }), {
      target: { value: "zzzz-no-such-topic" },
    });
    expect(screen.getByText("No topics match your search")).toBeInTheDocument();
  });

  it("has Thai and English text for every block", () => {
    const texts = GUIDE_SECTIONS.flatMap((section) => [
      section.title,
      section.summary,
      ...section.blocks.flatMap((block) => {
        if (block.kind === "p" || block.kind === "tip" || block.kind === "warning") {
          return [block.text];
        }
        if (block.kind === "steps" || block.kind === "list") return block.items;
        if (block.kind === "keys") return block.items.map((item) => item.text);
        return [];
      }),
    ]);
    const missing = texts.filter((value) => !value.th.trim() || !value.en.trim());
    expect(missing).toEqual([]);
  });
});
