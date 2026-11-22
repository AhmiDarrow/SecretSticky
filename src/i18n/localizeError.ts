import { formatNumber } from "./format";
import type { TranslateFn } from "./translate";

const EXACT: Record<string, string> = {
  "vault is locked": "error.vaultLocked",
  "vault already unlocked": "error.alreadyUnlocked",
  "vault not initialized": "error.notInitialized",
  "vault already exists": "error.alreadyExists",
  "note not found": "error.noteNotFound",
  "cannot resolve app data dir": "error.noDataDir",
  "password must be at least 12 characters": "error.passwordShort",
  "no recovery key configured": "error.noRecovery",
  "Something went wrong": "error.somethingWrong",
  "note window cannot access another note": "error.noteAcl",
  "unauthorized window": "error.unauthorized",
  "only the manager window may perform this action": "error.managerOnly",
  "note body only available to its sticky window": "error.bodyOnly",
  "url not allowed": "error.urlNotAllowed",
};

const PREFIXES: ReadonlyArray<readonly [string, string]> = [
  [
    "atomic vault replace failed (saved stickies left untouched): ",
    "error.atomicReplace",
  ],
  ["io: ", "error.io"],
  ["crypto: ", "error.crypto"],
  ["serialize: ", "error.serialize"],
  ["open url: ", "error.openUrl"],
  ["window icon: ", "error.windowIcon"],
  ["window: ", "error.window"],
];

/** Pull a display string out of an invoke rejection without the `Error:` prefix. */
export function errorText(err: unknown): string {
  if (err == null) return "";
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message || "Something went wrong";
  if (typeof err === "object") {
    const record = err as Record<string, unknown>;
    if (typeof record.message === "string" && record.message.trim()) {
      return record.message;
    }
    if (typeof record.error === "string" && record.error.trim()) {
      return record.error;
    }
  }
  return String(err);
}

function isBadPasswordText(lower: string): boolean {
  return (
    lower.includes("invalid master password") ||
    lower.includes("bad password") ||
    lower.includes("invalid password") ||
    lower.includes("wrong password") ||
    lower.includes("invalid recovery") ||
    lower.includes("bad recovery")
  );
}

/**
 * Map known English backend strings to catalog keys.
 * Already-translated text and unknown strings are returned as-is.
 */
export function localizeMessage(
  raw: string,
  locale: string,
  t: TranslateFn,
): string {
  const msg = raw.trim();
  if (!msg) return t("error.somethingWrong");
  const key = EXACT[msg];
  if (key) return t(key);
  if (isBadPasswordText(msg.toLowerCase())) return t("error.badPassword");

  const newer = msg.match(
    /^vault format version (\d+) is newer than this app supports \((\d+)\)/,
  );
  if (newer) {
    return t("error.vaultVersionNewer", {
      version: newer[1],
      supported: newer[2],
    });
  }
  const invalid = msg.match(/^vault format version (\d+) is invalid/);
  if (invalid) return t("error.vaultVersionInvalid", { version: invalid[1] });

  const limited = msg.match(
    /^Too many failed attempts\. Retry in ([0-9.]+)s\./,
  );
  if (limited) {
    const seconds = Number(limited[1]);
    return t("error.tooMany", {
      seconds: formatNumber(locale, Number.isFinite(seconds) ? seconds : 0),
    });
  }

  for (const [prefix, prefixKey] of PREFIXES) {
    if (msg.startsWith(prefix)) {
      return t(prefixKey, { detail: msg.slice(prefix.length) });
    }
  }
  return msg;
}
