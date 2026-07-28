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
    void getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion("0.1.2"));
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
      // Brief pause so open stickies can finish debounced saves before vault_lock.
      await new Promise((r) => window.setTimeout(r, 150));
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
    if (newPw.length < 8) {
      setPwMsg("New password must be at least 8 characters.");
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
          <div>
            <h1>SecretSticky</h1>
            <p className="muted">Encrypted sticky notes</p>
          </div>
        </div>
        <div className="header-actions">
          <button
            type="button"
            onClick={() => {
              setShowAbout((v) => !v);
              if (!showAbout) setShowPassword(false);
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
              if (!showPassword) setShowAbout(false);
            }}
            disabled={busy}
          >
            Password
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
              width={56}
              height={56}
              alt="SecretSticky"
              draggable={false}
            />
            <div>
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
                <button
                  type="button"
                  className="ghost"
                  onClick={() => setShowAbout(false)}
                >
                  Close
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

      <section className="new-row">
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
        {creating && <span className="muted fine-inline">Opening sticky…</span>}
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
              className="ghost danger"
              title="Delete"
              onClick={async () => {
                if (!confirm("Delete this note permanently?")) return;
                try {
                  await api.deleteNote(n.id);
                  await refresh();
                } catch (e) {
                  setError(String(e));
                }
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
          <span className="muted">
            X closes to tray · stickies hide manager · copy clears in 30s
          </span>
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
    </div>
  );
}
