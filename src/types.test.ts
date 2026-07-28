import { describe, expect, it } from "vitest";
import { applyNoteTheme, COLORS, type NoteColor } from "./types";

describe("COLORS palette", () => {
  it("has eight sticky colors with unique ids", () => {
    expect(COLORS).toHaveLength(8);
    const ids = COLORS.map((c) => c.id);
    expect(new Set(ids).size).toBe(8);
  });

  it("every color has hex background and text", () => {
    for (const c of COLORS) {
      expect(c.css).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(c.text).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(c.label.length).toBeGreaterThan(0);
    }
  });

  it("marks only dark stickies as dark", () => {
    const dark = COLORS.filter((c) => c.dark).map((c) => c.id);
    expect(dark.sort()).toEqual(["black", "darkgreen"].sort());
  });

  it("includes required product colors", () => {
    const ids = new Set(COLORS.map((c) => c.id));
    for (const need of [
      "yellow",
      "black",
      "darkgreen",
      "green",
      "pink",
      "blue",
      "purple",
      "gray",
    ] as NoteColor[]) {
      expect(ids.has(need)).toBe(true);
    }
  });
});

describe("applyNoteTheme", () => {
  it("sets CSS variables and body colors", () => {
    applyNoteTheme("#ffe566", "#1a1508");
    const root = document.documentElement;
    expect(root.style.getPropertyValue("--note-bg")).toBe("#ffe566");
    expect(root.style.getPropertyValue("--note-text")).toBe("#1a1508");
    expect(root.style.getPropertyValue("--note-fg")).toBe("#1a1508");
    expect(document.body.style.background).toBe("rgb(255, 229, 102)");
    expect(document.body.style.color).toBe("rgb(26, 21, 8)");
  });
});
