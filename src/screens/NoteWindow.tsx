import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../api";
import { copySecret } from "../clipboard";
import {
  COLORS,
  applyNoteTheme,
  type NoteColor,
  type NoteDto,
} from "../types";

interface Props {
  noteId: string;
}

export function NoteWindow({ noteId }: Props) {
  const [note, setNote] = useState<NoteDto | null>(null);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(true);
  const [copyHint, setCopyHint] = useState<string | null>(null);

  const saveTimer = useRef<number | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement | null>(null);
  const titleRef = useRef<HTMLInputElement | null>(null);
  // Stable window handle — never put getCurrentWindow() in hook deps (new ref each render → reload fight).
  const winRef = useRef(getCurrentWindow());
  // Latest draft values for debounced save (avoids stale closures + partial patches).
  const titleDraft = useRef("");
  const bodyDraft = useRef("");
  const colorDraft = useRef<NoteColor | null>(null);
  const alwaysOnTopDraft = useRef<boolean | null>(null);
  const loadedOnce = useRef(false);
  const saveGen = useRef(0);
  const dirty = useRef(false);

  const applyTheme = (n: Pick<NoteDto, "color_css" | "color_text_css" | "color">) => {
    const text =
      n.color_text_css ||
      COLORS.find((c) => c.id === n.color)?.text ||
      "#1f2937";
    applyNoteTheme(n.color_css, text);
  };

  const load = useCallback(async () => {
    try {
      const n = await api.getNote(noteId);
      // If user already typed during load, never clobber their draft.
      if (dirty.current && loadedOnce.current) {
        setNote((prev) => prev ?? n);
        return;
      }
      setNote(n);
      if (!dirty.current) {
        setTitle(n.title);
        setBody(n.body);
        titleDraft.current = n.title;
        bodyDraft.current = n.body;
      }
      colorDraft.current = n.color;
      alwaysOnTopDraft.current = n.always_on_top;
      applyTheme(n);
      setError(null);
      loadedOnce.current = true;
      try {
        await winRef.current.setTitle(n.title.trim() || "Sticky note");
      } catch {
        /* ignore */
      }
      window.setTimeout(() => {
        bodyRef.current?.focus();
      }, 50);
    } catch (e) {
      setError(String(e));
    }
  }, [noteId]);

  // Load exactly once per noteId — do not re-run on every keystroke.
  useEffect(() => {
    loadedOnce.current = false;
    dirty.current = false;
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [noteId]);

  const flushSave = useCallback(async () => {
    if (!loadedOnce.current) return;
    const gen = ++saveGen.current;
    setSaved(false);
    try {
      const updated = await api.updateNote({
        id: noteId,
        title: titleDraft.current,
        body: bodyDraft.current,
        color: colorDraft.current ?? undefined,
        always_on_top: alwaysOnTopDraft.current ?? undefined,
      });
      // Ignore out-of-order responses so a slow save can't rewind the editor.
      if (gen !== saveGen.current) return;
      setNote((prev) => {
        if (!prev) return updated;
        return {
          ...prev,
          // Keep local draft text as source of truth while typing.
          title: titleDraft.current,
          body: bodyDraft.current,
          color: colorDraft.current ?? updated.color,
          color_css: updated.color_css,
          color_text_css: updated.color_text_css,
          always_on_top: alwaysOnTopDraft.current ?? updated.always_on_top,
          updated_at: updated.updated_at,
        };
      });
      setSaved(true);
      try {
        await winRef.current.setTitle(titleDraft.current.trim() || "Sticky note");
      } catch {
        /* ignore */
      }
    } catch (e) {
      if (gen === saveGen.current) {
        setError(String(e));
      }
    }
  }, [noteId]);

  const scheduleSave = useCallback(() => {
    dirty.current = true;
    setSaved(false);
    if (saveTimer.current) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      void flushSave();
    }, 400);
  }, [flushSave]);

  const persistGeometry = useCallback(async () => {
    try {
      const win = winRef.current;
      const pos = await win.outerPosition();
      const size = await win.innerSize();
      const scale = await win.scaleFactor();
      // Coalesce geometry with latest draft so we don't drop in-flight body saves.
      await api.updateNote({
        id: noteId,
        title: titleDraft.current,
        body: bodyDraft.current,
        color: colorDraft.current ?? undefined,
        always_on_top: alwaysOnTopDraft.current ?? undefined,
        x: pos.x / scale,
        y: pos.y / scale,
        width: size.width / scale,
        height: size.height / scale,
      });
    } catch {
      /* ignore */
    }
  }, [noteId]);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    let moveTimer: number | null = null;
    let resizeTimer: number | null = null;
    const win = winRef.current;

    win
      .onMoved(() => {
        if (moveTimer) window.clearTimeout(moveTimer);
        moveTimer = window.setTimeout(() => {
          void persistGeometry();
        }, 250);
      })
      .then((u) => unsubs.push(u));
    win
      .onResized(() => {
        if (resizeTimer) window.clearTimeout(resizeTimer);
        resizeTimer = window.setTimeout(() => {
          void persistGeometry();
        }, 250);
      })
      .then((u) => unsubs.push(u));

    return () => {
      unsubs.forEach((u) => u());
      if (saveTimer.current) {
        window.clearTimeout(saveTimer.current);
        // Best-effort final flush on unmount.
        void flushSave();
      }
      if (moveTimer) window.clearTimeout(moveTimer);
      if (resizeTimer) window.clearTimeout(resizeTimer);
    };
  }, [noteId, persistGeometry, flushSave]);

  const doCopy = async (text: string, label: string) => {
    if (!text) {
      setCopyHint("Nothing to copy");
      window.setTimeout(() => setCopyHint(null), 1500);
      return;
    }
    const res = await copySecret(text, 30_000);
    if (!res.ok) {
      setError(res.error ?? "Clipboard copy failed");
      return;
    }
    setCopyHint(`${label} · clears in ${Math.round(res.clearedInMs / 1000)}s`);
    window.setTimeout(() => setCopyHint(null), 2500);
  };

  if (error && !note) {
    return (
      <div className="note-shell">
        <div className="note-titlebar" data-tauri-drag-region>
          <span className="pad muted">SecretSticky</span>
          <div className="titlebar-actions" data-tauri-drag-region="false">
            <button
              type="button"
              className="ghost no-drag"
              title="Close"
              onClick={() => winRef.current.close()}
            >
              ✕
            </button>
          </div>
        </div>
        <p className="error pad">{error}</p>
        <p className="pad muted">
          Close this window and open the note again from the manager.
        </p>
      </div>
    );
  }

  if (!note) {
    return (
      <div className="note-shell">
        <p className="pad muted">Loading…</p>
      </div>
    );
  }

  const isDark =
    note.color === "black" ||
    note.color === "darkgreen" ||
    COLORS.find((c) => c.id === note.color)?.dark === true;

  return (
    <div className={`note-shell${isDark ? " note-dark" : ""}`}>
      <div className="note-titlebar" data-tauri-drag-region>
        <div className="swatches compact no-drag" data-tauri-drag-region="false">
          {COLORS.map((c) => (
            <button
              key={c.id}
              type="button"
              className={`swatch tiny no-drag${note.color === c.id ? " active" : ""}${
                c.dark ? " dark-swatch" : ""
              }`}
              style={{ background: c.css }}
              title={c.label}
              onClick={() => {
                colorDraft.current = c.id;
                setNote({
                  ...note,
                  color: c.id,
                  color_css: c.css,
                  color_text_css: c.text,
                });
                applyNoteTheme(c.css, c.text);
                scheduleSave();
              }}
            />
          ))}
        </div>
        <div className="titlebar-actions no-drag" data-tauri-drag-region="false">
          <span className="save-dot" title={saved ? "Saved" : "Saving…"}>
            {saved ? "✓" : "…"}
          </span>
          <button
            type="button"
            className="ghost no-drag"
            title="Copy selection or full body (auto-clears in 30s)"
            onClick={async () => {
              const el = bodyRef.current;
              const selected =
                el && el.selectionStart !== el.selectionEnd
                  ? body.slice(el.selectionStart, el.selectionEnd)
                  : "";
              if (selected) {
                await doCopy(selected, "Copied selection");
              } else {
                await doCopy(body, "Copied body");
              }
            }}
          >
            ⎘
          </button>
          <button
            type="button"
            className="ghost no-drag"
            title={note.always_on_top ? "Unpin" : "Always on top"}
            onClick={() => {
              const next = !note.always_on_top;
              alwaysOnTopDraft.current = next;
              setNote({ ...note, always_on_top: next });
              scheduleSave();
            }}
          >
            {note.always_on_top ? "📌" : "📍"}
          </button>
          <button
            type="button"
            className="ghost no-drag"
            title="Close"
            onClick={async () => {
              if (saveTimer.current) {
                window.clearTimeout(saveTimer.current);
                saveTimer.current = null;
              }
              await flushSave();
              await persistGeometry();
              try {
                await winRef.current.destroy();
              } catch {
                await winRef.current.close();
              }
            }}
          >
            ✕
          </button>
        </div>
      </div>
      <input
        ref={titleRef}
        className="note-title-input no-drag"
        data-tauri-drag-region="false"
        placeholder="Title"
        value={title}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        tabIndex={0}
        onChange={(e) => {
          const v = e.target.value;
          titleDraft.current = v;
          setTitle(v);
          scheduleSave();
        }}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      />
      <textarea
        ref={bodyRef}
        className="note-body no-drag"
        data-tauri-drag-region="false"
        placeholder="Secrets, API keys, passwords…"
        value={body}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoFocus
        tabIndex={0}
        onChange={(e) => {
          const v = e.target.value;
          bodyDraft.current = v;
          setBody(v);
          scheduleSave();
        }}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
        onCopy={(e) => {
          // Intercept native copy so secrets still auto-clear.
          const el = e.currentTarget;
          const selected = body.slice(el.selectionStart, el.selectionEnd);
          if (!selected) return;
          e.preventDefault();
          void doCopy(selected, "Copied");
        }}
      />
      {(copyHint || error) && (
        <p className={`note-status${error && !copyHint ? " error" : ""}`}>
          {copyHint ?? error}
        </p>
      )}
    </div>
  );
}
