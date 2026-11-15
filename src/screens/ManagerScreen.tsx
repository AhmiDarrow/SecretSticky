import { useCallback, useEffect, useState, type FormEvent } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { clearClipboard } from "../clipboard";
import {
  AUTO_LOCK_PRESET_SECS,
  errorText,
  formatDuration,
  formatNumber,
  LOCALES,
  localizeMessage,
  pluralCategory,
  Template,
  useI18n,
} from "../i18n";
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

type Tone = "ok" | "err" | "busy";
interface ToneMsg {
  tone: Tone;
  text: string;
}

interface Props {
  onLock: () => void | Promise<void>;
}

export function ManagerScreen({ onLock }: Props) {
  const { t, locale, setLocale } = useI18n();
  const [notes, setNotes] = useState<NotePreviewDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showAutoLock, setShowAutoLock] = useState(false);
  const [showLanguage, setShowLanguage] = useState(false);
  const [autoLockSecs, setAutoLockSecs] = useState<number>(DEFAULT_AUTO_LOCK_SECS);
  const [deleteTarget, setDeleteTarget] = useState<NotePreviewDto | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [currentPw, setCurrentPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [confirmPw, setConfirmPw] = useState("");
  const [pwMsg, setPwMsg] = useState<ToneMsg | null>(null);
  const [appVersion, setAppVersion] = useState<string>("…");
  const [updateMsg, setUpdateMsg] = useState<ToneMsg | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [pendingVersion, setPendingVersion] = useState<string | null>(null);

  const showError = (err: unknown) => {
    setError(localizeMessage(errorText(err), locale, t));
  };

  const colorName = (id: string) => t(`color.${id}`);
  const noteTitle = (title: string) => title.trim() || t("notes.untitled");

  const refresh = useCallback(async () => {
    try {
      const list = await api.listNotes();
      setNotes(list);
      setError(null);
    } catch (e) {
      setError(localizeMessage(errorText(e), locale, t));
    }
  }, [locale, t]);

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
        setUpdateMsg({
          tone: "ok",
          text: t("update.upToDate", { version: appVersion }),
        });
      } else if (result.kind === "available") {
        setPendingVersion(result.version);
        setUpdateMsg({
          tone: "ok",
          text: t("update.ready", { version: result.version }),
        });
      } else {
        setUpdateMsg({
          tone: "err",
          text: localizeMessage(result.message, locale, t),
        });
      }
    } finally {
      setUpdateBusy(false);
    }
  };

  const runUpdateInstall = async () => {
    setUpdateBusy(true);
    setUpdateMsg({ tone: "busy", text: t("update.downloading") });
    try {
      const ok = await downloadAndInstallUpdate((pct) => {
        if (pct == null) {
          setUpdateMsg({ tone: "busy", text: t("update.downloading") });
        } else {
          setUpdateMsg({
            tone: "busy",
            text: t("update.downloadingPct", { pct: formatNumber(locale, pct) }),
          });
        }
      });
      if (!ok) {
        setUpdateMsg({
          tone: "ok",
          text: t("update.upToDate", { version: appVersion }),
        });
        setPendingVersion(null);
      }
    } catch (e) {
      setUpdateMsg({
        tone: "err",
        text: localizeMessage(errorText(e), locale, t),
      });
    } finally {
      setUpdateBusy(false);
    }
  };

  const create = (color?: string) => {
    setCreating(true);
    setError(null);
    void api
      .createNote(color as never)
      .then(() => refresh())
      .catch((e) => showError(e))
      .finally(() => setCreating(false));
  };

  const lock = async () => {
    setBusy(true);
    try {
      await clearClipboard();
      await api.lock();
      await onLock();
    } catch (e) {
      showError(e);
    } finally {
      setBusy(false);
    }
  };

  const changePassword = async (e: FormEvent) => {
    e.preventDefault();
    setPwMsg(null);
    setError(null);
    if (newPw.length < 12) {
      setPwMsg({ tone: "err", text: t("password.errShort") });
      return;
    }
    if (newPw !== confirmPw) {
      setPwMsg({ tone: "err", text: t("password.errMismatch") });
      return;
    }
    setBusy(true);
    try {
      await api.changePassword(currentPw, newPw);
      setCurrentPw("");
      setNewPw("");
      setConfirmPw("");
      setPwMsg({ tone: "ok", text: t("password.updated") });
      setShowPassword(false);
    } catch (err) {
      setPwMsg({
        tone: "err",
        text: localizeMessage(errorText(err), locale, t),
      });
    } finally {
      setBusy(false);
    }
  };

  const applyAutoLock = async (secs: number) => {
    const safe = Number.isFinite(secs)
      ? Math.min(Math.max(Math.round(secs), 0), 12 * 3600)
      : 0;
    setBusy(true);
    setError(null);
    try {
      await api.setIdleLockSecs(safe);
      setAutoLockSecs(safe);
    } catch (e) {
      showError(e);
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
      showError(e);
    } finally {
      setDeleting(false);
    }
  };

  const closePanels = () => {
    setShowAbout(false);
    setShowPassword(false);
    setShowAutoLock(false);
    setShowLanguage(false);
    setDeleteTarget(null);
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
            <p className="muted">{t("manager.tagline")}</p>
          </div>
        </div>
        <div className="header-actions">
          <button
            type="button"
            onClick={() => {
              const next = !showAbout;
              closePanels();
              setShowAbout(next);
            }}
            disabled={busy}
          >
            {t("manager.about")}
          </button>
          <button
            type="button"
            onClick={() => {
              const next = !showPassword;
              closePanels();
              setPwMsg(null);
              setShowPassword(next);
            }}
            disabled={busy}
          >
            {t("manager.password")}
          </button>
          <button
            type="button"
            aria-expanded={showAutoLock}
            className={autoLockSecs === 0 ? undefined : "primary"}
            title={
              autoLockSecs === 0
                ? t("autoLock.titleOff")
                : t("autoLock.titleAfter", {
                    duration: formatDuration(locale, autoLockSecs, t),
                  })
            }
            onClick={() => {
              const next = !showAutoLock;
              closePanels();
              setShowAutoLock(next);
            }}
            disabled={busy}
          >
            {t("manager.autoLock")}
          </button>
          <button
            type="button"
            aria-expanded={showLanguage}
            onClick={() => {
              const next = !showLanguage;
              closePanels();
              setShowLanguage(next);
            }}
            disabled={busy}
          >
            {t("manager.language")}
          </button>
          <button type="button" onClick={() => api.openAll()} disabled={busy}>
            {t("manager.openAll")}
          </button>
          <button type="button" className="danger" onClick={lock} disabled={busy}>
            {t("manager.lock")}
          </button>
        </div>
      </header>

      {showAbout && (
        <section className="about-panel" aria-label={t("about.aria")}>
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
              <h2>{t("about.title")}</h2>
              <p className="about-hello">{t("about.hello")}</p>
              <p className="muted fine-inline">{t("about.blurb")}</p>
              <p className="muted fine-inline about-version">
                {t("about.version", { version: appVersion })}
              </p>
              <div className="about-links">
                <button
                  type="button"
                  className="primary"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_PROFILE).catch((e) => showError(e));
                  }}
                >
                  {t("about.githubProfile")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_REPO).catch((e) => showError(e));
                  }}
                >
                  {t("about.projectRepo")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void api.openExternalUrl(GITHUB_RELEASES).catch((e) => showError(e));
                  }}
                >
                  {t("about.releases")}
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
                    ? t("about.checking")
                    : t("about.checkUpdates")}
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
                      ? t("about.installing")
                      : t("about.installRestart", { version: pendingVersion })}
                  </button>
                )}
              </div>
              {updateMsg && (
                <p
                  className={
                    updateMsg.tone === "ok"
                      ? "ok fine-inline"
                      : updateMsg.tone === "err"
                        ? "error fine-inline"
                        : "fine-inline"
                  }
                >
                  {updateMsg.text}
                </p>
              )}
              <div className="about-footer">
                <span className="muted fine-inline about-footer-meta">
                  {t("about.localVault")}
                </span>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => setShowAbout(false)}
                >
                  {t("common.close")}
                </button>
              </div>
            </div>
          </div>
        </section>
      )}

      {showPassword && (
        <section className="password-panel">
          <h2>{t("password.title")}</h2>
          <p className="muted fine-inline">{t("password.blurb")}</p>
          <form className="password-form" onSubmit={changePassword}>
            <label>
              {t("password.current")}
              <input
                type="password"
                autoComplete="current-password"
                value={currentPw}
                onChange={(e) => setCurrentPw(e.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              {t("password.new")}
              <input
                type="password"
                autoComplete="new-password"
                value={newPw}
                onChange={(e) => setNewPw(e.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              {t("password.confirm")}
              <input
                type="password"
                autoComplete="new-password"
                value={confirmPw}
                onChange={(e) => setConfirmPw(e.target.value)}
                disabled={busy}
              />
            </label>
            {pwMsg && (
              <p className={pwMsg.tone === "ok" ? "ok" : "error"}>{pwMsg.text}</p>
            )}
            <div className="password-actions">
              <button type="submit" className="primary" disabled={busy}>
                {busy ? t("password.updating") : t("password.submit")}
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
                {t("common.cancel")}
              </button>
            </div>
          </form>
        </section>
      )}

      {showAutoLock && (
        <section className="about-panel auto-lock-panel" aria-label={t("autoLock.aria")}>
          <div className="auto-lock-head">
            <div>
              <h2>{t("autoLock.title")}</h2>
              <p className="muted fine-inline">{t("autoLock.blurb")}</p>
            </div>
            <span className="auto-lock-now" aria-live="polite">
              {autoLockSecs === 0
                ? t("autoLock.off")
                : t("autoLock.now", {
                    duration: formatDuration(locale, autoLockSecs, t),
                  })}
            </span>
          </div>
          <div className="auto-lock-presets" role="group" aria-label={t("autoLock.presetsAria")}>
            {AUTO_LOCK_PRESET_SECS.map((secs) => (
              <button
                key={secs}
                type="button"
                className={autoLockSecs === secs ? "primary" : undefined}
                disabled={busy}
                title={
                  secs === 0
                    ? t("autoLock.never")
                    : t("autoLock.lockAfter", {
                        duration: formatDuration(locale, secs, t),
                      })
                }
                onClick={() => {
                  void applyAutoLock(secs);
                }}
              >
                {formatDuration(locale, secs, t)}
              </button>
            ))}
          </div>
          <div className="about-footer auto-lock-footer">
            <span className="muted fine-inline about-footer-meta">
              {t("autoLock.footnote")}
            </span>
            <button
              type="button"
              className="ghost"
              onClick={() => setShowAutoLock(false)}
            >
              {t("autoLock.done")}
            </button>
          </div>
        </section>
      )}

      {showLanguage && (
        <section className="about-panel" aria-label={t("language.aria")}>
          <h2>{t("language.title")}</h2>
          <p className="muted fine-inline">{t("language.blurb")}</p>
          <div className="lang-grid" role="group" aria-label={t("language.aria")}>
            {LOCALES.map((item) => (
              <button
                key={item.id}
                type="button"
                className={locale === item.id ? "primary" : undefined}
                aria-pressed={locale === item.id}
                onClick={() => {
                  void setLocale(item.id);
                }}
              >
                {item.native}
              </button>
            ))}
          </div>
          <div className="about-footer">
            <span className="muted fine-inline about-footer-meta">
              {t("language.current")}: {LOCALES.find((item) => item.id === locale)?.native}
            </span>
            <button
              type="button"
              className="ghost"
              onClick={() => setShowLanguage(false)}
            >
              {t("common.close")}
            </button>
          </div>
        </section>
      )}

      <section className="new-row">
        <div className="new-row-main">
          <span className="label">{t("notes.new")}</span>
          <div className="swatches" aria-label={t("notes.colorsAria")}>
            {COLORS.map((c) => (
              <button
                key={c.id}
                type="button"
                className={`swatch${c.dark ? " dark-swatch" : ""}`}
                title={colorName(c.id)}
                aria-label={t("notes.newColor", { color: colorName(c.id) })}
                style={{ background: c.css }}
                disabled={busy}
                onClick={() => create(c.id)}
              />
            ))}
          </div>
        </div>
        {creating && (
          <span className="muted fine-inline new-row-status">{t("notes.opening")}</span>
        )}
      </section>

      {error && <p className="error pad">{error}</p>}

      <ul className="note-list">
        {notes.length === 0 && <li className="empty">{t("notes.empty")}</li>}
        {notes.map((n) => (
          <li key={n.id} className="note-row">
            <button
              type="button"
              className="note-open"
              onClick={() => {
                void api.openNote(n.id).catch((e) => showError(e));
              }}
            >
              <span className="dot" style={{ background: n.color_css }} aria-hidden />
              <span className="note-title">{noteTitle(n.title)}</span>
              <span className="note-preview muted">
                {t("notes.openToEdit", { color: colorName(n.color) })}
              </span>
            </button>
            <button
              type="button"
              className="note-delete ghost danger"
              title={t("notes.deleteTitle")}
              aria-label={t("notes.deleteAria", { title: noteTitle(n.title) })}
              onClick={() => {
                closePanels();
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
            {t(`manager.count.${pluralCategory(locale, notes.length)}`, {
              count: formatNumber(locale, notes.length),
            })}
          </span>
          <span className="muted">{t("notes.trayHint")}</span>
          <span className="muted">
            {t("notes.copyClears", { seconds: formatNumber(locale, 30) })}
          </span>
        </div>
        <div className="footer-actions">
          <button
            type="button"
            className="ghost"
            title={t("notes.hideTitle")}
            disabled={busy}
            onClick={() => {
              void api.hideMain();
            }}
          >
            {t("notes.hide")}
          </button>
          <button
            type="button"
            className="ghost danger"
            title={t("notes.quitTitle")}
            disabled={busy}
            onClick={() => {
              void clearClipboard().finally(() => {
                void api.quitApp();
              });
            }}
          >
            {t("common.quit")}
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
            <h2 id="delete-dialog-title">{t("delete.title")}</h2>
            <p id="delete-dialog-desc" className="muted fine-inline">
              <Template
                text={t("delete.body")}
                slots={{
                  title: (
                    <strong className="modal-note-name">
                      {noteTitle(deleteTarget.title)}
                    </strong>
                  ),
                }}
              />
            </p>
            <div className="modal-actions">
              <button
                type="button"
                className="ghost"
                disabled={deleting}
                onClick={() => setDeleteTarget(null)}
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                className="danger-solid"
                disabled={deleting}
                onClick={() => {
                  void confirmDelete();
                }}
              >
                {deleting ? t("common.deleting") : t("common.delete")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
