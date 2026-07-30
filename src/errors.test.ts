import { describe, expect, it } from "vitest";
import {
  formatInvokeError,
  isBadPasswordError,
  isRateLimitedError,
} from "./errors";

describe("formatInvokeError", () => {
  it("handles nullish and primitives", () => {
    expect(formatInvokeError(null)).toBe("Something went wrong");
    expect(formatInvokeError(undefined)).toBe("Something went wrong");
    expect(formatInvokeError("vault locked")).toBe("vault locked");
    expect(formatInvokeError(new Error("boom"))).toBe("boom");
  });

  it("reads message/error fields from objects", () => {
    expect(formatInvokeError({ message: "nope" })).toBe("nope");
    expect(formatInvokeError({ error: "fail" })).toBe("fail");
  });
});

describe("isBadPasswordError", () => {
  it("detects common bad-password phrasings", () => {
    expect(isBadPasswordError("Bad password")).toBe(true);
    expect(isBadPasswordError({ message: "invalid recovery key" })).toBe(true);
    expect(isBadPasswordError("network down")).toBe(false);
  });
});

describe("isRateLimitedError", () => {
  it("detects throttle messaging", () => {
    expect(isRateLimitedError("Too many attempts — try again in 10s")).toBe(
      true,
    );
    expect(isRateLimitedError("please wait")).toBe(true);
    expect(isRateLimitedError("Bad password")).toBe(false);
  });
});

describe("error classifier boundaries", () => {
  it("does not treat rate-limit as bad password", () => {
    const msg = "Too many unlock attempts. Try again in 30 seconds.";
    expect(isRateLimitedError(msg)).toBe(true);
    expect(isBadPasswordError(msg)).toBe(false);
  });

  it("detects backend BadPassword string", () => {
    expect(isBadPasswordError("Bad password")).toBe(true);
    expect(isBadPasswordError("invalid password")).toBe(true);
    expect(isBadPasswordError("wrong password")).toBe(true);
    // "incorrect password" is not in the classifier — document that boundary
    expect(isBadPasswordError("incorrect password")).toBe(false);
  });
});
