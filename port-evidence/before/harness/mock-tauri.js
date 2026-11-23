// Before-evidence harness: answers the shipped app's Tauri commands with sample data so
// each screen can be rendered in a headless browser. Query parameters pick the state:
//   state=setup|locked|unlocked   note=<id> (a sticky window)   act=Label1,Label2 (buttons
//   clicked in order after the first render)   fill=1|short (types sample text into inputs)
//   empty=1 (no notes)   idle=<secs> (auto-lock setting)
const q = new URLSearchParams(location.search);
const state = q.get("state") || "unlocked";
const noteId = q.get("note");
const COLORS = {
  yellow: ["#ffe566", "#1a1508"], green: ["#b8e08a", "#14210f"], pink: ["#f5a8c0", "#2a0f18"],
  blue: ["#7ec4f5", "#0c1a28"], purple: ["#c79be0", "#1c0f24"], gray: ["#d4d4d8", "#18181b"],
  black: ["#121212", "#fafafa"], darkgreen: ["#163d2c", "#ecfdf5"],
};
const sample = q.get("empty") ? [] : [
  ["n1", "Wi-Fi", "yellow", "Guest network: hunter2hunter2\nRouter admin: see drawer", true],
  ["n2", "Server keys", "darkgreen", "staging api key: sk-sample-0000\nrotate monthly", false],
  ["n3", "", "pink", "", false],
  ["n4", "Bank PIN reminder (sample)", "blue", "Not a real PIN.", false],
];
const notes = sample.map(([id, title, color, body, top]) => ({
  id, title, body, color, color_css: COLORS[color][0], color_text_css: COLORS[color][1],
  x: 120, y: 120, width: 345, height: 280, always_on_top: top,
  created_at: "2026-11-01T10:00:00Z", updated_at: "2026-11-01T10:00:00Z",
}));
const status = () => ({
  initialized: state !== "setup", unlocked: state === "unlocked", note_count: notes.length,
  idle_lock_secs: Number(q.get("idle") || 900), has_recovery_key: true,
});
const callbacks = new Map();
let next = 1;
window.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: noteId ? `note-${noteId}` : "main" },
    currentWebview: { label: noteId ? `note-${noteId}` : "main" },
  },
  plugins: { path: { sep: "/", delimiter: ";" } },
  transformCallback(cb) { const id = next++; callbacks.set(id, cb); return id; },
  unregisterCallback(id) { callbacks.delete(id); },
  runCallback(id, data) { callbacks.get(id)?.(data); },
  callbacks,
  convertFileSrc: (p) => p,
  async invoke(cmd, args) {
    switch (cmd) {
      case "vault_status": return status();
      case "vault_setup": return "ABCD-EFGH-JKLM-NPQR-STUV-WXYZ-2345-6789-ABCD-EFGH-JKLM-NPQR";
      case "vault_unlock": case "vault_unlock_recovery":
        throw "wrong password or corrupted vault";
      case "notes_list": return notes.map(({ body, ...p }) => p);
      case "notes_get": return notes.find((n) => n.id === args.id);
      case "notes_update": return { ...notes.find((n) => n.id === args.id), ...args };
      case "plugin:app|version": return "0.1.8";
      case "plugin:event|listen": return next++;
      case "plugin:updater|check": return null;
      default: return null;
    }
  },
};
// The window size the screen is shown at (headless browsers keep a minimum width, so the
// page is laid out at this size and the screenshot is cropped to it).
if (q.get("w") && q.get("h")) {
  const css = `width:${q.get("w")}px;height:${q.get("h")}px;overflow:hidden;`;
  document.documentElement.style.cssText += css;
  document.body.style.cssText += css;
}
// Scripted typing and clicks after the first render.
const acts = (q.get("act") || "").split(",").filter(Boolean);
const fill = q.get("fill");
async function run() {
  await new Promise((r) => setTimeout(r, 400));
  if (fill) {
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    for (const el of document.querySelectorAll("input")) {
      set.call(el, fill === "short" ? "short" : "correct horse battery");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }
  for (const label of acts) {
    await new Promise((r) => setTimeout(r, 250));
    const b = [...document.querySelectorAll("button")].find(
      (x) => x.textContent.trim() === label || x.getAttribute("aria-label") === label || x.title === label,
    );
    b?.click();
  }
}
run();
