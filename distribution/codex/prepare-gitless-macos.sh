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

expected=f484c09919c3c58e722ef12a466158bcb478e56bada2e216b7cbbcdfc3102d1c
url=https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.3/gil-codex-macos-arm64-0.2.1-preview.3.tar.gz
if [ -z "$archive" ]; then
  stage=$(/usr/bin/mktemp -d /private/tmp/gil-preview-download.XXXXXX)
  archive=$stage/preview.tar.gz
  /usr/bin/curl --fail --location --show-error --silent --proto '=https' --proto-redir '=https' \
    --connect-timeout 20 --max-time 180 --output "$archive" "$url" ||
    fail 'Download failed. No marketplace was registered; retry after checking the connection.'
else
  case "$archive" in /*) ;; *) fail 'Archive path must be absolute.' ;; esac
  [ -f "$archive" ] && [ ! -L "$archive" ] || fail 'Archive must be a regular file, not a link.'
  stage=$(/usr/bin/mktemp -d /private/tmp/gil-preview-download.XXXXXX)
  /bin/cp "$archive" "$stage/preview.tar.gz"
  archive=$stage/preview.tar.gz
fi
[ -f "$archive" ] && [ ! -L "$archive" ] || fail 'Archive must be a regular file, not a link.'
actual=$(/usr/bin/shasum -a 256 "$archive")
[ "${actual%% *}" = "$expected" ] || fail 'SHA-256 mismatch. Nothing was extracted or installed.'

# The pinned digest verifies these known archive bytes before any extraction.
# It is not an Apple signature or notarization, and does not bypass OS execution checks.
/bin/mkdir -m 700 "$destination" || fail 'Could not reserve the new destination; nothing was overwritten.'
/usr/bin/tar -xzf "$archive" -C "$destination" ||
  fail 'Extraction failed. The incomplete new directory remains; do not register it.'
[ -f "$destination/.agents/plugins/marketplace.json" ] &&
  [ -x "$destination/plugins/gil-companion-prototype/core/darwin-arm64/gil" ] ||
  fail 'Prepared files are incomplete. Do not register this directory.'
printf '%s\n' 'Verified and prepared unsigned preview 0.2.1-preview.3. NOT installed yet.'
printf 'GIL_MARKETPLACE_ROOT=%s\n' "$destination"
printf '%s\n' 'Keep this directory for the local marketplace. Use official Codex Plugin management next.'
printf '%s\n' 'Do not clear quarantine, disable Gatekeeper, or modify Plugin caches if execution is blocked.'
