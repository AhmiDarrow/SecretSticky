export type NoteColor =
  | "yellow"
  | "green"
  | "pink"
  | "blue"
  | "purple"
  | "gray"
  | "black"
  | "darkgreen";

export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
  note_count: number;
  idle_lock_secs: number;
  has_recovery_key: boolean;
}

/** Full note including body — only for the owning sticky window. */
export interface NoteDto {
  id: string;
  title: string;
  body: string;
  color: NoteColor;
  color_css: string;
  color_text_css: string;
  x: number;
  y: number;
  width: number;
  height: number;
  always_on_top: boolean;
  created_at: string;
  updated_at: string;
}

/** Manager list item — no body over IPC. */
export interface NotePreviewDto {
  id: string;
  title: string;
  color: NoteColor;
  color_css: string;
  color_text_css: string;
  x: number;
  y: number;
  width: number;
  height: number;
  always_on_top: boolean;
  created_at: string;
  updated_at: string;
}

/** Sticky palette — backgrounds tuned so body text stays high-contrast. */
export const COLORS: Array<{
  id: NoteColor;
  label: string;
  css: string;
  text: string;
  dark?: boolean;
}> = [
  { id: "yellow", label: "Yellow", css: "#ffe566", text: "#1a1508" },
  { id: "green", label: "Green", css: "#b8e08a", text: "#14210f" },
  { id: "pink", label: "Pink", css: "#f5a8c0", text: "#2a0f18" },
  { id: "blue", label: "Blue", css: "#7ec4f5", text: "#0c1a28" },
  { id: "purple", label: "Purple", css: "#c79be0", text: "#1c0f24" },
  { id: "gray", label: "Gray", css: "#d4d4d8", text: "#18181b" },
  { id: "black", label: "Black", css: "#121212", text: "#fafafa", dark: true },
  {
    id: "darkgreen",
    label: "Dark green",
    css: "#163d2c",
    text: "#ecfdf5",
    dark: true,
  },
];

export function applyNoteTheme(bg: string, text: string) {
  const root = document.documentElement;
  root.style.setProperty("--note-bg", bg);
  root.style.setProperty("--note-text", text);
  root.style.setProperty("--note-fg", text);
  document.body.style.background = bg;
  document.body.style.color = text;
}
