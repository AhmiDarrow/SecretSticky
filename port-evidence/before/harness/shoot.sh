#!/usr/bin/env bash
# Renders every screen of the shipped React UI (with mock-tauri.js sample data) to
# port-evidence/before/*.png. Needs `npx vite --port 1422` running in the repo root and
# Microsoft Edge. Pages are laid out at the window size and cropped to it.
set -u
cd "$(dirname "$0")/.."
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
BASE="http://localhost:1422/port-evidence/before/harness/index.html"
shot() {
  local name=$1 w=$2 h=$3 qs=$4
  local ww=$(( w < 600 ? 600 : w ))
  timeout 60 "$EDGE" --headless=old --disable-gpu --hide-scrollbars --user-data-dir="$TEMP/sticky-edge" \
    --window-size=$ww,$h --virtual-time-budget=5000 --screenshot="$(cygpath -w "$PWD")/$name.png" \
    "$BASE?w=$w&h=$h&$qs" >/dev/null 2>&1
  powershell -NoProfile -Command "Add-Type -AssemblyName System.Drawing; \$s=[System.Drawing.Image]::FromFile('$(cygpath -w "$PWD")/$name.png'); \$b=New-Object System.Drawing.Bitmap $w,$h; \$g=[System.Drawing.Graphics]::FromImage(\$b); \$g.DrawImage(\$s,0,0,(New-Object System.Drawing.Rectangle 0,0,$w,$h),[System.Drawing.GraphicsUnit]::Pixel); \$s.Dispose(); \$b.Save('$(cygpath -w "$PWD")/$name.png'); \$b.Dispose()"
  echo "$name"
}
shot 01-setup 440 560 "state=setup"
shot 01-setup-short-password 440 560 "state=setup&fill=short&act=Create%20vault"
shot 02-recovery-key 440 560 "state=setup&fill=1&act=Create%20vault"
shot 03-unlock 440 560 "state=locked"
shot 03-unlock-wrong-password 440 560 "state=locked&fill=1&act=Unlock"
shot 03-unlock-recovery-key 440 560 "state=locked&act=Use%20recovery%20key"
shot 04-manager 440 560 "state=unlocked"
shot 04-manager-empty 440 560 "state=unlocked&empty=1"
shot 04-manager-narrow 400 500 "state=unlocked"
shot 04-manager-wide 900 700 "state=unlocked"
shot 05-about 440 560 "state=unlocked&act=About"
shot 06-password 440 560 "state=unlocked&act=Password"
shot 07-auto-lock 440 560 "state=unlocked&act=Auto-lock"
shot 07-auto-lock-narrow 400 500 "state=unlocked&act=Auto-lock"
shot 08-delete-dialog 440 560 "state=unlocked&act=Delete%20Wi-Fi"
shot 10-note-yellow 345 280 "state=unlocked&note=n1"
shot 10-note-dark-green 345 280 "state=unlocked&note=n2"
shot 10-note-empty-pink 345 280 "state=unlocked&note=n3"
shot 10-note-wide 600 420 "state=unlocked&note=n4"
shot 11-note-locked 345 280 "state=locked&note=n1"
