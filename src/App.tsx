import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import { clearClipboard } from "./clipboard";
import type { VaultStatus } from "./types";
import { UnlockScreen } from "./screens/UnlockScreen";
import { ManagerScreen } from "./screens/ManagerScreen";
import { NoteWindow } from "./screens/NoteWindow";
import "./App.css";

declare global {
  interface Window {
    __SECRETSTICKY_NOTE_ID__?: string;
  }
}

/** Resolve sticky id: init-script → window label → ?note= → #note= */
function resolveNoteId(): string | null {
  // 1) Injected by Rust initialization_script (most reliable)
  const injected = window.__SECRETSTICKY_NOTE_ID__;
  if (injected && injected.length > 0) {
    return injected;
  }

  // 2) Tauri window label `note-<uuid>`
  try {
    const label = getCurrentWindow().label;
    if (label.startsWith("note-") && label.length > 5) {
      return label.slice(5);
    }
  } catch {
    /* not in tauri / too early */
  }

  // 3) Query string (legacy)
  try {
    const q = new URLSearchParams(window.location.search);
    const fromQuery = q.get("note");
    if (fromQuery && fromQuery.length > 0) {
      return fromQuery;
    }
  } catch {
    /* ignore */
  }

  // 4) Hash fallback: #note=<id>
  const hash = window.location.hash.replace(/^#/, "");
  if (hash.startsWith("note=")) {
    return decodeURIComponent(hash.slice(5));
  }

  return null;
}

export default function App() {
  const [noteId, setNoteId] = useState<string | null>(() => resolveNoteId());
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [boot, setBoot] = useState(true);

  const refresh = async () => {
    try {
      const s = await api.status();
      setStatus(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBoot(false);
    }
  };

  useEffect(() => {
    // Label / init script can settle a tick after first paint in new webviews.
    if (!noteId) {
      let tries = 0;
      const t = window.setInterval(() => {
        tries += 1;
        const id = resolveNoteId();
        if (id) {
          setNoteId(id);
          window.clearInterval(t);
        } else if (tries >= 20) {
          window.clearInterval(t);
        }
      }, 50);
      return () => window.clearInterval(t);
    }
  }, [noteId]);

  useEffect(() => {
    // Note windows: mark body early so we never flash the dark manager chrome.
    if (noteId) {
      document.documentElement.dataset.mode = "note";
      document.body.classList.add("is-note-window");
    } else {
      document.documentElement.dataset.mode = "main";
    }

    refresh();
    const unsubs: Array<() => void> = [];
    listen("vault-locked", () => {
      void clearClipboard();
      refresh();
    }).then((u) => unsubs.push(u));
    listen("notes-changed", () => {
      // manager refreshes itself; note windows ignore
    }).then((u) => unsubs.push(u));

    // Only poll idle - do NOT touch here or the vault never auto-locks.
    const idle = window.setInterval(() => {
      api.checkIdle().catch(() => {});
    }, 30_000);

    const onActivity = () => {
      api.touch().catch(() => {});
    };
    window.addEventListener("pointerdown", onActivity);
    window.addEventListener("keydown", onActivity);

    return () => {
      unsubs.forEach((u) => u());
      clearInterval(idle);
      window.removeEventListener("pointerdown", onActivity);
      window.removeEventListener("keydown", onActivity);
    };
  }, [noteId]);

  if (boot || !status) {
    return (
      <div className={noteId ? "note-shell" : "boot"}>
        <div className={noteId ? "pad muted" : "boot-card"}>
          {!noteId && (
            <div className="logo-mark" aria-hidden>
              S
            </div>
          )}
          <p>{noteId ? "Loading note…" : "Loading SecretSticky…"}</p>
          {error && <p className="error">{error}</p>}
        </div>
      </div>
    );
  }

  // Dedicated sticky note window — independent of manager; never blocks it.
  if (noteId) {
    if (!status.unlocked) {
      return (
        <div className="note-locked">
          <p>Vault locked</p>
          <button type="button" onClick={() => api.showMain()}>
            Unlock
          </button>
        </div>
      );
    }
    return <NoteWindow noteId={noteId} />;
  }

  // Main manager / unlock gate
  if (!status.initialized || !status.unlocked) {
    return (
      <UnlockScreen
        status={status}
        onUnlocked={async () => {
          // Stay on the manager after unlock. Open notes explicitly
          // (list click / Open all / tray) so we don't cascade every sticky.
          await refresh();
        }}
      />
    );
  }

  return <ManagerScreen onLock={refresh} />;
}
