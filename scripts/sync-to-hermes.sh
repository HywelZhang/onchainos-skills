#!/usr/bin/env bash
# sync-to-hermes.sh — copy this fork's skills/ into the Hermes skills dir (whole-dir copy).
# Why NOT `hermes skills install <url>`: it drops _shared/ and non-references subdirs,
# which breaks cross-skill preflight references. We copy complete skill dirs and verify
# per-skill file counts. Backups land OUTSIDE the scanned skills root so Hermes won't
# index them as skills.
#
# Usage:  bash scripts/sync-to-hermes.sh            # auto-detect Hermes home
#         HERMES_SKILLS_DIR=/path/to/skills bash scripts/sync-to-hermes.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORK_SKILLS="$REPO_ROOT/skills"

if [ -n "${HERMES_SKILLS_DIR:-}" ]; then
  HERMES_SKILLS="$HERMES_SKILLS_DIR"
elif [ -n "${LOCALAPPDATA:-}" ] && [ -d "${LOCALAPPDATA//\\//}/hermes/skills" ]; then
  HERMES_SKILLS="${LOCALAPPDATA//\\//}/hermes/skills"
elif [ -d "$HOME/.local/share/hermes/skills" ]; then
  HERMES_SKILLS="$HOME/.local/share/hermes/skills"
else
  echo "ERROR: cannot locate Hermes skills dir; set HERMES_SKILLS_DIR" >&2
  exit 1
fi

OKX_DEST="$HERMES_SKILLS/okx"
mkdir -p "$OKX_DEST"

TS="$(date +%Y%m%d-%H%M%S)"
if find "$OKX_DEST" -mindepth 2 -maxdepth 2 -name SKILL.md 2>/dev/null | grep -q .; then
  BK="$(dirname "$HERMES_SKILLS")/skills-backup-$TS"
  mkdir -p "$(dirname "$BK")"
  cp -r "$OKX_DEST" "$BK"
  echo "backup: $OKX_DEST -> $BK"
else
  echo "no existing skills found at $OKX_DEST — backup skipped"
fi

FAIL=0
for skill in "$FORK_SKILLS"/*/; do
  name="$(basename "$skill")"
  [ -f "$skill/SKILL.md" ] || continue
  rm -rf "$OKX_DEST/$name"
  cp -r "$skill" "$OKX_DEST/$name"
  src_n="$(find "$skill" -type f | wc -l | tr -d ' ')"
  dst_n="$(find "$OKX_DEST/$name" -type f | wc -l | tr -d ' ')"
  if [ "$src_n" != "$dst_n" ]; then
    echo "MISMATCH $name: src=$src_n dst=$dst_n"
    FAIL=1
  else
    echo "ok  $name ($dst_n files)"
  fi
done

echo "synced: $FORK_SKILLS -> $OKX_DEST"
exit $FAIL
