/** Map Tauri/invoke failures into a short user-facing string. */
export function formatInvokeError(err: unknown): string {
  if (err == null) return "Something went wrong";
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message || "Something went wrong";
  if (typeof err === "object") {
    const o = err as Record<string, unknown>;
    if (typeof o.message === "string" && o.message.trim()) return o.message;
    if (typeof o.error === "string" && o.error.trim()) return o.error;
  }
  try {
    return String(err);
  } catch {
    return "Something went wrong";
  }
}

/** True when the backend rejected unlock for a bad password / recovery key. */
export function isBadPasswordError(err: unknown): boolean {
  const msg = formatInvokeError(err).toLowerCase();
  return (
    msg.includes("bad password") ||
    msg.includes("invalid password") ||
    msg.includes("wrong password") ||
    msg.includes("invalid recovery") ||
    msg.includes("bad recovery")
  );
}

/** True when unlock is rate-limited / cooling down. */
export function isRateLimitedError(err: unknown): boolean {
  const msg = formatInvokeError(err).toLowerCase();
  return (
    msg.includes("too many") ||
    msg.includes("try again") ||
    msg.includes("rate limit") ||
    msg.includes("cooldown") ||
    msg.includes("wait")
  );
}
