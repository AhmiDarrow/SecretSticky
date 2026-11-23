#!/usr/bin/env node
// Fails when a tracked text file carries what a public repository must not: an email address
// (other than GitHub's no-reply ones and example domains), a path into someone's home folder,
// a token, an API key, or a private or secret key. Run by CI and `npm run sanitize`.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const SKIP = /\.(png|jpe?g|gif|ico|icns|webp|woff2?|ttf|otf|exe|msi|zip|gz|lock)$|^package-lock\.json$/i;

const CHECKS = [
  {
    what: "an email address",
    re: /[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}/g,
    // File names such as icon@2x.png look like addresses; no-reply and example ones are fine.
    allow: (m) =>
      /@\d+x\.(png|jpe?g|webp|svg)$/i.test(m) ||
      /@users\.noreply\.github\.com$/i.test(m) ||
      /@(example\.(com|org|net)|localhost)$/i.test(m),
  },
  {
    what: "a path into a home folder",
    re: /\b[A-Z]:[\\/]+Users[\\/]+[^\\/\s"'`<>]+|\/home\/[a-z_][a-z0-9_-]*|\/Users\/[A-Za-z][^/\s"'`<>]*/g,
    // Placeholders, as docs write them.
    allow: (m) => /[\\/](<[^>]+>|you|user|username|name|USER|\$USER|\{[^}]+\}|%USERNAME%)$/i.test(m),
  },
  { what: "a GitHub token", re: /\bgh[pousr]_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{30,}/g },
  { what: "an API key", re: /\bsk-[A-Za-z0-9]{20,}|\bAKIA[0-9A-Z]{16}\b|\bxox[baprs]-[A-Za-z0-9-]{10,}/g },
  {
    what: "a private or secret key",
    re: /BEGIN [A-Z ]*PRIVATE KEY|(?:rsign|minisign) encrypted secret key|dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5/g,
  },
];

const files = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" }).split("\0").filter(Boolean);
const problems = [];
for (const file of files) {
  if (SKIP.test(file) || file === "scripts/sanitize-check.mjs") continue;
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    continue;
  }
  if (text.includes("\0")) continue; // binary
  for (const { what, re, allow } of CHECKS) {
    for (const m of text.matchAll(re)) {
      if (allow && allow(m[0])) continue;
      const line = text.slice(0, m.index).split("\n").length;
      problems.push(`${file}:${line}: ${what}`);
    }
  }
}
if (problems.length) {
  // Where, and what kind: never the value itself, so the log does not repeat it.
  console.error(`sanitize check: ${problems.length} problem(s)\n` + problems.join("\n"));
  process.exit(1);
}
console.log(`sanitize check: ${files.length} tracked files clean`);
