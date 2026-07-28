import { invoke } from "@tauri-apps/api/core";
import type { NoteColor, NoteDto, NotePreviewDto, VaultStatus } from "./types";

/** Thin invoke wrappers — names match screen call sites. */
export const api = {
  status: () => invoke<VaultStatus>("vault_status"),
  setup: (password: string) => invoke<string>("vault_setup", { password }),
  unlock: (password: string) => invoke<void>("vault_unlock", { password }),
  unlockRecovery: (recoveryKey: string) =>
    invoke<void>("vault_unlock_recovery", { recoveryKey }),
  lock: () => invoke<void>("vault_lock"),
  touch: () => invoke<void>("vault_touch"),
  checkIdle: () => invoke<boolean>("vault_check_idle"),

  listNotes: () => invoke<NotePreviewDto[]>("notes_list"),
  getNote: (id: string) => invoke<NoteDto>("notes_get", { id }),
  createNote: (color?: NoteColor | string) =>
    invoke<NotePreviewDto>("notes_create", { color: color ?? null }),
  updateNote: (args: {
    id: string;
    title?: string | null;
    body?: string | null;
    color?: NoteColor | string | null;
    x?: number | null;
    y?: number | null;
    width?: number | null;
    height?: number | null;
    always_on_top?: boolean | null;
  }) => invoke<NoteDto>("notes_update", args),
  deleteNote: (id: string) => invoke<void>("notes_delete", { id }),
  openNote: (id: string) => invoke<void>("notes_open_window", { id }),
  openAll: () => invoke<void>("notes_open_all"),

  setIdleLockSecs: (secs: number) =>
    invoke<void>("set_idle_lock_secs", { secs }),
  changePassword: (current: string, newPassword: string) =>
    invoke<void>("change_password", {
      current,
      newPassword,
    }),

  showMain: () => invoke<void>("show_main"),
  hideMain: () => invoke<void>("hide_main"),
  quitApp: () => invoke<void>("quit_app"),
};
