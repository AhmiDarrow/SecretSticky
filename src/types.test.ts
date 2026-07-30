import { describe, expect, it } from "vitest";
import { COLORS, applyNoteTheme, type NoteColor } from "./types";

describe("COLORS palette", () => {
  it("has eight sticky colors", () => {
    expect(COLORS).toHaveLength(8);
  });

  it("ids are unique and match NoteColor union", () => {
    const ids = COLORS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
    const expected: NoteColor[] = [
      "yellow",
      "green",
      "pink",
      "blue",
      "purple",
      "gray",
      "black",
      "darkgreen",
    ];
    expect(ids.sort()).toEqual([...expected].sort());
  });

  it("every entry has css hex and contrasting text", () => {
    for (const c of COLORS) {
      expect(c.css).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(c.text).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(c.label.length).toBeGreaterThan(0);
      expect(c.css.toLowerCase()).not.toBe(c.text.toLowerCase());
    }
  });

  it("marks only dark backgrounds as dark", () => {
    const dark = COLORS.filter((c) => c.dark).map((c) => c.id);
    expect(dark).toEqual(expect.arrayContaining(["black", "darkgreen"]));
    expect(dark).toHaveLength(2);
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
