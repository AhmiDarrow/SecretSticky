import { useCallback, useEffect, useState, type FormEvent } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { clearClipboard } from "../clipboard";
import { COLORS, type NotePreviewDto } from "../types";
import {
  checkForAppUpdate,
  downloadAndInstallUpdate,
} from "../updater";
import appMark from "../assets/app-mark.png";

const GITHUB_PROFILE = "https://github.com/AhmiDarrow";
const GITHUB_REPO = "https://github.com/AhmiDarrow/SecretSticky";
const GITHUB_RELEASES = "https://github.com/AhmiDarrow/SecretSticky/releases";

/** Backend default: 15 minutes. Used until the live status arrives. */
const DEFAULT_AUTO_LOCK_SECS = 15 * 60;

/** Auto-lock presets — Off … 12 h. These are the only values the UI sends. */
const AUTO_LOCK_PRESETS: ReadonlyArray<{ secs: number; label: string }> = [
  { secs: 0, label: "Off" },
  { secs: 60, label: "1 min" },
  { secs: 300, label: "5 min" },
  { secs: 900, label: "15 min" },
  { secs: 1800, label: "30 min" },
  { secs: 3600, label: "1 hr" },
  { secs: 7200, label: "2 hr" },
  { secs: 14400, label: "4 hr" },
  { secs: 28800, label: "8 hr" },
  { secs: 43200, label: "12 hr" },
];

