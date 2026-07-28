import { useState, type FormEvent } from "react";
import { api } from "../api";
import { copySecret } from "../clipboard";
import type { VaultStatus } from "../types";

interface Props {
  status: VaultStatus;
  onUnlocked: () => void | Promise<void>;
}

export function UnlockScreen({ status, onUnlocked }: Props) {
  const isSetup = !status.initialized;
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [useRecovery, setUseRecovery] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      if (isSetup) {
        if (password.length < 8) {
          throw new Error("Use at least 8 characters");
        }
        if (password !== confirm) {
          throw new Error("Passwords do not match");
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
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  if (recoveryKey) {
    return (
      <div className="gate">
        <div className="gate-card">
          <div className="logo-mark">S</div>
          <h1>Save your recovery key</h1>
          <p className="muted">
            This is shown <strong>once</strong>. Store it offline. Losing both
            your master password and this key means your notes cannot be
            recovered.
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
              Copy key (2 min) — continue
            </button>
            <button
              type="button"
              className="ghost"
              onClick={async () => {
                await onUnlocked();
              }}
            >
              I already saved it — continue
            </button>
          </div>
          <p className="fineprint">
            Protects data at rest on disk. Does not protect a fully compromised
            PC while unlocked.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="gate">
      <div className="gate-card">
        <div className="logo-mark">S</div>
        <h1>{isSetup ? "Create your vault" : "Unlock SecretSticky"}</h1>
        <p className="muted">
          {isSetup
            ? "Choose a strong master password. Notes are encrypted on disk."
            : "Enter your master password to open encrypted sticky notes."}
        </p>
        <form onSubmit={submit} className="gate-form">
          <label>
            {isSetup
              ? "Master password"
              : useRecovery
                ? "Recovery key"
                : "Master password"}
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
              Confirm password
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
            {busy ? "Working…" : isSetup ? "Create vault" : "Unlock"}
          </button>
        </form>

        {/* Escape hatches — setup has nowhere else to "go back", so offer tray/quit */}
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
              {useRecovery ? "Use master password" : "Use recovery key"}
            </button>
          )}
          <button
            type="button"
            className="ghost"
            disabled={busy}
            title="Hide to tray — open again from the tray icon"
            onClick={() => {
              void api.hideMain();
            }}
          >
            {isSetup ? "Not now — hide to tray" : "Hide to tray"}
          </button>
          <button
            type="button"
            className="ghost danger"
            disabled={busy}
            title="Quit SecretSticky"
            onClick={() => {
              void api.quitApp();
            }}
          >
            Quit
          </button>
        </div>

        <p className="fineprint">
          Local only · Argon2id + XChaCha20-Poly1305 · no cloud
        </p>
      </div>
    </div>
  );
}
