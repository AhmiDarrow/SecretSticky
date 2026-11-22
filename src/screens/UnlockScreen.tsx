import { useState, type FormEvent } from "react";
import { api } from "../api";
import { copySecret } from "../clipboard";
import {
  errorText,
  LanguageSelect,
  localizeMessage,
  RichText,
  useI18n,
} from "../i18n";
import type { VaultStatus } from "../types";
import appMark from "../assets/app-mark.png";

interface Props {
  status: VaultStatus;
  onUnlocked: () => void | Promise<void>;
}

export function UnlockScreen({ status, onUnlocked }: Props) {
  const { t, locale } = useI18n();
  const isSetup = !status.initialized;
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [useRecovery, setUseRecovery] = useState(false);

  const showError = (err: unknown) => {
    setError(localizeMessage(errorText(err), locale, t));
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      if (isSetup) {
        if (password.length < 12) {
          throw new Error(t("unlock.errShort"));
        }
        if (password !== confirm) {
          throw new Error(t("unlock.errMismatch"));
        }
        const key = await api.setup(password);
        setRecoveryKey(key);
      } else if (useRecovery) {
        await api.unlockRecovery(password);
        setPassword("");
        await onUnlocked();
      } else {
        await api.unlock(password);
        setPassword("");
        await onUnlocked();
      }
    } catch (err) {
      showError(err);
    } finally {
      setBusy(false);
    }
  };

  if (recoveryKey) {
    return (
      <div className="gate">
        <div className="gate-card">
          <div className="logo-mark" aria-hidden>
            <img src={appMark} alt="" width={44} height={44} draggable={false} />
          </div>
          <h1>{t("recovery.title")}</h1>
          <p className="muted">
            <RichText text={t("recovery.body")} />
          </p>
          <pre className="recovery">{recoveryKey}</pre>
          <div className="gate-actions">
            <button
              type="button"
              className="primary"
              onClick={async () => {
                await copySecret(recoveryKey, 120_000);
                await onUnlocked();
              }}
            >
              {t("recovery.copyContinue")}
            </button>
            <button
              type="button"
              className="ghost"
              onClick={async () => {
                await onUnlocked();
              }}
            >
              {t("recovery.savedContinue")}
            </button>
          </div>
          <LanguageSelect compact />
          <p className="fineprint">{t("recovery.fineprint")}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="gate">
      <div className="gate-card">
        <div className="logo-mark" aria-hidden>
          <img src={appMark} alt="" width={44} height={44} draggable={false} />
        </div>
        <h1>{isSetup ? t("unlock.titleSetup") : t("unlock.titleUnlock")}</h1>
        <p className="muted">
          {isSetup ? t("unlock.blurbSetup") : t("unlock.blurbUnlock")}
        </p>
        <form onSubmit={submit} className="gate-form">
          <label>
            {isSetup
              ? t("unlock.masterPassword")
              : useRecovery
                ? t("unlock.recoveryKey")
                : t("unlock.masterPassword")}
            <input
              type={useRecovery ? "text" : "password"}
              autoFocus
              autoComplete={isSetup ? "new-password" : "current-password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy}
              spellCheck={false}
            />
          </label>
          {isSetup && (
            <label>
              {t("unlock.confirmPassword")}
              <input
                type="password"
                autoComplete="new-password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                disabled={busy}
              />
            </label>
          )}
          {error && <p className="error">{error}</p>}
          <button type="submit" className="primary" disabled={busy}>
            {busy
              ? t("common.working")
              : isSetup
                ? t("unlock.createVault")
                : t("common.unlock")}
          </button>
        </form>

        <div className="gate-actions">
          {!isSetup && status.has_recovery_key && (
            <button
              type="button"
              className="ghost"
              disabled={busy}
              onClick={() => {
                setUseRecovery((v) => !v);
                setPassword("");
                setError(null);
              }}
            >
              {useRecovery ? t("unlock.useMaster") : t("unlock.useRecovery")}
            </button>
          )}
          <button
            type="button"
            className="ghost"
            disabled={busy}
            title={t("unlock.hideTrayTitle")}
            onClick={() => {
              void api.hideMain();
            }}
          >
            {isSetup ? t("unlock.hideTraySetup") : t("unlock.hideTray")}
          </button>
          <button
            type="button"
            className="ghost danger"
            disabled={busy}
            title={t("unlock.quitTitle")}
            onClick={() => {
              void api.quitApp();
            }}
          >
            {t("common.quit")}
          </button>
        </div>

        <LanguageSelect compact />
        <p className="fineprint">{t("unlock.fineprint")}</p>
      </div>
    </div>
  );
}