function formatAutoLock(secs: number): string {
  if (!Number.isFinite(secs) || secs <= 0) return "Off";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins} min`;
  const hrs = mins / 60;
  return `${hrs % 1 === 0 ? hrs : hrs.toFixed(1)} hr${hrs === 1 ? "" : "s"}`;
}

interface Props {
  onLock: () => void | Promise<void>;
}

export function ManagerScreen({ onLock }: Props) {
  const [notes, setNotes] = useState<NotePreviewDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showAutoLock, setShowAutoLock] = useState(false);
  const [autoLockSecs, setAutoLockSecs] = useState<number>(DEFAULT_AUTO_LOCK_SECS);
  const [deleteTarget, setDeleteTarget] = useState<NotePreviewDto | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [currentPw, setCurrentPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [confirmPw, setConfirmPw] = useState("");
  const [pwMsg, setPwMsg] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string>("…");
  const [updateMsg, setUpdateMsg] = useState<string | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [pendingVersion, setPendingVersion] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await api.listNotes();
      setNotes(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    void api
      .status()
      .then((s) => setAutoLockSecs(s.idle_lock_secs))
      .catch(() => setAutoLockSecs(DEFAULT_AUTO_LOCK_SECS));
    void getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion("—"));
    let unlisten: (() => void) | undefined;
    listen("notes-changed", () => {
      refresh();
    }).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, [refresh]);

  const runUpdateCheck = async () => {
    setUpdateBusy(true);
    setUpdateMsg(null);
    setPendingVersion(null);
    try {
      const result = await checkForAppUpdate();
      if (result.kind === "up-to-date") {
        setUpdateMsg(`You're on the latest version (${appVersion}).`);
      } else if (result.kind === "available") {
        setPendingVersion(result.version);
        setUpdateMsg(`Update ${result.version} is ready to install.`);
      } else {
        setUpdateMsg(result.message);
      }
    } finally {
      setUpdateBusy(false);
    }
  };

  const runUpdateInstall = async () => {
    setUpdateBusy(true);
    setUpdateMsg("Downloading update…");
    try {
      const ok = await downloadAndInstallUpdate((pct) => {
        if (pct == null) {
          setUpdateMsg("Downloading update…");
        } else {
          setUpdateMsg(`Downloading update… ${pct}%`);
        }
      });
      if (!ok) {
        setUpdateMsg(`You're on the latest version (${appVersion}).`);
        setPendingVersion(null);
      }
      // relaunch() exits the process on success
    } catch (e) {
      setUpdateMsg(String(e));
    } finally {
      setUpdateBusy(false);
    }
  };

  const create = (color?: string) => {
    // Fire-and-forget: Rust returns after vault write; sticky window opens async.
    // Do not block manager UI / other color clicks.
    setCreating(true);
    setError(null);
    void api
      .createNote(color as never)
      .then(() => refresh())
      .catch((e) => setError(String(e)))
      .finally(() => setCreating(false));
  };

  const lock = async () => {
    setBusy(true);
    try {
      await clearClipboard();
      // Backend vault_lock waits ~500ms after vault-about-to-lock so stickies
      // can flush their 400ms debounce; no extra client delay needed here.
      await api.lock();
      await onLock();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const changePassword = async (e: FormEvent) => {
    e.preventDefault();
    setPwMsg(null);
    setError(null);
    if (newPw.length < 12) {
      setPwMsg("New password must be at least 12 characters.");
      return;
    }
    if (newPw !== confirmPw) {
      setPwMsg("New passwords do not match.");
      return;
    }
    setBusy(true);
    try {
      await api.changePassword(currentPw, newPw);
      setCurrentPw("");
      setNewPw("");
      setConfirmPw("");
      setPwMsg("Password updated. Recovery key is unchanged.");
      setShowPassword(false);
    } catch (err) {
      setPwMsg(String(err));
    } finally {
      setBusy(false);
    }
  };

  const applyAutoLock = async (secs: number) => {
    // Sanitize client-side too: finite, integer, 0 ..= 12 h only.
    const safe = Number.isFinite(secs)
      ? Math.min(Math.max(Math.round(secs), 0), 12 * 3600)
      : 0;
    setBusy(true);
    setError(null);
    try {
      await api.setIdleLockSecs(safe);
      setAutoLockSecs(safe);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    setError(null);
    try {
      await api.deleteNote(deleteTarget.id);
      setDeleteTarget(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="manager">
      <header className="manager-header">
        <div className="brand-block">
          <img
            className="brand-mark"
            src={appMark}
            width={40}
            height={40}
            alt=""
            draggable={false}
          />
          <div className="brand-text">
            <h1>SecretSticky</h1>
            <p className="muted">Encrypted sticky notes</p>
          </div>
        </div>
        <div className="header-actions">
          <button
            type="button"
            onClick={() => {
              setShowAbout((v) => !v);
              if (!showAbout) {
                setShowPassword(false);
                setShowAutoLock(false);
                setDeleteTarget(null);
              }
            }}
            disabled={busy}
          >
            About
          </button>
          <button
            type="button"
            onClick={() => {
              setShowPassword((v) => !v);
              setPwMsg(null);
              if (!showPassword) {
                setShowAbout(false);
                setShowAutoLock(false);
                setDeleteTarget(null);
              }
            }}
            disabled={busy}
          >
            Password
          </button>
          <button
            type="button"
            aria-expanded={showAutoLock}
            className={autoLockSecs === 0 ? undefined : "primary"}
            title={
              autoLockSecs === 0
                ? "Auto-lock is off"
                : `Auto-lock after ${formatAutoLock(autoLockSecs)} of inactivity`
            }
            onClick={() => {
              setShowAutoLock((v) => !v);
              if (!showAutoLock) {
                setShowAbout(false);
                setShowPassword(false);
                setDeleteTarget(null);
              }
            }}
            disabled={busy}
          >
            Auto-lock
          </button>
          <button
            type="button"
            onClick={() => api.openAll()}
            disabled={busy}
          >
            Open all
          </button>
          <button type="button" className="danger" onClick={lock} disabled={busy}>
            Lock
          </button>
        </div>
      </header>

      {showAbout && (
        <section className="about-panel" aria-label="About SecretSticky">
          <div className="about-row">
            <img
              className="about-mark"
              src={appMark}
              width={44}
              height={44}
              alt="SecretSticky"
              draggable={false}
            />
            <div className="about-body">
              <h2>About</h2>
              <p className="about-hello">Hi I&apos;m Ahmi, hope this helps!</p>
              <p className="muted fine-inline">
                Local-only encrypted sticky notes for Windows. MIT licensed.
              </p>
              <p className="muted fine-inline about-version">
                Version {appVersion}
              </p>
              <div className="about-links">
                <button
                  type="button"
                  className="primary"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_PROFILE).catch((e) =>
                      setError(String(e)),
                    );
                  }}
                >
                  GitHub profile
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_REPO).catch((e) =>
                      setError(String(e)),
                    );
                  }}
                >
                  Project repo
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_RELEASES).catch((e) =>
                      setError(String(e)),
                    );
                  }}
                >
                  Releases
                </button>
              </div>
              <div className="about-update">
                <button
                  type="button"
                  disabled={busy || updateBusy}
                  onClick={() => {
                    void runUpdateCheck();
                  }}
                >
                  {updateBusy && !pendingVersion
                    ? "Checking…"
                    : "Check for updates"}
                </button>
                {pendingVersion && (
                  <button
                    type="button"
                    className="primary"
                    disabled={busy || updateBusy}
                    onClick={() => {
                      void runUpdateInstall();
                    }}
                  >
                    {updateBusy
                      ? "Installing…"
                      : `Install ${pendingVersion} & restart`}
                  </button>
                )}
              </div>
              {updateMsg && (
                <p
                  className={
                    updateMsg.startsWith("You're on") ||
                    updateMsg.startsWith("Update ")
                      ? "ok fine-inline"
                      : "error fine-inline"
                  }
                >
                  {updateMsg}
                </p>
              )}
              <div className="about-footer">
                <span className="muted fine-inline about-footer-meta">
                  Local vault · no cloud
                </span>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => setShowAbout(false)}
                >
                  Close
                </button>
              </div>
            </div>
          </div>
        </section>
      )}

      {showPassword && (
        <section className="password-panel">
          <h2>Change master password</h2>
          <p className="muted fine-inline">
            Re-wraps the vault key. Your recovery key stays valid.
          </p>
          <form className="password-form" onSubmit={changePassword}>
            <label>
              Current password
              <input
                type="password"
                autoComplete="current-password"
                value={currentPw}
                onChange={(e) => setCurrentPw(e.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              New password
              <input
                type="password"
                autoComplete="new-password"
                value={newPw}
                onChange={(e) => setNewPw(e.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              Confirm new password
              <input
                type="password"
                autoComplete="new-password"
                value={confirmPw}
                onChange={(e) => setConfirmPw(e.target.value)}
                disabled={busy}
              />
            </label>
            {pwMsg && (
              <p className={pwMsg.startsWith("Password updated") ? "ok" : "error"}>
                {pwMsg}
              </p>
            )}
            <div className="password-actions">
              <button type="submit" className="primary" disabled={busy}>
                {busy ? "Updating…" : "Update password"}
              </button>
              <button
                type="button"
                className="ghost"
                disabled={busy}
                onClick={() => {
                  setShowPassword(false);
                  setCurrentPw("");
                  setNewPw("");
                  setConfirmPw("");
                  setPwMsg(null);
                }}
              >
                Cancel
              </button>
            </div>
          </form>
        </section>
      )}

      {showAutoLock && (
        <section
          className="about-panel auto-lock-panel"
          aria-label="Auto-lock timer"
        >
          <div className="auto-lock-head">
            <div>
              <h2>Auto-lock</h2>
              <p className="muted fine-inline">
                Lock the vault after inactivity. Idle is checked roughly every
                30 seconds, so locking can lag by up to half a minute.
              </p>
            </div>
            <span className="auto-lock-now" aria-live="polite">
              {autoLockSecs === 0
                ? "Off"
                : `Now: ${formatAutoLock(autoLockSecs)}`}
            </span>
          </div>
          <div
            className="auto-lock-presets"
            role="group"
            aria-label="Auto-lock presets"
          >
            {AUTO_LOCK_PRESETS.map((p) => (
              <button
                key={p.secs}
                type="button"
                className={autoLockSecs === p.secs ? "primary" : undefined}
                disabled={busy}
                title={
                  p.secs === 0
                    ? "Never auto-lock"
                    : `Lock after ${p.label} of inactivity`
                }
                onClick={() => {
                  void applyAutoLock(p.secs);
                }}
              >
                {p.label}
              </button>
            ))}
          </div>
          <div className="about-footer auto-lock-footer">
            <span className="muted fine-inline about-footer-meta">
              Off keeps notes open until you lock or quit. Existing stickies are
              never touched by this setting.
            </span>
            <button
              type="button"
              className="ghost"
              onClick={() => setShowAutoLock(false)}
            >
              Done
            </button>
          </div>
        </section>
      )}

      <section className="new-row">
        <div className="new-row-main">
          <span className="label">New note</span>
          <div className="swatches" aria-label="Note colors">
            {COLORS.map((c) => (
              <button
                key={c.id}
                type="button"
                className={`swatch${c.dark ? " dark-swatch" : ""}`}
                title={c.label}
                aria-label={`New ${c.label} note`}
                style={{ background: c.css }}
                disabled={busy}
                onClick={() => create(c.id)}
              />
            ))}
          </div>
        </div>
        {creating && <span className="muted fine-inline new-row-status">Opening sticky…</span>}
      </section>

      {error && <p className="error pad">{error}</p>}

      <ul className="note-list">
        {notes.length === 0 && (
          <li className="empty">No notes yet — pick a color to create one.</li>
        )}
        {notes.map((n) => (
          <li key={n.id} className="note-row">
            <button
              type="button"
              className="note-open"
              onClick={() => {
                // Fire-and-forget — Rust opens window async; manager stays live.
                void api.openNote(n.id).catch((e) => setError(String(e)));
              }}
            >
              <span
                className="dot"
                style={{ background: n.color_css }}
                aria-hidden
              />
              <span className="note-title">
                {n.title.trim() || "Untitled note"}
              </span>
              <span className="note-preview muted">
                {n.color} · open to edit
              </span>
            </button>
            <button
              type="button"
              className="note-delete ghost danger"
              title="Delete note"
              aria-label={`Delete ${n.title.trim() || "Untitled note"}`}
              onClick={() => {
                setShowAbout(false);
                setShowPassword(false);
                setDeleteTarget(n);
              }}
            >
              ✕
            </button>
          </li>
        ))}
      </ul>

      <footer className="manager-footer">
        <div className="footer-meta">
          <span>
            {notes.length} note{notes.length === 1 ? "" : "s"} · local vault
          </span>
          <span className="muted">X → tray · stickies hide manager</span>
          <span className="muted">Copy clears in 30s</span>
        </div>
        <div className="footer-actions">
          <button
            type="button"
            className="ghost"
            title="Hide manager; app stays in the tray"
            disabled={busy}
            onClick={() => {
              void api.hideMain();
            }}
          >
            Hide to tray
          </button>
          <button
            type="button"
            className="ghost danger"
            title="Close all stickies and quit"
            disabled={busy}
            onClick={() => {
              void clearClipboard().finally(() => {
                void api.quitApp();
              });
            }}
          >
            Quit
          </button>
        </div>
      </footer>

      {deleteTarget && (
        <div
          className="modal-backdrop"
          role="presentation"
          onClick={() => {
            if (!deleting) setDeleteTarget(null);
          }}
        >
          <div
            className="modal-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="delete-dialog-title"
            aria-describedby="delete-dialog-desc"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="delete-dialog-title">Delete note?</h2>
            <p id="delete-dialog-desc" className="muted fine-inline">
              <strong className="modal-note-name">
                {deleteTarget.title.trim() || "Untitled note"}
              </strong>{" "}
              will be removed permanently from the vault.
            </p>
            <div className="modal-actions">
              <button
                type="button"
                className="ghost"
                disabled={deleting}
                onClick={() => setDeleteTarget(null)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="danger-solid"
                disabled={deleting}
                onClick={() => {
                  void confirmDelete();
                }}
              >
                {deleting ? "Deleting…" : "Delete"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
