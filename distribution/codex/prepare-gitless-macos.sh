#!/bin/sh
# Prepare the already-published preview; official Codex commands perform installation.
# No Git, developer toolchain, configuration edits, OS overrides, or project writes.
set -eu
umask 077

fail() { printf '%s\n' "$*" >&2; exit 1; }
usage() {
  printf '%s\n' 'Usage: /bin/sh prepare-gitless-macos.sh --destination ABSOLUTE_NEW_DIRECTORY [--archive ABSOLUTE_EXISTING_ARCHIVE]'
}
destination=''
archive=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    --destination) [ "$#" -ge 2 ] || fail 'Missing destination'; destination=$2; shift 2 ;;
    --archive) [ "$#" -ge 2 ] || fail 'Missing archive'; archive=$2; shift 2 ;;
    --help) usage; exit 0 ;;
    *) usage >&2; fail 'Unknown argument' ;;
  esac
done
[ "$(/usr/bin/uname -s)" = Darwin ] && [ "$(/usr/bin/uname -m)" = arm64 ] ||
  fail 'This preview is for native macOS Apple Silicon only.'
case "$destination" in /*) ;; *) fail 'An absolute destination is required.' ;; esac
case "$destination" in *'
'*|*/../*|*/./*|*/..|*/.|*/|/) fail 'Use a normalized new directory path without newlines.' ;; esac
[ ! -e "$destination" ] && [ ! -L "$destination" ] ||
  fail 'Destination already exists; nothing was replaced. Choose a new directory.'
parent=${destination%/*}
[ -n "$parent" ] || parent=/
[ -d "$parent" ] || fail 'Create the parent directory first; no installation was attempted.'
cursor=$parent
while [ "$cursor" != / ]; do
  [ ! -L "$cursor" ] || fail 'Destination ancestors must not be symbolic links.'
  cursor=${cursor%/*}
  [ -n "$cursor" ] || cursor=/
done

# Published 0.2.1-preview.6 (unsigned prerelease, 2026-10-27). Changing these pins requires a new reviewed release.
version=0.2.1-preview.6
expected=477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5
expected_bytes=3189503
url=https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.6/gil-codex-macos-arm64-0.2.1-preview.6.tar.gz
stage=$(/usr/bin/mktemp -d /private/tmp/gil-preview-download.XXXXXX)
if [ -z "$archive" ]; then
  # Download to a partial file first; only a complete, hash-verified file is ever extracted.
  /usr/bin/curl --fail --location --show-error --silent --proto '=https' --proto-redir '=https' \
    --connect-timeout 20 --max-time 180 --output "$stage/preview.tar.gz.part" "$url" ||
    fail 'Download failed or was interrupted. Nothing was extracted or registered; retry after checking the connection.'
  /bin/mv "$stage/preview.tar.gz.part" "$stage/preview.tar.gz"
  archive=$stage/preview.tar.gz
else
  case "$archive" in /*) ;; *) fail 'Archive path must be absolute.' ;; esac
  [ -f "$archive" ] && [ ! -L "$archive" ] || fail 'Archive must be a regular file, not a link.'
  /bin/cp "$archive" "$stage/preview.tar.gz"
  archive=$stage/preview.tar.gz
fi
[ -f "$archive" ] && [ ! -L "$archive" ] || fail 'Archive must be a regular file, not a link.'
actual_bytes=$(/usr/bin/stat -f %z "$archive")
[ "$actual_bytes" = "$expected_bytes" ] ||
  fail "Archive size mismatch ($actual_bytes bytes, expected $expected_bytes): incomplete or wrong download. Nothing was extracted or installed."
actual=$(/usr/bin/shasum -a 256 "$archive")
[ "${actual%% *}" = "$expected" ] || fail 'SHA-256 mismatch. Nothing was extracted or installed.'

# The pinned digest verifies these known archive bytes before any extraction.
# It is not an Apple signature or notarization, and does not bypass OS execution checks.
# Safe extraction: inspect members first with the co-located read-only checker (exact names via tar -tzf,
# entry types via the first column of tar -tvzf; file names are never parsed out of the verbose listing).
# This runs only after the pinned size/SHA-256 check above; there is no option to skip that check.
checker=${0%/*}/check-archive-members.sh
[ "$checker" != "$0/check-archive-members.sh" ] || checker=./check-archive-members.sh
[ -f "$checker" ] || fail 'check-archive-members.sh must sit next to this script. Nothing was extracted.'
/bin/sh "$checker" "$archive" || exit 1
/bin/mkdir -m 700 "$destination" || fail 'Could not reserve the new destination; nothing was overwritten.'
# -p restores the reviewed file modes (0644 files, 0755 executable) despite umask 077; ownership is not applied.
/usr/bin/tar -xzpf "$archive" -C "$destination" --no-same-owner ||
  fail 'Extraction failed. The incomplete new directory remains; do not register it.'
/bin/chmod 700 "$destination" # the archive's top-level entry would otherwise widen the root we reserved
[ -f "$destination/.agents/plugins/marketplace.json" ] &&
  [ -f "$destination/release.json" ] &&
  [ -x "$destination/plugins/gil-companion-prototype/core/darwin-arm64/gil" ] ||
  fail 'Prepared files are incomplete. Do not register this directory.'
/usr/bin/grep -q "\"version\": \"$version\"" "$destination/plugins/gil-companion-prototype/.codex-plugin/plugin.json" ||
  fail 'Prepared manifest version differs from the pinned version. Do not register this directory.'
printf 'Verified and prepared unsigned preview %s. NOT installed yet.\n' "$version"
printf 'GIL_MARKETPLACE_ROOT=%s\n' "$destination"
printf '%s\n' 'Keep this directory for the local marketplace. Use official Codex Plugin management next.'
printf '%s\n' 'Do not clear quarantine, disable Gatekeeper, or modify Plugin caches if execution is blocked.'
