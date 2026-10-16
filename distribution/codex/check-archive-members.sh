#!/bin/sh
# Read-only member inspection for a tar.gz archive: refuse absolute paths, parent traversal,
# links, device/special entries and names that cannot be listed unambiguously. Never extracts.
# Called by prepare-gitless-macos.sh AFTER the pinned size/SHA-256 check; usable alone for tests.
set -eu
fail() { printf '%s\n' "$*" >&2; exit 1; }
[ "$#" -eq 1 ] || fail 'Usage: /bin/sh check-archive-members.sh ABSOLUTE_ARCHIVE'
archive=$1
case "$archive" in /*) ;; *) fail 'Archive path must be absolute.' ;; esac
[ -f "$archive" ] && [ ! -L "$archive" ] || fail 'Archive must be a regular file, not a link.'

# Names only (one per line): exact paths including spaces, no field parsing.
names=$(/usr/bin/tar -tzf "$archive") || fail 'Archive listing failed. Nothing was extracted.'
# Verbose listing: only the first character (entry type) is used; file names are not parsed from it.
types=$(/usr/bin/tar -tvzf "$archive" | /usr/bin/cut -c1) || fail 'Archive listing failed. Nothing was extracted.'
name_count=$(printf '%s\n' "$names" | /usr/bin/grep -c '' || true)
type_count=$(printf '%s\n' "$types" | /usr/bin/grep -c '' || true)
[ "$name_count" -gt 0 ] || fail 'Archive has no members; refusing to extract.'
[ "$name_count" = "$type_count" ] ||
  fail 'Archive member names cannot be listed unambiguously (embedded newline?); refusing to extract.'
printf '%s\n' "$types" | while IFS= read -r kind; do
  case "$kind" in -|d) ;; *) fail 'Archive contains links or special files; refusing to extract.' ;; esac
done
printf '%s\n' "$names" | while IFS= read -r name; do
  case "$name" in
    /*) fail 'Archive contains an absolute path; refusing to extract.' ;;
    ..|../*|*/..|*/../*) fail 'Archive contains a parent-traversing path; refusing to extract.' ;;
    '') fail 'Archive contains an empty member name; refusing to extract.' ;;
  esac
done
printf 'Archive members accepted: %s regular files/directories, no links or special entries.\n' "$name_count"
